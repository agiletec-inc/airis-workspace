use anyhow::Result;
use colored::Colorize;
use indexmap::IndexMap;
use std::path::Path;

use crate::manifest::Manifest;
use crate::templates::TemplateEngine;

use super::write_with_backup;

pub(super) fn generate_package_json(
    manifest: &Manifest,
    engine: &TemplateEngine,
    resolved_catalog: &IndexMap<String, String>,
    _force: bool,
) -> Result<()> {
    let path = Path::new("package.json");
    let content = engine.render_package_json(manifest, resolved_catalog)?;
    write_with_backup(path, &content)?;
    println!(
        "   {} package.json (synced from manifest.toml)",
        "✓".green()
    );
    Ok(())
}

pub(super) fn generate_pnpm_workspace(
    _manifest: &Manifest,
    _engine: &TemplateEngine,
    _force: bool,
) -> Result<()> {
    // pnpm-workspace.yaml is now user-owned.
    // airis no longer overwrites it from manifest.toml to prevent accidental data loss.
    Ok(())
}
