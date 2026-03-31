use super::*;
use crate::manifest::Manifest;

fn minimal_manifest() -> Manifest {
    let toml_str = r#"
[workspace]
name = "test-project"
image = "node:22-alpine"
workdir = "/app"
package_manager = "pnpm@10.22.0"
volumes = []

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]
"#;
    toml::from_str(toml_str).unwrap()
}

#[test]
fn test_compose_context_default_volumes() {
    let manifest = minimal_manifest();
    let engine = TemplateEngine::new().unwrap();
    let context = engine
        .prepare_docker_compose_data(&manifest, "/nonexistent")
        .unwrap();

    let volume_names = context["volume_names"].as_array().unwrap();

    // No bind mount, no workspace volumes — volume_names comes from service volumes only
    assert_eq!(volume_names.len(), 0);
}

#[test]
fn test_compose_context_no_workspace_service() {
    let manifest = minimal_manifest();
    let engine = TemplateEngine::new().unwrap();
    let context = engine
        .prepare_docker_compose_data(&manifest, "/nonexistent")
        .unwrap();

    // workspace_service and workspace_env should not exist
    assert!(context.get("workspace_service").is_none());
    assert!(context.get("workspace_env").is_none());
}

#[test]
fn test_compose_no_workspace_service_block() {
    let manifest = minimal_manifest();
    let engine = TemplateEngine::new().unwrap();
    let result = engine.render_docker_compose(&manifest).unwrap();

    // Should NOT contain workspace service definition
    assert!(!result.contains("command: sleep infinity"));
    assert!(!result.contains("healthcheck:"));
    // x-app-base removed — each service has its own build/image
    assert!(!result.contains("x-app-base"));
}

#[test]
fn test_render_npmrc() {
    let engine = TemplateEngine::new().unwrap();
    let result = engine.render_npmrc().unwrap();
    assert!(result.contains("store-dir=/pnpm/store"));
    assert!(result.contains("virtual-store-dir=.pnpm"));
    assert!(result.contains("DO NOT EDIT"));
}

#[test]
fn test_compose_context_custom_volumes() {
    // workspace.volumes are now ignored — no bind mount means no workspace volumes
    let toml_str = r#"
[workspace]
name = "test-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"
volumes = ["custom_vol:/app/custom", "data_vol:/app/data"]

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let context = engine
        .prepare_docker_compose_data(&manifest, "/nonexistent")
        .unwrap();

    let volume_names = context["volume_names"].as_array().unwrap();

    // No service volumes defined → empty
    assert_eq!(volume_names.len(), 0);
}

#[test]
fn test_compose_template_renders_no_bind_mount() {
    let manifest = minimal_manifest();
    let engine = TemplateEngine::new().unwrap();
    let result = engine.render_docker_compose(&manifest).unwrap();

    // Should NOT contain bind mount or workspace volumes
    assert!(!result.contains("./:/app:delegated"));
    assert!(!result.contains("node_modules:/app/node_modules"));
    assert!(!result.contains("CHOKIDAR_USEPOLLING"));
    assert!(!result.contains("WATCHPACK_POLLING"));

    // x-app-base removed — each service has its own build/image
    assert!(!result.contains("x-app-base"));
}

#[test]
fn test_compose_template_no_workspace_volumes() {
    // workspace.volumes are ignored — only service volumes matter
    let toml_str = r#"
[workspace]
name = "test-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"
volumes = ["my_cache:/app/.cache"]

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let result = engine.render_docker_compose(&manifest).unwrap();

    // workspace volumes are not rendered in service definitions
    assert!(!result.contains("- my_cache:/app/.cache"));
    assert!(!result.contains("- node_modules:/app/node_modules"));
    assert!(!result.contains("./:/app:delegated"));
}

#[test]
fn test_compose_context_different_workdir() {
    let toml_str = r#"
[workspace]
name = "test-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/workspace/app"
volumes = []

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let context = engine
        .prepare_docker_compose_data(&manifest, "/nonexistent")
        .unwrap();

    // workdir should be set correctly
    assert_eq!(context["workdir"], "/workspace/app");
    // No workspace volumes
    let volume_names = context["volume_names"].as_array().unwrap();
    assert_eq!(volume_names.len(), 0);
}

