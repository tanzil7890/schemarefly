//! dbt-specific function extraction and preprocessing
//!
//! Handles dbt Jinja templates like {{ ref('model') }} and {{ source('source', 'table') }}

use schemarefly_dbt::Manifest;
use std::collections::HashMap;

/// A reference to a dbt model or source
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbtReference {
    /// ref('model_name')
    Ref {
        model_name: String,
        /// Resolved unique_id from manifest
        unique_id: Option<String>,
    },

    /// source('source_name', 'table_name')
    Source {
        source_name: String,
        table_name: String,
        /// Resolved unique_id from manifest
        unique_id: Option<String>,
    },
}

/// Extracts dbt-specific functions from SQL
pub struct DbtFunctionExtractor;

impl DbtFunctionExtractor {
    /// Extract all dbt references from SQL
    ///
    /// Returns a list of references found in the SQL.
    pub fn extract(sql: &str) -> Vec<DbtReference> {
        let mut references = Vec::new();

        // Find all {{ }} blocks
        let mut start = 0;
        while let Some(open) = sql[start..].find("{{") {
            let open_pos = start + open;
            if let Some(close) = sql[open_pos..].find("}}") {
                let close_pos = open_pos + close;
                let content = &sql[open_pos + 2..close_pos].trim();

                if let Some(ref_) = Self::parse_ref(content) {
                    references.push(ref_);
                } else if let Some(source) = Self::parse_source(content) {
                    references.push(source);
                }

                start = close_pos + 2;
            } else {
                break;
            }
        }

        references
    }

    /// Parse ref() function
    ///
    /// Examples:
    /// - ref('users')
    /// - ref("users")
    /// - ref('my_model')
    fn parse_ref(content: &str) -> Option<DbtReference> {
        let trimmed = content.trim();

        if !trimmed.starts_with("ref(") {
            return None;
        }

        // Extract model name from ref('model_name')
        let inner = trimmed.strip_prefix("ref(")?.strip_suffix(')')?;
        let model_name = Self::extract_string_literal(inner)?;

        Some(DbtReference::Ref {
            model_name: model_name.to_string(),
            unique_id: None,
        })
    }

    /// Parse source() function
    ///
    /// Examples:
    /// - source('raw', 'users')
    /// - source("raw", "users")
    fn parse_source(content: &str) -> Option<DbtReference> {
        let trimmed = content.trim();

        if !trimmed.starts_with("source(") {
            return None;
        }

        // Extract source and table from source('source_name', 'table_name')
        let inner = trimmed.strip_prefix("source(")?.strip_suffix(')')?;

        // Split by comma
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() != 2 {
            return None;
        }

        let source_name = Self::extract_string_literal(parts[0].trim())?;
        let table_name = Self::extract_string_literal(parts[1].trim())?;

        Some(DbtReference::Source {
            source_name: source_name.to_string(),
            table_name: table_name.to_string(),
            unique_id: None,
        })
    }

    /// Extract string literal from quoted string
    ///
    /// Handles both single and double quotes.
    fn extract_string_literal(s: &str) -> Option<&str> {
        let trimmed = s.trim();

        if let Some(content) = trimmed.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
            return Some(content);
        }

