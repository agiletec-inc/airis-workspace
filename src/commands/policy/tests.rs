//! Tests for the policy command

use super::checkers::{check_forbidden_files, check_required_env};
use super::*;

#[test]
fn test_policy_config_default() {
    let config = PolicyConfig::default();
    assert!(!config.gates.require_clean_git);
    assert!(config.gates.require_env.is_empty());
    assert!(config.gates.forbid_files.is_empty());
}

#[test]
fn test_policy_template() {
    let template = PolicyConfig::template();
    assert!(template.contains("[gates]"));
    assert!(template.contains("require_clean_git"));
    assert!(template.contains("[security]"));
    assert!(template.contains("scan_secrets"));
}

#[test]
fn test_forbidden_files_check() {
    let temp = tempfile::tempdir().unwrap();
    let forbidden_file = temp.path().join(".env.local");
    std::fs::write(&forbidden_file, "SECRET=123").unwrap();

    let mut result = PolicyResult::default();
    result.passed = true;

    // Use absolute path to avoid thread-safety issues with set_current_dir
    let abs_path = forbidden_file.to_string_lossy().to_string();
    check_forbidden_files(&[abs_path.clone()], &mut result).unwrap();

    assert!(!result.violations.is_empty());
    assert!(result.violations[0].message.contains(".env.local"));
}

#[test]
fn test_required_env_missing() {
    let mut result = PolicyResult::default();
    result.passed = true;

    check_required_env(&["DEFINITELY_NOT_SET_12345".to_string()], &mut result);

    assert!(!result.violations.is_empty());
    assert!(
        result.violations[0]
            .message
            .contains("DEFINITELY_NOT_SET_12345")
    );
}

#[test]
fn test_severity_enum() {
    assert_eq!(Severity::Error, Severity::Error);
    assert_ne!(Severity::Error, Severity::Warning);
}
