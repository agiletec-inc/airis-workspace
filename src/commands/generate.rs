use anyhow::{Context, Result};
use colored::Colorize;
use dialoguer::Confirm;
use indexmap::IndexMap;
use std::env;
use std::fs;
use std::io::IsTerminal;
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
///
/// If `force` is true, overwrites existing files directly (used by `doctor --fix`).
/// If `force` is false, writes to `.md` files for comparison (safe default for `generate files`).
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

    // Generate .env.example if [env] section has required or optional vars
    let env_example_generated = if !manifest.env.required.is_empty() || !manifest.env.optional.is_empty() {
        generate_env_example(manifest, &engine)?;
        true
    } else {
        false
    };

    // Generate LLM context for AI assistants
    generate_llm_context(manifest, &engine)?;

    // Generate CLAUDE.md for Claude Code
    generate_claude_md(manifest, &engine)?;

    // Generate .envrc for direnv
    generate_envrc(manifest, &engine)?;

    println!();
    println!("{}", "✅ Generated files:".green());
    println!("   - package.json (with workspaces)");
    println!("   - Dockerfile.dev");
    println!("   - compose.yml");
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
    println!("   - .workspace/llm-context.md");
    println!("   - CLAUDE.md");
    println!("   - .envrc");
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
    force: bool,
) -> Result<()> {
    let path = Path::new("package.json");
    let content = engine.render_package_json(manifest, resolved_catalog)?;

    if path.exists() && !force {
        // Don't overwrite existing package.json - write to .md for comparison
        let md_path = Path::new("package.json.md");
        fs::write(md_path, &content)
            .with_context(|| "Failed to write package.json.md")?;
        println!(
            "   {} package.json exists → wrote package.json.md for comparison",
            "📄".yellow()
        );
    } else {
        write_with_backup(path, &content)?;
        if force {
            println!("   {} package.json (overwritten)", "✓".green());
        }
    }
    Ok(())
}

