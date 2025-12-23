//! Dependency graph (DAG) construction and traversal
//!
//! Builds forward and reverse dependency graphs for impact analysis.

use std::collections::{HashMap, HashSet, VecDeque};
use crate::manifest::Manifest;

/// Node identifier (unique_id from manifest)
pub type NodeId = String;

/// Dependency graph with forward and reverse edges
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    /// Forward edges: node -> list of nodes it depends on (parents)
    parents: HashMap<NodeId, Vec<NodeId>>,

    /// Reverse edges: node -> list of nodes that depend on it (children)
    children: HashMap<NodeId, Vec<NodeId>>,

    /// All nodes in the graph
    nodes: HashSet<NodeId>,
}

impl DependencyGraph {
    /// Build a dependency graph from a manifest
    pub fn from_manifest(manifest: &Manifest) -> Self {
        let mut parents: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let mut children: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let mut nodes: HashSet<NodeId> = HashSet::new();

        // Use parent_map and child_map from manifest if available
        if !manifest.parent_map.is_empty() && !manifest.child_map.is_empty() {
            for (node_id, parent_ids) in &manifest.parent_map {
                nodes.insert(node_id.clone());
                parents.insert(node_id.clone(), parent_ids.clone());

                // Also add parents to nodes set
                for parent_id in parent_ids {
                    nodes.insert(parent_id.clone());
                }
            }

            for (node_id, child_ids) in &manifest.child_map {
                nodes.insert(node_id.clone());
                children.insert(node_id.clone(), child_ids.clone());

                // Also add children to nodes set
                for child_id in child_ids {
                    nodes.insert(child_id.clone());
                }
            }
        } else {
            // Build from depends_on if parent/child maps not available
            for (node_id, node) in &manifest.nodes {
                nodes.insert(node_id.clone());

                let deps = &node.depends_on.nodes;
                if !deps.is_empty() {
                    parents.insert(node_id.clone(), deps.clone());

                    // Build reverse edges
                    for dep_id in deps {
                        children
                            .entry(dep_id.clone())
                            .or_insert_with(Vec::new)
                            .push(node_id.clone());

                        nodes.insert(dep_id.clone());
                    }
                }
            }

            // Add sources to nodes
            for source_id in manifest.sources.keys() {
                nodes.insert(source_id.clone());
            }
        }

        Self {
            parents,
            children,
            nodes,
        }
    }

    /// Get all nodes in the graph
    pub fn all_nodes(&self) -> Vec<&NodeId> {
        self.nodes.iter().collect()
    }

    /// Get immediate parents (dependencies) of a node
    pub fn parents(&self, node_id: &str) -> Vec<&NodeId> {
        self.parents
            .get(node_id)
            .map(|deps| deps.iter().collect())
            .unwrap_or_default()
    }

    /// Get immediate children (dependents) of a node
    pub fn children(&self, node_id: &str) -> Vec<&NodeId> {
        self.children
            .get(node_id)
            .map(|deps| deps.iter().collect())
            .unwrap_or_default()
    }

    /// Get all downstream nodes (transitive closure of children)
    ///
    /// This is the "blast radius" - all models affected if this node changes.
    pub fn downstream(&self, node_id: &str) -> Vec<NodeId> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut result = Vec::new();

        // Start with immediate children
        if let Some(children) = self.children.get(node_id) {
            for child in children {
                queue.push_back(child.clone());
            }
        }

        // BFS to find all downstream nodes
        while let Some(current) = queue.pop_front() {
            if visited.contains(&current) {
                continue;
            }

            visited.insert(current.clone());
            result.push(current.clone());

            // Add children of current node to queue
            if let Some(children) = self.children.get(&current) {
                for child in children {
                    if !visited.contains(child) {
                        queue.push_back(child.clone());
                    }
                }
            }
        }

        result
    }

    /// Get all upstream nodes (transitive closure of parents)
    pub fn upstream(&self, node_id: &str) -> Vec<NodeId> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut result = Vec::new();

        // Start with immediate parents
        if let Some(parents) = self.parents.get(node_id) {
            for parent in parents {
                queue.push_back(parent.clone());
            }
        }

        // BFS to find all upstream nodes
        while let Some(current) = queue.pop_front() {
            if visited.contains(&current) {
                continue;
            }

            visited.insert(current.clone());
            result.push(current.clone());

            // Add parents of current node to queue
            if let Some(parents) = self.parents.get(&current) {
                for parent in parents {
                    if !visited.contains(parent) {
                        queue.push_back(parent.clone());
                    }
                }
            }
        }

        result
    }

    /// Check if there's a path from source to target
    pub fn has_path(&self, source: &str, target: &str) -> bool {
        let downstream = self.downstream(source);
        downstream.contains(&target.to_string())
    }

    /// Get topological sort of all nodes
    pub fn topological_sort(&self) -> Option<Vec<NodeId>> {
        let mut in_degree: HashMap<NodeId, usize> = HashMap::new();
        let mut result = Vec::new();
        let mut queue = VecDeque::new();

        // Calculate in-degrees
        for node in &self.nodes {
            in_degree.insert(node.clone(), 0);
        }

        for (_, parents) in &self.parents {
            for parent in parents {
                *in_degree.entry(parent.clone()).or_insert(0) += 0;
            }
        }

        for (node, parents) in &self.parents {
            *in_degree.get_mut(node).unwrap() = parents.len();
        }

        // Find nodes with no dependencies
        for (node, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(node.clone());
            }
        }

        // Kahn's algorithm
        while let Some(node) = queue.pop_front() {
            result.push(node.clone());

            // For each child, decrease in-degree
            if let Some(children) = self.children.get(&node) {
                for child in children {
                    if let Some(degree) = in_degree.get_mut(child) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(child.clone());
                        }
                    }
                }
            }
        }

        // Check if all nodes were visited (no cycles)
        if result.len() == self.nodes.len() {
            Some(result)
        } else {
            None // Graph has cycles
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Manifest;
    use std::path::Path;

    #[test]
    fn build_dag_from_manifest() {
        let manifest_path = Path::new("../../fixtures/mini-dbt-project/target/manifest.json");

        if manifest_path.exists() {
            let manifest = Manifest::from_file(manifest_path).unwrap();
            let dag = DependencyGraph::from_manifest(&manifest);

            // Should have nodes
            assert!(!dag.all_nodes().is_empty());

            // Users model should have the raw.users source as parent
            let users_parents = dag.parents("model.mini_dbt_project.users");
            assert!(!users_parents.is_empty());

            // Source should have users model as child
            let source_children = dag.children("source.mini_dbt_project.raw.users");
            assert!(!source_children.is_empty());
        }
    }

    #[test]
    fn downstream_impact() {
        let manifest_path = Path::new("../../fixtures/mini-dbt-project/target/manifest.json");

        if manifest_path.exists() {
            let manifest = Manifest::from_file(manifest_path).unwrap();
            let dag = DependencyGraph::from_manifest(&manifest);

            // Get downstream of source - should include users model
            let downstream = dag.downstream("source.mini_dbt_project.raw.users");

            // Should contain the users model
            assert!(downstream.contains(&"model.mini_dbt_project.users".to_string()));
        }
    }
}
