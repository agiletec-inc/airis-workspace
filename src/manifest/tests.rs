use super::validation::levenshtein_distance;
use super::*;
use std::io::Write;

/// Helper: create a minimal valid manifest TOML string
fn minimal_manifest() -> String {
    r#"
version = 1
"#
    .to_string()
}

/// Helper: write a manifest string to a temp file and load it
fn load_from_str(content: &str) -> anyhow::Result<Manifest> {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(content.as_bytes()).unwrap();
    Manifest::load(tmp.path())
}

#[test]
fn test_validate_passes_for_minimal_manifest() {
    let manifest = load_from_str(&minimal_manifest());
    assert!(manifest.is_ok());
}

#[test]
fn test_validate_duplicate_ports() {
    let toml = r#"
version = 1

[service.redis]
image = "redis:7"
port = 6379

[service.cache]
image = "redis:7"
port = 6379
"#;
    let err = load_from_str(toml).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Duplicate port 6379"), "got: {msg}");
    assert!(msg.contains("redis"), "got: {msg}");
    assert!(msg.contains("cache"), "got: {msg}");
}

#[test]
fn test_validate_no_duplicate_when_ports_differ() {
    let toml = r#"
version = 1

[service.redis]
image = "redis:7"
port = 6379

[service.postgres]
image = "postgres:16"
port = 5432
"#;
    assert!(load_from_str(toml).is_ok());
}

#[test]
fn test_validate_skip_none_ports() {
    let toml = r#"
version = 1

[service.redis]
image = "redis:7"

[service.cache]
image = "redis:7"
"#;
    assert!(load_from_str(toml).is_ok());
}

#[test]
fn test_validate_catalog_follow_missing_reference() {
    let toml = r#"
version = 1

[packages.catalog]
react = "latest"

[packages.catalog.react-dom]
follow = "nonexistent"
"#;
    let err = load_from_str(toml).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("react-dom") && msg.contains("nonexistent"),
        "got: {msg}"
    );
}

#[test]
fn test_validate_catalog_follow_valid_reference() {
    let toml = r#"
version = 1

[packages.catalog]
react = "latest"

[packages.catalog.react-dom]
follow = "react"
"#;
    assert!(load_from_str(toml).is_ok());
}

#[test]
fn test_validate_guard_deny_wrap_conflict() {
    let toml = r#"
version = 1

[guards]
deny = ["pnpm"]

[guards.wrap]
pnpm = "docker compose exec workspace pnpm"
"#;
    let err = load_from_str(toml).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("pnpm") && msg.contains("guards.deny") && msg.contains("guards.wrap"),
        "got: {msg}"
    );
}

#[test]
fn test_validate_guard_no_conflict() {
    let toml = r#"
version = 1

[guards]
deny = ["npm", "yarn"]

[guards.wrap]
pnpm = "docker compose exec workspace pnpm"
"#;
    assert!(load_from_str(toml).is_ok());
}

#[test]
fn test_validate_multiple_errors_collected() {
    let toml = r#"
version = 1

[service.a]
image = "redis:7"
port = 6379

[service.b]
image = "redis:7"
port = 6379

[packages.catalog.react-dom]
follow = "missing"

[guards]
deny = ["pnpm"]

[guards.wrap]
pnpm = "docker compose exec workspace pnpm"
"#;
    let err = load_from_str(toml).unwrap_err();
    let msg = err.to_string();
    // All three errors should be present
    assert!(msg.contains("Duplicate port"), "got: {msg}");
    assert!(msg.contains("missing"), "got: {msg}");
    assert!(msg.contains("Guard conflict"), "got: {msg}");
}

#[test]
fn test_resolve_name_from_path() {
    let ws = WorkspaceSection::default();
    let mut app = ProjectDefinition {
        path: Some("apps/corporate".to_string()),
        framework: Some("nextjs".to_string()),
        ..Default::default()
    };
    assert!(app.name.is_empty());
    app.resolve(&ws);
    assert_eq!(app.name, "corporate");
}

