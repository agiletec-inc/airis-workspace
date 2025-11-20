//! Snapshot module: capture existing files before regeneration
//!
//! This module handles creating backups and snapshots of existing configuration
//! files before they are overwritten by `airis init --snapshot`.

use anyhow::{Context, Result};
use chrono::Utc;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

/// Directory for airis tool data
const AIRIS_DIR: &str = ".airis";
/// Subdirectory for file backups
const BACKUPS_DIR: &str = "backups";
/// Snapshot metadata file
const SNAPSHOTS_FILE: &str = "snapshots.toml";

/// A single snapshot entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotEntry {
    /// Original file path (relative to workspace root)
    pub path: String,
    /// Backup file path (relative to workspace root)
    pub backup_path: String,
    /// File format (json, yaml, toml)
    pub format: String,
    /// Timestamp when snapshot was captured
    pub captured_at: String,
    /// SHA256 hash of the original content
    pub hash: String,
}

/// Collection of snapshots
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Snapshots {
    pub snapshot: Vec<SnapshotEntry>,
}

impl Snapshots {
    /// Load snapshots from .airis/snapshots.toml
    pub fn load() -> Result<Option<Self>> {
        let path = Path::new(AIRIS_DIR).join(SNAPSHOTS_FILE);
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let snapshots: Snapshots = toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        Ok(Some(snapshots))
    }

    /// Save snapshots to .airis/snapshots.toml
    pub fn save(&self) -> Result<()> {
        let airis_dir = Path::new(AIRIS_DIR);
        fs::create_dir_all(airis_dir)
            .with_context(|| format!("Failed to create {}", airis_dir.display()))?;

        let path = airis_dir.join(SNAPSHOTS_FILE);
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize snapshots")?;

        fs::write(&path, content)
            .with_context(|| format!("Failed to write {}", path.display()))?;

        Ok(())
    }
}

/// Files to capture in snapshot
const SNAPSHOT_TARGETS: &[&str] = &[
    "package.json",
    "pnpm-workspace.yaml",
    "justfile",
    "docker-compose.yml",
    ".github/workflows/ci.yml",
    ".github/workflows/release.yml",
];

/// Run snapshot capture before regeneration
pub fn capture_snapshots() -> Result<Snapshots> {
    println!("{}", "📸 Capturing snapshots of existing files...".bright_blue());

    // Create .airis/backups directory
    let backups_dir = Path::new(AIRIS_DIR).join(BACKUPS_DIR);
    fs::create_dir_all(&backups_dir)
        .with_context(|| format!("Failed to create {}", backups_dir.display()))?;

    let mut snapshots = Snapshots::default();
    let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let date_suffix = Utc::now().format("%Y-%m-%d").to_string();

    for &target in SNAPSHOT_TARGETS {
        let path = Path::new(target);
        if !path.exists() {
            continue;
        }

        // Read original content
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", target))?;

        // Calculate hash
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = format!("sha256:{:x}", hasher.finalize());

        // Determine format from extension
        let format = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt")
            .to_string();

        // Create backup filename
        let backup_filename = format!(
            "{}.{}.{}",
            target.replace('/', "."),
            date_suffix,
            if format == "txt" { "txt" } else { &format }
        );
        let backup_path = backups_dir.join(&backup_filename);

        // Write backup
        fs::write(&backup_path, &content)
            .with_context(|| format!("Failed to write backup {}", backup_path.display()))?;

        // Create snapshot entry
        let entry = SnapshotEntry {
            path: target.to_string(),
            backup_path: format!("{}/{}/{}", AIRIS_DIR, BACKUPS_DIR, backup_filename),
            format,
            captured_at: timestamp.clone(),
            hash,
        };

        println!("  {} {}", "✓".green(), target);
        snapshots.snapshot.push(entry);
    }

    // Also capture app-specific package.json files
    capture_app_package_jsons(&mut snapshots, &backups_dir, &timestamp, &date_suffix)?;

    // Save snapshots metadata
    snapshots.save()?;

    println!();
    println!(
        "{} {} files",
        "📦 Captured".bright_blue(),
        snapshots.snapshot.len().to_string().cyan()
    );

    Ok(snapshots)
}

