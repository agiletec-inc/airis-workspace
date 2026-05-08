use anyhow::Result;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use super::write_with_backup;
use crate::manifest::Manifest;

/// Resolve the Docker image for a dev workspace service based on its framework.
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

#[derive(Serialize, Deserialize, Debug)]
struct ComposeFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    services: IndexMap<String, ComposeService>,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    volumes: IndexMap<String, ComposeVolume>,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    networks: IndexMap<String, ComposeNetwork>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct ComposeService {
    image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    container_name: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    volumes: Vec<String>,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    environment: IndexMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    working_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    networks: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deploy: Option<ComposeDeploy>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct ComposeDeploy {
    #[serde(skip_serializing_if = "Option::is_none")]
    resources: Option<ComposeResources>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct ComposeResources {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    reservations: Vec<ComposeReservation>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct ComposeReservation {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    devices: Vec<ComposeDevice>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct ComposeDevice {
    #[serde(skip_serializing_if = "Option::is_none")]
    driver: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    count: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    capabilities: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct ComposeVolume {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    external: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct ComposeNetwork {
    #[serde(skip_serializing_if = "Option::is_none")]
    external: Option<bool>,
}

/// Generate workspace/docker-compose.yml from manifest.toml
pub fn generate_workspace_compose(manifest: &Manifest) -> Result<()> {
    let mut services = IndexMap::new();
    let mut volumes = IndexMap::new();
    let networks = IndexMap::new();

    let project_name = &manifest.project.id;

    // Workspace-wide global environment variables
    let mut workspace_env = IndexMap::new();
    workspace_env.insert("NODE_ENV".to_string(), "development".to_string());

    // Workspace-wide global volumes
    let mut workspace_volumes = vec![
        ".:/app".to_string(), // Source bind mount (for human/AI editing)
    ];

    // Identify global stacks to apply based on manifest (pnpm, cargo, etc.)
    let global_stacks = ["pnpm", "rust", "python"];

    // Apply global stack conventions (Caches, Env, and root-level isolation)
    for stack_name in global_stacks {
        let defaults = crate::conventions::framework_defaults(stack_name);
        for (cache_id, mount_path) in defaults.global_caches {
            let volume_name = format!("{}-{}", project_name, cache_id);
            volumes.insert(volume_name.clone(), ComposeVolume::default());
            workspace_volumes.push(format!("{}:{}", volume_name, mount_path));
        }
        for (key, val) in defaults.docker_env {
            workspace_env.insert(key.to_string(), val.to_string());
        }
        // Isolate root-level artifact dirs so host is never polluted by workspace-wide
        // package manager output (e.g. /app/node_modules from pnpm install at the root).
        for dir in defaults.isolated_dirs {
            let mount_point = format!("/app/{}", dir);
            if !workspace_volumes.contains(&mount_point) {
                workspace_volumes.push(mount_point);
            }
        }
    }

    // Process each application
    for app in &manifest.app {
        if let Some(ref path) = app.path {
            let mut app_env = IndexMap::new();
            let mut app_volumes = vec![format!("{}:/app/{}", ".", path)];
            let mut use_gpu = app.cuda.is_some();

            // 1. Get conventions from framework
            let framework = app.framework.as_deref().unwrap_or("node");
            let defaults = crate::conventions::framework_defaults(framework);

            // Add project-local isolation volumes (Anonymous volumes to hide host junk)
            for dir in defaults.isolated_dirs {
                let mount_point = format!("/app/{}/{}", path, dir);
                app_volumes.push(mount_point.clone());
                if !workspace_volumes.contains(&mount_point) {
                    workspace_volumes.push(mount_point);
                }
            }

            // Also add global caches for this specific service
            for (cache_id, mount_path) in defaults.global_caches {
                let volume_name = format!("{}-{}", project_name, cache_id);
                volumes.insert(volume_name.clone(), ComposeVolume::default());
                app_volumes.push(format!("{}:{}", volume_name, mount_path));
            }
            for (key, val) in defaults.docker_env {
                app_env.insert(key.to_string(), val.to_string());
            }

            // 2. Override/Extend with user-defined Stack definition
            if let Some(ref stack_name) = app.use_stack
                && let Some(stack_def) = manifest.stack.get(stack_name)
            {
                for dir in &stack_def.artifacts {
                    let volume_name = format!(
                        "{}-{}-{}",
                        project_name,
                        path.replace('/', "-"),
                        dir.replace('.', "").replace('/', "-")
                    );
                    volumes.insert(volume_name.clone(), ComposeVolume::default());
                    let mount_point = format!("{}:/app/{}/{}", volume_name, path, dir);
                    app_volumes.push(mount_point.clone());
                    if !workspace_volumes.contains(&mount_point) {
                        workspace_volumes.push(mount_point);
                    }
                }
                if stack_def.gpu {
                    use_gpu = true;
                }
            }

            // GPU Support
            let deploy = if use_gpu {
                Some(ComposeDeploy {
                    resources: Some(ComposeResources {
                        reservations: vec![ComposeReservation {
                            devices: vec![ComposeDevice {
                                driver: Some("nvidia".to_string()),
                                count: Some(serde_json::json!("all")),
                                capabilities: vec!["gpu".to_string()],
                            }],
                        }],
                    }),
                })
            } else {
                None
            };

            let image = resolve_service_image(
                app.framework.as_deref(),
                &manifest.workspace.image,
            );
            services.insert(
                app.name.clone(),
                ComposeService {
                    image,
                    container_name: Some(format!("{}-{}", project_name, app.name)),
                    volumes: app_volumes,
                    environment: app_env,
                    working_dir: Some(format!("/app/{}", path)),
                    deploy,
                    ..Default::default()
                },
            );
        }
    }

    // Process apps defined in [apps.X] table format (IndexMap, newer convention)
    for (name, app) in &manifest.apps {
        if services.contains_key(name) {
            continue; // already handled via [[app]] entries
        }
        if let Some(ref path) = app.path {
            let mut app_env = IndexMap::new();
            let mut app_volumes = vec![format!("{}:/app/{}", ".", path)];

            let framework = app
                .framework
                .as_deref()
                .or(app.app_type.as_deref())
                .unwrap_or("node");
            let defaults = crate::conventions::framework_defaults(framework);

            for dir in defaults.isolated_dirs {
                let mount_point = format!("/app/{}/{}", path, dir);
                app_volumes.push(mount_point.clone());
                if !workspace_volumes.contains(&mount_point) {
                    workspace_volumes.push(mount_point);
                }
            }
            for (cache_id, mount_path) in defaults.global_caches {
                let volume_name = format!("{}-{}", project_name, cache_id);
                volumes.insert(volume_name.clone(), ComposeVolume::default());
                app_volumes.push(format!("{}:{}", volume_name, mount_path));
            }
            for (key, val) in defaults.docker_env {
                app_env.insert(key.to_string(), val.to_string());
            }

            let image = resolve_service_image(Some(framework), &manifest.workspace.image);
            services.insert(
                name.clone(),
                ComposeService {
                    image,
                    container_name: Some(format!("{}-{}", project_name, name)),
                    volumes: app_volumes,
                    environment: app_env,
                    working_dir: Some(format!("/app/{}", path)),
                    ..Default::default()
                },
            );
        }
    }

    // Main workspace container (for airis run/exec)
    services.insert(
        "workspace".to_string(),
        ComposeService {
            image: manifest.workspace.image.clone(),
            container_name: Some(format!("{}-workspace", project_name)),
            volumes: workspace_volumes,
            environment: workspace_env,
            working_dir: Some("/app".to_string()),
            ..Default::default()
        },
    );

    let compose = ComposeFile {
        version: None,
        services,
        volumes,
        networks,
    };

    let content = serde_yml::to_string(&compose)?;

    // Standardize on compose.yaml (Docker Compose V2) at the project root
    let target_path = Path::new("compose.yaml");
    if let Some(parent) = target_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    write_with_backup(target_path, &content)?;

    Ok(())
}
