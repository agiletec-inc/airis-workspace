// Template definitions for global Claude Code configuration files.
// Content is embedded at compile time via include_str!().
// Used for initial setup when ~/.airis/claude/ doesn't exist yet.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

/// A template file to sync to ~/.claude/
pub struct TemplateFile {
    /// Relative path within ~/.claude/ (e.g., "rules/docker-first.md")
    pub rel_path: &'static str,
    /// File content
    pub content: &'static str,
}

/// A directory that airis fully owns inside ~/.claude/.
/// All files in this directory are airis-managed — orphans will be deleted.
pub struct ManagedDir {
    /// Path relative to ~/.claude/ (e.g., "rules")
    pub rel_dir: &'static str,
    /// Files in this directory
    pub files: &'static [TemplateFile],
}

/// Global ~/.claude/CLAUDE.md
pub fn global_claude_md() -> TemplateFile {
    TemplateFile {
        rel_path: "CLAUDE.md",
        content: include_str!("../../../templates/claude/CLAUDE.md"),
    }
}

/// All managed directories inside ~/.claude/
pub fn managed_dirs() -> Vec<ManagedDir> {
    vec![ManagedDir {
        rel_dir: "rules",
        files: &[
            TemplateFile {
                rel_path: "docker-first.md",
                content: include_str!("../../../templates/claude/rules/docker-first.md"),
            },
            TemplateFile {
                rel_path: "server-access.md",
                content: include_str!("../../../templates/claude/rules/server-access.md"),
            },
        ],
    }]
}

/// Initialize source directory from embedded templates.
/// Called when the source directory (e.g., ~/.airis/claude/) doesn't exist yet.
pub fn initialize_source_dir(source_dir: &Path) -> Result<()> {
    fs::create_dir_all(source_dir.join("rules"))
        .with_context(|| format!("Failed to create {}/rules", source_dir.display()))?;

    // Write CLAUDE.md
    let claude_md = global_claude_md();
    fs::write(source_dir.join("CLAUDE.md"), claude_md.content)
        .with_context(|| format!("Failed to write {}/CLAUDE.md", source_dir.display()))?;

    // Write rule files
    for managed in managed_dirs() {
        let dir = source_dir.join(managed.rel_dir);
        for file in managed.files {
            fs::write(dir.join(file.rel_path), file.content).with_context(|| {
                format!(
                    "Failed to write {}/{}/{}",
                    source_dir.display(),
                    managed.rel_dir,
                    file.rel_path
                )
            })?;
        }
    }

    Ok(())
}
