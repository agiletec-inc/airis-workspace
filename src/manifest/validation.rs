use anyhow::{Result, bail};

use super::*;

impl Manifest {
    /// Validate manifest consistency.
    ///
    /// Checks:
    /// 1. No duplicate ports across service entries
    /// 2. Catalog follow references point to existing catalog keys
    /// 3. No command appears in both guards.deny and guards.wrap
    /// 4. dep_group / env_group references resolve to defined groups
    /// 5. Catalog follow chains have no cycles
    /// 6. env.validation keys exist in env.required or env.optional
    /// 7. Catalog version strings that look like typos of "latest" / "lts" (warning only)
    pub fn validate(&self) -> Result<()> {
        let mut errors: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        // 1. Check for duplicate ports in service entries
        {
            let mut seen: std::collections::HashMap<u16, String> = std::collections::HashMap::new();
            for (name, svc) in &self.service {
                if let Some(port) = svc.port {
                    if let Some(prev) = seen.get(&port) {
                        errors.push(format!(
                            "Duplicate port {port}: services \"{prev}\" and \"{name}\" both bind to port {port}"
                        ));
                    } else {
                        seen.insert(port, name.clone());
                    }
                }
            }
        }

        // 2. Validate catalog follow references (skip if default_policy can resolve the target)
        for (key, entry) in &self.packages.catalog {
            if let CatalogEntry::Follow(f) = entry
                && !self.packages.catalog.contains_key(&f.follow)
                && self.packages.default_policy.is_none()
            {
                errors.push(format!(
                        "Catalog entry \"{key}\" follows \"{}\", which does not exist in packages.catalog (add it or set default_policy)",
                        f.follow
                    ));
            }
        }

        // 3. Check for commands in both guards.deny and guards.wrap
        for cmd in &self.guards.deny {
            if self.guards.wrap.contains_key(cmd) {
                errors.push(format!(
                    "Guard conflict: \"{cmd}\" appears in both guards.deny and guards.wrap"
                ));
            }
        }

        // 3b. Validate guard command names (shell metacharacter prevention)
        let cmd_re = regex::Regex::new(r"^[a-zA-Z0-9._+\-]+$").unwrap();
        for cmd in &self.guards.deny {
            if !cmd_re.is_match(cmd) {
                errors.push(format!(
                    "guards.deny contains invalid command name \"{cmd}\": only [a-zA-Z0-9._+-] allowed"
                ));
            }
        }
        for cmd in self.guards.wrap.keys() {
            if !cmd_re.is_match(cmd) {
                errors.push(format!(
                    "guards.wrap contains invalid command name \"{cmd}\": only [a-zA-Z0-9._+-] allowed"
                ));
            }
        }
        for wrapper in self.guards.wrap.values() {
            let dangerous_chars = ['`', '$', '(', ')', ';', '&', '|', '<', '>', '\n', '\r', '\\', '!', '{', '}'];
            if let Some(bad) = wrapper.chars().find(|c| dangerous_chars.contains(c)) {
                errors.push(format!(
                    "guards.wrap value \"{wrapper}\" contains dangerous character '{bad}': shell metacharacters are not allowed"
                ));
            }
        }
        for cmd in self.guards.deny_with_message.keys() {
            if !cmd_re.is_match(cmd) {
                errors.push(format!(
                    "guards.deny_with_message contains invalid command name \"{cmd}\": only [a-zA-Z0-9._+-] allowed"
                ));
            }
        }

        // 4. Validate dep_group / env_group references
        self.validate_group_references(&mut errors);

        // 5. Detect cycles in catalog follow chains
        self.validate_catalog_cycles(&mut errors);

        // 6. Detect orphaned env.validation keys
        self.validate_env_validation_keys(&mut errors);

        // 7. Detect likely typos of "latest" / "lts" in catalog versions (warning)
        self.detect_catalog_typos(&mut warnings);
        // 8. Reject host bind mounts in manifest-defined volumes
        self.validate_no_host_bind_mounts(&mut errors);
        // 9. Validate forbidden_patterns are valid regex
        self.validate_testing_patterns(&mut errors);
        // 10. Validate policy section
        self.validate_policy(&mut errors);

        for w in &warnings {
            eprintln!("\u{26a0}\u{fe0f}  {w}");
        }

        if !errors.is_empty() {
            bail!("Manifest validation failed:\n{}", errors.join("\n"));
        }

        Ok(())
    }

