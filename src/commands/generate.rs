use anyhow::{Context, Result};
use colored::Colorize;
use std::env;

use crate::generators::package_json::generate_project_package_json;
use crate::manifest::Manifest;
use crate::templates::TemplateEngine;

/// Sync justfile/docker-compose/package.json from manifest.toml contents
pub fn sync_from_manifest(manifest: &Manifest) -> Result<()> {
    let engine = TemplateEngine::new()?;
    println!("{}", "🧩 Rendering templates...".bright_blue());
    generate_docker_compose(&manifest, &engine)?;
    generate_justfile(&manifest, &engine)?;
    generate_package_json(&manifest, &engine)?;
    generate_pnpm_workspace(&manifest, &engine)?;

    // Generate individual project package.json files
    if !manifest.project.is_empty() {
        println!();
        println!("{}", "📦 Generating project package.json files...".bright_blue());
        let workspace_root = env::current_dir().context("Failed to get current directory")?;

        for project in &manifest.project {
            generate_project_package_json(project, &workspace_root)?;
        }
    }

    println!();
    println!("{}", "✅ Generated files:".green());
    println!("   - docker-compose.yml");
    println!("   - justfile");
    println!("   - package.json");
    println!("   - pnpm-workspace.yaml");
    if !manifest.project.is_empty() {
        println!("   - {} project package.json files", manifest.project.len());
    }
    println!();
    println!("{}", "Next steps:".bright_yellow());
    println!("  1. Review justfile/docker-compose.yml");
    println!("  2. Run `just up`");

    Ok(())
}

fn generate_justfile(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    let content = engine.render_justfile(manifest)?;
    let output = "justfile";
    std::fs::write(output, content).with_context(|| format!("Failed to write {}", output))?;
    Ok(())
}

fn generate_package_json(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    let content = engine.render_package_json(manifest)?;
    std::fs::write("package.json", content).context("Failed to write package.json")?;
    Ok(())
}

fn generate_pnpm_workspace(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    let content = engine.render_pnpm_workspace(manifest)?;
    std::fs::write("pnpm-workspace.yaml", content)
        .context("Failed to write pnpm-workspace.yaml")?;
    Ok(())
}

fn generate_docker_compose(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    let content = engine.render_docker_compose(manifest)?;
    std::fs::write("docker-compose.yml", content).context("Failed to write docker-compose.yml")?;
    Ok(())
}
