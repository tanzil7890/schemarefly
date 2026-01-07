//! dbt Jinja functions
//!
//! Implements dbt-specific Jinja functions like ref(), source(), var(), etc.

use minijinja::{Value, Error, ErrorKind};

/// ref() function - references another dbt model
///
/// Usage in Jinja: {{ ref('model_name') }} or {{ ref('package', 'model_name') }}
/// Returns: model_name (simple table reference)
pub fn ref_function(model_or_package: Value, model_name: Option<Value>) -> Result<Value, Error> {
    if let Some(model) = model_name {
        // Two-argument form: ref('package', 'model')
        let _package = model_or_package.as_str().ok_or_else(|| {
            Error::new(ErrorKind::InvalidOperation, "ref() package must be a string")
        })?;

        let model_str = model.as_str().ok_or_else(|| {
            Error::new(ErrorKind::InvalidOperation, "ref() model name must be a string")
        })?;

        // For now, just return the model name
        // In production, you might want to resolve to schema.model
        Ok(Value::from(model_str))
    } else {
        // Single-argument form: ref('model')
        let model_str = model_or_package.as_str().ok_or_else(|| {
            Error::new(ErrorKind::InvalidOperation, "ref() model name must be a string")
        })?;

        Ok(Value::from(model_str))
    }
}

/// source() function - references a source table
///
/// Usage in Jinja: {{ source('source_name', 'table_name') }}
/// Returns: source_name.table_name or just table_name
pub fn source_function(source_name: Value, table_name: Value) -> Result<Value, Error> {
    let source_str = source_name.as_str().ok_or_else(|| {
        Error::new(ErrorKind::InvalidOperation, "source() source name must be a string")
    })?;

    let table_str = table_name.as_str().ok_or_else(|| {
        Error::new(ErrorKind::InvalidOperation, "source() table name must be a string")
    })?;

    // Return source.table format
    // In production, you might resolve this based on sources.yml
    Ok(Value::from(format!("{}.{}", source_str, table_str)))
}

/// var() function - accesses project variables
///
/// Usage in Jinja: {{ var('variable_name') }} or {{ var('variable_name', 'default') }}
/// Returns: variable value or default
pub fn var_function(var_name: Value, default: Option<Value>) -> Result<Value, Error> {
    let var_str = var_name.as_str().ok_or_else(|| {
        Error::new(ErrorKind::InvalidOperation, "var() variable name must be a string")
    })?;

    // Try to get from context (would need to pass context through)
    // For now, return default if provided, otherwise error
    if let Some(default_val) = default {
        Ok(default_val)
    } else {
        Err(Error::new(
            ErrorKind::UndefinedError,
            format!("Variable '{}' is not defined", var_str),
        ))
    }
}

/// config() function - accesses model configuration
///
/// Usage in Jinja: {{ config(materialized='table') }}
/// Returns: empty string (config is for metadata, not SQL generation)
pub fn config_function(_kwargs: minijinja::value::Kwargs) -> Result<Value, Error> {
    // config() is used for model configuration metadata
    // It doesn't generate SQL output, so we return empty string
    Ok(Value::from(""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ref_single_arg() {
        let result = ref_function(Value::from("my_table"), None).unwrap();
        assert_eq!(result.as_str().unwrap(), "my_table");
    }

    #[test]
    fn test_ref_two_args() {
        let result = ref_function(Value::from("my_package"), Some(Value::from("my_table"))).unwrap();
        assert_eq!(result.as_str().unwrap(), "my_table");
    }

    #[test]
    fn test_source() {
        let result = source_function(Value::from("raw"), Value::from("customers")).unwrap();
        assert_eq!(result.as_str().unwrap(), "raw.customers");
    }

    #[test]
    fn test_var_with_default() {
        let result = var_function(Value::from("my_var"), Some(Value::from("default_value"))).unwrap();
        assert_eq!(result.as_str().unwrap(), "default_value");
    }

    // Note: config_function test removed - Kwargs::new() is private in minijinja 2.14+
    // The config() function is tested via integration tests in the preprocessor module
}
