mod doppler;

use anyhow::{Result, bail};

use crate::manifest::SecretsSection;

/// Trait for secret providers that wrap commands with env injection.
pub trait SecretProvider {
    /// Wrap a command so that secrets are injected as environment variables.
    /// Returns the full argument list to pass to `Command::new`.
    fn wrap_command(&self, program: &str, args: &[&str]) -> (String, Vec<String>);

    /// Check if the provider CLI tool is available on the system.
    fn is_available(&self) -> bool;

    /// Provider name for display/logging.
    fn name(&self) -> &str;
}

/// Create a secret provider from the manifest [secrets] section.
pub fn create_provider(secrets: &SecretsSection) -> Result<Box<dyn SecretProvider>> {
    match secrets.provider.as_str() {
        "doppler" => {
            let cfg = secrets.doppler.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "[secrets] provider is 'doppler' but [secrets.doppler] section is missing"
                )
            })?;
            Ok(Box::new(doppler::DopplerProvider {
                project: cfg.project.clone(),
                config: cfg.config.clone(),
            }))
        }
        other => bail!("Unknown secrets provider: '{}'. Supported: doppler", other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{DopplerSecretsConfig, SecretsSection};

    fn doppler_secrets(project: &str, config: &str) -> SecretsSection {
        SecretsSection {
            provider: "doppler".into(),
            doppler: Some(DopplerSecretsConfig {
                project: project.into(),
                config: config.into(),
            }),
        }
    }

    #[test]
    fn create_provider_doppler_returns_doppler() {
        let s = doppler_secrets("my-project", "dev");
        let p = create_provider(&s).unwrap();
        assert_eq!(p.name(), "doppler");
    }

    #[test]
    fn create_provider_unknown_returns_error() {
        let s = SecretsSection {
            provider: "vault".into(),
            doppler: None,
        };
        let msg = match create_provider(&s) {
            Ok(_) => panic!("expected error for unknown provider"),
            Err(e) => e.to_string(),
        };
        assert!(
            msg.contains("vault") && msg.contains("Supported"),
            "got: {msg}"
        );
    }

    #[test]
    fn create_provider_doppler_missing_section_returns_error() {
        let s = SecretsSection {
            provider: "doppler".into(),
            doppler: None,
        };
        let msg = match create_provider(&s) {
            Ok(_) => panic!("expected error for missing doppler section"),
            Err(e) => e.to_string(),
        };
        assert!(msg.contains("[secrets.doppler]"), "got: {msg}");
    }
}
