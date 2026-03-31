use super::*;
use indexmap::IndexMap;
use tempfile::tempdir;

use crate::manifest::Manifest;
use crate::test_lock::DIR_LOCK;

#[test]
fn test_run_missing_manifest() {
    let _guard = DIR_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();

    let result = std::panic::catch_unwind(|| {
        let result = run("test", &[]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("manifest.toml not found")
        );
    });

    std::env::set_current_dir(original_dir).unwrap();
    result.unwrap();
}

#[test]
fn test_run_missing_command() {
    let manifest_content = r#"
version = 1

[workspace]
name = "test"

[project]
rust_edition = "2024"
binary_name = "test"

[commands]
test = "echo 'test'"
"#;
    let manifest: Manifest = toml::from_str(manifest_content).unwrap();
    let commands = default_commands(&manifest).unwrap();
    assert!(!commands.contains_key("nonexistent"));
}

#[test]
fn test_get_package_manager_pnpm() {
    let manifest_content = r#"
version = 1
[workspace]
name = "test"
package_manager = "pnpm@10.22.0"
"#;
    let manifest: Manifest = toml::from_str(manifest_content).unwrap();
    assert_eq!(get_package_manager(&manifest), "pnpm");
}

#[test]
fn test_get_package_manager_bun() {
    let manifest_content = r#"
version = 1
[workspace]
name = "test"
package_manager = "bun@1.0.0"
"#;
    let manifest: Manifest = toml::from_str(manifest_content).unwrap();
    assert_eq!(get_package_manager(&manifest), "bun");
}

#[test]
fn test_get_package_manager_npm() {
    let manifest_content = r#"
version = 1
[workspace]
name = "test"
package_manager = "npm@10.0.0"
"#;
    let manifest: Manifest = toml::from_str(manifest_content).unwrap();
    assert_eq!(get_package_manager(&manifest), "npm");
}

#[test]
fn test_get_package_manager_yarn() {
    let manifest_content = r#"
version = 1
[workspace]
name = "test"
package_manager = "yarn@4.0.0"
"#;
    let manifest: Manifest = toml::from_str(manifest_content).unwrap();
    assert_eq!(get_package_manager(&manifest), "yarn");
}

#[test]
fn test_get_package_manager_default() {
    let manifest_content = r#"
version = 1
[workspace]
name = "test"
"#;
    let manifest: Manifest = toml::from_str(manifest_content).unwrap();
    assert_eq!(get_package_manager(&manifest), "pnpm");
}

#[test]
fn test_default_commands_uses_package_manager() {
    let _guard = DIR_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();

    std::fs::write("compose.yml", "version: '3'").unwrap();

    let manifest_content = r#"
version = 1
[workspace]
name = "test"
package_manager = "bun@1.0.0"
"#;
    let result = std::panic::catch_unwind(|| {
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();
        let cmds = default_commands(&manifest).unwrap();
        assert!(cmds.contains_key("up"));
        assert!(cmds.contains_key("down"));
        assert!(cmds.contains_key("ps"));
        assert!(!cmds.contains_key("install"));
        assert!(!cmds.contains_key("dev"));
        assert!(!cmds.contains_key("shell"));
        assert!(!cmds.contains_key("build"));
        assert!(!cmds.contains_key("test"));
        assert!(!cmds.contains_key("lint"));
        assert!(!cmds.contains_key("clean"));
        assert!(!cmds.contains_key("logs"));
        assert!(cmds.get("up").unwrap().contains("up -d --build"));
    });

    std::env::set_current_dir(original_dir).unwrap();
    result.unwrap();
}

#[test]
fn test_manifest_commands_override_defaults() {
    let _guard = DIR_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();
    std::fs::write("compose.yml", "version: '3'").unwrap();

    let manifest_content = r#"
version = 1
[workspace]
name = "test"
package_manager = "pnpm@10.0.0"
[commands]
test = "custom test command"
"#;
    let result = std::panic::catch_unwind(|| {
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();
        let mut commands = default_commands(&manifest).unwrap();
        for (key, value) in manifest.commands.iter() {
            commands.insert(key.clone(), value.clone());
        }
        assert_eq!(commands.get("test").unwrap(), "custom test command");
        assert!(commands.get("up").unwrap().contains("docker compose"));
        assert!(commands.get("up").unwrap().contains("--build"));
        assert!(!commands.contains_key("dev"));
        assert!(!commands.contains_key("install"));
    });

    std::env::set_current_dir(original_dir).unwrap();
    result.unwrap();
}