fn generate_pnpm_workspace(
    manifest: &Manifest,
    engine: &TemplateEngine,
    force: bool,
) -> Result<()> {
    let path = Path::new("pnpm-workspace.yaml");
    let content = engine.render_pnpm_workspace(manifest)?;

    if path.exists() && !force {
        // Don't overwrite existing pnpm-workspace.yaml - write to .md for comparison
        let md_path = Path::new("pnpm-workspace.yaml.md");
        fs::write(md_path, &content)
            .with_context(|| "Failed to write pnpm-workspace.yaml.md")?;
        println!(
            "   {} pnpm-workspace.yaml exists → wrote pnpm-workspace.yaml.md for comparison",
            "📄".yellow()
        );
    } else {
        write_with_backup(path, &content)?;
        if force {
            println!("   {} pnpm-workspace.yaml (overwritten)", "✓".green());
        }
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

fn generate_docker_compose(manifest: &Manifest, engine: &TemplateEngine, force: bool) -> Result<()> {
    let dockerfile_content = engine.render_dockerfile_dev(manifest)?;
    let compose_content = engine.render_docker_compose(manifest)?;

    let dockerfile_path = Path::new("Dockerfile");
    // Use modern naming (compose.yml), but check for legacy naming too
    let compose_path = Path::new("compose.yml");
    let legacy_compose_path = Path::new("docker-compose.yml");
    let compose_exists = compose_path.exists() || legacy_compose_path.exists();

    if dockerfile_path.exists() && !force {
        // Write to .md for comparison (safe default)
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
        if force {
            println!("   {} Dockerfile (overwritten)", "✓".green());
        }
    }

    if compose_exists && !force {
        // Write to .md for comparison (safe default)
        let md_path = Path::new("compose.yml.md");
        fs::write(md_path, &compose_content)
            .with_context(|| "Failed to write compose.yml.md")?;
        println!(
            "   {} compose.yml exists → wrote compose.yml.md for comparison",
            "📄".yellow()
        );
    } else {
        fs::write(compose_path, &compose_content)
            .with_context(|| "Failed to write compose.yml")?;
        if force {
            println!("   {} compose.yml (overwritten)", "✓".green());
        }
    }

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

fn generate_llm_context(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    let content = engine.render_llm_context(manifest)?;

    // Create .workspace directory if needed
    let workspace_dir = Path::new(".workspace");
    fs::create_dir_all(workspace_dir)
        .context("Failed to create .workspace directory")?;

    let path = workspace_dir.join("llm-context.md");
    fs::write(&path, &content)
        .with_context(|| "Failed to write .workspace/llm-context.md")?;

    println!("   {} Generated .workspace/llm-context.md for AI assistants", "🤖".green());

    Ok(())
}

fn generate_claude_md(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    let path = Path::new("CLAUDE.md");

    if path.exists() {
        // Analyze existing file for airis-specific sections
        let existing_content = fs::read_to_string(path)
            .with_context(|| "Failed to read CLAUDE.md")?;
        let existing_lower = existing_content.to_lowercase();

        let mut missing_sections = Vec::new();

        // Check for Docker First section
        // Look for: "docker first" phrase, or combination of airis commands
        let has_docker_first = existing_lower.contains("docker first")
            || existing_lower.contains("docker-first")
            || (existing_lower.contains("airis up") && existing_lower.contains("airis shell"));
        if !has_docker_first {
            missing_sections.push("docker_first");
        }

        // Check for Available Commands section
        // Look for: "available commands", "airis commands", or command table format
        let has_commands = existing_lower.contains("available commands")
            || existing_lower.contains("airis commands")
            || (existing_lower.contains("airis") && existing_lower.contains("| command |"));
        if !has_commands {
            missing_sections.push("commands");
        }

        if missing_sections.is_empty() {
            println!(
                "   {} CLAUDE.md exists with airis sections",
                "✅".green()
            );
            return Ok(());
        }

        // Show missing sections
        println!();
        println!(
            "{}",
            "⚠️  CLAUDE.md に以下のセクションが不足しています:".yellow()
        );
        for section in &missing_sections {
            let name = match *section {
                "docker_first" => "Docker First Development",
                "commands" => "Available Commands",
                _ => section,
            };
            println!("   - {}", name);
        }
        println!();

        // Check if running interactively (tty available)
        let is_interactive = std::io::stdin().is_terminal();

        let should_append = if is_interactive {
            // Interactive: ask user
            Confirm::new()
                .with_prompt("これらのセクションを CLAUDE.md に追記しますか？")
                .default(true)
                .interact()
                .unwrap_or(true)
        } else {
            // Non-interactive (Claude Code / CI): auto-append
            true
        };

        if should_append {
            // Generate content for missing sections
            let additional_content = engine.render_claude_md_sections(manifest, &missing_sections)?;

            // Append to existing file
            let mut new_content = existing_content;
            if !new_content.ends_with('\n') {
                new_content.push('\n');
            }
            new_content.push_str("\n---\n\n");
            new_content.push_str("<!-- Added by airis generate -->\n\n");
            new_content.push_str(&additional_content);

            fs::write(path, &new_content)
                .with_context(|| "Failed to append to CLAUDE.md")?;

            let mode_suffix = if is_interactive {
                ""
            } else {
                " (non-interactive mode)"
            };
            println!(
                "   {} CLAUDE.md に {} セクションを追記しました{}",
                "✅".green(),
                missing_sections.len(),
                mode_suffix
            );
        } else {
            println!(
                "   {} CLAUDE.md への追記をスキップしました",
                "⏭️".cyan()
            );
        }

        return Ok(());
    }

    // New file generation
    let content = engine.render_claude_md(manifest)?;
    fs::write(path, &content)
        .with_context(|| "Failed to write CLAUDE.md")?;
    println!("   {} Generated CLAUDE.md for Claude Code", "🤖".green());

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

// Note: generate_cargo_toml has been removed
// Cargo.toml is the source of truth for Rust projects and should not be auto-generated
// Use `airis bump-version` to sync versions between manifest.toml and Cargo.toml

fn generate_github_workflows(manifest: &Manifest, engine: &TemplateEngine, force: bool) -> Result<()> {
    // Create .github/workflows directory
    let workflows_dir = Path::new(".github/workflows");
    fs::create_dir_all(workflows_dir).context("Failed to create .github/workflows directory")?;

    // Generate ci.yml
    let ci_path = workflows_dir.join("ci.yml");
    let ci_content = engine.render_ci_yml(manifest)?;
    if ci_path.exists() && !force {
        let md_path = workflows_dir.join("ci.yml.md");
        fs::write(&md_path, &ci_content)
            .with_context(|| "Failed to write ci.yml.md")?;
        println!(
            "   {} ci.yml exists → wrote ci.yml.md for comparison",
            "📄".yellow()
        );
    } else {
        write_with_backup(&ci_path, &ci_content)?;
        if force {
            println!("   {} .github/workflows/ci.yml (overwritten)", "✓".green());
        }
    }

    // Generate release.yml
    let release_path = workflows_dir.join("release.yml");
    let release_content = engine.render_release_yml(manifest)?;
    if release_path.exists() && !force {
        let md_path = workflows_dir.join("release.yml.md");
        fs::write(&md_path, &release_content)
            .with_context(|| "Failed to write release.yml.md")?;
        println!(
            "   {} release.yml exists → wrote release.yml.md for comparison",
            "📄".yellow()
        );
    } else {
        write_with_backup(&release_path, &release_content)?;
        if force {
            println!("   {} .github/workflows/release.yml (overwritten)", "✓".green());
        }
    }

    Ok(())
}
