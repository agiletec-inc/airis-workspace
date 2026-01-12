//! Project discovery module for auto-migration.
//!
//! Scans the workspace to detect:
//! - Apps in apps/ directory (Next.js, Vite, Hono, Node, Rust)
//! - Libraries in libs/ directory
//! - Docker compose files (root, workspace/, supabase/, traefik/)
//! - Catalog entries from root package.json

use anyhow::{Context, Result};
use colored::Colorize;
use indexmap::IndexMap;
use serde_json::Value;
use std::fs;
use std::path::Path;

/// Detected framework for an app
#[derive(Debug, Clone, PartialEq)]
pub enum Framework {
    NextJs,
    Vite,
    Hono,
    Node,
    Rust,
    Python,
    Unknown,
}

impl std::fmt::Display for Framework {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Framework::NextJs => write!(f, "nextjs"),
            Framework::Vite => write!(f, "vite"),
            Framework::Hono => write!(f, "hono"),
            Framework::Node => write!(f, "node"),
            Framework::Rust => write!(f, "rust"),
            Framework::Python => write!(f, "python"),
            Framework::Unknown => write!(f, "unknown"),
        }
    }
}

/// Location category for docker-compose files
#[derive(Debug, Clone, PartialEq)]
pub enum ComposeLocation {
    Root,      // ./docker-compose.yml (should be moved)
    Workspace, // workspace/docker-compose.yml
    Supabase,  // supabase/docker-compose.yml
    Traefik,   // traefik/docker-compose.yml
    App,       // apps/*/docker-compose.yml
}

impl std::fmt::Display for ComposeLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComposeLocation::Root => write!(f, "root"),
            ComposeLocation::Workspace => write!(f, "workspace"),
            ComposeLocation::Supabase => write!(f, "supabase"),
            ComposeLocation::Traefik => write!(f, "traefik"),
            ComposeLocation::App => write!(f, "app"),
        }
    }
}

/// Detected application
#[derive(Debug, Clone)]
pub struct DetectedApp {
    pub name: String,
    pub path: String,
    pub framework: Framework,
    pub has_dockerfile: bool,
    #[allow(dead_code)]
    pub package_name: Option<String>,
}

/// Detected library
#[derive(Debug, Clone)]
pub struct DetectedLib {
    pub name: String,
    pub path: String,
    #[allow(dead_code)]
    pub package_name: Option<String>,
}

/// Detected docker-compose file
#[derive(Debug, Clone)]
pub struct DetectedCompose {
    pub path: String,
    pub location: ComposeLocation,
}

/// Result of project discovery
#[derive(Debug, Clone)]
pub struct DiscoveryResult {
    pub apps: Vec<DetectedApp>,
    pub libs: Vec<DetectedLib>,
    pub compose_files: Vec<DetectedCompose>,
    pub catalog: IndexMap<String, String>,
}

impl DiscoveryResult {
    pub fn is_empty(&self) -> bool {
        self.apps.is_empty() && self.libs.is_empty() && self.compose_files.is_empty()
    }
}

/// Run project discovery
pub fn run() -> Result<DiscoveryResult> {
    println!("{}", "🔍 Discovering project structure...".bright_blue());
    println!();

    let apps = scan_apps()?;
    let libs = scan_libs()?;
    let compose_files = find_compose_files()?;
    let catalog = extract_catalog()?;

    let result = DiscoveryResult {
        apps,
        libs,
        compose_files,
        catalog,
    };

    print_discovery_result(&result);

    Ok(result)
}

