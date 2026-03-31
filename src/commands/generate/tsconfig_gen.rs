use anyhow::{Context, Result};
use colored::Colorize;
use indexmap::IndexMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::commands::discover::discover_from_workspaces;
use crate::manifest::Manifest;
use crate::templates::TemplateEngine;

use super::write_with_backup;

pub(super) fn generate_tsconfig(
    manifest: &Manifest,
    engine: &TemplateEngine,
    resolved_catalog: &IndexMap<String, String>,
) -> Result<()> {
    println!();
    println!("{}", "📝 Generating tsconfig files...".bright_blue());

    let ts_major = detect_ts_major(manifest, resolved_catalog);

    // 1. tsconfig.base.json — shared compilerOptions
    let base_content = engine.render_tsconfig_base(manifest)?;
    let base_path = Path::new("tsconfig.base.json");
    write_with_backup(base_path, &base_content)?;
    println!(
        "   {} tsconfig.base.json (shared compilerOptions)",
        "✓".green()
    );

    // 2. Collect workspace paths for IDE path aliases
    let workspace_root = env::current_dir().context("Failed to get current directory")?;
    let workspace_patterns = if !manifest.packages.workspaces.is_empty() {
        &manifest.packages.workspaces
    } else {
        &manifest.workspace.workspaces
    };

    let mut path_entries: Vec<(String, String)> = Vec::new();
    if !workspace_patterns.is_empty() {
        let discovered = discover_from_workspaces(workspace_patterns, &workspace_root)?;
        for disc in &discovered {
            // Skip node_modules and build artifacts
            if disc.path.contains("node_modules")
                || disc.path.contains(".next")
                || disc.path.contains("/dist/")
            {
                continue;
            }
            let pkg_json_path = workspace_root.join(&disc.path).join("package.json");
            if let Ok(content) = fs::read_to_string(&pkg_json_path)
                && let Ok(json) = serde_json::from_str::<serde_json::Value>(&content)
                && let Some(name) = json.get("name").and_then(|n| n.as_str())
            {
                path_entries.push((name.to_string(), disc.path.clone()));
            }
        }
    }

    // 3. tsconfig.json — IDE config with paths
    let root_content = engine.render_tsconfig_root(manifest, &path_entries, ts_major)?;
    let root_path = Path::new("tsconfig.json");
    write_with_backup(root_path, &root_content)?;

    if ts_major >= 6 {
        println!(
            "   {} tsconfig.json (IDE, {} paths, TS{} — ignoreDeprecations: \"6.0\")",
            "✓".green(),
            path_entries.len(),
            ts_major,
        );
    } else {
        println!(
            "   {} tsconfig.json (IDE, {} paths, TS{})",
            "✓".green(),
            path_entries.len(),
            ts_major,
        );
    }

    // 4. Per-package tsconfig.json files
    if manifest.typescript.generate_per_package {
        let mut pkg_count = 0;
        let mut css_count = 0;
        for app in &manifest.app {
            // Skip rust packages
            if app.framework.as_deref() == Some("rust") {
                continue;
            }

            let pkg_path = if let Some(ref p) = app.path {
                PathBuf::from(p)
            } else {
                // Auto-detect path from workspace discovery
                let matched = path_entries.iter().find(|(name, _)| {
                    let scoped = format!(
                        "@{}/{}",
                        manifest
                            .workspace
                            .scope
                            .as_deref()
                            .unwrap_or(&manifest.workspace.name),
                        app.name
                    );
                    name == &scoped || name == &app.name
                });
                if let Some((_, path)) = matched {
                    PathBuf::from(path)
                } else {
                    continue;
                }
            };

            // Calculate relative path to root
            let depth = pkg_path.components().count();
            let rel_to_root = "../".repeat(depth);

            let pkg_tsconfig =
                engine.render_package_tsconfig(app, manifest, &rel_to_root, ts_major)?;
            let tsconfig_path = pkg_path.join("tsconfig.json");
            write_with_backup(&tsconfig_path, &pkg_tsconfig)?;
            pkg_count += 1;

            // Generate css.d.ts for Next.js apps (TS6 TS2882 fix)
            if app.framework.as_deref() == Some("nextjs") {
                let css_decl = engine.render_css_declaration();
                let src_dir = pkg_path.join("src");
                if src_dir.exists() {
                    let css_path = src_dir.join("css.d.ts");
                    write_with_backup(&css_path, &css_decl)?;
                    css_count += 1;
                }
            }
        }
        if pkg_count > 0 {
            print!(
                "   {} {} package tsconfig.json files",
                "✓".green(),
                pkg_count
            );
            if css_count > 0 {
                print!(" + {} css.d.ts", css_count);
            }
            println!();
        }
    }

    Ok(())
}

/// Detect TypeScript major version from manifest or resolved catalog.
pub(super) fn detect_ts_major(
    manifest: &Manifest,
    resolved_catalog: &IndexMap<String, String>,
) -> u32 {
    // Explicit override in [typescript]
    if let Some(v) = manifest.typescript.version {
        return v;
    }

    // Auto-detect from resolved catalog
    if let Some(version_str) = resolved_catalog.get("typescript") {
        let clean = version_str.trim_start_matches('^').trim_start_matches('~');
        if let Some(major_str) = clean.split('.').next()
            && let Ok(major) = major_str.parse::<u32>()
        {
            return major;
        }
    }

    // Default: assume TS5 (safe, no ignoreDeprecations)
    5
}
