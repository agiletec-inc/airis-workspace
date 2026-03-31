use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Load the list of previously generated files from .airis/generated.toml
pub(super) fn load_generation_registry(path: &Path) -> Vec<String> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    // Simple line-based format: one path per line (skip comments and empty lines)
    content
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect()
}

/// Save the current list of generated files to .airis/generated.toml
pub(super) fn save_generation_registry(path: &Path, paths: &[String]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut sorted = paths.to_vec();
    sorted.sort();
    sorted.dedup();
    let content = format!(
        "# Auto-managed by airis gen — do not edit\n# Lists all files generated from manifest.toml\n{}\n",
        sorted.join("\n")
    );
    fs::write(path, content).context("Failed to write generation registry")?;
    Ok(())
}
