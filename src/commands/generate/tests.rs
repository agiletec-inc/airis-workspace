use indexmap::IndexMap;
use std::fs;

use crate::manifest::Manifest;

use super::registry::{load_generation_registry, save_generation_registry};
use super::tsconfig_gen::detect_ts_major;

// ── detect_ts_major ──

#[test]
fn test_detect_ts_major_explicit_override() {
    let mut manifest = default_test_manifest();
    manifest.typescript.version = Some(6);

    let catalog = IndexMap::new();
    assert_eq!(detect_ts_major(&manifest, &catalog), 6);
}

#[test]
fn test_detect_ts_major_from_catalog() {
    let manifest = default_test_manifest();
    let mut catalog = IndexMap::new();
    catalog.insert("typescript".to_string(), "^5.6.0".to_string());
    assert_eq!(detect_ts_major(&manifest, &catalog), 5);
}

#[test]
fn test_detect_ts_major_from_catalog_tilde() {
    let manifest = default_test_manifest();
    let mut catalog = IndexMap::new();
    catalog.insert("typescript".to_string(), "~6.1.0".to_string());
    assert_eq!(detect_ts_major(&manifest, &catalog), 6);
}

#[test]
fn test_detect_ts_major_default() {
    let manifest = default_test_manifest();
    let catalog = IndexMap::new();
    assert_eq!(detect_ts_major(&manifest, &catalog), 5);
}

// ── load_generation_registry ──

#[test]
fn test_load_generation_registry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("generated.toml");
    fs::write(
        &path,
        "# comment\npackage.json\ntsconfig.json\n\npnpm-workspace.yaml\n",
    )
    .unwrap();

    let result = load_generation_registry(&path);
    assert_eq!(
        result,
        vec!["package.json", "tsconfig.json", "pnpm-workspace.yaml"]
    );
}

#[test]
fn test_load_generation_registry_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nonexistent.toml");
    let result = load_generation_registry(&path);
    assert!(result.is_empty());
}

// ── save_generation_registry ──

#[test]
fn test_save_generation_registry_deduplicates_and_sorts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".airis").join("generated.toml");

    let paths = vec![
        "tsconfig.json".to_string(),
        "package.json".to_string(),
        "tsconfig.json".to_string(), // duplicate
    ];
    save_generation_registry(&path, &paths).unwrap();

    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("tsconfig.json"));
    assert!(content.contains("package.json"));
    // Sorted: package.json before tsconfig.json
    let pkg_pos = content.find("package.json").unwrap();
    let ts_pos = content.find("tsconfig.json").unwrap();
    assert!(pkg_pos < ts_pos);
}

/// Helper: create a minimal manifest for testing
fn default_test_manifest() -> Manifest {
    toml::from_str("version = 1\n[project]\nid = \"test\"").unwrap()
}
