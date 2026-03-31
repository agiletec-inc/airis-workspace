use crate::manifest::ActionsVersions;
use crate::version_resolver::resolve_all_action_versions;
use anyhow::Result;

/// Resolved GitHub Actions versions with full action references (e.g., "actions/checkout@v6")
pub(super) struct ResolvedActions {
    pub checkout: String,
    pub pnpm: String,
    pub setup_node: String,
    pub cache: String,
    pub doppler: String,
    pub upload_artifact: String,
    pub download_artifact: String,
}

impl ResolvedActions {
    pub fn from_manifest(actions: &ActionsVersions) -> Result<Self> {
        let resolved = resolve_all_action_versions(actions)?;
        Ok(ResolvedActions {
            checkout: format!("actions/checkout@{}", resolved.checkout),
            pnpm: format!("pnpm/action-setup@{}", resolved.pnpm),
            setup_node: format!("actions/setup-node@{}", resolved.setup_node),
            cache: format!("actions/cache@{}", resolved.cache),
            doppler: format!("dopplerhq/cli-action@{}", resolved.doppler),
            upload_artifact: format!("actions/upload-artifact@{}", resolved.upload_artifact),
            download_artifact: format!("actions/download-artifact@{}", resolved.download_artifact),
        })
    }
}
