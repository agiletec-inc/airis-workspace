use super::*;

use std::fs;

use crate::manifest::{GlobalConfig, GuardLevel};

#[test]
fn test_global_config_default() {
    let config = GlobalConfig::default();
    assert_eq!(config.version, 1);

    // Test through get_level instead of direct field access
    assert_eq!(config.guards.get_level("npm"), GuardLevel::Enforce);
    assert_eq!(config.guards.get_level("yarn"), GuardLevel::Enforce);
    assert_eq!(config.guards.get_level("pnpm"), GuardLevel::Enforce);
    assert_eq!(config.guards.get_level("bun"), GuardLevel::Enforce);
    assert_eq!(config.guards.get_level("npx"), GuardLevel::Enforce);
}

#[test]
fn test_global_config_paths() {
    let config_path = GlobalConfig::config_path();
    assert!(config_path.is_ok());

    let bin_dir = GlobalConfig::bin_dir();
    assert!(bin_dir.is_ok());

    let config_path = config_path.unwrap();
    assert!(config_path.to_string_lossy().contains(".airis"));
    assert!(config_path.to_string_lossy().contains("global-config.toml"));

    let bin_dir = bin_dir.unwrap();
    assert!(bin_dir.to_string_lossy().contains(".airis"));
    assert!(bin_dir.to_string_lossy().ends_with("bin"));
}

#[test]
fn test_global_config_serialization() {
    let config = GlobalConfig::default();
    let toml_str = toml::to_string_pretty(&config).unwrap();

    let parsed: GlobalConfig = toml::from_str(&toml_str).unwrap();
    assert_eq!(parsed.version, config.version);
}
