//! State comparison for Slim CI workflows
//!
//! Compares current dbt manifest against a production state manifest
//! to identify modified models and their downstream impact.

use schemarefly_dbt::{Manifest, ManifestNode, DependencyGraph};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Reason why a model is considered modified
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModificationReason {
    /// Model is new (not in state manifest)
    New,
    /// SQL file content changed (checksum different)
    SqlChanged,
    /// Column definitions changed
    ColumnsChanged,
    /// Dependencies changed (refs to other models)
    DependenciesChanged,
    /// Contract configuration changed
    ContractChanged,
    /// Materialization changed
    MaterializationChanged,
    /// Model was deleted from current manifest
    Deleted,
}

impl std::fmt::Display for ModificationReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModificationReason::New => write!(f, "new model"),
            ModificationReason::SqlChanged => write!(f, "SQL changed"),
            ModificationReason::ColumnsChanged => write!(f, "columns changed"),
            ModificationReason::DependenciesChanged => write!(f, "dependencies changed"),
            ModificationReason::ContractChanged => write!(f, "contract changed"),
            ModificationReason::MaterializationChanged => write!(f, "materialization changed"),
            ModificationReason::Deleted => write!(f, "deleted"),
        }
    }
}

/// A model that has been modified between state and current manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifiedModel {
    /// Unique ID of the model (e.g., "model.project.users")
    pub unique_id: String,
    /// Model name (e.g., "users")
    pub name: String,
    /// Reasons why this model is considered modified
    pub reasons: Vec<ModificationReason>,
    /// Downstream models affected by this change (blast radius)
    pub downstream_impact: Vec<String>,
    /// Count of downstream models
    pub downstream_count: usize,
}

/// Result of comparing current manifest against state manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateComparisonResult {
    /// Models that have been modified
    pub modified_models: Vec<ModifiedModel>,
    /// Models that are new (not in state)
    pub new_models: Vec<String>,
    /// Models that were deleted (in state but not in current)
    pub deleted_models: Vec<String>,
    /// Total blast radius (unique downstream models affected)
    pub total_blast_radius: usize,
    /// All affected model IDs (modified + their downstream)
    pub all_affected_models: HashSet<String>,
}

impl StateComparisonResult {
    /// Check if there are any changes
    pub fn has_changes(&self) -> bool {
        !self.modified_models.is_empty() || !self.new_models.is_empty() || !self.deleted_models.is_empty()
    }

    /// Get all modified model IDs (for filtering)
    pub fn modified_model_ids(&self) -> HashSet<String> {
        self.modified_models
            .iter()
            .map(|m| m.unique_id.clone())
            .collect()
    }
}

/// Compares two dbt manifests to identify changes
pub struct StateComparison;

impl StateComparison {
    /// Compare current manifest against a state (production) manifest
    ///
    /// Returns a StateComparisonResult containing:
    /// - Modified models with reasons
    /// - New models (in current but not state)
    /// - Deleted models (in state but not current)
    /// - Blast radius analysis
    pub fn compare(current: &Manifest, state: &Manifest) -> StateComparisonResult {
        let mut modified_models = Vec::new();
        let mut new_models = Vec::new();
        let mut deleted_models = Vec::new();
        let mut all_affected_models = HashSet::new();

        // Build dependency graph from current manifest for downstream analysis
        let current_dag = DependencyGraph::from_manifest(current);

        // Get models from both manifests
        let current_models = current.models();
        let state_models = state.models();

        // Find new and modified models
        for (node_id, current_node) in &current_models {
            if let Some(state_node) = state_models.get(node_id) {
                // Model exists in both - check for modifications
                let reasons = Self::detect_modifications(current_node, state_node);

                if !reasons.is_empty() {
                    // Model has been modified
                    let downstream = current_dag.downstream(node_id);
                    let downstream_count = downstream.len();

                    // Add model and its downstream to affected set
                    all_affected_models.insert(node_id.clone());
                    all_affected_models.extend(downstream.iter().cloned());

                    modified_models.push(ModifiedModel {
                        unique_id: node_id.clone(),
                        name: current_node.name.clone(),
                        reasons,
                        downstream_impact: downstream,
                        downstream_count,
                    });
                }
            } else {
                // Model is new (not in state)
                let downstream = current_dag.downstream(node_id);

                all_affected_models.insert(node_id.clone());
                all_affected_models.extend(downstream.iter().cloned());

                modified_models.push(ModifiedModel {
                    unique_id: node_id.clone(),
                    name: current_node.name.clone(),
                    reasons: vec![ModificationReason::New],
                    downstream_impact: downstream.clone(),
                    downstream_count: downstream.len(),
                });

                new_models.push(node_id.clone());
            }
        }

        // Find deleted models (in state but not in current)
        for node_id in state_models.keys() {
            if !current_models.contains_key(node_id) {
                deleted_models.push(node_id.clone());
                all_affected_models.insert(node_id.clone());
            }
        }

        // Sort for deterministic output
        modified_models.sort_by(|a, b| a.unique_id.cmp(&b.unique_id));
        new_models.sort();
        deleted_models.sort();

        let total_blast_radius = all_affected_models.len();

        StateComparisonResult {
            modified_models,
            new_models,
            deleted_models,
            total_blast_radius,
            all_affected_models,
        }
    }

