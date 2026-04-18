use std::fs;
use std::path::PathBuf;
use std::collections::HashMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::schema::schema_default_version;

/// Guard intensity level
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GuardLevel {
    /// No guard, execute original command directly
    Off,
    /// Warn the user but proceed with original command
    Warn,
    /// Warn and block if inside an airis project or specific conditions met
    Enforce,
}

/// Predefined guard presets
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum GuardPreset {
    /// Package managers guarded, Docker untouched. Recommended for most.
    Balanced,
    /// Everything guarded strictly. Best for maximum AI protection.
    Strict,
    /// Warning only, never blocks. Best for existing workflows.
    Permissive,
}

impl Default for GuardPreset {
    fn default() -> Self {
        GuardPreset::Balanced
    }
}

/// Global guards configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GlobalGuardsSection {
    /// Selected preset for guard behavior
    #[serde(default)]
    pub preset: GuardPreset,

    /// Individual command overrides (command_name -> level)
    #[serde(default)]
    pub overrides: HashMap<String, GuardLevel>,
}

impl Default for GlobalGuardsSection {
    fn default() -> Self {
        GlobalGuardsSection {
            preset: GuardPreset::default(),
            overrides: HashMap::new(),
        }
    }
}

impl GlobalGuardsSection {
    /// Get the resolved level for a specific command based on preset and overrides
    pub fn get_level(&self, cmd: &str) -> GuardLevel {
        // 1. Check explicit overrides first
        if let Some(level) = self.overrides.get(cmd) {
            return level.clone();
        }

        // 2. Apply preset logic
        match self.preset {
            GuardPreset::Balanced => {
                match cmd {
                    "npm" | "pnpm" | "yarn" | "bun" | "pip" | "pip3" | "poetry" | "npx" => GuardLevel::Enforce,
                    "docker" | "docker-compose" => GuardLevel::Off,
                    _ => GuardLevel::Off,
                }
            }
            GuardPreset::Strict => GuardLevel::Enforce,
            GuardPreset::Permissive => {
                match cmd {
                    "npm" | "pnpm" | "yarn" | "bun" | "pip" | "pip3" | "poetry" | "npx" | "docker" | "docker-compose" => GuardLevel::Warn,
                    _ => GuardLevel::Off,
                }
            }
        }
    }

    /// List all commands that should have a wrapper script based on the preset
    pub fn active_commands(&self) -> Vec<String> {
        let base_commands = vec![
            "npm", "pnpm", "yarn", "bun", "npx",
            "pip", "pip3", "poetry",
            "docker", "docker-compose",
        ];
        
        base_commands
            .into_iter()
            .filter(|cmd| self.get_level(cmd) != GuardLevel::Off)
            .map(|s| s.to_string())
            .collect()
    }
}

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

/// Global configuration stored in ~/.airis/global-config.toml
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GlobalConfig {
    #[serde(default = "schema_default_version")]
    pub version: u32,
    #[serde(default)]
    pub guards: GlobalGuardsSection,
    #[serde(default)]
    pub claude: GlobalClaudeSection,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        GlobalConfig {
            version: 1,
            guards: GlobalGuardsSection::default(),
            claude: GlobalClaudeSection::default(),
        }
    }
}

impl GlobalConfig {
    pub fn config_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        Ok(home.join(".airis").join("global-config.toml"))
    }

    pub fn bin_dir() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        Ok(home.join(".airis").join("bin"))
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
