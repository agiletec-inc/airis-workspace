use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

use crate::manifest::Manifest;
use crate::templates::TemplateEngine;

use super::write_with_backup;

pub(super) fn generate_env_example(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    let content = engine.render_env_example(manifest)?;
    let path = Path::new(".env.example");

    fs::write(path, &content).with_context(|| "Failed to write .env.example")?;

    println!(
        "   {} Generated .env.example from [env] section",
        "📄".green()
    );

    Ok(())
}

pub(super) fn generate_npmrc(engine: &TemplateEngine) -> Result<()> {
    let content = engine.render_npmrc()?;
    let path = Path::new(".npmrc");

    write_with_backup(path, &content)?;
    println!("   {} .npmrc (pnpm store isolation)", "✓".green());

    Ok(())
}

pub(super) fn generate_envrc(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
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
