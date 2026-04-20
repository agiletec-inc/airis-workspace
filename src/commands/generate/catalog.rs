use anyhow::Result;
use indexmap::IndexMap;
use std::path::Path;

use crate::manifest::{CatalogEntry, ProjectDefinition};

/// Prepare catalog mapping for generation.
///
/// In pnpm catalogs mode, this doesn't resolve actual version numbers from npm.
/// It only prepares the mapping for package.json generation.
pub(super) fn resolve_catalog_versions(
    catalog: &IndexMap<String, CatalogEntry>,
) -> Result<IndexMap<String, String>> {
    let mut resolved: IndexMap<String, String> = IndexMap::new();

    for (package, _) in catalog {
        // For pnpm catalogs, we just need to know which packages are in the catalog
        resolved.insert(package.clone(), "catalog:".to_string());
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

    // 2. Preset resolution
    if app.preset.is_some() || !app.dep_groups.is_empty() {
        let resolved = crate::preset::resolve_app_presets(app, presets, dep_groups)?;
        for (k, v) in &resolved.deps {
            if v == "catalog" && !resolved_catalog.contains_key(k) {
                resolved_catalog.insert(k.clone(), "catalog:".to_string());
            }
            final_deps.insert(k.clone(), v.clone());
        }
        for (k, v) in &resolved.dev_deps {
            if v == "catalog" && !resolved_catalog.contains_key(k) {
                resolved_catalog.insert(k.clone(), "catalog:".to_string());
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
                        if !final_deps.contains_key(pkg)
                            && (resolved_catalog.contains_key(pkg)
                                || matches_wildcard_catalog(pkg, &wildcard_patterns))
                        {
                            final_deps.insert(pkg.clone(), "catalog:".to_string());
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
        final_deps.insert(k.clone(), v.clone());
    }
    for (k, v) in &app.dev_deps {
        final_dev_deps.insert(k.clone(), v.clone());
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
pub(super) fn matches_wildcard_catalog(package: &str, wildcards: &[(&str, &CatalogEntry)]) -> bool {
    for (pattern, _) in wildcards {
        if wildcard_matches(pattern, package) {
            return true;
        }
    }
    false
}

/// Simple wildcard matching: `*` matches any sequence of characters at the end.
pub(super) fn wildcard_matches(pattern: &str, name: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        name.starts_with(prefix)
    } else {
        pattern == name
    }
}
