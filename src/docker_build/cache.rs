//! Cache functions for Docker build artifacts

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use super::CachedArtifact;

/// Get cache directory path: ~/.airis/.cache/<project>/<hash>/
pub(crate) fn cache_dir(project: &str, hash: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let project_safe = project.replace('/', "_");
    PathBuf::from(home)
        .join(".airis")
        .join(".cache")
        .join(project_safe)
        .join(hash)
}

/// Check if cache hit exists for given project and hash
pub fn cache_hit(project: &str, hash: &str) -> Option<CachedArtifact> {
    let artifact_path = cache_dir(project, hash).join("artifact.json");
    if artifact_path.exists() {
        let content = fs::read_to_string(&artifact_path).ok()?;
        serde_json::from_str(&content).ok()
    } else {
        None
    }
}

/// Store artifact in cache
pub fn cache_store(project: &str, hash: &str, artifact: &CachedArtifact) -> Result<()> {
    let dir = cache_dir(project, hash);
    fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create cache directory: {}", dir.display()))?;

    let artifact_path = dir.join("artifact.json");
    let content = serde_json::to_string_pretty(artifact).context("Failed to serialize artifact")?;

    fs::write(&artifact_path, content)
        .with_context(|| format!("Failed to write cache: {}", artifact_path.display()))?;

    Ok(())
}
