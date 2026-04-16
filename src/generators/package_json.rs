use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde_json::json;
use std::fs;
use std::path::Path;

use crate::manifest::ProjectDefinition;

/// All fields managed in full-gen mode.
const FULL_GEN_FIELDS: &[&str] = &[
    "name",
    "version",
    "private",
    "type",
    "description",
    "main",
    "types",
    "bin",
    "exports",
    "scripts",
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "peerDependenciesMeta",
    "files",
];

/// Resolved dependencies and scripts for full-gen mode.
pub struct ResolvedPackageData {
    /// Resolved dependencies (package name → version string)
    pub deps: IndexMap<String, String>,
    /// Resolved devDependencies (usually empty — devDeps at root only)
    pub dev_deps: IndexMap<String, String>,
    /// Resolved scripts
    pub scripts: IndexMap<String, String>,
}

/// Generate package.json in full-gen mode.
///
/// This writes ALL fields including dependencies, scripts,
/// exports, main, bin, etc. The package.json is fully managed by airis gen.
pub fn generate_full_package_json(
    project: &ProjectDefinition,
    workspace_root: &Path,
    resolved_catalog: &IndexMap<String, String>,
    resolved_data: &ResolvedPackageData,
) -> Result<()> {
    let project_path = project
        .path
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Project '{}' has no path defined", project.name))?;

    let package_json_path = workspace_root.join(project_path).join("package.json");

    let package_name = if let Some(scope) = &project.scope {
        let scope = scope.trim_start_matches('@');
        format!("@{}/{}", scope, project.name)
    } else {
        format!("@workspace/{}", project.name)
    };

    let version = project.version.as_deref().unwrap_or("0.1.0");
    let private = project.private.unwrap_or(true);
    let module_type = project.module_type.as_deref().unwrap_or("module");

    if let Some(parent) = package_json_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {:?}", parent))?;
    }

    // Build the package.json from scratch (full-gen owns all fields)
    let mut obj = serde_json::Map::new();

    obj.insert("name".to_string(), json!(package_name));
    obj.insert("version".to_string(), json!(version));
    obj.insert("private".to_string(), json!(private));
    obj.insert("type".to_string(), json!(module_type));

    // description
    if let Some(ref desc) = project.description {
        obj.insert("description".to_string(), json!(desc));
    }

    // main
    if let Some(ref main) = project.main {
        obj.insert("main".to_string(), json!(main));
    }

    // types
    if let Some(ref types) = project.types {
        obj.insert("types".to_string(), json!(types));
    }

    // bin
    if !project.bin.is_empty() {
        let bin_map: serde_json::Map<String, serde_json::Value> = project
            .bin
            .iter()
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect();
        obj.insert("bin".to_string(), serde_json::Value::Object(bin_map));
    }

    // exports (free-form TOML value → JSON)
    if let Some(ref exports) = project.exports {
        let exports_json = toml_value_to_json(exports);
        obj.insert("exports".to_string(), exports_json);
    }

    // files
    if !project.files.is_empty() {
        obj.insert("files".to_string(), json!(project.files));
    }

    // scripts
    if !resolved_data.scripts.is_empty() {
        let scripts_map: serde_json::Map<String, serde_json::Value> = resolved_data
            .scripts
            .iter()
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect();
        obj.insert(
            "scripts".to_string(),
            serde_json::Value::Object(scripts_map),
        );
    }

    // dependencies — resolve "catalog" references to actual versions
    if !resolved_data.deps.is_empty() {
        let deps_map: serde_json::Map<String, serde_json::Value> = resolved_data
            .deps
            .iter()
            .map(|(k, v)| {
                let version = resolve_dep_version(k, v, resolved_catalog);
                (k.clone(), json!(version))
            })
            .collect();
        obj.insert(
            "dependencies".to_string(),
            serde_json::Value::Object(deps_map),
        );
    }

    // devDependencies
    if !resolved_data.dev_deps.is_empty() {
        let dev_deps_map: serde_json::Map<String, serde_json::Value> = resolved_data
            .dev_deps
            .iter()
            .map(|(k, v)| {
                let version = resolve_dep_version(k, v, resolved_catalog);
                (k.clone(), json!(version))
            })
            .collect();
        obj.insert(
            "devDependencies".to_string(),
            serde_json::Value::Object(dev_deps_map),
        );
    }

    // peerDependencies
    if !project.peer_deps.is_empty() {
        let peer_map: serde_json::Map<String, serde_json::Value> = project
            .peer_deps
            .iter()
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect();
        obj.insert(
            "peerDependencies".to_string(),
            serde_json::Value::Object(peer_map),
        );
    }

    // peerDependenciesMeta
    if let Some(ref meta) = project.peer_deps_meta {
        let meta_json = toml_value_to_json(meta);
        obj.insert("peerDependenciesMeta".to_string(), meta_json);
    }

    // Generation marker
    obj.insert(
        "_generated".to_string(),
        json!({
            "by": "airis gen",
            "mode": "full",
            "managed_fields": FULL_GEN_FIELDS,
            "warning": "This file is fully generated by airis gen. Do not edit manually."
        }),
    );

    let package_json = serde_json::Value::Object(obj);
    let content =
        serde_json::to_string_pretty(&package_json).context("Failed to serialize package.json")?;

    fs::write(&package_json_path, format!("{content}\n"))
        .with_context(|| format!("Failed to write {:?}", package_json_path))?;

    println!("  ✓ Generated {}", package_json_path.display());

    Ok(())
}

