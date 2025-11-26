use anyhow::Result;
use colored::Colorize;
use std::path::Path;

use crate::manifest::{MANIFEST_FILE, Manifest};

pub enum ManifestAction {
    DevApps,
    Rule { name: String },
}

pub fn run(action: ManifestAction) -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);
    if !manifest_path.exists() {
        anyhow::bail!("❌ manifest.toml not found. Run `airis init` to create it first.");
    }

    let manifest = Manifest::load(manifest_path)?;

    match action {
        ManifestAction::DevApps => {
            // Print apps_pattern (glob pattern for auto-discovery)
            println!("{}", manifest.dev.apps_pattern);
        }
        ManifestAction::Rule { name } => {
            let Some(rule) = manifest.rule.get(&name) else {
                anyhow::bail!(
                    "{} `{}` is not defined inside [rule] section of manifest.toml",
                    "❌ Rule".bright_red(),
                    name
                );
            };

            if rule.commands.is_empty() {
                println!(
                    "{} Rule `{}` has no commands configured in manifest.toml",
                    "⚠️".yellow(),
                    name
                );
                return Ok(());
            }

            for command in &rule.commands {
                println!("{command}");
            }
        }
    }

    Ok(())
}