#[test]
fn test_manifest_can_add_custom_commands() {
    let _guard = DIR_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();
    std::fs::write("compose.yml", "version: '3'").unwrap();

    let manifest_content = r#"
version = 1
[workspace]
name = "test"
[commands]
my-custom = "echo custom"
"#;
    let result = std::panic::catch_unwind(|| {
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();
        let mut commands = default_commands(&manifest).unwrap();
        for (key, value) in manifest.commands.iter() {
            commands.insert(key.clone(), value.clone());
        }
        assert_eq!(commands.get("my-custom").unwrap(), "echo custom");
        assert!(commands.contains_key("up"));
        assert!(commands.contains_key("down"));
    });

    std::env::set_current_dir(original_dir).unwrap();
    result.unwrap();
}

#[test]
fn test_validate_clean_path_safe_paths() {
    use build_ops::validate_clean_path;
    assert!(validate_clean_path(".next").is_some());
    assert!(validate_clean_path("dist").is_some());
    assert!(validate_clean_path("build").is_some());
    assert!(validate_clean_path("apps/dashboard/.next").is_some());
    assert!(validate_clean_path("node_modules").is_some());
}

#[test]
fn test_validate_clean_path_rejects_traversal() {
    use build_ops::validate_clean_path;
    assert!(validate_clean_path("..").is_none());
    assert!(validate_clean_path("../").is_none());
    assert!(validate_clean_path("../other-project").is_none());
    assert!(validate_clean_path("foo/../bar").is_none());
    assert!(validate_clean_path("../../important").is_none());
}

#[test]
fn test_validate_clean_path_rejects_absolute() {
    use build_ops::validate_clean_path;
    assert!(validate_clean_path("/").is_none());
    assert!(validate_clean_path("/tmp").is_none());
    assert!(validate_clean_path("/etc/passwd").is_none());
    assert!(validate_clean_path("~").is_none());
    assert!(validate_clean_path("~/Documents").is_none());
}

#[test]
fn test_validate_clean_path_rejects_shell_chars() {
    use build_ops::validate_clean_path;
    assert!(validate_clean_path("foo; rm -rf /").is_none());
    assert!(validate_clean_path("foo && bar").is_none());
    assert!(validate_clean_path("$(whoami)").is_none());
    assert!(validate_clean_path("`id`").is_none());
    assert!(validate_clean_path("foo|bar").is_none());
    assert!(validate_clean_path("foo > bar").is_none());
}

#[test]
fn test_validate_clean_path_rejects_dangerous() {
    use build_ops::validate_clean_path;
    assert!(validate_clean_path(".").is_none());
    assert!(validate_clean_path("./").is_none());
    assert!(validate_clean_path("").is_none());
}

#[test]
fn test_validate_clean_pattern_safe_patterns() {
    use build_ops::validate_clean_pattern;
    assert!(validate_clean_pattern("node_modules").is_some());
    assert!(validate_clean_pattern(".next").is_some());
    assert!(validate_clean_pattern("*.log").is_some());
    assert!(validate_clean_pattern("dist").is_some());
}

#[test]
fn test_validate_clean_pattern_rejects_paths() {
    use build_ops::validate_clean_pattern;
    assert!(validate_clean_pattern("foo/bar").is_none());
    assert!(validate_clean_pattern("../node_modules").is_none());
}

#[test]
fn test_validate_clean_pattern_rejects_shell_injection() {
    use build_ops::validate_clean_pattern;
    assert!(validate_clean_pattern("'; rm -rf /; '").is_none());
    assert!(validate_clean_pattern("$(whoami)").is_none());
    assert!(validate_clean_pattern("`id`").is_none());
}

#[test]
fn test_build_clean_command_filters_unsafe() {
    use build_ops::build_clean_command;
    let manifest_content = r#"
version = 1
[workspace]
name = "test"
[workspace.clean]
dirs = [".next", "../dangerous", "/etc", "dist"]
recursive = ["node_modules", "'; rm -rf /;"]
"#;
    let manifest: Manifest = toml::from_str(manifest_content).unwrap();
    let cmd = build_clean_command(&manifest);
    assert!(cmd.contains(".next"));
    assert!(cmd.contains("dist"));
    assert!(cmd.contains("node_modules"));
    assert!(cmd.contains("Skipped unsafe clean path: ../dangerous"));
    assert!(cmd.contains("Skipped unsafe clean path: /etc"));
    assert!(cmd.contains("Skipped unsafe recursive pattern"));
}

