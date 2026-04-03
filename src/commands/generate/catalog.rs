use anyhow::Result;
use colored::Colorize;
use indexmap::IndexMap;
use std::path::Path;

use crate::manifest::{CatalogEntry, Lock, ProjectDefinition};
use crate::version_resolver::resolve_version;

/// Resolve catalog version policies to actual version numbers.
///
/// Uses existing resolutions from airis.lock if available.
/// Wildcard patterns are resolved on-demand.
pub(super) fn resolve_catalog_versions(
    catalog: &IndexMap<String, CatalogEntry>,
    lock: &mut Lock,
    default_policy: Option<&str>,
) -> Result<IndexMap<String, String>> {
    if catalog.is_empty() {
        return Ok(IndexMap::new());
    }

    println!(
        "{}",
        "📦 Syncing catalog versions with airis.lock...".bright_blue()
    );

    let mut resolved: IndexMap<String, String> = IndexMap::new();

    for (package, entry) in catalog {
        // Skip wildcard patterns — they are resolved on-demand
        if package.contains('*') {
            continue;
        }

        // Use locked version if available
        if let Some(locked_version) = lock.get_catalog(package) {
            resolved.insert(package.clone(), locked_version.clone());
            continue;
        }

        let version = match entry {
            CatalogEntry::Policy(policy) => {
                let policy_str = policy.as_str();
                let version = resolve_version(package, policy_str)?;
                println!("  ✓ {} {} → {}", package, policy_str, version);
                version
            }
            CatalogEntry::Empty(_) => {
                let version = resolve_version(package, "latest")?;
                println!("  ✓ {} (default) → {}", package, version);
                version
            }
            CatalogEntry::Version(version) => version.clone(),
            CatalogEntry::Follow(follow_config) => {
                let target = &follow_config.follow;
                if let Some(target_version) = resolved.get(target) {
                    target_version.clone()
                } else if let Some(policy) = default_policy {
                    let target_version = resolve_version(target, policy)?;
                    println!("  ✓ {} {} → {}", target, policy, target_version);
                    resolved.insert(target.clone(), target_version.clone());
                    lock.update_catalog(target.clone(), target_version.clone());
                    target_version
                } else {
                    anyhow::bail!(
                        "Cannot resolve '{}': follow target '{}' not found in catalog",
                        package,
                        target
                    );
                }
            }
        };

        resolved.insert(package.clone(), version.clone());
        lock.update_catalog(package.clone(), version);
    }

    Ok(resolved)
}