#[test]
fn test_resolve_scope_from_workspace() {
    let mut ws = WorkspaceSection::default();
    ws.scope = Some("@myorg".to_string());
    let mut app = ProjectDefinition {
        name: "my-app".to_string(),
        ..Default::default()
    };
    assert!(app.scope.is_none());
    app.resolve(&ws);
    assert_eq!(app.scope.as_deref(), Some("@myorg"));
}

#[test]
fn test_resolve_scope_not_overridden() {
    let mut ws = WorkspaceSection::default();
    ws.scope = Some("@myorg".to_string());
    let mut app = ProjectDefinition {
        name: "my-app".to_string(),
        scope: Some("@custom".to_string()),
        ..Default::default()
    };
    app.resolve(&ws);
    assert_eq!(app.scope.as_deref(), Some("@custom"));
}

#[test]
fn test_resolve_port_from_framework() {
    let ws = WorkspaceSection::default();
    let mut app = ProjectDefinition {
        name: "my-app".to_string(),
        framework: Some("nextjs".to_string()),
        ..Default::default()
    };
    assert!(app.port.is_none());
    app.resolve(&ws);
    assert_eq!(app.port, Some(3000));
}

#[test]
fn test_resolve_deploy_defaults_from_framework() {
    let ws = WorkspaceSection::default();
    let mut app = ProjectDefinition {
        name: "my-app".to_string(),
        framework: Some("nextjs".to_string()),
        deploy: Some(AppDeployConfig {
            enabled: true,
            ..Default::default()
        }),
        ..Default::default()
    };
    app.resolve(&ws);
    let deploy = app.deploy.as_ref().unwrap();
    assert_eq!(deploy.variant.as_deref(), Some("nextjs"));
    assert_eq!(deploy.port, Some(3000));
    assert_eq!(deploy.health_path.as_deref(), Some("/api/health"));
}

#[test]
fn test_resolve_deploy_explicit_not_overridden() {
    let ws = WorkspaceSection::default();
    let mut app = ProjectDefinition {
        name: "my-app".to_string(),
        framework: Some("nextjs".to_string()),
        deploy: Some(AppDeployConfig {
            enabled: true,
            port: Some(8080),
            health_path: Some("/custom-health".to_string()),
            variant: Some("node".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };
    app.resolve(&ws);
    let deploy = app.deploy.as_ref().unwrap();
    assert_eq!(deploy.variant.as_deref(), Some("node"));
    assert_eq!(deploy.port, Some(8080));
    assert_eq!(deploy.health_path.as_deref(), Some("/custom-health"));
}

// ── dep_group / env_group reference validation ──

#[test]
fn test_validate_dep_group_missing_reference() {
    let toml = r#"
version = 1

[dep_group.shadcn]
"@radix-ui/react-slot" = "^1.0.0"

[[app]]
name = "dashboard"
path = "apps/dashboard"
dep_groups = ["shadcn", "nonexistent"]
"#;
    let err = load_from_str(toml).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("nonexistent"), "got: {msg}");
    assert!(msg.contains("dep_group"), "got: {msg}");
}

#[test]
fn test_validate_dep_group_valid_reference() {
    let toml = r#"
version = 1

[dep_group.shadcn]
"@radix-ui/react-slot" = "^1.0.0"

[[app]]
name = "dashboard"
path = "apps/dashboard"
dep_groups = ["shadcn"]
"#;
    assert!(load_from_str(toml).is_ok());
}

#[test]
fn test_validate_dev_dep_group_missing_reference() {
    let toml = r#"
version = 1

[[app]]
name = "dashboard"
path = "apps/dashboard"
dev_dep_groups = ["missing-group"]
"#;
    let err = load_from_str(toml).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("missing-group"), "got: {msg}");
}

#[test]
fn test_validate_env_group_missing_reference() {
    let toml = r#"
version = 1

[env_group.supabase]
SUPABASE_URL = "${SUPABASE_URL}"

[service.api]
image = "node:22"
env_groups = ["supabase", "nonexistent"]
"#;
    let err = load_from_str(toml).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("nonexistent"), "got: {msg}");
    assert!(msg.contains("env_group"), "got: {msg}");
}

