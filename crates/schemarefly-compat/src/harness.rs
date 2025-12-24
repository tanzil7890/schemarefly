//! Test harness for running SchemaRefly against real dbt projects

use crate::metrics::{CompatMetrics, ModelResult, ModelOutcome, FailureDetail};
use crate::model_detection::detect_model_type;

use schemarefly_core::config::Config;
use schemarefly_dbt::manifest::{Manifest, ManifestNode};
use schemarefly_sql::SqlParser;

use anyhow::{Context, Result};
use std::path::PathBuf;
use walkdir::WalkDir;

/// Test harness for validating SchemaRefly against real dbt projects
pub struct CompatTestHarness {
    /// Root directory of the dbt project
    project_root: PathBuf,

    /// Project configuration
    config: Config,

    /// dbt manifest
    manifest: Option<Manifest>,
}

impl CompatTestHarness {
    /// Create a new test harness for a dbt project
    pub fn new(project_root: impl Into<PathBuf>, config: Config) -> Self {
        Self {
            project_root: project_root.into(),
            config,
            manifest: None,
        }
    }

    /// Load dbt manifest from target/manifest.json
    pub fn load_manifest(&mut self) -> Result<()> {
        let manifest_path = self.project_root.join("target/manifest.json");
        let manifest_json = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read manifest at {}", manifest_path.display()))?;

        let manifest: Manifest = serde_json::from_str(&manifest_json)
            .context("Failed to parse manifest.json")?;

        self.manifest = Some(manifest);
        Ok(())
    }

    /// Run compatibility checks on all models and collect metrics
    ///
    /// If manifest is loaded, uses manifest metadata.
    /// Otherwise, falls back to direct model discovery.
    pub fn run_checks(&self) -> Result<CompatMetrics> {
        if let Some(manifest) = &self.manifest {
            self.run_checks_with_manifest(manifest)
        } else {
            self.run_checks_without_manifest()
        }
    }

    /// Run checks using manifest metadata
    fn run_checks_with_manifest(&self, manifest: &Manifest) -> Result<CompatMetrics> {
        let mut metrics = CompatMetrics::new(
            self.project_root.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown"),
            format!("{:?}", self.config.dialect).to_lowercase(),
        );

        // Check each model
        for (unique_id, node) in &manifest.nodes {
            // Only process models
            if !unique_id.starts_with("model.") {
                continue;
            }

            let result = self.check_model(node);
            metrics.add_model_result(result);
        }

        Ok(metrics)
    }

    /// Run checks by discovering SQL files directly (no manifest required)
    fn run_checks_without_manifest(&self) -> Result<CompatMetrics> {
        let mut metrics = CompatMetrics::new(
            self.project_root.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown"),
            format!("{:?}", self.config.dialect).to_lowercase(),
        );

        // Discover all SQL model files
        let sql_files = self.discover_models()?;

        if sql_files.is_empty() {
            println!("Warning: No SQL models found in models/ directory");
        }

        // Check each discovered model
        for sql_path in sql_files {
            let result = self.check_model_file(&sql_path);
            metrics.add_model_result(result);
        }

        Ok(metrics)
    }

