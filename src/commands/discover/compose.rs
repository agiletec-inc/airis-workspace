//! Docker compose file discovery.

use anyhow::Result;
use std::fs;
use std::path::Path;

use super::types::{ComposeLocation, DetectedCompose};

/// Find docker-compose files in the workspace
pub fn find_compose_files() -> Result<Vec<DetectedCompose>> {
    let mut files = Vec::new();

    // Check standard locations
    let locations = [
        // Modern naming (preferred)
        ("compose.yml", ComposeLocation::Root),
        ("compose.yaml", ComposeLocation::Root),
        ("workspace/compose.yml", ComposeLocation::Workspace),
        ("workspace/compose.yaml", ComposeLocation::Workspace),
        ("supabase/compose.yml", ComposeLocation::Supabase),
        ("supabase/compose.yaml", ComposeLocation::Supabase),
        ("traefik/compose.yml", ComposeLocation::Traefik),
        ("traefik/compose.yaml", ComposeLocation::Traefik),
        // Legacy naming (backwards compatibility)
        ("docker-compose.yml", ComposeLocation::Root),
        ("docker-compose.yaml", ComposeLocation::Root),
        ("workspace/docker-compose.yml", ComposeLocation::Workspace),
        ("workspace/docker-compose.yaml", ComposeLocation::Workspace),
        ("supabase/docker-compose.yml", ComposeLocation::Supabase),
        ("supabase/docker-compose.yaml", ComposeLocation::Supabase),
        ("traefik/docker-compose.yml", ComposeLocation::Traefik),
        ("traefik/docker-compose.yaml", ComposeLocation::Traefik),
    ];

    for (path, location) in locations {
        if Path::new(path).exists() {
            files.push(DetectedCompose {
                path: path.to_string(),
                location,
            });
        }
    }

    // Check for compose files in apps/ directories
    let apps_dir = Path::new("apps");
    if apps_dir.exists()
        && let Ok(entries) = fs::read_dir(apps_dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                for compose_name in ["compose.yml", "compose.yaml"] {
                    let compose_path = path.join(compose_name);
                    if compose_path.exists() {
                        let rel_path = compose_path
                            .strip_prefix(".")
                            .unwrap_or(&compose_path)
                            .to_string_lossy()
                            .to_string();
                        files.push(DetectedCompose {
                            path: rel_path,
                            location: ComposeLocation::App,
                        });
                    }
                }
            }
        }
    }

    Ok(files)
}
