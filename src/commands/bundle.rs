//! Bundle command: Generate complete deployment packages
//!
//! Creates distribution-ready artifacts from built projects:
//! - bundle.json: Metadata (version, hash, deps, timestamps)
//! - image.tar: Docker image tarball (docker save)
//! - artifact.tar.gz: Standalone build artifacts

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::docker_build::{cache_hit, compute_content_hash};

/// Bundle output result
#[derive(Debug)]
#[allow(dead_code)]
pub struct BundleResult {
    pub output_dir: PathBuf,
    pub bundle_json: PathBuf,
    pub image_tar: Option<PathBuf>,
    pub artifact_tar: Option<PathBuf>,
    pub k8s_dir: Option<PathBuf>,
}

/// Bundle metadata (bundle.json)
#[derive(Debug, Serialize, Deserialize)]
pub struct BundleMetadata {
    pub name: String,
    pub version: String,
    pub git_sha: String,
    pub git_branch: String,
    pub content_hash: String,
    pub runner_channel: String,
    pub dependencies: Vec<String>,
    pub created_at: String,
    pub image_ref: Option<String>,
    pub cache_hit: bool,
}

/// Run bundle command
pub fn run(project: &str, output_dir: Option<&Path>, k8s: bool) -> Result<BundleResult> {
    use colored::Colorize;

    println!("{}", "==================================".bright_blue());
    println!("{}", "airis bundle".bright_blue().bold());
    println!("Project: {}", project.cyan());
    if k8s {
        println!("K8s:     {}", "enabled".green());
    }
    println!("{}", "==================================".bright_blue());

    let root = std::env::current_dir()?;

    // 1. Validate project exists
    let project_path = root.join(project);
    if !project_path.exists() {
        bail!("Project not found: {}", project);
    }

    // 2. Calculate content hash
    let hash = compute_content_hash(&root, project)?;
    println!("📋 Content hash: {}", hash.yellow());

    // 3. Check for cached build
    let cached = cache_hit(project, &hash);
    let cache_hit_status = cached.is_some();

    if cached.is_none() {
        println!("{}", "⚠️  No cached build found. Run 'airis build --docker' first.".yellow());
        bail!("No cached build for {}. Run: airis build --docker {}", project, project);
    }

    let cached = cached.unwrap();
    println!("✅ Found cached build: {}", cached.image_ref.green());

    // 4. Create output directory
    let dist_dir = output_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| root.join("dist"));
    let project_name = project.rsplit('/').next().unwrap_or(project);
    let bundle_dir = dist_dir.join(project_name);
    fs::create_dir_all(&bundle_dir)
        .with_context(|| format!("Failed to create bundle directory: {}", bundle_dir.display()))?;

    println!("📦 Bundle output: {}", bundle_dir.display().to_string().cyan());

    // 5. Generate bundle.json
    let metadata = generate_metadata(project, &hash, &cached.image_ref, cache_hit_status)?;
    let bundle_json_path = bundle_dir.join("bundle.json");
    let json_content = serde_json::to_string_pretty(&metadata)?;
    fs::write(&bundle_json_path, &json_content)?;
    println!("✅ Generated: bundle.json");

    // 6. Export Docker image (docker save)
    let image_tar_path = bundle_dir.join("image.tar");
    let image_tar = export_docker_image(&cached.image_ref, &image_tar_path)?;
    if image_tar.is_some() {
        let size = fs::metadata(&image_tar_path)?.len();
        println!("✅ Generated: image.tar ({})", format_size(size).dimmed());
    }

    // 7. Package build artifacts
    let artifact_tar_path = bundle_dir.join("artifact.tar.gz");
    let artifact_tar = package_artifacts(&root, project, &artifact_tar_path)?;
    if artifact_tar.is_some() {
        let size = fs::metadata(&artifact_tar_path)?.len();
        println!("✅ Generated: artifact.tar.gz ({})", format_size(size).dimmed());
    }

    // 8. Generate Kubernetes manifests (if --k8s flag)
    let k8s_dir = if k8s {
        let k8s_path = generate_k8s_manifests(&bundle_dir, project, &cached.image_ref)?;
        println!("✅ Generated: k8s/ (deployment.yaml, service.yaml)");
        Some(k8s_path)
    } else {
        None
    };

    // 9. Print summary
    println!();
    println!("{}", "==================================".bright_blue());
    println!("{}", "✅ Bundle complete!".green().bold());
    println!("   Output: {}", bundle_dir.display());
    println!("   Hash:   {}", hash);
    if k8s {
        println!("   K8s:    {}/k8s/", bundle_dir.display());
    }
    println!("{}", "==================================".bright_blue());

    Ok(BundleResult {
        output_dir: bundle_dir,
        bundle_json: bundle_json_path,
        image_tar,
        artifact_tar,
        k8s_dir,
    })
}