#[test]
fn test_build_compose_command_no_compose_file_errors() {
    let _guard = DIR_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();

    let manifest_content = r#"
version = 1
[workspace]
name = "test"
"#;
    let result = std::panic::catch_unwind(|| {
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();
        let result = build_compose_command(&manifest, "up -d");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("No compose file found"));
        assert!(err_msg.contains("airis manifest json"));
        assert!(err_msg.contains("airis gen"));
    });

    std::env::set_current_dir(original_dir).unwrap();
    result.unwrap();
}

#[test]
fn test_build_compose_command_with_compose_file_succeeds() {
    let _guard = DIR_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();
    std::fs::write("compose.yml", "version: '3'").unwrap();

    let manifest_content = r#"
version = 1
[workspace]
name = "test"
"#;
    let result = std::panic::catch_unwind(|| {
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();
        let result = build_compose_command(&manifest, "up -d");
        assert!(result.is_ok());
        let cmd = result.unwrap();
        assert!(cmd.contains("-f compose.yml"));
        assert!(cmd.contains("up -d"));
    });

    std::env::set_current_dir(original_dir).unwrap();
    result.unwrap();
}

#[test]
fn test_build_compose_command_with_orchestration_succeeds() {
    let _guard = DIR_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();

    let manifest_content = r#"
version = 1
[workspace]
name = "test"
[orchestration.dev]
workspace = "compose.yml"
traefik = "traefik/compose.yml"
"#;
    let result = std::panic::catch_unwind(|| {
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();
        let result = build_compose_command(&manifest, "up -d");
        assert!(result.is_ok());
        let cmd = result.unwrap();
        assert!(cmd.contains("-f compose.yml"));
        assert!(cmd.contains("-f traefik/compose.yml"));
    });

    std::env::set_current_dir(original_dir).unwrap();
    result.unwrap();
}

#[test]
fn test_condense_status_minutes() {
    use services::condense_status;
    assert_eq!(condense_status("Up 3 minutes"), "Up 3m");
    assert_eq!(condense_status("Up 38 minutes"), "Up 38m");
    assert_eq!(condense_status("Up 1 minute"), "Up 1m");
}

#[test]
fn test_condense_status_hours() {
    use services::condense_status;
    assert_eq!(condense_status("Up 2 hours"), "Up 2h");
    assert_eq!(condense_status("Up About an hour"), "Up ~1h");
}

#[test]
fn test_condense_status_other() {
    use services::condense_status;
    assert_eq!(condense_status("Up 5 seconds"), "Up 5s");
    assert_eq!(condense_status("Up 3 days"), "Up 3d");
    assert_eq!(condense_status("Up About a minute"), "Up ~1m");
}

#[test]
fn test_condense_status_passthrough() {
    use services::condense_status;
    assert_eq!(condense_status("Exited (0)"), "Exited (0)");
    assert_eq!(condense_status("Created"), "Created");
}

#[test]
fn test_parse_service_ports_object_format() {
    use services::parse_service_ports_from_config;
    let config: serde_json::Value = serde_json::json!({
        "services": {
            "web": {
                "container_name": "my-web",
                "ports": [{"mode": "ingress", "target": 3000, "published": "3000", "protocol": "tcp"}]
            },
            "api": {
                "ports": [{"mode": "ingress", "target": 8080, "published": 8080, "protocol": "tcp"}]
            }
        }
    });
    let result = parse_service_ports_from_config(&config);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], ("api".to_string(), 8080));
    assert_eq!(result[1], ("my-web".to_string(), 3000));
}

#[test]
fn test_parse_service_ports_string_format() {
    use services::parse_service_ports_from_config;
    let config: serde_json::Value = serde_json::json!({
        "services": {
            "web": {"ports": ["3000:3000"]},
            "api": {"ports": ["8080:80"]}
        }
    });
    let result = parse_service_ports_from_config(&config);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], ("api".to_string(), 8080));
    assert_eq!(result[1], ("web".to_string(), 3000));
}

