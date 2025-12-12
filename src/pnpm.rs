//! pnpm-lock.yaml v9 parser and workspace dependency resolver
//!
//! Parses pnpm-lock.yaml to extract workspace dependencies for DAG construction.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
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
    pub optional_dependencies: HashMap<String, Dependency>,
    #[serde(default)]
    pub peer_dependencies: HashMap<String, Dependency>,
}

/// A dependency entry
#[derive(Debug, Deserialize)]
pub struct Dependency {
    pub specifier: String,
    pub version: String,
}

/// pnpm-workspace.yaml structure
#[derive(Debug, Deserialize)]
pub struct PnpmWorkspace {
    pub packages: Vec<String>,
}

/// Resolved workspace info
#[derive(Debug, Clone)]
pub struct WorkspacePackage {
    pub name: String,
    pub path: String,
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

    /// Get all workspace package paths from importers
    pub fn get_all_workspace_paths(&self) -> Vec<String> {
        self.importers
            .keys()
            .filter(|k| *k != ".")
            .cloned()
            .collect()
    }
}

impl PnpmWorkspace {
    /// Load from pnpm-workspace.yaml
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let ws: PnpmWorkspace = serde_yaml::from_str(&content)
            .with_context(|| "Failed to parse pnpm-workspace.yaml")?;

        Ok(ws)
    }
}

/// Build workspace package map from lockfile
/// Returns: path -> WorkspacePackage
pub fn build_workspace_map(lock: &PnpmLock) -> HashMap<String, WorkspacePackage> {
    let mut map = HashMap::new();

    for (path, importer) in &lock.importers {
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
                path: path.clone(),
                workspace_deps,
            },
        );
    }

    map
}

/// Resolve full dependency chain for a target package
/// Returns packages in topological order (dependencies first)
pub fn resolve_deps_order(
    target_path: &str,
    workspace_map: &HashMap<String, WorkspacePackage>,
) -> Result<Vec<String>> {
    let mut visited = HashSet::new();
    let mut order = Vec::new();

    fn visit(
        path: &str,
        workspace_map: &HashMap<String, WorkspacePackage>,
        visited: &mut HashSet<String>,
        order: &mut Vec<String>,
        stack: &mut HashSet<String>,
    ) -> Result<()> {
        if visited.contains(path) {
            return Ok(());
        }

        if stack.contains(path) {
            anyhow::bail!("Circular dependency detected: {}", path);
        }

        stack.insert(path.to_string());

        if let Some(pkg) = workspace_map.get(path) {
            for dep_path in &pkg.workspace_deps {
                visit(dep_path, workspace_map, visited, order, stack)?;
            }
        }

        stack.remove(path);
        visited.insert(path.to_string());
        order.push(path.to_string());

        Ok(())
    }

    let mut stack = HashSet::new();
    visit(target_path, workspace_map, &mut visited, &mut order, &mut stack)?;

    Ok(order)
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
