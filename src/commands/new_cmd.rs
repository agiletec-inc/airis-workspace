//! New command: scaffold new apps, services, and libraries from templates

use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

use crate::manifest::{Manifest, MANIFEST_FILE};

/// Template types available for scaffolding (legacy - kept for compatibility)
#[derive(Debug, Clone)]
pub enum TemplateType {
    /// Hono-based TypeScript API
    Api,
    /// Next.js web application
    Web,
    /// TypeScript library
    Lib,
    /// Rust service
    RustService,
    /// Python FastAPI service
    PyApi,
}

impl TemplateType {
    fn base_dir(&self) -> &str {
        match self {
            TemplateType::Api => "apps",
            TemplateType::Web => "apps",
            TemplateType::Lib => "libs",
            TemplateType::RustService => "apps",
            TemplateType::PyApi => "apps",
        }
    }

    fn display_name(&self) -> &str {
        match self {
            TemplateType::Api => "Hono API",
            TemplateType::Web => "Next.js Web App",
            TemplateType::Lib => "TypeScript Library",
            TemplateType::RustService => "Rust Service",
            TemplateType::PyApi => "Python FastAPI",
        }
    }
}

/// Get the base directory for a template category
fn get_base_dir(category: &str) -> &str {
    match category {
        "api" | "web" | "worker" | "cli" => "apps",
        "lib" => "libs",
        "edge" | "supabase-trigger" | "supabase-realtime" => "supabase/functions",
        _ => "apps",
    }
}

/// Resolve runtime alias to full runtime name
fn resolve_runtime(manifest: &Manifest, runtime: &str) -> String {
    manifest.runtimes.alias
        .get(runtime)
        .cloned()
        .unwrap_or_else(|| runtime.to_string())
}

/// Run the new command with runtime selection
pub fn run_with_runtime(category: &str, name: &str, runtime: &str) -> Result<()> {
    // Validate name
    if name.is_empty() {
        bail!("Project name cannot be empty");
    }

    if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        bail!("Project name can only contain alphanumeric characters, hyphens, and underscores");
    }

    // Load manifest if exists (for runtime aliases)
    let manifest = if Path::new(MANIFEST_FILE).exists() {
        Some(Manifest::load(MANIFEST_FILE)?)
    } else {
        None
    };

    // Resolve runtime alias
    let resolved_runtime = if let Some(ref m) = manifest {
        resolve_runtime(m, runtime)
    } else {
        runtime.to_string()
    };

    let base_dir = get_base_dir(category);
    let project_dir = Path::new(base_dir).join(name);

    // Check if directory already exists
    if project_dir.exists() {
        bail!(
            "Directory {} already exists. Choose a different name.",
            project_dir.display()
        );
    }

    // Ensure base directory exists
    if !Path::new(base_dir).exists() {
        fs::create_dir_all(base_dir)
            .with_context(|| format!("Failed to create {} directory", base_dir))?;
    }

    let display_name = format!("{} ({})", category, resolved_runtime);
    println!(
        "{} {} at {}",
        "Creating".bright_blue(),
        display_name,
        project_dir.display().to_string().cyan()
    );

    // Generate project based on category and runtime
    match (category, resolved_runtime.as_str()) {
        ("api", "hono") => generate_api_project(&project_dir, name)?,
        ("api", "fastapi") => generate_py_api(&project_dir, name)?,
        ("api", "rust-axum") => generate_rust_service(&project_dir, name)?,
        ("web", "nextjs") => generate_web_project(&project_dir, name)?,
        ("lib", "ts") => generate_lib_project(&project_dir, name)?,
        ("edge", "deno") => generate_edge_function(&project_dir, name)?,
        ("supabase-trigger", "plpgsql") => generate_supabase_trigger(&project_dir, name)?,
        ("supabase-realtime", "deno") => generate_supabase_realtime(&project_dir, name)?,
        _ => {
            bail!(
                "Unknown runtime '{}' for category '{}'. Available runtimes:\n  \
                api: hono, fastapi, rust-axum\n  \
                web: nextjs\n  \
                lib: ts\n  \
                edge: deno\n  \
                supabase-trigger: plpgsql\n  \
                supabase-realtime: deno",
                resolved_runtime, category
            );
        }
    }

    println!();
    println!("{}", "✅ Project created successfully!".green());
    println!();
    println!("{}", "Next steps:".bright_yellow());
    println!("  1. Run {} to regenerate workspace files", "airis init".cyan());
    println!("  2. Run {} to install dependencies", "airis install".cyan());
    println!("  3. Start development with {}", "airis dev".cyan());

    Ok(())
}