        if let Some(content) = trimmed.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            return Some(content);
        }

        None
    }

    /// Resolve dbt references using a manifest
    ///
    /// Updates the unique_id field in each reference.
    pub fn resolve(references: &mut [DbtReference], manifest: &Manifest) {
        for ref_ in references {
            match ref_ {
                DbtReference::Ref { model_name, unique_id } => {
                    // Find model by name
                    for (node_id, node) in manifest.models() {
                        if node.name == *model_name {
                            *unique_id = Some(node_id.clone());
                            break;
                        }
                    }
                }
                DbtReference::Source {
                    source_name,
                    table_name,
                    unique_id,
                } => {
                    // Find source by source_name and table_name
                    for (source_id, source) in &manifest.sources {
                        if source.source_name == *source_name && source.name == *table_name {
                            *unique_id = Some(source_id.clone());
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Preprocess SQL to replace dbt functions with table names
    ///
    /// This allows the SQL to be parsed by standard SQL parsers.
    /// Returns the preprocessed SQL and a map of replacements.
    pub fn preprocess(sql: &str, manifest: Option<&Manifest>) -> (String, HashMap<String, DbtReference>) {
        let mut result = sql.to_string();
        let mut replacements = HashMap::new();

        let mut references = Self::extract(sql);

        if let Some(manifest) = manifest {
            Self::resolve(&mut references, manifest);
        }

        // Replace each reference with a table name
        for (i, ref_) in references.iter().enumerate() {
            let placeholder = match ref_ {
                DbtReference::Ref { model_name, unique_id } => {
                    // Use the model name or unique_id
                    if let Some(id) = unique_id {
                        // Extract table name from unique_id
                        if let Some(node) = manifest.and_then(|m| m.get_node(id)) {
                            format!("{}.{}.{}",
                                node.database.as_ref().unwrap_or(&"db".to_string()),
                                node.schema.as_ref().unwrap_or(&"schema".to_string()),
                                node.alias.as_ref().unwrap_or(&node.name)
                            )
                        } else {
                            model_name.clone()
                        }
                    } else {
                        model_name.clone()
                    }
                }
                DbtReference::Source { source_name, table_name, unique_id } => {
                    if let Some(id) = unique_id {
                        if let Some(source) = manifest.and_then(|m| m.get_source(id)) {
                            format!("{}.{}.{}",
                                source.database.as_ref().unwrap_or(&"db".to_string()),
                                &source.schema,
                                source.identifier.as_ref().unwrap_or(&source.name)
                            )
                        } else {
                            format!("{}.{}", source_name, table_name)
                        }
                    } else {
                        format!("{}.{}", source_name, table_name)
                    }
                }
            };

            // Find and replace the {{ }} block
            let pattern = format!("{{{{ {} }}}}", Self::reference_pattern(ref_));
            let pattern2 = format!("{{{{{}}}}}", Self::reference_pattern(ref_));

            if let Some(pos) = result.find(&pattern) {
                let replacement_key = format!("__dbt_ref_{}__", i);
                result.replace_range(pos..pos + pattern.len(), &placeholder);
                replacements.insert(replacement_key, ref_.clone());
            } else if let Some(pos) = result.find(&pattern2) {
                let replacement_key = format!("__dbt_ref_{}__", i);
                result.replace_range(pos..pos + pattern2.len(), &placeholder);
                replacements.insert(replacement_key, ref_.clone());
            }
        }

        (result, replacements)
    }

    /// Get the pattern to match for a reference
    fn reference_pattern(ref_: &DbtReference) -> String {
        match ref_ {
            DbtReference::Ref { model_name, .. } => {
                format!("ref('{}')", model_name)
            }
            DbtReference::Source { source_name, table_name, .. } => {
                format!("source('{}', '{}')", source_name, table_name)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_ref() {
        let sql = "SELECT * FROM {{ ref('users') }}";
        let refs = DbtFunctionExtractor::extract(sql);

        assert_eq!(refs.len(), 1);
        assert!(matches!(refs[0], DbtReference::Ref { .. }));

        if let DbtReference::Ref { model_name, .. } = &refs[0] {
            assert_eq!(model_name, "users");
        }
    }

    #[test]
    fn extract_source() {
        let sql = "SELECT * FROM {{ source('raw', 'users') }}";
        let refs = DbtFunctionExtractor::extract(sql);

        assert_eq!(refs.len(), 1);
        assert!(matches!(refs[0], DbtReference::Source { .. }));

        if let DbtReference::Source { source_name, table_name, .. } = &refs[0] {
            assert_eq!(source_name, "raw");
            assert_eq!(table_name, "users");
        }
    }

    #[test]
    fn extract_multiple() {
        let sql = r#"
            WITH base AS (
                SELECT * FROM {{ source('raw', 'users') }}
            ),
            filtered AS (
                SELECT * FROM {{ ref('staging_users') }}
            )
            SELECT * FROM filtered
        "#;

        let refs = DbtFunctionExtractor::extract(sql);
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn preprocess_sql() {
        let sql = "SELECT * FROM {{ ref('users') }} WHERE active = true";
        let (preprocessed, replacements) = DbtFunctionExtractor::preprocess(sql, None);

        // Should have replaced the ref with the model name
        assert!(preprocessed.contains("users"));
        assert!(!preprocessed.contains("{{"));
        assert!(!replacements.is_empty());
    }

    #[test]
    fn preprocess_with_manifest() {
        let sql = "SELECT * FROM {{ ref('users') }}";

        // Load manifest if exists
        let manifest_path = std::path::Path::new("../../fixtures/mini-dbt-project/target/manifest.json");
        if manifest_path.exists() {
            let manifest = Manifest::from_file(manifest_path).unwrap();
            let (preprocessed, _) = DbtFunctionExtractor::preprocess(sql, Some(&manifest));

            // Should have replaced with full table name
            assert!(!preprocessed.contains("{{"));
            assert!(preprocessed.contains("."));
        }
    }
}