#[test]
fn test_parse_service_ports_no_ports() {
    use services::parse_service_ports_from_config;
    let config: serde_json::Value =
        serde_json::json!({"services": {"db": {"image": "postgres:15"}}});
    let result = parse_service_ports_from_config(&config);
    assert!(result.is_empty());
}

#[test]
fn test_parse_service_ports_empty_services() {
    use services::parse_service_ports_from_config;
    let config: serde_json::Value = serde_json::json!({"services": {}});
    let result = parse_service_ports_from_config(&config);
    assert!(result.is_empty());
}

#[test]
fn test_parse_service_ports_skips_zero_port() {
    use services::parse_service_ports_from_config;
    let config: serde_json::Value = serde_json::json!({
        "services": {"web": {"ports": [{"target": 3000, "published": "0"}]}}
    });
    let result = parse_service_ports_from_config(&config);
    assert!(result.is_empty());
}

#[test]
fn test_manifest_dev_urls_parsing() {
    let manifest_content = r#"
version = 1
[workspace]
name = "test"
[[dev.urls.infra]]
name = "Supabase Studio"
url = "http://localhost:54323"
[[dev.urls.apps]]
name = "Dashboard"
url = "http://localhost:3000"
[[dev.urls.apps]]
name = "API"
url = "http://localhost:8080"
"#;
    let manifest: Manifest = toml::from_str(manifest_content).unwrap();
    let urls = manifest.dev.urls.unwrap();
    assert_eq!(urls.infra.len(), 1);
    assert_eq!(urls.infra[0].name, "Supabase Studio");
    assert_eq!(urls.infra[0].url, "http://localhost:54323");
    assert_eq!(urls.apps.len(), 2);
    assert_eq!(urls.apps[0].name, "Dashboard");
    assert_eq!(urls.apps[1].name, "API");
}

#[test]
fn test_ensure_env_file_copies_example() {
    use compose::ensure_env_file;
    let _guard = DIR_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();

    let result = std::panic::catch_unwind(|| {
        std::fs::write(".env.example", "DATABASE_URL=postgres://localhost").unwrap();
        assert!(!std::path::Path::new(".env").exists());
        ensure_env_file();
        assert!(std::path::Path::new(".env").exists());
        let content = std::fs::read_to_string(".env").unwrap();
        assert_eq!(content, "DATABASE_URL=postgres://localhost");
    });

    std::env::set_current_dir(original_dir).unwrap();
    result.unwrap();
}

#[test]
fn test_ensure_env_file_noop_when_env_exists() {
    use compose::ensure_env_file;
    let _guard = DIR_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();

    let result = std::panic::catch_unwind(|| {
        std::fs::write(".env.example", "NEW_VALUE=true").unwrap();
        std::fs::write(".env", "EXISTING=keep").unwrap();
        ensure_env_file();
        let content = std::fs::read_to_string(".env").unwrap();
        assert_eq!(content, "EXISTING=keep");
    });

    std::env::set_current_dir(original_dir).unwrap();
    result.unwrap();
}

#[test]
fn test_ensure_env_file_noop_when_no_example() {
    use compose::ensure_env_file;
    let _guard = DIR_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();

    let result = std::panic::catch_unwind(|| {
        ensure_env_file();
        assert!(!std::path::Path::new(".env").exists());
    });

    std::env::set_current_dir(original_dir).unwrap();
    result.unwrap();
}

#[test]
fn test_dev_section_post_up_default_empty() {
    let manifest: Manifest = toml::from_str(
        r#"
version = 1
[workspace]
name = "test"
"#,
    )
    .unwrap();
    assert!(manifest.dev.post_up.is_empty());
}

#[test]
fn test_dev_section_post_up_with_hooks() {
    let manifest: Manifest = toml::from_str(
        r#"
version = 1
[workspace]
name = "test"
[dev]
post_up = [
    "docker compose exec workspace pnpm db:migrate",
    "docker compose exec workspace pnpm db:seed",
]
"#,
    )
    .unwrap();
    assert_eq!(manifest.dev.post_up.len(), 2);
    assert_eq!(
        manifest.dev.post_up[0],
        "docker compose exec workspace pnpm db:migrate"
    );
    assert_eq!(
        manifest.dev.post_up[1],
        "docker compose exec workspace pnpm db:seed"
    );
}