/// Run the new command to scaffold a project (legacy interface)
pub fn run(template_type: TemplateType, name: &str) -> Result<()> {
    let (category, runtime) = match template_type {
        TemplateType::Api => ("api", "hono"),
        TemplateType::Web => ("web", "nextjs"),
        TemplateType::Lib => ("lib", "ts"),
        TemplateType::RustService => ("api", "rust-axum"),
        TemplateType::PyApi => ("api", "fastapi"),
    };
    run_with_runtime(category, name, runtime)
}

/// Generate a Hono API project
fn generate_api_project(project_dir: &Path, name: &str) -> Result<()> {
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
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "lib": ["ES2022"],
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
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

    // Dockerfile
    let dockerfile = format!(
        r#"FROM node:22-alpine AS builder
WORKDIR /app
COPY package.json pnpm-lock.yaml ./
RUN corepack enable && pnpm install --frozen-lockfile
COPY . .
RUN pnpm build

FROM node:22-alpine
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

/// Generate a Next.js web project
fn generate_web_project(project_dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(project_dir.join("src/app"))
        .context("Failed to create src/app directory")?;

    // package.json
    let package_json = format!(
        r#"{{
  "name": "{}",
  "version": "0.1.0",
  "private": true,
  "scripts": {{
    "dev": "next dev",
    "build": "next build",
    "start": "next start",
    "lint": "next lint"
  }},
  "dependencies": {{
    "next": "catalog:",
    "react": "catalog:",
    "react-dom": "catalog:"
  }},
  "devDependencies": {{
    "typescript": "catalog:",
    "@types/node": "catalog:",
    "@types/react": "catalog:",
    "@types/react-dom": "catalog:"
  }}
}}
"#,
        name
    );
    fs::write(project_dir.join("package.json"), package_json)?;

    // next.config.js
    let next_config = r#"/** @type {import('next').NextConfig} */
const nextConfig = {
  output: 'standalone',
}

module.exports = nextConfig
"#;
    fs::write(project_dir.join("next.config.js"), next_config)?;

    // tsconfig.json
    let tsconfig = r#"{
  "compilerOptions": {
    "target": "ES2017",
    "lib": ["dom", "dom.iterable", "esnext"],
    "allowJs": true,
    "skipLibCheck": true,
    "strict": true,
    "noEmit": true,
    "esModuleInterop": true,
    "module": "esnext",
    "moduleResolution": "bundler",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "jsx": "preserve",
    "incremental": true,
    "plugins": [{ "name": "next" }],
    "paths": { "@/*": ["./src/*"] }
  },
  "include": ["next-env.d.ts", "**/*.ts", "**/*.tsx", ".next/types/**/*.ts"],
  "exclude": ["node_modules"]
}
"#;
    fs::write(project_dir.join("tsconfig.json"), tsconfig)?;

    // src/app/layout.tsx
    let layout = format!(
        r#"export const metadata = {{
  title: '{}',
  description: 'Generated by airis new',
}}

export default function RootLayout({{
  children,
}}: {{
  children: React.ReactNode
}}) {{
  return (
    <html lang="en">
      <body>{{children}}</body>
    </html>
  )
}}
"#,
        name
    );
    fs::write(project_dir.join("src/app/layout.tsx"), layout)?;

    // src/app/page.tsx
    let page = format!(
        r#"export default function Home() {{
  return (
    <main style={{ padding: '2rem' }}>
      <h1>{}</h1>
      <p>Welcome to your new Next.js app!</p>
    </main>
  )
}}
"#,
        name
    );
    fs::write(project_dir.join("src/app/page.tsx"), page)?;

    // .gitignore
    let gitignore = r#"node_modules/
.next/
out/
*.log
.env
.env.local
"#;
    fs::write(project_dir.join(".gitignore"), gitignore)?;

    println!("  {} package.json", "✓".green());
    println!("  {} next.config.js", "✓".green());
    println!("  {} tsconfig.json", "✓".green());
    println!("  {} src/app/layout.tsx", "✓".green());
    println!("  {} src/app/page.tsx", "✓".green());
    println!("  {} .gitignore", "✓".green());

    Ok(())
}

