//! Scans TypeScript/JavaScript source files for import statements
//! and extracts external and workspace dependency package names.
//!
//! Used by `airis gen` to auto-detect dependencies from source code,
//! eliminating the need to manually list deps in manifest.toml.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::LazyLock;

use anyhow::{Context, Result};
use regex::Regex;
use walkdir::WalkDir;

/// Pre-compiled import regex — compiled once, reused across all calls.
static IMPORT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?:^|[\s})])from\s+['"]([^'"]+)['"]|(?:import|require)\s*\(\s*['"]([^'"]+)['"]\s*\)"#,
    )
    .expect("import regex should compile")
});

/// Result of scanning a directory for import statements.
#[derive(Debug, Default)]
pub struct ScannedDeps {
    /// External npm packages (e.g., "next", "@fastify/formbody")
    pub external: BTreeSet<String>,
    /// Workspace packages matching the workspace scope (e.g., "@agiletec/ui")
    pub workspace: BTreeSet<String>,
}

/// Scan all TypeScript/JavaScript files in a directory for import statements.
///
/// Walks `app_path` recursively, skipping node_modules/dist/.next etc.
/// Extracts package names from `import`, `export`, `require()`, and `import()`.
///
/// - Packages matching `workspace_scope` (e.g., "@agiletec") → `workspace`
/// - Everything else → `external`
/// - Relative imports (`./`, `../`) and path aliases (`@/`, `~/`, `#`) are skipped
pub fn scan_imports(app_path: &Path, workspace_scope: &str) -> Result<ScannedDeps> {
    let mut deps = ScannedDeps::default();

    // Build ignore walker (respects .gitignore, skips common build dirs)
    let walker = WalkDir::new(app_path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_ignored_dir(e));

    for entry in walker {
        let entry = entry.with_context(|| format!("Walking {}", app_path.display()))?;
        let path = entry.path();

        if !is_scannable_file(path) {
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue, // skip unreadable files
        };

        extract_packages(&content, &IMPORT_RE, workspace_scope, &mut deps);
    }

    Ok(deps)
}

/// Scan a single file's content for imports (useful for testing).
#[allow(dead_code)]
pub fn scan_content(content: &str, workspace_scope: &str) -> ScannedDeps {
    let mut deps = ScannedDeps::default();
    extract_packages(content, &IMPORT_RE, workspace_scope, &mut deps);
    deps
}

/// Extract package names from regex matches and classify them.
///
/// Processes content line-by-line to skip comments:
/// - Line comments (`//`)
/// - Block comments (`/* ... */`)
/// - JSDoc lines (`* ...`, `/** ...`)
///
/// This prevents false positives from import examples in documentation.
fn extract_packages(content: &str, re: &Regex, workspace_scope: &str, deps: &mut ScannedDeps) {
    let mut in_block_comment = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Track block comment state
        if in_block_comment {
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }

        // Start of block comment
        if trimmed.starts_with("/*") {
            if !trimmed.contains("*/") {
                in_block_comment = true;
            }
            continue;
        }

        // Skip line comments and JSDoc continuation lines
        if trimmed.starts_with("//") || trimmed.starts_with('*') {
            continue;
        }

        // Apply import regex to non-comment lines only
        for caps in re.captures_iter(line) {
            let specifier = caps.get(1).or_else(|| caps.get(2));
            let specifier = match specifier {
                Some(m) => m.as_str(),
                None => continue,
            };

            // Skip relative imports and path aliases
            if specifier.starts_with('.')
                || specifier.starts_with("@/")
                || specifier.starts_with("~/")
                || specifier.starts_with('#')
            {
                continue;
            }

            let package_name = extract_package_name(specifier);

            if is_node_builtin(package_name) {
                continue;
            }

            if package_name.starts_with(workspace_scope) {
                deps.workspace.insert(package_name.to_string());
            } else {
                deps.external.insert(package_name.to_string());
            }
        }
    }
}