#[test]
fn test_validate_env_group_valid_reference() {
    let toml = r#"
version = 1

[env_group.supabase]
SUPABASE_URL = "${SUPABASE_URL}"

[service.api]
image = "node:22"
env_groups = ["supabase"]
"#;
    assert!(load_from_str(toml).is_ok());
}

#[test]
fn test_validate_preset_dep_group_missing_reference() {
    let toml = r#"
version = 1

[preset.nextjs-app]
framework = "nextjs"
dep_groups = ["nonexistent"]
"#;
    let err = load_from_str(toml).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("preset.nextjs-app") && msg.contains("nonexistent"),
        "got: {msg}"
    );
}

// ── catalog follow cycle detection ──

#[test]
fn test_validate_catalog_follow_cycle_direct() {
    let toml = r#"
version = 1

[packages.catalog.a]
follow = "b"

[packages.catalog.b]
follow = "a"
"#;
    let err = load_from_str(toml).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("cycle"), "got: {msg}");
}

#[test]
fn test_validate_catalog_follow_cycle_indirect() {
    let toml = r#"
version = 1

[packages.catalog.a]
follow = "b"

[packages.catalog.b]
follow = "c"

[packages.catalog.c]
follow = "a"
"#;
    let err = load_from_str(toml).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("cycle"), "got: {msg}");
}

#[test]
fn test_validate_catalog_follow_chain_no_cycle() {
    let toml = r#"
version = 1

[packages.catalog]
react = "latest"

[packages.catalog.react-dom]
follow = "react"

[packages.catalog."@types/react"]
follow = "react"
"#;
    assert!(load_from_str(toml).is_ok());
}

// ── env.validation orphan detection ──

#[test]
fn test_validate_env_validation_orphan() {
    let toml = r#"
version = 1

[env]
required = ["DATABASE_URL"]

[env.validation.DATABASE_URL]
pattern = "^postgresql://"

[env.validation.ORPHAN_VAR]
pattern = "^https://"
description = "Some URL"
"#;
    let err = load_from_str(toml).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("ORPHAN_VAR"), "got: {msg}");
    // DATABASE_URL is in required, so it should NOT appear in the error
    assert!(!msg.contains("DATABASE_URL"), "got: {msg}");
}

#[test]
fn test_validate_env_validation_all_declared() {
    let toml = r#"
version = 1

[env]
required = ["DATABASE_URL"]
optional = ["SENTRY_DSN"]

[env.validation.DATABASE_URL]
pattern = "^postgresql://"

[env.validation.SENTRY_DSN]
pattern = "^https://"
"#;
    assert!(load_from_str(toml).is_ok());
}

// ── catalog typo detection ──

#[test]
fn test_validate_catalog_typo_warning_lates() {
    // "lates" is Levenshtein distance 1 from "latest" — should warn but not error
    let toml = r#"
version = 1

[packages.catalog]
react = "lates"
"#;
    // Should load successfully (warning only, not error)
    assert!(load_from_str(toml).is_ok());
}

#[test]
fn test_validate_catalog_no_false_positive_semver() {
    // Semver strings should not trigger typo warnings
    let toml = r#"
version = 1

[packages.catalog]
react = "^18.2.0"
zod = "~3.22.0"
"#;
    assert!(load_from_str(toml).is_ok());
}

// ── levenshtein_distance unit tests ──

#[test]
fn test_levenshtein_distance() {
    assert_eq!(levenshtein_distance("latest", "latest"), 0);
    assert_eq!(levenshtein_distance("lates", "latest"), 1);
    assert_eq!(levenshtein_distance("latets", "latest"), 2);
    assert_eq!(levenshtein_distance("lts", "lts"), 0);
    assert_eq!(levenshtein_distance("lst", "lts"), 2);
    assert_eq!(levenshtein_distance("react", "latest"), 5);
}
