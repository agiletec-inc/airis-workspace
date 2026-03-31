//! Tests for the bundle command

use super::artifacts::{detect_artifact_dirs, format_size};
use super::k8s::{generate_deployment_yaml, generate_service_yaml};
use super::metadata::BundleMetadata;

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
