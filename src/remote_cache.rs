//! Remote cache for Docker build artifacts
//!
//! Supports S3 and OCI registry backends for sharing build cache across CI/CD.
//!
//! # Usage
//!
//! ```ignore
//! // S3 backend
//! let remote = Remote::parse("s3://bucket/prefix")?;
//!
//! // OCI backend
//! let remote = Remote::parse("oci://ghcr.io/org/cache")?;
//!
//! // Check for cache hit
//! if let Some(artifact) = remote_hit("apps/web", "abc123", &remote)? {
//!     println!("Cache hit: {}", artifact.image_ref);
//! }
//!
//! // Store after build
//! remote_store("apps/web", "abc123", &artifact, &remote)?;
//! ```

use anyhow::{Context, Result, bail};
use std::process::Command;

use crate::docker_build::CachedArtifact;

/// Remote cache backend
#[derive(Debug, Clone)]
pub enum Remote {
    /// S3 bucket storage
    S3 { bucket: String, prefix: String },
    /// OCI registry (using oras)
    Oci { registry: String },
}

impl Remote {
    /// Parse remote URL
    ///
    /// Supported formats:
    /// - `s3://bucket/prefix`
    /// - `oci://registry/image`
    pub fn parse(url: &str) -> Result<Self> {
        if let Some(rest) = url.strip_prefix("s3://") {
            let parts: Vec<&str> = rest.splitn(2, '/').collect();
            let bucket = parts.first().unwrap_or(&"").to_string();
            let prefix = parts.get(1).map(|s| s.to_string()).unwrap_or_default();

            if bucket.is_empty() {
                bail!("Invalid S3 URL: missing bucket name");
            }

            Ok(Remote::S3 { bucket, prefix })
        } else if let Some(rest) = url.strip_prefix("oci://") {
            if rest.is_empty() {
                bail!("Invalid OCI URL: missing registry");
            }
            Ok(Remote::Oci {
                registry: rest.to_string(),
            })
        } else {
            bail!(
                "Invalid remote cache URL: '{}'. Expected s3://bucket/prefix or oci://registry/image",
                url
            )
        }
    }

    /// Get cache key path for S3
    fn s3_key(&self, project: &str, hash: &str) -> String {
        let project_safe = project.replace('/', "_");
        let Remote::S3 { prefix, .. } = self else {
            unreachable!("s3_key called on non-S3 remote");
        };
        if prefix.is_empty() {
            format!("{}/{}/artifact.json", project_safe, hash)
        } else {
            format!("{}/{}/{}/artifact.json", prefix, project_safe, hash)
        }
    }

    /// Get OCI tag for cache
    fn oci_tag(&self, project: &str, hash: &str) -> String {
        let project_safe = project.replace('/', "-");
        let Remote::Oci { registry } = self else {
            unreachable!("oci_tag called on non-OCI remote");
        };
        format!("{}:{}-{}", registry, project_safe, hash)
    }
}

/// Check for remote cache hit
pub fn remote_hit(project: &str, hash: &str, remote: &Remote) -> Result<Option<CachedArtifact>> {
    match remote {
        Remote::S3 { bucket, .. } => {
            let key = remote.s3_key(project, hash);
            s3_get(bucket, &key)
        }
        Remote::Oci { .. } => {
            let tag = remote.oci_tag(project, hash);
            oci_pull(&tag)
        }
    }
}

/// Store artifact in remote cache
pub fn remote_store(
    project: &str,
    hash: &str,
    artifact: &CachedArtifact,
    remote: &Remote,
) -> Result<()> {
    match remote {
        Remote::S3 { bucket, .. } => {
            let key = remote.s3_key(project, hash);
            s3_put(bucket, &key, artifact)
        }
        Remote::Oci { .. } => {
            let tag = remote.oci_tag(project, hash);
            oci_push(&tag, artifact)
        }
    }
}

// =============================================================================
// S3 Backend (uses AWS CLI)
// =============================================================================

fn s3_get(bucket: &str, key: &str) -> Result<Option<CachedArtifact>> {
    let url = format!("s3://{}/{}", bucket, key);

    let output = Command::new("aws")
        .args(["s3", "cp", &url, "-"])
        .output()
        .context("Failed to run aws s3 cp (is AWS CLI installed?)")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // NoSuchKey = object not found → genuine cache miss
        if stderr.contains("NoSuchKey") || stderr.is_empty() {
            return Ok(None);
        }
        bail!("S3 cache fetch failed: {}", stderr.trim());
    }

    let content = String::from_utf8(output.stdout).context("Invalid UTF-8 from S3")?;

    let artifact: CachedArtifact =
        serde_json::from_str(&content).context("Failed to parse cached artifact from S3")?;

    Ok(Some(artifact))
}

