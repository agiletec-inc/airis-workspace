//! Tests for new command scaffolding

use super::*;
use tempfile::TempDir;

#[test]
fn test_get_base_dir() {
    assert_eq!(get_base_dir("api"), "apps");
    assert_eq!(get_base_dir("web"), "apps");
    assert_eq!(get_base_dir("lib"), "libs");
    assert_eq!(get_base_dir("worker"), "apps");
    assert_eq!(get_base_dir("edge"), "supabase/functions");
}

#[test]
fn test_empty_name_rejected() {
    let result = run_with_runtime("api", "", "hono");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cannot be empty"));
}

#[test]
fn test_invalid_name_rejected() {
    let result = run_with_runtime("api", "my app", "hono");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("alphanumeric"));
}

#[test]
fn test_generate_api_project() {
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path().join("test-api");

    api::generate_api_project(&project_dir, "test-api").unwrap();

    assert!(project_dir.join("package.json").exists());
    assert!(project_dir.join("tsconfig.json").exists());
    assert!(project_dir.join("src/index.ts").exists());
    assert!(project_dir.join("src/routes/health.ts").exists());
    assert!(project_dir.join("Dockerfile").exists());
}

#[test]
fn test_generate_lib_project() {
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path().join("test-lib");

    lib::generate_lib_project(&project_dir, "test-lib").unwrap();

    assert!(project_dir.join("package.json").exists());
    assert!(project_dir.join("tsconfig.json").exists());
    assert!(project_dir.join("src/index.ts").exists());
}

#[test]
fn test_generate_rust_service() {
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path().join("test-rust");

    rust::generate_rust_service(&project_dir, "test-rust").unwrap();

    assert!(project_dir.join("Cargo.toml").exists());
    assert!(project_dir.join("src/main.rs").exists());
    assert!(project_dir.join("Dockerfile").exists());
}

#[test]
fn test_generate_py_lib() {
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path().join("test-lib");

    python::generate_py_lib(&project_dir, "test-lib").unwrap();

    assert!(project_dir.join("pyproject.toml").exists());
    assert!(project_dir.join("src/test_lib/__init__.py").exists());
    assert!(project_dir.join(".gitignore").exists());
}

#[test]
fn test_generate_py_api() {
    let temp_dir = TempDir::new().unwrap();
    let project_dir = temp_dir.path().join("test-py");

    python::generate_py_api(&project_dir, "test-py").unwrap();

    assert!(project_dir.join("pyproject.toml").exists());
    assert!(project_dir.join("app/main.py").exists());
    assert!(project_dir.join("Dockerfile").exists());
}
