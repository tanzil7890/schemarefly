//! Compatibility report generation

use crate::metrics::CompatMetrics;
use colored::Colorize;
use serde::{Deserialize, Serialize};

/// Compatibility test report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatReport {
    /// All project metrics
    pub projects: Vec<CompatMetrics>,

    /// Aggregate statistics
    pub aggregate: AggregateStats,
}

/// Aggregate statistics across all projects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateStats {
    pub total_projects: usize,
    pub total_models: usize,
    pub overall_parse_success_rate: f64,
    pub overall_schema_inference_rate: f64,
    pub total_unsupported: usize,
}

impl CompatReport {
    /// Create a new report from project metrics
    pub fn new(projects: Vec<CompatMetrics>) -> Self {
        let aggregate = Self::compute_aggregate(&projects);
        Self { projects, aggregate }
    }

    /// Compute aggregate statistics
    fn compute_aggregate(projects: &[CompatMetrics]) -> AggregateStats {
        let total_projects = projects.len();
        let total_models: usize = projects.iter().map(|p| p.total_models).sum();
        let total_parsed: usize = projects.iter().map(|p| p.parsed_successfully).sum();
        let total_inferred: usize = projects.iter().map(|p| p.schema_inferred).sum();
        let total_unsupported: usize = projects.iter().map(|p| p.unsupported_models).sum();

        let overall_parse_success_rate = if total_models > 0 {
            total_parsed as f64 / total_models as f64
        } else {
            0.0
        };

        let overall_schema_inference_rate = if total_models > 0 {
            total_inferred as f64 / total_models as f64
        } else {
            0.0
        };

        AggregateStats {
            total_projects,
            total_models,
            overall_parse_success_rate,
            overall_schema_inference_rate,
            total_unsupported,
        }
    }

    /// Generate a human-readable terminal report
    pub fn print_terminal_report(&self) {
        println!("\n{}", "╔══════════════════════════════════════════════════════════════════╗".cyan());
        println!("{}", "║       SchemaRefly Compatibility Test Report                     ║".cyan().bold());
        println!("{}", "╚══════════════════════════════════════════════════════════════════╝".cyan());

        // Aggregate stats
        println!("\n{}", "Aggregate Statistics:".bold());
        println!("  Total Projects:              {}", self.aggregate.total_projects);
        println!("  Total Models:                {}", self.aggregate.total_models);
        println!("  Parse Success Rate:          {:.1}%", self.aggregate.overall_parse_success_rate * 100.0);
        println!("  Schema Inference Rate:       {:.1}%", self.aggregate.overall_schema_inference_rate * 100.0);
        println!("  Unsupported Models:          {}", self.aggregate.total_unsupported);

        // Per-project breakdown
        println!("\n{}", "Per-Project Breakdown:".bold());
        for metrics in &self.projects {
            println!("\n  {} ({})", metrics.project_name.green(), metrics.dialect.yellow());
            println!("    Total Models:            {}", metrics.total_models);
            println!("    Parsed Successfully:     {} ({:.1}%)",
                metrics.parsed_successfully,
                metrics.parse_success_rate() * 100.0
            );
            println!("    Schema Inferred:         {} ({:.1}%)",
                metrics.schema_inferred,
                metrics.schema_inference_rate() * 100.0
            );
            println!("    Parse Failures:          {}", metrics.parse_failures);
            println!("    Inference Failures:      {}", metrics.inference_failures);
            println!("    Unsupported:             {}", metrics.unsupported_models);

            // Top 5 failure codes
            let top_failures = metrics.top_failure_codes(5);
            if !top_failures.is_empty() {
                println!("    Top Failure Codes:");
                for (code, count) in top_failures {
                    println!("      {} - {} occurrences", code.red(), count);

                    // Show sample messages
                    if let Some(samples) = metrics.failure_samples.get(&code) {
                        for (i, sample) in samples.iter().enumerate() {
                            let preview = if sample.len() > 80 {
                                format!("{}...", &sample[..77])
                            } else {
                                sample.clone()
                            };
                            println!("        Sample {}: {}", i + 1, preview.dimmed());
                        }
                    }
                }
            }
        }

        // Summary
        println!("\n{}", "Summary:".bold());
        if self.aggregate.overall_parse_success_rate >= 0.95 {
            println!("  {} Excellent parse success rate (≥95%)", "✓".green());
        } else if self.aggregate.overall_parse_success_rate >= 0.85 {
            println!("  {} Good parse success rate (≥85%)", "!".yellow());
        } else {
            println!("  {} Parse success rate needs improvement (<85%)", "✗".red());
        }

        if self.aggregate.overall_schema_inference_rate >= 0.90 {
            println!("  {} Excellent schema inference rate (≥90%)", "✓".green());
        } else if self.aggregate.overall_schema_inference_rate >= 0.75 {
            println!("  {} Good schema inference rate (≥75%)", "!".yellow());
        } else {
            println!("  {} Schema inference rate needs improvement (<75%)", "✗".red());
        }

        println!();
    }

    /// Export report as JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Export report as JSON to file
    pub fn save_json(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        let json = self.to_json()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)?;
        Ok(())
    }
}
