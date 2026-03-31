//! Content hashing for Docker build cache invalidation

use anyhow::Result;
use std::fs;
use std::path::Path;

/// Compute content hash directly from source files (fast path for cache lookup)
/// This avoids building the full context directory when checking for cache hits
pub fn compute_content_hash(root: &Path, target: &str) -> Result<String> {
    use std::io::Read;
    use walkdir::WalkDir;

    let mut hasher = blake3::Hasher::new();

    // Hash essential root files
    let root_files = [
        "package.json",
        "pnpm-lock.yaml",
        "pnpm-workspace.yaml",
        "tsconfig.base.json",
        "tsconfig.json",
    ];

    for file in &root_files {
        let path = root.join(file);
        if path.exists() {
            hasher.update(file.as_bytes());
            let content = fs::read(&path)?;
            hasher.update(&content);
        }
    }

    // Hash target directory
    let target_dir = root.join(target);
    if target_dir.exists() {
        let mut files: Vec<_> = WalkDir::new(&target_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                let name = e.file_name().to_string_lossy();
                // Skip build artifacts and dependencies
                !name.contains("node_modules")
                    && !name.contains(".next")
                    && !name.contains("dist")
                    && !name.contains(".turbo")
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        files.sort();

        for path in files {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            hasher.update(rel.to_string_lossy().as_bytes());

            let mut file = fs::File::open(&path)?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            hasher.update(&buf);
        }
    }

    let hash = hasher.finalize();
    Ok(hash.to_hex()[..12].to_string())
}

/// Compute BLAKE3 hash of inputs
pub fn compute_hash(ctx_dir: &Path) -> Result<String> {
    use std::io::Read;
    use walkdir::WalkDir;

    let mut hasher = blake3::Hasher::new();

    // Hash all files in context (sorted for determinism)
    let mut files: Vec<_> = WalkDir::new(ctx_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .collect();

    files.sort();

    for path in files {
        // Hash relative path
        let rel = path.strip_prefix(ctx_dir).unwrap_or(&path);
        hasher.update(rel.to_string_lossy().as_bytes());

        // Hash content
        let mut file = fs::File::open(&path)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        hasher.update(&buf);
    }

    let hash = hasher.finalize();
    Ok(hash.to_hex()[..12].to_string())
}
