use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde_json;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::manifest::{CatalogEntry, Manifest};

pub fn run() -> Result<()> {
    println!("⚠️  DEPRECATED: 'airis sync-deps' is deprecated.");
    println!("   Use 'airis init' instead - it now resolves catalog versions automatically.");
    println!();
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
    if let Some(entry) = catalog.get(package)
        && let Some(target) = entry.follow_target() {
            if !catalog.contains_key(target) {
                anyhow::bail!(
                    "Cannot resolve '{}': follow target '{}' not found in [packages.catalog]",
                    package,
                    target
                );
            }
            visit_package(target, catalog, visited, visiting, order)?;
        }

    visiting.remove(package);
    visited.insert(package.to_string());
    order.push(package.to_string());

    Ok(())
}

/// Resolve a version policy to an actual version number
pub fn resolve_version(package: &str, policy: &str) -> Result<String> {
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

pub fn get_npm_latest(package: &str) -> Result<String> {
    let output = Command::new("npm")
        .args(["view", package, "version"])
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

pub fn get_npm_lts(package: &str) -> Result<String> {
    // Try to find LTS version from dist-tags
    let output = Command::new("npm")
        .args(["view", package, "dist-tags", "--json"])
        .output()
        .context(format!("Failed to query npm dist-tags for {}", package))?;

    if !output.status.success() {
        // Fallback to latest if dist-tags query fails
        return get_npm_latest(package);
    }

    let json_str = String::from_utf8(output.stdout)
        .context("Invalid UTF-8 from npm")?;

    let tags: serde_json::Value = serde_json::from_str(&json_str)
        .unwrap_or(serde_json::Value::Null);

    // Priority: "lts" tag > "*-lts" pattern (highest version) > "latest"
    if let Some(lts) = tags.get("lts").and_then(|v| v.as_str()) {
        return Ok(format!("^{}", lts));
    }

    // Find *-lts tags (e.g., v20-lts, v18-lts for Node.js)
    if let Some(obj) = tags.as_object() {
        let mut lts_versions: Vec<(&str, &str)> = obj
            .iter()
            .filter(|(k, _)| k.ends_with("-lts"))
            .filter_map(|(k, v)| v.as_str().map(|ver| (k.as_str(), ver)))
            .collect();

        // Sort by tag name to get highest LTS (e.g., v20-lts > v18-lts)
        lts_versions.sort_by(|a, b| b.0.cmp(a.0));

        if let Some((_, version)) = lts_versions.first() {
            return Ok(format!("^{}", version));
        }
    }

    // Fallback to latest
    get_npm_latest(package)
}

fn update_pnpm_workspace(catalog: &IndexMap<String, String>) -> Result<()> {
    let workspace_path = Path::new("pnpm-workspace.yaml");

    // Load manifest to get workspace packages
    let manifest = Manifest::load(Path::new("manifest.toml"))
        .context("Failed to load manifest.toml")?;

    let yaml = if workspace_path.exists() {
        // Read existing content
        let content = fs::read_to_string(workspace_path)
            .context("Failed to read pnpm-workspace.yaml")?;

        // Parse YAML
        serde_yaml::from_str(&content)
            .context("Failed to parse pnpm-workspace.yaml")?
    } else {
        // Create new YAML structure from manifest
        let mut root_map = serde_yaml::Mapping::new();

        // Add packages from manifest
        let packages: Vec<serde_yaml::Value> = manifest
            .packages
            .workspaces
            .iter()
            .map(|ws| serde_yaml::Value::String(ws.clone()))
            .collect();

        root_map.insert(
            serde_yaml::Value::String("packages".to_string()),
            serde_yaml::Value::Sequence(packages),
        );

        serde_yaml::Value::Mapping(root_map)
    };

    let mut yaml = yaml;

    // COMPLETELY REPLACE catalog section (don't merge - this removes deleted entries)
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

    // Write back to file
    let updated_content = serde_yaml::to_string(&yaml)
        .context("Failed to serialize YAML")?;

    fs::write(workspace_path, updated_content)
        .context("Failed to write pnpm-workspace.yaml")?;

    println!("📝 Updated pnpm-workspace.yaml");

    Ok(())
}

/// Migrate packages to use pnpm catalog references
pub fn run_migrate() -> Result<()> {
    use colored::Colorize;
    use glob::glob;
    use serde_json::Value;

    println!("{}", "🔄 Migrating packages to use catalog references...".bright_blue());
    println!();

    // Load manifest to get catalog
    let manifest = Manifest::load(Path::new("manifest.toml"))
        .context("Failed to load manifest.toml")?;

    let catalog = &manifest.packages.catalog;

    if catalog.is_empty() {
        println!("{}", "⚠️  No catalog entries found in manifest.toml".yellow());
        return Ok(());
    }

    // Find all package.json files
    let pattern = "{apps,libs}/*/package.json";
    let mut migrated_count = 0;
    let mut package_count = 0;

    for entry in glob(pattern).context("Failed to read glob pattern")? {
        let path = entry.context("Failed to read path")?;
        package_count += 1;

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let mut pkg: Value = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        let mut changed = false;

        // Process dependencies
        for dep_type in &["dependencies", "devDependencies", "peerDependencies"] {
            if let Some(deps) = pkg.get_mut(*dep_type)
                && let Some(deps_obj) = deps.as_object_mut() {
                    for (name, version) in deps_obj.iter_mut() {
                        // Check if this package is in catalog
                        if catalog.contains_key(name) {
                            let current = version.as_str().unwrap_or("");
                            if current != "catalog:" {
                                *version = Value::String("catalog:".to_string());
                                changed = true;
                                println!(
                                    "  {} {} {} → catalog:",
                                    "✓".green(),
                                    path.display(),
                                    name.cyan()
                                );
                            }
                        }
                    }
                }
        }

        if changed {
            // Write back with pretty formatting
            let formatted = serde_json::to_string_pretty(&pkg)
                .context("Failed to serialize JSON")?;

            fs::write(&path, formatted + "\n")
                .with_context(|| format!("Failed to write {}", path.display()))?;

            migrated_count += 1;
        }
    }

    println!();
    if migrated_count > 0 {
        println!(
            "{}",
            format!("✅ Migrated {} package(s) to use catalog references", migrated_count).green()
        );
        println!();
        println!("{}", "Next steps:".bright_yellow());
        println!("  1. Run 'airis install' to update lockfile");
        println!("  2. Commit the changes");
    } else {
        println!(
            "{}",
            format!("✅ All {} package(s) already using catalog references (or no catalog matches)", package_count).green()
        );
    }

    Ok(())
}
