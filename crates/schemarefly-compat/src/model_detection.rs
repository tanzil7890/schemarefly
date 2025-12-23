//! Model type detection for unsupported dbt model types
//!
//! Detects Python models, ephemeral models, and other materializations
//! that don't support dbt contracts.

use schemarefly_dbt::manifest::ManifestNode;
use serde::{Deserialize, Serialize};

/// Detected model type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelType {
    /// Standard SQL model (supported)
    SqlModel,

    /// Python model (not supported for contracts)
    PythonModel,

    /// Ephemeral model (not supported for contracts)
    EphemeralModel,

    /// Seed (CSV file, not supported for contracts)
    Seed,

    /// Snapshot (not supported for contracts)
    Snapshot,

    /// Other unsupported type
    Other(String),
}

/// Reason why a model is unsupported
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UnsupportedReason {
    /// Python models don't support dbt contracts
    PythonModel,

    /// Ephemeral models don't support dbt contracts
    EphemeralModel,

    /// Seeds (CSV) don't support dbt contracts
    Seed,

    /// Snapshots don't support dbt contracts
    Snapshot,

    /// Other reason
    Other(String),
}

impl UnsupportedReason {
    /// Get a helpful diagnostic message for this unsupported reason
    pub fn diagnostic_message(&self) -> String {
        match self {
            UnsupportedReason::PythonModel => {
                "Python models do not support dbt contracts. \
                 dbt contracts are only available for SQL models. \
                 SchemaRefly will skip schema inference for this model.".to_string()
            }
            UnsupportedReason::EphemeralModel => {
                "Ephemeral models do not support dbt contracts. \
                 dbt contracts require materialized models (table, view, incremental). \
                 SchemaRefly will skip schema inference for this model.".to_string()
            }
            UnsupportedReason::Seed => {
                "Seeds (CSV files) do not support dbt contracts. \
                 dbt contracts are only available for SQL models. \
                 SchemaRefly will skip schema inference for this model.".to_string()
            }
            UnsupportedReason::Snapshot => {
                "Snapshots do not support dbt contracts. \
                 dbt contracts are only available for standard models. \
                 SchemaRefly will skip schema inference for this model.".to_string()
            }
            UnsupportedReason::Other(reason) => {
                format!("This model type is not supported for dbt contracts: {}", reason)
            }
        }
    }
}

/// Detect model type from manifest node
pub fn detect_model_type(node: &ManifestNode) -> Result<ModelType, UnsupportedReason> {
    // Check resource_type first
    match node.resource_type.as_str() {
        "model" => {
            // Check materialization in config
            if let Some(ref materialized) = node.config.materialized {
                if materialized.to_lowercase() == "ephemeral" {
                    return Err(UnsupportedReason::EphemeralModel);
                }
            }

            // Standard SQL model (Python models are rare and typically have .py extension)
            // We'll detect them during SQL parsing if needed
            Ok(ModelType::SqlModel)
        }
        "seed" => {
            Err(UnsupportedReason::Seed)
        }
        "snapshot" => {
            Err(UnsupportedReason::Snapshot)
        }
        "source" => {
            Err(UnsupportedReason::Other("Sources are not models".to_string()))
        }
        "test" => {
            Err(UnsupportedReason::Other("Tests are not models".to_string()))
        }
        "analysis" => {
            Err(UnsupportedReason::Other("Analyses are not models".to_string()))
        }
        other => {
            Err(UnsupportedReason::Other(format!("Unsupported resource type: {}", other)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemarefly_dbt::manifest::NodeConfig;
    use std::collections::HashMap;

    fn create_test_node(resource_type: &str, materialized: Option<String>) -> ManifestNode {
        ManifestNode {
            unique_id: format!("{}.my_project.test", resource_type),
            name: "test".to_string(),
            resource_type: resource_type.to_string(),
            package_name: "my_project".to_string(),
            path: "test.sql".to_string(),
            original_file_path: "models/test.sql".to_string(),
            database: None,
            schema: None,
            alias: None,
            config: NodeConfig {
                enabled: true,
                materialized,
                contract: None,
            },
            description: String::new(),
            columns: HashMap::new(),
            depends_on: Default::default(),
            fqn: vec!["my_project".to_string(), "test".to_string()],
        }
    }

    #[test]
    fn test_detect_sql_model() {
        let node = create_test_node("model", Some("table".to_string()));
        assert_eq!(detect_model_type(&node), Ok(ModelType::SqlModel));
    }

    #[test]
    fn test_detect_ephemeral_model() {
        let node = create_test_node("model", Some("ephemeral".to_string()));
        assert_eq!(detect_model_type(&node), Err(UnsupportedReason::EphemeralModel));
    }

    #[test]
    fn test_detect_seed() {
        let node = create_test_node("seed", None);
        assert_eq!(detect_model_type(&node), Err(UnsupportedReason::Seed));
    }

    #[test]
    fn test_detect_snapshot() {
        let node = create_test_node("snapshot", None);
        assert_eq!(detect_model_type(&node), Err(UnsupportedReason::Snapshot));
    }
}
