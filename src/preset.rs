//! Preset resolution and profile variable expansion for manifest v2.
//!
//! Presets provide reusable deps/scripts/deploy defaults for [[app]] definitions.
//! Profile variables (`{profile.domain}`) are resolved at `airis gen` time.
//! dep_group provides reusable dependency groupings (e.g., shadcn radix-ui components).

use anyhow::{Context, Result, bail};
use indexmap::IndexMap;

use crate::manifest::{AppDeployConfig, PresetSection, ProfileSection, ProjectDefinition};

/// Resolved app with preset deps/scripts merged in.
/// App values always override preset values.
#[derive(Debug)]
#[allow(dead_code)]
pub struct ResolvedApp {
    pub deps: IndexMap<String, String>,
    pub dev_deps: IndexMap<String, String>,
    pub scripts: IndexMap<String, String>,
    pub framework: Option<String>,
    pub private: Option<bool>,
    pub deploy: Option<AppDeployConfig>,
}

/// Resolve presets for a single app.
/// Merges preset deps/scripts/deploy with app-level overrides.
/// Multiple presets are applied left-to-right (later presets override earlier ones).
///
/// Merge order (later wins):
///   1. Preset dep_groups → preset deps → preset scripts
///   2. App dep_groups → app deps → app scripts
pub fn resolve_app_presets(
    app: &ProjectDefinition,
    presets: &IndexMap<String, PresetSection>,
    dep_groups: &IndexMap<String, IndexMap<String, String>>,
) -> Result<ResolvedApp> {
    let mut merged_deps = IndexMap::new();
    let mut merged_dev_deps = IndexMap::new();
    let mut merged_scripts = IndexMap::new();
    let mut framework = None;
    let mut private = None;
    let mut deploy = app.deploy.clone();

    // Apply presets in order
    if let Some(ref preset_ref) = app.preset {
        for preset_name in preset_ref.as_list() {
            let preset = presets.get(preset_name).with_context(|| {
                format!(
                    "App '{}' references preset '{}' which is not defined in [preset.*]",
                    app.name, preset_name
                )
            })?;

            // Expand preset dep_groups first
            expand_dep_groups(&preset.dep_groups, dep_groups, &mut merged_deps)?;
            expand_dep_groups(&preset.dev_dep_groups, dep_groups, &mut merged_dev_deps)?;

            // Merge preset direct deps (override dep_group values)
            for (k, v) in &preset.deps {
                merged_deps.insert(k.clone(), v.clone());
            }
            for (k, v) in &preset.dev_deps {
                merged_dev_deps.insert(k.clone(), v.clone());
            }
            for (k, v) in &preset.scripts {
                merged_scripts.insert(k.clone(), v.clone());
            }

            if let Some(ref fw) = preset.framework {
                framework = Some(fw.clone());
            }
            if let Some(priv_) = preset.private {
                private = Some(priv_);
            }

            // Apply preset deploy defaults if app doesn't have explicit deploy
            if deploy.is_none()
                && let Some(ref defaults) = preset.deploy
            {
                deploy = Some(AppDeployConfig {
                    enabled: true,
                    variant: defaults.variant.clone(),
                    port: defaults.port,
                    health_path: defaults.health_path.clone(),
                    ..Default::default()
                });
            }
        }
    }

    // Expand app-level dep_groups
    expand_dep_groups(&app.dep_groups, dep_groups, &mut merged_deps)?;
    expand_dep_groups(&app.dev_dep_groups, dep_groups, &mut merged_dev_deps)?;

    // App values override preset values
    for (k, v) in &app.deps {
        merged_deps.insert(k.clone(), v.clone());
    }
    for (k, v) in &app.dev_deps {
        merged_dev_deps.insert(k.clone(), v.clone());
    }
    for (k, v) in &app.scripts {
        merged_scripts.insert(k.clone(), v.clone());
    }
    if app.framework.is_some() {
        framework = app.framework.clone();
    }
    if app.private.is_some() {
        private = app.private;
    }
    if app.deploy.is_some() {
        deploy = app.deploy.clone();
    }

    Ok(ResolvedApp {
        deps: merged_deps,
        dev_deps: merged_dev_deps,
        scripts: merged_scripts,
        framework,
        private,
        deploy,
    })
}

