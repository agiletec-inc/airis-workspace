use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use colored::Colorize;
use serde::Deserialize;

/// Discovered project structure
#[derive(Debug, Default)]
pub struct DiscoveredProject {
    pub apps: Vec<DiscoveredApp>,
    pub libs: Vec<DiscoveredLib>,
    pub compose_files: DiscoveredComposeFiles,
    pub catalog: Vec<CatalogEntry>,
}

#[derive(Debug)]
pub struct DiscoveredApp {
    pub name: String,
    pub path: PathBuf,
    pub app_type: AppType,
    pub port: Option<u16>,
}

#[derive(Debug)]
pub struct DiscoveredLib {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Default)]
pub struct DiscoveredComposeFiles {
    pub workspace: Option<PathBuf>,
    pub supabase: Vec<PathBuf>,
    pub traefik: Option<PathBuf>,
    pub root: Option<PathBuf>,
}

#[derive(Debug)]
pub struct CatalogEntry {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppType {
    NextJs,
    Node,
    Rust,
    Python,
    Unknown,
}

impl AppType {
    pub fn as_str(&self) -> &str {
        match self {
            AppType::NextJs => "nextjs",
            AppType::Node => "node",
            AppType::Rust => "rust",
            AppType::Python => "python",
            AppType::Unknown => "unknown",
        }
    }
}

/// Discover project structure (file signature-based, not directory name-based)
pub fn discover_project<P: AsRef<Path>>(root: P) -> Result<DiscoveredProject> {
    let root = root.as_ref();
    println!("{} Discovering project structure...", "🔍".blue());
    println!("  {} Scanning for packages (language-agnostic)...", "📦".blue());

    let mut project = DiscoveredProject::default();

    // Recursively discover packages based on file signatures
    let all_packages = discover_packages_recursive(root, 3)?;

    // Separate apps and libs based on heuristics
    for pkg in all_packages {
        if is_likely_lib(&pkg) {
            project.libs.push(DiscoveredLib {
                name: pkg.name.clone(),
                path: pkg.path.clone(),
            });
        } else {
            project.apps.push(pkg);
        }
    }

    println!(
        "  {} Found {} apps",
        "✓".green(),
        project.apps.len().to_string().cyan()
    );
    println!(
        "  {} Found {} libs",
        "✓".green(),
        project.libs.len().to_string().cyan()
    );

    // Discover docker-compose files
    project.compose_files = discover_compose_files(root)?;
    print_compose_discovery(&project.compose_files);

    // Extract catalog from package.json
    if let Some(catalog) = extract_catalog_from_package_json(root)? {
        project.catalog = catalog;
        println!(
            "  {} Extracted {} catalog entries from package.json",
            "✓".green(),
            project.catalog.len().to_string().cyan()
        );
    }

    Ok(project)
}

/// Recursively discover packages by file signatures (not directory names)
fn discover_packages_recursive(root: &Path, max_depth: usize) -> Result<Vec<DiscoveredApp>> {
    let mut packages = Vec::new();
    let ignored_dirs = vec![
        "node_modules",
        ".git",
        ".next",
        "dist",
        "build",
        "target",
        ".turbo",
        ".venv",
        "__pycache__",
        "venv",
        ".cache",
    ];

    fn scan_dir(
        dir: &Path,
        root: &Path,
        depth: usize,
        max_depth: usize,
        ignored: &[&str],
        packages: &mut Vec<DiscoveredApp>,
    ) -> Result<()> {
        if depth > max_depth {
            return Ok(());
        }

        // Check if this directory is a package (but not root)
        if dir != root {
            if let Some(pkg) = detect_package(dir)? {
                packages.push(pkg);
                // Don't recurse into package subdirectories
                return Ok(());
            }
        }

        // Recurse into subdirectories
        for entry in fs::read_dir(dir).with_context(|| format!("Failed to read {:?}", dir))? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if ignored.contains(&name) {
                continue;
            }

            scan_dir(&path, root, depth + 1, max_depth, ignored, packages)?;
        }

        Ok(())
    }

