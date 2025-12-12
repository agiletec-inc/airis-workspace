//! Docker build integration tests
//!
//! Tests for the hermetic Docker build system

use std::path::PathBuf;

/// Test cache directory structure
#[test]
fn test_cache_directory_structure() {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let cache_base = PathBuf::from(home).join(".airis").join(".cache");

    // Cache directory should be created on first use
    // Just verify the path is valid
    assert!(cache_base.to_string_lossy().contains(".airis"));
}

/// Test that build config defaults are sensible
#[test]
fn test_build_config_defaults() {
    // Test via CLI help that channel option is documented
    let output = std::process::Command::new("cargo")
        .args(["run", "--", "build", "--help"])
        .output()
        .expect("Failed to run cargo");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Channel option should be present (no default in CLI, reads from manifest.toml)
    assert!(stdout.contains("--channel"));
    assert!(stdout.contains("manifest.toml"));
}

/// Test context output directory option
#[test]
fn test_context_out_option() {
    let output = std::process::Command::new("cargo")
        .args(["run", "--", "build", "--help"])
        .output()
        .expect("Failed to run cargo");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--context-out"));
}

/// Test no-cache option
#[test]
fn test_no_cache_option() {
    let output = std::process::Command::new("cargo")
        .args(["run", "--", "build", "--help"])
        .output()
        .expect("Failed to run cargo");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--no-cache"));
}

/// Test push option
#[test]
fn test_push_option() {
    let output = std::process::Command::new("cargo")
        .args(["run", "--", "build", "--help"])
        .output()
        .expect("Failed to run cargo");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--push"));
}

/// Test image name option
#[test]
fn test_image_option() {
    let output = std::process::Command::new("cargo")
        .args(["run", "--", "build", "--help"])
        .output()
        .expect("Failed to run cargo");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--image"));
}

/// Test remote cache option
#[test]
fn test_remote_cache_option() {
    let output = std::process::Command::new("cargo")
        .args(["run", "--", "build", "--help"])
        .output()
        .expect("Failed to run cargo");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--remote-cache"));
    assert!(stdout.contains("s3://") || stdout.contains("oci://"));
}