#[test]
fn test_compose_context_volume_with_mode() {
    // workspace.volumes are ignored — service volumes with :ro/:rw are tested via service definitions
    let toml_str = r#"
[workspace]
name = "test-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"
volumes = ["config_vol:/app/config:ro", "data_vol:/app/data:rw"]

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]

[service.myservice]
image = "node:22-alpine"
volumes = ["config_vol:/app/config:ro", "data_vol:/app/data:rw"]
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let context = engine
        .prepare_docker_compose_data(&manifest, "/nonexistent")
        .unwrap();

    let volume_names = context["volume_names"].as_array().unwrap();

    // Volume names extracted from service volumes
    assert_eq!(volume_names.len(), 2);
    assert_eq!(volume_names[0], "config_vol");
    assert_eq!(volume_names[1], "data_vol");
}

#[test]
fn test_compose_context_malformed_volume_no_colon() {
    // workspace.volumes are now ignored — test with service volumes instead
    let toml_str = r#"
[workspace]
name = "test-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]

[service.myservice]
image = "node:22-alpine"
volumes = ["just_a_name"]
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let context = engine
        .prepare_docker_compose_data(&manifest, "/nonexistent")
        .unwrap();

    let volume_names = context["volume_names"].as_array().unwrap();

    // Volume name extraction from service volumes
    assert_eq!(volume_names.len(), 1);
    assert_eq!(volume_names[0], "just_a_name");
}

#[test]
fn test_compose_context_empty_string_volume() {
    // workspace.volumes are ignored — no volumes in output
    let toml_str = r#"
[workspace]
name = "test-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"
volumes = ["", "valid_vol:/app/valid"]

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let context = engine
        .prepare_docker_compose_data(&manifest, "/nonexistent")
        .unwrap();

    let volume_names = context["volume_names"].as_array().unwrap();
    assert_eq!(volume_names.len(), 0);
}

#[test]
fn test_render_env_example_with_required() {
    let toml_str = r#"
[workspace]
name = "test-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]

[env]
required = ["DATABASE_URL", "API_KEY"]
optional = ["SENTRY_DSN"]

[env.validation.DATABASE_URL]
pattern = "^postgresql://"
description = "PostgreSQL connection string"
example = "postgresql://user:pass@localhost:5432/db"

[env.validation.API_KEY]
description = "API authentication key"
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let result = engine.render_env_example(&manifest).unwrap();

    // Should contain header
    assert!(result.contains("# Auto-generated by airis init"));

    // Should contain required vars section
    assert!(result.contains("# Required environment variables"));
    assert!(result.contains("DATABASE_URL=postgresql://user:pass@localhost:5432/db"));
    assert!(result.contains("API_KEY=your_value_here"));

    // Should contain description as comment
    assert!(result.contains("# PostgreSQL connection string"));

    // Should contain optional vars section
    assert!(result.contains("# Optional environment variables"));
    assert!(result.contains("# SENTRY_DSN="));
}

#[test]
fn test_render_env_example_empty() {
    let toml_str = r#"
[workspace]
name = "test-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let result = engine.render_env_example(&manifest).unwrap();

    // Should only contain header when no env vars defined
    assert!(result.contains("# Auto-generated by airis init"));
    assert!(!result.contains("# Required environment variables"));
    assert!(!result.contains("# Optional environment variables"));
}

#[test]
fn test_render_envrc() {
    let toml_str = r#"
[workspace]
name = "my-awesome-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let result = engine.render_envrc(&manifest).unwrap();

    // Should contain header comment
    assert!(result.contains("# Auto-generated by airis init"));
    assert!(result.contains("# Enable with: direnv allow"));

    // Should add .airis/bin to PATH
    assert!(result.contains("export PATH=\"$PWD/.airis/bin:$PATH\""));

    // Should set COMPOSE_PROFILES
    assert!(result.contains("export COMPOSE_PROFILES=\"${COMPOSE_PROFILES:-shell,web}\""));

    // Should set COMPOSE_PROJECT_NAME from workspace name
    assert!(result.contains("export COMPOSE_PROJECT_NAME=\"my-awesome-project\""));
}

