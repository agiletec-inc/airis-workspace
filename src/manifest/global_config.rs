use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::schema::schema_default_version;

/// Claude Code global config section
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GlobalClaudeSection {
    #[serde(default = "default_claude_source")]
    pub source: String,
}

impl Default for GlobalClaudeSection {
    fn default() -> Self {
        GlobalClaudeSection {
            source: default_claude_source(),
        }
    }
}

fn default_claude_source() -> String {
    "~/.airis/claude".to_string()
}

/// Strategy for backing up files before modification
#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum BackupStrategy {
    /// Never backup, just overwrite (hard mode)
    #[default]
    None,
    /// Backup to .airis/backups/ (legacy mode)
    Backup,
    /// Check if git is clean before overwriting, fail/warn if dirty
    GitCheck,
}

/// Global configuration stored in ~/.airis/global-config.toml
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GlobalConfig {
    #[serde(default = "schema_default_version")]
    pub version: u32,
    #[serde(default)]
    pub claude: GlobalClaudeSection,
    /// Strategy for backups during 'airis gen'
    #[serde(default)]
    pub backup_strategy: BackupStrategy,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        GlobalConfig {
            version: 1,
            claude: GlobalClaudeSection::default(),
            backup_strategy: BackupStrategy::default(),
        }
    }
}

impl GlobalConfig {
    pub fn config_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        Ok(home.join(".airis").join("global-config.toml"))
    }

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

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(&config_path, content)?;
        Ok(())
    }
}
