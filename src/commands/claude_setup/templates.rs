// Template definitions for global Claude Code configuration files.
// Content is embedded at compile time via include_str!().

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
