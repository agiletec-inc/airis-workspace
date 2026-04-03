use std::process::Command;

use super::SecretProvider;

pub struct DopplerProvider {
    pub project: String,
    pub config: String,
}

impl SecretProvider for DopplerProvider {
    fn wrap_command(&self, program: &str, args: &[&str]) -> (String, Vec<String>) {
        let mut wrapped_args = vec![
            "run".to_string(),
            "--project".to_string(),
            self.project.clone(),
            "--config".to_string(),
            self.config.clone(),
            "--".to_string(),
            program.to_string(),
        ];
        wrapped_args.extend(args.iter().map(|s| s.to_string()));
        ("doppler".to_string(), wrapped_args)
    }

    fn is_available(&self) -> bool {
        Command::new("doppler")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn name(&self) -> &str {
        "doppler"
    }
}
