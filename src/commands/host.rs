use anyhow::{Result, bail};
use std::env;
use std::os::unix::process::CommandExt;
use std::process::Command;

/// Run a host command bypassing airis guards (`AIRIS_BYPASS=1`).
pub fn run(cmd: &[String]) -> Result<()> {
    let (name, args) = cmd
        .split_first()
        .expect("cmd is non-empty (enforced by clap)");

    // Find the real binary, excluding ~/.airis/bin so we don't re-enter the guard.
    let real_path = env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .filter(|p| !p.contains(".airis/bin"))
        .find_map(|p| {
            let candidate = std::path::Path::new(p).join(name);
            candidate.is_file().then_some(candidate)
        });

    let Some(binary) = real_path else {
        bail!("'{}' not found on host PATH (excluding ~/.airis/bin)", name);
    };

    let err = Command::new(&binary)
        .args(args)
        .env("AIRIS_BYPASS", "1")
        .exec();

    bail!("failed to exec {}: {}", binary.display(), err);
}
