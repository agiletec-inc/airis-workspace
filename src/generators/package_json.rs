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

    // Generate package name: use explicit scope if provided, otherwise @workspace
    let package_name = if let Some(scope) = &project.scope {
        let scope = scope.trim_start_matches('@');
        format!("@{}/{}", scope, project.name)
    } else {
        format!("@workspace/{}", project.name)
    };

    let mut package_json = json!({
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

    // Add optional fields if present
    if let Some(description) = &project.description {
        package_json["description"] = json!(description);
    }
    if let Some(main) = &project.main {
        package_json["main"] = json!(main);
    }
    if !project.bin.is_empty() {
        package_json["bin"] = json!(project.bin);
    }

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
            let resolved_version = if version == "catalog" || version == "catalog:" {
                // Look up in resolved catalog (supports both "catalog" and "catalog:" pnpm syntax)
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

    #[test]
    fn test_resolve_deps_catalog_colon_syntax() {
        let mut deps = IndexMap::new();
        deps.insert("zod".to_string(), "catalog:".to_string());
        deps.insert("commander".to_string(), "^13.1.0".to_string());

        let mut catalog = IndexMap::new();
        catalog.insert("zod".to_string(), "^3.23.0".to_string());

        let result = resolve_deps_from_catalog(&deps, &catalog);

        assert_eq!(result.get("zod").unwrap(), "^3.23.0");
        assert_eq!(result.get("commander").unwrap(), "^13.1.0");
    }

    #[test]
    fn test_generate_project_package_json_with_scope_and_bin() {
        let tmp_dir = std::env::temp_dir().join("airis_test_pkg_json");
        let project_dir = tmp_dir.join("products/test-app");
        std::fs::create_dir_all(&project_dir).unwrap();

        let mut bin = IndexMap::new();
        bin.insert("akm".to_string(), "dist/cli.js".to_string());

        let project = ProjectDefinition {
            name: "test-app".to_string(),
            kind: Some("app".to_string()),
            path: Some("products/test-app".to_string()),
            scope: Some("@myorg".to_string()),
            description: Some("A test CLI tool".to_string()),
            bin,
            main: Some("dist/index.js".to_string()),
            framework: Some("node".to_string()),
            runner: None,
            scripts: IndexMap::new(),
            deps: IndexMap::new(),
            dev_deps: IndexMap::new(),
            port: None,
            replicas: None,
            resources: None,
        };

        let catalog = IndexMap::new();
        generate_project_package_json(&project, &tmp_dir, &catalog).unwrap();

        let content = std::fs::read_to_string(project_dir.join("package.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(json["name"], "@myorg/test-app");
        assert_eq!(json["description"], "A test CLI tool");
        assert_eq!(json["main"], "dist/index.js");
        assert_eq!(json["bin"]["akm"], "dist/cli.js");

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_generate_project_package_json_default_scope() {
        let tmp_dir = std::env::temp_dir().join("airis_test_pkg_json_default");
        let project_dir = tmp_dir.join("libs/my-lib");
        std::fs::create_dir_all(&project_dir).unwrap();

        let project = ProjectDefinition {
            name: "my-lib".to_string(),
            kind: Some("lib".to_string()),
            path: Some("libs/my-lib".to_string()),
            scope: None,
            description: None,
            bin: IndexMap::new(),
            main: None,
            framework: None,
            runner: None,
            scripts: IndexMap::new(),
            deps: IndexMap::new(),
            dev_deps: IndexMap::new(),
            port: None,
            replicas: None,
            resources: None,
        };

        let catalog = IndexMap::new();
        generate_project_package_json(&project, &tmp_dir, &catalog).unwrap();

        let content = std::fs::read_to_string(project_dir.join("package.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(json["name"], "@workspace/my-lib");
        assert!(json.get("description").is_none());
        assert!(json.get("main").is_none());
        assert!(json.get("bin").is_none());

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