#[test]
fn test_artifact_volumes_generated_from_apps_and_libs() {
    let toml_str = r#"
[workspace]
name = "test-project"
image = "node:22-alpine"
workdir = "/app"

[workspace.clean]
dirs = [".next", "dist"]

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]

[apps.corporate]

[apps.dashboard]
path = "apps/dashboard"

[libs.ui]

[libs.logger]
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let context = engine
        .prepare_docker_compose_data(&manifest, "/nonexistent")
        .unwrap();

    let volume_names = context["volume_names"].as_array().unwrap();
    let vol_strs: Vec<&str> = volume_names.iter().map(|v| v.as_str().unwrap()).collect();

    // 4 packages × 2 clean dirs = 8 artifact volumes
    assert_eq!(vol_strs.len(), 8);
    assert!(vol_strs.contains(&"ws_next_apps_corporate"));
    assert!(vol_strs.contains(&"ws_dist_apps_corporate"));
    assert!(vol_strs.contains(&"ws_next_apps_dashboard"));
    assert!(vol_strs.contains(&"ws_dist_apps_dashboard"));
    assert!(vol_strs.contains(&"ws_next_libs_ui"));
    assert!(vol_strs.contains(&"ws_dist_libs_ui"));
    assert!(vol_strs.contains(&"ws_next_libs_logger"));
    assert!(vol_strs.contains(&"ws_dist_libs_logger"));
}

#[test]
fn test_service_volumes_extracted() {
    // Service-specific volumes should be extracted to volume_names
    let toml_str = r#"
[workspace]
name = "test-project"
image = "node:22-alpine"
workdir = "/app"

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = []

[service.myapp]
image = "node:22-alpine"
volumes = ["data_vol:/app/data", "cache_vol:/app/cache"]
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let context = engine
        .prepare_docker_compose_data(&manifest, "/nonexistent")
        .unwrap();

    let volume_names = context["volume_names"].as_array().unwrap();
    assert_eq!(volume_names.len(), 2);
    assert_eq!(volume_names[0], "data_vol");
    assert_eq!(volume_names[1], "cache_vol");
}

#[test]
fn test_artifact_volumes_only_on_build_services() {
    // External services (image-only, no build) should NOT get artifact volumes
    let toml_str = r#"
[workspace]
name = "test-project"
image = "node:22-alpine"
workdir = "/app"

[workspace.clean]
dirs = [".next"]

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*"]

[apps.web]

[service.web]
build = { context = "." }

[service.redis]
image = "redis:7-alpine"
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let context = engine
        .prepare_docker_compose_data(&manifest, "/nonexistent")
        .unwrap();

    let services = context["services"].as_array().unwrap();
    for svc in services {
        let name = svc["name"].as_str().unwrap();
        let volumes = svc["volumes"].as_array().unwrap();
        if name == "web" {
            // Build service: should have artifact volumes
            let vol_strs: Vec<&str> = volumes.iter().map(|v| v.as_str().unwrap()).collect();
            assert!(vol_strs.iter().any(|v| v.contains("ws_next_apps_web")));
        } else if name == "redis" {
            // Image-only service: no artifact volumes
            assert!(volumes.is_empty());
        }
    }
}

#[test]
fn test_no_artifact_volumes_without_apps() {
    // No apps/libs defined → no artifact volumes
    let toml_str = r#"
[workspace]
name = "test-project"
image = "node:22-alpine"
workdir = "/app"

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = []
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let context = engine
        .prepare_docker_compose_data(&manifest, "/nonexistent")
        .unwrap();

    let volume_names = context["volume_names"].as_array().unwrap();
    assert_eq!(volume_names.len(), 0);
}

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
fn test_service_volumes_only_service_specific() {
    // Service-specific volumes only — no workspace volumes merged
    let toml_str = r#"
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
workspaces = []

[service.sales-agent]
image = "node:22-alpine"
command = "pnpm dev"
volumes = ["sales_data:/app/data"]
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let context = engine
        .prepare_docker_compose_data(&manifest, "/nonexistent")
        .unwrap();

    let services = context["services"].as_array().unwrap();
    let svc = &services[0];
    let volumes = svc["volumes"].as_array().unwrap();
    let vol_strs: Vec<String> = volumes
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();

    // Should NOT contain bind mount or workspace volumes
    assert!(!vol_strs.contains(&"./:/app:delegated".to_string()));
    assert!(!vol_strs.contains(&"node_modules:/app/node_modules".to_string()));
    // Should contain only service-specific volume
    assert_eq!(vol_strs.len(), 1);
    assert!(vol_strs.contains(&"sales_data:/app/data".to_string()));
}

