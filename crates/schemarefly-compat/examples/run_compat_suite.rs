//! Example: Run compatibility test suite against real dbt projects
//!
//! Usage:
//!   cargo run --package schemarefly-compat --example run_compat_suite -- /path/to/dbt/project
//!
//! This will:
//! 1. Load the dbt manifest from target/manifest.json
//! 2. Run SchemaRefly compat checks on all models
//! 3. Generate a detailed compatibility report
//! 4. Output metrics (parse success rate, schema inference rate, top failures)

use schemarefly_compat::{CompatTestHarness, CompatReport};
use schemarefly_core::config::{Config, DialectConfig};
use std::env;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    // Get project path from command line args
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <dbt-project-path> [dialect]", args[0]);
        eprintln!("\nExample:");
        eprintln!("  {} /path/to/dbt/project bigquery", args[0]);
        eprintln!("\nDialects: bigquery, snowflake, postgres, ansi");
        std::process::exit(1);
    }

    let project_path = PathBuf::from(&args[1]);
    let dialect = if args.len() > 2 {
        parse_dialect(&args[2])?
    } else {
        DialectConfig::Ansi
    };

    println!("Running SchemaRefly compatibility test suite...");
    println!("Project: {}", project_path.display());
    println!("Dialect: {:?}\n", dialect);

    // Create test harness
    let config = Config {
        dialect,
        severity: Default::default(),
        allowlist: Default::default(),
        warehouse: None,
        redact_sensitive_data: false,
        project_root: project_path.clone(),
    };

    let mut harness = CompatTestHarness::new(&project_path, config);

    // Load manifest
    println!("Loading dbt manifest...");
    harness.load_manifest()?;

    // Run checks
    println!("Running compatibility checks...\n");
    let metrics = harness.run_checks()?;

    // Generate report
    let report = CompatReport::new(vec![metrics]);

    // Print terminal report
    report.print_terminal_report();

    // Optionally save JSON report
    let report_path = project_path.join("schemarefly-compat-report.json");
    report.save_json(&report_path)?;
    println!("\nDetailed JSON report saved to: {}", report_path.display());

    Ok(())
}

fn parse_dialect(s: &str) -> anyhow::Result<DialectConfig> {
    match s.to_lowercase().as_str() {
        "bigquery" => Ok(DialectConfig::BigQuery),
        "snowflake" => Ok(DialectConfig::Snowflake),
        "postgres" => Ok(DialectConfig::Postgres),
        "ansi" => Ok(DialectConfig::Ansi),
        _ => Err(anyhow::anyhow!("Unknown dialect: {}. Valid options: bigquery, snowflake, postgres, ansi", s)),
    }
}