/// Generate a TypeScript library
fn generate_lib_project(project_dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(project_dir.join("src"))
        .context("Failed to create src directory")?;

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
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "lib": ["ES2022"],
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
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

/// Generate a Rust service
fn generate_rust_service(project_dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(project_dir.join("src"))
        .context("Failed to create src directory")?;

    // Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2024"

[dependencies]
tokio = {{ version = "1", features = ["full"] }}
axum = "0.8"
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
anyhow = "1"
"#,
        name.replace('-', "_")
    );
    fs::write(project_dir.join("Cargo.toml"), cargo_toml)?;

    // src/main.rs
    let main_rs = format!(
        r#"use axum::{{
    routing::get,
    Json, Router,
}};
use serde::Serialize;
use std::net::SocketAddr;

#[derive(Serialize)]
struct Health {{
    status: String,
    service: String,
}}

async fn health() -> Json<Health> {{
    Json(Health {{
        status: "ok".to_string(),
        service: "{}".to_string(),
    }})
}}

#[tokio::main]
async fn main() -> anyhow::Result<()> {{
    tracing_subscriber::init();

    let app = Router::new()
        .route("/health", get(health))
        .route("/", get(|| async {{ "Hello from {}!" }}));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("listening on {{}}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}}
"#,
        name, name
    );
    fs::write(project_dir.join("src/main.rs"), main_rs)?;

    // Dockerfile
    let dockerfile = format!(
        r#"FROM rust:1.82-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /app
COPY . .
RUN cargo build --release

FROM alpine:3.19
WORKDIR /app
COPY --from=builder /app/target/release/{} /app/
ENV RUST_LOG=info
EXPOSE 3000
CMD ["./{}"]
"#,
        name.replace('-', "_"),
        name.replace('-', "_")
    );
    fs::write(project_dir.join("Dockerfile"), dockerfile)?;

    // .gitignore
    let gitignore = r#"target/
Cargo.lock
"#;
    fs::write(project_dir.join(".gitignore"), gitignore)?;

    println!("  {} Cargo.toml", "✓".green());
    println!("  {} src/main.rs", "✓".green());
    println!("  {} Dockerfile", "✓".green());
    println!("  {} .gitignore", "✓".green());

    Ok(())
}

/// Generate a Python FastAPI service
fn generate_py_api(project_dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(project_dir.join("app"))
        .context("Failed to create app directory")?;

    // pyproject.toml
    let pyproject = format!(
        r#"[project]
name = "{}"
version = "0.1.0"
description = "A FastAPI service"
requires-python = ">=3.11"
dependencies = [
    "fastapi>=0.109.0",
    "uvicorn[standard]>=0.27.0",
    "pydantic>=2.0.0",
]

[project.optional-dependencies]
dev = [
    "pytest>=7.0.0",
    "httpx>=0.26.0",
    "ruff>=0.1.0",
]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"
"#,
        name
    );
    fs::write(project_dir.join("pyproject.toml"), pyproject)?;

    // app/main.py
    let main_py = format!(
        r#"from fastapi import FastAPI
from datetime import datetime

app = FastAPI(title="{}", version="0.1.0")


@app.get("/")
async def root():
    return {{"message": "Welcome to {}"}}


@app.get("/health")
async def health():
    return {{
        "status": "ok",
        "timestamp": datetime.utcnow().isoformat(),
    }}
"#,
        name, name
    );
    fs::write(project_dir.join("app/main.py"), main_py)?;

    // app/__init__.py
    fs::write(project_dir.join("app/__init__.py"), "")?;

    // Dockerfile
    let dockerfile = r#"FROM python:3.12-slim

WORKDIR /app

# Install uv for faster installs
RUN pip install uv

COPY pyproject.toml ./
RUN uv pip install --system -e .

COPY . .

EXPOSE 8000
CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "8000"]
"#;
    fs::write(project_dir.join("Dockerfile"), dockerfile)?;

    // .gitignore
    let gitignore = r#"__pycache__/
*.py[cod]
*$py.class
.venv/
.env
dist/
*.egg-info/
"#;
    fs::write(project_dir.join(".gitignore"), gitignore)?;

    println!("  {} pyproject.toml", "✓".green());
    println!("  {} app/main.py", "✓".green());
    println!("  {} app/__init__.py", "✓".green());
    println!("  {} Dockerfile", "✓".green());
    println!("  {} .gitignore", "✓".green());

    Ok(())
}

