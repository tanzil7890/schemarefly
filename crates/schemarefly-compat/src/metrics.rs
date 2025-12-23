//! Compatibility metrics collection and tracking

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Overall compatibility metrics for a dbt project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatMetrics {
    /// Project name
    pub project_name: String,

    /// SQL dialect (bigquery, snowflake, postgres)
    pub dialect: String,

    /// Total number of models in the project
    pub total_models: usize,

    /// Number of models successfully parsed
    pub parsed_successfully: usize,

    /// Number of models with inferred schema
    pub schema_inferred: usize,

    /// Number of models that failed parsing
    pub parse_failures: usize,

    /// Number of models that failed schema inference
    pub inference_failures: usize,

    /// Number of unsupported models (Python, ephemeral, etc.)
    pub unsupported_models: usize,

    /// Individual model results
    pub model_results: Vec<ModelResult>,

    /// Top failure codes with counts
    pub failure_codes: HashMap<String, usize>,

    /// Failure samples (code -> example messages)
    pub failure_samples: HashMap<String, Vec<String>>,
}

impl CompatMetrics {
    pub fn new(project_name: impl Into<String>, dialect: impl Into<String>) -> Self {
        Self {
            project_name: project_name.into(),
            dialect: dialect.into(),
            total_models: 0,
            parsed_successfully: 0,
            schema_inferred: 0,
            parse_failures: 0,
            inference_failures: 0,
            unsupported_models: 0,
            model_results: Vec::new(),
            failure_codes: HashMap::new(),
            failure_samples: HashMap::new(),
        }
    }

    /// Calculate parse success rate (0.0 to 1.0)
    pub fn parse_success_rate(&self) -> f64 {
        if self.total_models == 0 {
            return 0.0;
        }
        self.parsed_successfully as f64 / self.total_models as f64
    }

    /// Calculate percentage of models with inferred schema (0.0 to 1.0)
    pub fn schema_inference_rate(&self) -> f64 {
        if self.total_models == 0 {
            return 0.0;
        }
        self.schema_inferred as f64 / self.total_models as f64
    }

    /// Get top N failure codes by frequency
    pub fn top_failure_codes(&self, n: usize) -> Vec<(String, usize)> {
        let mut codes: Vec<(String, usize)> = self.failure_codes.iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        codes.sort_by(|a, b| b.1.cmp(&a.1));
        codes.into_iter().take(n).collect()
    }

    /// Add a model result and update aggregate metrics
    pub fn add_model_result(&mut self, result: ModelResult) {
        self.total_models += 1;

        match &result.outcome {
            ModelOutcome::Success { schema_inferred } => {
                self.parsed_successfully += 1;
                if *schema_inferred {
                    self.schema_inferred += 1;
                }
            }
            ModelOutcome::ParseFailure(detail) => {
                self.parse_failures += 1;
                self.record_failure(detail);
            }
            ModelOutcome::InferenceFailure(detail) => {
                self.parsed_successfully += 1; // Parsing succeeded
                self.inference_failures += 1;
                self.record_failure(detail);
            }
            ModelOutcome::Unsupported { reason: _ } => {
                self.unsupported_models += 1;
            }
        }

        self.model_results.push(result);
    }

    /// Record a failure code and sample
    fn record_failure(&mut self, detail: &FailureDetail) {
        // Increment failure code count
        *self.failure_codes.entry(detail.code.clone()).or_insert(0) += 1;

        // Store sample (up to 3 samples per code)
        let samples = self.failure_samples.entry(detail.code.clone()).or_insert_with(Vec::new);
        if samples.len() < 3 {
            samples.push(detail.message.clone());
        }
    }
}

/// Result for a single model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResult {
    /// Model name (e.g., "my_model")
    pub model_name: String,

    /// Model file path
    pub file_path: String,

    /// Outcome of testing this model
    pub outcome: ModelOutcome,
}

/// Outcome of testing a model
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelOutcome {
    /// Model parsed and schema inferred successfully
    Success {
        schema_inferred: bool,
    },

    /// Failed to parse SQL
    ParseFailure(FailureDetail),

    /// Parsed successfully but failed to infer schema
    InferenceFailure(FailureDetail),

    /// Unsupported model type (Python, ephemeral, etc.)
    Unsupported {
        reason: String,
    },
}

/// Details about a failure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureDetail {
    /// Diagnostic code (e.g., "SR001", "SR010")
    pub code: String,

    /// Error message
    pub message: String,

    /// Optional context (e.g., SQL snippet)
    pub context: Option<String>,
}
