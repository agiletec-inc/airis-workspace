use anyhow::{Context, Result};
use colored::Colorize;
use std::env;
use std::fs;
use std::path::Path;

use crate::commands::discover::discover_from_workspaces;
use crate::manifest::{MANIFEST_FILE, Manifest};
use crate::ownership::{Ownership, get_ownership};
use crate::templates::TemplateEngine;

mod catalog;
mod hooks_gen;
mod package_gen;
mod registry;
mod tsconfig_gen;

use catalog::{resolve_catalog_versions, resolve_package_data};
use hooks_gen::generate_native_hooks;
use package_gen::{generate_package_json, generate_pnpm_workspace};
use registry::{load_generation_registry, save_generation_registry};
use tsconfig_gen::generate_tsconfig;

#[cfg(test)]
mod tests;

/// CLI entry point for `airis gen`
pub fn run(dry_run: bool, _force: bool, _migrate: bool) -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);

    if !manifest_path.exists() {
        println!("{}", "⛔ manifest.toml not found".bright_red());
        return Ok(());
    }

    let manifest = Manifest::load(manifest_path)?;

    if dry_run {
        preview_from_manifest(&manifest)?;
    } else {
        println!("{}", "🧩 Regenerating workspace files...".bright_blue());
        sync_from_manifest(&manifest)?;
    }

    Ok(())
}

pub(super) fn backup_file(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let ownership = get_ownership(path);
    if !matches!(ownership, Ownership::Tool) {
        return Ok(());
    }

    let backup_dir = Path::new(".airis/backups");
    fs::create_dir_all(backup_dir)?;

    let path_str = path.to_string_lossy().replace('/', "_");
    let backup_path = backup_dir.join(format!("{}.latest", path_str));

    fs::copy(path, &backup_path)?;
    Ok(())
}

pub(super) fn write_with_backup(path: &Path, content: &str) -> Result<()> {
    let ownership = get_ownership(path);
    if matches!(ownership, Ownership::User) {
        return Ok(());
    }
    backup_file(path)?;
    fs::write(path, content)?;
    Ok(())
}

pub fn preview_from_manifest(_manifest: &Manifest) -> Result<()> {
    println!("{}", "📋 Files that would be generated:".bright_yellow());
    // (Simplified preview logic)
    Ok(())
}

pub fn sync_from_manifest(manifest: &Manifest) -> Result<()> {
    let engine = TemplateEngine::new()?;
    let mut generated_paths: Vec<String> = Vec::new();

    let registry_path = Path::new(".airis/generated.toml");
    let previous_paths: Vec<String> = load_generation_registry(registry_path);

    if manifest.has_workspace() {
        let mut resolved_catalog = resolve_catalog_versions(&manifest.packages.catalog)?;

        generate_package_json(manifest, &engine, &resolved_catalog, true)?;
        generated_paths.push("package.json".into());

        if !manifest.packages.workspaces.is_empty() {
            generate_pnpm_workspace(manifest, &engine, true)?;
            generated_paths.push("pnpm-workspace.yaml".into());
        }

        let workspace_root = env::current_dir()?;
        let workspace_scope = manifest.workspace.scope.as_deref().unwrap_or("@workspace");
        let workspace_patterns = if !manifest.packages.workspaces.is_empty() {
            &manifest.packages.workspaces
        } else {
            &manifest.workspace.workspaces
        };

        let explicit_names: std::collections::HashSet<String> =
            manifest.app.iter().map(|a| a.name.clone()).collect();

        if !workspace_patterns.is_empty() {
            let discovered = discover_from_workspaces(workspace_patterns, &workspace_root)?;
            for disc in &discovered {
                if explicit_names.contains(&disc.name) { continue; }
                let mut auto_app = crate::manifest::ProjectDefinition {
                    path: Some(disc.path.clone()),
                    framework: Some(disc.framework.to_string()),
                    ..Default::default()
                };
                auto_app.resolve(&manifest.workspace);

                let resolved_data = resolve_package_data(
                    &auto_app,
                    &workspace_root,
                    workspace_scope,
                    &mut resolved_catalog,
                    &manifest.packages.catalog,
                    &manifest.preset,
                    &manifest.dep_group,
                )?;
                crate::generators::package_json::generate_full_package_json(
                    &auto_app,
                    &workspace_root,
                    &resolved_catalog,
                    &resolved_data,
                )?;
                if let Some(ref path) = auto_app.path {
                    generated_paths.push(format!("{}/package.json", path));
                }
            }
        }

        for app in &manifest.app {
            let resolved_data = resolve_package_data(
                app,
                &workspace_root,
                workspace_scope,
                &mut resolved_catalog,
                &manifest.packages.catalog,
                &manifest.preset,
                &manifest.dep_group,
            )?;
            crate::generators::package_json::generate_full_package_json(
                app,
                &workspace_root,
                &resolved_catalog,
                &resolved_data,
            )?;
            if let Some(ref path) = app.path {
                generated_paths.push(format!("{}/package.json", path));
            }
        }

        if !manifest.typescript.skip {
            generate_tsconfig(manifest, &engine, &resolved_catalog)?;
            generated_paths.extend(["tsconfig.base.json".into(), "tsconfig.json".into()]);
        }

        generate_native_hooks()?;
        generated_paths.extend(["hooks/pre-commit".into(), "hooks/pre-push".into()]);
    }

    crate::commands::clean::remove_orphaned_files(&previous_paths, &generated_paths, false);
    save_generation_registry(registry_path, &generated_paths)?;

    println!("\n{} Generation complete. Run `pnpm install` to resolve versions.", "✅".green());
    Ok(())
}
