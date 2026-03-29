//! Dependency graph visualization and analysis
//!
//! Provides commands to visualize and analyze the workspace dependency graph.

use anyhow::{Context, Result};
use colored::Colorize;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::dag::{Dag, DagNode, build_dag};
use crate::pnpm::{PnpmLock, build_workspace_map};

/// Dependency graph output for JSON serialization
#[derive(Serialize)]
struct DepsJson {
    format: &'static str,
    packages: Vec<PackageInfo>,
    edges: Vec<Edge>,
    cycles: Vec<Vec<String>>,
}

#[derive(Serialize)]
struct PackageInfo {
    id: String,
    path: String,
    #[serde(rename = "type")]
    pkg_type: String,
    deps_count: usize,
    dependents_count: usize,
}

#[derive(Serialize)]
struct Edge {
    from: String,
    to: String,
}

/// Show ASCII dependency tree
pub fn tree() -> Result<()> {
    let dag = load_dag()?;

    if dag.nodes.is_empty() {
        println!("{}", "No packages found in workspace".yellow());
        return Ok(());
    }

    println!("{}", "📦 Dependency Graph".bright_blue().bold());
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━".bright_blue());
    println!();

    // Find root packages (those with no dependents)
    let dependents = build_dependents_map(&dag);
    let mut roots: Vec<&String> = dag
        .nodes
        .keys()
        .filter(|id| !dependents.contains_key(*id) || dependents[*id].is_empty())
        .collect();
    roots.sort();

    // If no roots found (circular deps), just show all packages
    if roots.is_empty() {
        roots = dag.nodes.keys().collect();
        roots.sort();
    }

    for root in roots {
        print_tree(&dag, root, "", true, &mut HashSet::new());
    }

    println!();
    println!(
        "{}",
        format!("Total: {} packages", dag.nodes.len()).dimmed()
    );

    Ok(())
}

/// Output dependency graph as JSON
pub fn json() -> Result<()> {
    let dag = load_dag()?;
    let dependents = build_dependents_map(&dag);

    let mut packages: Vec<PackageInfo> = dag
        .nodes
        .values()
        .map(|node| {
            let pkg_type = if node.path.starts_with("apps/") {
                "app"
            } else if node.path.starts_with("libs/") {
                "lib"
            } else if node.path.starts_with("packages/") {
                "package"
            } else {
                "unknown"
            };

            PackageInfo {
                id: node.id.clone(),
                path: node.path.clone(),
                pkg_type: pkg_type.to_string(),
                deps_count: node.deps.len(),
                dependents_count: dependents.get(&node.id).map(|d| d.len()).unwrap_or(0),
            }
        })
        .collect();
    packages.sort_by(|a, b| a.id.cmp(&b.id));

    let mut edges: Vec<Edge> = Vec::new();
    for node in dag.nodes.values() {
        for dep in &node.deps {
            edges.push(Edge {
                from: node.id.clone(),
                to: dep.clone(),
            });
        }
    }
    edges.sort_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));

    let cycles = detect_cycles(&dag);

    let output = DepsJson {
        format: "airis.deps.v1",
        packages,
        edges,
        cycles,
    };

    let json = serde_json::to_string_pretty(&output)?;
    println!("{}", json);

    Ok(())
}

/// Show dependencies for a specific package
pub fn show(pkg: &str) -> Result<()> {
    let dag = load_dag()?;

    // Find the package by path or partial match
    let node = find_package(&dag, pkg)?;
    let dependents = build_dependents_map(&dag);

    println!("{}", format!("📦 {}", node.id).bright_blue().bold());
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━".bright_blue());
    println!();

    // Dependencies (what this package depends on)
    println!("{}", "Dependencies:".green());
    if node.deps.is_empty() {
        println!("  {}", "(none)".dimmed());
    } else {
        let mut deps: Vec<_> = node.deps.iter().collect();
        deps.sort();
        for dep in deps {
            println!("  └── {}", dep);
        }
    }
    println!();

    // Dependents (what depends on this package)
    println!("{}", "Dependents (packages that depend on this):".yellow());
    if let Some(deps) = dependents.get(&node.id) {
        if deps.is_empty() {
            println!("  {}", "(none)".dimmed());
        } else {
            let mut deps: Vec<_> = deps.iter().collect();
            deps.sort();
            for dep in deps {
                println!("  └── {}", dep);
            }
        }
    } else {
        println!("  {}", "(none)".dimmed());
    }
    println!();

    // Build order
    println!("{}", "Build order (dependencies first):".cyan());
    match dag.topo_order(&node.id) {
        Ok(order) => {
            for (i, n) in order.iter().enumerate() {
                let marker = if n.id == node.id { "→" } else { " " };
                println!("  {} {}. {}", marker, i + 1, n.id);
            }
        }
        Err(e) => {
            println!("  {} {}", "⚠️".yellow(), e);
        }
    }

    Ok(())
}

