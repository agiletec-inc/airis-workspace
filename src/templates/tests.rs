use crate::manifest::Manifest;
use crate::templates::TemplateEngine;
use indexmap::IndexMap;

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
image = "node:24-alpine"
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
image = "node:24-alpine"
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

// =============================================================================
// render_package_json — [root] section integration
// =============================================================================

/// Helper: render root package.json from a TOML string and return parsed JSON.
fn render_pkg(toml_str: &str, catalog: &IndexMap<String, String>) -> serde_json::Value {
    let manifest: Manifest = toml::from_str(toml_str).expect("valid toml");
    let engine = TemplateEngine::new().expect("TemplateEngine::new");
    let content = engine
        .render_package_json(&manifest, catalog)
        .expect("render_package_json must succeed");
    serde_json::from_str(&content).expect("valid JSON output")
}

#[test]
fn test_render_package_json_root_dev_deps_appear_in_output() {
    let toml_str = r#"
version = 1
[project]
id = "test"

[workspace]
name = "my-workspace"

[root.devDependencies]
vitest                = "catalog:"
jsdom                 = "catalog:"
"@vitest/coverage-v8" = "catalog:"
"#;
    let catalog = IndexMap::new();
    let json = render_pkg(toml_str, &catalog);
    let dev_deps = json["devDependencies"]
        .as_object()
        .expect("devDependencies must exist");
    assert!(
        dev_deps.contains_key("vitest"),
        "vitest must be in devDependencies"
    );
    assert_eq!(dev_deps["vitest"], "catalog:");
    assert!(
        dev_deps.contains_key("jsdom"),
        "jsdom must be in devDependencies"
    );
    assert!(
        dev_deps.contains_key("@vitest/coverage-v8"),
        "@vitest/coverage-v8 must be in devDependencies"
    );
}

#[test]
fn test_render_package_json_root_scripts_appear_in_output() {
    let toml_str = r#"
version = 1
[project]
id = "test"

[workspace]
name = "my-workspace"

[root.scripts]
test = "vitest run"
"#;
    let catalog = IndexMap::new();
    let json = render_pkg(toml_str, &catalog);
    let scripts = json["scripts"].as_object().expect("scripts must exist");
    assert_eq!(scripts["test"], "vitest run");
}

#[test]
fn test_render_package_json_root_deps_use_catalog_when_in_catalog() {
    // [root.devDependencies] versions referencing a package present in [packages.catalog]
    // must be normalised to "catalog:" by the generator.
    let toml_str = r#"
version = 1
[project]
id = "test"

[workspace]
name = "my-workspace"

[root.devDependencies]
vitest = "^3.0.0"
jsdom  = "^26.0.0"
"#;
    let mut catalog = IndexMap::new();
    catalog.insert("vitest".to_string(), "^3.0.0".to_string());
    // jsdom is NOT in the catalog — should keep the explicit version.
    let json = render_pkg(toml_str, &catalog);
    let dev_deps = json["devDependencies"]
        .as_object()
        .expect("devDependencies must exist");
    assert_eq!(
        dev_deps["vitest"], "catalog:",
        "catalog entry must become 'catalog:'"
    );
    assert_eq!(
        dev_deps["jsdom"], "^26.0.0",
        "non-catalog entry must keep explicit version"
    );
}

#[test]
fn test_render_package_json_root_overrides_packages_root_scripts() {
    // [root.scripts] must override same-name entries from [packages.root.scripts].
    let toml_str = r#"
version = 1
[project]
id = "test"

[workspace]
name = "my-workspace"

[packages.root.scripts]
test = "jest"
build = "tsc"

[root.scripts]
test = "vitest run"
"#;
    let catalog = IndexMap::new();
    let json = render_pkg(toml_str, &catalog);
    let scripts = json["scripts"].as_object().expect("scripts must exist");
    // [root.scripts] wins over [packages.root.scripts] for "test".
    assert_eq!(
        scripts["test"], "vitest run",
        "[root.scripts] must override [packages.root.scripts]"
    );
    // [packages.root.scripts] entry not overridden by [root] must be preserved.
    assert_eq!(
        scripts["build"], "tsc",
        "non-conflicting [packages.root.scripts] entry must survive"
    );
}

