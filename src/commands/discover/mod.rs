//! Project discovery module for auto-migration.
//!
//! Scans the workspace to detect:
//! - Apps in apps/ directory (Next.js, Vite, Hono, Node, Rust)
//! - Libraries in libs/ directory
//! - Docker compose files (root, workspace/, supabase/, traefik/)
//! - Catalog entries from root package.json

mod catalog;
mod compose;
mod detection;
mod display;
mod scanning;
pub mod types;

#[cfg(test)]
mod tests;

// Re-export public types (used by generate, migrate, init)
#[allow(unused_imports)]
pub use types::{
    ComposeLocation, DetectedApp, DetectedCompose, DetectedLib, DiscoveredProject, DiscoveryResult,
    Framework, PackageInfo,
};

// Re-export public functions
pub use scanning::discover_from_workspaces;

use crate::manifest::Manifest;
use anyhow::Result;
use colored::Colorize;

/// Run project discovery
pub fn run() -> Result<DiscoveryResult> {
    println!("{}", "🔍 Discovering project structure...".bright_blue());
    println!();

    // Extract catalog first (needed for package info extraction)
    let catalog = catalog::extract_catalog()?;
    let apps = scanning::scan_apps(&catalog)?;
    let libs = scanning::scan_libs(&catalog)?;
    let compose_files = compose::find_compose_files()?;

    let result = DiscoveryResult {
        apps,
        libs,
        compose_files,
        catalog,
    };

    display::print_discovery_result(&result);

    Ok(result)
}

/// Generate a recommended manifest.toml based on discovery facts
pub fn propose_manifest(discovery: &DiscoveryResult) -> Result<String> {
    // Project identity (fallback to directory name)
    let current_dir = std::env::current_dir()?;
    let dir_name = current_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "my-workspace".to_string());

    let mut manifest = Manifest::default_with_project(&dir_name);

    // Standardize apps
    for detected in &discovery.apps {
        let app = crate::manifest::ProjectDefinition {
            name: detected.name.clone(),
            path: Some(detected.path.clone()),
            use_stack: match detected.framework {
                Framework::NextJs => Some("nextjs".into()),
                Framework::Vite => Some("vite".into()),
                Framework::Hono => Some("hono".into()),
                Framework::Rust => Some("rust".into()),
                Framework::Python => Some("python".into()),
                _ => Some("node".into()),
            },
            ..Default::default()
        };
        manifest.app.push(app);
    }

    // Standardize libs (manifest.toml v2 uses the app list for both apps and libs)
    for detected in &discovery.libs {
        let lib = crate::manifest::ProjectDefinition {
            name: detected.name.clone(),
            path: Some(detected.path.clone()),
            kind: Some("lib".into()),
            ..Default::default()
        };
        manifest.app.push(lib);
    }

    // Convert existing Compose files into services or global rules
    manifest.orchestration.dev = Some(crate::manifest::OrchestrationDev {
        workspace: Some("compose.yaml".into()),
        supabase: None,
        traefik: None,
        restart: None,
    });

    // Generate the TOML string
    let toml_str = toml::to_string_pretty(&manifest)?;
    Ok(toml_str)
}
