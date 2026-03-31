//! Tests for project discovery.

use indexmap::IndexMap;
use std::fs;
use tempfile::tempdir;

use super::catalog::{extract_catalog_from_path, extract_package_info};
use super::detection::{detect_framework, get_package_name};
use super::scanning::discover_from_workspaces;
use super::types::Framework;

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
    fs::write(
        dir.path().join("pyproject.toml"),
        "[project]\nname = \"test\"",
    )
    .unwrap();

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

#[test]
fn test_extract_package_info_basic() {
    let dir = tempdir().unwrap();
    let pkg_json = r#"{
            "name": "test-app",
            "scripts": {
                "dev": "next dev",
                "build": "next build"
            },
            "dependencies": {
                "react": "^18.0.0",
                "next": "^14.0.0"
            },
            "devDependencies": {
                "typescript": "^5.0.0"
            }
        }"#;
    fs::write(dir.path().join("package.json"), pkg_json).unwrap();

    let catalog = IndexMap::new();
    let info = extract_package_info(dir.path(), &catalog);

    assert_eq!(info.scripts.get("dev"), Some(&"next dev".to_string()));
    assert_eq!(info.scripts.get("build"), Some(&"next build".to_string()));
    assert_eq!(info.deps.get("react"), Some(&"^18.0.0".to_string()));
    assert_eq!(info.deps.get("next"), Some(&"^14.0.0".to_string()));
    assert_eq!(info.dev_deps.get("typescript"), Some(&"^5.0.0".to_string()));
}

#[test]
fn test_extract_package_info_with_catalog_conversion() {
    let dir = tempdir().unwrap();
    let pkg_json = r#"{
            "name": "test-app",
            "dependencies": {
                "react": "^18.0.0",
                "lodash": "^4.0.0"
            },
            "devDependencies": {
                "typescript": "^5.0.0"
            }
        }"#;
    fs::write(dir.path().join("package.json"), pkg_json).unwrap();

    // Create a catalog with react and typescript
    let mut catalog = IndexMap::new();
    catalog.insert("react".to_string(), "^18.2.0".to_string());
    catalog.insert("typescript".to_string(), "^5.3.0".to_string());

    let info = extract_package_info(dir.path(), &catalog);

    // react and typescript should be converted to "catalog:"
    assert_eq!(info.deps.get("react"), Some(&"catalog:".to_string()));
    assert_eq!(
        info.dev_deps.get("typescript"),
        Some(&"catalog:".to_string())
    );
    // lodash is not in catalog, should keep original version
    assert_eq!(info.deps.get("lodash"), Some(&"^4.0.0".to_string()));
}

#[test]
fn test_extract_package_info_skips_workspace_refs() {
    let dir = tempdir().unwrap();
    let pkg_json = r#"{
            "name": "test-app",
            "dependencies": {
                "react": "^18.0.0",
                "@workspace/ui": "workspace:*"
            }
        }"#;
    fs::write(dir.path().join("package.json"), pkg_json).unwrap();

    let catalog = IndexMap::new();
    let info = extract_package_info(dir.path(), &catalog);

    assert_eq!(info.deps.get("react"), Some(&"^18.0.0".to_string()));
    // workspace: references should be skipped
    assert!(!info.deps.contains_key("@workspace/ui"));
}

#[test]
fn test_discover_from_workspaces_basic() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create apps/corporate with Next.js
    let app_dir = root.join("apps/corporate");
    fs::create_dir_all(&app_dir).unwrap();
    fs::write(
        app_dir.join("package.json"),
        r#"{"name": "corporate", "dependencies": {"next": "15.0.0"}}"#,
    )
    .unwrap();

    // Create libs/ui with plain Node
    let lib_dir = root.join("libs/ui");
    fs::create_dir_all(&lib_dir).unwrap();
    fs::write(
        lib_dir.join("package.json"),
        r#"{"name": "ui", "dependencies": {"react": "19.0.0"}}"#,
    )
    .unwrap();

    // Create a non-project directory (no package.json)
    fs::create_dir_all(root.join("apps/empty")).unwrap();

    let patterns = vec!["apps/*".to_string(), "libs/*".to_string()];
    let discovered = discover_from_workspaces(&patterns, root).unwrap();

    assert_eq!(discovered.len(), 2);
    assert_eq!(discovered[0].name, "corporate");
    assert_eq!(discovered[0].path, "apps/corporate");
    assert_eq!(discovered[0].framework, Framework::NextJs);
    assert_eq!(discovered[1].name, "ui");
    assert_eq!(discovered[1].path, "libs/ui");
}

#[test]
fn test_discover_from_workspaces_excludes_negated() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let app_dir = root.join("apps/web");
    fs::create_dir_all(&app_dir).unwrap();
    fs::write(app_dir.join("package.json"), r#"{"name": "web"}"#).unwrap();

    let next_dir = root.join("apps/web/.next");
    fs::create_dir_all(&next_dir).unwrap();
    fs::write(next_dir.join("package.json"), r#"{"name": "junk"}"#).unwrap();

    let patterns = vec!["apps/**".to_string(), "!**/.next".to_string()];
    let discovered = discover_from_workspaces(&patterns, root).unwrap();

    // .next should be excluded
    assert!(discovered.iter().all(|p| !p.path.contains(".next")));
    assert!(discovered.iter().any(|p| p.name == "web"));
}

#[test]
fn test_discover_from_workspaces_nested_products() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create products/airis/voice-gateway
    let vg_dir = root.join("products/airis/voice-gateway");
    fs::create_dir_all(&vg_dir).unwrap();
    fs::write(
        vg_dir.join("package.json"),
        r#"{"name": "voice-gateway", "dependencies": {"hono": "4.0.0"}}"#,
    )
    .unwrap();

    let patterns = vec!["products/**".to_string()];
    let discovered = discover_from_workspaces(&patterns, root).unwrap();

    assert!(discovered.iter().any(|p| p.name == "voice-gateway"));
    let vg = discovered
        .iter()
        .find(|p| p.name == "voice-gateway")
        .unwrap();
    assert_eq!(vg.framework, Framework::Hono);
}
