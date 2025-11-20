use anyhow::{Context, Result};
use colored::Colorize;
use indexmap::IndexMap;
use std::env;
use std::fs;
use std::path::Path;

use crate::commands::sync_deps::resolve_version;
use crate::generators::package_json::generate_project_package_json;
use crate::manifest::{CatalogEntry, Manifest};
use crate::ownership::{get_ownership, Ownership};
use crate::templates::TemplateEngine;

/// Backup a file to .airis/backups/ before modification
/// Only backs up tool-owned and hybrid files
fn backup_file(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let ownership = get_ownership(path);
    if !matches!(ownership, Ownership::Tool | Ownership::Hybrid) {
        return Ok(());
    }

    // Create .airis/backups directory
    let backup_dir = Path::new(".airis/backups");
    fs::create_dir_all(backup_dir)
        .with_context(|| "Failed to create .airis/backups directory")?;

    // Create backup filename: replace / with _ for nested paths
    let path_str = path.to_string_lossy().replace('/', "_");
    let backup_path = backup_dir.join(format!("{}.latest", path_str));

    fs::copy(path, &backup_path)
        .with_context(|| format!("Failed to backup {} to {}", path.display(), backup_path.display()))?;

    Ok(())
}

/// Write a file with ownership-aware backup
fn write_with_backup(path: &Path, content: &str) -> Result<()> {
    backup_file(path)?;
    fs::write(path, content)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

/// Sync justfile/docker-compose/package.json from manifest.toml contents
pub fn sync_from_manifest(manifest: &Manifest) -> Result<()> {
    // Resolve catalog versions from npm registry
    let resolved_catalog = resolve_catalog_versions(&manifest.packages.catalog)?;

    let engine = TemplateEngine::new()?;
    println!("{}", "🧩 Rendering templates...".bright_blue());
    generate_docker_compose(&manifest, &engine)?;
    generate_package_json(&manifest, &engine, &resolved_catalog)?;

    // Generate minimal pnpm-workspace.yaml for pnpm compatibility
    // (npm/yarn/bun use workspaces from package.json)
    if !manifest.packages.workspaces.is_empty() {
        generate_pnpm_workspace(&manifest, &engine)?;
    }

    // Check if this is a Rust project (for CI workflow detection)
    let is_rust_project = !manifest.project.rust_edition.is_empty()
        || !manifest.project.binary_name.is_empty();

    // Note: Cargo.toml is NOT generated - it's the source of truth for Rust projects
    // Use `airis bump-version` to sync versions between manifest.toml and Cargo.toml

    // Generate GitHub Actions workflows if CI is enabled
    if manifest.ci.enabled {
        generate_github_workflows(&manifest, &engine)?;
    }

    // Generate individual app package.json files
    if !manifest.app.is_empty() {
        println!();
        println!("{}", "📦 Generating app package.json files...".bright_blue());
        let workspace_root = env::current_dir().context("Failed to get current directory")?;

        for app in &manifest.app {
            generate_project_package_json(app, &workspace_root, &resolved_catalog)?;
        }
    }

    println!();
    println!("{}", "✅ Generated files:".green());
    println!("   - package.json (with workspaces)");
    println!("   - workspace/Dockerfile.dev");
    println!("   - workspace/docker-compose.yml");
    if manifest.ci.enabled {
        println!("   - .github/workflows/ci.yml");
        println!("   - .github/workflows/release.yml");
    }
    if is_rust_project {
        println!();
        println!("{}", "ℹ️  Cargo.toml is not generated (it's the source of truth)".cyan());
        println!("   Use `airis bump-version` to sync versions");
    }
    if !manifest.app.is_empty() {
        println!("   - {} app package.json files", manifest.app.len());
    }
    println!();
    println!("{}", "Next steps:".bright_yellow());
    println!("  1. Run `airis up` to start the workspace");
    println!("  2. Cache directories (.next, .swc, .turbo, node_modules) stay in Docker volumes");

    Ok(())
}

fn generate_package_json(
    manifest: &Manifest,
    engine: &TemplateEngine,
    resolved_catalog: &IndexMap<String, String>,
) -> Result<()> {
    let path = Path::new("package.json");
    let content = engine.render_package_json(manifest, resolved_catalog)?;
    write_with_backup(path, &content)?;
    Ok(())
}

fn generate_pnpm_workspace(
    manifest: &Manifest,
    engine: &TemplateEngine,
) -> Result<()> {
    let path = Path::new("pnpm-workspace.yaml");
    let content = engine.render_pnpm_workspace(manifest)?;
    write_with_backup(path, &content)?;
    Ok(())
}

/// Resolve catalog version policies to actual version numbers
fn resolve_catalog_versions(
    catalog: &IndexMap<String, CatalogEntry>,
) -> Result<IndexMap<String, String>> {
    if catalog.is_empty() {
        return Ok(IndexMap::new());
    }

    println!("{}", "📦 Resolving catalog versions from npm registry...".bright_blue());

    let mut resolved: IndexMap<String, String> = IndexMap::new();

    for (package, entry) in catalog {
        let version = match entry {
            CatalogEntry::Policy(policy) => {
                let policy_str = policy.as_str();
                let version = resolve_version(package, policy_str)?;
                println!("  ✓ {} {} → {}", package, policy_str, version);
                version
            }
            CatalogEntry::Version(version) => {
                println!("  ✓ {} {}", package, version);
                version.clone()
            }
            CatalogEntry::Follow(follow_config) => {
                // For follow entries, we'll resolve them in a second pass
                let target = &follow_config.follow;
                if let Some(target_version) = resolved.get(target) {
                    println!("  ✓ {} (follow {}) → {}", package, target, target_version);
                    target_version.clone()
                } else {
                    anyhow::bail!(
                        "Cannot resolve '{}': follow target '{}' not found or not yet resolved",
                        package,
                        target
                    );
                }
            }
        };

        resolved.insert(package.clone(), version);
    }

    Ok(resolved)
}

fn generate_docker_compose(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    // Create workspace/ directory if it doesn't exist
    let workspace_dir = Path::new("workspace");
    fs::create_dir_all(workspace_dir).context("Failed to create workspace/ directory")?;

    // Generate Dockerfile.dev
    let dockerfile_path = workspace_dir.join("Dockerfile.dev");
    let dockerfile_content = engine.render_dockerfile_dev(manifest)?;
    write_with_backup(&dockerfile_path, &dockerfile_content)?;

    // Generate docker-compose.yml
    let compose_path = workspace_dir.join("docker-compose.yml");
    let compose_content = engine.render_docker_compose(manifest)?;
    write_with_backup(&compose_path, &compose_content)?;

    Ok(())
}

// Note: generate_cargo_toml has been removed
// Cargo.toml is the source of truth for Rust projects and should not be auto-generated
// Use `airis bump-version` to sync versions between manifest.toml and Cargo.toml

fn generate_github_workflows(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    // Create .github/workflows directory
    let workflows_dir = Path::new(".github/workflows");
    fs::create_dir_all(workflows_dir).context("Failed to create .github/workflows directory")?;

    // Generate ci.yml
    let ci_path = workflows_dir.join("ci.yml");
    let ci_content = engine.render_ci_yml(manifest)?;
    write_with_backup(&ci_path, &ci_content)?;

    // Generate release.yml
    let release_path = workflows_dir.join("release.yml");
    let release_content = engine.render_release_yml(manifest)?;
    write_with_backup(&release_path, &release_content)?;

    Ok(())
}
