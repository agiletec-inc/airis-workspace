//! Framework detection and package name extraction.

use serde_json::Value;
use std::fs;
use std::path::Path;

use super::types::Framework;

/// Detect framework from app directory
pub fn detect_framework(app_path: &Path) -> Framework {
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
pub fn get_package_name(dir: &Path) -> Option<String> {
    let pkg_json_path = dir.join("package.json");
    if !pkg_json_path.exists() {
        return None;
    }

    let content = fs::read_to_string(&pkg_json_path).ok()?;
    let json: Value = serde_json::from_str(&content).ok()?;
    json["name"].as_str().map(|s| s.to_string())
}