#[test]
fn test_service_without_volumes_empty() {
    // When a service has no own volumes, volumes array should be empty
    let toml_str = r#"
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
workspaces = []

[service.frontend]
image = "node:22-alpine"
command = "pnpm dev"
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let context = engine
        .prepare_docker_compose_data(&manifest, "/nonexistent")
        .unwrap();

    let services = context["services"].as_array().unwrap();
    let svc = &services[0];
    let volumes = svc["volumes"].as_array().unwrap();

    // No own volumes → should be empty
    assert_eq!(volumes.len(), 0);
}

#[test]
fn test_compose_infra_service() {
    let toml_str = r#"
version = 1
mode = "docker-first"
[workspace]
name = "infra-test"
workdir = "/app"

[service.tunnel]
image = "cloudflare/cloudflared:latest"
network_mode = "host"

[service.app]
image = "myapp:latest"
networks = ["default", "proxy"]
labels = [
  "traefik.enable=true",
  "traefik.http.routers.app.rule=Host(`app.example.com`)",
]

[orchestration.networks.define.proxy]
external = true
name = "proxy"
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let result = engine.render_docker_compose(&manifest).unwrap();

    // network_mode
    assert!(
        result.contains("network_mode: host"),
        "missing network_mode"
    );
    // labels
    assert!(result.contains("traefik.enable=true"), "missing labels");
    assert!(
        result.contains("traefik.http.routers.app.rule=Host(`app.example.com`)"),
        "missing router label"
    );
    // service networks
    assert!(
        result.contains("- default"),
        "missing service network default"
    );
    assert!(result.contains("- proxy"), "missing service network proxy");
    // top-level networks section (data-driven)
    assert!(
        result.contains("external: true"),
        "missing external in network_defs"
    );
    assert!(
        result.contains("name: proxy"),
        "missing name in network_defs"
    );
    // should NOT contain hardcoded traefik network
    assert!(
        !result.contains("traefik_default"),
        "should not have hardcoded traefik network"
    );
}

#[test]
fn test_compose_gpu_service() {
    let toml_str = r#"
version = 1
mode = "docker-first"
[workspace]
name = "gpu-test"
workdir = "/app"

[service.ml]
image = "nvidia/cuda:12.6"
runtime = "nvidia"
devices = ["/dev/dri:/dev/dri"]

[service.ml.gpu]
driver = "nvidia"
count = "all"
capabilities = ["gpu"]
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let result = engine.render_docker_compose(&manifest).unwrap();

    assert!(result.contains("runtime: nvidia"), "missing runtime");
    assert!(result.contains("- /dev/dri:/dev/dri"), "missing devices");
    assert!(result.contains("driver: nvidia"), "missing gpu driver");
    assert!(result.contains("count: all"), "missing gpu count");
    assert!(
        result.contains("capabilities: [gpu]"),
        "missing gpu capabilities"
    );
    // ml service should have deploy.resources, not deploy.replicas
    let ml_section = result.split("  ml:").nth(1).unwrap();
    assert!(
        ml_section.contains("resources:"),
        "ml service should have deploy.resources"
    );
    assert!(
        !ml_section.contains("replicas:"),
        "ml service should not have replicas when gpu is set"
    );
}

#[test]
fn test_compose_gpu_defaults() {
    let toml_str = r#"
version = 1
mode = "docker-first"
[workspace]
name = "gpu-test"
workdir = "/app"

[service.ml]
image = "nvidia/cuda:12.6"
gpu = {}
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let svc = &manifest.service["ml"];
    let gpu = svc.gpu.as_ref().unwrap();

    assert_eq!(gpu.driver, "nvidia");
    assert_eq!(gpu.count, "all");
    assert_eq!(gpu.capabilities, vec!["gpu".to_string()]);
}