/// Capture package.json files from apps/ directory
fn capture_app_package_jsons(
    snapshots: &mut Snapshots,
    backups_dir: &Path,
    timestamp: &str,
    date_suffix: &str,
) -> Result<()> {
    let apps_dir = Path::new("apps");
    if !apps_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(apps_dir).context("Failed to read apps directory")? {
        let entry = entry?;
        let app_path = entry.path();

        if !app_path.is_dir() {
            continue;
        }

        let package_json = app_path.join("package.json");
        if !package_json.exists() {
            continue;
        }

        let app_name = app_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let relative_path = format!("apps/{}/package.json", app_name);

        // Read content
        let content = fs::read_to_string(&package_json)
            .with_context(|| format!("Failed to read {}", relative_path))?;

        // Calculate hash
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = format!("sha256:{:x}", hasher.finalize());

        // Create backup
        let backup_filename = format!("apps.{}.package.json.{}.json", app_name, date_suffix);
        let backup_path = backups_dir.join(&backup_filename);

        fs::write(&backup_path, &content)
            .with_context(|| format!("Failed to write backup {}", backup_path.display()))?;

        let entry = SnapshotEntry {
            path: relative_path.clone(),
            backup_path: format!("{}/{}/{}", AIRIS_DIR, BACKUPS_DIR, backup_filename),
            format: "json".to_string(),
            captured_at: timestamp.to_string(),
            hash,
        };

        println!("  {} {}", "✓".green(), relative_path);
        snapshots.snapshot.push(entry);
    }

    Ok(())
}

/// Compare current state with snapshots
/// Returns a list of differences found
pub fn compare_with_snapshots(snapshots: &Snapshots) -> Result<Vec<SnapshotDiff>> {
    let mut diffs = Vec::new();

    for entry in &snapshots.snapshot {
        let current_path = Path::new(&entry.path);

        if !current_path.exists() {
            diffs.push(SnapshotDiff {
                path: entry.path.clone(),
                diff_type: DiffType::Deleted,
                details: "File no longer exists".to_string(),
            });
            continue;
        }

        // Read current content and compare hash
        let current_content = fs::read_to_string(current_path)
            .with_context(|| format!("Failed to read {}", entry.path))?;

        let mut hasher = Sha256::new();
        hasher.update(current_content.as_bytes());
        let current_hash = format!("sha256:{:x}", hasher.finalize());

        if current_hash != entry.hash {
            // Content changed - for JSON files, try to show specific differences
            if entry.format == "json" {
                if let Some(details) = compare_json_content(&entry.backup_path, &entry.path)? {
                    diffs.push(SnapshotDiff {
                        path: entry.path.clone(),
                        diff_type: DiffType::Modified,
                        details,
                    });
                }
            } else {
                diffs.push(SnapshotDiff {
                    path: entry.path.clone(),
                    diff_type: DiffType::Modified,
                    details: "Content hash mismatch".to_string(),
                });
            }
        }
    }

    Ok(diffs)
}

/// Type of difference found
#[derive(Debug)]
pub enum DiffType {
    Modified,
    Deleted,
}

/// A difference between snapshot and current state
#[derive(Debug)]
pub struct SnapshotDiff {
    pub path: String,
    pub diff_type: DiffType,
    pub details: String,
}

