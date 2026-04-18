use crate::manifest::Manifest;

#[test]
fn test_glob_expansion_adds_products_workspaces() {
    // Test that packages.workspaces glob patterns are expanded via filesystem
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Create directories matching "products/*" glob with package.json
    std::fs::create_dir_all(root.join("products/sales-agent")).unwrap();
    std::fs::write(root.join("products/sales-agent/package.json"), "{}").unwrap();
    std::fs::create_dir_all(root.join("products/bidalert")).unwrap();
    std::fs::write(root.join("products/bidalert/package.json"), "{}").unwrap();

    let toml_str = r#"
version = 1
[project]
id = "test"

[workspace]
name = "test-project"
image = "node:22-alpine"
workdir = "/app"
volumes = ["node_modules:/app/node_modules"]

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["products/*"]
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let paths = manifest.all_workspace_paths_in(root.to_str().unwrap());

    // Should contain the two products directories
    assert!(paths.contains(&"products/sales-agent".to_string()));
    assert!(paths.contains(&"products/bidalert".to_string()));
    assert_eq!(paths.len(), 2);
}

#[test]
fn test_glob_expansion_skips_exclude_patterns() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("apps/web")).unwrap();
    std::fs::write(root.join("apps/web/package.json"), "{}").unwrap();

    let toml_str = r#"
version = 1
[project]
id = "test"

[workspace]
name = "test-project"
image = "node:22-alpine"
workdir = "/app"

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "!apps/internal"]
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let paths = manifest.all_workspace_paths_in(root.to_str().unwrap());

    // Should contain apps/web from glob, exclude pattern should be skipped
    assert!(paths.contains(&"apps/web".to_string()));
    assert!(!paths.contains(&"!apps/internal".to_string()));
}

#[test]
fn test_profile_effective_role() {
    use crate::manifest::ProfileSection;
    let default = ProfileSection::default();

    // Name-based inference
    assert_eq!(default.effective_role("prd"), "production");
    assert_eq!(default.effective_role("prod"), "production");
    assert_eq!(default.effective_role("production"), "production");
    assert_eq!(default.effective_role("local"), "local");
    assert_eq!(default.effective_role("dev"), "local");
    assert_eq!(default.effective_role("stg"), "staging");
    assert_eq!(default.effective_role("staging"), "staging");
    assert_eq!(default.effective_role("preview"), "staging");

    // Explicit role overrides name
    let custom = ProfileSection {
        role: Some("production".to_string()),
        ..Default::default()
    };
    assert_eq!(custom.effective_role("stg"), "production");
}
