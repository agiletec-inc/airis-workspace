use anyhow::{Result, bail};
use std::env;
use std::process::Command;

/// Run a host command bypassing airis guards (`AIRIS_BYPASS=1`).
pub fn run(cmd: &[String]) -> Result<()> {
    let (name, args) = cmd
        .split_first()
        .expect("cmd is non-empty (enforced by clap)");

    // Find the real binary, excluding ~/.airis/bin so we don't re-enter the
    // guard. `split_paths` applies the platform-correct PATH separator.
    let path_var = env::var_os("PATH").unwrap_or_default();
    let real_path = env::split_paths(&path_var)
        .filter(|p| !p.to_string_lossy().contains(".airis/bin"))
        .find_map(|p| {
            let candidate = p.join(name);
            candidate.is_file().then_some(candidate)
        });

    let Some(binary) = real_path else {
        bail!("'{}' not found on host PATH (excluding ~/.airis/bin)", name);
    };

    let mut command = Command::new(&binary);
    command.args(args).env("AIRIS_BYPASS", "1");

    // On Unix, replace the current process via exec(2) so signals and the exit
    // code pass through transparently. Windows has no exec(2): spawn the child,
    // wait for it, and propagate its exit code instead.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = command.exec();
        bail!("failed to exec {}: {}", binary.display(), err);
    }

    #[cfg(not(unix))]
    {
        use anyhow::Context;
        let status = command
            .status()
            .with_context(|| format!("failed to run {}", binary.display()))?;
        std::process::exit(status.code().unwrap_or(1));
    }
}
