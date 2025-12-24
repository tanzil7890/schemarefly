//! dbt context for Jinja templates
//!
//! Provides variables and configuration accessible in dbt Jinja templates.

use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use minijinja::Value as MinijinjaValue;

/// dbt context for Jinja rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbtContext {
    /// Project variables from dbt_project.yml
    pub vars: HashMap<String, serde_json::Value>,

    /// Target configuration (dev, prod, etc.)
    pub target: TargetContext,

    /// Model-specific configuration (renamed from 'config' to avoid shadowing the config() function)
    pub model_config: HashMap<String, serde_json::Value>,

    /// Environment variables (limited set for security)
    pub env_var: HashMap<String, String>,
}

/// Target context (database connection info)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetContext {
    pub name: String,
    pub schema: String,
    pub database: Option<String>,
    #[serde(rename = "type")]
    pub target_type: String,
}

impl DbtContext {
    /// Create a new dbt context
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
            target: TargetContext::default(),
            model_config: HashMap::new(),
            env_var: HashMap::new(),
        }
    }

    /// Add a project variable
    pub fn add_var(&mut self, key: impl Into<String>, value: serde_json::Value) -> &mut Self {
        self.vars.insert(key.into(), value);
        self
    }

    /// Add a model config value
    pub fn add_config(&mut self, key: impl Into<String>, value: serde_json::Value) -> &mut Self {
        self.model_config.insert(key.into(), value);
        self
    }

    /// Set target name
    pub fn set_target_name(&mut self, name: impl Into<String>) -> &mut Self {
        self.target.name = name.into();
        self
    }

    /// Set target schema
    pub fn set_target_schema(&mut self, schema: impl Into<String>) -> &mut Self {
        self.target.schema = schema.into();
        self
    }

    /// Convert to MiniJinja value for rendering
    pub fn to_minijinja_value(&self) -> MinijinjaValue {
        MinijinjaValue::from_serialize(self)
    }
}

impl Default for DbtContext {
    fn default() -> Self {
        let mut vars = HashMap::new();
        // Add common default variables that many dbt projects use
        vars.insert("start_date".to_string(), serde_json::Value::String("1999-01-01".to_string()));

        Self {
            vars,
            target: TargetContext::default(),
            model_config: HashMap::new(),
            env_var: HashMap::new(),
        }
    }
}

impl Default for TargetContext {
    fn default() -> Self {
        Self {
            name: "dev".to_string(),
            schema: "public".to_string(),
            database: None,
            target_type: "postgres".to_string(),
        }
    }
}

/// Builder for DbtContext
pub struct DbtContextBuilder {
    context: DbtContext,
}

impl DbtContextBuilder {
    pub fn new() -> Self {
        Self {
            context: DbtContext::new(),
        }
    }

    pub fn var(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.context.add_var(key, value);
        self
    }

    pub fn target_name(mut self, name: impl Into<String>) -> Self {
        self.context.set_target_name(name);
        self
    }

    pub fn target_schema(mut self, schema: impl Into<String>) -> Self {
        self.context.set_target_schema(schema);
        self
    }

    pub fn build(self) -> DbtContext {
        self.context
    }
}

impl Default for DbtContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}
