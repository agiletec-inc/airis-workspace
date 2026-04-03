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
mod docker_gen;
mod env_gen;
mod hooks_gen;
mod lockfile;
mod package_gen;
mod registry;
mod tsconfig_gen;

use catalog::{resolve_catalog_versions, resolve_package_data};
use docker_gen::generate_docker_compose;
use env_gen::{generate_env_example, generate_envrc, generate_npmrc};
use hooks_gen::{generate_git_hooks, generate_native_hooks};
use lockfile::sync_lockfile;
use package_gen::{generate_package_json, generate_pnpm_workspace};
use registry::{load_generation_registry, save_generation_registry};
use tsconfig_gen::generate_tsconfig;

#[cfg(test)]
mod tests;

/// Legacy compose file names that should be migrated to compose.yml
const LEGACY_COMPOSE_FILES: &[&str] =
    &["docker-compose.yml", "docker-compose.yaml", "compose.yaml"];

/// Compose override/variant files that should not exist (use manifest.toml instead)
const COMPOSE_VARIANTS: &[&str] = &[
    "compose.override.yml",
    "compose.override.yaml",
    "compose.dev.yml",
    "compose.prod.yml",
    "compose.test.yml",
    "docker-compose.override.yml",
    "docker-compose.override.yaml",
    "docker-compose.dev.yml",
    "docker-compose.prod.yml",
    "docker-compose.test.yml",
];

/// Check for legacy or variant compose files and return them
fn detect_legacy_compose_files() -> Vec<String> {
    let mut found = Vec::new();
    for name in LEGACY_COMPOSE_FILES.iter().chain(COMPOSE_VARIANTS.iter()) {
        if Path::new(name).exists() {
            found.push(name.to_string());
        }
    }
    found
}

/// CLI entry point for `airis gen`
/// Regenerates workspace files from existing manifest.toml
pub fn run(dry_run: bool, force: bool, migrate: bool) -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);

    if !manifest_path.exists() {
        println!("{}", "⛔ manifest.toml not found".bright_red());
        println!();
        println!("{}", "To create manifest.toml, use the MCP tool:".yellow());
        println!("  /airis:init");
        println!();
        println!(
            "{}",
            "This analyzes your repository and generates an optimized manifest.".cyan()
        );
        return Ok(());
    }

    // Check for legacy compose files
    let legacy_files = detect_legacy_compose_files();
    if !legacy_files.is_empty() && !force && !migrate && !dry_run {
        println!("{}", "⛔ Legacy compose files detected:".bright_red());
        for f in &legacy_files {
            println!("   {} {}", "•".red(), f);
        }
        println!();
        println!(
            "Only {} is supported. Choose an action:",
            "compose.yml".bright_cyan()
        );
        println!(
            "  {} — ignore legacy files and generate compose.yml",
            "airis gen --force".bright_cyan()
        );
        println!(
            "  {} — delete legacy files and generate compose.yml",
            "airis gen --migrate".bright_cyan()
        );
        anyhow::bail!("Legacy compose files exist. Use --force or --migrate.");
    }

    // Migrate: delete legacy files
    if migrate && !legacy_files.is_empty() {
        println!("{}", "🔄 Migrating compose files...".bright_blue());
        for f in &legacy_files {
            fs::remove_file(f)?;
            println!("   {} Deleted {}", "✗".red(), f);
        }
        println!();
    }

    println!("{}", "📖 Loading manifest.toml...".bright_blue());
    let manifest = Manifest::load(manifest_path)?;

    if dry_run {
        println!(
            "{}",
            "🔍 Dry-run mode: showing what would be generated...".bright_blue()
        );
        println!();
        preview_from_manifest(&manifest)?;
        println!();
        println!("{}", "ℹ️  No files were written (dry-run mode)".yellow());
        println!("{}", "To actually generate files, run:".bright_yellow());
        println!("  airis gen");
    } else {
        println!("{}", "🧩 Regenerating workspace files...".bright_blue());
        sync_from_manifest(&manifest)?;
    }

    Ok(())
}

/// Backup a file to .airis/backups/ before modification
/// Only backs up tool-owned and hybrid files
pub(super) fn backup_file(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let ownership = get_ownership(path);
    if !matches!(ownership, Ownership::Tool | Ownership::Hybrid) {
        return Ok(());
    }

    // Create .airis/backups directory
    let backup_dir = Path::new(".airis/backups");
    fs::create_dir_all(backup_dir).with_context(|| "Failed to create .airis/backups directory")?;

    // Create backup filename: replace / with _ for nested paths
    let path_str = path.to_string_lossy().replace('/', "_");
    let backup_path = backup_dir.join(format!("{}.latest", path_str));

    fs::copy(path, &backup_path).with_context(|| {
        format!(
            "Failed to backup {} to {}",
            path.display(),
            backup_path.display()
        )
    })?;

    Ok(())
}

