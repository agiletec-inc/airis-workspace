use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde_json::json;
use std::fs;
use std::path::Path;

use crate::manifest::ProjectDefinition;

/// Fields managed by airis in per-app package.json (hybrid mode).
/// All other fields (dependencies, scripts, exports, etc.) are user-managed.
const MANAGED_FIELDS: &[&str] = &["name", "version", "private", "type"];

/// Generate or update package.json for a project (hybrid mode).
///
/// Hybrid mode: airis manages only name/version/private/type.
/// Dependencies, scripts, exports, and all other fields stay in the
/// user's package.json and are never overwritten.
pub fn generate_project_package_json(
    project: &ProjectDefinition,
    workspace_root: &Path,
    _resolved_catalog: &IndexMap<String, String>,
) -> Result<()> {
    let project_path = project
        .path
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Project '{}' has no path defined", project.name))?;

    let package_json_path = workspace_root.join(project_path).join("package.json");

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

    // Ensure directory exists
    if let Some(parent) = package_json_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {:?}", parent))?;
    }

    let package_json = if package_json_path.exists() {
        // Hybrid: read existing, update only managed fields
        let existing = fs::read_to_string(&package_json_path)
            .with_context(|| format!("Failed to read {:?}", package_json_path))?;
        let mut json: serde_json::Value = serde_json::from_str(&existing)
            .with_context(|| format!("Failed to parse {:?}", package_json_path))?;
        let obj = json.as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("package.json is not a JSON object: {:?}", package_json_path))?;

        obj.insert("name".to_string(), json!(package_name));
        obj.insert("version".to_string(), json!(version));
        obj.insert("private".to_string(), json!(private));
        obj.insert("type".to_string(), json!(module_type));

        // Update generation marker
        obj.insert("_generated".to_string(), json!({
            "by": "airis gen",
            "managed_fields": MANAGED_FIELDS,
            "warning": "Only the fields listed in managed_fields are updated by airis gen. Everything else is yours."
        }));

        json
    } else {
        // New file: create minimal package.json
        json!({
            "name": package_name,
            "version": version,
            "private": private,
            "type": module_type,
            "_generated": {
                "by": "airis gen",
                "managed_fields": MANAGED_FIELDS,
                "warning": "Only the fields listed in managed_fields are updated by airis gen. Everything else is yours."
            }
        })
    };

    let content = serde_json::to_string_pretty(&package_json)
        .context("Failed to serialize package.json")?;

    fs::write(&package_json_path, format!("{content}\n"))
        .with_context(|| format!("Failed to write {:?}", package_json_path))?;

    println!("  ✓ Synced {}", package_json_path.display());

    Ok(())
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
            preset: None,
            profiles: None,
            depends_on: None,
            mem_limit: None,
            cpus: None,
            service: None,
        }
    }

    #[test]
    fn test_new_package_json_created_with_managed_fields_only() {
        let tmp_dir = std::env::temp_dir().join("airis_test_hybrid_new");
        let project_dir = tmp_dir.join("apps/test-app");
        // Ensure clean state
        let _ = std::fs::remove_dir_all(&tmp_dir);

        let mut project = default_project("test-app", "apps/test-app");
        project.scope = Some("@myorg".to_string());

        let catalog = IndexMap::new();
        generate_project_package_json(&project, &tmp_dir, &catalog).unwrap();

        let content = std::fs::read_to_string(project_dir.join("package.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Managed fields are set
        assert_eq!(json["name"], "@myorg/test-app");
        assert_eq!(json["version"], "0.1.0");
        assert_eq!(json["private"], true);
        assert_eq!(json["type"], "module");

        // No deps/scripts/exports — user adds these themselves
        assert!(json.get("dependencies").is_none());
        assert!(json.get("devDependencies").is_none());
        assert!(json.get("scripts").is_none());

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_existing_package_json_preserves_user_fields() {
        let tmp_dir = std::env::temp_dir().join("airis_test_hybrid_merge");
        let project_dir = tmp_dir.join("apps/my-app");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&project_dir).unwrap();

        // Write a user-managed package.json with deps, scripts, etc.
        let user_pkg = json!({
            "name": "@old/my-app",
            "version": "0.0.1",
            "scripts": { "dev": "next dev", "build": "next build" },
            "dependencies": { "react": "^19.0.0", "next": "^15.0.0" },
            "devDependencies": { "typescript": "^5.0.0" },
            "exports": { ".": "./dist/index.js" },
            "description": "My cool app"
        });
        std::fs::write(
            project_dir.join("package.json"),
            serde_json::to_string_pretty(&user_pkg).unwrap(),
        ).unwrap();

        let mut project = default_project("my-app", "apps/my-app");
        project.scope = Some("@agiletec".to_string());
        project.version = Some("1.0.0".to_string());

        let catalog = IndexMap::new();
        generate_project_package_json(&project, &tmp_dir, &catalog).unwrap();

        let content = std::fs::read_to_string(project_dir.join("package.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Managed fields are updated
        assert_eq!(json["name"], "@agiletec/my-app");
        assert_eq!(json["version"], "1.0.0");

        // User fields are preserved
        assert_eq!(json["scripts"]["dev"], "next dev");
        assert_eq!(json["scripts"]["build"], "next build");
        assert_eq!(json["dependencies"]["react"], "^19.0.0");
        assert_eq!(json["dependencies"]["next"], "^15.0.0");
        assert_eq!(json["devDependencies"]["typescript"], "^5.0.0");
        assert_eq!(json["exports"]["."], "./dist/index.js");
        assert_eq!(json["description"], "My cool app");

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_default_scope_is_workspace() {
        let tmp_dir = std::env::temp_dir().join("airis_test_hybrid_default_scope");
        let project_dir = tmp_dir.join("libs/my-lib");
        let _ = std::fs::remove_dir_all(&tmp_dir);

        let project = default_project("my-lib", "libs/my-lib");

        let catalog = IndexMap::new();
        generate_project_package_json(&project, &tmp_dir, &catalog).unwrap();

        let content = std::fs::read_to_string(project_dir.join("package.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(json["name"], "@workspace/my-lib");

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_custom_version_and_type() {
        let tmp_dir = std::env::temp_dir().join("airis_test_hybrid_custom");
        let project_dir = tmp_dir.join("libs/legacy");
        let _ = std::fs::remove_dir_all(&tmp_dir);

        let mut project = default_project("legacy", "libs/legacy");
        project.version = Some("2.1.0".to_string());
        project.private = Some(false);
        project.module_type = Some("commonjs".to_string());

        let catalog = IndexMap::new();
        generate_project_package_json(&project, &tmp_dir, &catalog).unwrap();

        let content = std::fs::read_to_string(project_dir.join("package.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(json["version"], "2.1.0");
        assert_eq!(json["private"], false);
        assert_eq!(json["type"], "commonjs");

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
