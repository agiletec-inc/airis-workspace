// Directory-level sync engine for ~/.claude/ managed directories.
// Implements overwrite semantics: write all template files, delete orphans.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::templates::{ManagedDir, TemplateFile};

/// Result of syncing a managed directory
pub struct SyncResult {
    pub written: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
    pub unchanged: Vec<PathBuf>,
}

/// Sync a managed directory: write all template files, delete orphans.
///
/// Algorithm:
/// 1. Create target directory if needed
/// 2. Delete any files NOT in the template set (orphans)
/// 3. Write all template files (skip if content unchanged)
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

/// Sync a single managed file (e.g., CLAUDE.md). Returns true if written, false if unchanged.
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
