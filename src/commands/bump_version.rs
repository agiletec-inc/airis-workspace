use anyhow::{Context, Result, bail};
use colored::Colorize;
use regex::Regex;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::manifest::{MANIFEST_FILE, Manifest, VersioningStrategy};

#[derive(Debug, Clone)]
pub enum BumpMode {
    Auto,  // Detect from commit message
    Major, // x.0.0
    Minor, // x.y.0
    Patch, // x.y.z
}

/// Bump version in Cargo.toml only (manifest.toml is NEVER modified)
/// Version source of truth is git tags
pub fn run(mode: BumpMode) -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);

    // Load manifest for versioning strategy only
    let manifest = if manifest_path.exists() {
        Some(
            Manifest::load(manifest_path)
                .with_context(|| format!("Failed to load {}", MANIFEST_FILE))?,
        )
    } else {
        None
    };

    // Get current version from Cargo.toml (which should be synced from git tags)
    let current_version =
        get_cargo_version()?.ok_or_else(|| anyhow::anyhow!("❌ No version found in Cargo.toml"))?;

    // Determine bump type
    let new_version = match mode {
        BumpMode::Auto => {
            // Detect from last commit message or versioning strategy
            let strategy = manifest
                .as_ref()
                .map(|m| m.versioning.strategy.clone())
                .unwrap_or(VersioningStrategy::Auto);

            match strategy {
                VersioningStrategy::Manual => {
                    bail!("❌ Versioning strategy is 'manual'. Use --major, --minor, or --patch.");
                }
                VersioningStrategy::Auto => {
                    // Default to minor bump
                    bump_version_string(&current_version, "minor")?
                }
                VersioningStrategy::ConventionalCommits => {
                    let commit_msg = get_pending_commit_message()?;
                    detect_bump_type_from_conventional_commit(&commit_msg, &current_version)?
                }
            }
        }
        BumpMode::Major => bump_version_string(&current_version, "major")?,
        BumpMode::Minor => bump_version_string(&current_version, "minor")?,
        BumpMode::Patch => bump_version_string(&current_version, "patch")?,
    };

    println!(
        "🚀 Bumping version: {} → {}",
        current_version.yellow(),
        new_version.green().bold()
    );

    // Update Cargo.toml only (manifest.toml is NEVER touched)
    update_cargo_toml(&new_version)?;

    println!("✅ Version bumped successfully!");
    println!("   Cargo.toml: {}", new_version.green());

    Ok(())
}

/// Get current version from Cargo.toml
fn get_cargo_version() -> Result<Option<String>> {
    let cargo_path = Path::new("Cargo.toml");

    if !cargo_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(cargo_path).with_context(|| "Failed to read Cargo.toml")?;

    // Extract version from Cargo.toml
    let version = content
        .lines()
        .find(|line| line.trim().starts_with("version = "))
        .and_then(|line| {
            line.split('=')
                .nth(1)
                .map(|v| v.trim().trim_matches('"').to_string())
        });

    Ok(version)
}

/// Bump version string by type
fn bump_version_string(current: &str, bump_type: &str) -> Result<String> {
    let parts: Vec<u32> = current.split('.').map(|s| s.parse().unwrap_or(0)).collect();

    if parts.len() < 3 {
        bail!("Invalid version format: {}", current);
    }

    let (major, minor, patch) = (parts[0], parts[1], parts[2]);

    let new_version = match bump_type {
        "major" => format!("{}.0.0", major + 1),
        "minor" => format!("{}.{}.0", major, minor + 1),
        "patch" => format!("{}.{}.{}", major, minor, patch + 1),
        _ => bail!("Unknown bump type: {}", bump_type),
    };

    Ok(new_version)
}

/// Get the commit message to analyze for conventional-commit bump detection.
///
/// When invoked from a git pre-commit hook, `.git/COMMIT_EDITMSG` already
/// contains the message that is about to be committed, so we read that
/// first. Outside of a hook (manual `airis bump-version --auto`), that file
/// holds a stale value from the previous commit — in that case we fall back
/// to `git log -1`, which reflects the user's latest intent.
fn get_pending_commit_message() -> Result<String> {
    // Prefer COMMIT_EDITMSG when it looks fresh (pre-commit hook context).
    let edit_msg_path = resolve_git_dir()?.join("COMMIT_EDITMSG");
    if edit_msg_path.exists()
        && let Ok(raw) = fs::read_to_string(&edit_msg_path)
    {
        let cleaned = strip_commit_comments(&raw);
        if !cleaned.is_empty() {
            return Ok(cleaned);
        }
    }

    // Fallback: last committed message (for manual invocation after commit).
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

/// Resolve the `.git` directory (handles worktrees via `git rev-parse`).
fn resolve_git_dir() -> Result<std::path::PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .with_context(|| "Failed to resolve git dir")?;

    if !output.status.success() {
        bail!("Failed to resolve git dir");
    }

    let dir = String::from_utf8(output.stdout)
        .with_context(|| "Invalid UTF-8 from git rev-parse")?
        .trim()
        .to_string();

    Ok(std::path::PathBuf::from(dir))
}

