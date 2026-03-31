use indexmap::IndexMap;
use std::fs;

use crate::manifest::{CatalogEntry, InjectValue, Manifest};

use super::catalog::{matches_wildcard_catalog, wildcard_matches};
use super::inject::resolve_inject_values;
use super::registry::{load_generation_registry, save_generation_registry};
use super::tsconfig_gen::detect_ts_major;
use super::detect_legacy_compose_files;

// ── wildcard_matches ──

#[test]
fn test_wildcard_matches_prefix() {
    assert!(wildcard_matches(
        "@radix-ui/react-*",
        "@radix-ui/react-slot"
    ));
    assert!(wildcard_matches(
        "@radix-ui/react-*",
        "@radix-ui/react-dialog"
    ));
    assert!(!wildcard_matches("@radix-ui/react-*", "@radix-ui/themes"));
}

#[test]
fn test_wildcard_matches_exact() {
    assert!(wildcard_matches("react", "react"));
    assert!(!wildcard_matches("react", "react-dom"));
}

#[test]
fn test_wildcard_matches_star_only() {
    // "*" matches everything
    assert!(wildcard_matches("*", "anything"));
    assert!(wildcard_matches("*", ""));
}

// ── matches_wildcard_catalog ──

#[test]
fn test_matches_wildcard_catalog_hit() {
    let entry = CatalogEntry::Policy(crate::manifest::VersionPolicy::Latest);
    let wildcards = vec![("@radix-ui/react-*", &entry)];
    assert!(matches_wildcard_catalog("@radix-ui/react-slot", &wildcards));
}

#[test]
fn test_matches_wildcard_catalog_miss() {
    let entry = CatalogEntry::Policy(crate::manifest::VersionPolicy::Latest);
    let wildcards = vec![("@radix-ui/react-*", &entry)];
    assert!(!matches_wildcard_catalog("zod", &wildcards));
}

#[test]
fn test_matches_wildcard_catalog_empty() {
    let wildcards: Vec<(&str, &CatalogEntry)> = vec![];
    assert!(!matches_wildcard_catalog("react", &wildcards));
}

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

// ── resolve_inject_values ──

#[test]
fn test_resolve_inject_simple() {
    let mut inject = IndexMap::new();
    inject.insert(
        "my-key".to_string(),
        InjectValue::Simple("hello world".to_string()),
    );

    let catalog = IndexMap::new();
    let result = resolve_inject_values(&inject, &catalog).unwrap();
    assert_eq!(result.get("my-key").unwrap(), "hello world");
}

#[test]
fn test_resolve_inject_template() {
    let mut inject = IndexMap::new();
    inject.insert(
        "sdk-version".to_string(),
        InjectValue::Template {
            template: "SDK_VERSION = \"{version}\"".to_string(),
            from_catalog: "my-sdk".to_string(),
        },
    );

    let mut catalog = IndexMap::new();
    catalog.insert("my-sdk".to_string(), "^2.5.0".to_string());

    let result = resolve_inject_values(&inject, &catalog).unwrap();
    assert_eq!(
        result.get("sdk-version").unwrap(),
        "SDK_VERSION = \"2.5.0\""
    );
}

#[test]
fn test_resolve_inject_template_missing_catalog() {
    let mut inject = IndexMap::new();
    inject.insert(
        "key".to_string(),
        InjectValue::Template {
            template: "v{version}".to_string(),
            from_catalog: "nonexistent".to_string(),
        },
    );

    let catalog = IndexMap::new();
    let result = resolve_inject_values(&inject, &catalog).unwrap();
    // Should skip when catalog entry is missing
    assert!(result.get("key").is_none());
}

// ── load_generation_registry ──

#[test]
fn test_load_generation_registry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("generated.toml");
    fs::write(
        &path,
        "# comment\npackage.json\ncompose.yml\n\npnpm-workspace.yaml\n",
    )
    .unwrap();

    let result = load_generation_registry(&path);
    assert_eq!(
        result,
        vec!["package.json", "compose.yml", "pnpm-workspace.yaml"]
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
        "compose.yml".to_string(),
        "package.json".to_string(),
        "compose.yml".to_string(), // duplicate
    ];
    save_generation_registry(&path, &paths).unwrap();

    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("compose.yml"));
    assert!(content.contains("package.json"));
    // Sorted: compose.yml before package.json
    let compose_pos = content.find("compose.yml").unwrap();
    let pkg_pos = content.find("package.json").unwrap();
    assert!(compose_pos < pkg_pos);
}

// ── detect_legacy_compose_files (filesystem test) ──

#[test]
fn test_detect_legacy_compose_files() {
    let _guard = crate::test_lock::DIR_LOCK.lock().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();

    let result = std::panic::catch_unwind(|| {
        // No legacy files exist
        assert!(detect_legacy_compose_files().is_empty());

        // Create a legacy file
        fs::write("docker-compose.yml", "version: '3'").unwrap();
        let found = detect_legacy_compose_files();
        assert!(found.contains(&"docker-compose.yml".to_string()));
    });

    std::env::set_current_dir(original_dir).unwrap();
    result.unwrap();
}

/// Helper: create a minimal manifest for testing
fn default_test_manifest() -> Manifest {
    toml::from_str("version = 1").unwrap()
}