fn s3_put(bucket: &str, key: &str, artifact: &CachedArtifact) -> Result<()> {
    let url = format!("s3://{}/{}", bucket, key);
    let content = serde_json::to_string_pretty(artifact)?;

    // NamedTempFile is auto-deleted on drop, even if an error occurs.
    let temp_file = tempfile::NamedTempFile::new().context("Failed to create temp file")?;
    std::fs::write(temp_file.path(), &content)?;
    let temp_file_str = temp_file
        .path()
        .to_str()
        .context("Temp file path contains non-UTF-8 characters")?;

    let output = Command::new("aws")
        .args(["s3", "cp", temp_file_str, &url])
        .output()
        .context("Failed to run aws s3 cp")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to upload to S3: {}", stderr.trim());
    }

    Ok(())
}

// =============================================================================
// OCI Backend (uses oras CLI)
// =============================================================================

fn oci_pull(tag: &str) -> Result<Option<CachedArtifact>> {
    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let temp_dir_str = temp_dir
        .path()
        .to_str()
        .context("Temp directory path contains non-UTF-8 characters")?;

    let output = Command::new("oras")
        .args(["pull", tag, "-o", temp_dir_str])
        .output()
        .context("Failed to run oras pull (is oras installed?)")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not found") || stderr.contains("404") || stderr.is_empty() {
            return Ok(None);
        }
        bail!("OCI cache pull failed: {}", stderr.trim());
    }

    let artifact_path = temp_dir.path().join("artifact.json");
    if !artifact_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&artifact_path)?;
    let artifact: CachedArtifact =
        serde_json::from_str(&content).context("Failed to parse cached artifact from OCI")?;

    Ok(Some(artifact))
}

fn oci_push(tag: &str, artifact: &CachedArtifact) -> Result<()> {
    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let artifact_path = temp_dir.path().join("artifact.json");
    let content = serde_json::to_string_pretty(artifact)?;
    std::fs::write(&artifact_path, &content)?;

    let output = Command::new("oras")
        .args(["push", tag, "artifact.json:application/json"])
        .current_dir(temp_dir.path())
        .output()
        .context("Failed to run oras push")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to push to OCI registry: {}", stderr.trim());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_s3_url() {
        let remote = Remote::parse("s3://my-bucket/cache/prefix").unwrap();
        match remote {
            Remote::S3 { bucket, prefix } => {
                assert_eq!(bucket, "my-bucket");
                assert_eq!(prefix, "cache/prefix");
            }
            _ => panic!("Expected S3"),
        }
    }

    #[test]
    fn test_parse_s3_url_no_prefix() {
        let remote = Remote::parse("s3://my-bucket").unwrap();
        match remote {
            Remote::S3 { bucket, prefix } => {
                assert_eq!(bucket, "my-bucket");
                assert_eq!(prefix, "");
            }
            _ => panic!("Expected S3"),
        }
    }

    #[test]
    fn test_parse_oci_url() {
        let remote = Remote::parse("oci://ghcr.io/org/cache").unwrap();
        match remote {
            Remote::Oci { registry } => {
                assert_eq!(registry, "ghcr.io/org/cache");
            }
            _ => panic!("Expected OCI"),
        }
    }

    #[test]
    fn test_parse_invalid_url() {
        assert!(Remote::parse("http://example.com").is_err());
        assert!(Remote::parse("s3://").is_err());
        assert!(Remote::parse("oci://").is_err());
    }

    #[test]
    fn test_s3_key() {
        let remote = Remote::S3 {
            bucket: "bucket".to_string(),
            prefix: "cache".to_string(),
        };
        let key = remote.s3_key("apps/web", "abc123");
        assert_eq!(key, "cache/apps_web/abc123/artifact.json");
    }

    #[test]
    fn test_oci_tag() {
        let remote = Remote::Oci {
            registry: "ghcr.io/org/cache".to_string(),
        };
        let tag = remote.oci_tag("apps/web", "abc123");
        assert_eq!(tag, "ghcr.io/org/cache:apps-web-abc123");
    }
}