/// Strip comment lines (starting with `#`) and trim the result.
fn strip_commit_comments(raw: &str) -> String {
    raw.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Detect version bump type from a Conventional Commits message.
///
/// Only the subject line is inspected for the type prefix and `!` marker,
/// so quoted `feat!:` examples inside commit bodies do not trigger bumps.
/// `BREAKING CHANGE` footers (anywhere in the message) still force a major bump.
fn detect_bump_type_from_conventional_commit(
    commit_msg: &str,
    current_version: &str,
) -> Result<String> {
    let subject = commit_msg.lines().next().unwrap_or("").trim();

    // BREAKING CHANGE footer anywhere, or `!` before the colon in the subject.
    let has_breaking_footer = commit_msg.contains("BREAKING CHANGE");
    let subject_is_breaking = subject
        .split_once(':')
        .map(|(prefix, _)| prefix.ends_with('!'))
        .unwrap_or(false);

    if has_breaking_footer || subject_is_breaking {
        return bump_version_string(current_version, "major");
    }

    // feat: → minor
    if subject.starts_with("feat:") || subject.starts_with("feat(") {
        return bump_version_string(current_version, "minor");
    }

    // fix: → patch
    if subject.starts_with("fix:") || subject.starts_with("fix(") {
        return bump_version_string(current_version, "patch");
    }

    // chore:, docs:, style:, refactor:, test: → patch
    if subject.starts_with("chore:")
        || subject.starts_with("docs:")
        || subject.starts_with("style:")
        || subject.starts_with("refactor:")
        || subject.starts_with("test:")
    {
        return bump_version_string(current_version, "patch");
    }

    // Default: patch
    bump_version_string(current_version, "patch")
}

/// Update version in Cargo.toml
fn update_cargo_toml(new_version: &str) -> Result<()> {
    let cargo_path = Path::new("Cargo.toml");

    if !cargo_path.exists() {
        // Cargo.toml not found (maybe not a Rust project)
        return Ok(());
    }

    let content = fs::read_to_string(cargo_path).with_context(|| "Failed to read Cargo.toml")?;

    // Replace version line
    let updated = Regex::new(r#"version = "[\d.]+""#)?
        .replace(&content, format!(r#"version = "{}""#, new_version));

    fs::write(cargo_path, updated.as_ref()).with_context(|| "Failed to write Cargo.toml")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bump_version_string() {
        // patch bump
        let result = bump_version_string("1.0.0", "patch");
        assert_eq!(result.unwrap(), "1.0.1");

        // minor bump
        let result = bump_version_string("1.0.0", "minor");
        assert_eq!(result.unwrap(), "1.1.0");

        // major bump
        let result = bump_version_string("1.0.0", "major");
        assert_eq!(result.unwrap(), "2.0.0");
    }

    #[test]
    fn test_conventional_commits_detection() {
        // feat: → minor
        let result = detect_bump_type_from_conventional_commit("feat: add new feature", "1.0.0");
        assert_eq!(result.unwrap(), "1.1.0");

        // fix: → patch
        let result = detect_bump_type_from_conventional_commit("fix: bug fix", "1.1.0");
        assert_eq!(result.unwrap(), "1.1.1");

        // feat! subject → major
        let result = detect_bump_type_from_conventional_commit("feat!: api change", "1.1.1");
        assert_eq!(result.unwrap(), "2.0.0");

        // BREAKING CHANGE footer → major
        let result = detect_bump_type_from_conventional_commit(
            "feat: add api\n\nBREAKING CHANGE: rename endpoint",
            "2.0.0",
        );
        assert_eq!(result.unwrap(), "3.0.0");
    }

    #[test]
    fn test_breaking_marker_only_matches_subject() {
        // A `fix:` commit whose body happens to contain `feat!:` as an
        // example must NOT trigger a major bump (this is the bug that
        // caused the runaway version history).
        let body_quoting_breaking =
            "fix: remove dead code\n\nThe previous feat!: change left dead resolvers behind.";
        let result = detect_bump_type_from_conventional_commit(body_quoting_breaking, "4.0.1");
        assert_eq!(result.unwrap(), "4.0.2");
    }

    #[test]
    fn test_scoped_breaking_marker() {
        // `feat(scope)!:` must be recognized as breaking.
        let result = detect_bump_type_from_conventional_commit("feat(api)!: redesign", "1.2.3");
        assert_eq!(result.unwrap(), "2.0.0");
    }

    #[test]
    fn test_strip_commit_comments() {
        let raw = "fix: something\n# please enter the commit message\n\n# Changes to be committed:";
        assert_eq!(strip_commit_comments(raw), "fix: something");
    }
}
