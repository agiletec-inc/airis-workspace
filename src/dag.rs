//! Dependency graph (DAG) construction and traversal
//!
//! Builds a directed acyclic graph from manifest.toml and pnpm-lock.yaml

use anyhow::Result;
use std::collections::{HashMap, HashSet};

/// A node in the dependency graph
#[derive(Debug, Clone)]
pub struct DagNode {
    pub id: String,        // e.g., "apps/focustoday-api"
    #[allow(dead_code)]
    pub name: String,      // e.g., "focustoday-api" or "@agiletec/focustoday-api"
    pub path: String,      // relative path from root
    pub deps: Vec<String>, // IDs of dependencies
}

/// Dependency graph
#[derive(Debug, Default)]
pub struct Dag {
    pub nodes: HashMap<String, DagNode>,
}

impl Dag {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    /// Add a node to the DAG
    pub fn add_node(&mut self, node: DagNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    /// Get node by ID
    #[allow(dead_code)]
    pub fn get(&self, id: &str) -> Option<&DagNode> {
        self.nodes.get(id)
    }

    /// Get topological order starting from target
    /// Returns nodes in dependency-first order
    pub fn topo_order(&self, target: &str) -> Result<Vec<&DagNode>> {
        let mut visited = HashSet::new();
        let mut order = Vec::new();
        let mut stack = HashSet::new();

        self.visit(target, &mut visited, &mut order, &mut stack)?;

        Ok(order)
    }

    fn visit<'a>(
        &'a self,
        id: &str,
        visited: &mut HashSet<String>,
        order: &mut Vec<&'a DagNode>,
        stack: &mut HashSet<String>,
    ) -> Result<()> {
        if visited.contains(id) {
            return Ok(());
        }

        if stack.contains(id) {
            anyhow::bail!("Circular dependency detected: {}", id);
        }

        stack.insert(id.to_string());

        if let Some(node) = self.nodes.get(id) {
            for dep_id in &node.deps {
                self.visit(dep_id, visited, order, stack)?;
            }
            stack.remove(id);
            visited.insert(id.to_string());
            order.push(node);
        }

        Ok(())
    }

    /// Get all dependency paths for a target (in build order)
    pub fn get_dep_paths(&self, target: &str) -> Result<Vec<String>> {
        let order = self.topo_order(target)?;
        Ok(order.iter().map(|n| n.path.clone()).collect())
    }
}

/// Build DAG from workspace map
pub fn build_dag(workspace_map: &HashMap<String, crate::pnpm::WorkspacePackage>) -> Dag {
    let mut dag = Dag::new();

    for (path, pkg) in workspace_map {
        let node = DagNode {
            id: path.clone(),
            name: pkg.name.clone(),
            path: path.clone(),
            deps: pkg.workspace_deps.clone(),
        };
        dag.add_node(node);
    }

    dag
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topo_order() {
        let mut dag = Dag::new();

        dag.add_node(DagNode {
            id: "a".to_string(),
            name: "a".to_string(),
            path: "apps/a".to_string(),
            deps: vec!["b".to_string(), "c".to_string()],
        });

        dag.add_node(DagNode {
            id: "b".to_string(),
            name: "b".to_string(),
            path: "libs/b".to_string(),
            deps: vec!["c".to_string()],
        });

        dag.add_node(DagNode {
            id: "c".to_string(),
            name: "c".to_string(),
            path: "libs/c".to_string(),
            deps: vec![],
        });

        let order = dag.topo_order("a").unwrap();
        let ids: Vec<_> = order.iter().map(|n| n.id.as_str()).collect();

        // c must come before b, b must come before a
        assert!(ids.iter().position(|&x| x == "c") < ids.iter().position(|&x| x == "b"));
        assert!(ids.iter().position(|&x| x == "b") < ids.iter().position(|&x| x == "a"));
    }
}
