use crate::manifest::Manifest;

/// Resolve `env_groups` + explicit `env` from an AppDeployConfig into a single env list.
///
/// Group entries are expanded first, then explicit entries are appended (allowing overrides).
/// Each entry is formatted as "KEY: VALUE" for deploy compose environment sections.
pub fn resolve_deploy_env(
    deploy: &crate::manifest::AppDeployConfig,
    manifest: &Manifest,
) -> Vec<String> {
    let mut resolved: Vec<String> = Vec::new();
    for group_name in &deploy.env_groups {
        if let Some(group) = manifest.env_group.get(group_name) {
            for (key, value) in group {
                resolved.push(format!("{key}: {value}"));
            }
        }
    }
    // Explicit env entries override group entries
    resolved.extend(deploy.env.iter().cloned());
    resolved
}