/// Extract the npm package name from an import specifier.
///
/// - `"next"` → `"next"`
/// - `"next/image"` → `"next"`
/// - `"@fastify/formbody"` → `"@fastify/formbody"`
/// - `"@agiletec/ui/button"` → `"@agiletec/ui"`
fn extract_package_name(specifier: &str) -> &str {
    if let Some(stripped) = specifier.strip_prefix('@') {
        // Scoped package: @scope/name or @scope/name/subpath
        // Find the second '/' (after @scope/name)
        let after_scope = match stripped.find('/') {
            Some(i) => i + 1,         // position of first '/' in original string
            None => return specifier, // just "@scope" (unusual but handle it)
        };
        // Find the next '/' after "@scope/name"
        match specifier[after_scope + 1..].find('/') {
            Some(i) => &specifier[..after_scope + 1 + i],
            None => specifier, // no subpath
        }
    } else {
        // Unscoped package: name or name/subpath
        match specifier.find('/') {
            Some(i) => &specifier[..i],
            None => specifier,
        }
    }
}

/// Check if a directory entry should be skipped during walk.
fn is_ignored_dir(entry: &walkdir::DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    let name = entry.file_name().to_string_lossy();
    matches!(
        name.as_ref(),
        "node_modules"
            | "dist"
            | ".next"
            | ".turbo"
            | ".swc"
            | ".cache"
            | "build"
            | "out"
            | "coverage"
            | "tests"
            | "__tests__"
            | "__mocks__"
            | ".git"
    )
}

/// Check if a file should be scanned for imports.
fn is_scannable_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ts" | "tsx" | "js" | "jsx" | "mts" | "mjs")
    )
}

