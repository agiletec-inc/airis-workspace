// Directory-level sync engine for ~/.claude/ managed directories.
// Two modes:
// 1. Template-based: overwrite semantics for embedded templates (used for initial setup)
// 2. Source-based: registry-tracked sync from source directory (runtime, no rebuild needed)

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::templates::{ManagedDir, TemplateFile};

/// Result of syncing a managed directory (used by tests)
#[allow(dead_code)]
pub struct SyncResult {
    pub written: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
    pub unchanged: Vec<PathBuf>,
}

/// Sync a managed directory: write all template files, delete orphans. (used by tests)
#[allow(dead_code)]
pub fn sync_managed_dir(claude_home: &Path, managed: &ManagedDir) -> Result<SyncResult> {
    let target_dir = claude_home.join(managed.rel_dir);
    fs::create_dir_all(&target_dir)
        .with_context(|| format!("Failed to create {}", target_dir.display()))?;

    let mut result = SyncResult {
        written: Vec::new(),
        deleted: Vec::new(),
        unchanged: Vec::new(),
    };

    // Collect expected filenames
    let expected: HashSet<&str> = managed.files.iter().map(|f| f.rel_path).collect();

    // Phase 1: Delete orphans (files in dir not in template set)
    if let Ok(entries) = fs::read_dir(&target_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if !expected.contains(name_str.as_ref()) {
                    fs::remove_file(&path)
                        .with_context(|| format!("Failed to remove orphan {}", path.display()))?;
                    result.deleted.push(path);
                }
            }
        }
    }

    // Phase 2: Write all template files
    for file in managed.files {
        let target_path = target_dir.join(file.rel_path);
        let needs_write = if target_path.exists() {
            let existing = fs::read_to_string(&target_path).unwrap_or_default();
            existing != file.content
        } else {
            true
        };

        if needs_write {
            fs::write(&target_path, file.content)
                .with_context(|| format!("Failed to write {}", target_path.display()))?;
            result.written.push(target_path);
        } else {
            result.unchanged.push(target_path);
        }
    }

    Ok(result)
}

// ── Source-based sync (registry-tracked) ────────────────────────────

/// Result of syncing from a source directory
pub struct SourceSyncResult {
    pub written: Vec<String>,
    pub deleted: Vec<String>,
    pub unchanged: Vec<String>,
}

/// Load the list of previously synced files from the registry
pub fn load_claude_registry(path: &Path) -> Vec<String> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    content
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect()
}

/// Save the current list of synced files to the registry
pub fn save_claude_registry(path: &Path, paths: &[String]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut sorted = paths.to_vec();
    sorted.sort();
    sorted.dedup();
    let content = format!(
        "# Auto-managed by airis — do not edit\n# Tracks files synced to ~/.claude/ from source directory\n{}\n",
        sorted.join("\n")
    );
    fs::write(path, content).context("Failed to write claude registry")?;
    Ok(())
}

/// Write a file only if content has changed. Returns true if written.
fn write_if_changed(target: &Path, content: &str) -> Result<bool> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    if target.exists() {
        let existing = fs::read_to_string(target).unwrap_or_default();
        if existing == content {
            return Ok(false);
        }
    }
    fs::write(target, content).with_context(|| format!("Failed to write {}", target.display()))?;
    Ok(true)
}

/// Sync from source directory to ~/.claude/ using registry-based tracking.
/// Only manages files listed in the registry. Never touches other files.
pub fn sync_from_source(
    source_dir: &Path,
    claude_home: &Path,
    registry_path: &Path,
) -> Result<SourceSyncResult> {
    let previous = load_claude_registry(registry_path);
    let mut current: Vec<String> = Vec::new();
    let mut result = SourceSyncResult {
        written: Vec::new(),
        deleted: Vec::new(),
        unchanged: Vec::new(),
    };

    // Sync CLAUDE.md
    let claude_md_source = source_dir.join("CLAUDE.md");
    if claude_md_source.exists() {
        let content = fs::read_to_string(&claude_md_source)
            .with_context(|| format!("Failed to read {}", claude_md_source.display()))?;
        let target = claude_home.join("CLAUDE.md");
        if write_if_changed(&target, &content)? {
            result.written.push("CLAUDE.md".to_string());
        } else {
            result.unchanged.push("CLAUDE.md".to_string());
        }
        current.push("CLAUDE.md".to_string());
    }

    // Sync rules/*.md
    let rules_source = source_dir.join("rules");
    if rules_source.exists() {
        fs::create_dir_all(claude_home.join("rules"))?;
        for entry in fs::read_dir(&rules_source)
            .with_context(|| format!("Failed to read {}", rules_source.display()))?
            .flatten()
        {
            if entry.path().is_file() {
                let name = entry.file_name().to_string_lossy().to_string();
                let content = fs::read_to_string(entry.path())?;
                let rel = format!("rules/{}", name);
                let target = claude_home.join(&rel);
                if write_if_changed(&target, &content)? {
                    result.written.push(rel.clone());
                } else {
                    result.unchanged.push(rel.clone());
                }
                current.push(rel);
            }
        }
    }

    // Remove orphans: files in previous registry but not in current source
    let current_set: HashSet<&str> = current.iter().map(|s| s.as_str()).collect();
    for prev in &previous {
        if !current_set.contains(prev.as_str()) {
            let target = claude_home.join(prev);
            if target.exists() {
                fs::remove_file(&target)
                    .with_context(|| format!("Failed to remove orphan {}", target.display()))?;
                result.deleted.push(prev.clone());
            }
        }
    }

    save_claude_registry(registry_path, &current)?;
    Ok(result)
}

// ── Template-based sync (compile-time embedded) ─────────────────────

/// Sync a single managed file (e.g., CLAUDE.md). Returns true if written, false if unchanged. (used by tests)
#[allow(dead_code)]
pub fn sync_single_file(claude_home: &Path, file: &TemplateFile) -> Result<bool> {
    let target_path = claude_home.join(file.rel_path);

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let needs_write = if target_path.exists() {
        let existing = fs::read_to_string(&target_path).unwrap_or_default();
        existing != file.content
    } else {
        true
    };

    if needs_write {
        fs::write(&target_path, file.content)
            .with_context(|| format!("Failed to write {}", target_path.display()))?;
    }

    Ok(needs_write)
}
