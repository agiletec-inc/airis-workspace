//! Python project scaffolding (library and FastAPI)

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Generate a Python library (uv + hatchling, src layout)
pub fn generate_py_lib(project_dir: &Path, name: &str) -> Result<()> {
    let pkg_name = name.replace('-', "_");
    fs::create_dir_all(project_dir.join(format!("src/{}", pkg_name)))
        .context("Failed to create src directory")?;

    // pyproject.toml
    let pyproject = format!(
        r#"[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[project]
name = "{}"
version = "0.1.0"
description = ""
requires-python = ">=3.12"
dependencies = []

[project.optional-dependencies]
dev = [
    "pytest>=8.0.0",
    "pytest-asyncio>=0.24.0",
    "ruff>=0.8.0",
]

[tool.hatch.build.targets.wheel]
packages = ["src/{}"]

[tool.ruff]
target-version = "py312"
line-length = 100

[tool.ruff.lint]
select = ["E", "F", "I", "N", "W", "UP"]

[tool.pytest.ini_options]
asyncio_mode = "auto"
testpaths = ["tests"]
"#,
        name, pkg_name
    );
    fs::write(project_dir.join("pyproject.toml"), pyproject)?;

    // src/<pkg>/__init__.py
    let init_py = format!(
        r#""""{}"""
"#,
        name
    );
    fs::write(
        project_dir.join(format!("src/{}/__init__.py", pkg_name)),
        init_py,
    )?;

    // .gitignore
    let gitignore = r#"__pycache__/
*.py[cod]
*$py.class
.venv/
dist/
*.egg-info/
"#;
    fs::write(project_dir.join(".gitignore"), gitignore)?;

    println!("  {} pyproject.toml", "✓".green(),);
    println!("  {} src/{}/__init__.py", "✓".green(), pkg_name);
    println!("  {} .gitignore", "✓".green());

    Ok(())
}

/// Generate a Python FastAPI service
pub fn generate_py_api(project_dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(project_dir.join("app")).context("Failed to create app directory")?;

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
    let python_image = crate::channel::defaults::PYTHON_IMAGE;
    let dockerfile = format!(
        r#"FROM {python_image}

WORKDIR /app

# Install uv for faster installs
RUN pip install uv

COPY pyproject.toml ./
RUN uv pip install --system -e .

COPY . .

EXPOSE 8000
CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "8000"]
"#
    );
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
