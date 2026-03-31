//! TypeScript library scaffolding

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Generate a TypeScript library
pub fn generate_lib_project(project_dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(project_dir.join("src")).context("Failed to create src directory")?;

    // package.json
    let package_json = format!(
        r#"{{
  "name": "{}",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "main": "./dist/index.js",
  "types": "./dist/index.d.ts",
  "exports": {{
    ".": {{
      "types": "./dist/index.d.ts",
      "import": "./dist/index.js"
    }}
  }},
  "scripts": {{
    "build": "tsup src/index.ts --format esm --dts",
    "dev": "tsup src/index.ts --format esm --dts --watch",
    "test": "vitest",
    "lint": "biome check src/"
  }},
  "devDependencies": {{
    "typescript": "catalog:",
    "tsup": "catalog:",
    "vitest": "catalog:"
  }}
}}
"#,
        name
    );
    fs::write(project_dir.join("package.json"), package_json)?;

    // tsconfig.json
    let tsconfig = r#"{
  "extends": "../../tsconfig.base.json",
  "compilerOptions": {
    "outDir": "./dist",
    "rootDir": "./src",
    "declaration": true,
    "declarationMap": true,
    "sourceMap": true
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist"]
}
"#;
    fs::write(project_dir.join("tsconfig.json"), tsconfig)?;

    // src/index.ts
    let index_ts = format!(
        r#"/**
 * {} - A TypeScript library
 */

export function hello(name: string): string {{
  return `Hello, ${{name}}!`
}}

export default {{ hello }}
"#,
        name
    );
    fs::write(project_dir.join("src/index.ts"), index_ts)?;

    // .gitignore
    let gitignore = r#"node_modules/
dist/
*.log
"#;
    fs::write(project_dir.join(".gitignore"), gitignore)?;

    println!("  {} package.json", "✓".green());
    println!("  {} tsconfig.json", "✓".green());
    println!("  {} src/index.ts", "✓".green());
    println!("  {} .gitignore", "✓".green());

    Ok(())
}