#[test]
fn test_extra_args_blocks_shell_injection_semicolon() {
    let _guard = DIR_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();
    std::fs::write(
        "manifest.toml",
        r#"
version = 1
[workspace]
name = "test"
package_manager = "pnpm"
service = "workspace"
image = "node:22"
[commands]
test = "echo safe"
"#,
    )
    .unwrap();

    let result = std::panic::catch_unwind(|| {
        let result = run("test", &["; rm -rf /".to_string()]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Shell metacharacters")
        );
    });

    std::env::set_current_dir(original_dir).unwrap();
    result.unwrap();
}

#[test]
fn test_extra_args_blocks_shell_injection_pipe() {
    let _guard = DIR_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();
    std::fs::write(
        "manifest.toml",
        r#"
version = 1
[workspace]
name = "test"
package_manager = "pnpm"
service = "workspace"
image = "node:22"
[commands]
test = "echo safe"
"#,
    )
    .unwrap();

    let result = std::panic::catch_unwind(|| {
        let result = run("test", &["| cat /etc/passwd".to_string()]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Shell metacharacters")
        );
    });

    std::env::set_current_dir(original_dir).unwrap();
    result.unwrap();
}

#[test]
fn test_extra_args_blocks_command_substitution() {
    let _guard = DIR_LOCK.lock().unwrap();
    let dir = tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();
    std::fs::write(
        "manifest.toml",
        r#"
version = 1
[workspace]
name = "test"
package_manager = "pnpm"
service = "workspace"
image = "node:22"
[commands]
test = "echo safe"
"#,
    )
    .unwrap();

    let result = std::panic::catch_unwind(|| {
        let result = run("test", &["$(whoami)".to_string()]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Shell metacharacters")
        );
    });

    std::env::set_current_dir(original_dir).unwrap();
    result.unwrap();
}

#[test]
fn test_extract_host_port_env_var_default() {
    use services::extract_host_port_from_service;
    let svc = crate::manifest::ServiceConfig {
        image: None,
        build: None,
        port: None,
        ports: vec!["${CORPORATE_PORT:-3000}:3000".to_string()],
        command: None,
        volumes: vec![],
        env: IndexMap::new(),
        profiles: vec![],
        depends_on: vec![],
        restart: None,
        shm_size: None,
        container_name: None,
        working_dir: None,
        extra_hosts: vec![],
        deploy: None,
        watch: vec![],
        devices: vec![],
        runtime: None,
        gpu: None,
        health_path: None,
        network_mode: None,
        labels: vec![],
        networks: vec![],
        env_groups: vec![],
        mem_limit: None,
        cpus: None,
    };
    assert_eq!(extract_host_port_from_service(&svc), Some(3000));
}

#[test]
fn test_extract_host_port_plain_number() {
    use services::extract_host_port_from_service;
    let svc = crate::manifest::ServiceConfig {
        image: None,
        build: None,
        port: None,
        ports: vec!["8080:80".to_string()],
        command: None,
        volumes: vec![],
        env: IndexMap::new(),
        profiles: vec![],
        depends_on: vec![],
        restart: None,
        shm_size: None,
        container_name: None,
        working_dir: None,
        extra_hosts: vec![],
        deploy: None,
        watch: vec![],
        devices: vec![],
        runtime: None,
        gpu: None,
        health_path: None,
        network_mode: None,
        labels: vec![],
        networks: vec![],
        env_groups: vec![],
        mem_limit: None,
        cpus: None,
    };
    assert_eq!(extract_host_port_from_service(&svc), Some(8080));
}

#[test]
fn test_extract_host_port_fallback_to_port_field() {
    use services::extract_host_port_from_service;
    let svc = crate::manifest::ServiceConfig {
        image: None,
        build: None,
        port: Some(9090),
        ports: vec![],
        command: None,
        volumes: vec![],
        env: IndexMap::new(),
        profiles: vec![],
        depends_on: vec![],
        restart: None,
        shm_size: None,
        container_name: None,
        working_dir: None,
        extra_hosts: vec![],
        deploy: None,
        watch: vec![],
        devices: vec![],
        runtime: None,
        gpu: None,
        health_path: None,
        network_mode: None,
        labels: vec![],
        networks: vec![],
        env_groups: vec![],
        mem_limit: None,
        cpus: None,
    };
    assert_eq!(extract_host_port_from_service(&svc), Some(9090));
}

