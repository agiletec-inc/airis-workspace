//! Hono API project scaffolding

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Generate a Hono API project
pub fn generate_api_project(project_dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(project_dir.join("src/routes"))
        .context("Failed to create src/routes directory")?;

    // package.json
    let package_json = format!(
        r#"{{
  "name": "{}",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {{
    "dev": "tsx watch src/index.ts",
    "build": "tsup src/index.ts --format esm --dts",
    "start": "node dist/index.js",
    "test": "vitest",
    "lint": "biome check src/"
  }},
  "dependencies": {{
    "hono": "catalog:",
    "@hono/node-server": "catalog:"
  }},
  "devDependencies": {{
    "typescript": "catalog:",
    "tsx": "catalog:",
    "tsup": "catalog:",
    "vitest": "catalog:",
    "@types/node": "catalog:"
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
        r#"import {{ serve }} from '@hono/node-server'
import {{ Hono }} from 'hono'
import {{ logger }} from 'hono/logger'
import {{ cors }} from 'hono/cors'
import {{ health }} from './routes/health'

const app = new Hono()

// Middleware
app.use('*', logger())
app.use('*', cors())

// Routes
app.route('/health', health)

app.get('/', (c) => {{
  return c.json({{ message: 'Welcome to {}' }})
}})

const port = parseInt(process.env.PORT || '3000', 10)

console.log(`Server is running on port ${{port}}`)

serve({{
  fetch: app.fetch,
  port,
}})

export default app
"#,
        name
    );
    fs::write(project_dir.join("src/index.ts"), index_ts)?;

    // src/routes/health.ts
    let health_ts = r#"import { Hono } from 'hono'

export const health = new Hono()

health.get('/', (c) => {
  return c.json({
    status: 'ok',
    timestamp: new Date().toISOString(),
  })
})
"#;
    fs::write(project_dir.join("src/routes/health.ts"), health_ts)?;

    // Dockerfile — pnpm installed without version pin (scaffold = fresh project)
    let node_image = crate::channel::defaults::NODE_LTS_IMAGE;
    let dockerfile = format!(
        r#"FROM {node_image} AS builder
WORKDIR /app
COPY package.json ./
RUN npm install -g pnpm && pnpm install
COPY . .
RUN pnpm build

FROM {node_image}
WORKDIR /app
COPY --from=builder /app/dist ./dist
COPY --from=builder /app/package.json ./
COPY --from=builder /app/node_modules ./node_modules
ENV NODE_ENV=production
EXPOSE 3000
CMD ["node", "dist/index.js"]
"#
    );
    fs::write(project_dir.join("Dockerfile"), dockerfile)?;

    // .gitignore
    let gitignore = r#"node_modules/
dist/
*.log
.env
.env.local
"#;
    fs::write(project_dir.join(".gitignore"), gitignore)?;

    // README.md
    let readme = format!(
        r#"# {}

A Hono-based API service.

## Development

```bash
# Install dependencies
pnpm install

# Start development server
pnpm dev

# Build for production
pnpm build

# Run tests
pnpm test
```

## API Endpoints

- `GET /` - Welcome message
- `GET /health` - Health check
"#,
        name
    );
    fs::write(project_dir.join("README.md"), readme)?;

    println!("  {} package.json", "✓".green());
    println!("  {} tsconfig.json", "✓".green());
    println!("  {} src/index.ts", "✓".green());
    println!("  {} src/routes/health.ts", "✓".green());
    println!("  {} Dockerfile", "✓".green());
    println!("  {} .gitignore", "✓".green());
    println!("  {} README.md", "✓".green());

    Ok(())
}
