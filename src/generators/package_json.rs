use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde_json::json;
use std::fs;
use std::path::Path;

use crate::manifest::ProjectDefinition;

/// Convert toml::Value to serde_json::Value
fn toml_to_json(value: &toml::Value) -> serde_json::Value {
    match value {
        toml::Value::String(s) => json!(s),
        toml::Value::Integer(i) => json!(i),
        toml::Value::Float(f) => json!(f),
        toml::Value::Boolean(b) => json!(b),
        toml::Value::Datetime(d) => json!(d.to_string()),
        toml::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(toml_to_json).collect())
        }
        toml::Value::Table(table) => {
            let mut map = serde_json::Map::new();
            for (k, v) in table {
                map.insert(k.clone(), toml_to_json(v));
            }
            serde_json::Value::Object(map)
        }
    }
}

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
    let peer_dependencies = resolve_deps_from_catalog(&project.peer_deps, resolved_catalog);

    // Generate package name: use explicit scope if provided, otherwise @workspace
    let package_name = if let Some(scope) = &project.scope {
        let scope = scope.trim_start_matches('@');
        format!("@{}/{}", scope, project.name)
    } else {
        format!("@workspace/{}", project.name)
    };

    let version = project.version.as_deref().unwrap_or("0.1.0");
    let private = project.private.unwrap_or(true);
    let module_type = project.module_type.as_deref().unwrap_or("module");

    let mut package_json = json!({
        "name": package_name,
        "version": version,
        "private": private,
        "type": module_type,
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
    if let Some(types) = &project.types {
        package_json["types"] = json!(types);
    }
    if let Some(exports) = &project.exports {
        package_json["exports"] = toml_to_json(exports);
    }
    if !project.bin.is_empty() {
        package_json["bin"] = json!(project.bin);
    }
    if !project.files.is_empty() {
        package_json["files"] = json!(project.files);
    }
    if !project.tags.is_empty() {
        package_json["tags"] = json!(project.tags);
        package_json["turbo"] = json!({ "tags": project.tags });
    }
    if !peer_dependencies.is_empty() {
        package_json["peerDependencies"] = json!(peer_dependencies);
    }
    if let Some(peer_deps_meta) = &project.peer_deps_meta {
        package_json["peerDependenciesMeta"] = toml_to_json(peer_deps_meta);
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

    fn default_project(name: &str, path: &str) -> ProjectDefinition {
        ProjectDefinition {
            name: name.to_string(),
            kind: None,
            path: Some(path.to_string()),
            scope: None,
            description: None,
            bin: IndexMap::new(),
            main: None,
            types: None,
            version: None,
            private: None,
            module_type: None,
            exports: None,
            peer_deps: IndexMap::new(),
            peer_deps_meta: None,
            tags: vec![],
            files: vec![],
            framework: None,
            runner: None,
            scripts: IndexMap::new(),
            deps: IndexMap::new(),
            dev_deps: IndexMap::new(),
            port: None,
            replicas: None,
            resources: None,
            deploy: None,
        }
    }

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

        let mut project = default_project("test-app", "products/test-app");
        project.kind = Some("app".to_string());
        project.scope = Some("@myorg".to_string());
        project.description = Some("A test CLI tool".to_string());
        project.bin = bin;
        project.main = Some("dist/index.js".to_string());
        project.framework = Some("node".to_string());

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

        let project = default_project("my-lib", "libs/my-lib");

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

    #[test]
    fn test_exports_simple() {
        let tmp_dir = std::env::temp_dir().join("airis_test_exports_simple");
        let project_dir = tmp_dir.join("libs/simple");
        std::fs::create_dir_all(&project_dir).unwrap();

        let mut project = default_project("simple", "libs/simple");
        // Single entry point: exports = { "." = "./dist/index.js" }
        let mut table = toml::map::Map::new();
        table.insert(".".to_string(), toml::Value::String("./dist/index.js".to_string()));
        project.exports = Some(toml::Value::Table(table));

        let catalog = IndexMap::new();
        generate_project_package_json(&project, &tmp_dir, &catalog).unwrap();

        let content = std::fs::read_to_string(project_dir.join("package.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(json["exports"]["."], "./dist/index.js");

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_exports_subpath() {
        let tmp_dir = std::env::temp_dir().join("airis_test_exports_subpath");
        let project_dir = tmp_dir.join("libs/pricing");
        std::fs::create_dir_all(&project_dir).unwrap();

        let mut project = default_project("pricing", "libs/pricing");
        // Subpath exports: { "." = { import = "./dist/index.js", types = "./dist/index.d.ts" }, "./utils" = "./dist/utils.js" }
        let mut root_entry = toml::map::Map::new();
        root_entry.insert("import".to_string(), toml::Value::String("./dist/index.js".to_string()));
        root_entry.insert("types".to_string(), toml::Value::String("./dist/index.d.ts".to_string()));

        let mut table = toml::map::Map::new();
        table.insert(".".to_string(), toml::Value::Table(root_entry));
        table.insert("./utils".to_string(), toml::Value::String("./dist/utils.js".to_string()));
        project.exports = Some(toml::Value::Table(table));

        let catalog = IndexMap::new();
        generate_project_package_json(&project, &tmp_dir, &catalog).unwrap();

        let content = std::fs::read_to_string(project_dir.join("package.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(json["exports"]["."]["import"], "./dist/index.js");
        assert_eq!(json["exports"]["."]["types"], "./dist/index.d.ts");
        assert_eq!(json["exports"]["./utils"], "./dist/utils.js");

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_peer_deps_and_tags() {
        let tmp_dir = std::env::temp_dir().join("airis_test_peer_tags");
        let project_dir = tmp_dir.join("libs/ui");
        std::fs::create_dir_all(&project_dir).unwrap();

        let mut project = default_project("ui", "libs/ui");
        project.peer_deps.insert("react".to_string(), "catalog".to_string());
        project.peer_deps.insert("react-dom".to_string(), "catalog".to_string());
        project.tags = vec!["shared".to_string(), "ui".to_string()];

        // peer_deps_meta: { react = { optional = true } }
        let mut react_meta = toml::map::Map::new();
        react_meta.insert("optional".to_string(), toml::Value::Boolean(true));
        let mut meta = toml::map::Map::new();
        meta.insert("react".to_string(), toml::Value::Table(react_meta));
        project.peer_deps_meta = Some(toml::Value::Table(meta));

        let mut catalog = IndexMap::new();
        catalog.insert("react".to_string(), "^19.0.0".to_string());
        catalog.insert("react-dom".to_string(), "^19.0.0".to_string());

        generate_project_package_json(&project, &tmp_dir, &catalog).unwrap();

        let content = std::fs::read_to_string(project_dir.join("package.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(json["peerDependencies"]["react"], "^19.0.0");
        assert_eq!(json["peerDependencies"]["react-dom"], "^19.0.0");
        assert_eq!(json["tags"], json!(["shared", "ui"]));
        assert_eq!(json["turbo"]["tags"], json!(["shared", "ui"]));
        assert_eq!(json["peerDependenciesMeta"]["react"]["optional"], true);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_custom_version_and_type() {
        let tmp_dir = std::env::temp_dir().join("airis_test_custom_ver");
        let project_dir = tmp_dir.join("libs/legacy");
        std::fs::create_dir_all(&project_dir).unwrap();

        let mut project = default_project("legacy", "libs/legacy");
        project.version = Some("2.1.0".to_string());
        project.private = Some(false);
        project.module_type = Some("commonjs".to_string());
        project.types = Some("dist/index.d.ts".to_string());
        project.files = vec!["dist".to_string(), "README.md".to_string()];

        let catalog = IndexMap::new();
        generate_project_package_json(&project, &tmp_dir, &catalog).unwrap();

        let content = std::fs::read_to_string(project_dir.join("package.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(json["version"], "2.1.0");
        assert_eq!(json["private"], false);
        assert_eq!(json["type"], "commonjs");
        assert_eq!(json["types"], "dist/index.d.ts");
        assert_eq!(json["files"], json!(["dist", "README.md"]));

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