/// Compare JSON files and return specific differences
fn compare_json_content(backup_path: &str, current_path: &str) -> Result<Option<String>> {
    let backup_content = fs::read_to_string(backup_path)?;
    let current_content = fs::read_to_string(current_path)?;

    let backup: serde_json::Value = serde_json::from_str(&backup_content)
        .unwrap_or(serde_json::Value::Null);
    let current: serde_json::Value = serde_json::from_str(&current_content)
        .unwrap_or(serde_json::Value::Null);

    if backup == current {
        return Ok(None);
    }

    // Try to find specific differences in dependencies
    let mut details = Vec::new();

    if let (Some(backup_deps), Some(current_deps)) = (
        backup.get("dependencies").and_then(|d| d.as_object()),
        current.get("dependencies").and_then(|d| d.as_object()),
    ) {
        // Check for removed dependencies
        for key in backup_deps.keys() {
            if !current_deps.contains_key(key) {
                details.push(format!("- {} removed", key));
            }
        }
        // Check for added dependencies
        for key in current_deps.keys() {
            if !backup_deps.contains_key(key) {
                details.push(format!("+ {} added", key));
            }
        }
        // Check for changed versions
        for (key, backup_val) in backup_deps {
            if let Some(current_val) = current_deps.get(key) {
                if backup_val != current_val {
                    details.push(format!(
                        "~ {} {} → {}",
                        key,
                        backup_val.as_str().unwrap_or("?"),
                        current_val.as_str().unwrap_or("?")
                    ));
                }
            }
        }
    }

    // Same for devDependencies
    if let (Some(backup_deps), Some(current_deps)) = (
        backup.get("devDependencies").and_then(|d| d.as_object()),
        current.get("devDependencies").and_then(|d| d.as_object()),
    ) {
        for key in backup_deps.keys() {
            if !current_deps.contains_key(key) {
                details.push(format!("- {} (dev) removed", key));
            }
        }
        for key in current_deps.keys() {
            if !backup_deps.contains_key(key) {
                details.push(format!("+ {} (dev) added", key));
            }
        }
    }

    if details.is_empty() {
        Ok(Some("Content changed (non-dependency fields)".to_string()))
    } else {
        Ok(Some(details.join(", ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_snapshot_entry_serialization() {
        let entry = SnapshotEntry {
            path: "package.json".to_string(),
            backup_path: ".airis/backups/package.json.2025-11-20.json".to_string(),
            format: "json".to_string(),
            captured_at: "2025-11-20T10:30:00Z".to_string(),
            hash: "sha256:abcd1234".to_string(),
        };

        let snapshots = Snapshots {
            snapshot: vec![entry],
        };

        let toml_str = toml::to_string(&snapshots).unwrap();
        assert!(toml_str.contains("[[snapshot]]"));
        assert!(toml_str.contains("package.json"));
    }

    #[test]
    fn test_snapshot_targets_contains_expected_files() {
        assert!(SNAPSHOT_TARGETS.contains(&"package.json"));
        assert!(SNAPSHOT_TARGETS.contains(&"pnpm-workspace.yaml"));
        assert!(SNAPSHOT_TARGETS.contains(&"justfile"));
        assert!(SNAPSHOT_TARGETS.contains(&"docker-compose.yml"));
        assert!(SNAPSHOT_TARGETS.contains(&".github/workflows/ci.yml"));
        assert!(SNAPSHOT_TARGETS.contains(&".github/workflows/release.yml"));
    }

    #[test]
    fn test_compare_json_content_identical() {
        let temp_dir = TempDir::new().unwrap();

        let content = r#"{"name": "test", "version": "1.0.0"}"#;
        let backup_path = temp_dir.path().join("backup.json");
        let current_path = temp_dir.path().join("current.json");

        fs::write(&backup_path, content).unwrap();
        fs::write(&current_path, content).unwrap();

        let result = compare_json_content(
            backup_path.to_str().unwrap(),
            current_path.to_str().unwrap(),
        ).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_compare_json_content_added_dependency() {
        let temp_dir = TempDir::new().unwrap();

        let backup = r#"{"dependencies": {"react": "^18.0.0"}}"#;
        let current = r#"{"dependencies": {"react": "^18.0.0", "lodash": "^4.0.0"}}"#;

        let backup_path = temp_dir.path().join("backup.json");
        let current_path = temp_dir.path().join("current.json");

        fs::write(&backup_path, backup).unwrap();
        fs::write(&current_path, current).unwrap();

        let result = compare_json_content(
            backup_path.to_str().unwrap(),
            current_path.to_str().unwrap(),
        ).unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().contains("lodash added"));
    }

    #[test]
    fn test_compare_json_content_removed_dependency() {
        let temp_dir = TempDir::new().unwrap();

        let backup = r#"{"dependencies": {"react": "^18.0.0", "lodash": "^4.0.0"}}"#;
        let current = r#"{"dependencies": {"react": "^18.0.0"}}"#;

        let backup_path = temp_dir.path().join("backup.json");
        let current_path = temp_dir.path().join("current.json");

        fs::write(&backup_path, backup).unwrap();
        fs::write(&current_path, current).unwrap();

        let result = compare_json_content(
            backup_path.to_str().unwrap(),
            current_path.to_str().unwrap(),
        ).unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().contains("lodash removed"));
    }

    #[test]
    fn test_compare_json_content_changed_version() {
        let temp_dir = TempDir::new().unwrap();

        let backup = r#"{"dependencies": {"react": "^17.0.0"}}"#;
        let current = r#"{"dependencies": {"react": "^18.0.0"}}"#;

        let backup_path = temp_dir.path().join("backup.json");
        let current_path = temp_dir.path().join("current.json");

        fs::write(&backup_path, backup).unwrap();
        fs::write(&current_path, current).unwrap();

        let result = compare_json_content(
            backup_path.to_str().unwrap(),
            current_path.to_str().unwrap(),
        ).unwrap();
        assert!(result.is_some());
        let details = result.unwrap();
        assert!(details.contains("react"));
        assert!(details.contains("^17.0.0"));
        assert!(details.contains("^18.0.0"));
    }

    #[test]
    fn test_compare_json_content_dev_dependencies() {
        let temp_dir = TempDir::new().unwrap();

        let backup = r#"{"devDependencies": {"typescript": "^5.0.0"}}"#;
        let current = r#"{"devDependencies": {}}"#;

        let backup_path = temp_dir.path().join("backup.json");
        let current_path = temp_dir.path().join("current.json");

        fs::write(&backup_path, backup).unwrap();
        fs::write(&current_path, current).unwrap();

        let result = compare_json_content(
            backup_path.to_str().unwrap(),
            current_path.to_str().unwrap(),
        ).unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().contains("typescript (dev) removed"));
    }

    #[test]
    fn test_compare_json_content_non_dependency_change() {
        let temp_dir = TempDir::new().unwrap();

        let backup = r#"{"name": "old-name"}"#;
        let current = r#"{"name": "new-name"}"#;

        let backup_path = temp_dir.path().join("backup.json");
        let current_path = temp_dir.path().join("current.json");

        fs::write(&backup_path, backup).unwrap();
        fs::write(&current_path, current).unwrap();

        let result = compare_json_content(
            backup_path.to_str().unwrap(),
            current_path.to_str().unwrap(),
        ).unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().contains("non-dependency fields"));
    }

    #[test]
    fn test_hash_calculation() {
        // Test that SHA256 hash is calculated correctly
        let content = r#"{"name": "test"}"#;
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = format!("sha256:{:x}", hasher.finalize());

        assert!(hash.starts_with("sha256:"));
        assert_eq!(hash.len(), 7 + 64); // "sha256:" + 64 hex chars
    }

    #[test]
    fn test_snapshots_serialization_roundtrip() {
        let entry = SnapshotEntry {
            path: "package.json".to_string(),
            backup_path: ".airis/backups/package.json.2025-11-20.json".to_string(),
            format: "json".to_string(),
            captured_at: "2025-11-20T10:30:00Z".to_string(),
            hash: "sha256:abcd1234".to_string(),
        };

        let snapshots = Snapshots {
            snapshot: vec![entry.clone()],
        };

        // Serialize to TOML
        let toml_str = toml::to_string(&snapshots).unwrap();

        // Deserialize back
        let loaded: Snapshots = toml::from_str(&toml_str).unwrap();

        assert_eq!(loaded.snapshot.len(), 1);
        assert_eq!(loaded.snapshot[0].path, entry.path);
        assert_eq!(loaded.snapshot[0].backup_path, entry.backup_path);
        assert_eq!(loaded.snapshot[0].format, entry.format);
        assert_eq!(loaded.snapshot[0].captured_at, entry.captured_at);
        assert_eq!(loaded.snapshot[0].hash, entry.hash);
    }

    #[test]
    fn test_snapshots_empty_default() {
        let snapshots = Snapshots::default();
        assert!(snapshots.snapshot.is_empty());
    }

    #[test]
    fn test_snapshot_entry_clone() {
        let entry = SnapshotEntry {
            path: "test.json".to_string(),
            backup_path: ".airis/backups/test.json".to_string(),
            format: "json".to_string(),
            captured_at: "2025-11-20T10:30:00Z".to_string(),
            hash: "sha256:test".to_string(),
        };

        let cloned = entry.clone();
        assert_eq!(entry.path, cloned.path);
        assert_eq!(entry.hash, cloned.hash);
    }

    #[test]
    fn test_diff_type_debug() {
        let modified = DiffType::Modified;
        let deleted = DiffType::Deleted;

        // Verify Debug trait is implemented
        assert!(format!("{:?}", modified).contains("Modified"));
        assert!(format!("{:?}", deleted).contains("Deleted"));
    }

    #[test]
    fn test_snapshot_diff_debug() {
        let diff = SnapshotDiff {
            path: "test.json".to_string(),
            diff_type: DiffType::Modified,
            details: "Content changed".to_string(),
        };

        // Verify Debug trait is implemented
        let debug_str = format!("{:?}", diff);
        assert!(debug_str.contains("test.json"));
        assert!(debug_str.contains("Modified"));
    }
}
