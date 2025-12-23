use clap::{Parser, Subcommand};
use colored::Colorize;
use anyhow::Result;
use std::path::{Path, PathBuf};

use schemarefly_core::{Report, Config, Diagnostic};
use schemarefly_dbt::{Manifest, DependencyGraph, ContractExtractor};
use schemarefly_engine::ContractDiff;
use schemarefly_sql::{SqlParser, DbtFunctionExtractor, SchemaInference, InferenceContext};

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

fn main() -> Result<()> {
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
            drift_command(&config, &output, cli.verbose)
        }
        Commands::InitContracts { models } => {
            init_contracts_command(&config, &models, cli.verbose)
        }
    }
}

/// Check command - validate schema contracts
fn check_command(
    config: &Config,
    output: &PathBuf,
    markdown: Option<&Path>,
    verbose: bool,
) -> Result<()> {
    if verbose {
        eprintln!("{}", "Running schema contract checks...".cyan());
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

    // Load manifest
    let manifest = Manifest::from_file(manifest_path)?;

    if verbose {
        eprintln!("{}", "Building dependency graph...".cyan());
    }

    // Build dependency graph for impact analysis
    let dag = DependencyGraph::from_manifest(&manifest);

    // Create inference context with table schemas from manifest
    let context = InferenceContext::from_manifest(&manifest).with_catalog(true);

    // Create SQL parser based on config dialect
    let parser = SqlParser::from_dialect(&config.dialect);

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
        if let Some(contract) = ContractExtractor::extract_from_node(node) {
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

            // Parse SQL
            let parsed = match parser.parse(&preprocessed_sql, Some(&sql_file_path)) {
                Ok(p) => p,
                Err(e) => {
                    // Convert parse error to diagnostic
                    all_diagnostics.push(e.to_diagnostic());
                    continue;
                }
            };

            // Infer schema
            let inference = SchemaInference::new(&context);
            let inferred_schema = match parsed.first_statement() {
                Some(stmt) => match inference.infer_statement(stmt) {
                    Ok(schema) => schema,
                    Err(e) => {
                        // Convert inference error to diagnostic
                        all_diagnostics.push(inference.create_diagnostic(&e));
                        continue;
                    }
                },
                None => {
                    let diag = Diagnostic::new(
                        schemarefly_core::DiagnosticCode::SqlParseError,
                        schemarefly_core::Severity::Error,
                        format!("No SQL statement found in {}", sql_file_path.display()),
                    );
                    all_diagnostics.push(diag);
                    continue;
                }
            };

            // Compare inferred schema to contract
            let diff = ContractDiff::compare(
                &node_id,
                &contract,
                &inferred_schema,
                Some(sql_file_path.display().to_string()),
            );

            // Add downstream impact to each diagnostic
            let downstream = dag.downstream(&node_id);
            let has_errors = diff.has_errors();
            let has_warnings = diff.has_warnings();
            let error_count = diff.error_count();
            let warning_count = diff.warning_count();

            for mut diag in diff.diagnostics {
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
fn drift_command(_config: &Config, output: &PathBuf, verbose: bool) -> Result<()> {
    if verbose {
        eprintln!("{}", "Detecting schema drift...".cyan());
    }

    println!("{}", "Drift detection not yet implemented (Phase 5)".yellow());
    println!("Output: {}", output.display());

    Ok(())
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
