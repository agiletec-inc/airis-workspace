use anyhow::Result;
use colored::Colorize;
use indexmap::IndexMap;
use std::path::Path;

use crate::manifest::{CatalogEntry, ProjectDefinition};
use crate::version_resolver::resolve_version;

/// Resolve catalog version policies to actual version numbers.
///
/// Supports wildcard patterns like `@radix-ui/react-* = "latest"`.
/// Wildcard entries are stored as patterns and resolved on-demand
/// when a concrete package name matches via `resolve_wildcard_version`.
pub(super) fn resolve_catalog_versions(
    catalog: &IndexMap<String, CatalogEntry>,
    default_policy: Option<&str>,
) -> Result<IndexMap<String, String>> {
    if catalog.is_empty() {
        return Ok(IndexMap::new());
    }

    println!(
        "{}",
        "📦 Resolving catalog versions from npm registry...".bright_blue()
    );

    let mut resolved: IndexMap<String, String> = IndexMap::new();

    for (package, entry) in catalog {
        // Skip wildcard patterns — they are resolved on-demand
        if package.contains('*') {
            let policy_str = match entry {
                CatalogEntry::Policy(p) => p.as_str().to_string(),
                CatalogEntry::Empty(_) => "latest".to_string(),
                CatalogEntry::Version(v) => v.clone(),
                _ => "latest".to_string(),
            };
            println!("  ✓ {} (wildcard pattern, policy: {})", package, policy_str);
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
                // Empty table {} = latest
                let version = resolve_version(package, "latest")?;
                println!("  ✓ {} (default) → {}", package, version);
                version
            }
            CatalogEntry::Version(version) => {
                println!("  ✓ {} {}", package, version);
                version.clone()
            }
            CatalogEntry::Follow(follow_config) => {
                let target = &follow_config.follow;
                if let Some(target_version) = resolved.get(target) {
                    println!("  ✓ {} (follow {}) → {}", package, target, target_version);
                    target_version.clone()
                } else if let Some(policy) = default_policy {
                    // Follow target not in catalog — resolve it via default_policy first
                    let target_version = resolve_version(target, policy)?;
                    println!("  ✓ {} {} → {}", target, policy, target_version);
                    resolved.insert(target.clone(), target_version.clone());
                    println!("  ✓ {} (follow {}) → {}", package, target, target_version);
                    target_version
                } else {
                    anyhow::bail!(
                        "Cannot resolve '{}': follow target '{}' not found in catalog (add it or set default_policy)",
                        package,
                        target
                    );
                }
            }
        };

        resolved.insert(package.clone(), version);
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
    presets: &IndexMap<String, crate::manifest::PresetSection>,
    dep_groups: &IndexMap<String, IndexMap<String, String>>,
    default_policy: Option<&str>,
) -> Result<crate::generators::package_json::ResolvedPackageData> {
    let mut final_deps = IndexMap::new();
    let mut final_dev_deps = IndexMap::new();
    let mut final_scripts = IndexMap::new();

    // 1. Convention defaults from framework
    let framework = app.framework.as_deref().unwrap_or("node");
    let conventions = crate::conventions::framework_defaults(framework);
    for (k, v) in conventions.default_scripts {
        final_scripts.insert(k.to_string(), v.to_string());
    }

    // 2. Preset resolution (includes dep_groups from preset)
    if app.preset.is_some() || !app.dep_groups.is_empty() {
        let resolved = crate::preset::resolve_app_presets(app, presets, dep_groups)?;
        for (k, v) in &resolved.deps {
            // Resolve "catalog" references: if not in resolved_catalog, use default_policy
            if v == "catalog" && !resolved_catalog.contains_key(k) {
                let policy = default_policy.unwrap_or("latest");
                match resolve_version(k, policy) {
                    Ok(version) => {
                        println!("  ✓ {} (dep_group, default: {}) → {}", k, policy, version);
                        resolved_catalog.insert(k.clone(), version);
                    }
                    Err(e) => {
                        eprintln!("  ⚠ Failed to resolve {}: {}", k, e);
                    }
                }
            }
            final_deps.insert(k.clone(), v.clone());
        }
        for (k, v) in &resolved.dev_deps {
            if v == "catalog" && !resolved_catalog.contains_key(k) {
                let policy = default_policy.unwrap_or("latest");
                match resolve_version(k, policy) {
                    Ok(version) => {
                        println!(
                            "  ✓ {} (dep_group dev, default: {}) → {}",
                            k, policy, version
                        );
                        resolved_catalog.insert(k.clone(), version);
                    }
                    Err(e) => {
                        eprintln!("  ⚠ Failed to resolve {}: {}", k, e);
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

    // 3. Import scan (auto-detect deps from source code)
    if let Some(ref app_path) = app.path {
        let full_path = workspace_root.join(app_path);
        if full_path.exists() {
            match crate::import_scanner::scan_imports(&full_path, workspace_scope) {
                Ok(scanned) => {
                    // External deps: use catalog version if available, or match wildcard
                    for pkg in &scanned.external {
                        if !final_deps.contains_key(pkg) {
                            if resolved_catalog.contains_key(pkg) {
                                final_deps.insert(pkg.clone(), "catalog".to_string());
                            } else if matches_wildcard_catalog(pkg, &wildcard_patterns) {
                                // Wildcard match: resolve version from npm and add to catalog
                                match resolve_version(pkg, "latest") {
                                    Ok(version) => {
                                        println!("  ✓ {} (wildcard) → {}", pkg, version);
                                        resolved_catalog.insert(pkg.clone(), version);
                                        final_deps.insert(pkg.clone(), "catalog".to_string());
                                    }
                                    Err(e) => {
                                        eprintln!("  ⚠ Failed to resolve {}: {}", pkg, e);
                                    }
                                }
                            } else if let Some(policy) = default_policy {
                                // Default policy fallback: resolve from npm
                                match resolve_version(pkg, policy) {
                                    Ok(version) => {
                                        println!("  ✓ {} (default: {}) → {}", pkg, policy, version);
                                        resolved_catalog.insert(pkg.clone(), version);
                                        final_deps.insert(pkg.clone(), "catalog".to_string());
                                    }
                                    Err(e) => {
                                        eprintln!("  ⚠ Failed to resolve {}: {}", pkg, e);
                                    }
                                }
                            }
                            // Not in catalog, no wildcard, no default policy → skip
                        }
                    }
                    // Workspace deps (skip self-reference)
                    let self_pkg_name = if let Some(ref scope) = app.scope {
                        let scope = scope.trim_start_matches('@');
                        format!("@{}/{}", scope, app.name)
                    } else {
                        format!("{}/{}", workspace_scope, app.name)
                    };
                    for pkg in &scanned.workspace {
                        if pkg == &self_pkg_name {
                            continue; // Skip self-reference
                        }
                        if !final_deps.contains_key(pkg) {
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

    // 4. Explicit deps from [[app]] override everything
    for (k, v) in &app.deps {
        final_deps.insert(k.clone(), v.clone());
    }
    for (k, v) in &app.dev_deps {
        final_dev_deps.insert(k.clone(), v.clone());
    }
    // Explicit scripts from [[app]] override convention + preset
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
pub(super) fn matches_wildcard_catalog(
    package: &str,
    wildcards: &[(&str, &CatalogEntry)],
) -> bool {
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