/// Generate bundle metadata
fn generate_metadata(
    project: &str,
    hash: &str,
    image_ref: &str,
    cache_hit: bool,
) -> Result<BundleMetadata> {
    // Get git info
    let git_sha = get_git_sha().unwrap_or_else(|| "unknown".to_string());
    let git_branch = get_git_branch().unwrap_or_else(|| "unknown".to_string());

    // Get version from manifest.toml
    let version = get_project_version(project).unwrap_or_else(|| "0.0.0".to_string());

    // Get runner channel from manifest.toml
    let runner_channel = get_runner_channel(project).unwrap_or_else(|| "lts".to_string());

    // Get dependencies (simplified - just list workspace deps)
    let dependencies = get_project_dependencies(project).unwrap_or_default();

    Ok(BundleMetadata {
        name: project.to_string(),
        version,
        git_sha,
        git_branch,
        content_hash: hash.to_string(),
        runner_channel,
        dependencies,
        created_at: chrono::Utc::now().to_rfc3339(),
        image_ref: Some(image_ref.to_string()),
        cache_hit,
    })
}

/// Export Docker image to tarball
fn export_docker_image(image_ref: &str, output_path: &Path) -> Result<Option<PathBuf>> {
    use colored::Colorize;

    println!("📤 Exporting Docker image...");

    let output = Command::new("docker")
        .args(["save", "-o", output_path.to_str().unwrap(), image_ref])
        .output()
        .context("Failed to run docker save")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("{}", format!("⚠️  docker save failed: {}", stderr).yellow());
        return Ok(None);
    }

    Ok(Some(output_path.to_path_buf()))
}

/// Package build artifacts to tar.gz
fn package_artifacts(root: &Path, project: &str, output_path: &Path) -> Result<Option<PathBuf>> {
    use colored::Colorize;

    let project_path = root.join(project);

    // Detect artifact directories based on project type
    let artifact_dirs = detect_artifact_dirs(&project_path);

    if artifact_dirs.is_empty() {
        println!("{}", "⚠️  No build artifacts found to package".yellow());
        return Ok(None);
    }

    println!("📦 Packaging artifacts: {:?}", artifact_dirs);

    // Create tar.gz using tar command
    let mut args = vec!["-czf".to_string(), output_path.to_str().unwrap().to_string()];

    for dir in &artifact_dirs {
        let rel_path = dir.strip_prefix(root).unwrap_or(dir);
        args.push("-C".to_string());
        args.push(root.to_str().unwrap().to_string());
        args.push(rel_path.to_str().unwrap().to_string());
    }

    let output = Command::new("tar")
        .args(&args)
        .output()
        .context("Failed to create tar.gz")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("{}", format!("⚠️  tar failed: {}", stderr).yellow());
        return Ok(None);
    }

    Ok(Some(output_path.to_path_buf()))
}

/// Detect artifact directories based on project type
fn detect_artifact_dirs(project_path: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // Next.js standalone
    let nextjs_standalone = project_path.join(".next").join("standalone");
    if nextjs_standalone.exists() {
        dirs.push(nextjs_standalone);
    }

    // Next.js static
    let nextjs_static = project_path.join(".next").join("static");
    if nextjs_static.exists() {
        dirs.push(nextjs_static);
    }

    // Generic dist
    let dist = project_path.join("dist");
    if dist.exists() {
        dirs.push(dist);
    }

    // Rust target/release
    let rust_release = project_path.join("target").join("release");
    if rust_release.exists() {
        dirs.push(rust_release);
    }

    // Python dist (wheel)
    let python_dist = project_path.join("dist");
    if python_dist.exists() && !dirs.contains(&python_dist) {
        dirs.push(python_dist);
    }

    // Public assets
    let public = project_path.join("public");
    if public.exists() {
        dirs.push(public);
    }

    dirs
}

// =============================================================================
// Helper functions
// =============================================================================

