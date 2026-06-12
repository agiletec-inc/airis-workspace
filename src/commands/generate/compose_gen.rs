use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::manifest::Manifest;

/// Resolve the Docker image for a service based on its framework.
///
/// Each framework needs a runtime that can execute its build tools (uv, cargo, npm).
/// Falls back to the manifest workspace image (typically node:24-alpine) for Node apps.
fn resolve_service_image(framework: Option<&str>, workspace_image: &str) -> String {
    match framework.unwrap_or("node") {
        "python" => "python:3.12-slim".to_string(),
        "rust" => "rust:1-slim".to_string(),
        _ => workspace_image.to_string(),
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct ComposeFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(default)]
    services: IndexMap<String, ComposeService>,
    #[serde(skip_serializing_if = "IndexMap::is_empty", default)]
    volumes: IndexMap<String, ComposeVolume>,
    #[serde(skip_serializing_if = "IndexMap::is_empty", default)]
    networks: IndexMap<String, ComposeNetwork>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
struct ComposeService {
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    container_name: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    volumes: Vec<String>,
    #[serde(skip_serializing_if = "IndexMap::is_empty", default)]
    environment: IndexMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    working_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    networks: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    ports: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    expose: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    restart: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    healthcheck: Option<ComposeHealthcheck>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    depends_on: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    profiles: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deploy: Option<ComposeDeploy>,
    /// Marker indicating this service is managed by `airis gen` and may be
    /// regenerated. Services without this marker are preserved verbatim.
    #[serde(rename = "x-airis-managed", skip_serializing_if = "is_false", default)]
    airis_managed: bool,
    /// Catch-all for any compose fields we don't model explicitly. Preserves
    /// user-written services when merging.
    #[serde(flatten)]
    extra: IndexMap<String, serde_yaml_ng::Value>,
}

fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
struct ComposeHealthcheck {
    test: Vec<String>,
    interval: String,
    timeout: String,
    retries: u32,
    start_period: String,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
struct ComposeDeploy {
    #[serde(skip_serializing_if = "Option::is_none")]
    resources: Option<ComposeResources>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
struct ComposeResources {
    /// A single map in the Compose spec (`{ cpus, memory, devices, ... }`),
    /// not a sequence.
    #[serde(skip_serializing_if = "Option::is_none")]
    reservations: Option<ComposeReservation>,
    /// Catch-all for unmodeled fields such as `limits`.
    #[serde(flatten)]
    extra: IndexMap<String, serde_yaml_ng::Value>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
struct ComposeReservation {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    devices: Vec<ComposeDevice>,
    /// Catch-all for unmodeled fields such as `cpus`, `memory`, `generic_resources`.
    #[serde(flatten)]
    extra: IndexMap<String, serde_yaml_ng::Value>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
struct ComposeDevice {
    #[serde(skip_serializing_if = "Option::is_none")]
    driver: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    count: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    capabilities: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
struct ComposeVolume {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    external: Option<bool>,
    #[serde(flatten)]
    extra: IndexMap<String, serde_yaml_ng::Value>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
struct ComposeNetwork {
    #[serde(skip_serializing_if = "Option::is_none")]
    external: Option<bool>,
    #[serde(flatten)]
    extra: IndexMap<String, serde_yaml_ng::Value>,
}

/// Convert a path component to a volume name slug:
/// "apps/api" + ".next" -> "apps-api-next"
fn slug(s: &str) -> String {
    s.replace('/', "-")
        .replace('.', "")
        .replace('_', "-")
        .trim_matches('-')
        .to_string()
}

/// Find an existing compose file at the project root, in Docker's official
/// priority order: compose.yaml > compose.yml > docker-compose.yaml > docker-compose.yml.
fn find_existing_compose() -> Option<PathBuf> {
    for name in [
        "compose.yaml",
        "compose.yml",
        "docker-compose.yaml",
        "docker-compose.yml",
    ] {
        let p = Path::new(name);
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }
    None
}

/// Generate the project-root compose.yaml from manifest.toml.
///
/// Behavior:
/// - Writes to existing `compose.yaml`/`.yml`/`docker-compose.yaml`/`.yml` if
///   present (preserving the file name); otherwise creates `compose.yaml`.
/// - Services are emitted with production-ready fields (restart, healthcheck,
///   ports) derived from `crate::conventions`.
/// - Each generated service gets `x-airis-managed: true`. On regeneration,
///   user-added services (without that marker) are preserved verbatim.
/// - Build artifact dirs (`.next`, `.turbo`, `node_modules`, etc.) are mounted
///   as named volumes so they never leak to the host.
pub fn generate_workspace_compose(manifest: &Manifest) -> Result<()> {
    let mut services: IndexMap<String, ComposeService> = IndexMap::new();
    let mut volumes: IndexMap<String, ComposeVolume> = IndexMap::new();
    let networks: IndexMap<String, ComposeNetwork> = IndexMap::new();

    let project_name = &manifest.project.id;

    // Workspace-wide environment variables
    let mut workspace_env = IndexMap::new();
    workspace_env.insert("NODE_ENV".to_string(), "development".to_string());

    // Workspace-wide volumes (shared by the workspace runner)
    let mut workspace_volumes = vec![".:/app".to_string()];

    // Apply global stack conventions (caches, env, root-level isolation)
    for stack_name in ["pnpm", "rust", "python"] {
        let defaults = crate::conventions::framework_defaults(stack_name);
        for (cache_id, mount_path) in defaults.global_caches {
            let volume_name = format!("{}-{}", project_name, cache_id);
            volumes.insert(volume_name.clone(), ComposeVolume::default());
            workspace_volumes.push(format!("{}:{}", volume_name, mount_path));
        }
        for (key, val) in defaults.docker_env {
            workspace_env.insert(key.to_string(), val.to_string());
        }
        // Root-level isolated dirs as named volumes (host pollution guard).
        for dir in defaults.isolated_dirs {
            let volume_name = format!("{}-root-{}", project_name, slug(dir));
            volumes.entry(volume_name.clone()).or_default();
            let mount_str = format!("{}:/app/{}", volume_name, dir);
            if !workspace_volumes.contains(&mount_str) {
                workspace_volumes.push(mount_str);
            }
        }
    }

    // Per-app services (from [[app]] entries)
    for app in &manifest.app {
        let Some(path) = app.path.as_deref() else {
            continue;
        };
        let framework = app.framework.as_deref().unwrap_or("node");
        let stack_def = app
            .use_stack
            .as_deref()
            .and_then(|name| manifest.stack.get(name));
        let use_gpu = app.cuda.is_some() || stack_def.is_some_and(|s| s.gpu);
        let extra_artifacts: Vec<&str> = stack_def
            .map(|s| s.artifacts.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default();
        let svc = build_app_service(
            project_name,
            &app.name,
            path,
            framework,
            use_gpu,
            &extra_artifacts,
            &manifest.workspace.image,
            &mut volumes,
        );
        services.insert(app.name.clone(), svc);
    }

    // Per-app services from [apps.X] map (skip duplicates)
    for (name, app) in &manifest.apps {
        if services.contains_key(name) {
            continue;
        }
        let Some(path) = app.path.as_deref() else {
            continue;
        };
        let framework = app
            .framework
            .as_deref()
            .or(app.app_type.as_deref())
            .unwrap_or("node");
        let svc = build_app_service(
            project_name,
            name,
            path,
            framework,
            false,
            &[],
            &manifest.workspace.image,
            &mut volumes,
        );
        services.insert(name.clone(), svc);
    }

    // Workspace runner (dev container for `docker compose exec workspace ...`)
    services.insert(
        "workspace".to_string(),
        ComposeService {
            image: Some(manifest.workspace.image.clone()),
            container_name: Some(format!("{}-workspace", project_name)),
            volumes: workspace_volumes,
            environment: workspace_env,
            working_dir: Some("/app".to_string()),
            restart: Some("unless-stopped".to_string()),
            airis_managed: true,
            ..Default::default()
        },
    );

    let generated = ComposeFile {
        version: None,
        services,
        volumes,
        networks,
    };

    // Merge with any existing root compose to preserve user-authored services.
    let target_path = find_existing_compose().unwrap_or_else(|| PathBuf::from("compose.yaml"));
    let final_compose = if target_path.exists() {
        merge_with_existing(generated, &target_path)?
    } else {
        generated
    };

    let header = "# Auto-merged by `airis gen`. Services without `x-airis-managed: true`\n\
                  # are user-owned and will not be touched on regeneration.\n";
    let body =
        serde_yaml_ng::to_string(&final_compose).context("failed to serialize compose.yaml")?;
    let content = format!("{}{}", header, body);

    fs::write(&target_path, content)
        .with_context(|| format!("failed to write {}", target_path.display()))?;

    Ok(())
}

/// Build a single app service with production-ready fields and named volumes
/// for all isolated dirs.
#[allow(clippy::too_many_arguments)]
fn build_app_service(
    project_name: &str,
    name: &str,
    path: &str,
    framework: &str,
    use_gpu_override: bool,
    stack_artifacts: &[&str],
    workspace_image: &str,
    volumes: &mut IndexMap<String, ComposeVolume>,
) -> ComposeService {
    let defaults = crate::conventions::framework_defaults(framework);
    let mut env = IndexMap::new();
    let mut svc_volumes = vec![format!(".:/app/{}", path)];

    // Named volumes for every isolated dir (no anonymous-volume leaks).
    let isolated: Vec<&str> = defaults
        .isolated_dirs
        .iter()
        .copied()
        .chain(stack_artifacts.iter().copied())
        .collect();
    for dir in isolated {
        let volume_name = format!("{}-{}-{}", project_name, slug(name), slug(dir));
        volumes.entry(volume_name.clone()).or_default();
        svc_volumes.push(format!("{}:/app/{}/{}", volume_name, path, dir));
    }
    for (cache_id, mount_path) in defaults.global_caches {
        let volume_name = format!("{}-{}", project_name, cache_id);
        volumes.entry(volume_name.clone()).or_default();
        svc_volumes.push(format!("{}:{}", volume_name, mount_path));
    }
    for (key, val) in defaults.docker_env {
        env.insert(key.to_string(), val.to_string());
    }

    let healthcheck = defaults.healthcheck_test().map(|test| ComposeHealthcheck {
        test,
        interval: "30s".to_string(),
        timeout: "10s".to_string(),
        retries: 3,
        start_period: "10s".to_string(),
    });

    let ports = if defaults.port > 0 {
        vec![format!("{0}:{0}", defaults.port)]
    } else {
        Vec::new()
    };

    let deploy = if use_gpu_override {
        Some(ComposeDeploy {
            resources: Some(ComposeResources {
                reservations: Some(ComposeReservation {
                    devices: vec![ComposeDevice {
                        driver: Some("nvidia".to_string()),
                        count: Some(serde_json::json!("all")),
                        capabilities: vec!["gpu".to_string()],
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            }),
        })
    } else {
        None
    };

    ComposeService {
        image: Some(resolve_service_image(Some(framework), workspace_image)),
        container_name: Some(format!("{}-{}", project_name, name)),
        volumes: svc_volumes,
        environment: env,
        working_dir: Some(format!("/app/{}", path)),
        ports,
        restart: Some("unless-stopped".to_string()),
        healthcheck,
        deploy,
        airis_managed: true,
        ..Default::default()
    }
}

/// Resolve YAML merge keys (`<<`) throughout a value tree.
///
/// Compose files commonly DRY up shared config with anchors and `<<` merge
/// keys (e.g. `<<: *defaults`). `serde_yaml_ng` resolves anchors but leaves
/// `<<` as a literal key, so merges must be applied before deserializing into
/// the typed model. Keys already present on the host mapping win over merged
/// ones, matching the YAML merge-key spec.
fn resolve_merge_keys(value: &mut serde_yaml_ng::Value) {
    use serde_yaml_ng::Value;
    match value {
        Value::Mapping(map) => {
            for (_, child) in map.iter_mut() {
                resolve_merge_keys(child);
            }
            if let Some(merge) = map.remove("<<") {
                let sources = match merge {
                    Value::Sequence(seq) => seq,
                    single => vec![single],
                };
                for source in sources {
                    if let Value::Mapping(source_map) = source {
                        for (key, val) in source_map {
                            if !map.contains_key(&key) {
                                map.insert(key, val);
                            }
                        }
                    }
                }
            }
        }
        Value::Sequence(seq) => {
            for item in seq.iter_mut() {
                resolve_merge_keys(item);
            }
        }
        _ => {}
    }
}

/// Merge generated services with the existing compose file:
/// - Existing services with `x-airis-managed: true` are replaced by the new
///   generated version (or removed if no longer generated).
/// - Existing services without the marker are preserved verbatim.
/// - New generated services are appended.
/// - Existing volumes/networks are preserved; generated ones are added if absent.
fn merge_with_existing(generated: ComposeFile, existing_path: &Path) -> Result<ComposeFile> {
    let raw = fs::read_to_string(existing_path)
        .with_context(|| format!("failed to read {}", existing_path.display()))?;
    let existing: ComposeFile = if raw.trim().is_empty() {
        ComposeFile::default()
    } else {
        let mut value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&raw)
            .with_context(|| format!("failed to parse {} as YAML", existing_path.display()))?;
        resolve_merge_keys(&mut value);
        serde_yaml_ng::from_value(value)
            .with_context(|| format!("failed to parse {} as YAML", existing_path.display()))?
    };

    let mut merged = ComposeFile {
        version: existing.version.clone(),
        services: IndexMap::new(),
        volumes: existing.volumes.clone(),
        networks: existing.networks.clone(),
    };

    // 1. Walk existing services in order; replace airis-managed ones.
    for (name, svc) in &existing.services {
        if svc.airis_managed {
            if let Some(new_svc) = generated.services.get(name) {
                merged.services.insert(name.clone(), new_svc.clone());
            }
            // If the generated set no longer includes this name, drop it.
        } else {
            // User-authored: preserve as-is.
            merged.services.insert(name.clone(), svc.clone());
        }
    }

    // 2. Append any generated services that didn't already exist.
    for (name, svc) in &generated.services {
        if !merged.services.contains_key(name) {
            merged.services.insert(name.clone(), svc.clone());
        }
    }

    // 3. Merge volumes/networks (existing wins, additions appended).
    for (k, v) in generated.volumes {
        merged.volumes.entry(k).or_insert(v);
    }
    for (k, v) in generated.networks {
        merged.networks.entry(k).or_insert(v);
    }

    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn slug_normalizes_paths_and_dots() {
        assert_eq!(slug("apps/api"), "apps-api");
        assert_eq!(slug(".next"), "next");
        assert_eq!(slug("node_modules"), "node-modules");
        assert_eq!(slug(".pytest_cache"), "pytest-cache");
    }

    #[test]
    fn merge_preserves_user_services() {
        let dir = tempdir().unwrap();
        let existing_path = dir.path().join("compose.yaml");
        fs::write(
            &existing_path,
            "services:\n  postgres:\n    image: postgres:16\n    ports: [5432:5432]\n",
        )
        .unwrap();

        let mut generated_services = IndexMap::new();
        generated_services.insert(
            "api".to_string(),
            ComposeService {
                image: Some("node:24-alpine".to_string()),
                airis_managed: true,
                ..Default::default()
            },
        );
        let generated = ComposeFile {
            version: None,
            services: generated_services,
            volumes: IndexMap::new(),
            networks: IndexMap::new(),
        };

        let merged = merge_with_existing(generated, &existing_path).unwrap();
        assert!(merged.services.contains_key("postgres"));
        assert!(merged.services.contains_key("api"));
        // postgres preserved verbatim (no airis-managed marker)
        assert!(!merged.services["postgres"].airis_managed);
    }

    #[test]
    fn merge_replaces_airis_managed_services() {
        let dir = tempdir().unwrap();
        let existing_path = dir.path().join("compose.yaml");
        fs::write(
            &existing_path,
            "services:\n  api:\n    image: stale:1\n    x-airis-managed: true\n",
        )
        .unwrap();

        let mut generated_services = IndexMap::new();
        generated_services.insert(
            "api".to_string(),
            ComposeService {
                image: Some("node:24-alpine".to_string()),
                airis_managed: true,
                ..Default::default()
            },
        );
        let generated = ComposeFile {
            version: None,
            services: generated_services,
            volumes: IndexMap::new(),
            networks: IndexMap::new(),
        };

        let merged = merge_with_existing(generated, &existing_path).unwrap();
        assert_eq!(
            merged.services["api"].image.as_deref(),
            Some("node:24-alpine")
        );
    }

    #[test]
    fn merge_drops_airis_managed_services_no_longer_generated() {
        let dir = tempdir().unwrap();
        let existing_path = dir.path().join("compose.yaml");
        fs::write(
            &existing_path,
            "services:\n  removed-app:\n    image: foo:1\n    x-airis-managed: true\n  user-svc:\n    image: bar:1\n",
        )
        .unwrap();

        let generated = ComposeFile {
            version: None,
            services: IndexMap::new(),
            volumes: IndexMap::new(),
            networks: IndexMap::new(),
        };

        let merged = merge_with_existing(generated, &existing_path).unwrap();
        assert!(!merged.services.contains_key("removed-app"));
        assert!(merged.services.contains_key("user-svc"));
    }

    #[test]
    fn build_app_service_has_production_fields() {
        let mut volumes = IndexMap::new();
        let svc = build_app_service(
            "myproj",
            "api",
            "apps/api",
            "nextjs",
            false,
            &[],
            "node:24-alpine",
            &mut volumes,
        );
        assert_eq!(svc.restart.as_deref(), Some("unless-stopped"));
        assert!(svc.healthcheck.is_some());
        assert_eq!(svc.ports, vec!["3000:3000"]);
        assert!(svc.airis_managed);
        // .next must be a named volume, not a bind mount or anonymous.
        assert!(volumes.contains_key("myproj-api-next"));
        assert!(
            svc.volumes
                .iter()
                .any(|v| v.starts_with("myproj-api-next:"))
        );
    }

    #[test]
    fn merge_parses_gpu_resource_reservations() {
        // Regression: `deploy.resources.reservations` is a map in the Compose
        // spec, not a sequence. Parsing a GPU service must not fail.
        let dir = tempdir().unwrap();
        let existing_path = dir.path().join("compose.yaml");
        fs::write(
            &existing_path,
            r#"services:
  gpu-svc:
    image: cuda:12
    deploy:
      resources:
        limits:
          memory: 4g
        reservations:
          devices:
            - driver: nvidia
              count: 1
              capabilities: [gpu]
"#,
        )
        .unwrap();

        let merged = merge_with_existing(ComposeFile::default(), &existing_path).unwrap();
        assert!(merged.services.contains_key("gpu-svc"));
    }

    #[test]
    fn merge_resolves_yaml_merge_keys() {
        // Regression: compose files DRY up config with YAML anchors and `<<`
        // merge keys, including nested ones (e.g. inside `environment`).
        let dir = tempdir().unwrap();
        let existing_path = dir.path().join("compose.yaml");
        fs::write(
            &existing_path,
            r#"x-base: &base
  restart: always
  networks: [traefik]
x-env: &env
  NODE_ENV: production
services:
  web:
    <<: *base
    image: nginx
    environment:
      <<: *env
      EXTRA: "1"
"#,
        )
        .unwrap();

        let merged = merge_with_existing(ComposeFile::default(), &existing_path).unwrap();
        let web = &merged.services["web"];
        assert_eq!(web.restart.as_deref(), Some("always"));
        assert_eq!(web.networks, vec!["traefik"]);
        assert_eq!(web.image.as_deref(), Some("nginx"));
        assert_eq!(
            web.environment.get("NODE_ENV").map(String::as_str),
            Some("production")
        );
        assert_eq!(web.environment.get("EXTRA").map(String::as_str), Some("1"));
    }
}