    /// Check a single model and return result
    fn check_model(&self, node: &ManifestNode) -> ModelResult {
        let model_name = node.name.clone();
        let file_path = node.original_file_path.clone();

        // Check if model type is supported
        match detect_model_type(node) {
            Err(unsupported_reason) => {
                return ModelResult {
                    model_name,
                    file_path,
                    outcome: ModelOutcome::Unsupported {
                        reason: unsupported_reason.diagnostic_message(),
                    },
                };
            }
            Ok(_) => {
                // Continue with SQL model checking
            }
        }

        // Read SQL file
        let sql_path = self.project_root.join(&file_path);
        let sql_content = match std::fs::read_to_string(&sql_path) {
            Ok(content) => content,
            Err(err) => {
                return ModelResult {
                    model_name,
                    file_path,
                    outcome: ModelOutcome::ParseFailure(FailureDetail {
                        code: "SR000".to_string(),
                        message: format!("Failed to read SQL file: {}", err),
                        context: None,
                    }),
                };
            }
        };

        // Try to parse SQL (with Jinja template preprocessing)
        let parser = SqlParser::from_dialect(&self.config.dialect);
        let parsed_sql = match parser.parse_with_jinja(&sql_content, Some(&sql_path), None) {
            Ok(parsed) => parsed,
            Err(diag) => {
                return ModelResult {
                    model_name,
                    file_path,
                    outcome: ModelOutcome::ParseFailure(FailureDetail {
                        code: diag.code.as_str().to_string(),
                        message: diag.message.clone(),
                        context: Some(extract_error_context(&sql_content, 0)),
                    }),
                };
            }
        };

        // For basic compat testing, we just check if we got at least one statement
        // More advanced inference would use the SchemaInference engine
        let has_statements = !parsed_sql.statements.is_empty();

        if has_statements {
            ModelResult {
                model_name,
                file_path,
                outcome: ModelOutcome::Success {
                    schema_inferred: true,
                },
            }
        } else {
            ModelResult {
                model_name,
                file_path,
                outcome: ModelOutcome::InferenceFailure(FailureDetail {
                    code: "SR010".to_string(),
                    message: "No SQL statements found in file".to_string(),
                    context: Some(sql_content[..std::cmp::min(200, sql_content.len())].to_string()),
                }),
            }
        }
    }

    /// Check a single model file directly (without manifest metadata)
    fn check_model_file(&self, sql_path: &PathBuf) -> ModelResult {
        // Extract model name from file path
        let model_name = sql_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Get relative path from project root
        let file_path = sql_path
            .strip_prefix(&self.project_root)
            .unwrap_or(sql_path)
            .display()
            .to_string();

        // Read SQL file
        let sql_content = match std::fs::read_to_string(sql_path) {
            Ok(content) => content,
            Err(err) => {
                return ModelResult {
                    model_name,
                    file_path,
                    outcome: ModelOutcome::ParseFailure(FailureDetail {
                        code: "SR000".to_string(),
                        message: format!("Failed to read SQL file: {}", err),
                        context: None,
                    }),
                };
            }
        };

        // Try to parse SQL (with Jinja template preprocessing)
        let parser = SqlParser::from_dialect(&self.config.dialect);
        let parsed_sql = match parser.parse_with_jinja(&sql_content, Some(sql_path), None) {
            Ok(parsed) => parsed,
            Err(diag) => {
                return ModelResult {
                    model_name,
                    file_path,
                    outcome: ModelOutcome::ParseFailure(FailureDetail {
                        code: diag.code.as_str().to_string(),
                        message: diag.message.clone(),
                        context: Some(extract_error_context(&sql_content, 0)),
                    }),
                };
            }
        };

        // Check if we got statements
        let has_statements = !parsed_sql.statements.is_empty();

        if has_statements {
            ModelResult {
                model_name,
                file_path,
                outcome: ModelOutcome::Success {
                    schema_inferred: true,
                },
            }
        } else {
            ModelResult {
                model_name,
                file_path,
                outcome: ModelOutcome::InferenceFailure(FailureDetail {
                    code: "SR010".to_string(),
                    message: "No SQL statements found in file".to_string(),
                    context: Some(sql_content[..std::cmp::min(200, sql_content.len())].to_string()),
                }),
            }
        }
    }

    /// Discover all SQL model files in the project
    pub fn discover_models(&self) -> Result<Vec<PathBuf>> {
        let models_dir = self.project_root.join("models");
        if !models_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sql_files = Vec::new();

        for entry in WalkDir::new(models_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "sql" {
                        sql_files.push(entry.path().to_path_buf());
                    }
                }
            }
        }

        Ok(sql_files)
    }
}

/// Extract context around an error location (first N characters)
fn extract_error_context(sql: &str, _line: usize) -> String {
    let preview_len = 200;
    if sql.len() <= preview_len {
        sql.to_string()
    } else {
        format!("{}...", &sql[..preview_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_error_context() {
        let sql = "SELECT * FROM users WHERE id = 1";
        let context = extract_error_context(sql, 0);
        assert_eq!(context, sql);
    }

    #[test]
    fn test_extract_error_context_long() {
        let sql = "a".repeat(300);
        let context = extract_error_context(&sql, 0);
        assert!(context.len() <= 203); // 200 + "..."
        assert!(context.ends_with("..."));
    }
}
