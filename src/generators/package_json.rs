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
    /// Resolved devDependencies
    pub dev_deps: IndexMap<String, String>,
    /// Resolved scripts
    pub scripts: IndexMap<String, String>,
}

/// Generate package.json in full-gen mode.
pub fn generate_full_package_json(
    project: &ProjectDefinition,
    workspace_root: &Path,
    _resolved_catalog: &IndexMap<String, String>,
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

    let mut obj = serde_json::Map::new();

    obj.insert("name".to_string(), json!(package_name));
    obj.insert("version".to_string(), json!(version));
    obj.insert("private".to_string(), json!(private));
    obj.insert("type".to_string(), json!(module_type));

    if let Some(ref desc) = project.description {
        obj.insert("description".to_string(), json!(desc));
    }

    if let Some(ref main) = project.main {
        obj.insert("main".to_string(), json!(main));
    }

    if let Some(ref types) = project.types {
        obj.insert("types".to_string(), json!(types));
    }

    if !project.bin.is_empty() {
        let bin_map: serde_json::Map<String, serde_json::Value> = project
            .bin
            .iter()
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect();
        obj.insert("bin".to_string(), serde_json::Value::Object(bin_map));
    }

    if let Some(ref exports) = project.exports {
        let exports_json = toml_value_to_json(exports);
        obj.insert("exports".to_string(), exports_json);
    }

    if !project.files.is_empty() {
        obj.insert("files".to_string(), json!(project.files));
    }

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

    // Dependencies — always use "catalog:" protocol for anything in catalog
    if !resolved_data.deps.is_empty() {
        let deps_map: serde_json::Map<String, serde_json::Value> = resolved_data
            .deps
            .iter()
            .map(|(k, v)| {
                // In pnpm catalogs mode, we prefer "catalog:" for shared deps
                let version = if v == "catalog" || v == "catalog:" {
                    "catalog:".to_string()
                } else {
                    v.clone()
                };
                (k.clone(), json!(version))
            })
            .collect();
        obj.insert(
            "dependencies".to_string(),
            serde_json::Value::Object(deps_map),
        );
    }

    if !resolved_data.dev_deps.is_empty() {
        let dev_deps_map: serde_json::Map<String, serde_json::Value> = resolved_data
            .dev_deps
            .iter()
            .map(|(k, v)| {
                let version = if v == "catalog" || v == "catalog:" {
                    "catalog:".to_string()
                } else {
                    v.clone()
                };
                (k.clone(), json!(version))
            })
            .collect();
        obj.insert(
            "devDependencies".to_string(),
            serde_json::Value::Object(dev_deps_map),
        );
    }

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

    if let Some(ref meta) = project.peer_deps_meta {
        let meta_json = toml_value_to_json(meta);
        obj.insert("peerDependenciesMeta".to_string(), meta_json);
    }

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
