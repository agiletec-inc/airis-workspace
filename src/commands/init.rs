use std::env;
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;

use crate::commands::{discover, generate};
use crate::manifest::{Manifest, MANIFEST_FILE};

/// Initialize or optimize manifest-driven workspace files
pub fn run(force: bool) -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);
    let project_name = env::current_dir()
        .ok()
        .and_then(|dir| dir.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "my-monorepo".to_string());

    let manifest = if manifest_path.exists() && !force {
        println!("{}", "📖 Loading existing manifest.toml...".bright_blue());
        Manifest::load(manifest_path)?
    } else {
        // Check if this is an existing project with apps/libs
        let current_dir = env::current_dir()?;
        let has_apps = current_dir.join("apps").exists();
        let has_libs = current_dir.join("libs").exists();

        if has_apps || has_libs {
            println!(
                "{}",
                "🔍 Existing project detected! Auto-discovering structure..."
                    .bright_blue()
            );
            println!();

            // Discover project structure
            let discovered = discover::discover_project(&current_dir)?;

            println!();
            println!(
                "{}",
                "📝 Generating manifest.toml from discovered structure..."
                    .bright_blue()
            );

            // Create manifest from discovered structure
            let manifest = create_manifest_from_discovery(&project_name, discovered, &current_dir);
            manifest
                .save(manifest_path)
                .context("Failed to write manifest.toml")?;
            manifest
        } else {
            // New project - use default template
            let action = if manifest_path.exists() {
                "♻️  Re-initializing manifest.toml template..."
            } else {
                "📝 Generating manifest.toml template..."
            };
            println!("{}", action.bright_blue());
            let manifest = Manifest::default_with_project(&project_name);
            manifest
                .save(manifest_path)
                .context("Failed to write manifest.toml")?;
            manifest
        }
    };

    println!("{}", "🧩 Optimizing workspace files...".bright_blue());
    generate::sync_from_manifest(&manifest)?;

    println!();
    println!("{}", "✅ Workspace synced from manifest.toml".green());
    println!("{}", "Next steps:".bright_yellow());
    println!("  1. Review generated manifest.toml");
    println!("  2. Edit if needed (apps, libs, catalog policies)");
    println!("  3. Re-run `airis init` to re-sync files");
    println!("  4. Run `just up`");

    Ok(())
}

fn create_manifest_from_discovery(
    project_name: &str,
    discovered: discover::DiscoveredProject,
    root: &Path,
) -> Manifest {
    use crate::manifest::*;
    use std::collections::HashSet;

    let mut manifest = Manifest::default_with_project(project_name);

    // Set dev.apps from discovered apps
    manifest.dev.apps = discovered
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
                port: app.port,
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
            .root
            .dev_dependencies
            .insert(entry.name, entry.version);
    }

    manifest
}

