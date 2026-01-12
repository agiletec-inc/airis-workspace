//! Safe filesystem operations with workspace boundary enforcement
//!
//! This module provides safe wrappers around filesystem operations that:
//! - Enforce workspace boundaries (never operate outside workspace root)
//! - Create automatic backups before destructive operations
//! - Respect file ownership rules
//! - Support dry-run mode for previewing changes
//!
//! # Design Philosophy
//!
//! "Never delete what you can't restore"
//!
//! All destructive operations (delete, overwrite) automatically create backups
//! in `.airis/backups/` before proceeding. Users can always recover their data.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::Local;

use crate::ownership::{get_ownership, Ownership};

/// Backup directory relative to workspace root
const BACKUP_DIR: &str = ".airis/backups";

/// Maximum backup age in days (older backups can be cleaned)
#[allow(dead_code)]
const BACKUP_MAX_AGE_DAYS: u32 = 30;

/// Result of a safe filesystem operation
#[derive(Debug)]
pub struct SafeOpResult {
    /// What was done (or would be done in dry-run)
    pub action: SafeAction,
    /// Path that was affected
    pub path: PathBuf,
    /// Backup path if a backup was created
    pub backup: Option<PathBuf>,
}

/// Type of action performed
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafeAction {
    /// File was created (didn't exist before)
    Created,
    /// File was overwritten (backup created)
    Overwritten,
    /// File was deleted (backup created)
    Deleted,
    /// File was skipped (user-owned or other reason)
    Skipped(String),
    /// Dry-run: shows what would happen
    WouldCreate,
    WouldOverwrite,
    WouldDelete,
}

/// Safe filesystem context bound to a workspace root
pub struct SafeFS {
    /// Workspace root directory (absolute path)
    root: PathBuf,
    /// Whether to actually perform operations or just preview
    dry_run: bool,
}

impl SafeFS {
    /// Create a new SafeFS context for the given workspace root
    ///
    /// # Arguments
    /// * `root` - Workspace root directory (must contain manifest.toml)
    /// * `dry_run` - If true, operations are simulated but not performed
    ///
    /// # Errors
    /// Returns error if root doesn't exist or isn't a valid workspace
    pub fn new(root: impl AsRef<Path>, dry_run: bool) -> Result<Self> {
        let root = root.as_ref();

        // Canonicalize to absolute path
        let root = root.canonicalize()
            .with_context(|| format!("Workspace root not found: {}", root.display()))?;

        // Verify it's a valid workspace (has manifest.toml)
        if !root.join("manifest.toml").exists() {
            bail!(
                "Not a valid airis workspace: {} (missing manifest.toml)",
                root.display()
            );
        }

        Ok(Self { root, dry_run })
    }

    /// Create a SafeFS for the current directory
    pub fn current(dry_run: bool) -> Result<Self> {
        Self::new(".", dry_run)
    }

    /// Get the workspace root path
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Check if dry-run mode is enabled
    pub fn is_dry_run(&self) -> bool {
        self.dry_run
    }

    /// Resolve a path relative to workspace root and validate it's within bounds
    ///
    /// # Security
    /// This is the core security function. It ensures:
    /// 1. The resolved path is absolute
    /// 2. The resolved path is within the workspace root
    /// 3. No symlink escapes are possible
    fn resolve_and_validate(&self, path: impl AsRef<Path>) -> Result<PathBuf> {
        let path = path.as_ref();

        // If path is absolute, use it directly; otherwise, join with root
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };

        // For existing paths, canonicalize to resolve symlinks
        // For non-existing paths, canonicalize parent and append filename
        let canonical = if full_path.exists() {
            full_path.canonicalize()?
        } else {
            // Path doesn't exist yet - canonicalize parent
            let parent = full_path.parent()
                .ok_or_else(|| anyhow::anyhow!("Invalid path: no parent"))?;

            if parent.exists() {
                let canonical_parent = parent.canonicalize()?;
                let filename = full_path.file_name()
                    .ok_or_else(|| anyhow::anyhow!("Invalid path: no filename"))?;
                canonical_parent.join(filename)
            } else {
                // Parent doesn't exist either - this is fine for new paths
                // Just ensure we don't have any .. in the path
                if path.to_string_lossy().contains("..") {
                    bail!(
                        "Path traversal rejected: {} (contains '..')",
                        path.display()
                    );
                }
                full_path
            }
        };

        // CRITICAL: Ensure path is within workspace root
        if !canonical.starts_with(&self.root) {
            bail!(
                "Operation rejected: path '{}' is outside workspace '{}'",
                canonical.display(),
                self.root.display()
            );
        }

