//! Rust service scaffolding (Axum)

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Generate a Rust service
pub fn generate_rust_service(project_dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(project_dir.join("src")).context("Failed to create src directory")?;

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
    let rust_image = crate::channel::defaults::RUST_IMAGE;
    let alpine_image = crate::channel::defaults::ALPINE_IMAGE;
    let bin_name = name.replace('-', "_");
    let dockerfile = format!(
        r#"FROM {rust_image} AS builder
RUN apt-get update && apt-get install -y --no-install-recommends musl-tools && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
RUN cargo build --release

FROM {alpine_image}
WORKDIR /app
COPY --from=builder /app/target/release/{bin_name} /app/
ENV RUST_LOG=info
EXPOSE 3000
CMD ["./{bin_name}"]
"#
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
