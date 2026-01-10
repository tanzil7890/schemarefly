use clap::{Parser, Subcommand};
use colored::Colorize;
use anyhow::Result;
use std::path::{Path, PathBuf};

use schemarefly_core::{Report, Config, Diagnostic, DialectConfig};
use schemarefly_dbt::{Manifest, DependencyGraph, ContractExtractor};
use schemarefly_engine::{DriftDetection, StateComparison, StateComparisonResult};
use schemarefly_sql::DbtFunctionExtractor;
use schemarefly_catalog::{WarehouseAdapter, TableIdentifier, BigQueryAdapter, SnowflakeAdapter, SnowflakeAdapterBuilder};

/// SchemaRefly - Schema contract verification for dbt
#[derive(Parser)]
#[command(name = "schemarefly")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to config file (default: schemarefly.toml)
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check schema contracts against inferred schemas
    Check {
        /// Output file for report.json
        #[arg(short, long, default_value = "report.json")]
        output: PathBuf,

        /// Also output markdown report
        #[arg(short, long)]
        markdown: Option<PathBuf>,

        /// Path to production state manifest (for Slim CI mode)
        /// Compares current manifest against this state to find modified models
        #[arg(long, value_name = "PATH")]
        state: Option<PathBuf>,

        /// Only check modified models (requires --state)
        /// Skips unchanged models for faster CI runs
        #[arg(long)]
        modified_only: bool,

        /// Output a concise PR comment to stdout (optimized for GitHub PRs)
        /// Includes collapsible details and summary badge
        #[arg(long)]
        pr_comment: bool,
    },

    /// Initialize SchemaRefly in a dbt project
    Init {
        /// Path to dbt project (default: current directory)
        #[arg(short, long)]
        path: Option<PathBuf>,

        /// SQL dialect to use
        #[arg(short, long, default_value = "bigquery")]
        dialect: String,

        /// Skip creating GitHub workflow
        #[arg(long)]
        skip_workflow: bool,

        /// Force overwrite existing files
        #[arg(short, long)]
        force: bool,
    },

    /// Show downstream impact for a model
    Impact {
        /// Model name to analyze (can be short name or unique_id)
        model: String,

        /// Path to dbt manifest.json
        #[arg(short = 'f', long, default_value = "target/manifest.json")]
        manifest: PathBuf,
    },

    /// Detect schema drift from warehouse
    Drift {
        /// Output file for drift report
        #[arg(short, long, default_value = "drift-report.json")]
        output: PathBuf,
    },

    /// Initialize contracts for existing models (generates YAML stubs)
    InitContracts {
        /// Models to initialize (or all if not specified)
        models: Vec<String>,

        /// Output directory for generated contract YAML files
        #[arg(short, long, default_value = "contracts")]
        output_dir: PathBuf,

        /// Path to dbt manifest.json
        #[arg(short = 'f', long, default_value = "target/manifest.json")]
        manifest: PathBuf,

        /// Path to catalog.json (optional, improves type inference)
        #[arg(long)]
        catalog: Option<PathBuf>,

        /// Overwrite existing contract files
        #[arg(long)]
        force: bool,

        /// Only generate contracts for models with enforced contracts
        #[arg(long)]
        enforced_only: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load config if specified
    let config = if let Some(config_path) = &cli.config {
        Config::from_file(config_path)?
    } else if std::path::Path::new("schemarefly.toml").exists() {
        Config::from_file(std::path::Path::new("schemarefly.toml"))?
    } else {
        if cli.verbose {
            eprintln!("{}", "No config file found, using defaults".yellow());
        }
        Config::default()
    };

    if cli.verbose {
        eprintln!("{} dialect: {:?}", "Using".cyan(), config.dialect);
    }

    match cli.command {
        Commands::Check { output, markdown, state, modified_only, pr_comment } => {
            check_command(&config, &output, markdown.as_ref().map(|v| v.as_path()), state.as_ref(), modified_only, pr_comment, cli.verbose)
        }
        Commands::Init { path, dialect, skip_workflow, force } => {
            init_command(path.as_ref(), &dialect, skip_workflow, force, cli.verbose)
        }
        Commands::Impact { model, manifest } => {
            impact_command(&config, &model, &manifest, cli.verbose)
        }
        Commands::Drift { output } => {
            drift_command(&config, &output, cli.verbose).await
        }
        Commands::InitContracts { models, output_dir, manifest, catalog, force, enforced_only } => {
            init_contracts_command(&config, &models, &output_dir, &manifest, catalog.as_ref(), force, enforced_only, cli.verbose)
        }
    }
}