        Ok(canonical)
    }

    /// Get the backup directory path
    fn backup_dir(&self) -> PathBuf {
        self.root.join(BACKUP_DIR)
    }

    /// Create a backup of a file before modifying it
    ///
    /// Returns the backup path if successful
    fn create_backup(&self, path: &Path) -> Result<PathBuf> {
        if !path.exists() {
            bail!("Cannot backup non-existent file: {}", path.display());
        }

        let backup_dir = self.backup_dir();
        fs::create_dir_all(&backup_dir)
            .with_context(|| format!("Failed to create backup directory: {}", backup_dir.display()))?;

        // Create timestamped backup filename
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let relative_path = path.strip_prefix(&self.root).unwrap_or(path);
        let backup_name = format!(
            "{}.{}.bak",
            relative_path.to_string_lossy().replace('/', "_"),
            timestamp
        );
        let backup_path = backup_dir.join(&backup_name);

        // Copy file to backup location
        if path.is_dir() {
            copy_dir_recursive(path, &backup_path)?;
        } else {
            fs::copy(path, &backup_path)
                .with_context(|| format!("Failed to create backup: {}", backup_path.display()))?;
        }

        Ok(backup_path)
    }

    /// Write content to a file safely
    ///
    /// # Behavior
    /// - Creates parent directories if needed
    /// - Creates backup if file exists and will be overwritten
    /// - Respects ownership rules
    /// - In dry-run mode, only reports what would happen
    pub fn write(
        &self,
        path: impl AsRef<Path>,
        content: impl AsRef<[u8]>,
    ) -> Result<SafeOpResult> {
        let path = self.resolve_and_validate(path)?;
        let relative_path = path.strip_prefix(&self.root).unwrap_or(&path);
        let ownership = get_ownership(relative_path);

        // Check if file exists
        let exists = path.exists();

        // Check ownership rules
        if exists && ownership == Ownership::User {
            return Ok(SafeOpResult {
                action: SafeAction::Skipped(format!(
                    "User-owned file (use --force to override): {}",
                    relative_path.display()
                )),
                path,
                backup: None,
            });
        }

        if self.dry_run {
            return Ok(SafeOpResult {
                action: if exists { SafeAction::WouldOverwrite } else { SafeAction::WouldCreate },
                path,
                backup: None,
            });
        }

        // Create parent directories
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        // Create backup if overwriting
        let backup = if exists {
            Some(self.create_backup(&path)?)
        } else {
            None
        };

        // Write the file
        fs::write(&path, content)
            .with_context(|| format!("Failed to write file: {}", path.display()))?;

        Ok(SafeOpResult {
            action: if exists { SafeAction::Overwritten } else { SafeAction::Created },
            path,
            backup,
        })
    }

    /// Delete a file or directory safely
    ///
    /// # Behavior
    /// - Creates backup before deletion
    /// - Respects ownership rules (won't delete user-owned files)
    /// - In dry-run mode, only reports what would happen
    pub fn delete(&self, path: impl AsRef<Path>) -> Result<SafeOpResult> {
        let path = self.resolve_and_validate(path)?;
        let relative_path = path.strip_prefix(&self.root).unwrap_or(&path);
        let ownership = get_ownership(relative_path);

        // Check if path exists
        if !path.exists() {
            return Ok(SafeOpResult {
                action: SafeAction::Skipped("File does not exist".to_string()),
                path,
                backup: None,
            });
        }

        // Never delete user-owned files
        if ownership == Ownership::User {
            return Ok(SafeOpResult {
                action: SafeAction::Skipped(format!(
                    "User-owned file cannot be deleted: {}",
                    relative_path.display()
                )),
                path,
                backup: None,
            });
        }

        if self.dry_run {
            return Ok(SafeOpResult {
                action: SafeAction::WouldDelete,
                path,
                backup: None,
            });
        }

        // Create backup
        let backup = self.create_backup(&path)?;

        // Delete
        if path.is_dir() {
            fs::remove_dir_all(&path)
                .with_context(|| format!("Failed to delete directory: {}", path.display()))?;
        } else {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to delete file: {}", path.display()))?;
        }

        Ok(SafeOpResult {
            action: SafeAction::Deleted,
            path,
            backup: Some(backup),
        })
    }

    /// Delete a file or directory for cleaning (tool-generated artifacts only)
    ///
    /// This is a more permissive delete for cleaning build artifacts.
    /// It still validates paths but allows deletion of common build outputs.
    pub fn clean_artifact(&self, path: impl AsRef<Path>) -> Result<SafeOpResult> {
        let path = self.resolve_and_validate(path)?;

        // Check if path exists
        if !path.exists() {
            return Ok(SafeOpResult {
                action: SafeAction::Skipped("Does not exist".to_string()),
                path,
                backup: None,
            });
        }

        // For clean operations, we're more permissive but still protect key files
        let relative_path = path.strip_prefix(&self.root).unwrap_or(&path);
        let relative_str = relative_path.to_string_lossy();

        // Protect critical paths even in clean mode
        let protected = [
            "manifest.toml",
            "package.json",
            "pnpm-lock.yaml",
            "Cargo.toml",
            "Cargo.lock",
            ".git",
            ".env",
            "src",
            "apps",
            "libs",
        ];

        // Cleanable artifacts within protected paths (e.g., apps/*/node_modules)
        let cleanable_artifacts = ["node_modules", ".next", "dist", ".turbo"];

        // Check if this is a cleanable artifact within a protected path
        let is_cleanable_artifact = cleanable_artifacts.iter().any(|artifact| {
            relative_str.ends_with(artifact)
                || relative_str.contains(&format!("/{}/", artifact))
                || relative_str.ends_with(&format!("/{}", artifact))
        });

        // Only protect if it's not a cleanable artifact
        if !is_cleanable_artifact {
            for p in protected {
                if relative_str == p || relative_str.starts_with(&format!("{}/", p)) {
                    return Ok(SafeOpResult {
                        action: SafeAction::Skipped(format!("Protected path: {}", relative_str)),
                        path,
                        backup: None,
                    });
                }
            }
        }

        if self.dry_run {
            return Ok(SafeOpResult {
                action: SafeAction::WouldDelete,
                path,
                backup: None,
            });
        }

        // For artifacts, we don't backup (they're regenerable)
        if path.is_dir() {
            fs::remove_dir_all(&path)
                .with_context(|| format!("Failed to clean: {}", path.display()))?;
        } else {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to clean: {}", path.display()))?;
        }

        Ok(SafeOpResult {
            action: SafeAction::Deleted,
            path,
            backup: None,
        })
    }

    /// Check if a path is safe to operate on (within workspace)
    pub fn is_safe_path(&self, path: impl AsRef<Path>) -> bool {
        self.resolve_and_validate(path).is_ok()
    }

    /// List all backups
    #[allow(dead_code)]
    pub fn list_backups(&self) -> Result<Vec<PathBuf>> {
        let backup_dir = self.backup_dir();
        if !backup_dir.exists() {
            return Ok(Vec::new());
        }

        let mut backups = Vec::new();
        for entry in fs::read_dir(&backup_dir)? {
            let entry = entry?;
            if entry.path().extension().is_some_and(|ext| ext == "bak") {
                backups.push(entry.path());
            }
        }

        // Sort by modification time (newest first)
        backups.sort_by(|a, b| {
            let a_time = fs::metadata(a).and_then(|m| m.modified()).ok();
            let b_time = fs::metadata(b).and_then(|m| m.modified()).ok();
            b_time.cmp(&a_time)
        });

        Ok(backups)
    }
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_workspace() -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("manifest.toml"), "version = 1\n[workspace]\nname = \"test\"").unwrap();
        dir
    }

    #[test]
    fn test_safefs_rejects_outside_workspace() {
        let workspace = create_test_workspace();
        let safe_fs = SafeFS::new(workspace.path(), false).unwrap();

        // Absolute path outside workspace should be rejected
        assert!(safe_fs.resolve_and_validate("/etc/passwd").is_err());
        assert!(safe_fs.resolve_and_validate("/tmp/foo").is_err());

        // Path traversal should be rejected
        assert!(safe_fs.resolve_and_validate("../outside").is_err());
        assert!(safe_fs.resolve_and_validate("foo/../../outside").is_err());
    }

    #[test]
    fn test_safefs_allows_inside_workspace() {
        let workspace = create_test_workspace();
        let safe_fs = SafeFS::new(workspace.path(), false).unwrap();

        // Paths inside workspace should be allowed
        assert!(safe_fs.resolve_and_validate("src/main.rs").is_ok());
        assert!(safe_fs.resolve_and_validate("apps/dashboard/package.json").is_ok());
        assert!(safe_fs.resolve_and_validate(".next").is_ok());
    }

    #[test]
    fn test_safefs_write_creates_backup() {
        let workspace = create_test_workspace();
        let safe_fs = SafeFS::new(workspace.path(), false).unwrap();

        // Use a tool-owned file (pnpm-workspace.yaml) so it can be overwritten
        let test_file = workspace.path().join("pnpm-workspace.yaml");
        fs::write(&test_file, "original content").unwrap();

        // Overwrite with SafeFS
        let result = safe_fs.write("pnpm-workspace.yaml", "new content").unwrap();

        // Should have created a backup
        assert!(matches!(result.action, SafeAction::Overwritten));
        assert!(result.backup.is_some());
        assert!(result.backup.as_ref().unwrap().exists());

        // Backup should contain original content
        let backup_content = fs::read_to_string(result.backup.unwrap()).unwrap();
        assert_eq!(backup_content, "original content");

        // New file should have new content
        let new_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(new_content, "new content");
    }

    #[test]
    fn test_safefs_dry_run() {
        let workspace = create_test_workspace();
        let safe_fs = SafeFS::new(workspace.path(), true).unwrap();

        // Use a tool-owned file (pnpm-workspace.yaml)
        let test_file = workspace.path().join("pnpm-workspace.yaml");
        fs::write(&test_file, "original content").unwrap();

        // Try to overwrite in dry-run mode
        let result = safe_fs.write("pnpm-workspace.yaml", "new content").unwrap();

        // Should report would-overwrite but not actually change anything
        assert!(matches!(result.action, SafeAction::WouldOverwrite));
        assert!(result.backup.is_none());

        // File should still have original content
        let content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, "original content");
    }

    #[test]
    fn test_safefs_respects_user_ownership() {
        let workspace = create_test_workspace();
        let safe_fs = SafeFS::new(workspace.path(), false).unwrap();

        // Try to delete manifest.toml (user-owned)
        let result = safe_fs.delete("manifest.toml").unwrap();

        // Should be skipped
        assert!(matches!(result.action, SafeAction::Skipped(_)));

        // File should still exist
        assert!(workspace.path().join("manifest.toml").exists());
    }

    #[test]
    fn test_safefs_clean_artifact_protects_critical() {
        let workspace = create_test_workspace();
        let safe_fs = SafeFS::new(workspace.path(), false).unwrap();

        // Create src directory
        fs::create_dir_all(workspace.path().join("src")).unwrap();
        fs::write(workspace.path().join("src/main.rs"), "fn main() {}").unwrap();

        // Try to clean src (protected)
        let result = safe_fs.clean_artifact("src").unwrap();
        assert!(matches!(result.action, SafeAction::Skipped(_)));
        assert!(workspace.path().join("src").exists());

        // Try to clean manifest.toml (protected)
        let result = safe_fs.clean_artifact("manifest.toml").unwrap();
        assert!(matches!(result.action, SafeAction::Skipped(_)));
    }

    #[test]
    fn test_safefs_clean_artifact_allows_build_dirs() {
        let workspace = create_test_workspace();
        let safe_fs = SafeFS::new(workspace.path(), false).unwrap();

        // Create build artifact directories
        fs::create_dir_all(workspace.path().join(".next")).unwrap();
        fs::create_dir_all(workspace.path().join("node_modules")).unwrap();
        fs::create_dir_all(workspace.path().join("dist")).unwrap();

        // These should be cleanable
        let result = safe_fs.clean_artifact(".next").unwrap();
        assert!(matches!(result.action, SafeAction::Deleted));

        let result = safe_fs.clean_artifact("node_modules").unwrap();
        assert!(matches!(result.action, SafeAction::Deleted));

        let result = safe_fs.clean_artifact("dist").unwrap();
        assert!(matches!(result.action, SafeAction::Deleted));
    }

    #[test]
    fn test_safefs_clean_artifact_allows_node_modules_in_protected_paths() {
        let workspace = create_test_workspace();
        let safe_fs = SafeFS::new(workspace.path(), false).unwrap();

        // Create apps and libs directories with node_modules inside
        fs::create_dir_all(workspace.path().join("apps/my-app/node_modules")).unwrap();
        fs::create_dir_all(workspace.path().join("libs/my-lib/node_modules")).unwrap();
        fs::create_dir_all(workspace.path().join("apps/my-app/.next")).unwrap();

        // node_modules inside apps should be cleanable
        let result = safe_fs.clean_artifact("apps/my-app/node_modules").unwrap();
        assert!(matches!(result.action, SafeAction::Deleted));
        assert!(!workspace.path().join("apps/my-app/node_modules").exists());

        // node_modules inside libs should be cleanable
        let result = safe_fs.clean_artifact("libs/my-lib/node_modules").unwrap();
        assert!(matches!(result.action, SafeAction::Deleted));
        assert!(!workspace.path().join("libs/my-lib/node_modules").exists());

        // .next inside apps should be cleanable
        let result = safe_fs.clean_artifact("apps/my-app/.next").unwrap();
        assert!(matches!(result.action, SafeAction::Deleted));
        assert!(!workspace.path().join("apps/my-app/.next").exists());

        // But apps/my-app itself should still be protected
        fs::create_dir_all(workspace.path().join("apps/my-app/src")).unwrap();
        let result = safe_fs.clean_artifact("apps/my-app/src").unwrap();
        assert!(matches!(result.action, SafeAction::Skipped(_)));
        assert!(workspace.path().join("apps/my-app/src").exists());
    }
}
