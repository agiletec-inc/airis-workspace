//! Version resolution utilities for npm packages
//!
//! Provides functions to resolve version policies (latest, lts) to actual version numbers
//! by querying the npm registry via HTTP (no npm CLI dependency).

use anyhow::{Context, Result};

const NPM_REGISTRY: &str = "https://registry.npmjs.org";

/// Resolve a version policy to an actual version number
///
/// Supports:
/// - "latest" → fetch latest version from npm
/// - "lts" → fetch LTS version from npm dist-tags
/// - "^X.Y.Z" or "~X.Y.Z" → pass through as-is
/// - Any other string → treat as specific version
pub fn resolve_version(package: &str, policy: &str) -> Result<String> {
    match policy {
        "latest" => get_npm_latest(package),
        "lts" => get_npm_lts(package),
        version if version.starts_with('^') || version.starts_with('~') => Ok(version.to_string()),
        _ => Ok(policy.to_string()),
    }
}

/// Fetch dist-tags for a package from the npm registry
fn fetch_dist_tags(package: &str) -> Result<serde_json::Value> {
    let url = format!("{NPM_REGISTRY}/-/package/{package}/dist-tags");
    let body = ureq::get(&url)
        .call()
        .context(format!("Failed to fetch dist-tags for {package}"))?
        .into_body()
        .read_to_string()
        .context(format!("Failed to read response body for {package}"))?;

    let json: serde_json::Value = serde_json::from_str(&body)
        .context(format!("Failed to parse dist-tags JSON for {package}"))?;

    Ok(json)
}

/// Get the latest version of a package from npm registry
pub fn get_npm_latest(package: &str) -> Result<String> {
    let tags = fetch_dist_tags(package)?;

    let version = tags
        .get("latest")
        .and_then(|v| v.as_str())
        .context(format!("No 'latest' dist-tag found for {package}"))?;

    Ok(format!("^{version}"))
}

/// Get the LTS version of a package from npm dist-tags
///
/// Priority:
/// 1. "lts" tag if it exists
/// 2. Highest "*-lts" tag (e.g., v20-lts > v18-lts for Node.js)
/// 3. Falls back to "latest" if no LTS tag found
pub fn get_npm_lts(package: &str) -> Result<String> {
    let tags = fetch_dist_tags(package)?;

    // Priority: "lts" tag > "*-lts" pattern (highest version) > "latest"
    if let Some(lts) = tags.get("lts").and_then(|v| v.as_str()) {
        return Ok(format!("^{lts}"));
    }

    // Find *-lts tags (e.g., v20-lts, v18-lts for Node.js)
    if let Some(obj) = tags.as_object() {
        let mut lts_versions: Vec<(&str, &str)> = obj
            .iter()
            .filter(|(k, _)| k.ends_with("-lts"))
            .filter_map(|(k, v)| v.as_str().map(|ver| (k.as_str(), ver)))
            .collect();

        // Sort by tag name to get highest LTS (e.g., v20-lts > v18-lts)
        lts_versions.sort_by(|a, b| b.0.cmp(a.0));

        if let Some((_, version)) = lts_versions.first() {
            return Ok(format!("^{version}"));
        }
    }

    // Fallback to latest
    let version = tags
        .get("latest")
        .and_then(|v| v.as_str())
        .context(format!("No 'latest' dist-tag found for {package}"))?;

    Ok(format!("^{version}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_version_passthrough() {
        // Specific versions should pass through
        assert_eq!(resolve_version("react", "^18.0.0").unwrap(), "^18.0.0");
        assert_eq!(resolve_version("react", "~17.0.0").unwrap(), "~17.0.0");
        assert_eq!(resolve_version("react", "18.0.0").unwrap(), "18.0.0");
    }

    // Note: Tests for "latest" and "lts" require network access
    // They are tested implicitly via integration tests
}