/// Write a file with ownership-aware backup
pub(super) fn write_with_backup(path: &Path, content: &str) -> Result<()> {
    backup_file(path)?;
    fs::write(path, content).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

/// Preview what files would be generated (dry-run mode)
pub fn preview_from_manifest(manifest: &Manifest) -> Result<()> {
    use std::path::Path;

    println!("{}", "📋 Files that would be generated:".bright_yellow());
    println!();

    let has_workspace = manifest.has_workspace();

    // Check existing files vs new files
    let files_to_check = vec![
        ("package.json", has_workspace),
        ("compose.yml", has_workspace),
        (
            "pnpm-workspace.yaml",
            has_workspace && !manifest.packages.workspaces.is_empty(),
        ),
        (
            "tsconfig.base.json",
            has_workspace && !manifest.typescript.skip,
        ),
        ("tsconfig.json", has_workspace && !manifest.typescript.skip),
    ];

    for (file, should_generate) in files_to_check {
        if !should_generate {
            continue;
        }

        let path = Path::new(file);
        let ownership = get_ownership(path);
        let status = if path.exists() {
            match ownership {
                Ownership::Tool => "exists → would overwrite (tool-owned)".green(),
                Ownership::Hybrid => "exists → would update (marker-protected)".green(),
                Ownership::User => "exists → would skip (user-owned)".yellow(),
            }
        } else {
            "would be created".green()
        };
        println!("   {} {}", file, status);
    }

    println!();
    println!(
        "   {} Use `airis diff` to preview changes before generating.",
        "💡".cyan()
    );

    // Show project info
    println!();
    println!("{}", "📦 Project info from manifest.toml:".bright_blue());
    println!("   Name: {}", manifest.project.id);
    if !manifest.project.description.is_empty() {
        println!("   Description: {}", manifest.project.description);
    }
    // CI workflows are project-owned (not generated by airis)
    println!("   Workspaces: {:?}", manifest.packages.workspaces);

    Ok(())
}

/// Sync generated files from manifest.toml contents
///
/// All tool-owned files are always overwritten (with backup to .airis/backups/).
/// The `force` parameter is retained for API compatibility but has no effect.
pub fn sync_from_manifest(manifest: &Manifest) -> Result<()> {
    sync_from_manifest_with_force(manifest, false)
}

/// Sync from manifest with explicit force flag
pub fn sync_from_manifest_with_force(manifest: &Manifest, force: bool) -> Result<()> {
    let has_workspace = manifest.has_workspace();
    let engine = TemplateEngine::new()?;
    let mut generated_files: Vec<String> = Vec::new();
    let mut generated_paths: Vec<String> = Vec::new(); // Actual file paths for orphan tracking
    // Load or initialize airis.lock
    let lock_path = Path::new(crate::manifest::LOCK_FILE);
    let mut lock = if lock_path.exists() {
        crate::manifest::Lock::load(lock_path)?
    } else {
        crate::manifest::Lock::default()
    };

    // Load previous generation registry for orphan detection
    let registry_path = Path::new(".airis/generated.toml");
    let previous_paths: Vec<String> = load_generation_registry(registry_path);

    // Node.js workspace files (only when [workspace] package_manager is set)
    if has_workspace {
        let mut resolved_catalog = resolve_catalog_versions(
            &manifest.packages.catalog,
            &mut lock,
            manifest.packages.default_policy.as_deref(),
        )?;

        println!("{}", "🧩 Rendering templates...".bright_blue());
        generate_docker_compose(manifest, &engine, force)?;
        generated_paths.push("compose.yml".into());
        generate_package_json(manifest, &engine, &resolved_catalog, force)?;
        generated_paths.push("package.json".into());

        generated_files.extend([
            "package.json (with workspaces)".into(),
            "compose.yml".into(),
        ]);

        if !manifest.packages.workspaces.is_empty() {
            generate_pnpm_workspace(manifest, &engine, force)?;
            generated_paths.push("pnpm-workspace.yaml".into());
        }

        // Generate individual app package.json files (auto-discovery + explicit)
        {
            println!();
            println!(
                "{}",
                "📦 Generating app package.json files (full-gen mode)...".bright_blue()
            );
            let workspace_root = env::current_dir().context("Failed to get current directory")?;

            // Workspace scope for import scanner (e.g., "@agiletec")
            let workspace_scope = manifest.workspace.scope.as_deref().unwrap_or("@workspace");

            // Collect workspace patterns from both v1 and v2 locations
            let workspace_patterns = if !manifest.packages.workspaces.is_empty() {
                &manifest.packages.workspaces
            } else {
                &manifest.workspace.workspaces
            };

            // Build set of explicitly defined app names (these take priority)
            let explicit_names: std::collections::HashSet<String> =
                manifest.app.iter().map(|a| a.name.clone()).collect();

            // Auto-discover projects from workspace patterns
            let mut app_count = 0;
            if !workspace_patterns.is_empty() {
                let discovered = discover_from_workspaces(workspace_patterns, &workspace_root)?;
                for disc in &discovered {
                    if explicit_names.contains(&disc.name) {
                        continue; // Explicit [[app]] takes priority
                    }
                    let mut auto_app = crate::manifest::ProjectDefinition {
                        path: Some(disc.path.clone()),
                        framework: Some(disc.framework.to_string()),
                        ..Default::default()
                    };
                    auto_app.resolve(&manifest.workspace);

                    // Full-gen: scan imports + convention scripts
                    let resolved_data = resolve_package_data(
                        &auto_app,
                        &workspace_root,
                        workspace_scope,
                        &mut resolved_catalog,
                        &manifest.packages.catalog,
                        &mut lock,
                        &manifest.preset,
                        &manifest.dep_group,
                        manifest.packages.default_policy.as_deref(),
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
                    app_count += 1;
                }
            }

            // Generate for explicitly defined apps
            for app in &manifest.app {
                let resolved_data = resolve_package_data(
                    app,
                    &workspace_root,
                    workspace_scope,
                    &mut resolved_catalog,
                    &manifest.packages.catalog,
                    &mut lock,
                    &manifest.preset,
                    &manifest.dep_group,
                    manifest.packages.default_policy.as_deref(),
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
                app_count += 1;
            }

            generated_files.push(format!("{} app package.json files (full-gen)", app_count));
        }

        // Generate .npmrc for pnpm store isolation
        generate_npmrc(&engine)?;
        generated_paths.push(".npmrc".into());
        generated_files.push(".npmrc (pnpm store isolation)".into());

        // Generate tsconfig files (tsconfig.base.json + tsconfig.json)
        if !manifest.typescript.skip {
            generate_tsconfig(manifest, &engine, &resolved_catalog)?;
            generated_paths.extend(["tsconfig.base.json".into(), "tsconfig.json".into()]);
            generated_files.push("tsconfig.base.json + tsconfig.json".into());
        }

        // Generate .env.example if [env] section has required or optional vars
        if !manifest.env.required.is_empty() || !manifest.env.optional.is_empty() {
            generate_env_example(manifest, &engine)?;
            generated_files.push(".env.example".into());
        }

        // Generate .envrc for direnv
        generate_envrc(manifest, &engine)?;
        generated_paths.push(".envrc".into());
        generated_files.push(".envrc".into());

        // Generate git hooks
        generate_git_hooks(&engine)?;
        generate_native_hooks()?;
        generated_paths.extend([
            ".husky/pre-commit".into(),
            ".husky/pre-push".into(),
            "hooks/pre-commit".into(),
            "hooks/pre-push".into(),
        ]);
        generated_files.extend([
            ".husky/pre-commit".into(),
            ".husky/pre-push".into(),
            "hooks/pre-commit".into(),
            "hooks/pre-push".into(),
        ]);

        // Sync pnpm-lock.yaml
        sync_lockfile(manifest)?;

        // Save updated airis.lock
        lock.save(lock_path)?;
        generated_files.push("airis.lock (synced)".into());
        generated_paths.push("airis.lock".into());
    }

    // Clean orphaned generated files (previously generated but no longer needed)
    let orphan_count =
        crate::commands::clean::remove_orphaned_files(&previous_paths, &generated_paths, false);
    if orphan_count > 0 {
        println!("   🧹 Removed {} orphaned file(s)", orphan_count);
    }

    // Save current generation registry
    save_generation_registry(registry_path, &generated_paths)?;

    // Summary
    println!();
    println!("{}", "✅ Generated files:".green());
    for file in &generated_files {
        println!("   - {}", file);
    }
    let is_rust_project =
        !manifest.project.rust_edition.is_empty() || !manifest.project.binary_name.is_empty();
    if is_rust_project {
        println!();
        println!(
            "{}",
            "ℹ️  Cargo.toml is not generated (it's the source of truth)".cyan()
        );
        println!("   Use `airis bump-version` to sync versions");
    }

    if has_workspace {
        println!();
        println!("{}", "Next steps:".bright_yellow());
        println!("  1. Run `airis up` to start the workspace");
        println!("  2. Run `airis hooks install` to install Git hooks");
        println!(
            "  3. Cache directories (.next, .swc, .turbo, node_modules) stay in Docker volumes"
        );
    }

    Ok(())
}
