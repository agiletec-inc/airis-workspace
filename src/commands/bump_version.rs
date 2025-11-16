use anyhow::{bail, Context, Result};
use colored::Colorize;
use regex::Regex;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::manifest::{Manifest, VersioningStrategy, MANIFEST_FILE};

#[derive(Debug, Clone)]
pub enum BumpMode {
    Auto,     // Detect from commit message
    Major,    // x.0.0
    Minor,    // x.y.0
    Patch,    // x.y.z
}

/// Bump version in manifest.toml and sync to Cargo.toml
pub fn run(mode: BumpMode) -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);

    if !manifest_path.exists() {
        bail!(
            "❌ {} not found. Run {} first.",
            MANIFEST_FILE.bold(),
            "airis init".bold()
        );
    }

    let mut manifest = Manifest::load(manifest_path)
        .with_context(|| format!("Failed to load {}", MANIFEST_FILE))?;

    let current_version = manifest.versioning.source.clone();

    // Determine bump type
    let new_version = match mode {
        BumpMode::Auto => {
            // Detect from last commit message or versioning strategy
            match manifest.versioning.strategy {
                VersioningStrategy::Manual => {
                    bail!("❌ Versioning strategy is 'manual'. Use --major, --minor, or --patch.");
                }
                VersioningStrategy::Auto => {
                    // Default to minor bump
                    manifest.versioning.bump_minor()?
                }
                VersioningStrategy::ConventionalCommits => {
                    // Get last commit message
                    let commit_msg = get_last_commit_message()?;
                    detect_bump_from_conventional_commit(&commit_msg, &mut manifest)?
                }
            }
        }
        BumpMode::Major => manifest.versioning.bump_major()?,
        BumpMode::Minor => manifest.versioning.bump_minor()?,
        BumpMode::Patch => manifest.versioning.bump_patch()?,
    };

    println!(
        "🚀 Bumping version: {} → {}",
        current_version.yellow(),
        new_version.green().bold()
    );

    // Save updated manifest.toml
    manifest.save(manifest_path)?;

    // Update Cargo.toml
    update_cargo_toml(&new_version)?;

    println!("✅ Version bumped successfully!");
    println!("   manifest.toml: {}", new_version.green());
    println!("   Cargo.toml: {}", new_version.green());

    Ok(())
}

/// Get the last commit message
fn get_last_commit_message() -> Result<String> {
    let output = Command::new("git")
        .args(["log", "-1", "--pretty=%B"])
        .output()
        .with_context(|| "Failed to get git commit message")?;

    if !output.status.success() {
        bail!("Failed to get git commit message");
    }

    let msg = String::from_utf8(output.stdout)
        .with_context(|| "Invalid UTF-8 in commit message")?
        .trim()
        .to_string();

    Ok(msg)
}

/// Detect version bump type from Conventional Commits message
fn detect_bump_from_conventional_commit(
    commit_msg: &str,
    manifest: &mut Manifest,
) -> Result<String> {
    // BREAKING CHANGE or feat!: → major
    if commit_msg.contains("BREAKING CHANGE") || commit_msg.contains("!:") {
        return manifest.versioning.bump_major();
    }

    // feat: → minor
    if commit_msg.starts_with("feat:") || commit_msg.starts_with("feat(") {
        return manifest.versioning.bump_minor();
    }

    // fix: → patch
    if commit_msg.starts_with("fix:") || commit_msg.starts_with("fix(") {
        return manifest.versioning.bump_patch();
    }

    // chore:, docs:, style:, refactor:, test: → patch
    if commit_msg.starts_with("chore:")
        || commit_msg.starts_with("docs:")
        || commit_msg.starts_with("style:")
        || commit_msg.starts_with("refactor:")
        || commit_msg.starts_with("test:")
    {
        return manifest.versioning.bump_patch();
    }

    // Default: patch
    manifest.versioning.bump_patch()
}

/// Update version in Cargo.toml
fn update_cargo_toml(new_version: &str) -> Result<()> {
    let cargo_path = Path::new("Cargo.toml");

    if !cargo_path.exists() {
        // Cargo.toml not found (maybe not a Rust project)
        return Ok(());
    }

    let content = fs::read_to_string(cargo_path)
        .with_context(|| "Failed to read Cargo.toml")?;

    // Replace version line
    let updated = Regex::new(r#"version = "[\d.]+""#)?
        .replace(&content, format!(r#"version = "{}""#, new_version));

    fs::write(cargo_path, updated.as_ref())
        .with_context(|| "Failed to write Cargo.toml")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conventional_commits_detection() {
        let mut manifest = Manifest::default_with_project("test");
        manifest.versioning.source = "1.0.0".to_string();

        // feat: → minor
        let result = detect_bump_from_conventional_commit("feat: add new feature", &mut manifest);
        assert!(result.is_ok());
        assert_eq!(manifest.versioning.source, "1.1.0");

        // fix: → patch
        manifest.versioning.source = "1.1.0".to_string();
        let result = detect_bump_from_conventional_commit("fix: bug fix", &mut manifest);
        assert!(result.is_ok());
        assert_eq!(manifest.versioning.source, "1.1.1");

        // BREAKING CHANGE → major
        manifest.versioning.source = "1.1.1".to_string();
        let result = detect_bump_from_conventional_commit(
            "feat!: BREAKING CHANGE: api change",
            &mut manifest,
        );
        assert!(result.is_ok());
        assert_eq!(manifest.versioning.source, "2.0.0");
    }
}
