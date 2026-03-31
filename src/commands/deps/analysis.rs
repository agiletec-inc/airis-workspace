//! Cycle detection and architecture validation

use std::collections::HashSet;

use crate::dag::Dag;

/// Detect cycles in the dependency graph
pub(super) fn detect_cycles(dag: &Dag) -> Vec<Vec<String>> {
    let mut cycles = Vec::new();
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();
    let mut path = Vec::new();

    for id in dag.nodes.keys() {
        if !visited.contains(id) {
            find_cycles_dfs(
                dag,
                id,
                &mut visited,
                &mut rec_stack,
                &mut path,
                &mut cycles,
            );
        }
    }

    cycles
}

fn find_cycles_dfs(
    dag: &Dag,
    node_id: &str,
    visited: &mut HashSet<String>,
    rec_stack: &mut HashSet<String>,
    path: &mut Vec<String>,
    cycles: &mut Vec<Vec<String>>,
) {
    visited.insert(node_id.to_string());
    rec_stack.insert(node_id.to_string());
    path.push(node_id.to_string());

    if let Some(node) = dag.nodes.get(node_id) {
        for dep in &node.deps {
            if !visited.contains(dep) {
                find_cycles_dfs(dag, dep, visited, rec_stack, path, cycles);
            } else if rec_stack.contains(dep) {
                // Found a cycle - extract it
                let cycle_start = path.iter().position(|x| x == dep).unwrap_or(0);
                let cycle: Vec<String> = path[cycle_start..].to_vec();
                if !cycles.iter().any(|c| cycles_equal(c, &cycle)) {
                    cycles.push(cycle);
                }
            }
        }
    }

    path.pop();
    rec_stack.remove(node_id);
}

/// Check if two cycles are equivalent (same nodes, possibly different starting point)
fn cycles_equal(a: &[String], b: &[String]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let a_set: HashSet<&String> = a.iter().collect();
    let b_set: HashSet<&String> = b.iter().collect();

    a_set == b_set
}

/// Check architecture rules:
/// - Apps can only depend on libs
/// - No cross-app dependencies
pub(super) fn check_architecture(dag: &Dag) -> Vec<String> {
    let mut violations = Vec::new();

    for node in dag.nodes.values() {
        let is_app = node.path.starts_with("apps/");

        if is_app {
            for dep in &node.deps {
                // Check if app depends on another app
                if dep.starts_with("apps/") {
                    violations.push(format!("Cross-app dependency: {} → {}", node.id, dep));
                }
            }
        }
    }

    violations
}