/// Check for circular dependencies
pub fn check() -> Result<()> {
    let dag = load_dag()?;

    println!(
        "{}",
        "🔍 Checking for circular dependencies...".bright_blue()
    );
    println!();

    let cycles = detect_cycles(&dag);

    if cycles.is_empty() {
        println!("{}", "✅ No circular dependencies detected".green());

        // Additional architecture checks
        println!();
        println!("{}", "📋 Architecture validation:".bright_blue());

        let violations = check_architecture(&dag);
        if violations.is_empty() {
            println!("  {} Apps only depend on libs", "✓".green());
            println!("  {} No cross-app dependencies", "✓".green());
        } else {
            for violation in &violations {
                println!("  {} {}", "✗".red(), violation);
            }
            anyhow::bail!("{} architecture violation(s) found", violations.len());
        }

        Ok(())
    } else {
        println!("{}", "⚠️  Circular dependencies detected:".red().bold());
        println!();

        for (i, cycle) in cycles.iter().enumerate() {
            println!(
                "  {}. {} → {}",
                i + 1,
                cycle.join(" → "),
                cycle.first().unwrap_or(&String::new())
            );
        }

        anyhow::bail!("{} circular dependency cycle(s) found", cycles.len());
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Load DAG from pnpm-lock.yaml
fn load_dag() -> Result<Dag> {
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
fn build_dependents_map(dag: &Dag) -> HashMap<String, Vec<String>> {
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

/// Print a single node in the tree
fn print_tree(
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

/// Find a package by ID, path, or partial match
fn find_package<'a>(dag: &'a Dag, query: &str) -> Result<&'a DagNode> {
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

/// Detect cycles in the dependency graph
fn detect_cycles(dag: &Dag) -> Vec<Vec<String>> {
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
fn check_architecture(dag: &Dag) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_dependents_map() {
        let mut dag = Dag::new();

        dag.add_node(DagNode {
            id: "apps/web".to_string(),
            name: "web".to_string(),
            path: "apps/web".to_string(),
            deps: vec!["libs/ui".to_string()],
        });

        dag.add_node(DagNode {
            id: "libs/ui".to_string(),
            name: "ui".to_string(),
            path: "libs/ui".to_string(),
            deps: vec![],
        });

        let dependents = build_dependents_map(&dag);

        assert!(
            dependents
                .get("libs/ui")
                .unwrap()
                .contains(&"apps/web".to_string())
        );
        assert!(dependents.get("apps/web").unwrap().is_empty());
    }

    #[test]
    fn test_detect_cycles_no_cycle() {
        let mut dag = Dag::new();

        dag.add_node(DagNode {
            id: "a".to_string(),
            name: "a".to_string(),
            path: "apps/a".to_string(),
            deps: vec!["b".to_string()],
        });

        dag.add_node(DagNode {
            id: "b".to_string(),
            name: "b".to_string(),
            path: "libs/b".to_string(),
            deps: vec![],
        });

        let cycles = detect_cycles(&dag);
        assert!(cycles.is_empty());
    }

    #[test]
    fn test_detect_cycles_with_cycle() {
        let mut dag = Dag::new();

        dag.add_node(DagNode {
            id: "a".to_string(),
            name: "a".to_string(),
            path: "libs/a".to_string(),
            deps: vec!["b".to_string()],
        });

        dag.add_node(DagNode {
            id: "b".to_string(),
            name: "b".to_string(),
            path: "libs/b".to_string(),
            deps: vec!["a".to_string()],
        });

        let cycles = detect_cycles(&dag);
        assert!(!cycles.is_empty());
    }

    #[test]
    fn test_check_architecture_violation() {
        let mut dag = Dag::new();

        dag.add_node(DagNode {
            id: "apps/web".to_string(),
            name: "web".to_string(),
            path: "apps/web".to_string(),
            deps: vec!["apps/api".to_string()],
        });

        dag.add_node(DagNode {
            id: "apps/api".to_string(),
            name: "api".to_string(),
            path: "apps/api".to_string(),
            deps: vec![],
        });

        let violations = check_architecture(&dag);
        assert!(!violations.is_empty());
        assert!(violations[0].contains("Cross-app dependency"));
    }

    #[test]
    fn test_check_architecture_valid() {
        let mut dag = Dag::new();

        dag.add_node(DagNode {
            id: "apps/web".to_string(),
            name: "web".to_string(),
            path: "apps/web".to_string(),
            deps: vec!["libs/ui".to_string()],
        });

        dag.add_node(DagNode {
            id: "libs/ui".to_string(),
            name: "ui".to_string(),
            path: "libs/ui".to_string(),
            deps: vec![],
        });

        let violations = check_architecture(&dag);
        assert!(violations.is_empty());
    }
}
