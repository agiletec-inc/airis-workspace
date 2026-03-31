use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

use crate::manifest::Manifest;
use crate::templates::TemplateEngine;

use super::write_with_backup;

pub(super) fn generate_docker_compose(
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

pub(super) fn generate_service_dockerfiles(
    manifest: &Manifest,
    engine: &TemplateEngine,
) -> Result<()> {
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
        let framework = app.framework.as_deref().unwrap_or("node");
        let dockerfile_path = Path::new(app_path).join("Dockerfile");

        // Python projects maintain their own Dockerfiles — just verify it exists
        if framework == "python" {
            if dockerfile_path.exists() {
                println!(
                    "   {} {}/Dockerfile (python — user-managed)",
                    "✓".green(),
                    app_path,
                );
            } else {
                println!(
                    "   {} {}/Dockerfile missing (python projects need a hand-written Dockerfile)",
                    "⚠".yellow(),
                    app_path,
                );
            }
            continue;
        }

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
            .unwrap_or(match framework {
                "nextjs" => "nextjs",
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