    /// Detect specific modifications between two versions of a model
    fn detect_modifications(current: &ManifestNode, state: &ManifestNode) -> Vec<ModificationReason> {
        let mut reasons = Vec::new();

        // Check SQL file path change (indicates potential content change)
        // Note: dbt doesn't include raw SQL in manifest, so we compare paths
        // For actual content comparison, we'd need to read the files
        if current.path != state.path || current.original_file_path != state.original_file_path {
            reasons.push(ModificationReason::SqlChanged);
        }

        // Check column changes
        if Self::columns_changed(&current.columns, &state.columns) {
            reasons.push(ModificationReason::ColumnsChanged);
        }

        // Check dependency changes
        if Self::dependencies_changed(&current.depends_on.nodes, &state.depends_on.nodes) {
            reasons.push(ModificationReason::DependenciesChanged);
        }

        // Check contract changes
        if Self::contract_changed(&current.config.contract, &state.config.contract) {
            reasons.push(ModificationReason::ContractChanged);
        }

        // Check materialization changes
        if current.config.materialized != state.config.materialized {
            reasons.push(ModificationReason::MaterializationChanged);
        }

        reasons
    }

    /// Check if columns have changed
    fn columns_changed(
        current: &HashMap<String, schemarefly_dbt::ColumnDefinition>,
        state: &HashMap<String, schemarefly_dbt::ColumnDefinition>,
    ) -> bool {
        // Different number of columns
        if current.len() != state.len() {
            return true;
        }

        // Check each column
        for (name, current_col) in current {
            match state.get(name) {
                Some(state_col) => {
                    // Check data type changes
                    if current_col.data_type != state_col.data_type {
                        return true;
                    }
                }
                None => return true, // Column not in state
            }
        }

        false
    }

    /// Check if dependencies have changed
    fn dependencies_changed(current: &[String], state: &[String]) -> bool {
        let current_set: HashSet<_> = current.iter().collect();
        let state_set: HashSet<_> = state.iter().collect();
        current_set != state_set
    }

    /// Check if contract configuration has changed
    fn contract_changed(
        current: &Option<schemarefly_dbt::ContractConfig>,
        state: &Option<schemarefly_dbt::ContractConfig>,
    ) -> bool {
        match (current, state) {
            (Some(c), Some(s)) => c.enforced != s.enforced,
            (Some(_), None) | (None, Some(_)) => true,
            (None, None) => false,
        }
    }

