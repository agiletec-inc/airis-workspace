use std::fs;

use serde_json::json;

use super::dir_sync::{
    load_claude_registry, save_claude_registry, sync_from_source, sync_managed_dir,
    sync_single_file,
};
use super::templates::{self, ManagedDir, TemplateFile};
use super::{check_plugin_installed, is_legacy_airis_entry, remove_legacy_airis_entries};

#[test]
fn test_sync_managed_dir_creates_files() {
    let dir = tempfile::tempdir().unwrap();
    let files: &'static [TemplateFile] = Box::leak(Box::new([
        TemplateFile {
            rel_path: "a.md",
            content: "content a",
        },
        TemplateFile {
            rel_path: "b.md",
            content: "content b",
        },
    ]));
    let managed = ManagedDir {
        rel_dir: "rules",
        files,
    };
    let result = sync_managed_dir(dir.path(), &managed).unwrap();

    assert_eq!(result.written.len(), 2);
    assert_eq!(result.deleted.len(), 0);
    assert_eq!(result.unchanged.len(), 0);
    assert_eq!(
        fs::read_to_string(dir.path().join("rules/a.md")).unwrap(),
        "content a"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("rules/b.md")).unwrap(),
        "content b"
    );
}

#[test]
fn test_sync_managed_dir_deletes_orphans() {
    let dir = tempfile::tempdir().unwrap();
    let rules_dir = dir.path().join("rules");
    fs::create_dir_all(&rules_dir).unwrap();
    // Pre-existing files
    fs::write(rules_dir.join("old-rule.md"), "old content").unwrap();
    fs::write(rules_dir.join("a.md"), "stale content").unwrap();

    let files: &'static [TemplateFile] = Box::leak(Box::new([TemplateFile {
        rel_path: "a.md",
        content: "new content",
    }]));
    let managed = ManagedDir {
        rel_dir: "rules",
        files,
    };
    let result = sync_managed_dir(dir.path(), &managed).unwrap();

    assert_eq!(result.deleted.len(), 1);
    assert!(result.deleted[0].ends_with("old-rule.md"));
    assert!(!rules_dir.join("old-rule.md").exists());
    assert_eq!(result.written.len(), 1); // a.md updated
    assert_eq!(
        fs::read_to_string(rules_dir.join("a.md")).unwrap(),
        "new content"
    );
}

#[test]
fn test_sync_managed_dir_skips_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let rules_dir = dir.path().join("rules");
    fs::create_dir_all(&rules_dir).unwrap();
    fs::write(rules_dir.join("a.md"), "content a").unwrap();

    let files: &'static [TemplateFile] = Box::leak(Box::new([TemplateFile {
        rel_path: "a.md",
        content: "content a",
    }]));
    let managed = ManagedDir {
        rel_dir: "rules",
        files,
    };
    let result = sync_managed_dir(dir.path(), &managed).unwrap();

    assert_eq!(result.written.len(), 0);
    assert_eq!(result.unchanged.len(), 1);
    assert_eq!(result.deleted.len(), 0);
}

#[test]
fn test_sync_single_file_creates_and_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let file = TemplateFile {
        rel_path: "CLAUDE.md",
        content: "v1",
    };

    // Create
    assert!(sync_single_file(dir.path(), &file).unwrap());
    assert_eq!(
        fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap(),
        "v1"
    );

    // Update
    let file_v2 = TemplateFile {
        rel_path: "CLAUDE.md",
        content: "v2",
    };
    assert!(sync_single_file(dir.path(), &file_v2).unwrap());
    assert_eq!(
        fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap(),
        "v2"
    );

    // Unchanged
    assert!(!sync_single_file(dir.path(), &file_v2).unwrap());
}

#[test]
fn test_check_plugin_installed_detects_presence() {
    let dir = tempfile::tempdir().unwrap();
    let plugins_dir = dir.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    // No file → not installed
    assert!(!check_plugin_installed(dir.path()).unwrap());

    // Empty plugins → not installed
    let plugins = json!({
        "version": 2,
        "plugins": {}
    });
    fs::write(
        plugins_dir.join("installed_plugins.json"),
        serde_json::to_string_pretty(&plugins).unwrap(),
    )
    .unwrap();
    assert!(!check_plugin_installed(dir.path()).unwrap());

    // With airis plugin → installed
    let plugins = json!({
        "version": 2,
        "plugins": {
            "airis-mcp-gateway@airis-mcp-gateway": [{
                "scope": "user",
                "version": "0.2.0"
            }]
        }
    });
    fs::write(
        plugins_dir.join("installed_plugins.json"),
        serde_json::to_string_pretty(&plugins).unwrap(),
    )
    .unwrap();
    assert!(check_plugin_installed(dir.path()).unwrap());
}

#[test]
fn test_is_legacy_airis_entry() {
    let legacy = json!({
        "matcher": "Bash",
        "hooks": [{
            "type": "command",
            "command": "bash ~/.claude/hooks/airis/docker-first-guard.sh"
        }]
    });
    assert!(is_legacy_airis_entry(&legacy));

    let plugin = json!({
        "matcher": "Bash",
        "hooks": [{
            "type": "command",
            "command": "bash \"${CLAUDE_PLUGIN_ROOT}/hooks/docker-first-guard.sh\""
        }]
    });
    assert!(!is_legacy_airis_entry(&plugin));

    let user = json!({
        "matcher": "Bash",
        "hooks": [{
            "type": "command",
            "command": "my-custom-hook.sh"
        }]
    });
    assert!(!is_legacy_airis_entry(&user));
}

