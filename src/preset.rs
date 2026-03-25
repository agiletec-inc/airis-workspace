//! Preset resolution and profile variable expansion for manifest v2.
//!
//! Presets provide reusable deps/scripts/deploy defaults for [[app]] definitions.
//! Profile variables (`{profile.domain}`) are resolved at `airis gen` time.

use anyhow::{Context, Result, bail};
use indexmap::IndexMap;

use crate::manifest::{
    AppDeployConfig, PresetSection, ProfileSection, ProjectDefinition,
};

/// Resolved app with preset deps/scripts merged in.
/// App values always override preset values.
#[derive(Debug)]
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
pub fn resolve_app_presets(
    app: &ProjectDefinition,
    presets: &IndexMap<String, PresetSection>,
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

            // Merge deps (preset values, app overrides later)
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
            if deploy.is_none() {
                if let Some(ref defaults) = preset.deploy {
                    deploy = Some(AppDeployConfig {
                        enabled: true,
                        variant: defaults.variant.clone(),
                        port: defaults.port,
                        health_path: defaults
                            .health_path
                            .clone()
                            .unwrap_or_else(|| "/health".to_string()),
                        ..Default::default()
                    });
                }
            }
        }
    }

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

/// Resolve all apps' presets.
pub fn resolve_all_presets(
    apps: &[ProjectDefinition],
    presets: &IndexMap<String, PresetSection>,
) -> Result<Vec<ResolvedApp>> {
    apps.iter()
        .map(|app| resolve_app_presets(app, presets))
        .collect()
}

/// Resolve `{profile.*}` placeholders in a string.
/// E.g., "{profile.domain}" → "stg.agiletec.net"
/// `$VAR` patterns are left as-is (runtime variables).
pub fn resolve_profile_vars(template: &str, profile: &ProfileSection) -> String {
    template
        .replace("{profile.domain}", &profile.domain)
        .replace("{profile.node_env}", &profile.node_env)
}

/// Resolve a profile with inheritance.
/// If profile has `inherits`, merge parent first, then child overrides.
pub fn resolve_profile<'a>(
    name: &str,
    profiles: &'a IndexMap<String, ProfileSection>,
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
            env_source: if matches!(profile.env_source, crate::manifest::EnvSource::Simple(ref s) if s == "dotenv") {
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
            },
        );

        let mut app = empty_project("corporate");
        app.preset = Some(PresetRef::Single("nextjs-app".to_string()));

        // Add app-specific dep
        app.deps
            .insert("stripe".to_string(), "catalog".to_string());

        let resolved = resolve_app_presets(&app, &presets).unwrap();

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
            },
        );

        let mut app = empty_project("dashboard");
        app.preset = Some(PresetRef::Single("ts-lib".to_string()));
        app.scripts.insert(
            "typecheck".to_string(),
            "tsc -p tsconfig.json --noEmit".to_string(),
        );

        let resolved = resolve_app_presets(&app, &presets).unwrap();

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
            },
        );

        let mut app = empty_project("ui");
        app.preset = Some(PresetRef::Multiple(vec![
            "base".to_string(),
            "shadcn".to_string(),
        ]));

        let resolved = resolve_app_presets(&app, &presets).unwrap();

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

        let result = resolve_app_presets(&app, &presets);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not defined in [preset.*]"));
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
}
