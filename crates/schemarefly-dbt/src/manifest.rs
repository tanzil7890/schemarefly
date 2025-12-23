//! dbt manifest.json parsing
//!
//! Parses dbt-generated manifest.json to extract models, sources, and dependencies.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// dbt manifest.json structure (subset of fields we care about)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// Metadata about the manifest
    pub metadata: ManifestMetadata,

    /// Model and test nodes
    pub nodes: HashMap<String, ManifestNode>,

    /// Source definitions
    pub sources: HashMap<String, ManifestSource>,

    /// Parent map (node -> list of parent nodes)
    #[serde(default)]
    pub parent_map: HashMap<String, Vec<String>>,

    /// Child map (node -> list of child nodes)
    #[serde(default)]
    pub child_map: HashMap<String, Vec<String>>,
}

impl Manifest {
    /// Load manifest from file
    pub fn from_file(path: &Path) -> Result<Self, ManifestError> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| ManifestError::IoError(path.display().to_string(), e.to_string()))?;

        Self::from_str(&contents)
    }

    /// Parse manifest from JSON string
    pub fn from_str(json: &str) -> Result<Self, ManifestError> {
        serde_json::from_str(json)
            .map_err(|e| ManifestError::ParseError(e.to_string()))
    }

    /// Get all model nodes (filters out tests, seeds, etc.)
    pub fn models(&self) -> HashMap<String, &ManifestNode> {
        self.nodes
            .iter()
            .filter(|(_, node)| node.resource_type == "model")
            .map(|(id, node)| (id.clone(), node))
            .collect()
    }

    /// Get a specific node by unique_id
    pub fn get_node(&self, unique_id: &str) -> Option<&ManifestNode> {
        self.nodes.get(unique_id)
    }

    /// Get a specific source by unique_id
    pub fn get_source(&self, unique_id: &str) -> Option<&ManifestSource> {
        self.sources.get(unique_id)
    }
}

/// Manifest metadata
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestMetadata {
    pub dbt_schema_version: String,
    pub dbt_version: String,
    pub generated_at: String,
    #[serde(default)]
    pub invocation_id: Option<String>,
}

/// A node in the manifest (model, test, snapshot, etc.)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestNode {
    /// Unique identifier (e.g., "model.my_project.users")
    pub unique_id: String,

    /// Node name (e.g., "users")
    pub name: String,

    /// Resource type (model, test, snapshot, etc.)
    pub resource_type: String,

    /// Package name
    pub package_name: String,

    /// Relative path to SQL file
    pub path: String,

    /// Original file path
    pub original_file_path: String,

    /// Database name
    #[serde(default)]
    pub database: Option<String>,

    /// Schema name
    #[serde(default)]
    pub schema: Option<String>,

    /// Alias (output table name)
    #[serde(default)]
    pub alias: Option<String>,

    /// Node configuration
    #[serde(default)]
    pub config: NodeConfig,

    /// Description
    #[serde(default)]
    pub description: String,

    /// Column definitions
    #[serde(default)]
    pub columns: HashMap<String, ColumnDefinition>,

    /// Dependencies
    #[serde(default)]
    pub depends_on: DependsOn,

    /// Fully qualified name
    #[serde(default)]
    pub fqn: Vec<String>,
}

/// Node configuration (from dbt_project.yml or model config)
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Whether the node is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Materialization type
    #[serde(default)]
    pub materialized: Option<String>,

    /// Contract configuration
    #[serde(default)]
    pub contract: Option<ContractConfig>,
}

fn default_true() -> bool {
    true
}

/// Contract configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractConfig {
    /// Whether the contract is enforced
    pub enforced: bool,
}

/// Column definition from manifest
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnDefinition {
    /// Column name
    pub name: String,

    /// Description
    #[serde(default)]
    pub description: String,

    /// Data type (if specified in contract)
    #[serde(default)]
    pub data_type: Option<String>,
}

/// Dependencies structure
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DependsOn {
    /// List of node unique_ids this node depends on
    #[serde(default)]
    pub nodes: Vec<String>,
}

/// A source in the manifest
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestSource {
    /// Unique identifier (e.g., "source.my_project.raw.users")
    pub unique_id: String,

    /// Source name (e.g., "raw")
    pub source_name: String,

    /// Table name (e.g., "users")
    pub name: String,

    /// Database name
    #[serde(default)]
    pub database: Option<String>,

    /// Schema name
    pub schema: String,

    /// Identifier (actual table name)
    #[serde(default)]
    pub identifier: Option<String>,

    /// Column definitions
    #[serde(default)]
    pub columns: HashMap<String, ColumnDefinition>,
}

/// Manifest parsing errors
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("Failed to read manifest file {0}: {1}")]
    IoError(String, String),

    #[error("Failed to parse manifest JSON: {0}")]
    ParseError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fixture_manifest() {
        let manifest_path = Path::new("../../fixtures/mini-dbt-project/target/manifest.json");

        if manifest_path.exists() {
            let manifest = Manifest::from_file(manifest_path).unwrap();

            assert_eq!(manifest.metadata.dbt_version, "1.7.0");

            let models = manifest.models();
            assert!(!models.is_empty());

            // Check that we can find the users model
            let users_model = manifest.get_node("model.mini_dbt_project.users");
            assert!(users_model.is_some());

            if let Some(users) = users_model {
                assert_eq!(users.name, "users");
                assert_eq!(users.resource_type, "model");

                // Check contract is enforced
                assert!(users.config.contract.is_some());
                if let Some(contract) = &users.config.contract {
                    assert!(contract.enforced);
                }

                // Check columns
                assert!(users.columns.contains_key("id"));
                assert!(users.columns.contains_key("name"));
            }
        }
    }
}
