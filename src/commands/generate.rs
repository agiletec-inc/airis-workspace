use anyhow::{Context, Result};
use colored::Colorize;

use crate::config::WorkspaceConfig;
use crate::manifest::Manifest;
use crate::templates::TemplateEngine;

const WORKSPACE_FILE: &str = "workspace.yaml";

/// Sync justfile/docker-compose/package.json from MANIFEST.toml contents
pub fn sync_from_manifest(manifest: &Manifest) -> Result<()> {
    println!("{}", "🧱 Syncing workspace.yaml metadata...".bright_blue());
    let workspace_config: WorkspaceConfig = manifest.to_workspace_config();
    workspace_config
        .save(WORKSPACE_FILE)
        .context("Failed to write workspace.yaml")?;

    let engine = TemplateEngine::new()?;
    println!("{}", "🧩 Rendering templates...".bright_blue());
    generate_docker_compose(&manifest, &engine)?;
    generate_justfile(&manifest, &engine)?;
    generate_package_json(&manifest, &engine)?;
    generate_pnpm_workspace(&manifest, &engine)?;

    println!();
    println!("{}", "✅ Generated files:".green());
    println!("   - MANIFEST-driven workspace.yaml");
    println!("   - docker-compose.yml");
    println!("   - justfile");
    println!("   - package.json");
    println!("   - pnpm-workspace.yaml");
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