/// Resolve a dependency version string.
/// - "workspace:*" → kept as-is
/// - "catalog" → looked up in resolved_catalog
/// - anything else → kept as-is (explicit version)
fn resolve_dep_version(package: &str, version: &str, catalog: &IndexMap<String, String>) -> String {
    if version == "catalog" {
        catalog
            .get(package)
            .cloned()
            .unwrap_or_else(|| version.to_string())
    } else {
        version.to_string()
    }
}

/// Convert a TOML value to a serde_json Value.
fn toml_value_to_json(val: &toml::Value) -> serde_json::Value {
    match val {
        toml::Value::String(s) => json!(s),
        toml::Value::Integer(i) => json!(i),
        toml::Value::Float(f) => json!(f),
        toml::Value::Boolean(b) => json!(b),
        toml::Value::Datetime(d) => json!(d.to_string()),
        toml::Value::Array(a) => {
            serde_json::Value::Array(a.iter().map(toml_value_to_json).collect())
        }
        toml::Value::Table(t) => {
            let obj: serde_json::Map<String, serde_json::Value> = t
                .iter()
                .map(|(k, v)| (k.clone(), toml_value_to_json(v)))
                .collect();
            serde_json::Value::Object(obj)
        }
    }
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
            tsconfig: None,
            dep_groups: vec![],
            dev_dep_groups: vec![],
        }
    }

    #[test]
    fn test_package_json_generation_full() {
        let tmp_dir = std::env::temp_dir().join("airis_test_full_gen");
        let project_dir = tmp_dir.join("apps/test-app");
        let _ = std::fs::remove_dir_all(&tmp_dir);

        let mut project = default_project("test-app", "apps/test-app");
        project.scope = Some("@myorg".to_string());
        project.version = Some("1.2.3".to_string());

        let mut resolved_data = ResolvedPackageData {
            deps: IndexMap::new(),
            dev_deps: IndexMap::new(),
            scripts: IndexMap::new(),
        };
        resolved_data.deps.insert("react".to_string(), "^19.0.0".to_string());
        resolved_data.scripts.insert("dev".to_string(), "next dev".to_string());

        let catalog = IndexMap::new();
        generate_full_package_json(&project, &tmp_dir, &catalog, &resolved_data).unwrap();

        let content = std::fs::read_to_string(project_dir.join("package.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(json["name"], "@myorg/test-app");
        assert_eq!(json["version"], "1.2.3");
        assert_eq!(json["dependencies"]["react"], "^19.0.0");
        assert_eq!(json["scripts"]["dev"], "next dev");
        assert_eq!(json["_generated"]["mode"], "full");

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}