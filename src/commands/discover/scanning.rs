//! Workspace scanning for apps and libraries.

use anyhow::{Context, Result};
use indexmap::IndexMap;
use std::fs;
use std::path::Path;

use super::catalog::extract_package_info;
use super::detection::{detect_framework, get_package_name};
use super::types::{DetectedApp, DetectedLib, DiscoveredProject};

/// Discover projects from workspace glob patterns (e.g., "apps/*", "libs/*", "products/**").
///
/// Scans directories matching the patterns, detects framework from package.json/Cargo.toml.
/// Excludes negated patterns (starting with "!") and non-project directories.
pub fn discover_from_workspaces(
    patterns: &[String],
    workspace_root: &Path,
) -> Result<Vec<DiscoveredProject>> {
    let mut projects = Vec::new();
    let mut seen_paths = std::collections::HashSet::new();

    // Separate include and exclude patterns
    let exclude_patterns: Vec<&str> = patterns
        .iter()
        .filter(|p| p.starts_with('!'))
        .map(|p| p.trim_start_matches('!'))
        .collect();

    for pattern in patterns {
        if pattern.starts_with('!') {
            continue;
        }

        // Resolve glob pattern
        let full_pattern = workspace_root.join(pattern);
        let full_pattern_str = full_pattern.to_string_lossy();

        let entries = match glob::glob(&full_pattern_str) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            if !entry.is_dir() {
                continue;
            }

            // Get relative path
            let rel_path = entry
                .strip_prefix(workspace_root)
                .unwrap_or(&entry)
                .to_string_lossy()
                .to_string();

            // Check exclusion patterns
            if exclude_patterns
                .iter()
                .any(|ex| glob::Pattern::new(ex).is_ok_and(|p| p.matches(&rel_path)))
            {
                continue;
            }

            // Skip build artifacts and dependency directories
            if rel_path.contains("node_modules")
                || rel_path.contains(".next")
                || rel_path.contains(".pnpm")
                || rel_path.contains("dist/")
                || rel_path.contains("build/")
                || rel_path.contains(".turbo")
            {
                continue;
            }

            // Skip if already seen (overlapping patterns)
            if !seen_paths.insert(rel_path.clone()) {
                continue;
            }

            // Must have package.json or Cargo.toml to be a project
            if !entry.join("package.json").exists() && !entry.join("Cargo.toml").exists() {
                continue;
            }

            let name = entry
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let framework = detect_framework(&entry);

            projects.push(DiscoveredProject {
                name,
                path: rel_path,
                framework,
            });
        }
    }

    // Sort by path for consistent output
    projects.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(projects)
}

/// Scan apps/ directory for applications
pub fn scan_apps(catalog: &IndexMap<String, String>) -> Result<Vec<DetectedApp>> {
    let mut apps = Vec::new();
    let apps_dir = Path::new("apps");

    if !apps_dir.exists() {
        return Ok(apps);
    }

    let entries = fs::read_dir(apps_dir).context("Failed to read apps/ directory")?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let rel_path = format!("apps/{}", name);
        let framework = detect_framework(&path);
        let has_dockerfile = path.join("Dockerfile").exists();
        let package_name = get_package_name(&path);
        let pkg_info = extract_package_info(&path, catalog);

        apps.push(DetectedApp {
            name,
            path: rel_path,
            framework,
            has_dockerfile,
            package_name,
            scripts: pkg_info.scripts,
            deps: pkg_info.deps,
            dev_deps: pkg_info.dev_deps,
        });
    }

    // Sort by name for consistent output
    apps.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(apps)
}

/// Scan libs/ directory for libraries
pub fn scan_libs(catalog: &IndexMap<String, String>) -> Result<Vec<DetectedLib>> {
    let mut libs = Vec::new();
    let libs_dir = Path::new("libs");

    if !libs_dir.exists() {
        return Ok(libs);
    }

    // Scan top-level libs
    scan_libs_in_dir(libs_dir, "libs", catalog, &mut libs)?;

    // Scan nested libs (e.g., libs/supabase/*)
    let nested_dirs = ["supabase"];
    for nested in nested_dirs {
        let nested_path = libs_dir.join(nested);
        if nested_path.exists() && nested_path.is_dir() {
            scan_libs_in_dir(
                &nested_path,
                &format!("libs/{}", nested),
                catalog,
                &mut libs,
            )?;
        }
    }

    // Sort by path for consistent output
    libs.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(libs)
}

/// Helper to scan libraries in a specific directory
fn scan_libs_in_dir(
    dir: &Path,
    prefix: &str,
    catalog: &IndexMap<String, String>,
    libs: &mut Vec<DetectedLib>,
) -> Result<()> {
    let entries =
        fs::read_dir(dir).with_context(|| format!("Failed to read {} directory", prefix))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // Skip if no package.json (not a JS/TS library)
        if !path.join("package.json").exists() {
            continue;
        }

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Skip nested directories we'll scan separately
        if prefix == "libs" && name == "supabase" {
            continue;
        }

        let rel_path = format!("{}/{}", prefix, name);
        let package_name = get_package_name(&path);
        let pkg_info = extract_package_info(&path, catalog);

        libs.push(DetectedLib {
            name,
            path: rel_path,
            package_name,
            scripts: pkg_info.scripts,
            deps: pkg_info.deps,
            dev_deps: pkg_info.dev_deps,
        });
    }

    Ok(())
}
