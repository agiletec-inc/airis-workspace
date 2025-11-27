use anyhow::{Context, Result};
use colored::Colorize;
use indexmap::IndexMap;
use std::env;
use std::fs;
use std::path::Path;

use crate::commands::sync_deps::resolve_version;
use crate::generators::package_json::generate_project_package_json;
use crate::manifest::{CatalogEntry, Manifest, MANIFEST_FILE};
use crate::ownership::{get_ownership, Ownership};
use crate::templates::TemplateEngine;

/// CLI entry point for `airis generate files`
/// Regenerates workspace files from existing manifest.toml
pub fn run(dry_run: bool) -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);

    if !manifest_path.exists() {
        println!("{}", "⛔ manifest.toml not found".bright_red());
        println!();
        println!("{}", "To create manifest.toml, use the MCP tool:".yellow());
        println!("  /airis:init");
        println!();
        println!("{}", "This analyzes your repository and generates an optimized manifest.".cyan());
        return Ok(());
    }

    println!("{}", "📖 Loading manifest.toml...".bright_blue());
    let manifest = Manifest::load(manifest_path)?;

    if dry_run {
        println!("{}", "🔍 Dry-run mode: showing what would be generated...".bright_blue());
        println!();
        preview_from_manifest(&manifest)?;
        println!();
        println!("{}", "ℹ️  No files were written (dry-run mode)".yellow());
        println!("{}", "To actually generate files, run:".bright_yellow());
        println!("  airis generate files");
    } else {
        println!("{}", "🧩 Regenerating workspace files...".bright_blue());
        sync_from_manifest(&manifest)?;
    }

    Ok(())
}

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

/// Preview what files would be generated (dry-run mode)
pub fn preview_from_manifest(manifest: &Manifest) -> Result<()> {
    use std::path::Path;

    println!("{}", "📋 Files that would be generated:".bright_yellow());
    println!();

    // Check existing files vs new files
    let files_to_check = vec![
        ("package.json", true),
        ("Dockerfile", true),
        ("docker-compose.yml", true),
        ("pnpm-workspace.yaml", !manifest.packages.workspaces.is_empty()),
        (".github/workflows/ci.yml", manifest.ci.enabled),
        (".github/workflows/release.yml", manifest.ci.enabled),
    ];

    for (file, should_generate) in files_to_check {
        if !should_generate {
            continue;
        }

        let path = Path::new(file);
        let status = if path.exists() {
            "exists → would write .md for comparison".yellow()
        } else {
            "would be created".green()
        };
        println!("   {} {}", file, status);
    }

    // Show project info
    println!();
    println!("{}", "📦 Project info from manifest.toml:".bright_blue());
    println!("   Name: {}", manifest.project.id);
    if !manifest.project.description.is_empty() {
        println!("   Description: {}", manifest.project.description);
    }
    println!("   CI enabled: {}", manifest.ci.enabled);
    println!("   Workspaces: {:?}", manifest.packages.workspaces);

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
    println!("   - Dockerfile.dev");
    println!("   - docker-compose.yml");
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

    // Don't overwrite existing package.json - write to .md for comparison
    if path.exists() {
        let md_path = Path::new("package.json.md");
        fs::write(md_path, &content)
            .with_context(|| "Failed to write package.json.md")?;
        println!(
            "   {} package.json exists → wrote package.json.md for comparison",
            "📄".yellow()
        );
    } else {
        write_with_backup(path, &content)?;
    }
    Ok(())
}

fn generate_pnpm_workspace(
    manifest: &Manifest,
    engine: &TemplateEngine,
) -> Result<()> {
    let path = Path::new("pnpm-workspace.yaml");
    let content = engine.render_pnpm_workspace(manifest)?;

    // Don't overwrite existing pnpm-workspace.yaml - write to .md for comparison
    if path.exists() {
        let md_path = Path::new("pnpm-workspace.yaml.md");
        fs::write(md_path, &content)
            .with_context(|| "Failed to write pnpm-workspace.yaml.md")?;
        println!(
            "   {} pnpm-workspace.yaml exists → wrote pnpm-workspace.yaml.md for comparison",
            "📄".yellow()
        );
    } else {
        write_with_backup(path, &content)?;
    }
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
    let dockerfile_content = engine.render_dockerfile_dev(manifest)?;
    let compose_content = engine.render_docker_compose(manifest)?;

    // If actual files exist, write to .md for comparison (airis init default)
    // User can review and manually update, or use --force to overwrite
    let dockerfile_path = Path::new("Dockerfile");
    let compose_path = Path::new("docker-compose.yml");

    if dockerfile_path.exists() {
        let md_path = Path::new("Dockerfile.md");
        fs::write(md_path, &dockerfile_content)
            .with_context(|| "Failed to write Dockerfile.md")?;
        println!(
            "   {} Dockerfile exists → wrote Dockerfile.md for comparison",
            "📄".yellow()
        );
    } else {
        fs::write(dockerfile_path, &dockerfile_content)
            .with_context(|| "Failed to write Dockerfile")?;
    }

    if compose_path.exists() {
        let md_path = Path::new("docker-compose.yml.md");
        fs::write(md_path, &compose_content)
            .with_context(|| "Failed to write docker-compose.yml.md")?;
        println!(
            "   {} docker-compose.yml exists → wrote docker-compose.yml.md for comparison",
            "📄".yellow()
        );
    } else {
        fs::write(compose_path, &compose_content)
            .with_context(|| "Failed to write docker-compose.yml")?;
    }

    Ok(())
}

// Note: generate_cargo_toml has been removed
// Cargo.toml is the source of truth for Rust projects and should not be auto-generated
// Use `airis bump-version` to sync versions between manifest.toml and Cargo.toml

fn generate_github_workflows(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    // Create .github/workflows directory
    let workflows_dir = Path::new(".github/workflows");
    fs::create_dir_all(workflows_dir).context("Failed to create .github/workflows directory")?;

    // Generate ci.yml - don't overwrite existing
    let ci_path = workflows_dir.join("ci.yml");
    let ci_content = engine.render_ci_yml(manifest)?;
    if ci_path.exists() {
        let md_path = workflows_dir.join("ci.yml.md");
        fs::write(&md_path, &ci_content)
            .with_context(|| "Failed to write ci.yml.md")?;
        println!(
            "   {} ci.yml exists → wrote ci.yml.md for comparison",
            "📄".yellow()
        );
    } else {
        write_with_backup(&ci_path, &ci_content)?;
    }

    // Generate release.yml - don't overwrite existing
    let release_path = workflows_dir.join("release.yml");
    let release_content = engine.render_release_yml(manifest)?;
    if release_path.exists() {
        let md_path = workflows_dir.join("release.yml.md");
        fs::write(&md_path, &release_content)
            .with_context(|| "Failed to write release.yml.md")?;
        println!(
            "   {} release.yml exists → wrote release.yml.md for comparison",
            "📄".yellow()
        );
    } else {
        write_with_backup(&release_path, &release_content)?;
    }

    Ok(())
}
