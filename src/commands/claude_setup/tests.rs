use serde_json::json;

use super::hooks_merge::{is_airis_entry, merge_airis_hooks, remove_airis_hooks};
use super::scripts::{docker_first_edit_guard_script, docker_first_guard_script};

#[test]
fn test_merge_hooks_empty() {
    let mut settings = json!({});
    merge_airis_hooks(&mut settings).unwrap();

    let pre = settings["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(pre.len(), 2); // Bash guard + Edit guard
    assert!(is_airis_entry(&pre[0]));
    assert!(is_airis_entry(&pre[1]));

    let stop = settings["hooks"]["Stop"].as_array().unwrap();
    assert_eq!(stop.len(), 1);
    assert!(is_airis_entry(&stop[0]));
}

#[test]
fn test_merge_hooks_preserves_existing() {
    let user_hook = json!({
        "matcher": "Bash",
        "hooks": [{"type": "command", "command": "my-custom-hook.sh"}]
    });
    let mut settings = json!({
        "hooks": {
            "PreToolUse": [user_hook.clone()],
            "Stop": [user_hook.clone()]
        }
    });

    merge_airis_hooks(&mut settings).unwrap();

    // PreToolUse: airis entries first (Bash guard + Edit guard), then user
    let pre = settings["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(pre.len(), 3);
    assert!(is_airis_entry(&pre[0])); // Bash guard prepended
    assert!(is_airis_entry(&pre[1])); // Edit guard prepended
    assert!(!is_airis_entry(&pre[2])); // user preserved

    // Stop: user first, then airis
    let stop = settings["hooks"]["Stop"].as_array().unwrap();
    assert_eq!(stop.len(), 2);
    assert!(!is_airis_entry(&stop[0])); // user preserved
    assert!(is_airis_entry(&stop[1])); // airis appended
}

#[test]
fn test_merge_hooks_replaces_airis() {
    let mut settings = json!({});
    merge_airis_hooks(&mut settings).unwrap();
    // Run again — should not duplicate
    merge_airis_hooks(&mut settings).unwrap();

    let pre = settings["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(pre.len(), 2); // Still just Bash guard + Edit guard

    let stop = settings["hooks"]["Stop"].as_array().unwrap();
    assert_eq!(stop.len(), 1);
}

#[test]
fn test_generate_guard_script() {
    let script = docker_first_guard_script();
    assert!(script.contains("BLOCKED=("));
    assert!(script.contains("npm (install|i |ci|add|update|remove|uninstall)"));
    assert!(script.contains("pip3? install"));
    assert!(script.contains("brew install"));
    assert!(script.contains("exit 2"));
}

#[test]
fn test_generate_edit_guard_script() {
    let script = docker_first_edit_guard_script();
    assert!(script.contains("/Users/"));
    assert!(script.contains("PNPM_STORE_DIR"));
    assert!(script.contains("node_modules"));
    assert!(script.contains("exit 2"));
    // Should only check Docker/CI files (allowlist approach)
    assert!(script.contains("Dockerfile"));
    // Should check existing file content too
    assert!(script.contains("existing file"));
}

#[test]
fn test_uninstall_removes_airis_only() {
    let user_hook = json!({
        "matcher": "Bash",
        "hooks": [{"type": "command", "command": "my-custom-hook.sh"}]
    });
    let mut settings = json!({});
    merge_airis_hooks(&mut settings).unwrap();

    // Add a user hook
    settings["hooks"]["PreToolUse"]
        .as_array_mut()
        .unwrap()
        .push(user_hook.clone());

    // Uninstall
    remove_airis_hooks(&mut settings);

    let pre = settings["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(pre.len(), 1);
    assert!(!is_airis_entry(&pre[0])); // only user hook remains
}
