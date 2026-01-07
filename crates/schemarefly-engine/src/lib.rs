//! SchemaRefly engine - Core business logic
//!
//! This crate implements the main business logic for SchemaRefly:
//! - Contract diff engine
//! - Schema validation
//! - Drift detection
//! - State comparison for Slim CI
//! - Report generation

pub mod contract_diff;
pub mod drift_detector;
pub mod state_comparison;

pub use contract_diff::ContractDiff;
pub use drift_detector::DriftDetection;
pub use state_comparison::{StateComparison, StateComparisonResult, ModifiedModel, ModificationReason};
