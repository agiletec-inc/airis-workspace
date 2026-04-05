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
fn test_validate_guard_deny_invalid_command_name() {
    let toml = r#"
version = 1

[guards]
deny = ["npm", "bad command", "../escape"]
"#;
    let err = load_from_str(toml).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("bad command"), "got: {msg}");
    assert!(msg.contains("../escape"), "got: {msg}");
}

#[test]
fn test_validate_guard_wrap_invalid_command_name() {
    let toml = r#"
version = 1

[guards.wrap]
"npm;evil" = "docker compose exec workspace npm"
"#;
    let err = load_from_str(toml).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("npm;evil"), "got: {msg}");
}

#[test]
fn test_validate_guard_wrap_dangerous_wrapper_value() {
    let toml = r#"
version = 1

[guards.wrap]
pnpm = "docker $(whoami) pnpm"
"#;
    let err = load_from_str(toml).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("dangerous"), "got: {msg}");
}

#[test]
fn test_validate_guard_deny_with_message_invalid_name() {
    // This is the exact bug that caused airis gen to fail in agiletec:
    // "docker run" has a space, which is rejected by the command name regex
    let toml = r#"
version = 1

[guards.deny_with_message]
"docker run" = "Use 'airis up' instead."
"#;
    let err = load_from_str(toml).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("docker run"), "got: {msg}");
}

#[test]
fn test_validate_rejects_workspace_bind_mounts() {
    let toml = r#"
version = 1

[workspace]
volumes = ["./:/app"]
"#;
    let err = load_from_str(toml).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("[workspace].volumes"), "got: {msg}");
    assert!(msg.contains("bind mount"), "got: {msg}");
}

#[test]
fn test_validate_rejects_service_bind_mounts() {
    let toml = r#"
version = 1

[service.web]
image = "node:22"
volumes = ["./apps/web:/app"]
"#;
    let err = load_from_str(toml).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("[service.web].volumes"), "got: {msg}");
    assert!(msg.contains("bind mount"), "got: {msg}");
}

#[test]
fn test_validate_allows_named_volumes() {
    let toml = r#"
version = 1

[workspace]
volumes = ["workspace-node-modules:/app/node_modules"]

[service.web]
image = "node:22"
volumes = ["web-data:/app/data"]
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
    let ws = WorkspaceSection {
        scope: Some("@myorg".to_string()),
        ..Default::default()
    };
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
    let ws = WorkspaceSection {
        scope: Some("@myorg".to_string()),
        ..Default::default()
    };
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

// ── Testing governance section ──

#[test]
fn test_testing_section_defaults_when_absent() {
    let manifest = load_from_str(&minimal_manifest()).unwrap();
    assert_eq!(manifest.testing.mock_policy, schema::MockPolicy::Forbidden);
    assert!(manifest.testing.ai_rules.is_empty());
    assert!(manifest.testing.forbidden_patterns.is_empty());
    assert!(manifest.testing.type_enforcement.is_none());
    assert_eq!(manifest.testing.coverage.unit, 0);
    assert_eq!(manifest.testing.coverage.integration, 0);
    assert!(manifest.testing.levels.unit);
    assert!(!manifest.testing.levels.integration);
    assert!(!manifest.testing.levels.e2e);
    assert!(!manifest.testing.levels.smoke);
    assert!(manifest.testing.smoke.is_empty());
}

#[test]
fn test_testing_section_full_config() {
    let toml = r#"
version = 1

[testing]
mock_policy = "unit-only"
forbidden_patterns = ["vi\\.mock.*supabase", "jest\\.mock.*database"]
ai_rules = [
    "Never mock Supabase.",
    "Use generated types.",
]

[testing.coverage]
unit = 80
integration = 60

[testing.levels]
unit = true
integration = true
e2e = false
smoke = true

[testing.type_enforcement]
generated_types_path = "libs/database/src/types.ts"
required_imports = ["from.*@workspace/database"]

[[testing.smoke]]
name = "api-health"
command = "curl -sf http://localhost:3001/health"
timeout = 10
"#;
    let manifest = load_from_str(toml).unwrap();
    assert_eq!(manifest.testing.mock_policy, schema::MockPolicy::UnitOnly);
    assert_eq!(manifest.testing.forbidden_patterns.len(), 2);
    assert_eq!(manifest.testing.ai_rules.len(), 2);
    assert_eq!(manifest.testing.coverage.unit, 80);
    assert_eq!(manifest.testing.coverage.integration, 60);
    assert!(manifest.testing.levels.integration);
    assert!(manifest.testing.levels.smoke);
    assert!(!manifest.testing.levels.e2e);

    let te = manifest.testing.type_enforcement.as_ref().unwrap();
    assert_eq!(te.generated_types_path, "libs/database/src/types.ts");
    assert_eq!(te.required_imports.len(), 1);

    assert_eq!(manifest.testing.smoke.len(), 1);
    assert_eq!(manifest.testing.smoke[0].name, "api-health");
    assert_eq!(manifest.testing.smoke[0].timeout, 10);
}

#[test]
fn test_testing_section_mock_policy_forbidden() {
    let toml = r#"
version = 1

[testing]
mock_policy = "forbidden"
"#;
    let manifest = load_from_str(toml).unwrap();
    assert_eq!(manifest.testing.mock_policy, schema::MockPolicy::Forbidden);
}

#[test]
fn test_testing_section_mock_policy_allowed() {
    let toml = r#"
version = 1

[testing]
mock_policy = "allowed"
"#;
    let manifest = load_from_str(toml).unwrap();
    assert_eq!(manifest.testing.mock_policy, schema::MockPolicy::Allowed);
}

#[test]
fn test_testing_smoke_default_timeout() {
    let toml = r#"
version = 1

[[testing.smoke]]
name = "check"
command = "curl localhost"
"#;
    let manifest = load_from_str(toml).unwrap();
    assert_eq!(manifest.testing.smoke[0].timeout, 30);
}

#[test]
fn test_testing_invalid_regex_rejected() {
    let toml = r#"
version = 1

[testing]
mock_policy = "allowed"
forbidden_patterns = ["[invalid(regex"]
"#;
    let result = load_from_str(toml);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("invalid regex"),
        "Expected regex validation error, got: {err_msg}"
    );
}

