use clap::{Parser, Subcommand};
use colored::Colorize;
use anyhow::Result;
use std::path::{Path, PathBuf};

use schemarefly_core::{Report, Config, Diagnostic};
use schemarefly_dbt::{Manifest, DependencyGraph, ContractExtractor};
use schemarefly_engine::DriftDetection;
use schemarefly_sql::DbtFunctionExtractor;
use schemarefly_catalog::{WarehouseAdapter, TableIdentifier, BigQueryAdapter, SnowflakeAdapter};

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

    /// Initialize contracts for existing models
    InitContracts {
        /// Models to initialize (or all if not specified)
        models: Vec<String>,
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
        Commands::Check { output, markdown } => {
            check_command(&config, &output, markdown.as_ref().map(|v| v.as_path()), cli.verbose)
        }
        Commands::Impact { model, manifest } => {
            impact_command(&config, &model, &manifest, cli.verbose)
        }
        Commands::Drift { output } => {
            drift_command(&config, &output, cli.verbose).await
        }
        Commands::InitContracts { models } => {
            init_contracts_command(&config, &models, cli.verbose)
        }
    }
}

/// Check command - validate schema contracts (with Salsa incremental computation)
fn check_command(
    config: &Config,
    output: &PathBuf,
    markdown: Option<&Path>,
    verbose: bool,
) -> Result<()> {
    use schemarefly_incremental::{SchemaReflyDatabase, queries};

    if verbose {
        eprintln!("{}", "Running schema contract checks (with incremental computation)...".cyan());
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
    let manifest_input = queries::ManifestInput::new(&db, manifest_json);
    let config_input = queries::ConfigInput::new(&db, config.clone());

    if verbose {
        eprintln!("{}", "Building dependency graph...".cyan());
    }

    // Get manifest from Salsa (cached)
    let manifest_opt = queries::manifest(&db, manifest_input);
    let manifest = manifest_opt.ok_or_else(|| anyhow::anyhow!("Failed to parse manifest"))?;

    // Build dependency graph for impact analysis
    let dag = DependencyGraph::from_manifest(&manifest);

    if verbose {
        eprintln!("{}", "Checking contracts for all models...".cyan());
    }

    // Collect diagnostics from all contract checks
    let mut all_diagnostics = Vec::new();
    let mut checked_models = 0;
    let mut models_with_contracts = 0;

    // Check each model with a contract
    for (node_id, node) in manifest.models() {
        // Extract contract if present
        if let Some(_contract) = ContractExtractor::extract_from_node(node) {
            models_with_contracts += 1;

            if verbose {
                eprintln!("  {} {}...", "Checking".cyan(), node.name);
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
    }

    // Build report with diagnostics
    let report = Report::from_diagnostics(all_diagnostics);

    // Save JSON report
    report.save_to_file(output)?;

    if verbose {
        eprintln!("{} {}", "Report saved to:".green(), output.display());
    }

    // Save markdown report if requested
    if let Some(md_path) = markdown {
        let markdown_content = generate_markdown_report(&report);
        std::fs::write(md_path, markdown_content)?;
        if verbose {
            eprintln!("{} {}", "Markdown report saved to:".green(), md_path.display());
        }
    }

    // Print summary
    print_report_summary(&report);

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
                Box::new(BigQueryAdapter::new(project_id, credentials))
            } else {
                Box::new(BigQueryAdapter::with_adc(project_id))
            }
        }
        "snowflake" => {
            let account = warehouse_config.settings.get("account")
                .ok_or_else(|| anyhow::anyhow!("Snowflake requires 'account' in warehouse settings"))?;
            let username = warehouse_config.settings.get("username")
                .ok_or_else(|| anyhow::anyhow!("Snowflake requires 'username' in warehouse settings"))?;
            let password = warehouse_config.settings.get("password")
                .ok_or_else(|| anyhow::anyhow!("Snowflake requires 'password' in warehouse settings"))?;

            let mut adapter = SnowflakeAdapter::new(account, username, password);

            if let Some(warehouse) = warehouse_config.settings.get("warehouse") {
                adapter = adapter.with_warehouse(warehouse);
            }
            if let Some(role) = warehouse_config.settings.get("role") {
                adapter = adapter.with_role(role);
            }

            Box::new(adapter)
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

/// Init contracts command - generate contracts from current schemas
fn init_contracts_command(_config: &Config, models: &[String], verbose: bool) -> Result<()> {
    if verbose {
        eprintln!("{}", "Initializing contracts...".cyan());
    }

    println!("{}", "Contract initialization not yet implemented (Phase 4)".yellow());
    if !models.is_empty() {
        println!("Models: {}", models.join(", "));
    } else {
        println!("All models");
    }

    Ok(())
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
fn generate_markdown_report(report: &Report) -> String {
    let mut md = String::new();

    md.push_str("# Schema Contract Check Report\n\n");
    md.push_str(&format!("**Version:** {}\n\n", report.version));
    md.push_str(&format!("**Timestamp:** {}\n\n", report.timestamp));

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
