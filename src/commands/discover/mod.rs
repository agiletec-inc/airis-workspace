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