    scan_dir(root, root, 0, max_depth, &ignored_dirs, &mut packages)?;

    Ok(packages)
}

/// Detect if a directory is a package based on file signatures
fn detect_package(dir: &Path) -> Result<Option<DiscoveredApp>> {
    // TypeScript/Node: package.json
    if dir.join("package.json").exists() {
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let app_type = detect_app_type(dir);
        let port = extract_port_from_package_json(dir);

        return Ok(Some(DiscoveredApp {
            name,
            path: dir.to_path_buf(),
            app_type,
            port,
        }));
    }

    // Rust: Cargo.toml
    if dir.join("Cargo.toml").exists() {
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        return Ok(Some(DiscoveredApp {
            name,
            path: dir.to_path_buf(),
            app_type: AppType::Rust,
            port: None,
        }));
    }

    // Python: pyproject.toml or requirements.txt
    if dir.join("pyproject.toml").exists() || dir.join("requirements.txt").exists() {
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        return Ok(Some(DiscoveredApp {
            name,
            path: dir.to_path_buf(),
            app_type: AppType::Python,
            port: None,
        }));
    }

    Ok(None)
}

/// Heuristic to determine if a package is likely a library
fn is_likely_lib(pkg: &DiscoveredApp) -> bool {
    let path_str = pkg.path.to_string_lossy().to_lowercase();

    // Check directory structure hints
    if path_str.contains("/libs/")
        || path_str.contains("/packages/")
        || path_str.contains("/shared/")
        || path_str.contains("/common/")
    {
        return true;
    }

    // Check package name hints
    let name_lower = pkg.name.to_lowercase();
    if name_lower.starts_with("lib-")
        || name_lower.starts_with("pkg-")
        || name_lower.contains("-lib")
        || name_lower.contains("-utils")
        || name_lower.contains("-common")
    {
        return true;
    }

    // TypeScript/Node: check for library indicators in package.json
    if pkg.app_type == AppType::Node || pkg.app_type == AppType::NextJs {
        if let Ok(content) = fs::read_to_string(pkg.path.join("package.json")) {
            if let Ok(package) = serde_json::from_str::<serde_json::Value>(&content) {
                // Has "main" or "exports" but no "scripts.dev"
                if (package.get("main").is_some() || package.get("exports").is_some())
                    && package
                        .get("scripts")
                        .and_then(|s| s.get("dev"))
                        .is_none()
                {
                    return true;
                }
            }
        }
    }

    false
}


fn detect_app_type(app_dir: &Path) -> AppType {
    // Check for Next.js
    if app_dir.join("next.config.js").exists()
        || app_dir.join("next.config.mjs").exists()
        || app_dir.join("next.config.ts").exists()
    {
        return AppType::NextJs;
    }

    // Check for Rust
    if app_dir.join("Cargo.toml").exists() {
        return AppType::Rust;
    }

    // Check for Python
    if app_dir.join("main.py").exists()
        || app_dir.join("pyproject.toml").exists()
        || app_dir.join("requirements.txt").exists()
    {
        return AppType::Python;
    }

    // Check for Node.js (has package.json but not Next.js)
    if app_dir.join("package.json").exists() {
        return AppType::Node;
    }

    AppType::Unknown
}

fn extract_port_from_package_json(app_dir: &Path) -> Option<u16> {
    let package_json_path = app_dir.join("package.json");
    if !package_json_path.exists() {
        return None;
    }

    let content = fs::read_to_string(&package_json_path).ok()?;
    let package: serde_json::Value = serde_json::from_str(&content).ok()?;

    // Try to extract port from dev script (e.g., "next dev -p 3000")
    if let Some(scripts) = package.get("scripts").and_then(|s| s.as_object()) {
        if let Some(dev_script) = scripts.get("dev").and_then(|s| s.as_str()) {
            // Parse "-p 3000" or "--port 3000"
            if let Some(port_str) = dev_script
                .split_whitespace()
                .skip_while(|&s| s != "-p" && s != "--port")
                .nth(1)
            {
                return port_str.parse().ok();
            }
        }
    }

    None
}

