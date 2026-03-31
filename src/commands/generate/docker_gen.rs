use anyhow::Result;
use colored::Colorize;
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