/// Expand dep_group references into a deps map.
fn expand_dep_groups(
    group_names: &[String],
    dep_groups: &IndexMap<String, IndexMap<String, String>>,
    target: &mut IndexMap<String, String>,
) -> Result<()> {
    for name in group_names {
        let group = dep_groups
            .get(name)
            .with_context(|| format!("dep_group '{}' is not defined in [dep_group.*]", name))?;
        for (k, v) in group {
            target.insert(k.clone(), v.clone());
        }
    }
    Ok(())
}

/// Resolve all apps' presets.
#[allow(dead_code)]
pub fn resolve_all_presets(
    apps: &[ProjectDefinition],
    presets: &IndexMap<String, PresetSection>,
    dep_groups: &IndexMap<String, IndexMap<String, String>>,
) -> Result<Vec<ResolvedApp>> {
    apps.iter()
        .map(|app| resolve_app_presets(app, presets, dep_groups))
        .collect()
}

/// Resolve `{profile.*}` placeholders in a string.
/// E.g., "{profile.domain}" → "stg.agiletec.net"
/// `$VAR` patterns are left as-is (runtime variables).
#[allow(dead_code)]
pub fn resolve_profile_vars(template: &str, profile: &ProfileSection) -> String {
    template
        .replace("{profile.domain}", &profile.domain)
        .replace("{profile.node_env}", &profile.node_env)
}