fn discover_compose_files(root: &Path) -> Result<DiscoveredComposeFiles> {
    let mut compose_files = DiscoveredComposeFiles::default();

    // Check root
    let root_compose = root.join("docker-compose.yml");
    if root_compose.exists() {
        compose_files.root = Some(root_compose);
    }

    // Check workspace/
    let workspace_compose = root.join("workspace").join("docker-compose.yml");
    if workspace_compose.exists() {
        compose_files.workspace = Some(workspace_compose);
    }

    // Check supabase/
    let supabase_dir = root.join("supabase");
    if supabase_dir.exists() {
        let supabase_compose = supabase_dir.join("docker-compose.yml");
        let supabase_override = supabase_dir.join("docker-compose.override.yml");

        if supabase_compose.exists() {
            compose_files.supabase.push(supabase_compose);
        }
        if supabase_override.exists() {
            compose_files.supabase.push(supabase_override);
        }
    }

    // Check traefik/
    let traefik_compose = root.join("traefik").join("docker-compose.yml");
    if traefik_compose.exists() {
        compose_files.traefik = Some(traefik_compose);
    }

    Ok(compose_files)
}

fn print_compose_discovery(compose_files: &DiscoveredComposeFiles) {
    if compose_files.root.is_some() {
        println!("  {} Found docker-compose.yml at root", "⚠️ ".yellow());
    }
    if compose_files.workspace.is_some() {
        println!(
            "  {} Found workspace/docker-compose.yml",
            "✓".green()
        );
    }
    if !compose_files.supabase.is_empty() {
        println!(
            "  {} Found {} supabase compose files",
            "✓".green(),
            compose_files.supabase.len().to_string().cyan()
        );
    }
    if compose_files.traefik.is_some() {
        println!("  {} Found traefik/docker-compose.yml", "✓".green());
    }
}

#[derive(Debug, Deserialize)]
struct PackageJson {
    pnpm: Option<PnpmConfig>,
}

#[derive(Debug, Deserialize)]
struct PnpmConfig {
    catalog: Option<serde_json::Map<String, serde_json::Value>>,
}

fn extract_catalog_from_package_json(root: &Path) -> Result<Option<Vec<CatalogEntry>>> {
    let package_json_path = root.join("package.json");
    if !package_json_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&package_json_path)
        .with_context(|| "Failed to read package.json")?;

    let package: PackageJson =
        serde_json::from_str(&content).with_context(|| "Failed to parse package.json")?;

    if let Some(pnpm) = package.pnpm {
        if let Some(catalog) = pnpm.catalog {
            let entries: Vec<CatalogEntry> = catalog
                .into_iter()
                .filter_map(|(name, value)| {
                    value.as_str().map(|version| CatalogEntry {
                        name,
                        version: version.to_string(),
                    })
                })
                .collect();
            return Ok(Some(entries));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_nextjs() {
        let temp = tempfile::tempdir().unwrap();
        let app_dir = temp.path().join("my-app");
        fs::create_dir(&app_dir).unwrap();
        fs::write(app_dir.join("next.config.js"), "").unwrap();

        assert_eq!(detect_app_type(&app_dir), AppType::NextJs);
    }

    #[test]
    fn test_detect_rust() {
        let temp = tempfile::tempdir().unwrap();
        let app_dir = temp.path().join("my-app");
        fs::create_dir(&app_dir).unwrap();
        fs::write(app_dir.join("Cargo.toml"), "").unwrap();

        assert_eq!(detect_app_type(&app_dir), AppType::Rust);
    }

    #[test]
    fn test_detect_node() {
        let temp = tempfile::tempdir().unwrap();
        let app_dir = temp.path().join("my-app");
        fs::create_dir(&app_dir).unwrap();
        fs::write(app_dir.join("package.json"), "{}").unwrap();

        assert_eq!(detect_app_type(&app_dir), AppType::Node);
    }
}
