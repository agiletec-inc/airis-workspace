//! DAG loading and dependency map helpers

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

use crate::dag::{Dag, DagNode, build_dag};
use crate::pnpm::{PnpmLock, build_workspace_map};

/// Load DAG from pnpm-lock.yaml
pub(super) fn load_dag() -> Result<Dag> {
    let lock_path = Path::new("pnpm-lock.yaml");

    if !lock_path.exists() {
        anyhow::bail!(
            "pnpm-lock.yaml not found. Run 'pnpm install' first or ensure you're in the workspace root."
        );
    }

    let lock = PnpmLock::load(lock_path).context("Failed to parse pnpm-lock.yaml")?;
    let workspace_map = build_workspace_map(&lock);
    let dag = build_dag(&workspace_map);

    Ok(dag)
}

/// Build a map of package -> packages that depend on it
pub(super) fn build_dependents_map(dag: &Dag) -> HashMap<String, Vec<String>> {
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

    // Initialize all packages with empty vectors
    for id in dag.nodes.keys() {
        dependents.insert(id.clone(), Vec::new());
    }

    // Build reverse dependency map
    for node in dag.nodes.values() {
        for dep in &node.deps {
            if let Some(list) = dependents.get_mut(dep) {
                list.push(node.id.clone());
            }
        }
    }

    dependents
}

/// Find a package by ID, path, or partial match
pub(super) fn find_package<'a>(dag: &'a Dag, query: &str) -> Result<&'a DagNode> {
    // Exact match by ID
    if let Some(node) = dag.nodes.get(query) {
        return Ok(node);
    }

    // Partial match
    let matches: Vec<&DagNode> = dag
        .nodes
        .values()
        .filter(|n| n.id.contains(query) || n.path.contains(query))
        .collect();

    match matches.len() {
        0 => anyhow::bail!("Package '{}' not found in workspace", query),
        1 => Ok(matches[0]),
        _ => {
            let names: Vec<&str> = matches.iter().map(|n| n.id.as_str()).collect();
            anyhow::bail!(
                "Ambiguous package query '{}'. Matches: {}",
                query,
                names.join(", ")
            );
        }
    }
}
