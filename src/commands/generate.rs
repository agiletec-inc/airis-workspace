use anyhow::{Context, Result};
use colored::Colorize;
use indexmap::IndexMap;
use std::env;

use crate::commands::sync_deps::resolve_version;
use crate::generators::package_json::generate_project_package_json;
use crate::manifest::{CatalogEntry, Manifest};
use crate::templates::TemplateEngine;

/// Sync justfile/docker-compose/package.json from manifest.toml contents
pub fn sync_from_manifest(manifest: &Manifest) -> Result<()> {
    // Resolve catalog versions from npm registry
    let resolved_catalog = resolve_catalog_versions(&manifest.packages.catalog)?;

    let engine = TemplateEngine::new()?;
    println!("{}", "🧩 Rendering templates...".bright_blue());
    generate_docker_compose(&manifest, &engine)?;
    generate_justfile(&manifest, &engine)?;
    generate_package_json(&manifest, &engine)?;
    generate_pnpm_workspace(&manifest, &engine, &resolved_catalog)?;

    // Generate individual project package.json files
    if !manifest.project.is_empty() {
        println!();
        println!("{}", "📦 Generating project package.json files...".bright_blue());
        let workspace_root = env::current_dir().context("Failed to get current directory")?;

        for project in &manifest.project {
            generate_project_package_json(project, &workspace_root, &resolved_catalog)?;
        }
    }

    println!();
    println!("{}", "✅ Generated files:".green());
    println!("   - docker-compose.yml");
    println!("   - justfile");
    println!("   - package.json");
    println!("   - pnpm-workspace.yaml");
    if !manifest.project.is_empty() {
        println!("   - {} project package.json files", manifest.project.len());
    }
    println!();
    println!("{}", "Next steps:".bright_yellow());
    println!("  1. Review justfile/docker-compose.yml");
    println!("  2. Run `just up`");

    Ok(())
}

fn generate_justfile(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    let content = engine.render_justfile(manifest)?;
    let output = "justfile";
    std::fs::write(output, content).with_context(|| format!("Failed to write {}", output))?;
    Ok(())
}

fn generate_package_json(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    let content = engine.render_package_json(manifest)?;
    std::fs::write("package.json", content).context("Failed to write package.json")?;
    Ok(())
}

fn generate_pnpm_workspace(
    manifest: &Manifest,
    engine: &TemplateEngine,
    resolved_catalog: &IndexMap<String, String>,
) -> Result<()> {
    let content = engine.render_pnpm_workspace(manifest, resolved_catalog)?;
    std::fs::write("pnpm-workspace.yaml", content)
        .context("Failed to write pnpm-workspace.yaml")?;
    Ok(())
}

/// Resolve catalog version policies to actual version numbers
fn resolve_catalog_versions(
    catalog: &IndexMap<String, CatalogEntry>,
) -> Result<IndexMap<String, String>> {
    if catalog.is_empty() {
        return Ok(IndexMap::new());
    }

    println!("{}", "📦 Resolving catalog versions from npm registry...".bright_blue());

    let mut resolved: IndexMap<String, String> = IndexMap::new();

    for (package, entry) in catalog {
        let version = match entry {
            CatalogEntry::Policy(policy) => {
                let policy_str = policy.as_str();
                let version = resolve_version(package, policy_str)?;
                println!("  ✓ {} {} → {}", package, policy_str, version);
                version
            }
            CatalogEntry::Version(version) => {
                println!("  ✓ {} {}", package, version);
                version.clone()
            }
            CatalogEntry::Follow(follow_config) => {
                // For follow entries, we'll resolve them in a second pass
                let target = &follow_config.follow;
                if let Some(target_version) = resolved.get(target) {
                    println!("  ✓ {} (follow {}) → {}", package, target, target_version);
                    target_version.clone()
                } else {
                    anyhow::bail!(
                        "Cannot resolve '{}': follow target '{}' not found or not yet resolved",
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

fn generate_docker_compose(manifest: &Manifest, engine: &TemplateEngine) -> Result<()> {
    let content = engine.render_docker_compose(manifest)?;
    std::fs::write("docker-compose.yml", content).context("Failed to write docker-compose.yml")?;
    Ok(())
}
