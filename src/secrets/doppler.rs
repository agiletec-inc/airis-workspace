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

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> DopplerProvider {
        DopplerProvider {
            project: "my-project".into(),
            config: "dev".into(),
        }
    }

    #[test]
    fn wrap_command_uses_doppler_as_program() {
        let (program, _) = provider().wrap_command("docker", &["compose", "up"]);
        assert_eq!(program, "doppler");
    }

    #[test]
    fn wrap_command_passes_args_as_list_not_shell_string() {
        // Security property: each element must be a separate list entry.
        // If args were concatenated into a shell string, a value like
        // "up; rm -rf /" would execute an injected command.
        let (_, args) = provider().wrap_command("docker", &["compose", "up; rm -rf /"]);
        // The injected string must appear verbatim as one element, not split.
        assert!(
            args.iter().any(|a| a == "up; rm -rf /"),
            "argument must be a single list element, not interpreted by a shell"
        );
        // No element should be just "rm" or "rf"
        assert!(!args.iter().any(|a| a == "rm"), "shell must not split args");
    }

    #[test]
    fn wrap_command_structure_is_correct() {
        let (_, args) = provider().wrap_command("docker", &["compose", "up", "-d"]);
        // Expected: run --project my-project --config dev -- docker compose up -d
        assert_eq!(args[0], "run");
        assert_eq!(args[1], "--project");
        assert_eq!(args[2], "my-project");
        assert_eq!(args[3], "--config");
        assert_eq!(args[4], "dev");
        assert_eq!(args[5], "--");
        assert_eq!(args[6], "docker");
        assert_eq!(args[7], "compose");
        assert_eq!(args[8], "up");
        assert_eq!(args[9], "-d");
    }

    #[test]
    fn wrap_command_with_no_extra_args() {
        let (program, args) = provider().wrap_command("docker", &[]);
        assert_eq!(program, "doppler");
        assert_eq!(
            args,
            [
                "run",
                "--project",
                "my-project",
                "--config",
                "dev",
                "--",
                "docker"
            ]
        );
    }

    #[test]
    fn name_returns_doppler() {
        assert_eq!(provider().name(), "doppler");
    }

    #[test]
    fn project_and_config_values_are_preserved() {
        let p = DopplerProvider {
            project: "prod-app".into(),
            config: "prd".into(),
        };
        let (_, args) = p.wrap_command("sh", &[]);
        let proj_pos = args.iter().position(|a| a == "--project").unwrap();
        let cfg_pos = args.iter().position(|a| a == "--config").unwrap();
        assert_eq!(args[proj_pos + 1], "prod-app");
        assert_eq!(args[cfg_pos + 1], "prd");
    }
}
