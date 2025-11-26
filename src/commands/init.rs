use std::env;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use colored::Colorize;

use crate::commands::{discover, generate, guards, snapshot};
use crate::manifest::{Manifest, MANIFEST_FILE};

/// Get git repository root directory
fn get_git_root() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            String::from_utf8(output.stdout)
                .ok()
                .map(|s| s.trim().to_string())
        })
}

/// Get project name from git repository root directory name
fn get_project_name_from_git() -> Option<String> {
    get_git_root()
        .and_then(|root| {
            Path::new(&root)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
        })
}

/// Get version from latest git tag (e.g., v1.8.3 -> 1.8.3)
fn get_version_from_git_tag() -> Option<String> {
    Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            String::from_utf8(output.stdout)
                .ok()
                .map(|s| s.trim().trim_start_matches('v').to_string())
        })
}

/// Sync version from git tag to Cargo.toml only
/// manifest.toml is NEVER modified - it's sacred
fn sync_cargo_version_from_git_tag() -> Result<()> {
    use std::fs;

    let git_version = match get_version_from_git_tag() {
        Some(v) => v,
        None => return Ok(()), // No git tags, skip sync
    };

    let cargo_path = Path::new("Cargo.toml");
    if !cargo_path.exists() {
        return Ok(());
    }

    let cargo_content = fs::read_to_string(cargo_path)?;

    // Extract current version from Cargo.toml
    let current_version = cargo_content
        .lines()
        .find(|line| line.trim().starts_with("version = "))
        .and_then(|line| {
            line.split('=')
                .nth(1)
                .map(|v| v.trim().trim_matches('"').to_string())
        });

    if let Some(current) = current_version {
        if current == git_version {
            return Ok(()); // Already in sync
        }

        // Update Cargo.toml with new version
        let updated_content = cargo_content
            .lines()
            .map(|line| {
                if line.trim().starts_with("version = ") {
                    format!("version = \"{}\"", git_version)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        fs::write(cargo_path, updated_content + "\n")?;

        println!(
            "{} {} → {}",
            "🔄 Syncing Cargo.toml version from git tag:".bright_blue(),
            current.yellow(),
            git_version.green()
        );
    }

    Ok(())
}

/// Validate manifest.toml and print warnings for issues
fn validate_manifest(manifest: &Manifest) {
    let mut warnings = Vec::new();

    // Check: auto_version enabled but version field exists in project
    if manifest.ci.auto_version && !manifest.project.version.is_empty() {
        warnings.push(format!(
            "{} [project].version is defined but auto_version is enabled.\n   \
             Remove version from manifest.toml - it should only exist in Cargo.toml.\n   \
             Git tags are the source of truth when auto_version = true.",
            "⚠️ ".yellow()
        ));
    }

    // Check: empty apps but docker-compose.yml exists (dangerous overwrite risk)
    if manifest.apps.is_empty() {
        let compose_exists = Path::new("docker-compose.yml").exists()
            || Path::new("docker-compose.yml").exists();
        let apps_dir_exists = Path::new("apps").exists();

        if compose_exists && apps_dir_exists {
            warnings.push(format!(
                "{} No apps defined in manifest.toml but docker-compose.yml and apps/ exist.\n   \
                 This may cause existing compose services to be overwritten.\n   \
                 Run `airis init --snapshot` first, then define apps in manifest.toml.",
                "⚠️ ".yellow()
            ));
        }
    }

    // Check: project.id is missing
    if manifest.project.id.is_empty() {
        warnings.push(format!(
            "{} [project].id is not defined.\n   \
             Add a project identifier to manifest.toml.",
            "⚠️ ".yellow()
        ));
    }

    // Check: workspace.name is default
    if manifest.workspace.name == "my-workspace" {
        warnings.push(format!(
            "{} [workspace].name is using default value 'my-workspace'.\n   \
             Consider setting a proper workspace name.",
            "💡".cyan()
        ));
    }

    // Check: guards defined but npm/pnpm not in deny list
    if !manifest.guards.deny.is_empty() {
        let has_npm_guard = manifest.guards.deny.iter().any(|cmd| {
            cmd == "npm" || cmd == "pnpm" || cmd == "yarn"
        });
        if !has_npm_guard {
            warnings.push(format!(
                "{} Guards are defined but no package manager (npm/pnpm/yarn) is blocked.\n   \
                 Add them to [guards].deny for Docker-first enforcement.",
                "💡".cyan()
            ));
        }
    }

    // Print warnings
    if !warnings.is_empty() {
        println!();
        println!("{}", "📋 Manifest validation warnings:".bright_yellow());
        for warning in warnings {
            println!("   {}", warning);
        }
        println!();
    }
}

// REMOVED: sync_from_filesystem() - bidirectional sync is handled by /airis:init LLM command
// The Rust CLI is now READ-ONLY: manifest.toml → generated files only
// For updating manifest.toml from filesystem changes, use: /airis:init (Claude Code custom command)

/// Initialize or optimize manifest-driven workspace files
///
/// READ-ONLY MODE: manifest.toml → generated files
/// - Reads manifest.toml (source of truth)
/// - Generates workspace files (package.json, pnpm-workspace.yaml, workflows, etc.)
/// - NEVER modifies manifest.toml (except on initial creation)
/// - For updating manifest.toml from filesystem: use `/airis:init` (Claude Code command)
///
/// Initial creation mode (when manifest.toml doesn't exist):
/// - Auto-discovers apps/libs structure
/// - Creates manifest.toml from discovered structure
/// - Only happens once on first run
///
/// Snapshot behavior:
/// - Default: auto-snapshot on first run (when .airis/snapshots.toml doesn't exist)
/// - --snapshot: force snapshot capture
/// - --no-snapshot: skip snapshot (for CI or repeated runs)
pub fn run(force_snapshot: bool, no_snapshot: bool) -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);
    let current_dir = env::current_dir()?;

    // Determine if snapshot should be captured
    let snapshots_exist = Path::new(".airis/snapshots.toml").exists();
    let should_snapshot = if no_snapshot {
        false
    } else if force_snapshot {
        true
    } else {
        // Auto-snapshot on first run only
        !snapshots_exist
    };

    // Capture snapshots if needed
    if should_snapshot {
        if !snapshots_exist {
            println!("{}", "📸 First-time initialization detected — snapshot enabled automatically".bright_blue());
        }
        snapshot::capture_snapshots()?;
        println!();
    }

    let manifest = if manifest_path.exists() {
        // Load existing manifest (READ-ONLY mode)
        println!("{}", "📖 Loading existing manifest.toml...".bright_blue());
        Manifest::load(manifest_path)?
    } else {
        // ✅ INITIAL CREATION MODE: Only happens when manifest.toml doesn't exist
        // Priority: git root directory name > current directory name > default
        let project_name = get_project_name_from_git()
            .or_else(|| {
                current_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| "my-monorepo".to_string());

        let has_apps = current_dir.join("apps").exists();
        let has_libs = current_dir.join("libs").exists();

        let manifest = if has_apps || has_libs {
            println!("{}", "🔍 Existing project detected! Auto-discovering structure...".bright_blue());
            println!();

            let discovered = discover::discover_project(&current_dir)?;

            println!();
            println!("{}", "📝 Generating manifest.toml from discovered structure...".bright_blue());

            create_manifest_from_discovery(&project_name, discovered, &current_dir)
        } else {
            println!("{}", "📝 Generating manifest.toml template...".bright_blue());
            Manifest::default_with_project(&project_name)
        };

        // Save newly created manifest
        manifest.save(manifest_path)
            .context("Failed to write manifest.toml")?;

        manifest
    };

    // Validate manifest and print warnings for configuration issues
    validate_manifest(&manifest);

    // Sync Cargo.toml version from git tag if auto_version is enabled
    // NOTE: manifest.toml is NEVER modified - only Cargo.toml is updated
    if manifest.ci.auto_version {
        sync_cargo_version_from_git_tag()?;
    }

    println!("{}", "🧩 Regenerating workspace files from manifest.toml...".bright_blue());
    generate::sync_from_manifest(&manifest)?;

    // Install guards if defined in manifest
    if !manifest.guards.deny.is_empty()
        || !manifest.guards.wrap.is_empty()
        || !manifest.guards.deny_with_message.is_empty()
    {
        println!();
        guards::install()?;

        // Generate .envrc for direnv auto-activation
        generate_envrc()?;
    }

    println!();
    println!("{}", "✅ Workspace files generated from manifest.toml".green());
    println!("{}", "Next steps:".bright_yellow());
    println!("  1. Edit manifest.toml to add/remove apps, libs, or catalog entries");
    println!("  2. Run `airis init` to regenerate workspace files from manifest");
    println!("  3. Use `/airis:init` (Claude Code) to sync filesystem → manifest.toml");
    println!("  4. Run `airis up` to start development");

    Ok(())
}

/// Setup .npmrc symlinks for Docker-First enforcement
/// This creates symlinks in apps/* and libs/* pointing to root .npmrc
pub fn setup_npmrc() -> Result<()> {
    use std::fs;
    use std::os::unix::fs::symlink;

    println!("{}", "🔗 Setting up .npmrc symlinks...".bright_blue());
    println!();

    let root_npmrc = Path::new(".npmrc");
    if !root_npmrc.exists() {
        anyhow::bail!("Root .npmrc not found. Create it first or run airis init.");
    }

    let mut created = 0;
    let mut skipped = 0;

    // Process apps directory
    let apps_dir = Path::new("apps");
    if apps_dir.exists() {
        for entry in fs::read_dir(apps_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            // Check if package.json exists (valid app)
            if !path.join("package.json").exists() {
                continue;
            }

            let npmrc_path = path.join(".npmrc");
            let relative_root = "../../.npmrc";

            if npmrc_path.exists() {
                // Check if it's already a symlink to root
                if npmrc_path.is_symlink() {
                    println!("  {} {} (already linked)", "⏭️".yellow(), npmrc_path.display());
                    skipped += 1;
                } else {
                    // Remove existing file and create symlink
                    fs::remove_file(&npmrc_path)?;
                    symlink(relative_root, &npmrc_path)?;
                    println!("  {} {} (replaced)", "✓".green(), npmrc_path.display());
                    created += 1;
                }
            } else {
                // Create new symlink
                symlink(relative_root, &npmrc_path)?;
                println!("  {} {}", "✓".green(), npmrc_path.display());
                created += 1;
            }
        }
    }

    // Process libs directory
    let libs_dir = Path::new("libs");
    if libs_dir.exists() {
        for entry in fs::read_dir(libs_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            // Check if package.json exists (valid lib)
            if !path.join("package.json").exists() {
                continue;
            }

            let npmrc_path = path.join(".npmrc");
            let relative_root = "../../.npmrc";

            if npmrc_path.exists() {
                if npmrc_path.is_symlink() {
                    println!("  {} {} (already linked)", "⏭️".yellow(), npmrc_path.display());
                    skipped += 1;
                } else {
                    fs::remove_file(&npmrc_path)?;
                    symlink(relative_root, &npmrc_path)?;
                    println!("  {} {} (replaced)", "✓".green(), npmrc_path.display());
                    created += 1;
                }
            } else {
                symlink(relative_root, &npmrc_path)?;
                println!("  {} {}", "✓".green(), npmrc_path.display());
                created += 1;
            }
        }
    }

    println!();
    println!(
        "{} Created {} symlinks, skipped {} existing",
        "✅".green(),
        created,
        skipped
    );
    println!();
    println!("{}", "🛡️  Triple-layer defense active:".bright_yellow());
    println!("  1. .npmrc symlinks (primary)");
    println!("  2. preinstall hooks (backup)");
    println!("  3. Root preinstall + monorepo check (fallback)");

    Ok(())
}

/// Generate .envrc for direnv to auto-activate guards
fn generate_envrc() -> Result<()> {
    use std::fs;

    let envrc_path = Path::new(".envrc");
    let envrc_content = r#"# Auto-generated by airis init
# This file activates Docker-first guards via direnv

# Add guards to PATH (intercepts npm, yarn, pnpm, etc.)
export PATH="$PWD/.airis/bin:$PATH"

# Optional: Source local environment variables
if [ -f .env.local ]; then
    dotenv .env.local
fi
"#;

    // Check if .envrc already exists
    if envrc_path.exists() {
        let existing = fs::read_to_string(envrc_path)?;
        if existing.contains(".airis/bin") {
            // Already configured, skip
            return Ok(());
        }
        // Append to existing .envrc
        let updated = format!("{}\n{}", existing.trim(), envrc_content);
        fs::write(envrc_path, updated)?;
    } else {
        fs::write(envrc_path, envrc_content)?;
    }

    println!();
    println!("{}", "📁 Generated .envrc for direnv".green());
    println!("{}", "To activate guards automatically:".bright_yellow());
    println!("  direnv allow");

    Ok(())
}

// REMOVED: merge_discovery_into_manifest() and sync_from_filesystem() are no longer used
// manifest.toml is NEVER modified after initial creation (READ-ONLY mode)
// For updating manifest.toml from filesystem: use /airis:init (Claude Code custom command)

fn create_manifest_from_discovery(
    project_name: &str,
    discovered: discover::DiscoveredProject,
    root: &Path,
) -> Manifest {
    use crate::manifest::*;
    use std::collections::HashSet;

    let mut manifest = Manifest::default_with_project(project_name);

    // apps_pattern already defaults to "apps/*/docker-compose.yml"
    // No need to explicitly list apps - they're auto-discovered at runtime

    // Add app configurations with type
    for app in &discovered.apps {
        let rel_path = app.path.strip_prefix(root).ok()
            .and_then(|p| p.to_str())
            .map(|s| s.to_string());

        let app_type_str = match app.app_type {
            discover::AppType::NextJs => Some("nextjs".to_string()),
            discover::AppType::Node => Some("node".to_string()),
            discover::AppType::Rust => Some("rust".to_string()),
            discover::AppType::Python => Some("python".to_string()),
            discover::AppType::Unknown => None,
        };

        manifest.apps.insert(
            app.name.clone(),
            AppConfig {
                path: rel_path,
                app_type: app_type_str,
            },
        );
    }

    // Add lib configurations
    for lib in &discovered.libs {
        let rel_path = lib.path.strip_prefix(root).ok()
            .and_then(|p| p.to_str())
            .map(|s| s.to_string());

        manifest.libs.insert(
            lib.name.clone(),
            LibConfig {
                path: rel_path,
            },
        );
    }

    // Infer workspace patterns from discovered apps and libs
    let mut workspace_patterns = HashSet::new();

    // Extract parent directories from apps (convert to relative paths)
    for app in &discovered.apps {
        if let Ok(rel_path) = app.path.strip_prefix(root) {
            if let Some(parent) = rel_path.parent() {
                if let Some(parent_str) = parent.to_str() {
                    if !parent_str.is_empty() {
                        // Extract the top-level directory (e.g., "apps" from "apps/dashboard")
                        let top_dir = parent_str.split('/').next().unwrap_or(parent_str);
                        workspace_patterns.insert(format!("{}/*", top_dir));
                    }
                }
            }
        }
    }

    // Extract parent directories from libs (convert to relative paths)
    for lib in &discovered.libs {
        if let Ok(rel_path) = lib.path.strip_prefix(root) {
            if let Some(parent) = rel_path.parent() {
                if let Some(parent_str) = parent.to_str() {
                    if !parent_str.is_empty() {
                        // Extract the top-level directory (e.g., "libs" from "libs/ui")
                        let top_dir = parent_str.split('/').next().unwrap_or(parent_str);
                        workspace_patterns.insert(format!("{}/*", top_dir));
                    }
                }
            }
        }
    }

    // Convert to sorted Vec for consistent output
    let mut workspaces: Vec<String> = workspace_patterns.into_iter().collect();
    workspaces.sort();
    manifest.packages.workspaces = workspaces;

    // Add catalog entries from discovered package.json
    for entry in discovered.catalog {
        manifest
            .packages
            .catalog
            .insert(entry.name, crate::manifest::CatalogEntry::Version(entry.version));
    }

    manifest
}