    /// Check that all dep_group and env_group references point to defined groups.
    fn validate_group_references(&self, errors: &mut Vec<String>) {
        // [[app]] dep_groups / dev_dep_groups
        for app in &self.app {
            let label = if app.name.is_empty() {
                "[[app]]".to_string()
            } else {
                format!("[[app]] \"{}\"", app.name)
            };
            for g in &app.dep_groups {
                if !self.dep_group.contains_key(g) {
                    errors.push(format!(
                        "{label}: dep_groups references undefined [dep_group.{g}]"
                    ));
                }
            }
            for g in &app.dev_dep_groups {
                if !self.dep_group.contains_key(g) {
                    errors.push(format!(
                        "{label}: dev_dep_groups references undefined [dep_group.{g}]"
                    ));
                }
            }
        }

        // [service.*] env_groups
        for (name, svc) in &self.service {
            for g in &svc.env_groups {
                if !self.env_group.contains_key(g) {
                    errors.push(format!(
                        "[service.{name}]: env_groups references undefined [env_group.{g}]"
                    ));
                }
            }
        }

        // [[app]] deploy.env_groups
        for app in &self.app {
            if let Some(ref deploy) = app.deploy {
                let label = if app.name.is_empty() {
                    "[[app]]".to_string()
                } else {
                    format!("[[app]] \"{}\"", app.name)
                };
                for g in &deploy.env_groups {
                    if !self.env_group.contains_key(g) {
                        errors.push(format!(
                            "{label}: deploy.env_groups references undefined [env_group.{g}]"
                        ));
                    }
                }
            }
        }

        // [preset.*] dep_groups / dev_dep_groups
        for (name, preset) in &self.preset {
            for g in &preset.dep_groups {
                if !self.dep_group.contains_key(g) {
                    errors.push(format!(
                        "[preset.{name}]: dep_groups references undefined [dep_group.{g}]"
                    ));
                }
            }
            for g in &preset.dev_dep_groups {
                if !self.dep_group.contains_key(g) {
                    errors.push(format!(
                        "[preset.{name}]: dev_dep_groups references undefined [dep_group.{g}]"
                    ));
                }
            }
        }
    }

    /// Detect cycles in catalog follow chains (e.g. A→B→C→A).
    fn validate_catalog_cycles(&self, errors: &mut Vec<String>) {
        for key in self.packages.catalog.keys() {
            let mut visited = std::collections::HashSet::new();
            let mut current = key.as_str();
            visited.insert(current);

            while let Some(CatalogEntry::Follow(f)) = self.packages.catalog.get(current) {
                let next = f.follow.as_str();
                if !visited.insert(next) {
                    // Cycle detected — build readable chain
                    let chain: Vec<&str> = visited.iter().copied().collect();
                    errors.push(format!(
                        "Catalog follow cycle detected: {} → {}",
                        chain.join(" → "),
                        next
                    ));
                    break;
                }
                current = next;
            }
        }
    }

    /// Warn when env.validation defines rules for variables not in env.required or env.optional.
    fn validate_env_validation_keys(&self, errors: &mut Vec<String>) {
        let declared: std::collections::HashSet<&str> = self
            .env
            .required
            .iter()
            .chain(self.env.optional.iter())
            .map(|s| s.as_str())
            .collect();

        for key in self.env.validation.keys() {
            if !declared.contains(key.as_str()) {
                errors.push(format!(
                    "[env.validation.{key}] defines rules but \"{key}\" is not listed in env.required or env.optional"
                ));
            }
        }
    }

    /// Emit warnings for catalog version strings that look like typos of "latest" or "lts".
    fn detect_catalog_typos(&self, warnings: &mut Vec<String>) {
        const KNOWN_POLICIES: &[&str] = &["latest", "lts"];

        for (key, entry) in &self.packages.catalog {
            if let CatalogEntry::Version(v) = entry {
                let lower = v.to_lowercase();
                // Skip semver-like strings (start with digit or caret/tilde/comparison)
                if lower.starts_with(|c: char| c.is_ascii_digit() || "^~<>=".contains(c)) {
                    continue;
                }
                // Skip exact matches
                if KNOWN_POLICIES.contains(&lower.as_str()) {
                    continue;
                }
                // Check Levenshtein distance against known policies
                for policy in KNOWN_POLICIES {
                    if levenshtein_distance(&lower, policy) <= 2 {
                        warnings.push(format!(
                            "Catalog \"{key}\" = \"{v}\" looks like a typo of \"{policy}\""
                        ));
                        break;
                    }
                }
            }
        }
    }

