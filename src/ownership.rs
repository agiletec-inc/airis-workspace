//! File ownership model for airis init
//!
//! Defines who "owns" each file type and how airis should handle them:
//! - Tool: fully managed by airis, safe to overwrite
//! - Hybrid: partially managed, merge specific fields only
//! - User: never touch, owned by the user

use std::path::Path;

/// Ownership level for a file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
    /// Fully managed by airis - safe to backup and overwrite
    Tool,
    /// Partially managed - merge specific fields, preserve others
    Hybrid,
    /// User-owned - never modify or delete
    User,
}

/// Get the ownership level for a given file path
pub fn get_ownership(path: &Path) -> Ownership {
    let path_str = path.to_string_lossy();

    // Exact matches first
    match path_str.as_ref() {
        // Tool-owned: fully generated from manifest
        "pnpm-workspace.yaml" => Ownership::Tool,
        ".github/workflows/ci.yml" => Ownership::Tool,
        ".github/workflows/release.yml" => Ownership::Tool,

        // Hybrid: partially managed
        "package.json" => Ownership::Hybrid,

        // User-owned: never touch
        "manifest.toml" => Ownership::User,
        "pnpm-lock.yaml" => Ownership::User,
        "tsconfig.json" => Ownership::User,
        "tsconfig.base.json" => Ownership::User,
        "tsconfig.build.json" => Ownership::User,
        "eslint.config.mjs" => Ownership::User,
        "vitest.config.ts" => Ownership::User,
        "README.md" => Ownership::User,
        "CHANGELOG.md" => Ownership::User,
        "PROJECT_INDEX.md" => Ownership::User,

        _ => {
            // Pattern matches
            if path_str.starts_with(".github/workflows/") {
                Ownership::Tool
            } else if path_str.starts_with("apps/") && path_str.ends_with("/package.json") {
                Ownership::Hybrid
            } else if path_str.starts_with("libs/") && path_str.ends_with("/package.json") {
                Ownership::Hybrid
            } else if path_str.starts_with("workspace/") {
                // workspace/ configs are user-owned for now
                // Future: could be tool-owned when docker-compose generation is added
                Ownership::User
            } else if path_str.starts_with("traefik/") {
                Ownership::User
            } else if path_str.starts_with("types/") {
                Ownership::User
            } else if path_str.starts_with(".airis/") {
                // .airis internal files are tool-owned
                Ownership::Tool
            } else {
                // Default: user-owned (safe default)
                Ownership::User
            }
        }
    }
}

/// Check if a file should be backed up before modification
#[allow(dead_code)]
pub fn should_backup(ownership: Ownership) -> bool {
    matches!(ownership, Ownership::Tool | Ownership::Hybrid)
}

/// Check if airis can overwrite a file
#[allow(dead_code)]
pub fn can_overwrite(ownership: Ownership) -> bool {
    matches!(ownership, Ownership::Tool | Ownership::Hybrid)
}

/// Check if airis should fully regenerate a file (vs merge)
#[allow(dead_code)]
pub fn should_regenerate(ownership: Ownership) -> bool {
    matches!(ownership, Ownership::Tool)
}

#[cfg(test)]
mod tests {
    use super::*;
    

    #[test]
    fn test_tool_owned_files() {
        assert_eq!(get_ownership(Path::new("pnpm-workspace.yaml")), Ownership::Tool);
        assert_eq!(get_ownership(Path::new(".github/workflows/ci.yml")), Ownership::Tool);
        assert_eq!(get_ownership(Path::new(".github/workflows/release.yml")), Ownership::Tool);
    }

    #[test]
    fn test_hybrid_files() {
        assert_eq!(get_ownership(Path::new("package.json")), Ownership::Hybrid);
        assert_eq!(get_ownership(Path::new("apps/dashboard/package.json")), Ownership::Hybrid);
        assert_eq!(get_ownership(Path::new("libs/ui/package.json")), Ownership::Hybrid);
    }

    #[test]
    fn test_user_owned_files() {
        assert_eq!(get_ownership(Path::new("manifest.toml")), Ownership::User);
        assert_eq!(get_ownership(Path::new("pnpm-lock.yaml")), Ownership::User);
        assert_eq!(get_ownership(Path::new("tsconfig.json")), Ownership::User);
        assert_eq!(get_ownership(Path::new("eslint.config.mjs")), Ownership::User);
    }

    #[test]
    fn test_default_is_user() {
        // Unknown files default to user-owned for safety
        assert_eq!(get_ownership(Path::new("random-file.txt")), Ownership::User);
        assert_eq!(get_ownership(Path::new("some/nested/path.json")), Ownership::User);
    }

    #[test]
    fn test_backup_logic() {
        assert!(should_backup(Ownership::Tool));
        assert!(should_backup(Ownership::Hybrid));
        assert!(!should_backup(Ownership::User));
    }
}
