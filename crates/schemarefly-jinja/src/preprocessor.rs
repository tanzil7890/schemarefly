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

        // Register dbt core functions
        env.add_function("ref", crate::functions::ref_function);
        env.add_function("source", crate::functions::source_function);
        env.add_function("config", crate::functions::config_function);

        // var() function needs access to context vars
        let context_vars = context.vars.clone();
        env.add_function("var", move |var_name: minijinja::Value, default: Option<minijinja::Value>| -> Result<minijinja::Value, minijinja::Error> {
            let var_str = var_name.as_str().ok_or_else(|| {
                minijinja::Error::new(minijinja::ErrorKind::InvalidOperation, "var() variable name must be a string")
            })?;

            // Check if variable exists in context
            if let Some(val) = context_vars.get(var_str) {
                Ok(minijinja::Value::from_serialize(val))
            } else if let Some(default_val) = default {
                Ok(default_val)
            } else {
                Err(minijinja::Error::new(
                    minijinja::ErrorKind::UndefinedError,
                    format!("Undefined variable: {}", var_str)
                ))
            }
        });

        // Register dbt_utils package macros (common stubs)
        // These return SQL-safe placeholders instead of erroring
        env.add_function("surrogate_key", |_columns: Vec<minijinja::Value>| -> String {
            "md5(concat_ws('|', *))".to_string()
        });
        env.add_function("generate_series", |_start: minijinja::Value, _end: minijinja::Value| -> String {
            "generate_series(0, 100)".to_string()
        });
        env.add_function("date_spine", |_args: minijinja::value::Rest<minijinja::Value>, _kwargs: minijinja::value::Kwargs| -> String {
            "SELECT CURRENT_DATE as date_day".to_string()
        });
        env.add_function("union_relations", |_relations: Vec<minijinja::Value>| -> String {
            "SELECT * FROM source_table".to_string()
        });
        env.add_function("get_column_values", |_table: minijinja::Value, _column: minijinja::Value| -> String {
            "column_value".to_string()
        });

        // Common custom macro stubs - graceful fallback for user-defined macros
        env.add_function("get_payment_type_description", |_payment_type: minijinja::Value| -> String {
            "payment_type_desc".to_string()
        });
        env.add_function("dynamic_partition", |_date_col: minijinja::Value, _granularity: minijinja::Value| -> String {
            "partition_column".to_string()
        });
        env.add_function("generate_surrogate_key", |_columns: Vec<minijinja::Value>| -> String {
            "surrogate_key_value".to_string()
        });
        env.add_function("get_date_dimension", |_date: minijinja::Value| -> String {
            "date_dimension".to_string()
        });
        env.add_function("cents_to_dollars", |_amount: minijinja::Value, _precision: Option<minijinja::Value>| -> String {
            "(amount / 100.0)".to_string()
        });

        // Additional dbt_utils functions
        env.add_function("concat", |_args: minijinja::value::Rest<minijinja::Value>| -> String {
            "concat(column1, column2)".to_string()
        });
        env.add_function("star", |_args: minijinja::value::Kwargs| -> String {
            "*".to_string()
        });
        env.add_function("pivot", |_args: minijinja::value::Kwargs| -> String {
            "pivoted_columns".to_string()
        });
        env.add_function("unpivot", |_args: minijinja::value::Kwargs| -> String {
            "unpivoted_columns".to_string()
        });
        env.add_function("groupby", |_columns: Vec<minijinja::Value>| -> String {
            "GROUP BY column1, column2".to_string()
        });
        env.add_function("get_url_host", |_url: minijinja::Value| -> String {
            "parse_url(url, 'HOST')".to_string()
        });
        env.add_function("get_url_parameter", |_url: minijinja::Value, _param: minijinja::Value| -> String {
            "parse_url(url, 'QUERY')".to_string()
        });

        // dbt_date package macros
        env.add_function("get_date_dimension", |_start_date: minijinja::Value, _end_date: minijinja::Value| -> String {
            "SELECT date_day FROM date_dimension".to_string()
        });
        env.add_function("get_fiscal_periods", |_ref_table: minijinja::Value, _params: minijinja::value::Kwargs| -> String {
            "SELECT date_day, fiscal_week_of_year, fiscal_period_number FROM fiscal_periods".to_string()
        });

        // metrics package support
        env.add_function("metric", |_metric_name: minijinja::Value| -> String {
            "metric_calculation".to_string()
        });
        env.add_function("calculate", |_metric: minijinja::Value, _params: minijinja::value::Kwargs| -> String {
            "(SELECT * FROM metric_table)".to_string()
        });

        // dbt cross-database macros (in dbt namespace)
        env.add_function("split_part", |_string: minijinja::Value, _delimiter: minijinja::Value, _part: minijinja::Value| -> String {
            "split_part(string, delimiter, part)".to_string()
        });

        // adapter methods
        env.add_function("get_columns_in_relation", |_relation: minijinja::Value| -> Vec<minijinja::Value> {
            vec![]
        });

        // dbt built-in functions
        env.add_function("is_incremental", || -> bool {
            false
        });
        env.add_function("statement", |_name: minijinja::Value, _params: minijinja::value::Kwargs| -> String {
            "".to_string()
        });

        // Custom user macros
        env.add_function("money", || -> String {
            "::numeric(16,2)".to_string()
        });
        env.add_function("load_result", |_name: minijinja::Value| -> minijinja::Value {
            // Return structure that supports .table.columns[0].values chain
            // values is a property (array) not a method
            minijinja::Value::from_serialize(&serde_json::json!({
                "data": [],
                "table": {
                    "columns": [{
                        "name": "column1",
                        "values": []
                    }]
                }
            }))
        });

        // Additional Jinja filters commonly used in dbt
        env.add_filter("as_bool", |value: String| -> bool {
            matches!(value.to_lowercase().as_str(), "true" | "1" | "yes")
        });

        // Add 'items' filter to convert dicts to iterable key-value pairs for MiniJinja
        env.add_filter("items", |value: minijinja::Value| -> Result<minijinja::Value, minijinja::Error> {
            if let Some(_obj) = value.as_object() {
                // Convert object to list of [key, value] pairs
                let mut pairs: Vec<Vec<minijinja::Value>> = vec![];

                // Iterate through keys
                for key in value.try_iter()? {
                    let val = value.get_item(&key)?;
                    pairs.push(vec![key.clone(), val]);
                }

                Ok(minijinja::Value::from_serialize(&pairs))
            } else {
                Ok(value)
            }
        });

        // Add 'values' method support for dbt result objects
        env.add_function("values", || -> Vec<String> {
            vec![]
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

        // Preprocess template to handle package namespaces
        // Transform dbt_utils.function() to function() since we registered functions directly
        let preprocessed_sql = self.preprocess_namespaces(sql);

        // Render the template
        let rendered = self.env
            .render_str(&preprocessed_sql, &self.context.to_minijinja_value())
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

    /// Preprocess template to handle package namespaces like dbt_utils.function()
    fn preprocess_namespaces(&self, sql: &str) -> String {
        // Replace package.function_name( with function_name(
        // This handles syntax like {{ dbt_utils.surrogate_key(...) }}, {{ dbt_date.get_date_dimension(...) }}, etc.
        sql
            // dbt_utils package
            .replace("dbt_utils.surrogate_key(", "surrogate_key(")
            .replace("dbt_utils.generate_surrogate_key(", "generate_surrogate_key(")
            .replace("dbt_utils.generate_series(", "generate_series(")
            .replace("dbt_utils.date_spine(", "date_spine(")
            .replace("dbt_utils.union_relations(", "union_relations(")
            .replace("dbt_utils.get_column_values(", "get_column_values(")
            .replace("dbt_utils.concat(", "concat(")
            .replace("dbt_utils.star(", "star(")
            .replace("dbt_utils.pivot(", "pivot(")
            .replace("dbt_utils.unpivot(", "unpivot(")
            .replace("dbt_utils.groupby(", "groupby(")
            .replace("dbt_utils.get_url_host(", "get_url_host(")
            .replace("dbt_utils.get_url_parameter(", "get_url_parameter(")
            // dbt_date package
            .replace("dbt_date.get_date_dimension(", "get_date_dimension(")
            .replace("dbt_date.get_fiscal_periods(", "get_fiscal_periods(")
            // metrics package
            .replace("metrics.calculate(", "calculate(")
            .replace("metrics.metric(", "metric(")
            // dbt cross-database macros
            .replace("dbt.split_part(", "split_part(")
            .replace("dbt.date_spine(", "date_spine(")
            // adapter methods
            .replace("adapter.get_columns_in_relation(", "get_columns_in_relation(")
            // Python-style dict methods - convert to MiniJinja filter syntax
            .replace(".items()", " | items")
            // Convert method calls to property access
            .replace(".values()", ".values")
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
