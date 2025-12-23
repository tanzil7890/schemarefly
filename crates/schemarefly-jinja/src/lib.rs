//! Jinja template preprocessing for dbt SQL models
//!
//! This crate handles:
//! - Preprocessing dbt SQL models with Jinja2 templates
//! - Providing dbt context (ref, source, var, config, etc.)
//! - Rendering templates to pure SQL for parsing
//! - Error handling with detailed diagnostics

pub mod preprocessor;
pub mod context;
pub mod functions;

pub use preprocessor::{JinjaPreprocessor, PreprocessResult, PreprocessError};
pub use context::{DbtContext, DbtContextBuilder};
pub use functions::{ref_function, source_function, var_function, config_function};