/// Generate a Supabase Edge Function
fn generate_edge_function(project_dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(project_dir)
        .context("Failed to create edge function directory")?;

    // index.ts - main edge function
    let index_ts = format!(
        r#"// {} - Supabase Edge Function
// Follow this setup guide to integrate the Deno language server with your editor:
// https://deno.land/manual/getting_started/setup_your_environment

import {{ corsHeaders }} from '../_shared/cors.ts'

console.log('Function "{}" up and running!')

Deno.serve(async (req) => {{
  // Handle CORS preflight
  if (req.method === 'OPTIONS') {{
    return new Response('ok', {{ headers: corsHeaders }})
  }}

  try {{
    const {{ name }} = await req.json()
    const data = {{
      message: `Hello ${{name || 'World'}}!`,
      timestamp: new Date().toISOString(),
      function: '{}',
    }}

    return new Response(JSON.stringify(data), {{
      headers: {{ ...corsHeaders, 'Content-Type': 'application/json' }},
      status: 200,
    }})
  }} catch (error) {{
    return new Response(JSON.stringify({{ error: error.message }}), {{
      headers: {{ ...corsHeaders, 'Content-Type': 'application/json' }},
      status: 400,
    }})
  }}
}})
"#,
        name, name, name
    );
    fs::write(project_dir.join("index.ts"), index_ts)?;

    println!("  {} index.ts", "✓".green());

    Ok(())
}

/// Generate a Supabase database trigger migration
fn generate_supabase_trigger(project_dir: &Path, name: &str) -> Result<()> {
    // For triggers, we create a migration file instead of a function directory
    let migrations_dir = Path::new("supabase/migrations");
    if !migrations_dir.exists() {
        fs::create_dir_all(migrations_dir)
            .context("Failed to create migrations directory")?;
    }

    // Generate timestamp for migration
    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
    let migration_file = migrations_dir.join(format!("{}_{}.sql", timestamp, name));

    let snake_name = name.replace('-', "_");
    let migration_sql = format!(
        r#"-- Migration: {}
-- Description: Database trigger for {}

-- Create the trigger function
CREATE OR REPLACE FUNCTION {}()
RETURNS TRIGGER AS $$
BEGIN
  -- Your trigger logic here
  -- Example: notify via pg_net
  -- PERFORM net.http_post(
  --   url := 'https://your-api.com/webhook',
  --   headers := '{{"Content-Type": "application/json"}}'::jsonb,
  --   body := jsonb_build_object(
  --     'table', TG_TABLE_NAME,
  --     'operation', TG_OP,
  --     'record', NEW
  --   )
  -- );

  RETURN NEW;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Create the trigger (uncomment and modify as needed)
-- CREATE TRIGGER {}_trigger
--   AFTER INSERT OR UPDATE ON your_table
--   FOR EACH ROW
--   EXECUTE FUNCTION {}();

-- Grant necessary permissions
-- GRANT EXECUTE ON FUNCTION {}() TO service_role;
"#,
        name, name, snake_name, snake_name, snake_name, snake_name
    );
    fs::write(&migration_file, migration_sql)?;

    println!("  {} {}", "✓".green(), migration_file.display());

    // Create empty function directory for consistency
    fs::create_dir_all(project_dir)
        .context("Failed to create function directory")?;

    let readme = format!(
        r#"# {}

This is a database trigger. The main logic is in the migration file:
`supabase/migrations/{}_{}.sql`

## Usage

1. Edit the migration file to add your trigger logic
2. Apply the migration with `supabase db push`
3. The trigger will fire automatically on database events

## pg_net Integration

To call external APIs from triggers, use pg_net:

```sql
PERFORM net.http_post(
  url := 'https://your-api.com/webhook',
  headers := '{{"Content-Type": "application/json"}}'::jsonb,
  body := jsonb_build_object('data', NEW)
);
```
"#,
        name, timestamp, name
    );
    fs::write(project_dir.join("README.md"), readme)?;

    println!("  {} README.md", "✓".green());

    Ok(())
}

