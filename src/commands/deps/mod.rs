//! Dependency graph visualization and analysis
//!
//! Provides commands to visualize and analyze the workspace dependency graph.

mod analysis;
mod display;
mod graph;

#[cfg(test)]
mod tests;

use anyhow::Result;
use colored::Colorize;
use serde::Serialize;
use std::collections::HashSet;

use analysis::{check_architecture, detect_cycles};
use display::print_tree;
use graph::{build_dependents_map, find_package, load_dag};

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