/// Check command - validate schema contracts (with Salsa incremental computation)
fn check_command(
    config: &Config,
    output: &PathBuf,
    markdown: Option<&Path>,
    state_path: Option<&PathBuf>,
    modified_only: bool,
    pr_comment: bool,
    verbose: bool,
) -> Result<()> {
    use schemarefly_incremental::{SchemaReflyDatabase, queries};

    // Validate flags
    if modified_only && state_path.is_none() {
        return Err(anyhow::anyhow!(
            "--modified-only requires --state <path> to be specified"
        ));
    }

    let is_slim_ci = state_path.is_some();

    if verbose {
        if is_slim_ci {
            eprintln!("{}", "Running Slim CI schema contract checks...".cyan().bold());
        } else {
            eprintln!("{}", "Running schema contract checks (with incremental computation)...".cyan());
        }
    }

    // Find manifest path
    let manifest_path = Path::new("target/manifest.json");
    if !manifest_path.exists() {
        return Err(anyhow::anyhow!(
            "Manifest not found at {}. Run 'dbt compile' or 'dbt build' first.",
            manifest_path.display()
        ));
    }

    if verbose {
        eprintln!("{} {}", "Loading manifest from:".cyan(), manifest_path.display());
    }

    // Initialize Salsa database for incremental computation
    let db = SchemaReflyDatabase::default();

    // Read manifest JSON
    let manifest_json = std::fs::read_to_string(manifest_path)?;

    // Create Salsa inputs
    let manifest_input = queries::ManifestInput::new(&db, manifest_json.clone());
    let config_input = queries::ConfigInput::new(&db, config.clone());

    if verbose {
        eprintln!("{}", "Building dependency graph...".cyan());
    }

    // Get manifest from Salsa (cached)
    let manifest_opt = queries::manifest(&db, manifest_input);
    let manifest = manifest_opt.ok_or_else(|| anyhow::anyhow!("Failed to parse manifest"))?;

    // Build dependency graph for impact analysis
    let dag = DependencyGraph::from_manifest(&manifest);

    // Slim CI: Compare against state manifest if provided
    let state_comparison = if let Some(state_manifest_path) = state_path {
        if !state_manifest_path.exists() {
            return Err(anyhow::anyhow!(
                "State manifest not found at {}",
                state_manifest_path.display()
            ));
        }

        if verbose {
            eprintln!("{} {}", "Loading state manifest from:".cyan(), state_manifest_path.display());
        }

        let state_manifest = Manifest::from_file(state_manifest_path)
            .map_err(|e| anyhow::anyhow!("Failed to load state manifest: {}", e))?;

        let comparison = StateComparison::compare(&manifest, &state_manifest);

        if verbose {
            eprintln!();
            eprintln!("{}", "=".repeat(60).bright_blue());
            eprintln!("{}", "Slim CI State Comparison".bold().bright_blue());
            eprintln!("{}", "=".repeat(60).bright_blue());
            eprintln!();
            eprintln!("  {} {}", "Modified models:".bold(), comparison.modified_models.len());
            eprintln!("  {} {}", "New models:".bold(), comparison.new_models.len());
            eprintln!("  {} {}", "Deleted models:".bold(), comparison.deleted_models.len());
            eprintln!("  {} {}", "Total blast radius:".bold(), comparison.total_blast_radius);
            eprintln!();

            if !comparison.modified_models.is_empty() {
                eprintln!("{}", "Modified models and their downstream impact:".bold());
                for modified in &comparison.modified_models {
                    let reasons: Vec<String> = modified.reasons.iter().map(|r| r.to_string()).collect();
                    eprintln!("  {} {} ({})", "→".yellow(), modified.name.yellow(), reasons.join(", "));
                    if modified.downstream_count > 0 {
                        eprintln!("    {} downstream models affected", modified.downstream_count.to_string().red());
                    }
                }
                eprintln!();
            }
        }

        Some(comparison)
    } else {
        None
    };

    // Get set of models to check (filter by modified if --modified-only)
    let models_to_check: std::collections::HashSet<String> = if modified_only {
        if let Some(ref comparison) = state_comparison {
            comparison.all_affected_models.clone()
        } else {
            std::collections::HashSet::new()
        }
    } else {
        // Check all models
        manifest.models().keys().cloned().collect()
    };

    if verbose {
        if modified_only {
            eprintln!("{} {} models (modified + downstream)...", "Checking".cyan(), models_to_check.len());
        } else {
            eprintln!("{}", "Checking contracts for all models...".cyan());
        }
    }

    // Collect diagnostics from all contract checks
    let mut all_diagnostics = Vec::new();
    let mut checked_models = 0;
    let mut models_with_contracts = 0;
    let mut skipped_models = 0;

    // Check each model with a contract
    for (node_id, node) in manifest.models() {
        // Skip if not in models_to_check (for Slim CI)
        if modified_only && !models_to_check.contains(&node_id) {
            skipped_models += 1;
            continue;
        }

        // Extract contract if present
        if let Some(_contract) = ContractExtractor::extract_from_node(node) {
            models_with_contracts += 1;

            if verbose {
                // Show if model is modified in Slim CI mode
                let modified_indicator = if let Some(ref comparison) = state_comparison {
                    if comparison.modified_model_ids().contains(&node_id) {
                        " [MODIFIED]".yellow().to_string()
                    } else if comparison.all_affected_models.contains(&node_id) {
                        " [DOWNSTREAM]".cyan().to_string()
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                eprintln!("  {} {}{}...", "Checking".cyan(), node.name, modified_indicator);
            }

            // Find SQL file path
            let sql_path = Path::new(&node.original_file_path);

            // If the path is relative, try to resolve it from project root
            let full_sql_path = if sql_path.is_relative() {
                // Try common dbt project structures
                let candidates = vec![
                    sql_path.to_path_buf(),
                    Path::new("models").join(sql_path),
                    PathBuf::from(&node.original_file_path),
                ];

                candidates.into_iter().find(|p| p.exists())
            } else {
                Some(sql_path.to_path_buf())
            };

            let Some(sql_file_path) = full_sql_path else {
                let diag = Diagnostic::new(
                    schemarefly_core::DiagnosticCode::SqlParseError,
                    schemarefly_core::Severity::Error,
                    format!("SQL file not found: {}", node.original_file_path),
                );
                all_diagnostics.push(diag);
                continue;
            };

            // Read SQL file
            let sql_content = match std::fs::read_to_string(&sql_file_path) {
                Ok(content) => content,
                Err(e) => {
                    let diag = Diagnostic::new(
                        schemarefly_core::DiagnosticCode::SqlParseError,
                        schemarefly_core::Severity::Error,
                        format!("Failed to read SQL file {}: {}", sql_file_path.display(), e),
                    );
                    all_diagnostics.push(diag);
                    continue;
                }
            };

            // Preprocess dbt template functions
            let (preprocessed_sql, _) = DbtFunctionExtractor::preprocess(&sql_content, Some(&manifest));

            // Create Salsa input for this SQL file (enables caching per file)
            let sql_file = queries::SqlFile::new(&db, sql_file_path.clone(), preprocessed_sql);

            // Use Salsa to check contract (cached if file unchanged)
            // This will automatically call parse_sql -> infer_schema -> compare
            let diagnostics = queries::check_contract(&db, sql_file, config_input, manifest_input);

            // Add downstream impact to each diagnostic
            let downstream = dag.downstream(&node_id);
            let has_errors = diagnostics.iter().any(|d| d.severity == schemarefly_core::Severity::Error);
            let has_warnings = diagnostics.iter().any(|d| d.severity == schemarefly_core::Severity::Warn);
            let error_count = diagnostics.iter().filter(|d| d.severity == schemarefly_core::Severity::Error).count();
            let warning_count = diagnostics.iter().filter(|d| d.severity == schemarefly_core::Severity::Warn).count();

            for mut diag in diagnostics {
                diag.impact = downstream.clone();
                all_diagnostics.push(diag);
            }

            checked_models += 1;

            if verbose && has_errors {
                eprintln!("    {} errors found", error_count.to_string().red());
            } else if verbose && has_warnings {
                eprintln!("    {} warnings", warning_count.to_string().yellow());
            } else if verbose {
                eprintln!("    {}", "✓ OK".green());
            }
        }
    }

    if verbose {
        eprintln!();
        eprintln!(
            "Checked {} models ({} with contracts)",
            checked_models, models_with_contracts
        );
        if modified_only {
            eprintln!("Skipped {} unchanged models", skipped_models);
        }
    }

    // Build report with diagnostics
    let mut report = Report::from_diagnostics(all_diagnostics);

    // Add Slim CI metadata if state comparison was performed
    if let Some(ref comparison) = state_comparison {
        let slim_ci_metadata = serde_json::json!({
            "slim_ci": {
                "enabled": true,
                "modified_only": modified_only,
                "modified_models": comparison.modified_models.iter().map(|m| {
                    serde_json::json!({
                        "unique_id": m.unique_id,
                        "name": m.name,
                        "reasons": m.reasons.iter().map(|r| r.to_string()).collect::<Vec<_>>(),
                        "downstream_count": m.downstream_count,
                        "downstream_models": m.downstream_impact
                    })
                }).collect::<Vec<_>>(),
                "new_models": comparison.new_models,
                "deleted_models": comparison.deleted_models,
                "total_blast_radius": comparison.total_blast_radius,
                "models_checked": checked_models,
                "models_skipped": skipped_models
            }
        });
        report.metadata = Some(slim_ci_metadata);
    }

    // Save JSON report
    report.save_to_file(output)?;

    if verbose {
        eprintln!("{} {}", "Report saved to:".green(), output.display());
    }

    // Save markdown report if requested
    if let Some(md_path) = markdown {
        let markdown_content = generate_markdown_report(&report, state_comparison.as_ref());
        std::fs::write(md_path, markdown_content)?;
        if verbose {
            eprintln!("{} {}", "Markdown report saved to:".green(), md_path.display());
        }
    }

    // Output PR comment if requested
    if pr_comment {
        let pr_markdown = generate_pr_comment(&report, state_comparison.as_ref());
        println!("{}", pr_markdown);
    } else {
        // Print summary (only if not in PR comment mode)
        print_report_summary(&report);
    }

    // Exit with error code if there are errors
    if report.has_errors() {
        std::process::exit(1);
    }

    Ok(())
}

/// Impact command - show downstream dependencies
fn impact_command(_config: &Config, model: &str, manifest_path: &PathBuf, verbose: bool) -> Result<()> {
    if verbose {
        eprintln!("{} {}", "Loading manifest from:".cyan(), manifest_path.display());
    }

    // Load manifest
    let manifest = Manifest::from_file(manifest_path)
        .map_err(|e| anyhow::anyhow!("Failed to load manifest: {}", e))?;

    if verbose {
        eprintln!("{}", "Building dependency graph...".cyan());
    }

    // Build dependency graph
    let dag = DependencyGraph::from_manifest(&manifest);

    // Find the node (support both short name and unique_id)
    let node_id = find_node_id(&manifest, model)?;

    if verbose {
        eprintln!("{} {}", "Analyzing impact for:".cyan(), node_id);
    }

    // Get downstream dependencies
    let downstream = dag.downstream(&node_id);

    // Print results
    println!("\n{}", "=".repeat(60).bright_blue());
    println!("{}", "Downstream Impact Analysis".bold().bright_blue());
    println!("{}", "=".repeat(60).bright_blue());
    println!();

    println!("{} {}", "Model:".bold(), node_id.green());
    println!("{} {}", "Downstream models:".bold(), downstream.len());
    println!();

    if downstream.is_empty() {
        println!("{}", "✓ No downstream dependencies".green());
        println!("This model can be modified without affecting other models.");
    } else {
        println!("{}", "Affected models (in dependency order):".bold());
        println!();

        for (i, dep) in downstream.iter().enumerate() {
            // Try to get model info
            let model_info = manifest.get_node(dep)
                .map(|n| format!("{} ({})", dep, n.resource_type))
                .unwrap_or_else(|| dep.clone());

            println!("  {}. {}", i + 1, model_info.yellow());
        }

        println!();
        println!("{}", "⚠ Changes to this model may break downstream models!".yellow().bold());
    }

    println!();
    println!("{}", "=".repeat(60).bright_blue());

    Ok(())
}

/// Find node ID from short name or unique_id
fn find_node_id(manifest: &Manifest, name: &str) -> Result<String> {
    // If it's already a unique_id (contains dots), use it directly
    if name.contains('.') {
        if manifest.get_node(name).is_some() || manifest.get_source(name).is_some() {
            return Ok(name.to_string());
        }
    }

    // Otherwise, search for matching model name
    for (node_id, node) in manifest.models() {
        if node.name == name {
            return Ok(node_id.clone());
        }
    }

    // Also check sources
    for (source_id, source) in &manifest.sources {
        if source.name == name {
            return Ok(source_id.clone());
        }
    }

    Err(anyhow::anyhow!(
        "Model '{}' not found in manifest. Try using the full unique_id (e.g., 'model.project.{}')",
        name,
        name
    ))
}

/// Drift command - detect warehouse schema changes
async fn drift_command(config: &Config, output: &PathBuf, verbose: bool) -> Result<()> {
    if verbose {
        eprintln!("{}", "Detecting schema drift...".cyan());
    }

    // Check warehouse configuration
    let warehouse_config = config.warehouse.as_ref()
        .ok_or_else(|| anyhow::anyhow!(
            "No warehouse configuration found in schemarefly.toml. \
             Add a [warehouse] section with type and connection settings."
        ))?;

    // Find manifest path
    let manifest_path = Path::new("target/manifest.json");
    if !manifest_path.exists() {
        return Err(anyhow::anyhow!(
            "Manifest not found at {}. Run 'dbt compile' or 'dbt build' first.",
            manifest_path.display()
        ));
    }

    if verbose {
        eprintln!("{} {}", "Loading manifest from:".cyan(), manifest_path.display());
    }

    // Load manifest
    let manifest = Manifest::from_file(manifest_path)?;

    // Create warehouse adapter based on config
    if verbose {
        eprintln!("{} {}...", "Connecting to".cyan(), warehouse_config.warehouse_type);
    }

    let adapter: Box<dyn WarehouseAdapter> = match warehouse_config.warehouse_type.to_lowercase().as_str() {
        "bigquery" => {
            let project_id = warehouse_config.settings.get("project_id")
                .ok_or_else(|| anyhow::anyhow!("BigQuery requires 'project_id' in warehouse settings"))?;

            if let Some(credentials) = warehouse_config.settings.get("credentials") {
                Box::new(BigQueryAdapter::from_service_account_json(project_id, credentials).await?)
            } else {
                Box::new(BigQueryAdapter::with_adc(project_id).await?)
            }
        }
        "snowflake" => {
            let account = warehouse_config.settings.get("account")
                .ok_or_else(|| anyhow::anyhow!("Snowflake requires 'account' in warehouse settings"))?;
            let username = warehouse_config.settings.get("username")
                .ok_or_else(|| anyhow::anyhow!("Snowflake requires 'username' in warehouse settings"))?;
            let password = warehouse_config.settings.get("password")
                .ok_or_else(|| anyhow::anyhow!("Snowflake requires 'password' in warehouse settings"))?;

            let mut builder = SnowflakeAdapterBuilder::with_password(account, username, password);

            if let Some(warehouse) = warehouse_config.settings.get("warehouse") {
                builder = builder.with_warehouse(warehouse);
            }
            if let Some(role) = warehouse_config.settings.get("role") {
                builder = builder.with_role(role);
            }

            Box::new(builder.build()?)
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported warehouse type '{}'. Supported: bigquery, snowflake",
                warehouse_config.warehouse_type
            ));
        }
    };

    // Test connection
    if verbose {
        eprintln!("{}", "Testing warehouse connection...".cyan());
    }

    adapter.test_connection().await
        .map_err(|e| anyhow::anyhow!("Failed to connect to warehouse: {}", e))?;

    if verbose {
        eprintln!("{}", "✓ Connection successful".green());
        eprintln!("{}", "Checking models with contracts...".cyan());
    }

    // Collect drift detections for all models with contracts
    let mut all_drift_detections = Vec::new();
    let mut checked_models = 0;
    let mut models_with_drift = 0;

    // Check each model with a contract
    for (node_id, node) in manifest.models() {
        if let Some(contract) = ContractExtractor::extract_from_node(node) {
            if verbose {
                eprintln!("  {} {}...", "Checking".cyan(), node.name);
            }

            // Parse table identifier from node
            // Format: database.schema.table
            let table_id = parse_table_identifier(&node_id, &node.database, &node.schema, &node.name)?;

            // Fetch actual schema from warehouse
            let actual_schema = match adapter.fetch_schema(&table_id).await {
                Ok(schema) => schema,
                Err(e) => {
                    if verbose {
                        eprintln!("    {} {}", "⚠ Warning:".yellow(), e);
                    }
                    // Skip this model if we can't fetch its schema
                    continue;
                }
            };

            // Compare expected (contract) vs actual (warehouse)
            let drift = DriftDetection::detect(
                node_id,
                &contract.schema,
                &actual_schema,
                Some(node.original_file_path.clone()),
            );

            let has_errors = drift.has_errors();
            let has_warnings = drift.has_warnings();
            let has_info = drift.has_info();

            if has_errors || has_warnings || has_info {
                models_with_drift += 1;
            }

            checked_models += 1;

            if verbose {
                if has_errors {
                    eprintln!("    {} {} drift errors", "✗".red(), drift.error_count());
                } else if has_warnings {
                    eprintln!("    {} {} drift warnings", "⚠".yellow(), drift.warning_count());
                } else if has_info {
                    eprintln!("    {} {} informational drifts", "ℹ".cyan(), drift.info_count());
                } else {
                    eprintln!("    {}", "✓ No drift".green());
                }
            }

            all_drift_detections.push(drift);
        }
    }

    if verbose {
        eprintln!();
        eprintln!(
            "Checked {} models, {} with drift detected",
            checked_models, models_with_drift
        );
    }

    // Collect all diagnostics from drift detections
    let all_diagnostics: Vec<Diagnostic> = all_drift_detections
        .iter()
        .flat_map(|d| d.diagnostics.clone())
        .collect();

    // Build drift report
    let report = Report::from_diagnostics(all_diagnostics);

    // Save JSON report
    report.save_to_file(output)?;

    if verbose {
        eprintln!("{} {}", "Drift report saved to:".green(), output.display());
    }

    // Print summary
    print_drift_summary(&report, checked_models, models_with_drift);

    // Exit with error code if there are errors
    if report.has_errors() {
        std::process::exit(1);
    }

    Ok(())
}

