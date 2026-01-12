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
///
/// Supports environment variable interpolation for sensitive settings like passwords.
/// When `use_env_vars` is enabled, settings are first looked up as environment
/// variables using the format `SCHEMAREFLY_{KEY}` (e.g., `SCHEMAREFLY_PASSWORD`).
///
/// # Example Configuration (schemarefly.toml)
///
/// ```toml
/// [warehouse]
/// type = "bigquery"
/// use_env_vars = true
///
/// [warehouse.settings]
/// project_id = "my-gcp-project"
/// # password will be read from SCHEMAREFLY_PASSWORD env var
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WarehouseConfig {
    /// Warehouse type: bigquery, snowflake, postgres
    #[serde(rename = "type")]
    pub warehouse_type: String,

    /// Use environment variables for settings (recommended for secrets)
    ///
    /// When enabled, settings are looked up in this order:
    /// 1. Environment variable `SCHEMAREFLY_{KEY}` (uppercase)
    /// 2. Value in `settings` map
    #[serde(default)]
    pub use_env_vars: bool,

    /// Connection settings (warehouse-specific)
    #[serde(default)]
    pub settings: HashMap<String, String>,
}

impl Default for WarehouseConfig {
    fn default() -> Self {
        Self {
            warehouse_type: "bigquery".to_string(),
            use_env_vars: true, // Default to true for security
            settings: HashMap::new(),
        }
    }
}

impl WarehouseConfig {
    /// Create a new WarehouseConfig with the specified type
    pub fn new(warehouse_type: impl Into<String>) -> Self {
        Self {
            warehouse_type: warehouse_type.into(),
            use_env_vars: true,
            settings: HashMap::new(),
        }
    }

    /// Set a configuration setting
    pub fn with_setting(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.settings.insert(key.into(), value.into());
        self
    }

    /// Enable or disable environment variable lookup
    pub fn with_env_vars(mut self, enabled: bool) -> Self {
        self.use_env_vars = enabled;
        self
    }

