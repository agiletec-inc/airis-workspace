use super::TemplateEngine;
use crate::manifest::Manifest;
use anyhow::Context;
use anyhow::Result;
use indexmap::IndexMap;

impl TemplateEngine {
    /// Render root package.json.
    ///
    /// Root package.json now uses "catalog:" protocol for shared dependencies,
    /// delegating actual resolution and locking to pnpm.
    #[allow(dead_code)]
    pub fn render_package_json(
        &self,
        manifest: &Manifest,
        resolved_catalog: &IndexMap<String, String>,
    ) -> Result<String> {
        let mut obj = serde_json::Map::new();

        obj.insert(
            "name".to_string(),
            serde_json::json!(manifest.workspace.name),
        );
        obj.insert("version".to_string(), serde_json::json!("0.0.0"));
        obj.insert("private".to_string(), serde_json::json!(true));
        obj.insert("type".to_string(), serde_json::json!("module"));

        if !manifest.workspace.package_manager.is_empty() {
            obj.insert(
                "packageManager".to_string(),
                serde_json::json!(manifest.workspace.package_manager),
            );
        }

        if !manifest.packages.workspaces.is_empty() {
            obj.insert(
                "workspaces".to_string(),
                serde_json::to_value(&manifest.packages.workspaces)?,
            );
        }

        // Add root dependencies from manifest.packages.root (v1) and manifest.root (v2).
        // manifest.root values are merged on top of manifest.packages.root so that
        // [root] declarations in manifest.toml always win.
        let root_pkg = &manifest.packages.root;

        // Merge scripts: packages.root first, then [root] overrides.
        let mut merged_scripts = root_pkg.scripts.clone();
        if let Some(ref root) = manifest.root {
            for (k, v) in &root.scripts {
                merged_scripts.insert(k.clone(), v.clone());
            }
        }
        if !merged_scripts.is_empty() {
            obj.insert(
                "scripts".to_string(),
                serde_json::to_value(&merged_scripts)?,
            );
        }

        // Merge dependencies: packages.root first, then [root] overrides.
        let mut merged_deps = root_pkg.dependencies.clone();
        if let Some(ref root) = manifest.root {
            for (k, v) in &root.dependencies {
                merged_deps.insert(k.clone(), v.clone());
            }
        }
        if !merged_deps.is_empty() {
            let mut deps = serde_json::Map::new();
            for (k, v) in &merged_deps {
                // If it's in the manifest catalog, use "catalog:"
                let version =
                    if resolved_catalog.contains_key(k) || v == "catalog" || v == "catalog:" {
                        "catalog:".to_string()
                    } else {
                        v.clone()
                    };
                deps.insert(k.clone(), serde_json::json!(version));
            }
            obj.insert("dependencies".to_string(), serde_json::Value::Object(deps));
        }

        // Merge devDependencies: packages.root first, then [root] overrides.
        let mut merged_dev_deps = root_pkg.dev_dependencies.clone();
        if let Some(ref root) = manifest.root {
            for (k, v) in &root.dev_dependencies {
                merged_dev_deps.insert(k.clone(), v.clone());
            }
        }
        if !merged_dev_deps.is_empty() {
            let mut dev_deps = serde_json::Map::new();
            for (k, v) in &merged_dev_deps {
                let version =
                    if resolved_catalog.contains_key(k) || v == "catalog" || v == "catalog:" {
                        "catalog:".to_string()
                    } else {
                        v.clone()
                    };
                dev_deps.insert(k.clone(), serde_json::json!(version));
            }
            obj.insert(
                "devDependencies".to_string(),
                serde_json::Value::Object(dev_deps),
            );
        }

        // Merge engines: packages.root first, then [root] overrides.
        let mut merged_engines = root_pkg.engines.clone();
        if let Some(ref root) = manifest.root {
            for (k, v) in &root.engines {
                merged_engines.insert(k.clone(), v.clone());
            }
        }
        if !merged_engines.is_empty() {
            obj.insert(
                "engines".to_string(),
                serde_json::to_value(&merged_engines)?,
            );
        }

        // pnpm specific config
        if !root_pkg.pnpm.overrides.is_empty()
            || !root_pkg
                .pnpm
                .peer_dependency_rules
                .ignore_missing
                .is_empty()
        {
            let mut pnpm = serde_json::Map::new();
            if !root_pkg.pnpm.overrides.is_empty() {
                pnpm.insert(
                    "overrides".to_string(),
                    serde_json::to_value(&root_pkg.pnpm.overrides)?,
                );
            }
            obj.insert("pnpm".to_string(), serde_json::Value::Object(pnpm));
        }

        // Update generation marker
        obj.insert(
            "_generated".to_string(),
            serde_json::json!({
                "by": "airis gen",
                "mode": "full",
                "warning": "This file is fully generated by airis gen. Do not edit manually."
            }),
        );

        let package_json = serde_json::Value::Object(obj);
        let content = serde_json::to_string_pretty(&package_json)
            .context("Failed to serialize package.json")?;
        Ok(format!("{content}\n"))
    }
}