/// Resolve package data for full-gen mode.
///
/// Combines: convention scripts + preset + dep_group + import scan → final deps/scripts
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_package_data(
    app: &ProjectDefinition,
    workspace_root: &Path,
    workspace_scope: &str,
    resolved_catalog: &mut IndexMap<String, String>,
    catalog_raw: &IndexMap<String, CatalogEntry>,
    lock: &mut Lock,
    presets: &IndexMap<String, crate::manifest::PresetSection>,
    dep_groups: &IndexMap<String, IndexMap<String, String>>,
    default_policy: Option<&str>,
) -> Result<crate::generators::package_json::ResolvedPackageData> {
    let mut final_deps = IndexMap::new();
    let mut final_dev_deps = IndexMap::new();
    let mut final_scripts = IndexMap::new();

    let app_name = app.name.clone();

    // 1. Convention defaults from framework
    let framework = app.framework.as_deref().unwrap_or("node");
    let conventions = crate::conventions::framework_defaults(framework);
    for (k, v) in conventions.default_scripts {
        final_scripts.insert(k.to_string(), v.to_string());
    }

    // 2. Preset resolution
    if app.preset.is_some() || !app.dep_groups.is_empty() {
        let resolved = crate::preset::resolve_app_presets(app, presets, dep_groups)?;
        for (k, v) in &resolved.deps {
            if v == "catalog" && !resolved_catalog.contains_key(k) {
                // Check lock first
                if let Some(locked) = lock.get_catalog(k) {
                    resolved_catalog.insert(k.clone(), locked.clone());
                } else {
                    let policy = default_policy.unwrap_or("latest");
                    if let Ok(version) = resolve_version(k, policy) {
                        println!("  ✓ {} (preset catalog, {}) → {}", k, policy, version);
                        resolved_catalog.insert(k.clone(), version.clone());
                        lock.update_catalog(k.clone(), version);
                    }
                }
            }
            final_deps.insert(k.clone(), v.clone());
        }
        for (k, v) in &resolved.dev_deps {
            if v == "catalog" && !resolved_catalog.contains_key(k) {
                if let Some(locked) = lock.get_catalog(k) {
                    resolved_catalog.insert(k.clone(), locked.clone());
                } else {
                    let policy = default_policy.unwrap_or("latest");
                    if let Ok(version) = resolve_version(k, policy) {
                        println!("  ✓ {} (preset dev-catalog, {}) → {}", k, policy, version);
                        resolved_catalog.insert(k.clone(), version.clone());
                        lock.update_catalog(k.clone(), version);
                    }
                }
            }
            final_dev_deps.insert(k.clone(), v.clone());
        }
        for (k, v) in &resolved.scripts {
            final_scripts.insert(k.clone(), v.clone());
        }
    }

    // Collect wildcard patterns from catalog for matching
    let wildcard_patterns: Vec<(&str, &CatalogEntry)> = catalog_raw
        .iter()
        .filter(|(k, _)| k.contains('*'))
        .map(|(k, v)| (k.as_str(), v))
        .collect();

    // 3. Import scan
    if let Some(ref app_path) = app.path {
        let full_path = workspace_root.join(app_path);
        if full_path.exists() {
            match crate::import_scanner::scan_imports(&full_path, workspace_scope) {
                Ok(scanned) => {
                    for pkg in &scanned.external {
                        if !final_deps.contains_key(pkg) {
                            if resolved_catalog.contains_key(pkg) {
                                final_deps.insert(pkg.clone(), "catalog".to_string());
                            } else if matches_wildcard_catalog(pkg, &wildcard_patterns) {
                                // Check lock first
                                if let Some(locked) = lock.get_catalog(pkg) {
                                    resolved_catalog.insert(pkg.clone(), locked.clone());
                                } else if let Ok(version) = resolve_version(pkg, "latest") {
                                    println!("  ✓ {} (wildcard) → {}", pkg, version);
                                    resolved_catalog.insert(pkg.clone(), version.clone());
                                    lock.update_catalog(pkg.clone(), version);
                                }
                                final_deps.insert(pkg.clone(), "catalog".to_string());
                            } else if let Some(policy) = default_policy {
                                if let Some(locked) = lock.get_catalog(pkg) {
                                    resolved_catalog.insert(pkg.clone(), locked.clone());
                                } else if let Ok(version) = resolve_version(pkg, policy) {
                                    println!("  ✓ {} (default: {}) → {}", pkg, policy, version);
                                    resolved_catalog.insert(pkg.clone(), version.clone());
                                    lock.update_catalog(pkg.clone(), version);
                                }
                                final_deps.insert(pkg.clone(), "catalog".to_string());
                            }
                        }
                    }
                    // Workspace deps
                    let self_pkg_name = if let Some(ref scope) = app.scope {
                        format!("@{}/{}", scope.trim_start_matches('@'), app.name)
                    } else {
                        format!("{}/{}", workspace_scope, app.name)
                    };
                    for pkg in &scanned.workspace {
                        if pkg != &self_pkg_name && !final_deps.contains_key(pkg) {
                            final_deps.insert(pkg.clone(), "workspace:*".to_string());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("  ⚠ Import scan failed for {}: {}", app_path, e);
                }
            }
        }
    }

    // 4. Explicit deps (override everything)
    for (k, v) in &app.deps {
        // If it's not "catalog" and not a specific version policy, it might need resolution
        if v != "catalog" && !v.starts_with("workspace:") && !v.starts_with("link:") {
            // Check app-specific lock
            if let Some(locked) = lock.get_app_dep(&app_name, k) {
                final_deps.insert(k.clone(), locked.clone());
            } else if v == "latest" || v == "lts" {
                if let Ok(version) = resolve_version(k, v) {
                    println!("  ✓ {}:{} {} → {}", app_name, k, v, version);
                    lock.update_app_dep(app_name.clone(), k.clone(), version.clone());
                    final_deps.insert(k.clone(), version);
                } else {
                    final_deps.insert(k.clone(), v.clone());
                }
            } else {
                final_deps.insert(k.clone(), v.clone());
            }
        } else {
            final_deps.insert(k.clone(), v.clone());
        }
    }
    for (k, v) in &app.dev_deps {
        if v != "catalog" && !v.starts_with("workspace:") && !v.starts_with("link:") {
            if let Some(locked) = lock.get_app_dep(&app_name, k) {
                final_dev_deps.insert(k.clone(), locked.clone());
            } else if v == "latest" || v == "lts" {
                if let Ok(version) = resolve_version(k, v) {
                    println!("  ✓ {}:{} (dev) {} → {}", app_name, k, v, version);
                    lock.update_app_dep(app_name.clone(), k.clone(), version.clone());
                    final_dev_deps.insert(k.clone(), version);
                } else {
                    final_dev_deps.insert(k.clone(), v.clone());
                }
            } else {
                final_dev_deps.insert(k.clone(), v.clone());
            }
        } else {
            final_dev_deps.insert(k.clone(), v.clone());
        }
    }
    for (k, v) in &app.scripts {
        final_scripts.insert(k.clone(), v.clone());
    }

    Ok(crate::generators::package_json::ResolvedPackageData {
        deps: final_deps,
        dev_deps: final_dev_deps,
        scripts: final_scripts,
    })
}

/// Check if a package name matches any wildcard pattern in the catalog.
/// Supports simple glob patterns like `@radix-ui/react-*`.
pub(super) fn matches_wildcard_catalog(package: &str, wildcards: &[(&str, &CatalogEntry)]) -> bool {
    for (pattern, _) in wildcards {
        if wildcard_matches(pattern, package) {
            return true;
        }
    }
    false
}

/// Simple wildcard matching: `*` matches any sequence of characters.
/// Only supports `*` at the end of a pattern (prefix match).
pub(super) fn wildcard_matches(pattern: &str, name: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        name.starts_with(prefix)
    } else {
        pattern == name
    }
}
