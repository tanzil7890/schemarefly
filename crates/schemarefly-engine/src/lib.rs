//! SchemaRefly engine - Core business logic
//!
//! This crate implements the main business logic for SchemaRefly:
//! - Contract diff engine
//! - Schema validation
//! - Report generation

pub mod contract_diff;

pub use contract_diff::ContractDiff;
