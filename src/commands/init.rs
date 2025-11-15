use std::env;
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;

use crate::commands::{discover, generate};
use crate::manifest::{Manifest, MANIFEST_FILE};

/// Initialize or optimize MANIFEST-driven workspace files
pub fn run(force: bool) -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);
    let project_name = env::current_dir()
        .ok()
        .and_then(|dir| dir.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "my-monorepo".to_string());

    let manifest = if manifest_path.exists() && !force {
        println!("{}", "📖 Loading existing MANIFEST.toml...".bright_blue());
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
                "📝 Generating MANIFEST.toml from discovered structure..."
                    .bright_blue()
            );

            // Create manifest from discovered structure
            let manifest = create_manifest_from_discovery(&project_name, discovered);
            manifest
                .save(manifest_path)
                .context("Failed to write MANIFEST.toml")?;
            manifest
        } else {
            // New project - use default template
            let action = if manifest_path.exists() {
                "♻️  Re-initializing MANIFEST.toml template..."
            } else {
                "📝 Generating MANIFEST.toml template..."
            };
            println!("{}", action.bright_blue());
            let manifest = Manifest::default_with_project(&project_name);
            manifest
                .save(manifest_path)
                .context("Failed to write MANIFEST.toml")?;
            manifest
        }
    };

    println!("{}", "🧩 Optimizing workspace files...".bright_blue());
    generate::sync_from_manifest(&manifest)?;

    println!();
    println!("{}", "✅ Workspace synced from MANIFEST.toml".green());
    println!("{}", "Next steps:".bright_yellow());
    println!("  1. Review generated MANIFEST.toml");
    println!("  2. Edit if needed (apps, libs, catalog policies)");
    println!("  3. Re-run `airis init` to re-sync files");
    println!("  4. Run `just up`");

    Ok(())
}

fn create_manifest_from_discovery(
    project_name: &str,
    discovered: discover::DiscoveredProject,
) -> Manifest {
    use crate::manifest::*;

    let mut manifest = Manifest::default_with_project(project_name);

    // Set apps from discovered
    manifest.dev.apps = discovered
        .apps
        .iter()
        .map(|app| app.name.clone())
        .collect();

    // TODO: Add discovered apps to manifest.apps section with proper configuration
    // TODO: Add discovered libs
    // TODO: Add catalog from discovered package.json

    manifest
}