#[test]
fn test_extract_host_port_no_ports() {
    use services::extract_host_port_from_service;
    let svc = crate::manifest::ServiceConfig {
        image: None,
        build: None,
        port: None,
        ports: vec![],
        command: None,
        volumes: vec![],
        env: IndexMap::new(),
        profiles: vec![],
        depends_on: vec![],
        restart: None,
        shm_size: None,
        container_name: None,
        working_dir: None,
        extra_hosts: vec![],
        deploy: None,
        watch: vec![],
        devices: vec![],
        runtime: None,
        gpu: None,
        health_path: None,
        network_mode: None,
        labels: vec![],
        networks: vec![],
        env_groups: vec![],
        mem_limit: None,
        cpus: None,
    };
    assert_eq!(extract_host_port_from_service(&svc), None);
}

#[test]
fn test_hash_file_deterministic() {
    use hooks::hash_file;
    let dir = tempdir().unwrap();
    let file = dir.path().join("test.txt");
    std::fs::write(&file, "hello world").unwrap();
    let hash1 = hash_file(&file).unwrap();
    let hash2 = hash_file(&file).unwrap();
    assert_eq!(hash1, hash2);
    assert_eq!(
        hash1,
        "d74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24"
    );
}

#[test]
fn test_hash_file_changes_with_content() {
    use hooks::hash_file;
    let dir = tempdir().unwrap();
    let file = dir.path().join("test.txt");
    std::fs::write(&file, "version 1").unwrap();
    let hash1 = hash_file(&file).unwrap();
    std::fs::write(&file, "version 2").unwrap();
    let hash2 = hash_file(&file).unwrap();
    assert_ne!(hash1, hash2);
}

#[test]
fn test_pre_command_hooks_default_is_none() {
    let hooks = crate::manifest::PreCommandHooks::default();
    assert!(hooks.pre_command.is_none());
    assert!(hooks.skip.is_empty());
    assert!(hooks.cache.is_none());
}

#[test]
fn test_hooks_section_parses_from_toml() {
    let manifest: Manifest = toml::from_str(
        r#"
[workspace]
name = "test"
[hooks]
pre_command = "pnpm install"
skip = ["up", "down", "ps"]
[hooks.cache]
key = "pnpm-lock.yaml"
[versioning]
strategy = "manual"
"#,
    )
    .unwrap();
    assert_eq!(manifest.hooks.pre_command.as_deref(), Some("pnpm install"));
    assert_eq!(manifest.hooks.skip, vec!["up", "down", "ps"]);
    assert_eq!(manifest.hooks.cache.as_ref().unwrap().key, "pnpm-lock.yaml");
}

#[test]
fn test_remap_exact_match() {
    let mut remap = IndexMap::new();
    remap.insert("npm install".to_string(), "airis up".to_string());
    remap.insert("docker compose up".to_string(), "airis up".to_string());
    let result = find_remap_match(&remap, "npm install");
    assert!(result.is_some());
    let (from, to) = result.unwrap();
    assert_eq!(from, "npm install");
    assert_eq!(to, "airis up");
}

#[test]
fn test_remap_case_insensitive() {
    let mut remap = IndexMap::new();
    remap.insert("npm install".to_string(), "airis up".to_string());
    assert!(find_remap_match(&remap, "NPM INSTALL").is_some());
    assert!(find_remap_match(&remap, "Npm Install").is_some());
}

#[test]
fn test_remap_prefix_match() {
    let mut remap = IndexMap::new();
    remap.insert("npm install".to_string(), "airis up".to_string());
    let result = find_remap_match(&remap, "npm install foo");
    assert!(result.is_some());
    assert_eq!(result.unwrap().1, "airis up");
}

#[test]
fn test_remap_no_match() {
    let mut remap = IndexMap::new();
    remap.insert("npm install".to_string(), "airis up".to_string());
    assert!(find_remap_match(&remap, "cargo build").is_none());
    assert!(find_remap_match(&remap, "npm").is_none());
}

#[test]
fn test_remap_no_partial_word_match() {
    let mut remap = IndexMap::new();
    remap.insert("npm".to_string(), "airis up".to_string());
    assert!(find_remap_match(&remap, "npmx").is_none());
    assert!(find_remap_match(&remap, "npm install").is_some());
}

#[test]
fn test_remap_empty_table() {
    let remap = IndexMap::new();
    assert!(find_remap_match(&remap, "npm install").is_none());
}
