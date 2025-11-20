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

/// Sync version from git tag to manifest.toml and Cargo.toml
fn sync_version_from_git_tag(manifest: &mut Manifest) -> Result<bool> {
    let git_version = match get_version_from_git_tag() {
        Some(v) => v,
        None => return Ok(false), // No git tags, skip sync
    };

    // Check if version needs updating
    let current_version = &manifest.project.version;
    if current_version == &git_version {
        return Ok(false); // Already in sync
    }

    // Update manifest version
    let old_version = current_version.clone();
    manifest.project.version = git_version.clone();

    // Also update versioning.source if it exists
    manifest.versioning.source = git_version.clone();

    println!(
        "{} {} → {}",
        "🔄 Syncing version from git tag:".bright_blue(),
        old_version.yellow(),
        git_version.green()
    );

    Ok(true)
}

/// Initialize or optimize manifest-driven workspace files
///
/// IMPORTANT: This command NEVER overwrites existing manifest.toml.
/// - If manifest.toml exists: read-only mode, regenerate other files
/// - If manifest.toml does not exist: create initial template
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

    let mut manifest = if manifest_path.exists() {
        // ✅ READ-ONLY MODE: Never modify existing manifest.toml
        println!("{}", "📖 Using existing manifest.toml as source of truth".bright_blue());
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

        if has_apps || has_libs {
            println!("{}", "🔍 Existing project detected! Auto-discovering structure...".bright_blue());
            println!();

            let discovered = discover::discover_project(&current_dir)?;

            println!();
            println!("{}", "📝 Generating manifest.toml from discovered structure...".bright_blue());

            let manifest = create_manifest_from_discovery(&project_name, discovered, &current_dir);
            manifest
                .save(manifest_path)
                .context("Failed to write manifest.toml")?;
            manifest
        } else {
            println!("{}", "📝 Generating manifest.toml template...".bright_blue());
            let manifest = Manifest::default_with_project(&project_name);
            manifest
                .save(manifest_path)
                .context("Failed to write manifest.toml")?;
            manifest
        }
    };

    // Sync version from git tag if auto_version is enabled
    if manifest.ci.auto_version {
        let version_updated = sync_version_from_git_tag(&mut manifest)?;
        if version_updated {
            // Save updated manifest with new version
            manifest
                .save(manifest_path)
                .context("Failed to save manifest.toml with updated version")?;
        }
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
    println!("{}", "✅ Workspace synced from manifest.toml".green());
    println!("{}", "Next steps:".bright_yellow());
    println!("  1. Edit manifest.toml if needed (apps, libs, catalog)");
    println!("  2. Re-run `airis init` to re-sync generated files");
    println!("  3. Run `airis up` to start development");

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

// DELETED: merge_discovery_into_manifest() is no longer used
// manifest.toml is never modified after initial creation

fn create_manifest_from_discovery(
    project_name: &str,
    discovered: discover::DiscoveredProject,
    root: &Path,
) -> Manifest {
    use crate::manifest::*;
    use std::collections::HashSet;

    let mut manifest = Manifest::default_with_project(project_name);

    // Set dev.autostart from discovered apps
    manifest.dev.autostart = discovered
        .apps
        .iter()
        .map(|app| app.name.clone())
        .collect();

    // Add app configurations with type and port
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

