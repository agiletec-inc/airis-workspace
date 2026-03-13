use anyhow::{Context, Result};
use colored::Colorize;
use indexmap::IndexMap;
use std::env;
use std::fs;
use std::path::Path;

use crate::version_resolver::resolve_version;
use crate::generators::package_json::generate_project_package_json;
use crate::manifest::{CatalogEntry, Manifest, MANIFEST_FILE};
use crate::ownership::{get_ownership, Ownership};
use crate::templates::TemplateEngine;

/// CLI entry point for `airis gen`
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
        println!("  airis gen");
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
        ("compose.yml", true),
        ("pnpm-workspace.yaml", !manifest.packages.workspaces.is_empty()),
        (".github/workflows/ci.yml", manifest.ci.enabled),
        (".github/workflows/release.yml", manifest.ci.enabled),
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
    println!("   {} Use `airis diff` to preview changes before generating.", "💡".cyan());

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

/// Sync generated files from manifest.toml contents
///
/// All tool-owned files are always overwritten (with backup to .airis/backups/).
/// The `force` parameter is retained for API compatibility but has no effect.
pub fn sync_from_manifest(manifest: &Manifest) -> Result<()> {
    sync_from_manifest_with_force(manifest, false)
}

/// Sync from manifest with explicit force flag
pub fn sync_from_manifest_with_force(manifest: &Manifest, force: bool) -> Result<()> {
    // Resolve catalog versions from npm registry
    let resolved_catalog = resolve_catalog_versions(&manifest.packages.catalog)?;

    let engine = TemplateEngine::new()?;
    println!("{}", "🧩 Rendering templates...".bright_blue());
    generate_docker_compose(manifest, &engine, force)?;
    generate_package_json(manifest, &engine, &resolved_catalog, force)?;

    // Generate minimal pnpm-workspace.yaml for pnpm compatibility
    // (npm/yarn/bun use workspaces from package.json)
    if !manifest.packages.workspaces.is_empty() {
        generate_pnpm_workspace(manifest, &engine, force)?;
    }

    // Check if this is a Rust project (for CI workflow detection)
    let is_rust_project = !manifest.project.rust_edition.is_empty()
        || !manifest.project.binary_name.is_empty();

    // Note: Cargo.toml is NOT generated - it's the source of truth for Rust projects
    // Use `airis bump-version` to sync versions between manifest.toml and Cargo.toml

    // Generate GitHub Actions workflows if CI is enabled
    if manifest.ci.enabled {
        generate_github_workflows(manifest, &engine, force)?;
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

    // Generate .npmrc for pnpm store isolation
    generate_npmrc(&engine)?;

    // Generate .env.example if [env] section has required or optional vars
    let env_example_generated = if !manifest.env.required.is_empty() || !manifest.env.optional.is_empty() {
        generate_env_example(manifest, &engine)?;
        true
    } else {
        false
    };

    // Generate .envrc for direnv
    generate_envrc(manifest, &engine)?;

    // Generate git hooks (.husky/pre-commit, .husky/pre-push)
    generate_git_hooks(&engine)?;

    // Generate native hooks (hooks/pre-commit, hooks/pre-push) for airis hooks install
    generate_native_hooks()?;

    println!();
    println!("{}", "✅ Generated files:".green());
    println!("   - package.json (with workspaces)");
    println!("   - Dockerfile");
    println!("   - compose.yml");
    println!("   - .npmrc (pnpm store isolation)");
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
    if env_example_generated {
        println!("   - .env.example");
    }
    println!("   - .envrc");
    println!("   - .husky/pre-commit");
    println!("   - .husky/pre-push");
    println!("   - hooks/pre-commit");
    println!("   - hooks/pre-push");
    println!();
    println!("{}", "Next steps:".bright_yellow());
    println!("  1. Run `airis up` to start the workspace");
    println!("  2. Run `airis hooks install` to install Git hooks");
    println!("  3. Cache directories (.next, .swc, .turbo, node_modules) stay in Docker volumes");

    Ok(())
}

fn generate_package_json(
    manifest: &Manifest,
    engine: &TemplateEngine,
    resolved_catalog: &IndexMap<String, String>,
    _force: bool,
) -> Result<()> {
    let path = Path::new("package.json");
    let content = engine.render_package_json(manifest, resolved_catalog)?;
    write_with_backup(path, &content)?;
    println!("   {} package.json (synced from manifest.toml)", "✓".green());
    Ok(())
}

fn generate_pnpm_workspace(
    manifest: &Manifest,
    engine: &TemplateEngine,
    _force: bool,
) -> Result<()> {
    let path = Path::new("pnpm-workspace.yaml");
    let content = engine.render_pnpm_workspace(manifest)?;

    // pnpm-workspace.yaml is Tool-owned — always overwrite from manifest.toml
    write_with_backup(path, &content)?;
    if path.exists() {
        println!("   {} pnpm-workspace.yaml (synced from manifest.toml)", "✓".green());
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

fn generate_docker_compose(manifest: &Manifest, engine: &TemplateEngine, _force: bool) -> Result<()> {
    let dockerfile_content = engine.render_dockerfile(manifest)?;
    let compose_content = engine.render_docker_compose(manifest)?;

    let dockerfile_path = Path::new("Dockerfile");
    let compose_path = Path::new("compose.yml");

    write_with_backup(dockerfile_path, &dockerfile_content)?;
    println!("   {} Dockerfile (synced from manifest.toml)", "✓".green());

    write_with_backup(compose_path, &compose_content)?;
    println!("   {} compose.yml (synced from manifest.toml)", "✓".green());

    Ok(())
}

fn generate_env_example(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    let content = engine.render_env_example(manifest)?;
    let path = Path::new(".env.example");

    fs::write(path, &content)
        .with_context(|| "Failed to write .env.example")?;

    println!("   {} Generated .env.example from [env] section", "📄".green());

    Ok(())
}


fn generate_npmrc(engine: &TemplateEngine) -> Result<()> {
    let content = engine.render_npmrc()?;
    let path = Path::new(".npmrc");

    write_with_backup(path, &content)?;
    println!("   {} .npmrc (pnpm store isolation)", "✓".green());

    Ok(())
}

fn generate_envrc(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    let path = Path::new(".envrc");

    // Skip if .envrc already exists (hand-crafted version preferred)
    if path.exists() {
        println!(
            "   {} .envrc exists, skipping (hand-crafted version preferred)",
            "⏭️".cyan()
        );
        return Ok(());
    }

    let content = engine.render_envrc(manifest)?;
    fs::write(path, &content)
        .with_context(|| "Failed to write .envrc")?;
    println!("   {} Generated .envrc for direnv", "📁".green());

    Ok(())
}

fn generate_git_hooks(_engine: &TemplateEngine) -> Result<()> {
    let husky_dir = Path::new(".husky");
    fs::create_dir_all(husky_dir).context("Failed to create .husky directory")?;

    let pre_commit_content = include_str!("../../hooks/pre-commit");
    let pre_push_content = include_str!("../../hooks/pre-push");

    // Pre-commit hook
    let pre_commit_path = husky_dir.join("pre-commit");
    fs::write(&pre_commit_path, pre_commit_content)
        .with_context(|| "Failed to write .husky/pre-commit")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&pre_commit_path, fs::Permissions::from_mode(0o755))
            .with_context(|| "Failed to set .husky/pre-commit permissions")?;
    }

    // Pre-push hook
    let pre_push_path = husky_dir.join("pre-push");
    fs::write(&pre_push_path, pre_push_content)
        .with_context(|| "Failed to write .husky/pre-push")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&pre_push_path, fs::Permissions::from_mode(0o755))
            .with_context(|| "Failed to set .husky/pre-push permissions")?;
    }

    println!(
        "   {} Generated .husky/pre-commit and .husky/pre-push",
        "🔒".green()
    );

    Ok(())
}

