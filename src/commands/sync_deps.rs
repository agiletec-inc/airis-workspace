use anyhow::{Context, Result};
use indexmap::IndexMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::manifest::{CatalogEntry, Manifest};

pub fn run() -> Result<()> {
    println!("🔄 Syncing dependencies from manifest.toml...");

    // Load manifest
    let manifest = Manifest::load(Path::new("manifest.toml"))
        .context("Failed to load manifest.toml")?;

    // Get catalog from manifest
    let catalog = &manifest.packages.catalog;

    if catalog.is_empty() {
        println!("⚠️  No catalog entries found in manifest.toml");
        return Ok(());
    }

    println!("📦 Found {} catalog entries", catalog.len());

    // Detect circular dependencies and compute resolution order
    let resolution_order = compute_resolution_order(catalog)?;

    // Resolve versions in dependency order
    let mut resolved_catalog: IndexMap<String, String> = IndexMap::new();

    for package in resolution_order {
        let entry = catalog.get(&package).unwrap();

        let version = match entry {
            CatalogEntry::Follow(follow_config) => {
                // Resolve by following another package
                let target = &follow_config.follow;

                if let Some(target_version) = resolved_catalog.get(target) {
                    println!("  {} (follow {}) → {}", package, target, target_version);
                    target_version.clone()
                } else {
                    anyhow::bail!(
                        "Internal error: follow target '{}' for '{}' should have been resolved earlier. \
                         This is a bug in airis sync-deps. Please report this issue.",
                        target,
                        package
                    );
                }
            }
            CatalogEntry::Policy(policy) => {
                let policy_str = policy.as_str();
                let version = resolve_version(&package, policy_str)?;
                println!("  {} {} → {}", package, policy_str, version);
                version
            }
            CatalogEntry::Version(version) => {
                println!("  {} {}", package, version);
                version.clone()
            }
        };

        resolved_catalog.insert(package, version);
    }

    // Update pnpm-workspace.yaml
    update_pnpm_workspace(&resolved_catalog)?;

    println!("✅ Dependency sync complete!");
    println!("   Run 'pnpm install' to apply changes");

    Ok(())
}

/// Compute resolution order with circular dependency detection
fn compute_resolution_order(catalog: &IndexMap<String, CatalogEntry>) -> Result<Vec<String>> {
    let mut order = Vec::new();
    let mut visited = HashSet::new();
    let mut visiting = HashSet::new();

    // Topological sort with cycle detection
    for package in catalog.keys() {
        if !visited.contains(package) {
            visit_package(
                package,
                catalog,
                &mut visited,
                &mut visiting,
                &mut order,
            )?;
        }
    }

    Ok(order)
}

fn visit_package(
    package: &str,
    catalog: &IndexMap<String, CatalogEntry>,
    visited: &mut HashSet<String>,
    visiting: &mut HashSet<String>,
    order: &mut Vec<String>,
) -> Result<()> {
    if visiting.contains(package) {
        anyhow::bail!("Circular dependency detected involving '{}'", package);
    }

    if visited.contains(package) {
        return Ok(());
    }

    visiting.insert(package.to_string());

    // If this package follows another, visit that first
    if let Some(entry) = catalog.get(package) {
        if let Some(target) = entry.follow_target() {
            if !catalog.contains_key(target) {
                anyhow::bail!(
                    "Cannot resolve '{}': follow target '{}' not found in [packages.catalog]",
                    package,
                    target
                );
            }
            visit_package(target, catalog, visited, visiting, order)?;
        }
    }

    visiting.remove(package);
    visited.insert(package.to_string());
    order.push(package.to_string());

    Ok(())
}

fn resolve_version(package: &str, policy: &str) -> Result<String> {
    match policy {
        "latest" => get_npm_latest(package),
        "lts" => get_npm_lts(package),
        version if version.starts_with('^') || version.starts_with('~') => {
            // Already a specific version
            Ok(version.to_string())
        }
        _ => {
            // Treat as specific version
            Ok(policy.to_string())
        }
    }
}

fn get_npm_latest(package: &str) -> Result<String> {
    let output = Command::new("npm")
        .args(&["view", package, "version"])
        .output()
        .context(format!("Failed to query npm for {}", package))?;

    if !output.status.success() {
        anyhow::bail!("npm view failed for {}", package);
    }

    let version = String::from_utf8(output.stdout)
        .context("Invalid UTF-8 from npm")?
        .trim()
        .to_string();

    Ok(format!("^{}", version))
}

fn get_npm_lts(package: &str) -> Result<String> {
    // For LTS, we use the "dist-tags.latest" approach
    // In the future, could check for actual LTS tags
    get_npm_latest(package)
}

fn update_pnpm_workspace(catalog: &IndexMap<String, String>) -> Result<()> {
    let workspace_path = Path::new("pnpm-workspace.yaml");

    if !workspace_path.exists() {
        anyhow::bail!("pnpm-workspace.yaml not found");
    }

    // Read existing content
    let content = fs::read_to_string(workspace_path)
        .context("Failed to read pnpm-workspace.yaml")?;

    // Parse YAML
    let mut yaml: serde_yaml::Value = serde_yaml::from_str(&content)
        .context("Failed to parse pnpm-workspace.yaml")?;

    // Update catalog section
    if let Some(catalog_section) = yaml.get_mut("catalog") {
        if let Some(catalog_map) = catalog_section.as_mapping_mut() {
            for (package, version) in catalog {
                let key = serde_yaml::Value::String(package.clone());
                let value = serde_yaml::Value::String(version.clone());
                catalog_map.insert(key, value);
            }
        }
    } else {
        // Create catalog section if it doesn't exist
        let mut catalog_map = serde_yaml::Mapping::new();
        for (package, version) in catalog {
            let key = serde_yaml::Value::String(package.clone());
            let value = serde_yaml::Value::String(version.clone());
            catalog_map.insert(key, value);
        }
        if let Some(root_map) = yaml.as_mapping_mut() {
            root_map.insert(
                serde_yaml::Value::String("catalog".to_string()),
                serde_yaml::Value::Mapping(catalog_map),
            );
        }
    }

    // Write back to file
    let updated_content = serde_yaml::to_string(&yaml)
        .context("Failed to serialize YAML")?;

    fs::write(workspace_path, updated_content)
        .context("Failed to write pnpm-workspace.yaml")?;

    println!("📝 Updated pnpm-workspace.yaml");

    Ok(())
}