#[test]
fn test_ci_workflow_custom_jobs() {
    let toml_str = r#"
[project]
id = "test-project"

[workspace]
package_manager = "pnpm"
members = ["apps/*"]

[ci]
enabled = true
runner = "self-hosted, linux"

[ci.jobs]
lint = 10
typecheck = 10
test = 20
e2e = 30

[profile.stg]
branch = "stg"
domain = "stg.example.com"

[profile.prd]
branch = "main"
domain = "example.com"
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let result = engine.render_ci_workflow(&manifest).unwrap();

    assert!(
        result.contains("runs-on: [self-hosted, linux]"),
        "runner should be self-hosted array"
    );
    assert!(result.contains("  lint:"), "should have lint job");
    assert!(result.contains("  typecheck:"), "should have typecheck job");
    assert!(result.contains("  test:"), "should have test job");
    assert!(result.contains("  e2e:"), "should have e2e job");
    assert!(
        result.contains("timeout-minutes: 30"),
        "e2e should have 30min timeout"
    );
    assert!(
        result.contains("timeout-minutes: 20"),
        "test should have 20min timeout"
    );
    assert!(
        result.contains("pnpm turbo run e2e"),
        "e2e job should run turbo e2e"
    );
}

#[test]
fn test_ci_workflow_default_jobs() {
    let toml_str = r#"
[project]
id = "test-project"

[workspace]
package_manager = "pnpm"
members = ["apps/*"]

[ci]
enabled = true

[profile.stg]
branch = "stg"
domain = "stg.example.com"

[profile.prd]
branch = "main"
domain = "example.com"
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let result = engine.render_ci_workflow(&manifest).unwrap();

    assert!(result.contains("  lint:"), "should have lint job");
    assert!(result.contains("  typecheck:"), "should have typecheck job");
    assert!(result.contains("  test:"), "should have test job");
    assert!(
        !result.contains("  e2e:"),
        "should NOT have e2e job by default"
    );
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
    let mut custom = ProfileSection::default();
    custom.role = Some("production".to_string());
    assert_eq!(custom.effective_role("stg"), "production");
}

#[test]
fn test_profile_role_in_ci_workflow() {
    let toml_str = r#"
[project]
id = "test-project"

[workspace]
package_manager = "pnpm"
members = ["apps/*"]

[ci]
enabled = true

[profile.staging]
branch = "develop"
domain = "stg.example.com"
role = "staging"

[profile.live]
branch = "release"
domain = "example.com"
role = "production"
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let result = engine.render_ci_workflow(&manifest).unwrap();

    assert!(
        result.contains("branches: [develop]"),
        "CI should use staging branch 'develop'"
    );
    assert!(
        result.contains("branches: [release]"),
        "PR target should use production branch 'release'"
    );
}

#[test]
fn test_notify_job_uses_ci_runner() {
    let toml_str = r#"
[project]
id = "test-project"

[workspace]
package_manager = "pnpm"
members = ["apps/*"]

[ci]
enabled = true
runner = "self-hosted, linux"

[[app]]
name = "my-app"
path = "apps/my-app"
framework = "nextjs"

[app.deploy]
enabled = true
port = 3000
health_path = "/health"
host = "{profile.domain}"

[profile.stg]
branch = "stg"
domain = "stg.example.com"
env_source = { doppler = { config = "stg", secret = "DOPPLER_TOKEN_STG" } }

[profile.prd]
branch = "main"
domain = "example.com"
env_source = { doppler = { config = "prd", secret = "DOPPLER_TOKEN_PRD" } }
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let result = engine.render_deploy_workflow(&manifest).unwrap();

    // Notify job should use the same runner as other jobs
    let notify_section = result.find("  notify:").expect("should have notify job");
    let after_notify = &result[notify_section..];
    assert!(
        after_notify.contains("runs-on: [self-hosted, linux]"),
        "notify should use ci.runner, not ubuntu-latest"
    );
    assert!(
        !after_notify.contains("runs-on: ubuntu-latest"),
        "notify should NOT use ubuntu-latest"
    );
}

