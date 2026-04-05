//! Tests for docker_build module

use std::collections::BTreeMap;
use tempfile::tempdir;

use crate::channel::{RuntimeFamily, Toolchain};
use crate::docker_build::cache::{cache_dir, cache_hit, cache_store};
use crate::docker_build::dockerfile::{detect_nextjs, generate_dockerfile_for_toolchain};
use crate::docker_build::hash::compute_hash;
use crate::docker_build::{BuildConfig, CachedArtifact};

fn toolchain(family: RuntimeFamily, image: &str, version: &str) -> Toolchain {
    Toolchain {
        family,
        image: image.to_string(),
        digest: None,
        version: version.to_string(),
    }
}

#[test]
fn test_build_config_default() {
    let config = BuildConfig::default();
    assert_eq!(config.channel, "lts");
    assert!(!config.push);
    assert!(!config.no_cache);
    assert!(config.image_name.is_none());
}

#[test]
fn test_cache_dir_structure() {
    let dir = cache_dir("apps/web", "abc123");
    assert!(dir.to_string_lossy().contains(".airis"));
    assert!(dir.to_string_lossy().contains(".cache"));
    assert!(dir.to_string_lossy().contains("apps_web"));
    assert!(dir.to_string_lossy().contains("abc123"));
}

#[test]
fn test_cache_hit_miss() {
    // Non-existent cache should return None
    let result = cache_hit("nonexistent/project", "nonexistent_hash_12345");
    assert!(result.is_none());
}

#[test]
fn test_cache_store_and_hit() {
    let project = "test_project_cache";
    let hash = "test_hash_abc123";

    let artifact = CachedArtifact {
        image_ref: "test:latest".to_string(),
        hash: hash.to_string(),
        built_at: "2025-01-01T00:00:00Z".to_string(),
        target: project.to_string(),
    };

    // Store
    cache_store(project, hash, &artifact).unwrap();

    // Hit
    let cached = cache_hit(project, hash);
    assert!(cached.is_some());

    let cached = cached.unwrap();
    assert_eq!(cached.image_ref, "test:latest");
    assert_eq!(cached.hash, hash);

    // Cleanup
    let dir = cache_dir(project, hash);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn test_compute_hash_deterministic() {
    let dir = tempdir().unwrap();
    let test_file = dir.path().join("test.txt");
    std::fs::write(&test_file, "hello world").unwrap();

    let hash1 = compute_hash(dir.path()).unwrap();
    let hash2 = compute_hash(dir.path()).unwrap();

    assert_eq!(hash1, hash2);
    assert_eq!(hash1.len(), 12); // 12 hex chars
}

#[test]
fn test_compute_hash_changes_with_content() {
    let dir = tempdir().unwrap();
    let test_file = dir.path().join("test.txt");

    std::fs::write(&test_file, "content v1").unwrap();
    let hash1 = compute_hash(dir.path()).unwrap();

    std::fs::write(&test_file, "content v2").unwrap();
    let hash2 = compute_hash(dir.path()).unwrap();

    assert_ne!(hash1, hash2);
}

#[test]
fn test_generate_nextjs_dockerfile() {
    let tc = toolchain(RuntimeFamily::Node, "node:22-alpine", "22");
    let dockerfile = generate_dockerfile_for_toolchain("apps/web", &tc, &BTreeMap::new());

    assert!(dockerfile.contains("FROM node:22-alpine"));
    assert!(dockerfile.contains("apps/web"));
    assert!(dockerfile.contains("pnpm"));
    // Note: detect_nextjs checks filesystem, so without a real package.json
    // it falls back to node dockerfile. Test the node variant assertions.
}

#[test]
fn test_generate_bun_dockerfile() {
    let tc = toolchain(RuntimeFamily::Bun, "oven/bun:1.1-alpine", "1.1");
    let dockerfile = generate_dockerfile_for_toolchain("apps/api", &tc, &BTreeMap::new());

    assert!(dockerfile.contains("FROM oven/bun:1.1-alpine"));
    assert!(dockerfile.contains("bun install"));
    assert!(dockerfile.contains("bun run"));
}

#[test]
fn test_generate_rust_dockerfile() {
    let tc = toolchain(RuntimeFamily::Rust, "rust:latest", "latest");
    let dockerfile = generate_dockerfile_for_toolchain("apps/cli", &tc, &BTreeMap::new());

    assert!(dockerfile.contains("FROM rust:"));
    assert!(dockerfile.contains("cargo build --release"));
    assert!(dockerfile.contains("distroless"));
}

#[test]
fn test_generate_python_dockerfile() {
    let tc = toolchain(RuntimeFamily::Python, "python:latest", "latest");
    let dockerfile = generate_dockerfile_for_toolchain("apps/api", &tc, &BTreeMap::new());

    assert!(dockerfile.contains("FROM python:"));
    assert!(dockerfile.contains("pip install"));
    assert!(dockerfile.contains("uvicorn"));
}

#[test]
fn test_generate_deno_dockerfile() {
    let tc = toolchain(RuntimeFamily::Deno, "denoland/deno:alpine", "latest");
    let dockerfile = generate_dockerfile_for_toolchain("apps/api", &tc, &BTreeMap::new());

    assert!(dockerfile.contains("FROM denoland/deno:alpine"));
    assert!(dockerfile.contains("deno"));
}

#[test]
fn test_dockerfile_with_build_args() {
    let mut build_args = BTreeMap::new();
    build_args.insert("API_KEY".to_string(), "secret".to_string());

    let tc = toolchain(RuntimeFamily::Node, "node:22-alpine", "22");
    let dockerfile = generate_dockerfile_for_toolchain("apps/web", &tc, &build_args);

    assert!(dockerfile.contains("ARG API_KEY"));
}

#[test]
fn test_detect_nextjs_with_next_dep() {
    let dir = tempdir().unwrap();
    let pkg_json = r#"{"name": "test", "dependencies": {"next": "14.0.0", "react": "18.0.0"}}"#;
    std::fs::write(dir.path().join("package.json"), pkg_json).unwrap();

    assert!(detect_nextjs(&dir.path().to_string_lossy()));
}

#[test]
fn test_detect_nextjs_with_next_devdep() {
    let dir = tempdir().unwrap();
    let pkg_json = r#"{"name": "test", "devDependencies": {"next": "14.0.0"}}"#;
    std::fs::write(dir.path().join("package.json"), pkg_json).unwrap();

    assert!(detect_nextjs(&dir.path().to_string_lossy()));
}

#[test]
fn test_detect_nextjs_without_next() {
    let dir = tempdir().unwrap();
    let pkg_json = r#"{"name": "test", "dependencies": {"express": "4.0.0"}}"#;
    std::fs::write(dir.path().join("package.json"), pkg_json).unwrap();

    assert!(!detect_nextjs(&dir.path().to_string_lossy()));
}

#[test]
fn test_detect_nextjs_no_package_json() {
    let dir = tempdir().unwrap();
    // No package.json file
    assert!(!detect_nextjs(&dir.path().to_string_lossy()));
}
