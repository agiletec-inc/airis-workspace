use anyhow::{Context, Result};
use colored::Colorize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Analyze affected packages based on git changes
pub fn run(base: &str, head: &str) -> Result<Vec<String>> {
    println!("{}", "🔍 Analyzing affected packages...".bright_blue());

    // 1. Get changed files from git
    let changed_files = get_changed_files(base, head)?;
    if changed_files.is_empty() {
        println!("{}", "✅ No changes detected".green());
        return Ok(vec![]);
    }

    println!("  📝 Changed files: {}", changed_files.len());

    // 2. Build dependency graph
    let graph = build_dependency_graph()?;
    println!("  📦 Packages found: {}", graph.len());

    // 3. Find directly changed packages
    let mut affected: HashSet<String> = HashSet::new();

    for file in &changed_files {
        if let Some(pkg) = get_package_from_path(file) {
            affected.insert(pkg);
        }
    }

    println!("  🎯 Directly changed: {}", affected.len());

    // 4. Find packages that depend on changed packages (transitive)
    let initial_affected: Vec<String> = affected.iter().cloned().collect();
    for pkg in initial_affected {
        find_dependents(&pkg, &graph, &mut affected);
    }

    let mut result: Vec<String> = affected.into_iter().collect();
    result.sort();

    println!();
    println!("{}", "📊 Affected packages:".green());
    for pkg in &result {
        println!("   - {}", pkg);
    }

    Ok(result)
}

/// Get list of changed files from git
fn get_changed_files(base: &str, head: &str) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["diff", "--name-only", &format!("{}...{}", base, head)])
        .output()
        .context("Failed to run git diff")?;

    if !output.status.success() {
        // Try without the range (for uncommitted changes)
        let output = Command::new("git")
            .args(["diff", "--name-only", "HEAD"])
            .output()
            .context("Failed to run git diff HEAD")?;

        let files: Vec<String> = String::from_utf8(output.stdout)
            .context("Invalid UTF-8 from git")?
            .lines()
            .map(|s| s.to_string())
            .collect();

        return Ok(files);
    }

    let files: Vec<String> = String::from_utf8(output.stdout)
        .context("Invalid UTF-8 from git")?
        .lines()
        .map(|s| s.to_string())
        .collect();

    Ok(files)
}

/// Build dependency graph from package.json files
fn build_dependency_graph() -> Result<HashMap<String, Vec<String>>> {
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();

    // Scan apps/ and libs/ directories
    for dir in &["apps", "libs", "packages"] {
        let dir_path = Path::new(dir);
        if !dir_path.exists() {
            continue;
        }

        if let Ok(entries) = fs::read_dir(dir_path) {
            for entry in entries.flatten() {
                let pkg_json = entry.path().join("package.json");
                if pkg_json.exists() {
                    if let Ok((name, deps)) = parse_package_json(&pkg_json) {
                        graph.insert(name, deps);
                    }
                }
            }
        }
    }

    // Also check nested libs (e.g., libs/supabase/*)
    let nested_libs = Path::new("libs/supabase");
    if nested_libs.exists() {
        if let Ok(entries) = fs::read_dir(nested_libs) {
            for entry in entries.flatten() {
                let pkg_json = entry.path().join("package.json");
                if pkg_json.exists() {
                    if let Ok((name, deps)) = parse_package_json(&pkg_json) {
                        graph.insert(name, deps);
                    }
                }
            }
        }
    }

    Ok(graph)
}

/// Parse package.json and extract name + dependencies
fn parse_package_json(path: &Path) -> Result<(String, Vec<String>)> {
    let content = fs::read_to_string(path)?;
    let json: Value = serde_json::from_str(&content)?;

    let name = json["name"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    let mut deps = Vec::new();

    // Collect dependencies
    if let Some(dependencies) = json["dependencies"].as_object() {
        for dep_name in dependencies.keys() {
            // Only include workspace packages (starting with @ or workspace:)
            if dep_name.starts_with('@') || dependencies[dep_name].as_str().map(|v| v.contains("workspace:")).unwrap_or(false) {
                deps.push(dep_name.clone());
            }
        }
    }

    // Collect devDependencies
    if let Some(dev_dependencies) = json["devDependencies"].as_object() {
        for dep_name in dev_dependencies.keys() {
            if dep_name.starts_with('@') || dev_dependencies[dep_name].as_str().map(|v| v.contains("workspace:")).unwrap_or(false) {
                deps.push(dep_name.clone());
            }
        }
    }

    Ok((name, deps))
}

/// Extract package name from file path
fn get_package_from_path(file_path: &str) -> Option<String> {
    let parts: Vec<&str> = file_path.split('/').collect();

    if parts.len() >= 2 {
        match parts[0] {
            "apps" | "packages" => {
                // apps/dashboard/... → @workspace/dashboard or check package.json
                let pkg_name = parts[1];
                // Try to read the actual package name from package.json
                let pkg_json = format!("{}/{}/package.json", parts[0], pkg_name);
                if let Ok(content) = fs::read_to_string(&pkg_json) {
                    if let Ok(json) = serde_json::from_str::<Value>(&content) {
                        if let Some(name) = json["name"].as_str() {
                            return Some(name.to_string());
                        }
                    }
                }
                Some(format!("@workspace/{}", pkg_name))
            }
            "libs" => {
                if parts.len() >= 3 && parts[1] == "supabase" {
                    // libs/supabase/client/... → check package.json
                    let pkg_json = format!("libs/supabase/{}/package.json", parts[2]);
                    if let Ok(content) = fs::read_to_string(&pkg_json) {
                        if let Ok(json) = serde_json::from_str::<Value>(&content) {
                            if let Some(name) = json["name"].as_str() {
                                return Some(name.to_string());
                            }
                        }
                    }
                    Some(format!("@workspace/{}", parts[2]))
                } else {
                    // libs/ui/... → @workspace/ui
                    let pkg_name = parts[1];
                    let pkg_json = format!("libs/{}/package.json", pkg_name);
                    if let Ok(content) = fs::read_to_string(&pkg_json) {
                        if let Ok(json) = serde_json::from_str::<Value>(&content) {
                            if let Some(name) = json["name"].as_str() {
                                return Some(name.to_string());
                            }
                        }
                    }
                    Some(format!("@workspace/{}", pkg_name))
                }
            }
            _ => None,
        }
    } else {
        None
    }
}

/// Find all packages that depend on the given package (recursively)
fn find_dependents(pkg: &str, graph: &HashMap<String, Vec<String>>, affected: &mut HashSet<String>) {
    for (name, deps) in graph {
        if deps.contains(&pkg.to_string()) && !affected.contains(name) {
            affected.insert(name.clone());
            // Recursively find packages that depend on this one
            find_dependents(name, graph, affected);
        }
    }
}

/// List all packages in the workspace
#[allow(dead_code)]
pub fn list_packages() -> Result<Vec<String>> {
    let graph = build_dependency_graph()?;
    let mut packages: Vec<String> = graph.keys().cloned().collect();
    packages.sort();
    Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_package_from_path() {
        assert_eq!(
            get_package_from_path("apps/dashboard/src/index.ts"),
            Some("@workspace/dashboard".to_string())
        );
        assert_eq!(
            get_package_from_path("libs/ui/components/Button.tsx"),
            Some("@workspace/ui".to_string())
        );
        assert_eq!(get_package_from_path("README.md"), None);
    }
}
