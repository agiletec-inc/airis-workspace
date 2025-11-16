use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde_json::json;
use std::fs;
use std::path::Path;

use crate::manifest::ProjectDefinition;

/// Generate package.json for a project from manifest definition
pub fn generate_project_package_json(
    project: &ProjectDefinition,
    workspace_root: &Path,
    resolved_catalog: &IndexMap<String, String>,
) -> Result<()> {
    let project_path = project
        .path
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Project '{}' has no path defined", project.name))?;

    let package_json_path = workspace_root.join(project_path).join("package.json");

    // Build dependencies with resolved catalog versions
    let dependencies = resolve_deps_from_catalog(&project.deps, resolved_catalog);
    let dev_dependencies = resolve_deps_from_catalog(&project.dev_deps, resolved_catalog);

    // Generate package name (e.g., @workspace/project-name)
    let package_name = format!("@workspace/{}", project.name);

    let package_json = json!({
        "name": package_name,
        "version": "0.1.0",
        "private": true,
        "type": "module",
        "scripts": project.scripts,
        "dependencies": dependencies,
        "devDependencies": dev_dependencies,
        "_generated": {
            "by": "airis init",
            "from": "manifest.toml",
            "warning": "⚠️  DO NOT EDIT - Update manifest.toml then rerun `airis init`"
        }
    });

    // Ensure directory exists
    if let Some(parent) = package_json_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {:?}", parent))?;
    }

    // Write package.json
    let content = serde_json::to_string_pretty(&package_json)
        .context("Failed to serialize package.json")?;

    fs::write(&package_json_path, content)
        .with_context(|| format!("Failed to write {:?}", package_json_path))?;

    println!("  ✓ Generated {}", package_json_path.display());

    Ok(())
}

/// Resolve dependencies from catalog
/// "react" = "catalog" -> "react": "^19.2.0" (from resolved_catalog)
/// "vite" = "^5.0.0" -> "vite": "^5.0.0" (unchanged)
fn resolve_deps_from_catalog(
    deps: &IndexMap<String, String>,
    resolved_catalog: &IndexMap<String, String>,
) -> IndexMap<String, String> {
    deps.iter()
        .map(|(name, version)| {
            let resolved_version = if version == "catalog" {
                // Look up in resolved catalog
                resolved_catalog
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| version.clone())
            } else {
                version.clone()
            };
            (name.clone(), resolved_version)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_deps_from_catalog() {
        let mut deps = IndexMap::new();
        deps.insert("react".to_string(), "catalog".to_string());
        deps.insert("vite".to_string(), "^5.0.0".to_string());

        let mut catalog = IndexMap::new();
        catalog.insert("react".to_string(), "^19.2.0".to_string());

        let result = resolve_deps_from_catalog(&deps, &catalog);

        assert_eq!(result.get("react").unwrap(), "^19.2.0");
        assert_eq!(result.get("vite").unwrap(), "^5.0.0");
    }
}
