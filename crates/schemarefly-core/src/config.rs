//! Configuration schema (schemarefly.toml)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::diagnostic::{DiagnosticCode, Severity};

/// SQL dialect configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DialectConfig {
    /// BigQuery SQL dialect
    BigQuery,

    /// Snowflake SQL dialect
    Snowflake,

    /// PostgreSQL SQL dialect
    Postgres,

    /// Generic ANSI SQL
    Ansi,
}

impl Default for DialectConfig {
    fn default() -> Self {
        Self::Ansi
    }
}

/// Severity threshold overrides for specific diagnostic codes
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeverityThreshold {
    /// Map of diagnostic code to severity override
    pub overrides: HashMap<String, Severity>,
}

impl Default for SeverityThreshold {
    fn default() -> Self {
        Self {
            overrides: HashMap::new(),
        }
    }
}

impl SeverityThreshold {
    /// Get severity for a diagnostic code, or default
    pub fn get_severity(&self, code: DiagnosticCode, default: Severity) -> Severity {
        self.overrides
            .get(code.as_str())
            .copied()
            .unwrap_or(default)
    }

    /// Set severity override for a code
    pub fn set_override(&mut self, code: DiagnosticCode, severity: Severity) {
        self.overrides.insert(code.as_str().to_string(), severity);
    }
}

/// Warehouse connection configuration for drift detection
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WarehouseConfig {
    /// Warehouse type (bigquery, snowflake, etc.)
    #[serde(rename = "type")]
    pub warehouse_type: String,

    /// Connection settings (warehouse-specific)
    #[serde(flatten)]
    pub settings: HashMap<String, String>,
}

impl Default for WarehouseConfig {
    fn default() -> Self {
        Self {
            warehouse_type: "bigquery".to_string(),
            settings: HashMap::new(),
        }
    }
}

/// Allowlist rules for specific models or patterns
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AllowlistRules {
    /// Allow type widening for these models (glob patterns)
    #[serde(default)]
    pub allow_widening: Vec<String>,

    /// Allow extra columns for these models (glob patterns)
    #[serde(default)]
    pub allow_extra_columns: Vec<String>,

    /// Completely skip checks for these models (glob patterns)
    #[serde(default)]
    pub skip_models: Vec<String>,
}

impl Default for AllowlistRules {
    fn default() -> Self {
        Self {
            allow_widening: Vec::new(),
            allow_extra_columns: Vec::new(),
            skip_models: Vec::new(),
        }
    }
}

impl AllowlistRules {
    /// Check if a model matches any pattern in the list
    fn matches_pattern(model: &str, patterns: &[String]) -> bool {
        patterns.iter().any(|pattern| {
            // Simple glob matching (* and **)
            if pattern.contains('*') {
                glob_match(pattern, model)
            } else {
                pattern == model
            }
        })
    }

    /// Check if widening is allowed for a model
    pub fn is_widening_allowed(&self, model: &str) -> bool {
        Self::matches_pattern(model, &self.allow_widening)
    }

    /// Check if extra columns are allowed for a model
    pub fn are_extra_columns_allowed(&self, model: &str) -> bool {
        Self::matches_pattern(model, &self.allow_extra_columns)
    }

    /// Check if a model should be skipped
    pub fn is_model_skipped(&self, model: &str) -> bool {
        Self::matches_pattern(model, &self.skip_models)
    }
}

/// Main configuration structure
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// SQL dialect
    #[serde(default)]
    pub dialect: DialectConfig,

    /// Severity thresholds
    #[serde(default)]
    pub severity: SeverityThreshold,

    /// Allowlist rules
    #[serde(default)]
    pub allowlist: AllowlistRules,

    /// Warehouse connection configuration (for drift detection)
    #[serde(default)]
    pub warehouse: Option<WarehouseConfig>,

    /// Redact sensitive data (schema names, column names, table names) in diagnostics and logs
    /// This is useful for privacy/security when sharing reports or logs
    #[serde(default)]
    pub redact_sensitive_data: bool,

    /// Project root path (for resolving relative paths)
    #[serde(skip)]
    pub project_root: std::path::PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dialect: DialectConfig::default(),
            severity: SeverityThreshold::default(),
            allowlist: AllowlistRules::default(),
            warehouse: None,
            redact_sensitive_data: false,
            project_root: std::env::current_dir().unwrap_or_default(),
        }
    }
}

impl Config {
    /// Load config from TOML file
    pub fn from_file(path: &std::path::Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::IoError(e.to_string()))?;

        let mut config: Config = toml::from_str(&contents)
            .map_err(|e| ConfigError::ParseError(e.to_string()))?;

        // Set project root to parent of config file
        if let Some(parent) = path.parent() {
            config.project_root = parent.to_path_buf();
        }

        Ok(config)
    }

    /// Load config from TOML string
    pub fn from_toml(toml: &str) -> Result<Self, ConfigError> {
        toml::from_str(toml)
            .map_err(|e| ConfigError::ParseError(e.to_string()))
    }

    /// Save config to TOML file
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<(), ConfigError> {
        let toml = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::SerializeError(e.to_string()))?;

        std::fs::write(path, toml)
            .map_err(|e| ConfigError::IoError(e.to_string()))?;

        Ok(())
    }
}

/// Simple glob matching (supports * and **)
fn glob_match(pattern: &str, text: &str) -> bool {
    // Very simple implementation - just handle basic * wildcard
    if pattern == "*" || pattern == "**" {
        return true;
    }

    if let Some(star_pos) = pattern.find('*') {
        let prefix = &pattern[..star_pos];
        let suffix = &pattern[star_pos + 1..];

        text.starts_with(prefix) && text.ends_with(suffix)
    } else {
        pattern == text
    }
}

/// Config error types
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    IoError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Serialize error: {0}")]
    SerializeError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = Config::default();
        assert_eq!(config.dialect, DialectConfig::Ansi);
    }

    #[test]
    fn severity_override() {
        let mut threshold = SeverityThreshold::default();
        threshold.set_override(DiagnosticCode::ContractExtraColumn, Severity::Warn);

        assert_eq!(
            threshold.get_severity(DiagnosticCode::ContractExtraColumn, Severity::Error),
            Severity::Warn
        );
    }

    #[test]
    fn allowlist_pattern_matching() {
        let mut rules = AllowlistRules::default();
        rules.allow_extra_columns = vec!["staging.*".to_string()];

        assert!(rules.are_extra_columns_allowed("staging.users"));
        assert!(!rules.are_extra_columns_allowed("prod.users"));
    }

    #[test]
    fn config_toml_roundtrip() {
        let config = Config::default();
        let toml = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml).unwrap();
        assert_eq!(config.dialect, parsed.dialect);
    }

    #[test]
    fn glob_matching() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("staging.*", "staging.users"));
        assert!(glob_match("*.sql", "model.sql"));
        assert!(!glob_match("staging.*", "prod.users"));
    }
}