/// Parse table identifier from dbt node information
fn parse_table_identifier(
    _node_id: &str,
    database: &Option<String>,
    schema: &Option<String>,
    table: &str,
) -> Result<TableIdentifier> {
    let db = database.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Model missing database information"))?
        .clone();

    let sch = schema.as_ref()
        .ok_or_else(|| anyhow::anyhow!("Model missing schema information"))?
        .clone();

    Ok(TableIdentifier {
        database: db,
        schema: sch,
        table: table.to_string(),
    })
}

/// Print drift detection summary
fn print_drift_summary(report: &Report, checked_models: usize, models_with_drift: usize) {
    println!("\n{}", "=".repeat(60).bright_blue());
    println!("{}", "Schema Drift Detection Report".bold().bright_blue());
    println!("{}", "=".repeat(60).bright_blue());
    println!();

    println!("Models checked: {}", checked_models);
    println!("Models with drift: {}", models_with_drift);
    println!();

    println!("{}", "Summary:".bold());
    println!("  Total drift diagnostics: {}", report.summary.total);

    if report.summary.errors > 0 {
        println!("  Errors:   {}", format!("{}", report.summary.errors).red().bold());
    } else {
        println!("  Errors:   {}", format!("{}", report.summary.errors).green());
    }

    if report.summary.warnings > 0 {
        println!("  Warnings: {}", format!("{}", report.summary.warnings).yellow());
    } else {
        println!("  Warnings: {}", format!("{}", report.summary.warnings).green());
    }

    println!("  Info:     {}", report.summary.info);
    println!();

    if report.diagnostics.is_empty() {
        println!("{}", "✓ No drift detected!".green().bold());
    } else {
        println!("{}", "Drift Details:".bold());
        for diag in &report.diagnostics {
            let severity_str = match diag.severity {
                schemarefly_core::Severity::Error => "ERROR".red().bold(),
                schemarefly_core::Severity::Warn => "WARN".yellow().bold(),
                schemarefly_core::Severity::Info => "INFO".cyan(),
            };

            println!("  [{}] {}: {}", severity_str, diag.code, diag.message);

            if let Some(loc) = &diag.location {
                println!("    Location: {}", loc.file);
            }

            if let Some(exp) = &diag.expected {
                println!("    Expected: {}", exp);
            }
            if let Some(act) = &diag.actual {
                println!("    Actual:   {}", act);
            }
        }
    }

    println!();
    println!("{}", "=".repeat(60).bright_blue());
}