/// Print discovery results to console
fn print_discovery_result(result: &DiscoveryResult) {
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
            println!(
                "   {:<18} {}",
                lib.path.bright_cyan(),
                "TypeScript".white()
            );
        }
        println!();
    }

    // Compose files
    if !result.compose_files.is_empty() {
        println!("{}", "🐳 Docker Compose Files:".green());
        for compose in &result.compose_files {
            let status = match compose.location {
                ComposeLocation::Root => format!(
                    "{} {}",
                    "→".yellow(),
                    "workspace/docker-compose.yml".yellow()
                ),
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

/// Scan apps/ directory for applications
fn scan_apps() -> Result<Vec<DetectedApp>> {
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

        apps.push(DetectedApp {
            name,
            path: rel_path,
            framework,
            has_dockerfile,
            package_name,
        });
    }

    // Sort by name for consistent output
    apps.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(apps)
}

/// Scan libs/ directory for libraries
fn scan_libs() -> Result<Vec<DetectedLib>> {
    let mut libs = Vec::new();
    let libs_dir = Path::new("libs");

    if !libs_dir.exists() {
        return Ok(libs);
    }

    // Scan top-level libs
    scan_libs_in_dir(&libs_dir, "libs", &mut libs)?;

    // Scan nested libs (e.g., libs/supabase/*)
    let nested_dirs = ["supabase"];
    for nested in nested_dirs {
        let nested_path = libs_dir.join(nested);
        if nested_path.exists() && nested_path.is_dir() {
            scan_libs_in_dir(&nested_path, &format!("libs/{}", nested), &mut libs)?;
        }
    }

    // Sort by path for consistent output
    libs.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(libs)
}

/// Helper to scan libraries in a specific directory
fn scan_libs_in_dir(dir: &Path, prefix: &str, libs: &mut Vec<DetectedLib>) -> Result<()> {
    let entries = fs::read_dir(dir).with_context(|| format!("Failed to read {} directory", prefix))?;

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

        libs.push(DetectedLib {
            name,
            path: rel_path,
            package_name,
        });
    }

    Ok(())
}

/// Detect framework from app directory
fn detect_framework(app_path: &Path) -> Framework {
    // Check for Rust project first
    if app_path.join("Cargo.toml").exists() {
        return Framework::Rust;
    }

    // Check for Python project
    if app_path.join("pyproject.toml").exists()
        || app_path.join("setup.py").exists()
        || app_path.join("requirements.txt").exists()
    {
        return Framework::Python;
    }

    // Check package.json for JS/TS frameworks
    let pkg_json_path = app_path.join("package.json");
    if !pkg_json_path.exists() {
        return Framework::Unknown;
    }

    let content = match fs::read_to_string(&pkg_json_path) {
        Ok(c) => c,
        Err(_) => return Framework::Unknown,
    };

    let json: Value = match serde_json::from_str(&content) {
        Ok(j) => j,
        Err(_) => return Framework::Unknown,
    };

    // Check dependencies for framework detection
    let deps = json["dependencies"].as_object();
    let dev_deps = json["devDependencies"].as_object();

    let has_dep = |name: &str| -> bool {
        deps.is_some_and(|d| d.contains_key(name)) || dev_deps.is_some_and(|d| d.contains_key(name))
    };

    // Priority order: most specific to least specific
    if has_dep("next") {
        Framework::NextJs
    } else if has_dep("hono") {
        Framework::Hono
    } else if has_dep("vite") {
        Framework::Vite
    } else {
        // Default to Node for any JS/TS project with package.json
        Framework::Node
    }
}

/// Get package name from package.json
fn get_package_name(dir: &Path) -> Option<String> {
    let pkg_json_path = dir.join("package.json");
    if !pkg_json_path.exists() {
        return None;
    }

    let content = fs::read_to_string(&pkg_json_path).ok()?;
    let json: Value = serde_json::from_str(&content).ok()?;
    json["name"].as_str().map(|s| s.to_string())
}

/// Find docker-compose files in the workspace
fn find_compose_files() -> Result<Vec<DetectedCompose>> {
    let mut files = Vec::new();

    // Check standard locations
    let locations = [
        ("docker-compose.yml", ComposeLocation::Root),
        ("docker-compose.yaml", ComposeLocation::Root),
        ("workspace/docker-compose.yml", ComposeLocation::Workspace),
        ("workspace/docker-compose.yaml", ComposeLocation::Workspace),
        ("supabase/docker-compose.yml", ComposeLocation::Supabase),
        ("supabase/docker-compose.yaml", ComposeLocation::Supabase),
        ("traefik/docker-compose.yml", ComposeLocation::Traefik),
        ("traefik/docker-compose.yaml", ComposeLocation::Traefik),
    ];

    for (path, location) in locations {
        if Path::new(path).exists() {
            files.push(DetectedCompose {
                path: path.to_string(),
                location,
            });
        }
    }

    // Check for compose files in apps/ directories
    let apps_dir = Path::new("apps");
    if apps_dir.exists() {
        if let Ok(entries) = fs::read_dir(apps_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    for compose_name in ["docker-compose.yml", "docker-compose.yaml"] {
                        let compose_path = path.join(compose_name);
                        if compose_path.exists() {
                            let rel_path = compose_path
                                .strip_prefix(".")
                                .unwrap_or(&compose_path)
                                .to_string_lossy()
                                .to_string();
                            files.push(DetectedCompose {
                                path: rel_path,
                                location: ComposeLocation::App,
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(files)
}

/// Extract catalog entries from root package.json
fn extract_catalog() -> Result<IndexMap<String, String>> {
    extract_catalog_from_path(Path::new("."))
}

/// Extract catalog entries from package.json in the given directory
fn extract_catalog_from_path(base_path: &Path) -> Result<IndexMap<String, String>> {
    let mut catalog = IndexMap::new();

    let pkg_json_path = base_path.join("package.json");
    if !pkg_json_path.exists() {
        return Ok(catalog);
    }

    let content = fs::read_to_string(&pkg_json_path).context("Failed to read package.json")?;
    let json: Value = serde_json::from_str(&content).context("Failed to parse package.json")?;

    // Extract from devDependencies (common location for shared tooling)
    if let Some(dev_deps) = json["devDependencies"].as_object() {
        // Common catalog packages
        let catalog_packages = [
            "typescript",
            "eslint",
            "prettier",
            "@types/node",
            "tsup",
            "vitest",
            "jest",
            "@typescript-eslint/eslint-plugin",
            "@typescript-eslint/parser",
        ];

        for pkg in catalog_packages {
            if let Some(version) = dev_deps.get(pkg).and_then(|v| v.as_str()) {
                // Skip workspace: references
                if !version.starts_with("workspace:") {
                    catalog.insert(pkg.to_string(), version.to_string());
                }
            }
        }
    }

    // Also check pnpm-workspace.yaml for existing catalog
    let pnpm_workspace_path = base_path.join("pnpm-workspace.yaml");
    if pnpm_workspace_path.exists() {
        if let Ok(content) = fs::read_to_string(pnpm_workspace_path) {
            // Simple YAML parsing for catalog section
            // Format: catalog:
            //           package: "version"
            let mut in_catalog = false;
            for line in content.lines() {
                if line.trim() == "catalog:" {
                    in_catalog = true;
                    continue;
                }
                if in_catalog {
                    if !line.starts_with(' ') && !line.starts_with('\t') && !line.is_empty() {
                        break; // End of catalog section
                    }
                    // Parse "  package: version" or "  package: \"version\""
                    let trimmed = line.trim();
                    if let Some((key, value)) = trimmed.split_once(':') {
                        let key = key.trim().trim_matches('"');
                        let value = value.trim().trim_matches('"').trim_matches('\'');
                        if !key.is_empty() && !value.is_empty() {
                            catalog.insert(key.to_string(), value.to_string());
                        }
                    }
                }
            }
        }
    }

    Ok(catalog)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_detect_framework_nextjs() {
        let dir = tempdir().unwrap();
        let pkg_json = r#"{"name": "test", "dependencies": {"next": "14.0.0", "react": "18.0.0"}}"#;
        fs::write(dir.path().join("package.json"), pkg_json).unwrap();

        assert_eq!(detect_framework(dir.path()), Framework::NextJs);
    }

    #[test]
    fn test_detect_framework_vite() {
        let dir = tempdir().unwrap();
        let pkg_json = r#"{"name": "test", "devDependencies": {"vite": "5.0.0"}}"#;
        fs::write(dir.path().join("package.json"), pkg_json).unwrap();

        assert_eq!(detect_framework(dir.path()), Framework::Vite);
    }

    #[test]
    fn test_detect_framework_hono() {
        let dir = tempdir().unwrap();
        let pkg_json = r#"{"name": "test", "dependencies": {"hono": "4.0.0"}}"#;
        fs::write(dir.path().join("package.json"), pkg_json).unwrap();

        assert_eq!(detect_framework(dir.path()), Framework::Hono);
    }

    #[test]
    fn test_detect_framework_rust() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

        assert_eq!(detect_framework(dir.path()), Framework::Rust);
    }

    #[test]
    fn test_detect_framework_python() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("pyproject.toml"), "[project]\nname = \"test\"").unwrap();

        assert_eq!(detect_framework(dir.path()), Framework::Python);
    }

    #[test]
    fn test_detect_framework_node_fallback() {
        let dir = tempdir().unwrap();
        let pkg_json = r#"{"name": "test", "dependencies": {"express": "4.0.0"}}"#;
        fs::write(dir.path().join("package.json"), pkg_json).unwrap();

        assert_eq!(detect_framework(dir.path()), Framework::Node);
    }

    #[test]
    fn test_detect_framework_unknown() {
        let dir = tempdir().unwrap();
        // No package.json or Cargo.toml

        assert_eq!(detect_framework(dir.path()), Framework::Unknown);
    }

    #[test]
    fn test_get_package_name() {
        let dir = tempdir().unwrap();
        let pkg_json = r#"{"name": "@workspace/test-app"}"#;
        fs::write(dir.path().join("package.json"), pkg_json).unwrap();

        assert_eq!(
            get_package_name(dir.path()),
            Some("@workspace/test-app".to_string())
        );
    }

    #[test]
    fn test_extract_catalog() {
        let dir = tempdir().unwrap();
        let pkg_json = r#"{
            "name": "test-workspace",
            "devDependencies": {
                "typescript": "^5.0.0",
                "eslint": "^8.0.0",
                "@workspace/internal": "workspace:*"
            }
        }"#;

        fs::write(dir.path().join("package.json"), pkg_json).unwrap();

        let catalog = extract_catalog_from_path(dir.path()).unwrap();

        assert_eq!(catalog.get("typescript"), Some(&"^5.0.0".to_string()));
        assert_eq!(catalog.get("eslint"), Some(&"^8.0.0".to_string()));
        // workspace: references should be skipped
        assert!(!catalog.contains_key("@workspace/internal"));
    }
}