#[test]
fn test_testing_valid_regex_accepted() {
    let toml = r#"
version = 1

[testing]
mock_policy = "allowed"
forbidden_patterns = ["vi\\.mock.*supabase", "jest\\.mock.*database"]
"#;
    assert!(load_from_str(toml).is_ok());
}

#[test]
fn test_testing_invalid_required_imports_regex_rejected() {
    let toml = r#"
version = 1

[testing]
mock_policy = "allowed"

[testing.type_enforcement]
generated_types_path = "libs/db/types.ts"
required_imports = ["[broken("]
"#;
    let result = load_from_str(toml);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("invalid regex"),
        "Expected regex validation error, got: {err_msg}"
    );
}

// ── Policy section tests ──

#[test]
fn test_policy_section_defaults_when_absent() {
    let manifest = load_from_str(&minimal_manifest()).unwrap();
    assert_eq!(
        manifest.policy.testing.mock_policy,
        schema::MockPolicy::Forbidden
    );
    assert!(manifest.policy.testing.ai_rules.is_empty());
    assert!(manifest.policy.security.banned_env_vars.is_empty());
    assert!(manifest.policy.security.allowed_paths.is_empty());
    assert!(!manifest.policy.security.scan_secrets);
}

#[test]
fn test_policy_testing_full_config() {
    let toml = r#"
version = 1

[policy.testing]
mock_policy = "unit-only"
forbidden_patterns = ["vi\\.mock.*supabase"]
ai_rules = ["Use real DB."]

[policy.testing.coverage]
unit = 80
integration = 60

[policy.testing.type_enforcement]
generated_types_path = "libs/database/src/types.ts"
required_imports = ["from.*@workspace/database"]
"#;
    let manifest = load_from_str(toml).unwrap();
    assert_eq!(
        manifest.policy.testing.mock_policy,
        schema::MockPolicy::UnitOnly
    );
    assert_eq!(manifest.policy.testing.forbidden_patterns.len(), 1);
    assert_eq!(manifest.policy.testing.ai_rules.len(), 1);
    assert_eq!(manifest.policy.testing.coverage.unit, 80);
    assert_eq!(manifest.policy.testing.coverage.integration, 60);
    assert!(manifest.policy.testing.type_enforcement.is_some());
}

#[test]
fn test_policy_security_config() {
    let toml = r#"
version = 1

[policy.security]
banned_env_vars = ["SUPABASE_SERVICE_ROLE_KEY", "SUPABASE_SECRET_KEY"]
allowed_paths = ["supabase/functions/*", "products/*/worker/*"]
scan_secrets = true
max_file_size_mb = 100
"#;
    let manifest = load_from_str(toml).unwrap();
    assert_eq!(manifest.policy.security.banned_env_vars.len(), 2);
    assert_eq!(manifest.policy.security.allowed_paths.len(), 2);
    assert!(manifest.policy.security.scan_secrets);
    assert_eq!(manifest.policy.security.max_file_size_mb, 100);
}

#[test]
fn test_policy_security_invalid_glob_rejected() {
    let toml = r#"
version = 1

[policy.security]
banned_env_vars = ["SECRET"]
allowed_paths = ["[invalid"]
"#;
    let result = load_from_str(toml);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("invalid glob"),
        "Expected glob validation error, got: {err_msg}"
    );
}

#[test]
fn test_testing_fallback_to_policy_testing() {
    // When [testing] is used (deprecated), policy.testing should get the values
    let toml = r#"
version = 1

[testing]
mock_policy = "forbidden"
ai_rules = ["No mocks."]
"#;
    let manifest = load_from_str(toml).unwrap();
    // Fallback should copy testing → policy.testing
    assert_eq!(
        manifest.policy.testing.mock_policy,
        schema::MockPolicy::Forbidden
    );
    assert_eq!(manifest.policy.testing.ai_rules.len(), 1);
    assert_eq!(manifest.policy.testing.ai_rules[0], "No mocks.");
}

#[test]
fn test_policy_testing_takes_precedence_over_testing() {
    // When both [testing] and [policy.testing] exist, policy.testing wins
    let toml = r#"
version = 1

[testing]
mock_policy = "allowed"
ai_rules = ["Old rule."]

[policy.testing]
mock_policy = "forbidden"
ai_rules = ["New rule."]
"#;
    let manifest = load_from_str(toml).unwrap();
    assert_eq!(
        manifest.policy.testing.mock_policy,
        schema::MockPolicy::Forbidden
    );
    assert_eq!(manifest.policy.testing.ai_rules[0], "New rule.");
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