/// Init command - initialize SchemaRefly in a dbt project
fn init_command(
    path: Option<&PathBuf>,
    dialect: &str,
    skip_workflow: bool,
    force: bool,
    verbose: bool,
) -> Result<()> {
    let project_path = path.map(|p| p.clone()).unwrap_or_else(|| PathBuf::from("."));

    if verbose {
        eprintln!("{} {}...", "Initializing SchemaRefly in".cyan(), project_path.display());
    }

    // Detect dbt project
    let dbt_project_path = project_path.join("dbt_project.yml");
    if !dbt_project_path.exists() {
        return Err(anyhow::anyhow!(
            "No dbt_project.yml found at {}. Please run this command from a dbt project root.",
            project_path.display()
        ));
    }

    println!("{}", "Detected dbt project".green());

    // Validate dialect
    let dialect_enum = match dialect.to_lowercase().as_str() {
        "bigquery" => DialectConfig::BigQuery,
        "snowflake" => DialectConfig::Snowflake,
        "postgres" | "postgresql" => DialectConfig::Postgres,
        "ansi" => DialectConfig::Ansi,
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported dialect '{}'. Supported: bigquery, snowflake, postgres, ansi",
                dialect
            ));
        }
    };

    // Create schemarefly.toml
    let config_path = project_path.join("schemarefly.toml");
    if config_path.exists() && !force {
        println!("{} schemarefly.toml already exists (use --force to overwrite)", "Skipping:".yellow());
    } else {
        let config_content = generate_config_template(dialect_enum);
        std::fs::write(&config_path, config_content)?;
        println!("{} schemarefly.toml", "Created:".green());
    }

    // Create GitHub workflow (unless --skip-workflow)
    if !skip_workflow {
        let workflow_dir = project_path.join(".github").join("workflows");
        let workflow_path = workflow_dir.join("schemarefly.yml");

        if workflow_path.exists() && !force {
            println!("{} .github/workflows/schemarefly.yml already exists (use --force to overwrite)", "Skipping:".yellow());
        } else {
            std::fs::create_dir_all(&workflow_dir)?;
            let workflow_content = generate_workflow_template();
            std::fs::write(&workflow_path, workflow_content)?;
            println!("{} .github/workflows/schemarefly.yml", "Created:".green());
        }
    }

    // Print next steps
    println!();
    println!("{}", "=".repeat(60).bright_blue());
    println!("{}", "SchemaRefly initialized successfully!".bold().green());
    println!("{}", "=".repeat(60).bright_blue());
    println!();
    println!("{}", "Next steps:".bold());
    println!("  1. Run {} to compile your dbt project", "dbt compile".cyan());
    println!("  2. Run {} to check contracts", "schemarefly check".cyan());
    println!("  3. (Optional) Run {} to generate contract stubs", "schemarefly init-contracts".cyan());
    println!();
    println!("{}", "For CI integration:".bold());
    println!("  The generated workflow will run schema checks on every PR.");
    println!("  Commit .github/workflows/schemarefly.yml to enable.");
    println!();

    Ok(())
}