    fn validate_no_host_bind_mounts(&self, errors: &mut Vec<String>) {
        for volume in &self.workspace.volumes {
            if is_host_bind_mount(volume) {
                errors.push(format!(
                    "[workspace].volumes contains host bind mount \"{volume}\"; use named volumes only"
                ));
            }
        }

        for (name, svc) in &self.service {
            for volume in &svc.volumes {
                if is_host_bind_mount(volume) {
                    errors.push(format!(
                        "[service.{name}].volumes contains host bind mount \"{volume}\"; use named volumes only"
                    ));
                }
            }
        }
    }
}

fn is_host_bind_mount(spec: &str) -> bool {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return false;
    }

    if trimmed.starts_with("./")
        || trimmed.starts_with("../")
        || trimmed == "."
        || trimmed == ".."
        || trimmed.starts_with('/')
        || trimmed.starts_with("~/")
        || trimmed.starts_with("${")
    {
        return true;
    }

    let mut parts = trimmed.split(':');
    let source = match parts.next() {
        Some(source) => source,
        None => return false,
    };

    if source.is_empty() {
        return false;
    }

    if source.starts_with("./")
        || source.starts_with("../")
        || source == "."
        || source == ".."
        || source.starts_with('/')
        || source.starts_with("~/")
        || source.starts_with("${")
    {
        return true;
    }

    source.len() >= 3
        && source.as_bytes()[1] == b':'
        && (source.as_bytes()[2] == b'/' || source.as_bytes()[2] == b'\\')
}

impl Manifest {
    /// Validate that testing.forbidden_patterns are valid regex.
    fn validate_testing_patterns(&self, errors: &mut Vec<String>) {
        for (i, pattern) in self.testing.forbidden_patterns.iter().enumerate() {
            if regex::Regex::new(pattern).is_err() {
                errors.push(format!(
                    "[testing].forbidden_patterns[{i}]: invalid regex \"{}\"",
                    pattern
                ));
            }
        }
        if let Some(te) = &self.testing.type_enforcement {
            for (i, pattern) in te.required_imports.iter().enumerate() {
                if regex::Regex::new(pattern).is_err() {
                    errors.push(format!(
                        "[testing.type_enforcement].required_imports[{i}]: invalid regex \"{}\"",
                        pattern
                    ));
                }
            }
        }
    }

    /// Validate [policy] section: testing patterns + security allowed_paths.
    fn validate_policy(&self, errors: &mut Vec<String>) {
        // Validate policy.testing forbidden_patterns
        for (i, pattern) in self.policy.testing.forbidden_patterns.iter().enumerate() {
            if regex::Regex::new(pattern).is_err() {
                errors.push(format!(
                    "[policy.testing].forbidden_patterns[{i}]: invalid regex \"{pattern}\""
                ));
            }
        }
        if let Some(te) = &self.policy.testing.type_enforcement {
            for (i, pattern) in te.required_imports.iter().enumerate() {
                if regex::Regex::new(pattern).is_err() {
                    errors.push(format!(
                        "[policy.testing.type_enforcement].required_imports[{i}]: invalid regex \"{pattern}\""
                    ));
                }
            }
        }

        // Validate policy.security.allowed_paths are valid glob patterns
        for (i, pattern) in self.policy.security.allowed_paths.iter().enumerate() {
            if glob::Pattern::new(pattern).is_err() {
                errors.push(format!(
                    "[policy.security].allowed_paths[{i}]: invalid glob pattern \"{pattern}\""
                ));
            }
        }
    }
}

/// Simple Levenshtein distance for short strings (catalog typo detection).
pub(crate) fn levenshtein_distance(a: &str, b: &str) -> usize {
    let b_len = b.len();
    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0; b_len + 1];

    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(curr[j] + 1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b_len]
}