/// Check if a module specifier is a Node.js built-in module.
fn is_node_builtin(name: &str) -> bool {
    // Strip "node:" prefix if present
    let name = name.strip_prefix("node:").unwrap_or(name);
    matches!(
        name,
        "assert"
            | "buffer"
            | "child_process"
            | "cluster"
            | "console"
            | "constants"
            | "crypto"
            | "dgram"
            | "dns"
            | "domain"
            | "events"
            | "fs"
            | "http"
            | "http2"
            | "https"
            | "inspector"
            | "module"
            | "net"
            | "os"
            | "path"
            | "perf_hooks"
            | "process"
            | "punycode"
            | "querystring"
            | "readline"
            | "repl"
            | "stream"
            | "string_decoder"
            | "sys"
            | "timers"
            | "tls"
            | "trace_events"
            | "tty"
            | "url"
            | "util"
            | "v8"
            | "vm"
            | "wasi"
            | "worker_threads"
            | "zlib"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_package_name_unscoped() {
        assert_eq!(extract_package_name("next"), "next");
        assert_eq!(extract_package_name("next/image"), "next");
        assert_eq!(extract_package_name("next/font/google"), "next");
        assert_eq!(extract_package_name("react"), "react");
        assert_eq!(extract_package_name("react-dom/client"), "react-dom");
    }

    #[test]
    fn test_extract_package_name_scoped() {
        assert_eq!(
            extract_package_name("@fastify/formbody"),
            "@fastify/formbody"
        );
        assert_eq!(extract_package_name("@agiletec/ui"), "@agiletec/ui");
        assert_eq!(extract_package_name("@agiletec/ui/button"), "@agiletec/ui");
        assert_eq!(
            extract_package_name("@radix-ui/react-dialog"),
            "@radix-ui/react-dialog"
        );
        assert_eq!(extract_package_name("@slack/web-api"), "@slack/web-api");
    }

    #[test]
    fn test_scan_standard_imports() {
        let content = r#"
import { useState } from 'react'
import type { Metadata } from 'next'
import Fastify from 'fastify'
import { Button } from '@agiletec/ui'
import { logger } from '@agiletec/logger'
import { resolveConfig } from './config'
import type { MyType } from '../types'
"#;
        let deps = scan_content(content, "@agiletec");
        assert!(deps.external.contains("react"));
        assert!(deps.external.contains("next"));
        assert!(deps.external.contains("fastify"));
        assert!(deps.workspace.contains("@agiletec/ui"));
        assert!(deps.workspace.contains("@agiletec/logger"));
        // Relative imports should not be included
        assert!(!deps.external.contains("./config"));
        assert!(!deps.external.contains("../types"));
    }

    #[test]
    fn test_scan_reexports() {
        let content = r#"
export { buildSipTransferTwiml } from '@agiletec/voice-shared'
export type { CallSummary } from '@agiletec/voice-shared'
export { default as Button } from '@radix-ui/react-dialog'
"#;
        let deps = scan_content(content, "@agiletec");
        assert!(deps.workspace.contains("@agiletec/voice-shared"));
        assert!(deps.external.contains("@radix-ui/react-dialog"));
    }

    #[test]
    fn test_scan_dynamic_imports() {
        let content = r#"
const mod = await import('next/dynamic')
const pkg = require('@slack/bolt')
const lazy = import('react-dom/client')
"#;
        let deps = scan_content(content, "@agiletec");
        assert!(deps.external.contains("next"));
        assert!(deps.external.contains("@slack/bolt"));
        assert!(deps.external.contains("react-dom"));
    }

    #[test]
    fn test_scan_subpath_imports() {
        let content = r#"
import Image from 'next/image'
import { Inter } from 'next/font/google'
import { createClient } from '@supabase/supabase-js'
import { createBrowserClient } from '@supabase/ssr'
"#;
        let deps = scan_content(content, "@agiletec");
        assert!(deps.external.contains("next"));
        assert!(deps.external.contains("@supabase/supabase-js"));
        assert!(deps.external.contains("@supabase/ssr"));
        // Should NOT have "next/image" or "next/font" as separate entries
        assert!(!deps.external.contains("next/image"));
        assert!(!deps.external.contains("next/font/google"));
    }

    #[test]
    fn test_skip_relative_and_alias_imports() {
        let content = r#"
import { foo } from './utils'
import { bar } from '../lib/helpers'
import { baz } from '@/components/button'
import { qux } from '~/config'
import { hash } from '#internal'
"#;
        let deps = scan_content(content, "@agiletec");
        assert!(deps.external.is_empty());
        assert!(deps.workspace.is_empty());
    }

    #[test]
    fn test_skip_node_builtins() {
        let content = r#"
import fs from 'fs'
import path from 'path'
import { createHash } from 'crypto'
import { Buffer } from 'node:buffer'
import { Worker } from 'node:worker_threads'
"#;
        let deps = scan_content(content, "@agiletec");
        assert!(deps.external.is_empty());
        assert!(deps.workspace.is_empty());
    }

    #[test]
    fn test_scan_mixed_quotes() {
        let content = r#"
import { a } from "react"
import { b } from 'next'
const c = require("express")
"#;
        let deps = scan_content(content, "@agiletec");
        assert!(deps.external.contains("react"));
        assert!(deps.external.contains("next"));
        assert!(deps.external.contains("express"));
    }

    // Integration test: scan actual agiletec source directories
    #[test]
    fn test_scan_real_agiletec_corporate() {
        let app_path = Path::new("/Users/kazuki/github/agiletec-inc/agiletec/apps/corporate");
        if !app_path.exists() {
            return; // Skip if not on the dev machine
        }
        let deps = scan_imports(app_path, "@agiletec").unwrap();

        // Corporate site should have these deps
        assert!(deps.external.contains("next"), "should detect 'next'");
        assert!(deps.external.contains("react"), "should detect 'react'");
        assert!(
            deps.workspace.contains("@agiletec/ui"),
            "should detect '@agiletec/ui'"
        );
    }

    #[test]
    fn test_scan_real_agiletec_voice_gateway() {
        let app_path =
            Path::new("/Users/kazuki/github/agiletec-inc/agiletec/products/airis/voice-gateway");
        if !app_path.exists() {
            return;
        }
        let deps = scan_imports(app_path, "@agiletec").unwrap();

        assert!(deps.external.contains("fastify"), "should detect 'fastify'");
        assert!(
            deps.workspace.contains("@agiletec/logger"),
            "should detect '@agiletec/logger'"
        );
    }

    #[test]
    fn test_skip_line_comments() {
        let content = r#"
// import { foo } from 'should-not-detect'
import { bar } from 'real-package'
"#;
        let deps = scan_content(content, "@agiletec");
        assert!(!deps.external.contains("should-not-detect"));
        assert!(deps.external.contains("real-package"));
    }

    #[test]
    fn test_skip_block_comments() {
        let content = r#"
/*
import { foo } from 'should-not-detect'
import { bar } from 'also-not-detect'
*/
import { baz } from 'real-package'
"#;
        let deps = scan_content(content, "@agiletec");
        assert!(!deps.external.contains("should-not-detect"));
        assert!(!deps.external.contains("also-not-detect"));
        assert!(deps.external.contains("real-package"));
    }

    #[test]
    fn test_skip_jsdoc_imports() {
        let content = r#"
/**
 * Usage example:
 *   import { getPricingCached } from '@agiletec/pricing/server'
 *   import { usePricing } from '@agiletec/pricing/client'
 *   import { fetchPricing } from '@agiletec/pricing'
 */
export function fetchPricing() {}
"#;
        let deps = scan_content(content, "@agiletec");
        // JSDoc imports should NOT be detected
        assert!(!deps.workspace.contains("@agiletec/pricing"));
        // No external deps either
        assert!(deps.external.is_empty());
    }

    #[test]
    fn test_skip_jsdoc_continuation_lines() {
        let content = r#"
/**
 * @example
 * import { createNotificationRouter } from '@agiletec/notification'
 */
import { logger } from '@agiletec/logger'
"#;
        let deps = scan_content(content, "@agiletec");
        assert!(!deps.workspace.contains("@agiletec/notification"));
        assert!(deps.workspace.contains("@agiletec/logger"));
    }

    #[test]
    fn test_inline_block_comment_on_same_line() {
        let content = r#"
/* import { foo } from 'not-real' */
import { bar } from 'real-package'
"#;
        let deps = scan_content(content, "@agiletec");
        assert!(!deps.external.contains("not-real"));
        assert!(deps.external.contains("real-package"));
    }

    #[test]
    fn test_real_agiletec_pricing_no_self_ref() {
        let app_path = Path::new("/Users/kazuki/github/agiletec-inc/agiletec/libs/pricing");
        if !app_path.exists() {
            return;
        }
        let deps = scan_imports(app_path, "@agiletec").unwrap();
        // pricing should NOT contain itself as a dependency
        assert!(
            !deps.workspace.contains("@agiletec/pricing"),
            "pricing should not self-reference (detected from JSDoc comments)"
        );
    }

    #[test]
    fn test_real_agiletec_notification_no_self_ref() {
        let app_path = Path::new("/Users/kazuki/github/agiletec-inc/agiletec/libs/notification");
        if !app_path.exists() {
            return;
        }
        let deps = scan_imports(app_path, "@agiletec").unwrap();
        assert!(
            !deps.workspace.contains("@agiletec/notification"),
            "notification should not self-reference"
        );
    }

    #[test]
    fn test_scan_multiline_imports() {
        let content = r#"
import {
  Controller,
  type ControllerProps,
  type FieldPath,
  FormProvider,
} from 'react-hook-form'

import { cn } from '../lib/utils'
"#;
        let deps = scan_content(content, "@agiletec");
        assert!(
            deps.external.contains("react-hook-form"),
            "should detect multi-line import"
        );
        assert!(!deps.external.contains("../lib/utils"));
    }

    #[test]
    fn test_real_agiletec_ui_has_react_hook_form() {
        let app_path = Path::new("/Users/kazuki/github/agiletec-inc/agiletec/libs/ui");
        if !app_path.exists() {
            return;
        }
        let deps = scan_imports(app_path, "@agiletec").unwrap();
        assert!(
            deps.external.contains("react-hook-form"),
            "ui should detect react-hook-form from multi-line import in form.tsx"
        );
    }
}