/// Generate schemarefly.toml template
fn generate_config_template(dialect: DialectConfig) -> String {
    let dialect_str = match dialect {
        DialectConfig::BigQuery => "bigquery",
        DialectConfig::Snowflake => "snowflake",
        DialectConfig::Postgres => "postgres",
        DialectConfig::Ansi => "ansi",
    };

    format!(r#"# SchemaRefly Configuration
# See https://github.com/owner/schemarefly for documentation

# SQL dialect for your dbt project
dialect = "{dialect_str}"

# Severity overrides for specific diagnostic codes
# Uncomment to change default severities
[severity.overrides]
# CONTRACT_EXTRA_COLUMN = "warn"
# SQL_SELECT_STAR_UNEXPANDABLE = "info"

# Allowlist rules (glob patterns)
[allowlist]
# Allow type widening for specific models
allow_widening = [
    # "staging.*"
]

# Allow extra columns for specific models
allow_extra_columns = [
    # "staging.*"
]

# Skip checks entirely for specific models
skip_models = [
    # "temp_*",
    # "test_*"
]

# Warehouse connection (for drift detection)
# Uncomment and configure for your warehouse
# [warehouse]
# type = "{dialect_str}"
#
# [warehouse.settings]
# # BigQuery settings
# # project_id = "your-project"
# # credentials = "path/to/credentials.json"  # Or use ADC (Application Default Credentials)
#
# # Snowflake settings
# # account = "your-account"
# # username = "your-username"
# # password = "${{SNOWFLAKE_PASSWORD}}"  # Use environment variable
# # warehouse = "your-warehouse"
# # role = "your-role"
"#)
}

/// Generate GitHub workflow template
fn generate_workflow_template() -> String {
    r#"# SchemaRefly - Schema Contract Verification
# This workflow validates dbt schema contracts on every PR

name: Schema Contracts

on:
  pull_request:
    branches: [main, master, develop]
    paths:
      - 'models/**'
      - 'dbt_project.yml'
      - 'schemarefly.toml'
  push:
    branches: [main, master]

env:
  DBT_PROFILES_DIR: ${{ github.workspace }}

jobs:
  schema-check:
    name: Check Schema Contracts
    runs-on: ubuntu-latest

    steps:
      - name: Checkout repository
        uses: actions/checkout@v4
        with:
          fetch-depth: 0  # Needed for state comparison

      - name: Setup Python
        uses: actions/setup-python@v5
        with:
          python-version: '3.11'

      - name: Install dbt
        run: pip install dbt-core dbt-bigquery  # Adjust for your adapter

      - name: Install SchemaRefly
        run: |
          # Download latest release
          curl -fsSL https://github.com/owner/schemarefly/releases/latest/download/schemarefly-x86_64-unknown-linux-gnu.tar.gz | tar -xz
          sudo mv schemarefly-*/schemarefly /usr/local/bin/

      - name: Compile dbt project
        run: dbt compile

      # For PRs: Compare against main branch (Slim CI)
      - name: Get production state
        if: github.event_name == 'pull_request'
        run: |
          git checkout origin/${{ github.base_ref }} -- target/manifest.json || true
          if [ -f target/manifest.json ]; then
            mv target/manifest.json prod-manifest.json
            git checkout ${{ github.sha }} -- target/manifest.json
            dbt compile  # Recompile current branch
          fi

      - name: Check contracts (PR - Slim CI)
        if: github.event_name == 'pull_request' && hashFiles('prod-manifest.json') != ''
        run: |
          schemarefly check \
            --state prod-manifest.json \
            --modified-only \
            --pr-comment \
            > pr-comment.md

      - name: Check contracts (Full)
        if: github.event_name != 'pull_request' || hashFiles('prod-manifest.json') == ''
        run: schemarefly check --pr-comment > pr-comment.md

      - name: Comment on PR
        if: github.event_name == 'pull_request'
        uses: actions/github-script@v7
        with:
          script: |
            const fs = require('fs');
            const comment = fs.readFileSync('pr-comment.md', 'utf8');

            // Find existing comment
            const { data: comments } = await github.rest.issues.listComments({
              owner: context.repo.owner,
              repo: context.repo.repo,
              issue_number: context.issue.number,
            });

            const botComment = comments.find(c =>
              c.user.type === 'Bot' &&
              c.body.includes('<!-- schemarefly-report -->')
            );

            if (botComment) {
              await github.rest.issues.updateComment({
                owner: context.repo.owner,
                repo: context.repo.repo,
                comment_id: botComment.id,
                body: comment
              });
            } else {
              await github.rest.issues.createComment({
                owner: context.repo.owner,
                repo: context.repo.repo,
                issue_number: context.issue.number,
                body: comment
              });
            }
"#.to_string()
}

/// Init contracts command - generate contracts from current schemas
fn init_contracts_command(
    _config: &Config,
    models: &[String],
    output_dir: &PathBuf,
    manifest_path: &PathBuf,
    catalog_path: Option<&PathBuf>,
    force: bool,
    enforced_only: bool,
    verbose: bool,
) -> Result<()> {
    if verbose {
        eprintln!("{}", "Generating contract stubs...".cyan());
    }

    // Load manifest
    if !manifest_path.exists() {
        return Err(anyhow::anyhow!(
            "Manifest not found at {}. Run 'dbt compile' first.",
            manifest_path.display()
        ));
    }

    let manifest = Manifest::from_file(manifest_path)
        .map_err(|e| anyhow::anyhow!("Failed to load manifest: {}", e))?;

    if verbose {
        eprintln!("{} {}", "Loaded manifest from:".cyan(), manifest_path.display());
    }

    // Load catalog if provided (for better type inference)
    let catalog_data: Option<serde_json::Value> = if let Some(cat_path) = catalog_path {
        if cat_path.exists() {
            let content = std::fs::read_to_string(cat_path)?;
            let parsed: serde_json::Value = serde_json::from_str(&content)?;
            if verbose {
                eprintln!("{} {}", "Loaded catalog from:".cyan(), cat_path.display());
            }
            Some(parsed)
        } else {
            if verbose {
                eprintln!("{} {} (will use SQL inference)", "Catalog not found:".yellow(), cat_path.display());
            }
            None
        }
    } else {
        None
    };

    // Create output directory
    std::fs::create_dir_all(output_dir)?;

    // Filter models to process
    let all_models = manifest.models();
    let models_to_process: Vec<_> = all_models
        .iter()
        .filter(|(node_id, node)| {
            // Filter by model names if provided
            if !models.is_empty() {
                let matches = models.iter().any(|m| {
                    node.name == *m || node_id.contains(m)
                });
                if !matches {
                    return false;
                }
            }

            // Filter by enforced_only
            if enforced_only {
                if let Some(contract) = &node.config.contract {
                    return contract.enforced;
                }
                return false;
            }

            true
        })
        .collect();

    if models_to_process.is_empty() {
        println!("{}", "No models found matching the criteria.".yellow());
        return Ok(());
    }

    println!("{} {} models to generate contracts for", "Found:".green(), models_to_process.len());

    let mut generated = 0;
    let mut skipped = 0;

    for (node_id, node) in models_to_process {
        let contract_file = output_dir.join(format!("{}.yml", node.name));

        if contract_file.exists() && !force {
            if verbose {
                eprintln!("  {} {} (exists)", "Skipping:".yellow(), node.name);
            }
            skipped += 1;
            continue;
        }

        // Generate contract YAML
        let contract_yaml = generate_contract_yaml(node_id, node, &catalog_data, &manifest)?;

        // Write to file
        std::fs::write(&contract_file, contract_yaml)?;

        if verbose {
            eprintln!("  {} {}", "Generated:".green(), contract_file.display());
        }

        generated += 1;
    }

    // Print summary
    println!();
    println!("{}", "=".repeat(60).bright_blue());
    println!("{}", "Contract Generation Complete".bold().green());
    println!("{}", "=".repeat(60).bright_blue());
    println!();
    println!("Generated: {} contracts", generated);
    println!("Skipped:   {} (already exist)", skipped);
    println!();
    println!("{}", "Next steps:".bold());
    println!("  1. Review generated contracts in {}/", output_dir.display());
    println!("  2. Copy relevant sections to your dbt schema.yml files");
    println!("  3. Run {} to validate", "schemarefly check".cyan());
    println!();

    Ok(())
}

/// Generate contract YAML for a model
fn generate_contract_yaml(
    node_id: &str,
    node: &schemarefly_dbt::ManifestNode,
    catalog_data: &Option<serde_json::Value>,
    manifest: &Manifest,
) -> Result<String> {
    let mut yaml = String::new();

    yaml.push_str(&format!("# Generated contract for {}\n", node.name));
    yaml.push_str(&format!("# Model: {}\n", node_id));
    yaml.push_str(&format!("# Path: {}\n", node.original_file_path));
    yaml.push_str("\n");
    yaml.push_str("# Copy this to your schema.yml file under the model definition\n");
    yaml.push_str("# See: https://docs.getdbt.com/docs/collaborate/govern/model-contracts\n");
    yaml.push_str("\n");

    yaml.push_str(&format!("- name: {}\n", node.name));
    yaml.push_str("  config:\n");
    yaml.push_str("    contract:\n");
    yaml.push_str("      enforced: true\n");
    yaml.push_str("  columns:\n");

    // Try to get columns from catalog first
    let columns = get_columns_for_model(node_id, node, catalog_data, manifest);

    for (col_name, col_type, description) in columns {
        yaml.push_str(&format!("    - name: {}\n", col_name));
        yaml.push_str(&format!("      data_type: {}\n", col_type));
        if let Some(desc) = description {
            yaml.push_str(&format!("      description: \"{}\"\n", desc));
        }
    }

    Ok(yaml)
}

/// Get columns for a model from catalog or SQL inference
fn get_columns_for_model(
    node_id: &str,
    node: &schemarefly_dbt::ManifestNode,
    catalog_data: &Option<serde_json::Value>,
    manifest: &Manifest,
) -> Vec<(String, String, Option<String>)> {
    let mut columns = Vec::new();

    // Try catalog first
    if let Some(catalog) = catalog_data {
        if let Some(nodes) = catalog.get("nodes") {
            if let Some(cat_node) = nodes.get(node_id) {
                if let Some(cat_columns) = cat_node.get("columns") {
                    if let Some(cols_obj) = cat_columns.as_object() {
                        for (name, col_info) in cols_obj {
                            let col_type = col_info.get("type")
                                .and_then(|t| t.as_str())
                                .unwrap_or("STRING")
                                .to_string();
                            let description = col_info.get("description")
                                .and_then(|d| d.as_str())
                                .map(|s| s.to_string());
                            columns.push((name.clone(), col_type, description));
                        }
                        return columns;
                    }
                }
            }
        }
    }

    // Fall back to existing columns in manifest
    if !node.columns.is_empty() {
        for (name, col) in &node.columns {
            let col_type = col.data_type.clone().unwrap_or_else(|| "STRING".to_string());
            let description = if col.description.is_empty() {
                None
            } else {
                Some(col.description.clone())
            };
            columns.push((name.clone(), col_type, description));
        }
        if !columns.is_empty() {
            return columns;
        }
    }

    // Fall back to SQL inference
    let sql_path = Path::new(&node.original_file_path);
    if sql_path.exists() {
        if let Ok(sql_content) = std::fs::read_to_string(sql_path) {
            let (preprocessed, _) = DbtFunctionExtractor::preprocess(&sql_content, Some(manifest));

            // Simple column extraction from SELECT statement
            if let Some(inferred) = infer_columns_from_sql(&preprocessed) {
                return inferred;
            }
        }
    }

    // Default: return placeholder columns
    vec![
        ("id".to_string(), "INT64".to_string(), Some("Primary key".to_string())),
        ("created_at".to_string(), "TIMESTAMP".to_string(), Some("Creation timestamp".to_string())),
    ]
}

/// Simple SQL column inference from SELECT statement
fn infer_columns_from_sql(sql: &str) -> Option<Vec<(String, String, Option<String>)>> {
    // Very basic: extract column names from the first SELECT
    // This is a simplified version - the full inference is in schemarefly-sql

    let sql_upper = sql.to_uppercase();
    let select_idx = sql_upper.find("SELECT")?;
    let from_idx = sql_upper.find(" FROM ")?;

    if from_idx <= select_idx {
        return None;
    }

    let select_clause = &sql[select_idx + 6..from_idx];
    let columns: Vec<(String, String, Option<String>)> = select_clause
        .split(',')
        .filter_map(|col| {
            let col = col.trim();
            if col.is_empty() || col == "*" {
                return None;
            }

            // Handle "expr AS alias" or just "column_name"
            let name = if let Some(as_idx) = col.to_uppercase().rfind(" AS ") {
                col[as_idx + 4..].trim().to_string()
            } else {
                // Get the last part after any dots
                col.split('.').last()?.trim().to_string()
            };

            // Clean up the name
            let name = name.trim_matches(|c| c == '`' || c == '"' || c == '\'').to_string();

            if name.is_empty() {
                return None;
            }

            Some((name, "STRING".to_string(), None))
        })
        .collect();

    if columns.is_empty() {
        None
    } else {
        Some(columns)
    }
}

/// Print report summary to stdout
fn print_report_summary(report: &Report) {
    println!("\n{}", "=".repeat(60).bright_blue());
    println!("{}", "Schema Contract Check Report".bold().bright_blue());
    println!("{}", "=".repeat(60).bright_blue());
    println!();

    println!("Version: {}", report.version);
    println!("Timestamp: {}", report.timestamp);
    println!();

    println!("{}", "Summary:".bold());
    println!("  Total diagnostics: {}", report.summary.total);

    if report.summary.errors > 0 {
        println!("  Errors:   {}", format!("{}", report.summary.errors).red().bold());
    } else {
        println!("  Errors:   {}", format!("{}", report.summary.errors).green());
    }

    if report.summary.warnings > 0 {
        println!("  Warnings: {}", format!("{}", report.summary.warnings).yellow());
    } else {
        println!("  Warnings: {}", format!("{}", report.summary.warnings).green());
    }

    println!("  Info:     {}", report.summary.info);
    println!();

    if report.diagnostics.is_empty() {
        println!("{}", "✓ No issues found!".green().bold());
    } else {
        println!("{}", "Diagnostics:".bold());
        for diag in &report.diagnostics {
            let severity_str = match diag.severity {
                schemarefly_core::Severity::Error => "ERROR".red().bold(),
                schemarefly_core::Severity::Warn => "WARN".yellow().bold(),
                schemarefly_core::Severity::Info => "INFO".cyan(),
            };

            println!("  [{}] {}: {}", severity_str, diag.code, diag.message);

            if let Some(loc) = &diag.location {
                print!("    at {}:", loc.file);
                if let Some(line) = loc.line {
                    print!("{}", line);
                }
                println!();
            }

            if diag.expected.is_some() || diag.actual.is_some() {
                if let Some(exp) = &diag.expected {
                    println!("    Expected: {}", exp);
                }
                if let Some(act) = &diag.actual {
                    println!("    Actual:   {}", act);
                }
            }

            if !diag.impact.is_empty() {
                println!("    Impact: {} downstream models", diag.impact.len());
                for model in &diag.impact {
                    println!("      - {}", model);
                }
            }
        }
    }

    println!();
    println!("{}", "=".repeat(60).bright_blue());
}

/// Generate markdown report
fn generate_markdown_report(report: &Report, state_comparison: Option<&StateComparisonResult>) -> String {
    let mut md = String::new();

    md.push_str("# Schema Contract Check Report\n\n");
    md.push_str(&format!("**Version:** {}\n\n", report.version));
    md.push_str(&format!("**Timestamp:** {}\n\n", report.timestamp));

    // Add Slim CI section if state comparison was performed
    if let Some(comparison) = state_comparison {
        md.push_str("## Slim CI Analysis\n\n");
        md.push_str("This report was generated in **Slim CI mode**, comparing against a production state manifest.\n\n");

        md.push_str("### Change Summary\n\n");
        md.push_str(&format!("| Metric | Count |\n"));
        md.push_str(&format!("|--------|-------|\n"));
        md.push_str(&format!("| Modified models | {} |\n", comparison.modified_models.len()));
        md.push_str(&format!("| New models | {} |\n", comparison.new_models.len()));
        md.push_str(&format!("| Deleted models | {} |\n", comparison.deleted_models.len()));
        md.push_str(&format!("| Total blast radius | {} |\n", comparison.total_blast_radius));
        md.push_str("\n");

        if !comparison.modified_models.is_empty() {
            md.push_str("### Modified Models\n\n");
            for modified in &comparison.modified_models {
                let reasons: Vec<String> = modified.reasons.iter().map(|r| r.to_string()).collect();
                md.push_str(&format!("#### `{}`\n\n", modified.name));
                md.push_str(&format!("- **Unique ID:** `{}`\n", modified.unique_id));
                md.push_str(&format!("- **Reason:** {}\n", reasons.join(", ")));
                md.push_str(&format!("- **Downstream impact:** {} models\n", modified.downstream_count));

                if !modified.downstream_impact.is_empty() {
                    md.push_str("\n**Affected downstream models:**\n\n");
                    for downstream in &modified.downstream_impact {
                        md.push_str(&format!("- `{}`\n", downstream));
                    }
                }
                md.push_str("\n");
            }
        }

        if !comparison.deleted_models.is_empty() {
            md.push_str("### Deleted Models\n\n");
            md.push_str("⚠️ The following models were removed:\n\n");
            for deleted in &comparison.deleted_models {
                md.push_str(&format!("- `{}`\n", deleted));
            }
            md.push_str("\n");
        }

        md.push_str("---\n\n");
    }

    md.push_str("## Summary\n\n");
    md.push_str(&format!("- Total diagnostics: {}\n", report.summary.total));
    md.push_str(&format!("- Errors: {}\n", report.summary.errors));
    md.push_str(&format!("- Warnings: {}\n", report.summary.warnings));
    md.push_str(&format!("- Info: {}\n", report.summary.info));
    md.push_str("\n");

    if report.diagnostics.is_empty() {
        md.push_str("✅ **No issues found!**\n");
    } else {
        md.push_str("## Diagnostics\n\n");

        for diag in &report.diagnostics {
            let severity_emoji = match diag.severity {
                schemarefly_core::Severity::Error => "❌",
                schemarefly_core::Severity::Warn => "⚠️",
                schemarefly_core::Severity::Info => "ℹ️",
            };

            md.push_str(&format!("### {} {} - {}\n\n", severity_emoji, diag.severity, diag.code));
            md.push_str(&format!("{}\n\n", diag.message));

            if let Some(loc) = &diag.location {
                md.push_str(&format!("**Location:** {}", loc.file));
                if let Some(line) = loc.line {
                    md.push_str(&format!(":{}", line));
                }
                md.push_str("\n\n");
            }

            if let Some(exp) = &diag.expected {
                md.push_str(&format!("**Expected:** `{}`\n\n", exp));
            }
            if let Some(act) = &diag.actual {
                md.push_str(&format!("**Actual:** `{}`\n\n", act));
            }

            if !diag.impact.is_empty() {
                md.push_str(&format!("**Impact:** {} downstream models\n\n", diag.impact.len()));
                for model in &diag.impact {
                    md.push_str(&format!("- {}\n", model));
                }
                md.push_str("\n");
            }
        }
    }

    md
}

/// Generate PR comment markdown (optimized for GitHub PRs)
/// Includes status badge, collapsible details, and concise summary
fn generate_pr_comment(report: &Report, state_comparison: Option<&StateComparisonResult>) -> String {
    let mut md = String::new();

    // Hidden marker for finding/updating the comment
    md.push_str("<!-- schemarefly-report -->\n\n");

    // Status badge
    let (status_emoji, status_text) = if report.summary.errors > 0 {
        ("❌", "Schema Contract Check Failed")
    } else if report.summary.warnings > 0 {
        ("⚠️", "Schema Contract Check Passed with Warnings")
    } else {
        ("✅", "Schema Contract Check Passed")
    };

    md.push_str(&format!("## {} {}\n\n", status_emoji, status_text));

    // Quick stats table
    md.push_str("| Metric | Count |\n");
    md.push_str("|--------|-------|\n");
    md.push_str(&format!("| Errors | {} |\n", report.summary.errors));
    md.push_str(&format!("| Warnings | {} |\n", report.summary.warnings));
    md.push_str(&format!("| Info | {} |\n", report.summary.info));

    // Add Slim CI metrics if available
    if let Some(comparison) = state_comparison {
        md.push_str(&format!("| Modified models | {} |\n", comparison.modified_models.len()));
        md.push_str(&format!("| Blast radius | {} |\n", comparison.total_blast_radius));
    }
    md.push_str("\n");

    // Show errors prominently (not collapsed)
    if report.summary.errors > 0 {
        md.push_str("### Errors\n\n");
        for diag in &report.diagnostics {
            if diag.severity == schemarefly_core::Severity::Error {
                md.push_str(&format!("- **{}**: {}\n", diag.code, diag.message));
                if let Some(loc) = &diag.location {
                    md.push_str(&format!("  - 📍 `{}`", loc.file));
                    if let Some(line) = loc.line {
                        md.push_str(&format!(":{}", line));
                    }
                    md.push_str("\n");
                }
            }
        }
        md.push_str("\n");
    }

    // Collapsible section for warnings
    if report.summary.warnings > 0 {
        md.push_str("<details>\n");
        md.push_str("<summary>⚠️ Warnings (click to expand)</summary>\n\n");
        for diag in &report.diagnostics {
            if diag.severity == schemarefly_core::Severity::Warn {
                md.push_str(&format!("- **{}**: {}\n", diag.code, diag.message));
                if let Some(loc) = &diag.location {
                    md.push_str(&format!("  - 📍 `{}`", loc.file));
                    if let Some(line) = loc.line {
                        md.push_str(&format!(":{}", line));
                    }
                    md.push_str("\n");
                }
            }
        }
        md.push_str("\n</details>\n\n");
    }

    // Collapsible section for Slim CI details
    if let Some(comparison) = state_comparison {
        if !comparison.modified_models.is_empty() {
            md.push_str("<details>\n");
            md.push_str("<summary>📊 Modified Models & Impact (click to expand)</summary>\n\n");

            md.push_str("| Model | Change Type | Downstream Impact |\n");
            md.push_str("|-------|-------------|-------------------|\n");
            for modified in &comparison.modified_models {
                let reasons: Vec<String> = modified.reasons.iter().map(|r| r.to_string()).collect();
                md.push_str(&format!(
                    "| `{}` | {} | {} models |\n",
                    modified.name,
                    reasons.join(", "),
                    modified.downstream_count
                ));
            }

            md.push_str("\n</details>\n\n");
        }

        // Show deleted models as a warning
        if !comparison.deleted_models.is_empty() {
            md.push_str("### ⚠️ Deleted Models\n\n");
            md.push_str("The following models were removed:\n\n");
            for deleted in &comparison.deleted_models {
                md.push_str(&format!("- `{}`\n", deleted));
            }
            md.push_str("\n");
        }
    }

    // Footer with timestamp
    md.push_str("---\n\n");
    md.push_str(&format!(
        "<sub>Generated by [SchemaRefly](https://github.com/owner/schemarefly) at {}</sub>\n",
        report.timestamp
    ));

    md
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
