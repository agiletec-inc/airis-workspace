//! pnpm-lock.yaml v9 parser and workspace dependency resolver
//!
//! Parses pnpm-lock.yaml to extract workspace dependencies for DAG construction.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// pnpm-lock.yaml v9 structure (minimal for dependency resolution)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PnpmLock {
    pub lockfile_version: String,
    #[serde(default)]
    pub importers: HashMap<String, Importer>,
}

/// An importer is a workspace package
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Importer {
    #[serde(default)]
    pub dependencies: HashMap<String, Dependency>,
    #[serde(default)]
    pub dev_dependencies: HashMap<String, Dependency>,
    #[serde(default)]
    #[allow(dead_code)]
    pub optional_dependencies: HashMap<String, Dependency>,
    #[serde(default)]
    pub peer_dependencies: HashMap<String, Dependency>,
}

/// A dependency entry
#[derive(Debug, Deserialize)]
pub struct Dependency {
    #[allow(dead_code)]
    pub specifier: String,
    pub version: String,
}

/// Resolved workspace info (path is stored as HashMap key, not duplicated here)
#[derive(Debug, Clone)]
pub struct WorkspacePackage {
    pub name: String,
    pub workspace_deps: Vec<String>, // names of workspace packages this depends on
}

impl PnpmLock {
    /// Load from pnpm-lock.yaml
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let lock: PnpmLock = serde_yaml::from_str(&content)
            .with_context(|| "Failed to parse pnpm-lock.yaml")?;

        if !lock.lockfile_version.starts_with("9.") {
            anyhow::bail!(
                "Unsupported lockfile version: {}. Only v9.x is supported.",
                lock.lockfile_version
            );
        }

        Ok(lock)
    }

    /// Extract workspace dependencies for a given importer path
    /// Returns package paths that are workspace links
    pub fn get_workspace_deps(&self, importer_path: &str) -> Vec<String> {
        let Some(importer) = self.importers.get(importer_path) else {
            return vec![];
        };

        let mut deps = Vec::new();

        // Check all dependency types
        for dep in importer.dependencies.values() {
            if let Some(path) = self.resolve_workspace_link(importer_path, &dep.version) {
                deps.push(path);
            }
        }
        for dep in importer.dev_dependencies.values() {
            if let Some(path) = self.resolve_workspace_link(importer_path, &dep.version) {
                deps.push(path);
            }
        }
        for dep in importer.peer_dependencies.values() {
            if let Some(path) = self.resolve_workspace_link(importer_path, &dep.version) {
                deps.push(path);
            }
        }

        deps
    }

    /// Resolve workspace link relative to importer path
    /// e.g., importer="libs/supabase/client", version="link:../types" -> "libs/supabase/types"
    fn resolve_workspace_link(&self, importer_path: &str, version: &str) -> Option<String> {
        if !version.starts_with("link:") {
            return None;
        }

        let link_path = version.strip_prefix("link:")?;

        // Use std::path for proper path resolution
        use std::path::PathBuf;

        let base = PathBuf::from(importer_path);
        let resolved = base.join(link_path);

        // Normalize the path (resolve .. and .)
        let mut components = Vec::new();
        for component in resolved.components() {
            match component {
                std::path::Component::ParentDir => {
                    components.pop();
                }
                std::path::Component::Normal(s) => {
                    components.push(s.to_string_lossy().to_string());
                }
                std::path::Component::CurDir => {}
                _ => {}
            }
        }

        Some(components.join("/"))
    }

}

/// Build workspace package map from lockfile
/// Returns: path -> WorkspacePackage
pub fn build_workspace_map(lock: &PnpmLock) -> HashMap<String, WorkspacePackage> {
    let mut map = HashMap::new();

    for path in lock.importers.keys() {
        if path == "." {
            continue; // Skip root
        }

        // Extract package name from dependencies (the key in the deps map)
        // For workspace packages, we need to find the name from package.json
        // For now, derive from path: apps/focustoday-api -> focustoday-api
        let name = path
            .rsplit('/')
            .next()
            .unwrap_or(path)
            .to_string();

        let workspace_deps = lock.get_workspace_deps(path);

        map.insert(
            path.clone(),
            WorkspacePackage {
                name,
                workspace_deps,
            },
        );
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_workspace_link() {
        let lock = PnpmLock {
            lockfile_version: "9.0".to_string(),
            importers: HashMap::new(),
        };

        // apps/focustoday-api depends on link:../../libs/env-config
        assert_eq!(
            lock.resolve_workspace_link("apps/focustoday-api", "link:../../libs/env-config"),
            Some("libs/env-config".to_string())
        );

        // libs/supabase/client depends on link:../types
        assert_eq!(
            lock.resolve_workspace_link("libs/supabase/client", "link:../types"),
            Some("libs/supabase/types".to_string())
        );

        // Non-link versions return None
        assert_eq!(lock.resolve_workspace_link("apps/foo", "1.2.3"), None);
        assert_eq!(lock.resolve_workspace_link("apps/foo", "workspace:*"), None);
    }
}