/// Generate native hooks (hooks/pre-commit, hooks/pre-push) for `airis hooks install`.
/// Skips if the hooks/ directory already exists (preserves user customizations).
fn generate_native_hooks() -> Result<()> {
    let hooks_dir = Path::new("hooks");

    if hooks_dir.exists() {
        println!(
            "   {} hooks/ directory exists, skipping (user customizations preserved)",
            "⏭️".cyan()
        );
        return Ok(());
    }

    fs::create_dir_all(hooks_dir).context("Failed to create hooks/ directory")?;

    let pre_commit_content = include_str!("../../hooks/pre-commit");
    let pre_push_content = include_str!("../../hooks/pre-push");

    let pre_commit_path = hooks_dir.join("pre-commit");
    let pre_push_path = hooks_dir.join("pre-push");

    fs::write(&pre_commit_path, pre_commit_content)
        .with_context(|| "Failed to write hooks/pre-commit")?;
    fs::write(&pre_push_path, pre_push_content)
        .with_context(|| "Failed to write hooks/pre-push")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&pre_commit_path, fs::Permissions::from_mode(0o755))
            .with_context(|| "Failed to set hooks/pre-commit permissions")?;
        fs::set_permissions(&pre_push_path, fs::Permissions::from_mode(0o755))
            .with_context(|| "Failed to set hooks/pre-push permissions")?;
    }

    println!(
        "   {} Generated hooks/pre-commit and hooks/pre-push",
        "🔒".green()
    );
    println!(
        "   {} Run `airis hooks install` to install them to .git/hooks/",
        "💡".cyan()
    );

    Ok(())
}

// Note: generate_cargo_toml has been removed
// Cargo.toml is the source of truth for Rust projects and should not be auto-generated
// Use `airis bump-version` to sync versions between manifest.toml and Cargo.toml

fn generate_github_workflows(manifest: &Manifest, engine: &TemplateEngine, _force: bool) -> Result<()> {
    // Create .github/workflows directory
    let workflows_dir = Path::new(".github/workflows");
    fs::create_dir_all(workflows_dir).context("Failed to create .github/workflows directory")?;

    // ci.yml and release.yml are Tool-owned — always overwrite from manifest.toml
    let ci_path = workflows_dir.join("ci.yml");
    let ci_content = engine.render_ci_yml(manifest)?;
    write_with_backup(&ci_path, &ci_content)?;
    println!("   {} .github/workflows/ci.yml (synced from manifest.toml)", "✓".green());

    let release_path = workflows_dir.join("release.yml");
    let release_content = engine.render_release_yml(manifest)?;
    write_with_backup(&release_path, &release_content)?;
    println!("   {} .github/workflows/release.yml (synced from manifest.toml)", "✓".green());

    Ok(())
}