#[test]
fn test_render_package_json_root_overrides_packages_root_dev_deps() {
    // [root.devDependencies] must override same-name entries from [packages.root.devDependencies].
    let toml_str = r#"
version = 1
[project]
id = "test"

[workspace]
name = "my-workspace"

[packages.root.devDependencies]
turbo = "^2.0.0"
vitest = "^2.0.0"

[root.devDependencies]
vitest = "catalog:"
"#;
    let catalog = IndexMap::new();
    let json = render_pkg(toml_str, &catalog);
    let dev_deps = json["devDependencies"]
        .as_object()
        .expect("devDependencies must exist");
    // [root.devDependencies] wins over [packages.root.devDependencies] for "vitest".
    assert_eq!(
        dev_deps["vitest"], "catalog:",
        "[root.devDependencies] must override [packages.root.devDependencies]"
    );
    // Non-conflicting entry from packages.root must survive.
    assert_eq!(
        dev_deps["turbo"], "^2.0.0",
        "non-conflicting [packages.root.devDependencies] entry must survive"
    );
}

#[test]
fn test_render_package_json_no_root_section_preserves_packages_root() {
    // When [root] is absent the existing [packages.root] behavior is unchanged.
    let toml_str = r#"
version = 1
[project]
id = "test"

[workspace]
name = "my-workspace"

[packages.root.devDependencies]
turbo = "catalog:"
typescript = "catalog:"

[packages.root.scripts]
build = "turbo build"
"#;
    let mut catalog = IndexMap::new();
    catalog.insert("turbo".to_string(), "^2.0.0".to_string());
    catalog.insert("typescript".to_string(), "latest".to_string());
    let json = render_pkg(toml_str, &catalog);
    let dev_deps = json["devDependencies"]
        .as_object()
        .expect("devDependencies must exist");
    assert_eq!(dev_deps["turbo"], "catalog:");
    assert_eq!(dev_deps["typescript"], "catalog:");
    let scripts = json["scripts"].as_object().expect("scripts must exist");
    assert_eq!(scripts["build"], "turbo build");
}

#[test]
fn test_render_package_json_agiletec_like_root_section() {
    // Simulate the agiletec manifest.toml [root] section added by PR #435.
    // After this change, airis gen must include vitest/jsdom/@vitest/coverage-v8
    // in the generated root package.json devDependencies.
    let toml_str = r#"
version = 1
[project]
id = "agiletec"

[workspace]
name = "agiletec"
scope = "@agiletec"
package_manager = "pnpm@10.33.0"
image = "node:24-bookworm"

[packages.catalog]
vitest             = "^3.0.0"
jsdom              = "^26.0.0"
"@vitest/coverage-v8" = "^3.0.0"

[packages.root.devDependencies]
turbo = "^2.9.6"

[root.devDependencies]
vitest                = "catalog:"
jsdom                 = "catalog:"
"@vitest/coverage-v8" = "catalog:"

[root.scripts]
test = "vitest run"
"#;
    let mut catalog = IndexMap::new();
    catalog.insert("vitest".to_string(), "^3.0.0".to_string());
    catalog.insert("jsdom".to_string(), "^26.0.0".to_string());
    catalog.insert("@vitest/coverage-v8".to_string(), "^3.0.0".to_string());

    let json = render_pkg(toml_str, &catalog);

    // vitest, jsdom, and @vitest/coverage-v8 must appear in devDependencies.
    let dev_deps = json["devDependencies"]
        .as_object()
        .expect("devDependencies must exist");
    assert!(
        dev_deps.contains_key("vitest"),
        "vitest must survive airis gen"
    );
    assert!(
        dev_deps.contains_key("jsdom"),
        "jsdom must survive airis gen"
    );
    assert!(
        dev_deps.contains_key("@vitest/coverage-v8"),
        "@vitest/coverage-v8 must survive airis gen"
    );
    // All three are in catalog — must be normalised to "catalog:".
    assert_eq!(dev_deps["vitest"], "catalog:");
    assert_eq!(dev_deps["jsdom"], "catalog:");
    assert_eq!(dev_deps["@vitest/coverage-v8"], "catalog:");

    // The pre-existing turbo entry from [packages.root.devDependencies] must survive.
    assert!(
        dev_deps.contains_key("turbo"),
        "turbo from packages.root must survive"
    );

    // test script must appear.
    let scripts = json["scripts"].as_object().expect("scripts must exist");
    assert_eq!(
        scripts["test"], "vitest run",
        "test script must survive airis gen"
    );
}
