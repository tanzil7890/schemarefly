//! Jinja template preprocessing
//!
//! Converts dbt SQL models with Jinja templates to pure SQL.

use minijinja::{Environment, Error as JinjaError};
use schemarefly_core::{Diagnostic, DiagnosticCode, Severity, Location};
use std::path::{Path, PathBuf};
use crate::context::DbtContext;

/// Result of Jinja preprocessing
#[derive(Debug, Clone)]
pub struct PreprocessResult {
    /// Original SQL with Jinja templates
    pub original_sql: String,

    /// Rendered SQL without Jinja templates
    pub rendered_sql: String,

    /// File path (if any)
    pub file_path: Option<PathBuf>,

    /// Whether any Jinja was detected and processed
    pub had_jinja: bool,
}

/// Error during Jinja preprocessing
#[derive(Debug, thiserror::Error)]
pub enum PreprocessError {
    #[error("Jinja render error: {message}")]
    RenderError {
        message: String,
        file_path: Option<PathBuf>,
        line: Option<usize>,
        column: Option<usize>,
    },

    #[error("Undefined variable: {name}")]
    UndefinedVariable {
        name: String,
        file_path: Option<PathBuf>,
    },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl PreprocessError {
    /// Convert to SchemaRefly diagnostic
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            PreprocessError::RenderError { message, file_path, line, column } => {
                let mut diag = Diagnostic::new(
                    DiagnosticCode::JinjaRenderError,
                    Severity::Error,
                    message.clone(),
                );

                if let Some(path) = file_path {
                    if let (Some(l), Some(c)) = (line, column) {
                        let location = Location {
                            file: path.display().to_string(),
                            line: Some(*l),
                            column: Some(*c),
                            end_line: None,
                            end_column: None,
                        };
                        diag = diag.with_location(location);
                    }
                }

                diag
            }
            PreprocessError::UndefinedVariable { name, file_path } => {
                let mut diag = Diagnostic::new(
                    DiagnosticCode::JinjaUndefinedVariable,
                    Severity::Error,
                    format!("Undefined variable: {}", name),
                );

                if let Some(path) = file_path {
                    let location = Location {
                        file: path.display().to_string(),
                        line: None,
                        column: None,
                        end_line: None,
                        end_column: None,
                    };
                    diag = diag.with_location(location);
                }

                diag
            }
            PreprocessError::IoError(e) => {
                Diagnostic::new(
                    DiagnosticCode::InternalError,
                    Severity::Error,
                    format!("IO error: {}", e),
                )
            }
        }
    }
}

/// Jinja template preprocessor for dbt SQL
pub struct JinjaPreprocessor {
    env: Environment<'static>,
    context: DbtContext,
}

impl JinjaPreprocessor {
    /// Create a new preprocessor with the given dbt context
    pub fn new(context: DbtContext) -> Self {
        let mut env = Environment::new();

        // Register dbt functions
        env.add_function("ref", crate::functions::ref_function);
        env.add_function("source", crate::functions::source_function);
        env.add_function("var", crate::functions::var_function);
        env.add_function("config", crate::functions::config_function);

        // Additional Jinja filters commonly used in dbt
        env.add_filter("as_bool", |value: String| -> bool {
            matches!(value.to_lowercase().as_str(), "true" | "1" | "yes")
        });

        Self { env, context }
    }

    /// Create a preprocessor with default context
    pub fn with_defaults() -> Self {
        Self::new(DbtContext::default())
    }

    /// Check if SQL contains Jinja templates
    pub fn has_jinja(sql: &str) -> bool {
        sql.contains("{{") || sql.contains("{%") || sql.contains("{#")
    }

    /// Preprocess SQL with Jinja templates
    pub fn preprocess(&self, sql: &str, file_path: Option<&Path>) -> Result<PreprocessResult, PreprocessError> {
        let had_jinja = Self::has_jinja(sql);

        // If no Jinja detected, return as-is
        if !had_jinja {
            return Ok(PreprocessResult {
                original_sql: sql.to_string(),
                rendered_sql: sql.to_string(),
                file_path: file_path.map(|p| p.to_path_buf()),
                had_jinja: false,
            });
        }

        // Render the template
        let rendered = self.env
            .render_str(sql, &self.context.to_minijinja_value())
            .map_err(|e| self.jinja_error_to_preprocess_error(e, file_path))?;

        Ok(PreprocessResult {
            original_sql: sql.to_string(),
            rendered_sql: rendered,
            file_path: file_path.map(|p| p.to_path_buf()),
            had_jinja: true,
        })
    }

    /// Preprocess SQL from a file
    pub fn preprocess_file(&self, path: &Path) -> Result<PreprocessResult, PreprocessError> {
        let sql = std::fs::read_to_string(path)?;
        self.preprocess(&sql, Some(path))
    }

    /// Convert MiniJinja error to PreprocessError
    fn jinja_error_to_preprocess_error(&self, error: JinjaError, file_path: Option<&Path>) -> PreprocessError {
        let message = error.to_string();

        // MiniJinja error detail is a string, not an object
        let (line, column) = (None, None);

        // Check if it's an undefined variable error
        if message.contains("undefined") || message.contains("not found") {
            if let Some(var_name) = Self::extract_variable_name(&message) {
                return PreprocessError::UndefinedVariable {
                    name: var_name,
                    file_path: file_path.map(|p| p.to_path_buf()),
                };
            }
        }

        PreprocessError::RenderError {
            message,
            file_path: file_path.map(|p| p.to_path_buf()),
            line,
            column,
        }
    }

    /// Extract variable name from error message
    fn extract_variable_name(message: &str) -> Option<String> {
        // Try to find variable name in quotes
        if let Some(start) = message.find('\'') {
            if let Some(end) = message[start + 1..].find('\'') {
                return Some(message[start + 1..start + 1 + end].to_string());
            }
        }
        None
    }
}

impl Default for JinjaPreprocessor {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_jinja() {
        assert!(JinjaPreprocessor::has_jinja("select * from {{ ref('table') }}"));
        assert!(JinjaPreprocessor::has_jinja("{% set var = 'value' %}"));
        assert!(JinjaPreprocessor::has_jinja("{# comment #}"));
        assert!(!JinjaPreprocessor::has_jinja("select * from table"));
    }

    #[test]
    fn test_no_jinja_passthrough() {
        let preprocessor = JinjaPreprocessor::with_defaults();
        let sql = "select * from table";
        let result = preprocessor.preprocess(sql, None).unwrap();

        assert_eq!(result.original_sql, sql);
        assert_eq!(result.rendered_sql, sql);
        assert!(!result.had_jinja);
    }

    #[test]
    fn test_simple_ref() {
        let preprocessor = JinjaPreprocessor::with_defaults();
        let sql = "select * from {{ ref('my_table') }}";
        let result = preprocessor.preprocess(sql, None).unwrap();

        assert!(result.had_jinja);
        assert_eq!(result.rendered_sql, "select * from my_table");
    }

    #[test]
    fn test_jinja_comment_removal() {
        let preprocessor = JinjaPreprocessor::with_defaults();
        let sql = "{#- This is a comment -#}\nselect * from table";
        let result = preprocessor.preprocess(sql, None).unwrap();

        assert!(result.had_jinja);
        assert_eq!(result.rendered_sql.trim(), "select * from table");
    }
}