#[test]
fn test_remove_legacy_airis_entries_preserves_others() {
    let mut settings = json!({
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": "Bash",
                    "hooks": [{"type": "command", "command": "bash ~/.claude/hooks/airis/guard.sh"}]
                },
                {
                    "matcher": "Bash",
                    "hooks": [{"type": "command", "command": "my-hook.sh"}]
                }
            ],
            "Stop": [
                {
                    "matcher": "",
                    "hooks": [{"type": "command", "command": "bash ~/.claude/hooks/airis/stop.sh"}]
                }
            ]
        }
    });

    assert!(remove_legacy_airis_entries(&mut settings));

    let pre = settings["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(pre.len(), 1);
    assert!(pre[0]["hooks"][0]["command"]
        .as_str()
        .unwrap()
        .contains("my-hook.sh"));

    let stop = settings["hooks"]["Stop"].as_array().unwrap();
    assert_eq!(stop.len(), 0);
}

// ── Source-based sync tests ─────────────────────────────────────────

#[test]
fn test_sync_from_source_basic() {
    let source = tempfile::tempdir().unwrap();
    let claude = tempfile::tempdir().unwrap();
    let registry = tempfile::NamedTempFile::new().unwrap();

    // Create source files
    fs::create_dir_all(source.path().join("rules")).unwrap();
    fs::write(source.path().join("CLAUDE.md"), "global rules").unwrap();
    fs::write(source.path().join("rules/a.md"), "rule a").unwrap();

    let result = sync_from_source(source.path(), claude.path(), registry.path()).unwrap();

    assert_eq!(result.written.len(), 2);
    assert_eq!(result.deleted.len(), 0);
    assert_eq!(
        fs::read_to_string(claude.path().join("CLAUDE.md")).unwrap(),
        "global rules"
    );
    assert_eq!(
        fs::read_to_string(claude.path().join("rules/a.md")).unwrap(),
        "rule a"
    );

    // Registry should be saved
    let reg = load_claude_registry(registry.path());
    assert!(reg.contains(&"CLAUDE.md".to_string()));
    assert!(reg.contains(&"rules/a.md".to_string()));
}

#[test]
fn test_sync_from_source_orphan_removal() {
    let source = tempfile::tempdir().unwrap();
    let claude = tempfile::tempdir().unwrap();
    let registry = tempfile::NamedTempFile::new().unwrap();

    // First sync: two rule files
    fs::create_dir_all(source.path().join("rules")).unwrap();
    fs::write(source.path().join("rules/a.md"), "rule a").unwrap();
    fs::write(source.path().join("rules/b.md"), "rule b").unwrap();
    sync_from_source(source.path(), claude.path(), registry.path()).unwrap();

    assert!(claude.path().join("rules/b.md").exists());

    // Second sync: remove b.md from source
    fs::remove_file(source.path().join("rules/b.md")).unwrap();
    let result = sync_from_source(source.path(), claude.path(), registry.path()).unwrap();

    assert_eq!(result.deleted.len(), 1);
    assert!(result.deleted.contains(&"rules/b.md".to_string()));
    assert!(!claude.path().join("rules/b.md").exists());
    assert!(claude.path().join("rules/a.md").exists());
}

#[test]
fn test_sync_from_source_preserves_non_airis_files() {
    let source = tempfile::tempdir().unwrap();
    let claude = tempfile::tempdir().unwrap();
    let registry = tempfile::NamedTempFile::new().unwrap();

    // Place a non-airis file in ~/.claude/rules/
    fs::create_dir_all(claude.path().join("rules")).unwrap();
    fs::write(claude.path().join("rules/user-custom.md"), "user rule").unwrap();

    // Sync airis rules
    fs::create_dir_all(source.path().join("rules")).unwrap();
    fs::write(source.path().join("rules/a.md"), "rule a").unwrap();
    let result = sync_from_source(source.path(), claude.path(), registry.path()).unwrap();

    // user-custom.md must NOT be deleted (not in registry)
    assert!(claude.path().join("rules/user-custom.md").exists());
    assert_eq!(result.deleted.len(), 0);
    assert_eq!(result.written.len(), 1);
}

#[test]
fn test_initialize_source_dir() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("claude");

    templates::initialize_source_dir(&source).unwrap();

    assert!(source.join("CLAUDE.md").exists());
    assert!(source.join("rules/docker-first.md").exists());
    assert!(source.join("rules/server-access.md").exists());

    // Content should match embedded templates
    let expected = templates::global_claude_md().content;
    assert_eq!(fs::read_to_string(source.join("CLAUDE.md")).unwrap(), expected);
}

#[test]
fn test_registry_load_save_roundtrip() {
    let file = tempfile::NamedTempFile::new().unwrap();

    let paths = vec![
        "CLAUDE.md".to_string(),
        "rules/docker-first.md".to_string(),
        "rules/server-access.md".to_string(),
    ];
    save_claude_registry(file.path(), &paths).unwrap();

    let loaded = load_claude_registry(file.path());
    assert_eq!(loaded.len(), 3);
    assert!(loaded.contains(&"CLAUDE.md".to_string()));
    assert!(loaded.contains(&"rules/docker-first.md".to_string()));
    assert!(loaded.contains(&"rules/server-access.md".to_string()));
}
