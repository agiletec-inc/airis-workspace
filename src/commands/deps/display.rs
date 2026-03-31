//! Tree display for dependency graph

use colored::Colorize;
use std::collections::HashSet;

use crate::dag::Dag;

/// Print a single node in the tree
pub(super) fn print_tree(
    dag: &Dag,
    node_id: &str,
    prefix: &str,
    is_last: bool,
    visited: &mut HashSet<String>,
) {
    let connector = if is_last { "└── " } else { "├── " };

    // Check for cycles
    if visited.contains(node_id) {
        println!("{}{}{} {}", prefix, connector, node_id, "(cycle)".red());
        return;
    }

    println!("{}{}{}", prefix, connector, node_id);
    visited.insert(node_id.to_string());

    if let Some(node) = dag.nodes.get(node_id) {
        let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });

        let mut deps: Vec<&String> = node.deps.iter().collect();
        deps.sort();

        for (i, dep) in deps.iter().enumerate() {
            let is_last_child = i == deps.len() - 1;
            print_tree(dag, dep, &child_prefix, is_last_child, visited);
        }
    }

    visited.remove(node_id);
}