/// Generate a Supabase Realtime handler
fn generate_supabase_realtime(project_dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(project_dir)
        .context("Failed to create realtime function directory")?;

    // index.ts - realtime broadcast/presence handler
    let index_ts = format!(
        r#"// {} - Supabase Realtime Handler
// Handles broadcast messages and presence state

import {{ createClient }} from 'https://esm.sh/@supabase/supabase-js@2'
import {{ corsHeaders }} from '../_shared/cors.ts'

const supabaseUrl = Deno.env.get('SUPABASE_URL')!
const supabaseServiceKey = Deno.env.get('SUPABASE_SERVICE_ROLE_KEY')!

console.log('Realtime handler "{}" up and running!')

Deno.serve(async (req) => {{
  // Handle CORS preflight
  if (req.method === 'OPTIONS') {{
    return new Response('ok', {{ headers: corsHeaders }})
  }}

  try {{
    const supabase = createClient(supabaseUrl, supabaseServiceKey)
    const {{ channel, event, payload }} = await req.json()

    // Get or create channel
    const realtimeChannel = supabase.channel(channel || '{}')

    // Subscribe and send broadcast
    await new Promise<void>((resolve, reject) => {{
      realtimeChannel
        .on('broadcast', {{ event: event || 'message' }}, (message) => {{
          console.log('Received:', message)
        }})
        .subscribe(async (status) => {{
          if (status === 'SUBSCRIBED') {{
            // Send broadcast message
            await realtimeChannel.send({{
              type: 'broadcast',
              event: event || 'message',
              payload: payload || {{ message: 'Hello from {}!' }},
            }})
            resolve()
          }} else if (status === 'CHANNEL_ERROR') {{
            reject(new Error('Channel subscription failed'))
          }}
        }})
    }})

    // Cleanup
    await supabase.removeChannel(realtimeChannel)

    return new Response(
      JSON.stringify({{
        success: true,
        channel,
        event: event || 'message',
        timestamp: new Date().toISOString(),
      }}),
      {{
        headers: {{ ...corsHeaders, 'Content-Type': 'application/json' }},
        status: 200,
      }}
    )
  }} catch (error) {{
    return new Response(
      JSON.stringify({{ error: error.message }}),
      {{
        headers: {{ ...corsHeaders, 'Content-Type': 'application/json' }},
        status: 500,
      }}
    )
  }}
}})
"#,
        name, name, name, name
    );
    fs::write(project_dir.join("index.ts"), index_ts)?;

    println!("  {} index.ts", "✓".green());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_template_type_base_dir() {
        assert_eq!(TemplateType::Api.base_dir(), "apps");
        assert_eq!(TemplateType::Web.base_dir(), "apps");
        assert_eq!(TemplateType::Lib.base_dir(), "libs");
        assert_eq!(TemplateType::RustService.base_dir(), "apps");
        assert_eq!(TemplateType::PyApi.base_dir(), "apps");
    }

    #[test]
    fn test_empty_name_rejected() {
        let result = run(TemplateType::Api, "");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_invalid_name_rejected() {
        let result = run(TemplateType::Api, "my app");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("alphanumeric"));
    }

    #[test]
    fn test_generate_api_project() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("test-api");

        generate_api_project(&project_dir, "test-api").unwrap();

        assert!(project_dir.join("package.json").exists());
        assert!(project_dir.join("tsconfig.json").exists());
        assert!(project_dir.join("src/index.ts").exists());
        assert!(project_dir.join("src/routes/health.ts").exists());
        assert!(project_dir.join("Dockerfile").exists());
    }

    #[test]
    fn test_generate_lib_project() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("test-lib");

        generate_lib_project(&project_dir, "test-lib").unwrap();

        assert!(project_dir.join("package.json").exists());
        assert!(project_dir.join("tsconfig.json").exists());
        assert!(project_dir.join("src/index.ts").exists());
    }

    #[test]
    fn test_generate_rust_service() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("test-rust");

        generate_rust_service(&project_dir, "test-rust").unwrap();

        assert!(project_dir.join("Cargo.toml").exists());
        assert!(project_dir.join("src/main.rs").exists());
        assert!(project_dir.join("Dockerfile").exists());
    }

    #[test]
    fn test_generate_py_api() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("test-py");

        generate_py_api(&project_dir, "test-py").unwrap();

        assert!(project_dir.join("pyproject.toml").exists());
        assert!(project_dir.join("app/main.py").exists());
        assert!(project_dir.join("Dockerfile").exists());
    }
}
