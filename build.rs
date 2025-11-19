use std::process::Command;

fn main() {
    // Tell Cargo to rerun if git HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");
    println!("cargo:rerun-if-changed=.git/refs/tags");

    // Get git describe info
    let git_describe = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Get short commit hash
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Check if current commit is exactly a tag (release build)
    let is_release = Command::new("git")
        .args(["describe", "--tags", "--exact-match"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    // Export to Rust
    println!("cargo:rustc-env=GIT_DESCRIBE={}", git_describe);
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);
    println!("cargo:rustc-env=IS_RELEASE={}", is_release);
}
