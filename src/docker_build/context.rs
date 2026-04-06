//! Docker build context builder

use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};

use crate::dag::Dag;
use crate::pnpm::PnpmLock;

/// Context builder - creates minimal Docker build context
pub struct ContextBuilder<'a> {
    root: &'a Path,
    dag: &'a Dag,
    #[allow(dead_code)]
    lock: &'a PnpmLock,
    target: &'a str,
}

impl<'a> ContextBuilder<'a> {
    pub fn new(root: &'a Path, dag: &'a Dag, lock: &'a PnpmLock, target: &'a str) -> Self {
        Self {
            root,
            dag,
            lock,
            target,
        }
    }

    /// Build minimal context directory
    /// Returns path to context dir (temp or specified)
    pub fn build(&self, out_dir: Option<&Path>) -> Result<PathBuf> {
        // Get dependency order
        let dep_paths = self.dag.get_dep_paths(self.target)?;

        // Create context directory
        let ctx_dir = match out_dir {
            Some(p) => {
                fs::create_dir_all(p)?;
                p.to_path_buf()
            }
            None => {
                let temp = tempfile::tempdir()?;
                let path = temp.path().to_path_buf();
                // Keep the temp directory alive (don't delete on drop)
                std::mem::forget(temp);
                path
            }
        };

        println!(
            "📦 Building context for {} ({} packages)",
            self.target,
            dep_paths.len()
        );

        // 1. Copy root files
        self.copy_root_files(&ctx_dir)?;

        // 2. Copy each dependency in order
        for dep_path in &dep_paths {
            self.copy_package(&ctx_dir, dep_path)?;
        }

        // 3. Generate inputs manifest for hash verification
        self.write_inputs_manifest(&ctx_dir, &dep_paths)?;

        Ok(ctx_dir)
    }

    fn copy_root_files(&self, ctx: &Path) -> Result<()> {
        // Essential root files for pnpm workspace
        let root_files = [
            "package.json",
            "pnpm-lock.yaml",
            "pnpm-workspace.yaml",
            "tsconfig.base.json",
            "tsconfig.json",
        ];

        for file in &root_files {
            let src = self.root.join(file);
            if src.exists() {
                let dst = ctx.join(file);
                fs::copy(&src, &dst).with_context(|| format!("Failed to copy {}", file))?;
            }
        }

        Ok(())
    }

    fn copy_package(&self, ctx: &Path, pkg_path: &str) -> Result<()> {
        let src_dir = self.root.join(pkg_path);
        let dst_dir = ctx.join(pkg_path);

        if !src_dir.exists() {
            bail!("Package directory not found: {}", pkg_path);
        }

        fs::create_dir_all(&dst_dir)?;

        // Files to copy for each package
        let essential_files = [
            "package.json",
            "tsconfig.json",
            "tsconfig.build.json",
        ];

        for file in &essential_files {
            let src = src_dir.join(file);
            if src.exists() {
                fs::copy(&src, dst_dir.join(file))?;
            }
        }

        // Copy src/ directory
        let src_src = src_dir.join("src");
        if src_src.exists() {
            copy_dir_recursive(&src_src, &dst_dir.join("src"))?;
        }

        // Copy public/ directory (for Next.js apps)
        let src_public = src_dir.join("public");
        if src_public.exists() {
            copy_dir_recursive(&src_public, &dst_dir.join("public"))?;
        }

        // Copy config files (next.config.*, tailwind.config.*, tsup.config.*, postcss.config.*)
        for entry in fs::read_dir(&src_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("next.config")
                || name_str.starts_with("tailwind.config")
                || name_str.starts_with("tsup.config")
                || name_str.starts_with("postcss.config")
            {
                fs::copy(entry.path(), dst_dir.join(&name))?;
            }
        }

        Ok(())
    }

    fn write_inputs_manifest(&self, ctx: &Path, dep_paths: &[String]) -> Result<()> {
        let manifest = serde_json::json!({
            "target": self.target,
            "dependencies": dep_paths,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        let airis_dir = ctx.join(".airis");
        fs::create_dir_all(&airis_dir)?;
        fs::write(
            airis_dir.join("inputs.json"),
            serde_json::to_string_pretty(&manifest)?,
        )?;

        Ok(())
    }
}

/// Recursively copy directory
pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}