/// Resolve a profile with inheritance.
/// If profile has `inherits`, merge parent first, then child overrides.
#[allow(dead_code)]
pub fn resolve_profile(
    name: &str,
    profiles: &IndexMap<String, ProfileSection>,
) -> Result<ProfileSection> {
    let profile = profiles
        .get(name)
        .with_context(|| format!("Profile '{}' not found", name))?;

    if let Some(ref parent_name) = profile.inherits {
        if parent_name == name {
            bail!("Profile '{}' inherits from itself", name);
        }
        let parent = resolve_profile(parent_name, profiles)?;

        // Child overrides parent
        Ok(ProfileSection {
            branch: profile.branch.clone().or(parent.branch),
            env_source: if matches!(profile.env_source, crate::manifest::EnvSource::Simple(ref s) if s == "dotenv")
            {
                parent.env_source
            } else {
                profile.env_source.clone()
            },
            domain: if profile.domain == "localhost" {
                parent.domain
            } else {
                profile.domain.clone()
            },
            node_env: profile.node_env.clone(),
            compose_profiles: if profile.compose_profiles.is_empty() {
                parent.compose_profiles
            } else {
                profile.compose_profiles.clone()
            },
            inherits: None, // Already resolved
            role: profile.role.clone().or(parent.role),
        })
    } else {
        Ok(profile.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{PresetDeployDefaults, PresetRef};

    fn empty_project(name: &str) -> ProjectDefinition {
        ProjectDefinition {
            name: name.to_string(),
            kind: None,
            path: Some(format!("apps/{}", name)),
            scope: None,
            description: None,
            bin: IndexMap::new(),
            main: None,
            types: None,
            version: None,
            private: None,
            module_type: None,
            exports: None,
            peer_deps: IndexMap::new(),
            peer_deps_meta: None,
            tags: vec![],
            files: vec![],
            framework: None,
            runner: None,
            scripts: IndexMap::new(),
            deps: IndexMap::new(),
            dev_deps: IndexMap::new(),
            port: None,
            replicas: None,
            resources: None,
            deploy: None,
            preset: None,
            profiles: None,
            depends_on: None,
            mem_limit: None,
            cpus: None,
            service: None,
            tsconfig: None,
            dep_groups: vec![],
            dev_dep_groups: vec![],
        }
    }

    #[test]
    fn test_resolve_single_preset() {
        let mut presets = IndexMap::new();
        let mut preset_deps = IndexMap::new();
        preset_deps.insert("react".to_string(), "catalog".to_string());
        preset_deps.insert("next".to_string(), "catalog".to_string());

        let mut preset_scripts = IndexMap::new();
        preset_scripts.insert("dev".to_string(), "next dev".to_string());
        preset_scripts.insert("build".to_string(), "next build".to_string());

        presets.insert(
            "nextjs-app".to_string(),
            PresetSection {
                framework: Some("nextjs".to_string()),
                private: Some(true),
                scripts: preset_scripts,
                deps: preset_deps,
                dev_deps: IndexMap::new(),
                deploy: Some(PresetDeployDefaults {
                    variant: Some("nextjs".to_string()),
                    port: Some(3000),
                    health_path: Some("/api/health".to_string()),
                }),
                dep_groups: vec![],
                dev_dep_groups: vec![],
                scope: None,
            },
        );

        let mut app = empty_project("corporate");
        app.preset = Some(PresetRef::Single("nextjs-app".to_string()));

        // Add app-specific dep
        app.deps.insert("stripe".to_string(), "catalog".to_string());

        let resolved = resolve_app_presets(&app, &presets, &IndexMap::new()).unwrap();

        // Preset deps + app deps merged
        assert_eq!(resolved.deps.len(), 3); // react, next, stripe
        assert!(resolved.deps.contains_key("react"));
        assert!(resolved.deps.contains_key("stripe"));

        // Preset scripts inherited
        assert_eq!(resolved.scripts.get("dev").unwrap(), "next dev");

        // Framework from preset
        assert_eq!(resolved.framework.as_deref(), Some("nextjs"));
    }

    #[test]
    fn test_app_overrides_preset() {
        let mut presets = IndexMap::new();
        let mut preset_scripts = IndexMap::new();
        preset_scripts.insert("typecheck".to_string(), "tsc --noEmit".to_string());

        presets.insert(
            "ts-lib".to_string(),
            PresetSection {
                framework: None,
                private: None,
                scripts: preset_scripts,
                deps: IndexMap::new(),
                dev_deps: IndexMap::new(),
                deploy: None,
                dep_groups: vec![],
                dev_dep_groups: vec![],
                scope: None,
            },
        );

        let mut app = empty_project("dashboard");
        app.preset = Some(PresetRef::Single("ts-lib".to_string()));
        app.scripts.insert(
            "typecheck".to_string(),
            "tsc -p tsconfig.json --noEmit".to_string(),
        );

        let resolved = resolve_app_presets(&app, &presets, &IndexMap::new()).unwrap();

        // App script overrides preset
        assert_eq!(
            resolved.scripts.get("typecheck").unwrap(),
            "tsc -p tsconfig.json --noEmit"
        );
    }

    #[test]
    fn test_multiple_presets() {
        let mut presets = IndexMap::new();

        let mut base_deps = IndexMap::new();
        base_deps.insert("react".to_string(), "catalog".to_string());
        presets.insert(
            "base".to_string(),
            PresetSection {
                framework: None,
                private: None,
                scripts: IndexMap::new(),
                deps: base_deps,
                dev_deps: IndexMap::new(),
                deploy: None,
                dep_groups: vec![],
                dev_dep_groups: vec![],
                scope: None,
            },
        );

        let mut ui_deps = IndexMap::new();
        ui_deps.insert("clsx".to_string(), "catalog".to_string());
        presets.insert(
            "shadcn".to_string(),
            PresetSection {
                framework: None,
                private: None,
                scripts: IndexMap::new(),
                deps: ui_deps,
                dev_deps: IndexMap::new(),
                deploy: None,
                dep_groups: vec![],
                dev_dep_groups: vec![],
                scope: None,
            },
        );

        let mut app = empty_project("ui");
        app.preset = Some(PresetRef::Multiple(vec![
            "base".to_string(),
            "shadcn".to_string(),
        ]));

        let resolved = resolve_app_presets(&app, &presets, &IndexMap::new()).unwrap();

        // Both presets' deps merged
        assert_eq!(resolved.deps.len(), 2);
        assert!(resolved.deps.contains_key("react"));
        assert!(resolved.deps.contains_key("clsx"));
    }

    #[test]
    fn test_missing_preset_errors() {
        let presets = IndexMap::new();
        let mut app = empty_project("test");
        app.preset = Some(PresetRef::Single("nonexistent".to_string()));

        let result = resolve_app_presets(&app, &presets, &IndexMap::new());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not defined in [preset.*]")
        );
    }

    #[test]
    fn test_resolve_profile_vars() {
        let profile = ProfileSection {
            branch: Some("stg".to_string()),
            env_source: crate::manifest::EnvSource::Simple("dotenv".to_string()),
            domain: "stg.agiletec.net".to_string(),
            node_env: "production".to_string(),
            compose_profiles: vec![],
            inherits: None,
            role: None,
        };

        assert_eq!(
            resolve_profile_vars("dashboard.{profile.domain}", &profile),
            "dashboard.stg.agiletec.net"
        );

        // $VAR stays as-is
        assert_eq!(
            resolve_profile_vars("$SUPABASE_URL", &profile),
            "$SUPABASE_URL"
        );
    }

    #[test]
    fn test_dep_groups_expansion() {
        let presets = IndexMap::new();
        let mut dep_groups = IndexMap::new();

        let mut shadcn = IndexMap::new();
        shadcn.insert("@radix-ui/react-dialog".to_string(), "catalog".to_string());
        shadcn.insert("@radix-ui/react-slot".to_string(), "catalog".to_string());
        shadcn.insert("clsx".to_string(), "catalog".to_string());
        dep_groups.insert("shadcn".to_string(), shadcn);

        let mut app = empty_project("corporate");
        app.dep_groups = vec!["shadcn".to_string()];
        app.deps.insert("stripe".to_string(), "catalog".to_string());

        let resolved = resolve_app_presets(&app, &presets, &dep_groups).unwrap();

        // dep_group deps + app direct deps
        assert_eq!(resolved.deps.len(), 4);
        assert!(resolved.deps.contains_key("@radix-ui/react-dialog"));
        assert!(resolved.deps.contains_key("clsx"));
        assert!(resolved.deps.contains_key("stripe"));
    }

    #[test]
    fn test_preset_dep_groups() {
        let mut presets = IndexMap::new();
        let mut dep_groups = IndexMap::new();

        let mut nextjs_base = IndexMap::new();
        nextjs_base.insert("next".to_string(), "catalog".to_string());
        nextjs_base.insert("react".to_string(), "catalog".to_string());
        dep_groups.insert("nextjs-base".to_string(), nextjs_base);

        presets.insert(
            "nextjs-app".to_string(),
            PresetSection {
                framework: Some("nextjs".to_string()),
                private: Some(true),
                scripts: IndexMap::new(),
                deps: IndexMap::new(),
                dev_deps: IndexMap::new(),
                dep_groups: vec!["nextjs-base".to_string()],
                dev_dep_groups: vec![],
                scope: None,
                deploy: None,
            },
        );

        let mut app = empty_project("myapp");
        app.preset = Some(crate::manifest::PresetRef::Single("nextjs-app".to_string()));

        let resolved = resolve_app_presets(&app, &presets, &dep_groups).unwrap();

        assert!(resolved.deps.contains_key("next"));
        assert!(resolved.deps.contains_key("react"));
        assert_eq!(resolved.framework.as_deref(), Some("nextjs"));
    }

    #[test]
    fn test_missing_dep_group_errors() {
        let presets = IndexMap::new();
        let dep_groups = IndexMap::new();

        let mut app = empty_project("test");
        app.dep_groups = vec!["nonexistent".to_string()];

        let result = resolve_app_presets(&app, &presets, &dep_groups);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not defined in [dep_group.*]")
        );
    }
}