    /// Compare SQL file contents to detect changes
    /// This is a more accurate method when file access is available
    pub fn sql_files_differ(current_path: &str, state_path: &str) -> bool {
        use std::path::Path;

        let current = Path::new(current_path);
        let state = Path::new(state_path);

        // If either file doesn't exist, consider them different
        if !current.exists() || !state.exists() {
            return true;
        }

        // Compare file contents
        match (std::fs::read_to_string(current), std::fs::read_to_string(state)) {
            (Ok(c), Ok(s)) => {
                // Normalize whitespace for comparison
                let c_normalized = c.split_whitespace().collect::<Vec<_>>().join(" ");
                let s_normalized = s.split_whitespace().collect::<Vec<_>>().join(" ");
                c_normalized != s_normalized
            }
            _ => true, // If we can't read either file, consider them different
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemarefly_dbt::{ManifestMetadata, NodeConfig, DependsOn, ColumnDefinition, ContractConfig};
    use std::collections::HashMap;

    fn create_test_manifest(models: Vec<(&str, &str, Vec<&str>)>) -> Manifest {
        let mut nodes = HashMap::new();

        for (id, name, deps) in models {
            nodes.insert(
                id.to_string(),
                ManifestNode {
                    unique_id: id.to_string(),
                    name: name.to_string(),
                    resource_type: "model".to_string(),
                    package_name: "test".to_string(),
                    path: format!("models/{}.sql", name),
                    original_file_path: format!("models/{}.sql", name),
                    database: Some("db".to_string()),
                    schema: Some("schema".to_string()),
                    alias: None,
                    config: NodeConfig::default(),
                    description: String::new(),
                    columns: HashMap::new(),
                    depends_on: DependsOn {
                        nodes: deps.into_iter().map(String::from).collect(),
                    },
                    fqn: vec![name.to_string()],
                },
            );
        }

        Manifest {
            metadata: ManifestMetadata {
                dbt_schema_version: "1.0".to_string(),
                dbt_version: "1.7.0".to_string(),
                generated_at: "2024-01-01".to_string(),
                invocation_id: None,
            },
            nodes,
            sources: HashMap::new(),
            parent_map: HashMap::new(),
            child_map: HashMap::new(),
        }
    }

    #[test]
    fn test_no_changes() {
        let manifest = create_test_manifest(vec![
            ("model.test.a", "a", vec![]),
            ("model.test.b", "b", vec!["model.test.a"]),
        ]);

        let result = StateComparison::compare(&manifest, &manifest);

        assert!(!result.has_changes());
        assert!(result.modified_models.is_empty());
        assert!(result.new_models.is_empty());
        assert!(result.deleted_models.is_empty());
    }

    #[test]
    fn test_new_model() {
        let state = create_test_manifest(vec![
            ("model.test.a", "a", vec![]),
        ]);

        let current = create_test_manifest(vec![
            ("model.test.a", "a", vec![]),
            ("model.test.b", "b", vec!["model.test.a"]),
        ]);

        let result = StateComparison::compare(&current, &state);

        assert!(result.has_changes());
        assert_eq!(result.new_models.len(), 1);
        assert!(result.new_models.contains(&"model.test.b".to_string()));
    }

    #[test]
    fn test_deleted_model() {
        let state = create_test_manifest(vec![
            ("model.test.a", "a", vec![]),
            ("model.test.b", "b", vec!["model.test.a"]),
        ]);

        let current = create_test_manifest(vec![
            ("model.test.a", "a", vec![]),
        ]);

        let result = StateComparison::compare(&current, &state);

        assert!(result.has_changes());
        assert_eq!(result.deleted_models.len(), 1);
        assert!(result.deleted_models.contains(&"model.test.b".to_string()));
    }

    #[test]
    fn test_dependency_change() {
        let state = create_test_manifest(vec![
            ("model.test.a", "a", vec![]),
            ("model.test.b", "b", vec![]),
            ("model.test.c", "c", vec!["model.test.a"]),
        ]);

        // Change c to depend on b instead of a
        let current = create_test_manifest(vec![
            ("model.test.a", "a", vec![]),
            ("model.test.b", "b", vec![]),
            ("model.test.c", "c", vec!["model.test.b"]),
        ]);

        let result = StateComparison::compare(&current, &state);

        assert!(result.has_changes());
        let modified = result.modified_models.iter()
            .find(|m| m.unique_id == "model.test.c")
            .expect("Model c should be modified");

        assert!(modified.reasons.contains(&ModificationReason::DependenciesChanged));
    }

    #[test]
    fn test_blast_radius() {
        let state = create_test_manifest(vec![
            ("model.test.a", "a", vec![]),
            ("model.test.b", "b", vec!["model.test.a"]),
            ("model.test.c", "c", vec!["model.test.b"]),
            ("model.test.d", "d", vec!["model.test.c"]),
        ]);

        // Modify model b's dependencies
        let current = create_test_manifest(vec![
            ("model.test.a", "a", vec![]),
            ("model.test.b", "b", vec![]), // Changed: removed dependency on a
            ("model.test.c", "c", vec!["model.test.b"]),
            ("model.test.d", "d", vec!["model.test.c"]),
        ]);

        let result = StateComparison::compare(&current, &state);

        // Model b changed, and c and d are downstream
        let modified_b = result.modified_models.iter()
            .find(|m| m.unique_id == "model.test.b")
            .expect("Model b should be modified");

        assert_eq!(modified_b.downstream_count, 2); // c and d
        assert!(modified_b.downstream_impact.contains(&"model.test.c".to_string()));
        assert!(modified_b.downstream_impact.contains(&"model.test.d".to_string()));
    }
}
