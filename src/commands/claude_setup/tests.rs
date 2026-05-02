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
    assert!(
        pre[0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("my-hook.sh")
    );

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

    // Every rule listed in managed_dirs() must land on disk after init.
    // Adding a new rule to managed_dirs() without updating tests should
    // not be possible — this loop catches the omission automatically.
    assert!(source.join("CLAUDE.md").exists());
    for managed in templates::managed_dirs() {
        for file in managed.files {
            let path = source.join(managed.rel_dir).join(file.rel_path);
            assert!(path.exists(), "{} should exist after init", path.display());
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                file.content,
                "{} content should match embedded template",
                path.display()
            );
        }
    }

    // CLAUDE.md content matches the embedded template.
    let expected = templates::global_claude_md().content;
    assert_eq!(
        fs::read_to_string(source.join("CLAUDE.md")).unwrap(),
        expected
    );
}

#[test]
fn test_managed_dirs_includes_all_expected_rules() {
    // Hard list of rule files we expect after merging llm-rules into
    // airis-workspace. Touching managed_dirs() without updating this set
    // should fail loudly so we don't accidentally drop a rule.
    let expected: std::collections::HashSet<&str> = [
        "docker-first.md",
        "server-access.md",
        "orbstack.md",
        "planning.md",
        "bug-fix.md",
        "secrets-tier.md",
    ]
    .into_iter()
    .collect();

    let mut actual: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for managed in templates::managed_dirs() {
        if managed.rel_dir == "rules" {
            for file in managed.files {
                actual.insert(file.rel_path);
            }
        }
    }

    assert_eq!(
        actual, expected,
        "rules/ template set drifted from expected. \
         If you added a rule on purpose, update both managed_dirs() and this test."
    );
}

#[test]
fn test_no_infra_constants_in_templates() {
    // Templates ship in an MIT-licensed binary. Anything that leaks a
    // server hostname / IP / personal username pollutes downstream
    // installs and re-introduces the private-overlay problem the merge
    // was meant to eliminate. This test is the regression guard.
    let bad_substrings = [
        "100.82.",       // a real Tailscale CGNAT range we used to leak
        "kazuki@",       // personal SSH user
        "/Users/kazuki", // personal $HOME
        "RTX 5070",      // GPU model that pinned a specific machine
        "ssd-2tb",       // host-specific volume label
        "agile-server",  // single-machine hostname
        "video-restore", // project name unrelated to the rule
    ];

    let mut hits: Vec<String> = Vec::new();
    for managed in templates::managed_dirs() {
        for file in managed.files {
            for needle in bad_substrings {
                if file.content.contains(needle) {
                    // secrets-tier.md uses one of these as a *teaching example*
                    // of what NOT to do — explicit allow-list keeps the test
                    // honest without requiring renaming the example value.
                    if file.rel_path == "secrets-tier.md" && needle == "192.0.2.42" {
                        continue;
                    }
                    hits.push(format!("{} contains {:?}", file.rel_path, needle));
                }
            }
        }
    }
    let global = templates::global_claude_md();
    for needle in bad_substrings {
        if global.content.contains(needle) {
            hits.push(format!("CLAUDE.md contains {:?}", needle));
        }
    }

    assert!(
        hits.is_empty(),
        "templates should not embed infra constants:\n  {}",
        hits.join("\n  ")
    );
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
