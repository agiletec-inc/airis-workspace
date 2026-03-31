use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::schema::schema_default_version;

/// Global guards section for ~/.airis/global-config.toml
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GlobalGuardsSection {
    /// Commands to block outside of airis projects
    #[serde(default = "default_global_deny")]
    pub deny: Vec<String>,
}

impl Default for GlobalGuardsSection {
    fn default() -> Self {
        GlobalGuardsSection {
            deny: default_global_deny(),
        }
    }
}

fn default_global_deny() -> Vec<String> {
    vec![
        "npm".to_string(),
        "yarn".to_string(),
        "pnpm".to_string(),
        "bun".to_string(),
        "npx".to_string(),
    ]
}

/// Global configuration stored in ~/.airis/global-config.toml
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GlobalConfig {
    #[serde(default = "schema_default_version")]
    pub version: u32,
    #[serde(default)]
    pub guards: GlobalGuardsSection,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        GlobalConfig {
            version: 1,
            guards: GlobalGuardsSection::default(),
        }
    }
}

impl GlobalConfig {
    /// Get the path to the global config file (~/.airis/global-config.toml)
    pub fn config_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        Ok(home.join(".airis").join("global-config.toml"))
    }

    /// Get the path to the global bin directory (~/.airis/bin)
    pub fn bin_dir() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        Ok(home.join(".airis").join("bin"))
    }

    /// Load global config from ~/.airis/global-config.toml
    /// Returns default config if file doesn't exist
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read {:?}", config_path))?;

        let config: GlobalConfig =
            toml::from_str(&content).with_context(|| "Failed to parse global-config.toml")?;

        Ok(config)
    }

    /// Save global config to ~/.airis/global-config.toml
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        // Create parent directory if needed
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("Failed to create {:?}", parent))?;
        }

        let content = toml::to_string_pretty(self)
            .with_context(|| "Failed to serialize global-config.toml")?;

        fs::write(&config_path, content)
            .with_context(|| format!("Failed to write {:?}", config_path))?;

        Ok(())
    }
}
