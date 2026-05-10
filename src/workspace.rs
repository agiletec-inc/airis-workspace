//! Workspace pattern resolution.
//!
//! Reads workspace member glob patterns from authoritative sources, in priority:
//! 1. manifest.toml `[packages].workspaces` (explicit override)
//! 2. `pnpm-workspace.yaml` `packages:` field
//! 3. `Cargo.toml` `[workspace] members`
//!
//! No hardcoded fallback. If none of the above declare workspaces, the caller
//! treats the repository as a single project (when `package.json`, `Cargo.toml`,
//! or `pyproject.toml` exists at the root) or finds nothing.

use std::fs;
use std::path::Path;

/// Resolve workspace glob patterns from authoritative sources.
pub fn resolve_patterns(root: &Path, manifest_workspaces: &[String]) -> Vec<String> {
    if !manifest_workspaces.is_empty() {
        return manifest_workspaces.to_vec();
    }
    if let Some(p) = read_pnpm_workspace_yaml(root) {
        return p;
    }
    if let Some(p) = read_cargo_workspace(root) {
        return p;
    }
    Vec::new()
}

/// Repository has a single project at the root if any of the standard
/// project files exist there.
pub fn is_single_project_root(root: &Path) -> bool {
    root.join("package.json").exists()
        || root.join("Cargo.toml").exists()
        || root.join("pyproject.toml").exists()
}

fn read_pnpm_workspace_yaml(root: &Path) -> Option<Vec<String>> {
    let content = fs::read_to_string(root.join("pnpm-workspace.yaml")).ok()?;
    let parsed: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content).ok()?;
    let packages = parsed.get("packages")?.as_sequence()?;
    let result: Vec<String> = packages
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

fn read_cargo_workspace(root: &Path) -> Option<Vec<String>> {
    let content = fs::read_to_string(root.join("Cargo.toml")).ok()?;
    let parsed: toml::Value = toml::from_str(&content).ok()?;
    let members = parsed.get("workspace")?.get("members")?.as_array()?;
    let result: Vec<String> = members
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn manifest_workspaces_take_precedence() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("pnpm-workspace.yaml"),
            "packages:\n  - 'apps/*'\n",
        )
        .unwrap();
        let patterns = resolve_patterns(dir.path(), &["custom/*".to_string()]);
        assert_eq!(patterns, vec!["custom/*"]);
    }

    #[test]
    fn pnpm_workspace_yaml_is_read_when_manifest_empty() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("pnpm-workspace.yaml"),
            "packages:\n  - 'apps/*'\n  - 'libs/*'\n",
        )
        .unwrap();
        let patterns = resolve_patterns(dir.path(), &[]);
        assert_eq!(patterns, vec!["apps/*", "libs/*"]);
    }

    #[test]
    fn cargo_workspace_members_are_read_when_pnpm_absent() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/foo\", \"crates/bar\"]\n",
        )
        .unwrap();
        let patterns = resolve_patterns(dir.path(), &[]);
        assert_eq!(patterns, vec!["crates/foo", "crates/bar"]);
    }

    #[test]
    fn empty_when_nothing_declared() {
        let dir = tempdir().unwrap();
        let patterns = resolve_patterns(dir.path(), &[]);
        assert!(patterns.is_empty());
    }

    #[test]
    fn cargo_without_workspace_section_is_skipped() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"foo\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        let patterns = resolve_patterns(dir.path(), &[]);
        assert!(patterns.is_empty());
    }

    #[test]
    fn pnpm_takes_precedence_over_cargo() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("pnpm-workspace.yaml"),
            "packages:\n  - 'apps/*'\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/foo\"]\n",
        )
        .unwrap();
        let patterns = resolve_patterns(dir.path(), &[]);
        assert_eq!(patterns, vec!["apps/*"]);
    }

    #[test]
    fn single_project_root_detection() {
        let dir = tempdir().unwrap();
        assert!(!is_single_project_root(dir.path()));

        fs::write(dir.path().join("package.json"), "{}").unwrap();
        assert!(is_single_project_root(dir.path()));
    }
}
