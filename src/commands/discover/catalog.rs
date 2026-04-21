//! Catalog extraction from package.json and pnpm-workspace.yaml.

use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde_json::Value;
use std::fs;
use std::path::Path;

use super::types::PackageInfo;

/// Extract catalog entries from root package.json
pub fn extract_catalog() -> Result<IndexMap<String, String>> {
    extract_catalog_from_path(Path::new("."))
}

/// Extract catalog entries from package.json in the given directory
pub fn extract_catalog_from_path(base_path: &Path) -> Result<IndexMap<String, String>> {
    let mut catalog = IndexMap::new();

    let pkg_json_path = base_path.join("package.json");
    if !pkg_json_path.exists() {
        return Ok(catalog);
    }

    let content = fs::read_to_string(&pkg_json_path).context("Failed to read package.json")?;
    let json: Value = serde_json::from_str(&content).context("Failed to parse package.json")?;

    // Extract from devDependencies (common location for shared tooling)
    if let Some(dev_deps) = json["devDependencies"].as_object() {
        // Common catalog packages
        let catalog_packages = [
            "typescript",
            "eslint",
            "prettier",
            "@types/node",
            "tsup",
            "vitest",
            "jest",
            "@typescript-eslint/eslint-plugin",
            "@typescript-eslint/parser",
        ];

        for pkg in catalog_packages {
            if let Some(version) = dev_deps.get(pkg).and_then(|v| v.as_str()) {
                // Skip workspace: references
                if !version.starts_with("workspace:") {
                    catalog.insert(pkg.to_string(), version.to_string());
                }
            }
        }
    }

    // Also check pnpm-workspace.yaml for existing catalog
    let pnpm_workspace_path = base_path.join("pnpm-workspace.yaml");
    if pnpm_workspace_path.exists()
        && let Ok(content) = fs::read_to_string(&pnpm_workspace_path)
    {
        #[derive(serde::Deserialize)]
        struct PnpmWorkspace {
            catalog: Option<IndexMap<String, String>>,
        }

        if let Ok(workspace) = serde_yml::from_str::<PnpmWorkspace>(&content)
            && let Some(existing_catalog) = workspace.catalog
        {
            for (pkg, version) in existing_catalog {
                catalog.insert(pkg, version);
            }
        }
    }

    Ok(catalog)
}

/// Extract scripts, dependencies, and devDependencies from package.json
/// Converts catalog-matching packages to "catalog:" references
pub fn extract_package_info(dir: &Path, catalog: &IndexMap<String, String>) -> PackageInfo {
    let pkg_json_path = dir.join("package.json");
    if !pkg_json_path.exists() {
        return PackageInfo::default();
    }

    let content = match fs::read_to_string(&pkg_json_path) {
        Ok(c) => c,
        Err(_) => return PackageInfo::default(),
    };

    let json: Value = match serde_json::from_str(&content) {
        Ok(j) => j,
        Err(_) => return PackageInfo::default(),
    };

    let mut info = PackageInfo::default();

    // Extract scripts
    if let Some(scripts) = json["scripts"].as_object() {
        for (key, value) in scripts {
            if let Some(v) = value.as_str() {
                info.scripts.insert(key.clone(), v.to_string());
            }
        }
    }

    // Extract dependencies, converting catalog matches to "catalog:"
    if let Some(deps) = json["dependencies"].as_object() {
        for (name, version) in deps {
            if let Some(v) = version.as_str() {
                // Skip workspace: references
                if v.starts_with("workspace:") {
                    continue;
                }
                // Convert to catalog: if package exists in catalog
                let resolved = if catalog.contains_key(name) {
                    "catalog:".to_string()
                } else {
                    v.to_string()
                };
                info.deps.insert(name.clone(), resolved);
            }
        }
    }

    // Extract devDependencies, converting catalog matches to "catalog:"
    if let Some(dev_deps) = json["devDependencies"].as_object() {
        for (name, version) in dev_deps {
            if let Some(v) = version.as_str() {
                // Skip workspace: references
                if v.starts_with("workspace:") {
                    continue;
                }
                // Convert to catalog: if package exists in catalog
                let resolved = if catalog.contains_key(name) {
                    "catalog:".to_string()
                } else {
                    v.to_string()
                };
                info.dev_deps.insert(name.clone(), resolved);
            }
        }
    }

    info
}
