//! Bundle metadata: BundleMetadata struct and git info helpers

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Bundle metadata (bundle.json)
#[derive(Debug, Serialize, Deserialize)]
pub struct BundleMetadata {
    pub name: String,
    pub version: String,
    pub git_sha: String,
    pub git_branch: String,
    pub content_hash: String,
    pub runner_channel: String,
    pub dependencies: Vec<String>,
    pub created_at: String,
    pub image_ref: Option<String>,
    pub cache_hit: bool,
}

/// Generate bundle metadata
pub(super) fn generate_metadata(
    project: &str,
    hash: &str,
    image_ref: &str,
    cache_hit: bool,
) -> Result<BundleMetadata> {
    // Get git info
    let git_sha = get_git_sha().unwrap_or_else(|| "unknown".to_string());
    let git_branch = get_git_branch().unwrap_or_else(|| "unknown".to_string());

    // Get version from manifest.toml
    let version = get_project_version(project).unwrap_or_else(|| "0.0.0".to_string());

    // Get runner channel from manifest.toml
    let runner_channel = get_runner_channel(project).unwrap_or_else(|| "lts".to_string());

    // Get dependencies (simplified - just list workspace deps)
    let dependencies = get_project_dependencies(project).unwrap_or_default();

    Ok(BundleMetadata {
        name: project.to_string(),
        version,
        git_sha,
        git_branch,
        content_hash: hash.to_string(),
        runner_channel,
        dependencies,
        created_at: chrono::Utc::now().to_rfc3339(),
        image_ref: Some(image_ref.to_string()),
        cache_hit,
    })
}

fn get_git_sha() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn get_git_branch() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn get_project_version(project: &str) -> Option<String> {
    // Try to read from manifest.toml
    let content = fs::read_to_string("manifest.toml").ok()?;
    let manifest: toml::Value = toml::from_str(&content).ok()?;

    let project_name = project.rsplit('/').next().unwrap_or(project);

    // Check [projects.<name>.version]
    manifest
        .get("projects")?
        .get(project_name)?
        .get("version")?
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| {
            // Fallback to workspace version
            manifest
                .get("workspace")?
                .get("version")?
                .as_str()
                .map(|s| s.to_string())
        })
}

fn get_runner_channel(project: &str) -> Option<String> {
    let content = fs::read_to_string("manifest.toml").ok()?;
    let manifest: toml::Value = toml::from_str(&content).ok()?;

    let project_name = project.rsplit('/').next().unwrap_or(project);

    manifest
        .get("projects")?
        .get(project_name)?
        .get("runner")?
        .get("channel")?
        .as_str()
        .map(|s| s.to_string())
}

fn get_project_dependencies(project: &str) -> Option<Vec<String>> {
    // Read package.json dependencies
    let package_json_path = PathBuf::from(project).join("package.json");
    let content = fs::read_to_string(&package_json_path).ok()?;
    let pkg: serde_json::Value = serde_json::from_str(&content).ok()?;

    let mut deps = Vec::new();

    if let Some(dependencies) = pkg.get("dependencies").and_then(|d| d.as_object()) {
        for (name, _) in dependencies {
            if name.starts_with('@') && name.contains('/') {
                // Workspace dependency
                deps.push(name.clone());
            }
        }
    }

    Some(deps)
}