fn get_git_sha() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn get_git_branch() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn get_project_version(project: &str) -> Option<String> {
    // Try to read from manifest.toml
    let content = fs::read_to_string("manifest.toml").ok()?;
    let manifest: toml::Value = toml::from_str(&content).ok()?;

    let project_name = project.rsplit('/').next().unwrap_or(project);

    // Check [projects.<name>.version]
    manifest
        .get("projects")?
        .get(project_name)?
        .get("version")?
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| {
            // Fallback to workspace version
            manifest.get("workspace")?.get("version")?.as_str().map(|s| s.to_string())
        })
}

fn get_runner_channel(project: &str) -> Option<String> {
    let content = fs::read_to_string("manifest.toml").ok()?;
    let manifest: toml::Value = toml::from_str(&content).ok()?;

    let project_name = project.rsplit('/').next().unwrap_or(project);

    manifest
        .get("projects")?
        .get(project_name)?
        .get("runner")?
        .get("channel")?
        .as_str()
        .map(|s| s.to_string())
}

fn get_project_dependencies(project: &str) -> Option<Vec<String>> {
    // Read package.json dependencies
    let package_json_path = PathBuf::from(project).join("package.json");
    let content = fs::read_to_string(&package_json_path).ok()?;
    let pkg: serde_json::Value = serde_json::from_str(&content).ok()?;

    let mut deps = Vec::new();

    if let Some(dependencies) = pkg.get("dependencies").and_then(|d| d.as_object()) {
        for (name, _) in dependencies {
            if name.starts_with('@') && name.contains('/') {
                // Workspace dependency
                deps.push(name.clone());
            }
        }
    }

    Some(deps)
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Generate Kubernetes manifests (deployment.yaml, service.yaml)
fn generate_k8s_manifests(bundle_dir: &Path, project: &str, image_ref: &str) -> Result<PathBuf> {
    use crate::manifest::{K8sResources, Manifest, ResourceSpec};

    // Create k8s/ directory
    let k8s_dir = bundle_dir.join("k8s");
    fs::create_dir_all(&k8s_dir)
        .with_context(|| format!("Failed to create k8s directory: {}", k8s_dir.display()))?;

    // Load manifest.toml to get project config
    let manifest_content = fs::read_to_string("manifest.toml")
        .with_context(|| "Failed to read manifest.toml")?;
    let manifest: Manifest = toml::from_str(&manifest_content)
        .with_context(|| "Failed to parse manifest.toml")?;

    // Find project config in [[app]] array
    let project_name = project.rsplit('/').next().unwrap_or(project);
    let app_config = manifest.app.iter().find(|app| {
        app.name == project_name || app.name == project || app.path.as_deref() == Some(project)
    });

    // Get K8s configuration with defaults
    let port = app_config.and_then(|c| c.port).unwrap_or(3000);
    let replicas = app_config.and_then(|c| c.replicas).unwrap_or(1);
    let resources = app_config
        .and_then(|c| c.resources.clone())
        .unwrap_or_else(|| K8sResources {
            requests: Some(ResourceSpec {
                cpu: Some("100m".to_string()),
                memory: Some("128Mi".to_string()),
            }),
            limits: Some(ResourceSpec {
                cpu: Some("500m".to_string()),
                memory: Some("512Mi".to_string()),
            }),
        });

    // Generate deployment.yaml
    let deployment = generate_deployment_yaml(project_name, image_ref, port, replicas, &resources);
    let deployment_path = k8s_dir.join("deployment.yaml");
    fs::write(&deployment_path, &deployment)
        .with_context(|| "Failed to write deployment.yaml")?;

    // Generate service.yaml
    let service = generate_service_yaml(project_name, port);
    let service_path = k8s_dir.join("service.yaml");
    fs::write(&service_path, &service)
        .with_context(|| "Failed to write service.yaml")?;

    Ok(k8s_dir)
}

/// Generate Kubernetes Deployment YAML
fn generate_deployment_yaml(
    name: &str,
    image: &str,
    port: u16,
    replicas: u32,
    resources: &crate::manifest::K8sResources,
) -> String {
    let requests = resources.requests.as_ref();
    let limits = resources.limits.as_ref();

    let requests_cpu = requests.and_then(|r| r.cpu.as_deref()).unwrap_or("100m");
    let requests_memory = requests.and_then(|r| r.memory.as_deref()).unwrap_or("128Mi");
    let limits_cpu = limits.and_then(|l| l.cpu.as_deref()).unwrap_or("500m");
    let limits_memory = limits.and_then(|l| l.memory.as_deref()).unwrap_or("512Mi");

    format!(
        r#"# Generated by airis bundle --k8s
# DO NOT EDIT - Regenerate with: airis bundle {name} --k8s
apiVersion: apps/v1
kind: Deployment
metadata:
  name: {name}
  labels:
    app: {name}
spec:
  replicas: {replicas}
  selector:
    matchLabels:
      app: {name}
  template:
    metadata:
      labels:
        app: {name}
    spec:
      containers:
      - name: {name}
        image: {image}
        ports:
        - containerPort: {port}
        resources:
          requests:
            cpu: "{requests_cpu}"
            memory: "{requests_memory}"
          limits:
            cpu: "{limits_cpu}"
            memory: "{limits_memory}"
"#
    )
}

/// Generate Kubernetes Service YAML
fn generate_service_yaml(name: &str, port: u16) -> String {
    format!(
        r#"# Generated by airis bundle --k8s
# DO NOT EDIT - Regenerate with: airis bundle {name} --k8s
apiVersion: v1
kind: Service
metadata:
  name: {name}
spec:
  selector:
    app: {name}
  ports:
  - port: {port}
    targetPort: {port}
  type: ClusterIP
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_detect_artifact_dirs_empty() {
        let temp = tempfile::tempdir().unwrap();
        let dirs = detect_artifact_dirs(temp.path());
        assert!(dirs.is_empty());
    }

    #[test]
    fn test_detect_artifact_dirs_with_dist() {
        let temp = tempfile::tempdir().unwrap();
        let dist = temp.path().join("dist");
        std::fs::create_dir(&dist).unwrap();

        let dirs = detect_artifact_dirs(temp.path());
        assert_eq!(dirs.len(), 1);
        assert!(dirs[0].ends_with("dist"));
    }

    #[test]
    fn test_bundle_metadata_serialization() {
        let metadata = BundleMetadata {
            name: "apps/web".to_string(),
            version: "1.0.0".to_string(),
            git_sha: "abc123".to_string(),
            git_branch: "main".to_string(),
            content_hash: "hash123".to_string(),
            runner_channel: "lts".to_string(),
            dependencies: vec!["@workspace/ui".to_string()],
            created_at: "2025-01-01T00:00:00Z".to_string(),
            image_ref: Some("app:latest".to_string()),
            cache_hit: true,
        };

        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("apps/web"));
        assert!(json.contains("1.0.0"));
        assert!(json.contains("abc123"));
    }

    #[test]
    fn test_generate_deployment_yaml() {
        use crate::manifest::{K8sResources, ResourceSpec};

        let resources = K8sResources {
            requests: Some(ResourceSpec {
                cpu: Some("200m".to_string()),
                memory: Some("256Mi".to_string()),
            }),
            limits: Some(ResourceSpec {
                cpu: Some("1000m".to_string()),
                memory: Some("1Gi".to_string()),
            }),
        };

        let yaml = generate_deployment_yaml("api", "myapp:v1.0.0", 8080, 3, &resources);

        assert!(yaml.contains("kind: Deployment"));
        assert!(yaml.contains("name: api"));
        assert!(yaml.contains("image: myapp:v1.0.0"));
        assert!(yaml.contains("containerPort: 8080"));
        assert!(yaml.contains("replicas: 3"));
        assert!(yaml.contains("cpu: \"200m\""));
        assert!(yaml.contains("memory: \"256Mi\""));
        assert!(yaml.contains("cpu: \"1000m\""));
        assert!(yaml.contains("memory: \"1Gi\""));
    }

    #[test]
    fn test_generate_deployment_yaml_defaults() {
        use crate::manifest::K8sResources;

        let resources = K8sResources {
            requests: None,
            limits: None,
        };

        let yaml = generate_deployment_yaml("web", "app:latest", 3000, 1, &resources);

        // Should use default values
        assert!(yaml.contains("cpu: \"100m\""));
        assert!(yaml.contains("memory: \"128Mi\""));
        assert!(yaml.contains("cpu: \"500m\""));
        assert!(yaml.contains("memory: \"512Mi\""));
    }

    #[test]
    fn test_generate_service_yaml() {
        let yaml = generate_service_yaml("api", 8080);

        assert!(yaml.contains("kind: Service"));
        assert!(yaml.contains("name: api"));
        assert!(yaml.contains("port: 8080"));
        assert!(yaml.contains("targetPort: 8080"));
        assert!(yaml.contains("type: ClusterIP"));
    }

    #[test]
    fn test_generate_service_yaml_default_port() {
        let yaml = generate_service_yaml("web", 3000);

        assert!(yaml.contains("port: 3000"));
        assert!(yaml.contains("targetPort: 3000"));
    }
}
