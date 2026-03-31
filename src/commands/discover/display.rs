//! Console output for discovery results.

use colored::Colorize;

use super::types::{ComposeLocation, DiscoveryResult, Framework};

/// Print discovery results to console
pub fn print_discovery_result(result: &DiscoveryResult) {
    // Apps
    if !result.apps.is_empty() {
        println!("{}", "📦 Detected Apps:".green());
        for app in &result.apps {
            let dockerfile_status = if app.has_dockerfile {
                "(has Dockerfile)".dimmed()
            } else {
                "(no Dockerfile)".yellow()
            };
            let runtime = if app.framework == Framework::Rust {
                "(local runtime)".dimmed()
            } else {
                dockerfile_status
            };
            println!(
                "   {:<18} {:<12} {}",
                app.path.bright_cyan(),
                app.framework.to_string().white(),
                runtime
            );
        }
        println!();
    }

    // Libraries
    if !result.libs.is_empty() {
        println!("{}", "📚 Detected Libraries:".green());
        for lib in &result.libs {
            println!("   {:<18} {}", lib.path.bright_cyan(), "TypeScript".white());
        }
        println!();
    }

    // Compose files
    if !result.compose_files.is_empty() {
        println!("{}", "🐳 Docker Compose Files:".green());
        for compose in &result.compose_files {
            let status = match compose.location {
                ComposeLocation::Root => {
                    format!("{} {}", "→".yellow(), "workspace/compose.yml".yellow())
                }
                _ => format!("{} (correct location)", "✓".green()),
            };
            println!("   {:<35} {}", compose.path.bright_cyan(), status);
        }
        println!();
    }

    // Catalog
    if !result.catalog.is_empty() {
        println!("{}", "📋 Extracted Catalog (from package.json):".green());
        for (name, version) in &result.catalog {
            println!("   {}: {}", name.white(), version.dimmed());
        }
        println!();
    }

    if result.is_empty() {
        println!(
            "{}",
            "ℹ️  No projects detected. This appears to be a new workspace.".dimmed()
        );
        println!();
    }
}
