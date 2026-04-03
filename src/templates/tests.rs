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

    // Now 3 root isolation volumes (node_modules, pnpm, pnpm-store) are always added
    assert_eq!(volume_names.len(), 3);
    assert!(
        volume_names
            .iter()
            .any(|v| v.as_str() == Some("ws_node_modules_root"))
    );
    assert!(
        volume_names
            .iter()
            .any(|v| v.as_str() == Some("ws_pnpm_root"))
    );
    assert!(
        volume_names
            .iter()
            .any(|v| v.as_str() == Some("ws_pnpm-store_root"))
    );
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
    assert!(result.contains("virtual-store-dir=/pnpm/virtual-store"));
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

    // Now 3 root isolation volumes (node_modules, pnpm, pnpm-store) are always added
    assert_eq!(volume_names.len(), 3);
}

#[test]
fn test_root_pnpm_volumes_use_absolute_container_paths() {
    let toml_str = r#"
[workspace]
name = "test-project"
image = "node:22-alpine"
workdir = "/app"
package_manager = "pnpm@10.22.0"

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = []

[service.web]
image = "node:22-alpine"
"#;
    let manifest: Manifest = toml::from_str(toml_str).unwrap();
    let engine = TemplateEngine::new().unwrap();
    let compose = engine
        .build_compose_file(&manifest, "/nonexistent")
        .unwrap();
    let service = compose.services.get("web").unwrap();

    assert!(
        service
            .volumes
            .iter()
            .any(|v| v == "ws_pnpm_root:/pnpm/virtual-store")
    );
    assert!(
        service
            .volumes
            .iter()
            .any(|v| v == "ws_pnpm-store_root:/pnpm/store")
    );
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
    // Now 3 root isolation volumes (node_modules, pnpm, pnpm-store) are always added
    let volume_names = context["volume_names"].as_array().unwrap();
    assert_eq!(volume_names.len(), 3);
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

    // Now 3 root isolation volumes + 2 service volumes = 5
    assert_eq!(volume_names.len(), 5);
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

    // Now 3 root isolation volumes + 1 service volume = 4
    assert_eq!(volume_names.len(), 4);
    assert!(
        volume_names
            .iter()
            .any(|v| v.as_str() == Some("just_a_name"))
    );
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
    // Now 3 root isolation volumes are always added
    assert_eq!(volume_names.len(), 3);
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

    // 4 packages × (2 clean + 3 recursive) = 20 artifact volumes + 3 root isolation volumes = 23
    assert_eq!(vol_strs.len(), 23);
    assert!(vol_strs.contains(&"ws_next_apps_corporate"));
    assert!(vol_strs.contains(&"ws_dist_apps_corporate"));
    assert!(vol_strs.contains(&"ws_node_modules_apps_corporate"));
    assert!(vol_strs.contains(&"ws_pnpm_apps_corporate"));
    assert!(vol_strs.contains(&"ws_pnpm-store_apps_corporate"));
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
    // 3 root isolation + 2 service volumes
    assert_eq!(volume_names.len(), 5);
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
            // Image-only service: also gets artifact volumes to prevent host contamination
            assert_eq!(volumes.len(), 7); // 3 root + 4 per-package (ws_next_apps_web, etc)
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
    // Now 3 root isolation volumes are always added
    assert_eq!(volume_names.len(), 3);
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
    // 3 root isolation + 1 service-specific
    assert_eq!(vol_strs.len(), 4);
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

    // Now 3 root isolation volumes are always added
    assert_eq!(volumes.len(), 3);
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
        result.contains("capabilities") && result.contains("gpu"),
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
