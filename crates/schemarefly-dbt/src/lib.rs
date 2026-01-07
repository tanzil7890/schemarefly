//! dbt artifact parsing and DAG construction
//!
//! This crate handles:
//! - Parsing manifest.json (dbt-generated artifacts)
//! - Building dependency graphs (DAG)
//! - Extracting contract definitions from model YAMLs
//! - Impact analysis (downstream dependencies)

pub mod manifest;
pub mod dag;
pub mod contract;

pub use manifest::{Manifest, ManifestNode, ManifestSource, NodeConfig, ContractConfig, ColumnDefinition, DependsOn, ManifestMetadata};
pub use dag::{DependencyGraph, NodeId};
pub use contract::ContractExtractor;
