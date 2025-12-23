//! SQL parsing and analysis
//!
//! This crate handles:
//! - Parsing SQL using datafusion-sqlparser-rs
//! - Resolving CTEs, aliases, and references
//! - Resolving dbt-specific functions (ref, source)
//! - Schema inference from SQL queries
//! - Extracting location information for diagnostics

pub mod parser;
pub mod resolver;
pub mod dbt_functions;
pub mod inference;

pub use parser::{SqlParser, ParsedSql, ParseError};
pub use resolver::{NameResolver, ResolvedName};
pub use dbt_functions::{DbtFunctionExtractor, DbtReference};
pub use inference::{SchemaInference, InferenceContext, InferenceError};
