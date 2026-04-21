use anyhow::Result;
use colored::Colorize;
use std::fs;
use std::path::Path;

use crate::manifest::{MANIFEST_FILE, Manifest};
use crate::ownership::{Ownership, get_ownership};
use crate::templates::TemplateEngine;

mod catalog;
mod compose_gen;
pub(crate) mod registry;
mod tsconfig_gen;

use catalog::resolve_catalog_versions;
use compose_gen::generate_workspace_compose;
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

    let config = crate::manifest::GlobalConfig::load().unwrap_or_default();
    match config.backup_strategy {
        crate::manifest::BackupStrategy::None => Ok(()),
        crate::manifest::BackupStrategy::GitCheck => {
            let status = std::process::Command::new("git")
                .args(["status", "--porcelain", &path.to_string_lossy()])
                .output();

            if let Ok(output) = status && !output.stdout.is_empty() {
                println!(
                    "   {} {} has uncommitted changes. Overwriting anyway.",
                    "⚠️".yellow(),
                    path.display()
                );
            }
            Ok(())
        }
        crate::manifest::BackupStrategy::Backup => {
            let backup_dir = Path::new(".airis/backups");
            fs::create_dir_all(backup_dir)?;
            let path_str = path.to_string_lossy().replace('/', "_");
            let backup_path = backup_dir.join(format!("{}.latest", path_str));
            fs::copy(path, &backup_path)?;
            Ok(())
        }
    }
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
    println!("   - compose.yaml");
    println!("   - tsconfig.json");
    Ok(())
}

pub fn sync_from_manifest(manifest: &Manifest) -> Result<()> {
    let engine = TemplateEngine::new()?;
    let mut generated_paths: Vec<String> = Vec::new();

    let registry_path = Path::new(".airis/generated.toml");
    let previous_paths: Vec<String> = load_generation_registry(registry_path);

    if manifest.has_workspace() {
        let resolved_catalog = resolve_catalog_versions(&manifest.packages.catalog)?;

        // Generate Docker Compose (Docker-First SoT)
        generate_workspace_compose(manifest)?;
        generated_paths.push("compose.yaml".into());

        // Generate TSConfig paths (Derived from discovery)
        if !manifest.typescript.skip {
            generate_tsconfig(manifest, &engine, &resolved_catalog)?;
            generated_paths.extend(["tsconfig.base.json".into(), "tsconfig.json".into()]);
        }
    }

    // Clean up orphaned files that are no longer being generated (e.g. package.json, hooks)
    crate::commands::clean::remove_orphaned_files(&previous_paths, &generated_paths, false);
    save_generation_registry(registry_path, &generated_paths)?;

    println!(
        "\n{} Generation complete.",
        "✅".green()
    );
    Ok(())
}
