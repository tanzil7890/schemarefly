//! Contract extraction from dbt manifests
//!
//! Extracts contract definitions (enforced schemas) from dbt model configurations.

use schemarefly_core::{Contract, Schema, Column, LogicalType, EnforcementPolicy};
use crate::manifest::{Manifest, ManifestNode};
use std::collections::HashMap;

/// Extract contracts from manifest
pub struct ContractExtractor;

impl ContractExtractor {
    /// Extract all contracts from a manifest
    pub fn extract_all(manifest: &Manifest) -> HashMap<String, Contract> {
        let mut contracts = HashMap::new();

        for (node_id, node) in manifest.models() {
            if let Some(contract) = Self::extract_from_node(node) {
                contracts.insert(node_id.clone(), contract);
            }
        }

        contracts
    }

    /// Extract contract from a single node
    pub fn extract_from_node(node: &ManifestNode) -> Option<Contract> {
        // Only extract if contract is enforced
        let contract_config = node.config.contract.as_ref()?;
        if !contract_config.enforced {
            return None;
        }

        // Build schema from column definitions
        let columns: Vec<Column> = node
            .columns
            .values()
            .filter_map(|col_def| {
                // Only include columns with data_type specified
                let data_type = col_def.data_type.as_ref()?;

                let logical_type = Self::parse_data_type(data_type);

                Some(Column::new(col_def.name.clone(), logical_type))
            })
            .collect();

        if columns.is_empty() {
            return None;
        }

        let schema = Schema::from_columns(columns);

        // Create contract with default enforcement policy
        let contract = Contract::new(schema)
            .with_policy(EnforcementPolicy::default())
            .with_enforced(true);

        Some(contract)
    }

    /// Parse dbt data_type string to LogicalType
    ///
    /// This is a simple parser for common types. More sophisticated parsing
    /// will be added in Phase 3 when we integrate SQL parsing.
    pub fn parse_data_type(data_type: &str) -> LogicalType {
        let lower = data_type.to_lowercase();

        match lower.as_str() {
            // Integers
            "int" | "integer" | "bigint" | "smallint" | "tinyint" | "int64" | "int4" | "int8" => {
                LogicalType::Int
            }

            // Floats
            "float" | "double" | "real" | "float64" | "float8" => LogicalType::Float,

            // Decimals
            s if s.starts_with("decimal") || s.starts_with("numeric") => {
                // Try to parse precision and scale
                // Format: decimal(precision, scale) or numeric(precision, scale)
                if let Some(start) = s.find('(') {
                    if let Some(end) = s.find(')') {
                        let params = &s[start + 1..end];
                        let parts: Vec<&str> = params.split(',').map(|s| s.trim()).collect();

                        let precision = parts.get(0).and_then(|p| p.parse().ok());
                        let scale = parts.get(1).and_then(|s| s.parse().ok());

                        return LogicalType::Decimal { precision, scale };
                    }
                }
                LogicalType::Decimal {
                    precision: None,
                    scale: None,
                }
            }

            // Strings
            "string" | "varchar" | "char" | "text" | "character varying" | "character" => {
                LogicalType::String
            }

            // Booleans
            "bool" | "boolean" => LogicalType::Bool,

            // Dates
            "date" => LogicalType::Date,

            // Timestamps
            "timestamp" | "datetime" | "timestamp_ntz" | "timestamp_ltz" | "timestamp_tz" => {
                LogicalType::Timestamp
            }

            // JSON/Variant
            "json" | "jsonb" | "variant" | "object" => LogicalType::Json,

            // Arrays
            s if s.starts_with("array") => {
                // For now, treat as array of unknown
                LogicalType::Array {
                    element_type: Box::new(LogicalType::Unknown),
                }
            }

            // Structs
            s if s.starts_with("struct") || s.starts_with("record") => {
                // For now, treat as empty struct
                LogicalType::Struct { fields: Vec::new() }
            }

            // Unknown/unsupported
            _ => LogicalType::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parse_data_types() {
        assert!(matches!(
            ContractExtractor::parse_data_type("integer"),
            LogicalType::Int
        ));

        assert!(matches!(
            ContractExtractor::parse_data_type("varchar"),
            LogicalType::String
        ));

        assert!(matches!(
            ContractExtractor::parse_data_type("timestamp"),
            LogicalType::Timestamp
        ));

        match ContractExtractor::parse_data_type("decimal(10, 2)") {
            LogicalType::Decimal { precision, scale } => {
                assert_eq!(precision, Some(10));
                assert_eq!(scale, Some(2));
            }
            _ => panic!("Expected Decimal type"),
        }
    }

    #[test]
    fn extract_contracts_from_manifest() {
        let manifest_path = Path::new("../../fixtures/mini-dbt-project/target/manifest.json");

        if manifest_path.exists() {
            let manifest = Manifest::from_file(manifest_path).unwrap();
            let contracts = ContractExtractor::extract_all(&manifest);

            // Should have extracted contract for users model
            let users_contract = contracts.get("model.mini_dbt_project.users");
            assert!(users_contract.is_some());

            if let Some(contract) = users_contract {
                assert!(contract.enforced);
                assert!(!contract.schema.columns.is_empty());

                // Should have columns: id, name, email, created_at
                let column_names = contract.schema.column_names();
                assert!(column_names.contains(&"id"));
                assert!(column_names.contains(&"name"));
                assert!(column_names.contains(&"email"));
                assert!(column_names.contains(&"created_at"));
            }
        }
    }
}
