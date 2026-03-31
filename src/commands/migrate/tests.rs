//! Tests for the migrate command

use super::*;
use crate::commands::discover::{
    ComposeLocation, DetectedApp, DetectedCompose, DetectedLib, DiscoveryResult, Framework,
};
use indexmap::IndexMap;
use tempfile::tempdir;

fn create_test_discovery() -> DiscoveryResult {
    let mut scripts = IndexMap::new();
    scripts.insert("dev".to_string(), "next dev".to_string());
    scripts.insert("build".to_string(), "next build".to_string());

    let mut deps = IndexMap::new();
    deps.insert("react".to_string(), "catalog:".to_string());
    deps.insert("next".to_string(), "catalog:".to_string());

    let mut dev_deps = IndexMap::new();
    dev_deps.insert("typescript".to_string(), "catalog:".to_string());

    let mut lib_scripts = IndexMap::new();
    lib_scripts.insert("build".to_string(), "tsup".to_string());

    DiscoveryResult {
        apps: vec![DetectedApp {
            name: "web".to_string(),
            path: "apps/web".to_string(),
            framework: Framework::NextJs,
            has_dockerfile: true,
            package_name: Some("@workspace/web".to_string()),
            scripts,
            deps,
            dev_deps,
        }],
        libs: vec![DetectedLib {
            name: "ui".to_string(),
            path: "libs/ui".to_string(),
            package_name: Some("@workspace/ui".to_string()),
            scripts: lib_scripts,
            deps: IndexMap::new(),
            dev_deps: IndexMap::new(),
        }],
        compose_files: vec![DetectedCompose {
            path: "docker-compose.yml".to_string(),
            location: ComposeLocation::Root,
        }],
        catalog: {
            let mut m = IndexMap::new();
            m.insert("typescript".to_string(), "^5.0.0".to_string());
            m
        },
    }
}

#[test]
fn test_plan_creates_workspace_dir_task() {
    let discovery = create_test_discovery();
    let plan = plan(discovery).unwrap();

    // Should have CreateDirectory task for workspace/
    assert!(plan.tasks.iter().any(|t| matches!(
        t,
        MigrationTask::CreateDirectory { path } if path == "workspace"
    )));
}

#[test]
fn test_plan_creates_move_task_for_root_compose() {
    let discovery = create_test_discovery();
    let plan = plan(discovery).unwrap();

    // Should have MoveFile task
    assert!(plan.tasks.iter().any(|t| matches!(
        t,
        MigrationTask::MoveFile { from, to }
        if from == "docker-compose.yml" && to == "workspace/docker-compose.yml"
    )));
}

#[test]
fn test_plan_always_includes_generate_manifest() {
    let discovery = create_test_discovery();
    let plan = plan(discovery).unwrap();

    // Should always have GenerateManifest task
    assert!(
        plan.tasks
            .iter()
            .any(|t| matches!(t, MigrationTask::GenerateManifest))
    );
}

#[test]
fn test_generate_manifest_content() {
    use super::manifest_gen::generate_manifest_content;

    let discovery = create_test_discovery();
    let content = generate_manifest_content(&discovery).unwrap();

    assert!(content.contains("version = 1"));
    // New format uses [[app]] instead of [apps.name]
    assert!(content.contains("[[app]]"));
    assert!(content.contains("name = \"web\""));
    assert!(content.contains("framework = \"nextjs\""));
    assert!(content.contains("kind = \"app\""));
    // Check library is also using [[app]] with kind = "lib"
    assert!(content.contains("kind = \"lib\""));
    assert!(content.contains("name = \"ui\""));
    // Check catalog
    assert!(content.contains("[packages.catalog]"));
    assert!(content.contains("typescript"));
    // Check scripts/deps are included
    assert!(content.contains("scripts = {"));
    assert!(content.contains("deps = {"));
}

#[test]
fn test_dry_run_does_not_create_files() {
    let dir = tempdir().unwrap();

    let discovery = DiscoveryResult {
        apps: vec![],
        libs: vec![],
        compose_files: vec![],
        catalog: IndexMap::new(),
    };

    let migration_plan = plan(discovery).unwrap();
    let _report = execute_in_dir(&migration_plan, true, dir.path()).unwrap();

    // manifest.toml should NOT exist after dry-run
    assert!(!dir.path().join("manifest.toml").exists());
}

#[test]
fn test_execute_creates_manifest() {
    let dir = tempdir().unwrap();

    let discovery = DiscoveryResult {
        apps: vec![],
        libs: vec![],
        compose_files: vec![],
        catalog: IndexMap::new(),
    };

    let migration_plan = plan(discovery).unwrap();
    let report = execute_in_dir(&migration_plan, false, dir.path()).unwrap();

    // manifest.toml should exist
    assert!(dir.path().join("manifest.toml").exists());
    assert!(!report.has_errors());
}
