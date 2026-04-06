use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

use crate::manifest::Manifest;
use crate::templates::TemplateEngine;

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