    /// Get a setting value, checking environment variables first if enabled
    ///
    /// When `use_env_vars` is true, the lookup order is:
    /// 1. Environment variable `SCHEMAREFLY_{KEY}` (key is uppercased)
    /// 2. Value from `settings` map
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = WarehouseConfig::new("snowflake").with_env_vars(true);
    /// // This will check SCHEMAREFLY_PASSWORD first, then settings["password"]
    /// let password = config.get_setting("password");
    /// ```
    pub fn get_setting(&self, key: &str) -> Option<String> {
        if self.use_env_vars {
            // Check environment variable first (SCHEMAREFLY_{KEY})
            let env_key = format!("SCHEMAREFLY_{}", key.to_uppercase());
            if let Ok(value) = std::env::var(&env_key) {
                if !value.is_empty() {
                    return Some(value);
                }
            }

            // Also check common environment variable patterns for credentials
            match key.to_lowercase().as_str() {
                "credentials" | "google_credentials" => {
                    // Check GOOGLE_APPLICATION_CREDENTIALS for BigQuery
                    if let Ok(value) = std::env::var("GOOGLE_APPLICATION_CREDENTIALS") {
                        if !value.is_empty() {
                            return Some(value);
                        }
                    }
                }
                "project_id" | "project" => {
                    // Check GCP_PROJECT or GOOGLE_CLOUD_PROJECT
                    for env_var in ["GCP_PROJECT", "GOOGLE_CLOUD_PROJECT", "GCLOUD_PROJECT"] {
                        if let Ok(value) = std::env::var(env_var) {
                            if !value.is_empty() {
                                return Some(value);
                            }
                        }
                    }
                }
                "account" => {
                    // Check SNOWFLAKE_ACCOUNT
                    if let Ok(value) = std::env::var("SNOWFLAKE_ACCOUNT") {
                        if !value.is_empty() {
                            return Some(value);
                        }
                    }
                }
                "username" | "user" => {
                    // Check common username env vars
                    for env_var in ["SNOWFLAKE_USER", "PGUSER", "DATABASE_USER"] {
                        if let Ok(value) = std::env::var(env_var) {
                            if !value.is_empty() {
                                return Some(value);
                            }
                        }
                    }
                }
                "password" => {
                    // Check common password env vars
                    for env_var in ["SNOWFLAKE_PASSWORD", "PGPASSWORD", "DATABASE_PASSWORD"] {
                        if let Ok(value) = std::env::var(env_var) {
                            if !value.is_empty() {
                                return Some(value);
                            }
                        }
                    }
                }
                "host" => {
                    // Check common host env vars
                    for env_var in ["PGHOST", "DATABASE_HOST"] {
                        if let Ok(value) = std::env::var(env_var) {
                            if !value.is_empty() {
                                return Some(value);
                            }
                        }
                    }
                }
                "port" => {
                    // Check common port env vars
                    for env_var in ["PGPORT", "DATABASE_PORT"] {
                        if let Ok(value) = std::env::var(env_var) {
                            if !value.is_empty() {
                                return Some(value);
                            }
                        }
                    }
                }
                "database" | "dbname" => {
                    // Check common database env vars
                    for env_var in ["PGDATABASE", "DATABASE_NAME"] {
                        if let Ok(value) = std::env::var(env_var) {
                            if !value.is_empty() {
                                return Some(value);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Fall back to settings map
        self.settings.get(key).cloned()
    }

    /// Get a required setting, returning an error if not found
    ///
    /// This is useful for settings that must be provided for the adapter to work.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let project_id = config.require_setting("project_id")?;
    /// ```
    pub fn require_setting(&self, key: &str) -> Result<String, String> {
        self.get_setting(key).ok_or_else(|| {
            if self.use_env_vars {
                format!(
                    "Missing required setting '{}'. Set it in schemarefly.toml [warehouse.settings] \
                     or via SCHEMAREFLY_{} environment variable",
                    key,
                    key.to_uppercase()
                )
            } else {
                format!(
                    "Missing required setting '{}' in [warehouse.settings]",
                    key
                )
            }
        })
    }

    /// Check if a setting exists (either in config or environment)
    pub fn has_setting(&self, key: &str) -> bool {
        self.get_setting(key).is_some()
    }

    /// Get all settings, including those from environment variables
    ///
    /// This merges settings from the config file with environment variables,
    /// with environment variables taking precedence.
    pub fn all_settings(&self) -> HashMap<String, String> {
        let mut result = self.settings.clone();

        if self.use_env_vars {
            // Add any SCHEMAREFLY_* environment variables
            for (key, value) in std::env::vars() {
                if let Some(setting_key) = key.strip_prefix("SCHEMAREFLY_") {
                    let lower_key = setting_key.to_lowercase();
                    result.insert(lower_key, value);
                }
            }
        }

        result
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

    #[test]
    fn warehouse_config_default() {
        let config = WarehouseConfig::default();
        assert_eq!(config.warehouse_type, "bigquery");
        assert!(config.use_env_vars);
        assert!(config.settings.is_empty());
    }

    #[test]
    fn warehouse_config_builder() {
        let config = WarehouseConfig::new("snowflake")
            .with_setting("account", "xy12345")
            .with_setting("username", "user")
            .with_env_vars(false);

        assert_eq!(config.warehouse_type, "snowflake");
        assert!(!config.use_env_vars);
        assert_eq!(config.settings.get("account"), Some(&"xy12345".to_string()));
        assert_eq!(config.settings.get("username"), Some(&"user".to_string()));
    }

    #[test]
    fn warehouse_config_get_setting_from_map() {
        let config = WarehouseConfig::new("bigquery")
            .with_setting("project_id", "my-project")
            .with_env_vars(false); // Disable env vars for this test

        assert_eq!(config.get_setting("project_id"), Some("my-project".to_string()));
        assert_eq!(config.get_setting("nonexistent"), None);
    }

    #[test]
    fn warehouse_config_require_setting() {
        let config = WarehouseConfig::new("bigquery")
            .with_setting("project_id", "my-project")
            .with_env_vars(false);

        assert_eq!(config.require_setting("project_id").unwrap(), "my-project");
        assert!(config.require_setting("nonexistent").is_err());
    }

    #[test]
    fn warehouse_config_has_setting() {
        let config = WarehouseConfig::new("postgres")
            .with_setting("host", "localhost")
            .with_env_vars(false);

        assert!(config.has_setting("host"));
        assert!(!config.has_setting("password"));
    }

    #[test]
    fn warehouse_config_env_var_override() {
        // Set an environment variable for testing
        std::env::set_var("SCHEMAREFLY_TEST_KEY", "env_value");

        let config = WarehouseConfig::new("postgres")
            .with_setting("test_key", "config_value")
            .with_env_vars(true);

        // Environment variable should take precedence
        assert_eq!(config.get_setting("test_key"), Some("env_value".to_string()));

        // Clean up
        std::env::remove_var("SCHEMAREFLY_TEST_KEY");
    }

    #[test]
    fn warehouse_config_env_var_disabled() {
        // Set an environment variable
        std::env::set_var("SCHEMAREFLY_DISABLED_KEY", "env_value");

        let config = WarehouseConfig::new("postgres")
            .with_setting("disabled_key", "config_value")
            .with_env_vars(false); // Disable env var lookup

        // Should use config value since env vars are disabled
        assert_eq!(config.get_setting("disabled_key"), Some("config_value".to_string()));

        // Clean up
        std::env::remove_var("SCHEMAREFLY_DISABLED_KEY");
    }

    #[test]
    fn warehouse_config_all_settings() {
        std::env::set_var("SCHEMAREFLY_EXTRA_KEY", "extra_value");

        let config = WarehouseConfig::new("postgres")
            .with_setting("host", "localhost")
            .with_env_vars(true);

        let all = config.all_settings();
        assert_eq!(all.get("host"), Some(&"localhost".to_string()));
        assert_eq!(all.get("extra_key"), Some(&"extra_value".to_string()));

        // Clean up
        std::env::remove_var("SCHEMAREFLY_EXTRA_KEY");
    }

    #[test]
    fn warehouse_config_toml_parsing() {
        let toml = r#"
            [warehouse]
            type = "snowflake"
            use_env_vars = true

            [warehouse.settings]
            account = "xy12345"
            warehouse = "COMPUTE_WH"
        "#;

        let config: Config = toml::from_str(toml).unwrap();
        let warehouse = config.warehouse.unwrap();

        assert_eq!(warehouse.warehouse_type, "snowflake");
        assert!(warehouse.use_env_vars);
        assert_eq!(warehouse.settings.get("account"), Some(&"xy12345".to_string()));
        assert_eq!(warehouse.settings.get("warehouse"), Some(&"COMPUTE_WH".to_string()));
    }
}
