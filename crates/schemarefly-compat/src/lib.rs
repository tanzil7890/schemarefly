//! Compatibility test suite for validating SchemaRefly against real dbt projects
//!
//! This crate provides infrastructure to test SchemaRefly against 10-20 real dbt projects
//! across different SQL dialects (BigQuery, Snowflake, Postgres) and track:
//! - Parse success rate
//! - % models with inferred schema
//! - Top failure codes and samples
//! - Unsupported model type detection (Python, ephemeral, etc.)

pub mod harness;
pub mod metrics;
pub mod model_detection;
pub mod report;

pub use harness::CompatTestHarness;
pub use metrics::{CompatMetrics, ModelResult, FailureDetail};
pub use model_detection::{ModelType, UnsupportedReason, detect_model_type};
pub use report::CompatReport;
