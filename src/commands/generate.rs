use anyhow::{Context, Result};
use colored::Colorize;
use indexmap::IndexMap;
use serde_json;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::commands::discover::discover_from_workspaces;
use crate::manifest::{CatalogEntry, InjectValue, MANIFEST_FILE, Manifest, ProjectDefinition};
use crate::ownership::{Ownership, get_ownership};
use crate::templates::TemplateEngine;
use crate::version_resolver::resolve_version;

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
fn write_with_backup(path: &Path, content: &str) -> Result<()> {
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
    let mut inject_count = 0;

    // Load previous generation registry for orphan detection
    let registry_path = Path::new(".airis/generated.toml");
    let previous_paths: Vec<String> = load_generation_registry(registry_path);

    // Node.js workspace files (only when [workspace] package_manager is set)
    if has_workspace {
        let mut resolved_catalog = resolve_catalog_versions(
            &manifest.packages.catalog,
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
                    let mut auto_app = ProjectDefinition {
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

        // Generate production Dockerfiles for services with [app.deploy] enabled
        generate_service_dockerfiles(manifest, &engine)?;
        let deploy_apps: Vec<_> = manifest
            .app
            .iter()
            .filter(|a| a.deploy.as_ref().is_some_and(|d| d.enabled))
            .collect();
        if !deploy_apps.is_empty() {
            for app in &deploy_apps {
                if let Some(ref path) = app.path {
                    generated_paths.push(format!("{}/Dockerfile", path));
                }
            }
            generated_files.push(format!(
                "{} service Dockerfiles (turbo prune)",
                deploy_apps.len()
            ));
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

        // Inject values into files with airis:inject markers
        inject_count = inject_values(&manifest.inject, &resolved_catalog)?;

        // Sync pnpm-lock.yaml
        sync_lockfile(manifest)?;
    } else if !manifest.inject.is_empty() {
        // Inject values even without workspace
        let resolved_catalog = IndexMap::new();
        inject_count = inject_values(&manifest.inject, &resolved_catalog)?;
    }

    // CI/CD workflow generation
    if manifest.ci.enabled {
        println!();
        println!("{}", "🚀 Generating CI/CD workflows...".bright_blue());
        if !manifest.deploy_profiles().is_empty() {
            generate_ci_workflow(manifest, &engine)?;
            generated_paths.push(".github/workflows/ci.yml".into());
            generate_deploy_workflow(manifest, &engine)?;
            generated_paths.push(".github/workflows/deploy.yml".into());
            if manifest.ci.e2e.enabled {
                generate_e2e_workflow(manifest, &engine)?;
                generated_paths.push(".github/workflows/e2e-staging.yml".into());
            }
        }
        generate_release_workflow(manifest, &engine)?;
        generated_paths.push(".github/workflows/release.yml".into());
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
    if inject_count > 0 {
        println!(
            "   - {} files updated via airis:inject markers",
            inject_count
        );
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

fn generate_package_json(
    manifest: &Manifest,
    engine: &TemplateEngine,
    resolved_catalog: &IndexMap<String, String>,
    _force: bool,
) -> Result<()> {
    let path = Path::new("package.json");
    let content = engine.render_package_json(manifest, resolved_catalog)?;
    write_with_backup(path, &content)?;
    println!(
        "   {} package.json (synced from manifest.toml)",
        "✓".green()
    );
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
        println!(
            "   {} pnpm-workspace.yaml (synced from manifest.toml)",
            "✓".green()
        );
    }
    Ok(())
}

/// Resolve catalog version policies to actual version numbers.
///
/// Supports wildcard patterns like `@radix-ui/react-* = "latest"`.
/// Wildcard entries are stored as patterns and resolved on-demand
/// when a concrete package name matches via `resolve_wildcard_version`.
fn resolve_catalog_versions(
    catalog: &IndexMap<String, CatalogEntry>,
    default_policy: Option<&str>,
) -> Result<IndexMap<String, String>> {
    if catalog.is_empty() {
        return Ok(IndexMap::new());
    }

    println!(
        "{}",
        "📦 Resolving catalog versions from npm registry...".bright_blue()
    );

    let mut resolved: IndexMap<String, String> = IndexMap::new();

    for (package, entry) in catalog {
        // Skip wildcard patterns — they are resolved on-demand
        if package.contains('*') {
            let policy_str = match entry {
                CatalogEntry::Policy(p) => p.as_str().to_string(),
                CatalogEntry::Empty(_) => "latest".to_string(),
                CatalogEntry::Version(v) => v.clone(),
                _ => "latest".to_string(),
            };
            println!("  ✓ {} (wildcard pattern, policy: {})", package, policy_str);
            continue;
        }

        let version = match entry {
            CatalogEntry::Policy(policy) => {
                let policy_str = policy.as_str();
                let version = resolve_version(package, policy_str)?;
                println!("  ✓ {} {} → {}", package, policy_str, version);
                version
            }
            CatalogEntry::Empty(_) => {
                // Empty table {} = latest
                let version = resolve_version(package, "latest")?;
                println!("  ✓ {} (default) → {}", package, version);
                version
            }
            CatalogEntry::Version(version) => {
                println!("  ✓ {} {}", package, version);
                version.clone()
            }
            CatalogEntry::Follow(follow_config) => {
                let target = &follow_config.follow;
                if let Some(target_version) = resolved.get(target) {
                    println!("  ✓ {} (follow {}) → {}", package, target, target_version);
                    target_version.clone()
                } else if let Some(policy) = default_policy {
                    // Follow target not in catalog — resolve it via default_policy first
                    let target_version = resolve_version(target, policy)?;
                    println!("  ✓ {} {} → {}", target, policy, target_version);
                    resolved.insert(target.clone(), target_version.clone());
                    println!("  ✓ {} (follow {}) → {}", package, target, target_version);
                    target_version
                } else {
                    anyhow::bail!(
                        "Cannot resolve '{}': follow target '{}' not found in catalog (add it or set default_policy)",
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

fn generate_docker_compose(
    manifest: &Manifest,
    engine: &TemplateEngine,
    _force: bool,
) -> Result<()> {
    let compose_content = engine.render_docker_compose(manifest)?;

    let compose_path = Path::new("compose.yml");

    write_with_backup(compose_path, &compose_content)?;
    println!("   {} compose.yml (synced from manifest.toml)", "✓".green());

    Ok(())
}

fn generate_env_example(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    let content = engine.render_env_example(manifest)?;
    let path = Path::new(".env.example");

    fs::write(path, &content).with_context(|| "Failed to write .env.example")?;

    println!(
        "   {} Generated .env.example from [env] section",
        "📄".green()
    );

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
    fs::write(path, &content).with_context(|| "Failed to write .envrc")?;
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

fn generate_ci_workflow(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    let workflows_dir = Path::new(".github/workflows");
    fs::create_dir_all(workflows_dir).context("Failed to create .github/workflows directory")?;

    let ci_path = workflows_dir.join("ci.yml");

    // Skip if file exists without airis gen marker (hand-managed)
    if ci_path.exists() {
        let existing = fs::read_to_string(&ci_path).unwrap_or_default();
        if !existing.starts_with("# Auto-generated by airis gen") {
            println!(
                "   {} .github/workflows/ci.yml exists (manually managed — add '# Auto-generated by airis gen' header to enable)",
                "⚠".yellow()
            );
            return Ok(());
        }
    }

    let content = engine.render_ci_workflow(manifest)?;
    write_with_backup(&ci_path, &content)?;
    println!("   {} .github/workflows/ci.yml", "✓".green());
    Ok(())
}

fn generate_deploy_workflow(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    let workflows_dir = Path::new(".github/workflows");
    fs::create_dir_all(workflows_dir).context("Failed to create .github/workflows directory")?;

    let deploy_path = workflows_dir.join("deploy.yml");

    // Skip if file exists without airis gen marker (hand-managed)
    if deploy_path.exists() {
        let existing = fs::read_to_string(&deploy_path).unwrap_or_default();
        if !existing.starts_with("# Auto-generated by airis gen") {
            println!(
                "   {} .github/workflows/deploy.yml exists (manually managed — add '# Auto-generated by airis gen' header to enable)",
                "⚠".yellow()
            );
            return Ok(());
        }
    }

    let content = engine.render_deploy_workflow(manifest)?;
    write_with_backup(&deploy_path, &content)?;
    println!("   {} .github/workflows/deploy.yml", "✓".green());
    Ok(())
}

fn generate_e2e_workflow(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    let workflows_dir = Path::new(".github/workflows");
    fs::create_dir_all(workflows_dir).context("Failed to create .github/workflows directory")?;

    let path = workflows_dir.join("e2e-staging.yml");

    // Skip if file exists without airis gen marker (hand-managed)
    if path.exists() {
        let existing = fs::read_to_string(&path).unwrap_or_default();
        if !existing.starts_with("# Auto-generated by airis gen") {
            println!(
                "   {} .github/workflows/e2e-staging.yml exists (manually managed — add '# Auto-generated by airis gen' header to enable)",
                "⚠".yellow()
            );
            return Ok(());
        }
    }

    let content = engine.render_e2e_workflow(manifest)?;
    write_with_backup(&path, &content)?;
    println!("   {} .github/workflows/e2e-staging.yml", "✓".green());
    Ok(())
}

fn generate_release_workflow(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    let workflows_dir = Path::new(".github/workflows");
    fs::create_dir_all(workflows_dir).context("Failed to create .github/workflows directory")?;

    let path = workflows_dir.join("release.yml");

    // Skip if file exists without airis gen marker (hand-managed)
    if path.exists() {
        let existing = fs::read_to_string(&path).unwrap_or_default();
        if !existing.starts_with("# Auto-generated by airis gen") {
            println!(
                "   {} .github/workflows/release.yml exists (manually managed — add '# Auto-generated by airis gen' header to enable)",
                "⚠".yellow()
            );
            return Ok(());
        }
    }

    let content = engine.render_release_workflow(manifest)?;
    write_with_backup(&path, &content)?;
    println!("   {} .github/workflows/release.yml", "✓".green());
    Ok(())
}

fn generate_service_dockerfiles(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    // Extract pnpm version from package_manager field (e.g., "pnpm@10.30.3" → "10.30.3")
    let pnpm_version = manifest
        .workspace
        .package_manager
        .split('@')
        .nth(1)
        .unwrap_or("latest");

    let deployable_apps: Vec<_> = manifest
        .app
        .iter()
        .filter(|a| a.deploy.as_ref().is_some_and(|d| d.enabled))
        .collect();

    if deployable_apps.is_empty() {
        return Ok(());
    }

    println!();
    println!(
        "{}",
        "🐳 Generating service Dockerfiles (turbo prune)...".bright_blue()
    );

    for app in &deployable_apps {
        let app_path = app.path.as_deref().unwrap_or(&app.name);
        let dockerfile_path = Path::new(app_path).join("Dockerfile");

        // Ensure directory exists
        if let Some(parent) = dockerfile_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {}", parent.display()))?;
        }

        let content = engine.render_service_dockerfile(app, pnpm_version)?;
        write_with_backup(&dockerfile_path, &content)?;

        let variant = app
            .deploy
            .as_ref()
            .and_then(|d| d.variant.as_deref())
            .unwrap_or(match app.framework.as_deref() {
                Some("nextjs") => "nextjs",
                _ => "node",
            });

        println!(
            "   {} {}/Dockerfile (variant: {})",
            "✓".green(),
            app_path,
            variant,
        );
    }

    Ok(())
}

// ── Lockfile synchronization ──────────────────────────────────

/// Sync pnpm-lock.yaml after package.json updates.
/// Uses `--lockfile-only` to avoid installing into node_modules (fast).
/// Runs via Docker if mode is docker-first, otherwise directly.
///
/// In docker-first mode: tries `docker compose exec` first (fast, uses running container).
/// If the container is not running, falls back to `docker compose run --rm` (starts a
/// temporary container, slower but always works without requiring `airis up` first).
fn sync_lockfile(manifest: &Manifest) -> Result<()> {
    use std::process::Command;

    // Only sync if pnpm-lock.yaml exists (skip for fresh projects)
    if !Path::new("pnpm-lock.yaml").exists() {
        return Ok(());
    }

    println!();
    println!("{}", "🔒 Syncing pnpm-lock.yaml...".bright_blue());

    let is_docker_first = matches!(manifest.mode, crate::manifest::Mode::DockerFirst);

    // Find a service to use
    let docker_service = manifest
        .docker
        .workspace
        .as_ref()
        .map(|w| w.service.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| manifest.service.keys().next().map(|s| s.as_str()));

    let status = if is_docker_first {
        let svc = match docker_service {
            Some(s) => s,
            None => {
                println!(
                    "   {} no service found for lockfile sync (run `docker compose exec <service> pnpm install --lockfile-only`)",
                    "⚠".yellow()
                );
                return Ok(());
            }
        };

        // Try exec first (fast, uses running container)
        let exec_status = Command::new("docker")
            .args([
                "compose",
                "exec",
                "-T",
                svc,
                "pnpm",
                "install",
                "--lockfile-only",
            ])
            .status();

        match exec_status {
            Ok(s) if s.success() => Ok(s),
            _ => {
                // Container not running — use doppler + docker compose run to inject env vars
                println!(
                    "   {} container not running, trying with doppler...",
                    "↻".yellow()
                );
                let doppler_status = Command::new("doppler")
                    .args([
                        "run",
                        "--",
                        "docker",
                        "compose",
                        "run",
                        "--rm",
                        "--no-deps",
                        "-T",
                        svc,
                        "pnpm",
                        "install",
                        "--lockfile-only",
                    ])
                    .status();

                match doppler_status {
                    Ok(s) if s.success() => Ok(s),
                    _ => {
                        // Doppler not available — use lightweight docker run with base image
                        println!(
                            "   {} doppler unavailable, using docker run...",
                            "↻".yellow()
                        );
                        let pm = &manifest.workspace.package_manager;
                        let image = &manifest.workspace.image;
                        Command::new("docker")
                            .args([
                                "run",
                                "--rm",
                                "-v",
                                &format!("{}:/app", std::env::current_dir()?.display()),
                                "-w",
                                "/app",
                                image,
                                "sh",
                                "-c",
                                &format!("npm install -g {} && pnpm install --lockfile-only", pm),
                            ])
                            .status()
                    }
                }
            }
        }
    } else {
        // Non-docker mode: run directly on host
        Command::new("pnpm")
            .args(["install", "--lockfile-only"])
            .status()
    };

    match status {
        Ok(s) if s.success() => {
            println!("   {} pnpm-lock.yaml synced", "✓".green());
        }
        Ok(_) => {
            println!(
                "   {} pnpm-lock.yaml sync failed (run `docker compose run --rm <service> pnpm install --lockfile-only`)",
                "⚠".yellow()
            );
        }
        Err(e) => {
            println!("   {} pnpm-lock.yaml sync skipped: {}", "⚠".yellow(), e);
        }
    }

    Ok(())
}

// ── Value injection into user-owned files ─────────────────────

/// Scan workspace files for `# airis:inject <key>` markers and replace
/// the next line with the resolved value from `[inject]` in manifest.toml.
///
/// Returns the number of files modified.
fn inject_values(
    inject: &IndexMap<String, InjectValue>,
    resolved_catalog: &IndexMap<String, String>,
) -> Result<usize> {
    if inject.is_empty() {
        return Ok(0);
    }

    println!();
    println!(
        "{}",
        "💉 Injecting values from manifest [inject]...".bright_blue()
    );

    // Resolve all inject values to plain strings
    let values = resolve_inject_values(inject, resolved_catalog)?;

    let marker_re = regex::Regex::new(r"#\s*airis:inject\s+([\w.\-]+)")
        .context("Failed to compile inject marker regex")?;

    let mut modified_count = 0;

    // Walk workspace respecting .gitignore
    let walker = walkdir::WalkDir::new(".")
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            // Skip .git directory, node_modules, dist, .next, .turbo
            !matches!(
                name,
                ".git"
                    | "node_modules"
                    | "dist"
                    | ".next"
                    | ".turbo"
                    | ".pnpm"
                    | ".cache"
                    | "coverage"
            )
        });

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();

        // Skip binary files and known non-text
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if matches!(
            ext,
            "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "ico"
                | "woff"
                | "woff2"
                | "ttf"
                | "eot"
                | "lock"
                | "zst"
                | "tar"
                | "gz"
        ) {
            continue;
        }

        // Quick check: skip files that don't contain our marker
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if !content.contains("airis:inject") {
            continue;
        }

        let mut lines: Vec<String> = content.lines().map(String::from).collect();
        let mut file_modified = false;

        let mut i = 0;
        while i < lines.len().saturating_sub(1) {
            if let Some(caps) = marker_re.captures(&lines[i]) {
                let key = &caps[1];
                if let Some(value) = values.get(key) {
                    // Preserve indentation of the target line
                    let target = &lines[i + 1];
                    let indent_len = target.len() - target.trim_start().len();
                    let indent: String = target.chars().take(indent_len).collect();
                    let new_line = format!("{indent}{value}");

                    if lines[i + 1] != new_line {
                        lines[i + 1] = new_line;
                        file_modified = true;
                    }
                } else {
                    println!(
                        "   {} marker '{}' in {} has no matching [inject] key",
                        "⚠".yellow(),
                        key,
                        path.display()
                    );
                }
            }
            i += 1;
        }

        if file_modified {
            backup_file(path)?;
            fs::write(path, lines.join("\n") + "\n").with_context(|| {
                format!("Failed to write injected values to {}", path.display())
            })?;
            println!("   {} {}", "→".green(), path.display());
            modified_count += 1;
        }
    }

    if modified_count == 0 {
        println!(
            "   {} All injected values are already up to date",
            "✓".green()
        );
    }

    Ok(modified_count)
}

/// Resolve inject values: simple strings pass through, template values
/// have `{version}` replaced with the resolved catalog version.
fn resolve_inject_values(
    inject: &IndexMap<String, InjectValue>,
    resolved_catalog: &IndexMap<String, String>,
) -> Result<IndexMap<String, String>> {
    let mut values = IndexMap::new();

    for (key, val) in inject {
        let resolved = match val {
            InjectValue::Simple(s) => s.clone(),
            InjectValue::Template {
                template,
                from_catalog,
            } => {
                let version = resolved_catalog
                    .get(from_catalog.as_str())
                    .cloned()
                    .unwrap_or_else(|| {
                        let stripped = from_catalog.trim_start_matches('@');
                        resolved_catalog.get(stripped).cloned().unwrap_or_default()
                    });
                if version.is_empty() {
                    println!(
                        "   {} inject key '{}': catalog entry '{}' not found, skipping",
                        "⚠".yellow(),
                        key,
                        from_catalog
                    );
                    continue;
                }
                let clean_version = version.trim_start_matches('^').trim_start_matches('~');
                template.replace("{version}", clean_version)
            }
        };
        values.insert(key.clone(), resolved);
    }

    Ok(values)
}

// ── TypeScript tsconfig generation ───────────────────────────

fn generate_tsconfig(
    manifest: &Manifest,
    engine: &TemplateEngine,
    resolved_catalog: &IndexMap<String, String>,
) -> Result<()> {
    println!();
    println!("{}", "📝 Generating tsconfig files...".bright_blue());

    let ts_major = detect_ts_major(manifest, resolved_catalog);

    // 1. tsconfig.base.json — shared compilerOptions
    let base_content = engine.render_tsconfig_base(manifest)?;
    let base_path = Path::new("tsconfig.base.json");
    write_with_backup(base_path, &base_content)?;
    println!(
        "   {} tsconfig.base.json (shared compilerOptions)",
        "✓".green()
    );

    // 2. Collect workspace paths for IDE path aliases
    let workspace_root = env::current_dir().context("Failed to get current directory")?;
    let workspace_patterns = if !manifest.packages.workspaces.is_empty() {
        &manifest.packages.workspaces
    } else {
        &manifest.workspace.workspaces
    };

    let mut path_entries: Vec<(String, String)> = Vec::new();
    if !workspace_patterns.is_empty() {
        let discovered = discover_from_workspaces(workspace_patterns, &workspace_root)?;
        for disc in &discovered {
            // Skip node_modules and build artifacts
            if disc.path.contains("node_modules")
                || disc.path.contains(".next")
                || disc.path.contains("/dist/")
            {
                continue;
            }
            let pkg_json_path = workspace_root.join(&disc.path).join("package.json");
            if let Ok(content) = fs::read_to_string(&pkg_json_path)
                && let Ok(json) = serde_json::from_str::<serde_json::Value>(&content)
                && let Some(name) = json.get("name").and_then(|n| n.as_str())
            {
                path_entries.push((name.to_string(), disc.path.clone()));
            }
        }
    }

    // 3. tsconfig.json — IDE config with paths
    let root_content = engine.render_tsconfig_root(manifest, &path_entries, ts_major)?;
    let root_path = Path::new("tsconfig.json");
    write_with_backup(root_path, &root_content)?;

    if ts_major >= 6 {
        println!(
            "   {} tsconfig.json (IDE, {} paths, TS{} — ignoreDeprecations: \"6.0\")",
            "✓".green(),
            path_entries.len(),
            ts_major,
        );
    } else {
        println!(
            "   {} tsconfig.json (IDE, {} paths, TS{})",
            "✓".green(),
            path_entries.len(),
            ts_major,
        );
    }

    // 4. Per-package tsconfig.json files
    if manifest.typescript.generate_per_package {
        let mut pkg_count = 0;
        let mut css_count = 0;
        for app in &manifest.app {
            // Skip rust packages
            if app.framework.as_deref() == Some("rust") {
                continue;
            }

            let pkg_path = if let Some(ref p) = app.path {
                PathBuf::from(p)
            } else {
                // Auto-detect path from workspace discovery
                let matched = path_entries.iter().find(|(name, _)| {
                    let scoped = format!(
                        "@{}/{}",
                        manifest
                            .workspace
                            .scope
                            .as_deref()
                            .unwrap_or(&manifest.workspace.name),
                        app.name
                    );
                    name == &scoped || name == &app.name
                });
                if let Some((_, path)) = matched {
                    PathBuf::from(path)
                } else {
                    continue;
                }
            };

            // Calculate relative path to root
            let depth = pkg_path.components().count();
            let rel_to_root = "../".repeat(depth);

            let pkg_tsconfig =
                engine.render_package_tsconfig(app, manifest, &rel_to_root, ts_major)?;
            let tsconfig_path = pkg_path.join("tsconfig.json");
            write_with_backup(&tsconfig_path, &pkg_tsconfig)?;
            pkg_count += 1;

            // Generate css.d.ts for Next.js apps (TS6 TS2882 fix)
            if app.framework.as_deref() == Some("nextjs") {
                let css_decl = engine.render_css_declaration();
                let src_dir = pkg_path.join("src");
                if src_dir.exists() {
                    let css_path = src_dir.join("css.d.ts");
                    write_with_backup(&css_path, &css_decl)?;
                    css_count += 1;
                }
            }
        }
        if pkg_count > 0 {
            print!(
                "   {} {} package tsconfig.json files",
                "✓".green(),
                pkg_count
            );
            if css_count > 0 {
                print!(" + {} css.d.ts", css_count);
            }
            println!();
        }
    }

    Ok(())
}

/// Detect TypeScript major version from manifest or resolved catalog.
fn detect_ts_major(manifest: &Manifest, resolved_catalog: &IndexMap<String, String>) -> u32 {
    // Explicit override in [typescript]
    if let Some(v) = manifest.typescript.version {
        return v;
    }

    // Auto-detect from resolved catalog
    if let Some(version_str) = resolved_catalog.get("typescript") {
        let clean = version_str.trim_start_matches('^').trim_start_matches('~');
        if let Some(major_str) = clean.split('.').next()
            && let Ok(major) = major_str.parse::<u32>()
        {
            return major;
        }
    }

    // Default: assume TS5 (safe, no ignoreDeprecations)
    5
}

/// Resolve package data for full-gen mode.
///
/// Combines: convention scripts + preset + dep_group + import scan → final deps/scripts
fn resolve_package_data(
    app: &ProjectDefinition,
    workspace_root: &Path,
    workspace_scope: &str,
    resolved_catalog: &mut IndexMap<String, String>,
    catalog_raw: &IndexMap<String, CatalogEntry>,
    presets: &IndexMap<String, crate::manifest::PresetSection>,
    dep_groups: &IndexMap<String, IndexMap<String, String>>,
    default_policy: Option<&str>,
) -> Result<crate::generators::package_json::ResolvedPackageData> {
    let mut final_deps = IndexMap::new();
    let mut final_dev_deps = IndexMap::new();
    let mut final_scripts = IndexMap::new();

    // 1. Convention defaults from framework
    let framework = app.framework.as_deref().unwrap_or("node");
    let conventions = crate::conventions::framework_defaults(framework);
    for (k, v) in conventions.default_scripts {
        final_scripts.insert(k.to_string(), v.to_string());
    }

    // 2. Preset resolution (includes dep_groups from preset)
    if app.preset.is_some() || !app.dep_groups.is_empty() {
        let resolved = crate::preset::resolve_app_presets(app, presets, dep_groups)?;
        for (k, v) in &resolved.deps {
            // Resolve "catalog" references: if not in resolved_catalog, use default_policy
            if v == "catalog" && !resolved_catalog.contains_key(k) {
                let policy = default_policy.unwrap_or("latest");
                match resolve_version(k, policy) {
                    Ok(version) => {
                        println!("  ✓ {} (dep_group, default: {}) → {}", k, policy, version);
                        resolved_catalog.insert(k.clone(), version);
                    }
                    Err(e) => {
                        eprintln!("  ⚠ Failed to resolve {}: {}", k, e);
                    }
                }
            }
            final_deps.insert(k.clone(), v.clone());
        }
        for (k, v) in &resolved.dev_deps {
            if v == "catalog" && !resolved_catalog.contains_key(k) {
                let policy = default_policy.unwrap_or("latest");
                match resolve_version(k, policy) {
                    Ok(version) => {
                        println!(
                            "  ✓ {} (dep_group dev, default: {}) → {}",
                            k, policy, version
                        );
                        resolved_catalog.insert(k.clone(), version);
                    }
                    Err(e) => {
                        eprintln!("  ⚠ Failed to resolve {}: {}", k, e);
                    }
                }
            }
            final_dev_deps.insert(k.clone(), v.clone());
        }
        for (k, v) in &resolved.scripts {
            final_scripts.insert(k.clone(), v.clone());
        }
    }

    // Collect wildcard patterns from catalog for matching
    let wildcard_patterns: Vec<(&str, &CatalogEntry)> = catalog_raw
        .iter()
        .filter(|(k, _)| k.contains('*'))
        .map(|(k, v)| (k.as_str(), v))
        .collect();

    // 3. Import scan (auto-detect deps from source code)
    if let Some(ref app_path) = app.path {
        let full_path = workspace_root.join(app_path);
        if full_path.exists() {
            match crate::import_scanner::scan_imports(&full_path, workspace_scope) {
                Ok(scanned) => {
                    // External deps: use catalog version if available, or match wildcard
                    for pkg in &scanned.external {
                        if !final_deps.contains_key(pkg) {
                            if resolved_catalog.contains_key(pkg) {
                                final_deps.insert(pkg.clone(), "catalog".to_string());
                            } else if matches_wildcard_catalog(pkg, &wildcard_patterns) {
                                // Wildcard match: resolve version from npm and add to catalog
                                match resolve_version(pkg, "latest") {
                                    Ok(version) => {
                                        println!("  ✓ {} (wildcard) → {}", pkg, version);
                                        resolved_catalog.insert(pkg.clone(), version);
                                        final_deps.insert(pkg.clone(), "catalog".to_string());
                                    }
                                    Err(e) => {
                                        eprintln!("  ⚠ Failed to resolve {}: {}", pkg, e);
                                    }
                                }
                            } else if let Some(policy) = default_policy {
                                // Default policy fallback: resolve from npm
                                match resolve_version(pkg, policy) {
                                    Ok(version) => {
                                        println!("  ✓ {} (default: {}) → {}", pkg, policy, version);
                                        resolved_catalog.insert(pkg.clone(), version);
                                        final_deps.insert(pkg.clone(), "catalog".to_string());
                                    }
                                    Err(e) => {
                                        eprintln!("  ⚠ Failed to resolve {}: {}", pkg, e);
                                    }
                                }
                            }
                            // Not in catalog, no wildcard, no default policy → skip
                        }
                    }
                    // Workspace deps (skip self-reference)
                    let self_pkg_name = if let Some(ref scope) = app.scope {
                        let scope = scope.trim_start_matches('@');
                        format!("@{}/{}", scope, app.name)
                    } else {
                        format!("{}/{}", workspace_scope, app.name)
                    };
                    for pkg in &scanned.workspace {
                        if pkg == &self_pkg_name {
                            continue; // Skip self-reference
                        }
                        if !final_deps.contains_key(pkg) {
                            final_deps.insert(pkg.clone(), "workspace:*".to_string());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("  ⚠ Import scan failed for {}: {}", app_path, e);
                }
            }
        }
    }

    // 4. Explicit deps from [[app]] override everything
    for (k, v) in &app.deps {
        final_deps.insert(k.clone(), v.clone());
    }
    for (k, v) in &app.dev_deps {
        final_dev_deps.insert(k.clone(), v.clone());
    }
    // Explicit scripts from [[app]] override convention + preset
    for (k, v) in &app.scripts {
        final_scripts.insert(k.clone(), v.clone());
    }

    Ok(crate::generators::package_json::ResolvedPackageData {
        deps: final_deps,
        dev_deps: final_dev_deps,
        scripts: final_scripts,
    })
}

/// Check if a package name matches any wildcard pattern in the catalog.
/// Supports simple glob patterns like `@radix-ui/react-*`.
fn matches_wildcard_catalog(package: &str, wildcards: &[(&str, &CatalogEntry)]) -> bool {
    for (pattern, _) in wildcards {
        if wildcard_matches(pattern, package) {
            return true;
        }
    }
    false
}

/// Simple wildcard matching: `*` matches any sequence of characters.
/// Only supports `*` at the end of a pattern (prefix match).
fn wildcard_matches(pattern: &str, name: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        name.starts_with(prefix)
    } else {
        pattern == name
    }
}

// =============================================================================
// Generation registry — tracks generated files for orphan detection
// =============================================================================

/// Load the list of previously generated files from .airis/generated.toml
fn load_generation_registry(path: &Path) -> Vec<String> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    // Simple line-based format: one path per line (skip comments and empty lines)
    content
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect()
}

/// Save the current list of generated files to .airis/generated.toml
fn save_generation_registry(path: &Path, paths: &[String]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut sorted = paths.to_vec();
    sorted.sort();
    sorted.dedup();
    let content = format!(
        "# Auto-managed by airis gen — do not edit\n# Lists all files generated from manifest.toml\n{}\n",
        sorted.join("\n")
    );
    fs::write(path, content).context("Failed to write generation registry")?;
    Ok(())
}