#[test]
fn test_docker_deploy_custom_timeout_and_retries() {
    let toml_str = r#"
[project]
id = "test-project"

[workspace]
package_manager = "pnpm"
members = ["apps/*"]

[ci]
enabled = true

[[app]]
name = "my-api"
path = "apps/my-api"
framework = "node"

[app.deploy]
enabled = true
port = 3000
health_path = "/healthz"
host = "{profile.domain}"
timeout = 20
health_retries = 10
health_retry_interval = 15

[profile.stg]
branch = "stg"
domain = "stg.example.com"
env_source = { doppler = { config = "stg", secret = "DOPPLER_TOKEN_STG" } }

[profile.prd]
branch = "main"
domain = "example.com"
env_source = { doppler = { config = "prd", secret = "DOPPLER_TOKEN_PRD" } }
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let result = engine.render_deploy_workflow(&manifest).unwrap();

    let deploy_section = result
        .find("deploy-my-api:")
        .expect("should have deploy job");
    let after_deploy = &result[deploy_section..];
    assert!(
        after_deploy.contains("timeout-minutes: 20"),
        "should use custom timeout"
    );
    assert!(
        after_deploy.contains("for i in 1 2 3 4 5 6 7 8 9 10;"),
        "should have 10 retries"
    );
    assert!(
        after_deploy.contains("sleep 15"),
        "should use custom retry interval"
    );
    assert!(
        after_deploy.contains("after 10 attempts"),
        "error message should reflect retry count"
    );
}

#[test]
fn test_worker_deploy_custom_domain() {
    let toml_str = r#"
[project]
id = "test-project"

[workspace]
package_manager = "pnpm"
members = ["apps/*"]

[ci]
enabled = true

[[app]]
name = "my-worker"
path = "apps/my-worker"
framework = "node"

[app.deploy]
enabled = true
deploy_target = "worker"
health_path = "/health"
workers_domain = "myorg.workers.dev"

[profile.stg]
branch = "stg"
domain = "stg.example.com"
env_source = { doppler = { config = "stg", secret = "DOPPLER_TOKEN_STG" } }

[profile.prd]
branch = "main"
domain = "example.com"
env_source = { doppler = { config = "prd", secret = "DOPPLER_TOKEN_PRD" } }
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let result = engine.render_deploy_workflow(&manifest).unwrap();

    assert!(
        result.contains("my-worker-production.myorg.workers.dev/health"),
        "production URL should use workers_domain and health_path"
    );
    assert!(
        result.contains("my-worker.myorg.workers.dev/health"),
        "staging URL should use workers_domain and health_path"
    );
    assert!(
        !result.contains("agiletec"),
        "should NOT contain hardcoded agiletec domain"
    );
}

#[test]
fn test_worker_deploy_missing_domain_errors() {
    let toml_str = r#"
[project]
id = "test-project"

[workspace]
package_manager = "pnpm"
members = ["apps/*"]

[ci]
enabled = true

[[app]]
name = "my-worker"
path = "apps/my-worker"
framework = "node"

[app.deploy]
enabled = true
deploy_target = "worker"
health_path = "/health"

[profile.stg]
branch = "stg"
domain = "stg.example.com"
env_source = { doppler = { config = "stg", secret = "DOPPLER_TOKEN_STG" } }

[profile.prd]
branch = "main"
domain = "example.com"
env_source = { doppler = { config = "prd", secret = "DOPPLER_TOKEN_PRD" } }
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let result = engine.render_deploy_workflow(&manifest);
    assert!(
        result.is_err(),
        "should error when workers_domain is missing for worker deploy"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("workers_domain"),
        "error should mention workers_domain"
    );
}

#[test]
fn test_infra_deploy_custom_network() {
    let toml_str = r#"
[project]
id = "test-project"

[ci]
enabled = true

[profile.prd]
branch = "main"
domain = "example.com"
env_source = { doppler = { config = "prd", secret = "DOPPLER_TOKEN" } }

[orchestration.networks]
proxy = "traefik-public"
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let result = engine.render_deploy_workflow(&manifest).unwrap();

    assert!(
        result.contains("docker network create traefik-public"),
        "should use custom network name from orchestration.networks.proxy"
    );
    assert!(
        !result.contains("docker network create proxy"),
        "should NOT use hardcoded 'proxy' network"
    );
}
