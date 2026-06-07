use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;

use crate::commands::migrate::{MigrationPlan, MigrationTask};
use crate::manifest::Manifest;

/// MCP Request
#[derive(Debug, Deserialize)]
struct McpRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    method: String,
    params: Option<Value>,
    id: Option<Value>,
}

/// MCP Response
#[derive(Debug, Serialize)]
struct McpResponse {
    jsonrpc: String,
    result: Option<Value>,
    error: Option<Value>,
    id: Option<Value>,
}

pub fn run() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut reader = stdin.lock();

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }

        let request: McpRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(_) => continue,
        };

        let response = handle_request(request)?;
        let response_json = serde_json::to_string(&response)?;
        stdout.write_all(response_json.as_bytes())?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }

    Ok(())
}

/// Server protocol version. Bump when the MCP spec we target changes; clients
/// negotiate down if needed.
const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

fn handle_request(request: McpRequest) -> Result<McpResponse> {
    let result = match request.method.as_str() {
        "initialize" => Some(json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {
                "tools": {
                    "listChanged": false
                },
                "resources": {
                    "subscribe": false,
                    "listChanged": false
                }
            },
            "serverInfo": {
                "name": "airis-workspace-mcp",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        "notifications/initialized" => None,
        "resources/list" => Some(handle_resources_list()?),
        "resources/read" => {
            let params = request.params.as_ref().context("Missing params")?;
            let uri = params["uri"].as_str().context("Missing uri")?;
            Some(handle_resources_read(uri)?)
        }
        "tools/list" => Some(json!({
            "tools": [
                {
                    "name": "workspace_init",
                    "description": "Initialize or sync manifest.toml with the current repository state. Detects existing apps, libs, and legacy docker-compose files (v1), proposing a normalized manifest.toml that follows the latest airis best practices and standardizes on compose.yaml (v2). After applying the proposed manifest with 'manifest_apply', it is highly recommended to run 'airis clean --purge --force' via shell to remove the legacy configuration files and complete the consolidation.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "workspace_cleanup",
                    "description": "Scan the workspace for legacy artifacts, orphaned backups, and unneeded temporary files. Use this to maintain environment hygiene after migrations or structural changes. It returns a list of files that should be cleaned up.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "workspace_discover",
                    "description": "Scan the workspace to detect current structural facts. Useful for gathering context before proposing manual manifest changes.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },

                {
                    "name": "manifest_validate",
                    "description": "Validate a proposed manifest.toml content without writing it to disk.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "manifest": {
                                "type": "string",
                                "description": "The proposed manifest.toml content"
                            }
                        },
                        "required": ["manifest"]
                    }
                },
                {
                    "name": "manifest_apply",
                    "description": "Write manifest.toml to disk and optionally run 'airis gen' to update the environment.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "manifest": {
                                "type": "string",
                                "description": "The manifest.toml content to apply"
                            },
                            "run_gen": {
                                "type": "boolean",
                                "description": "Whether to run 'airis gen' immediately after writing",
                                "default": true
                            }
                        },
                        "required": ["manifest"]
                    }
                },
                {
                    "name": "migration_execute",
                    "description": "Execute a list of physical migration tasks (moving files, creating directories).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "tasks": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "type": {
                                            "type": "string",
                                            "enum": ["create_directory", "move_file", "generate_manifest"]
                                        },
                                        "path": { "type": "string" },
                                        "from": { "type": "string" },
                                        "to": { "type": "string" }
                                    },
                                    "required": ["type"]
                                }
                            }
                        },
                        "required": ["tasks"]
                    }
                },
                {
                    "name": "workspace_gen",
                    "description": "Regenerate workspace files (package.json, pnpm-workspace.yaml, compose.yaml, CI workflows) from manifest.toml. Run after manifest_apply or manifest.toml edits to propagate changes. Equivalent to the 'airis gen' CLI command.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "dry_run": {
                                "type": "boolean",
                                "description": "Preview diffs without writing files",
                                "default": false
                            }
                        }
                    }
                },
                {
                    "name": "workspace_validate_all",
                    "description": "Run all workspace validation checks (manifest syntax, ports, Traefik networks, env vars, dependency architecture). Use to confirm manifest consistency after changes.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "workspace_doctor",
                    "description": "Diagnose workspace health and configuration drift. Read-only; returns actionable hints. Use when the user reports unexpected behavior before editing anything.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "workspace_verify",
                    "description": "Execute verification rules defined in manifest.toml [rule.verify] and app-specific stack rules inside the Docker workspace. Call after manifest changes, dependency updates, or when asked to confirm environment health.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "workspace_status",
                    "description": "Show running Docker services (equivalent to 'docker compose ps'). Use to confirm which containers are up before further actions.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "workspace_up",
                    "description": "Start the Docker workspace (airis up). Builds images if needed and starts all services. Call before any exec/run/test commands when the workspace is not running.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "workspace_down",
                    "description": "Stop all Docker services in the workspace (airis down).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "workspace_restart",
                    "description": "Restart one or all Docker services (airis restart [service]).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "service": {
                                "type": "string",
                                "description": "Service name to restart. Omit to restart all services."
                            }
                        }
                    }
                },
                {
                    "name": "workspace_logs",
                    "description": "Fetch Docker service logs (airis logs [service]). Use to debug runtime errors.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "service": {
                                "type": "string",
                                "description": "Service name. Omit for all services."
                            },
                            "tail": {
                                "type": "integer",
                                "description": "Number of log lines to return (default: 50).",
                                "default": 50
                            }
                        }
                    }
                },
                {
                    "name": "workspace_install",
                    "description": "Install dependencies inside the Docker workspace (airis install). Run after adding packages to package.json.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "workspace_run",
                    "description": "Run a task defined in manifest.toml [commands] or delegate to the Docker workspace (airis run <task>).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task": {
                                "type": "string",
                                "description": "Task name (e.g. build, migrate, seed)"
                            },
                            "extra_args": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Additional arguments forwarded to the task"
                            }
                        },
                        "required": ["task"]
                    }
                },
                {
                    "name": "workspace_exec",
                    "description": "Execute an arbitrary command inside the Docker workspace container (airis exec <cmd>). Auto-routes to the correct service based on the command's runtime.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cmd": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Command and arguments to run inside the container (e.g. [\"pnpm\", \"add\", \"zod\"])"
                            }
                        },
                        "required": ["cmd"]
                    }
                },
                {
                    "name": "workspace_test",
                    "description": "Run the test suite inside Docker (airis test). Equivalent to the project's test script.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "extra_args": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Additional arguments forwarded to the test runner"
                            }
                        }
                    }
                },
                {
                    "name": "workspace_lint",
                    "description": "Run linting inside Docker (airis lint).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "workspace_typecheck",
                    "description": "Run type checking inside Docker (airis typecheck).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "workspace_clean",
                    "description": "Remove build artifacts from the workspace (airis clean). Defaults to dry-run; pass force=true to actually delete.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "force": {
                                "type": "boolean",
                                "description": "Actually execute deletions (default: false = dry-run preview)",
                                "default": false
                            },
                            "purge": {
                                "type": "boolean",
                                "description": "Also remove legacy compose files and orphaned configs (requires manifest.toml)",
                                "default": false
                            }
                        }
                    }
                },
                {
                    "name": "guards_install",
                    "description": "Install airis command shims (airis guards install). With global=true installs global shims in ~/.airis/bin that intercept pnpm/npm/python outside Docker.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "global": {
                                "type": "boolean",
                                "description": "Install global shims in ~/.airis/bin (default: false = project-local)",
                                "default": false
                            }
                        }
                    }
                },
                {
                    "name": "guards_status",
                    "description": "Show current airis guard shim status (airis guards status).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "global": {
                                "type": "boolean",
                                "description": "Check global shims (~/.airis/bin) instead of project-local",
                                "default": false
                            }
                        }
                    }
                },
                {
                    "name": "guards_uninstall",
                    "description": "Remove airis guard shims (airis guards uninstall).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "global": {
                                "type": "boolean",
                                "description": "Remove global shims from ~/.airis/bin",
                                "default": false
                            }
                        }
                    }
                }
            ]
        })),
        "tools/call" => {
            let params = request.params.as_ref().context("Missing params")?;
            let name = params["name"].as_str().context("Missing tool name")?;
            let arguments = &params["arguments"];

            let tool_result = match name {
                "workspace_init" => handle_workspace_init()?,
                "workspace_cleanup" => handle_workspace_cleanup()?,
                "workspace_discover" => handle_workspace_discover()?,
                "manifest_validate" => handle_manifest_validate(arguments)?,
                "manifest_apply" => handle_manifest_apply(arguments)?,
                "migration_execute" => handle_migration_execute(arguments)?,
                "workspace_gen" => handle_workspace_gen(arguments)?,
                "workspace_validate_all" => handle_workspace_validate_all()?,
                "workspace_doctor" => handle_workspace_doctor()?,
                "workspace_verify" => handle_workspace_verify()?,
                "workspace_status" => handle_workspace_status()?,
                "workspace_up" => handle_workspace_up()?,
                "workspace_down" => handle_workspace_down()?,
                "workspace_restart" => handle_workspace_restart(arguments)?,
                "workspace_logs" => handle_workspace_logs(arguments)?,
                "workspace_install" => handle_workspace_install()?,
                "workspace_run" => handle_workspace_run(arguments)?,
                "workspace_exec" => handle_workspace_exec(arguments)?,
                "workspace_test" => handle_workspace_test(arguments)?,
                "workspace_lint" => handle_workspace_lint()?,
                "workspace_typecheck" => handle_workspace_typecheck()?,
                "workspace_clean" => handle_workspace_clean(arguments)?,
                "guards_install" => handle_guards_install(arguments)?,
                "guards_status" => handle_guards_status(arguments)?,
                "guards_uninstall" => handle_guards_uninstall(arguments)?,
                _ => json!({
                    "content": [
                        {
                            "type": "text",
                            "text": format!("Unknown tool: {}", name)
                        }
                    ],
                    "isError": true
                }),
            };
            Some(tool_result)
        }
        _ => None,
    };

    Ok(McpResponse {
        jsonrpc: "2.0".to_string(),
        result,
        error: None,
        id: request.id,
    })
}

fn handle_workspace_init() -> Result<Value> {
    // 1. Scan repo for facts
    let discovery = crate::commands::discover::run()?;

    // 2. Propose a manifest.toml based on those facts
    // This logic lives in the discover module or a new generator
    let proposed_manifest = crate::commands::discover::propose_manifest(&discovery)?;

    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": "Proposed manifest.toml based on repository scan:\n\n"
            },
            {
                "type": "text",
                "text": proposed_manifest
            },
            {
                "type": "text",
                "text": "\n\nReview this manifest and use 'manifest_apply' to save it and standardize on compose.yaml (V2)."
            }
        ]
    }))
}

fn handle_workspace_cleanup() -> Result<Value> {
    use glob::glob;

    let mut legacy_files = Vec::new();

    // 1. Old compose patterns
    let patterns = [
        "docker-compose.yml",
        "docker-compose.yaml",
        "docker-compose.override.yml",
        "compose.override.yml",
        "compose.override.yaml",
        "workspace/docker-compose.yml",
        "workspace/docker-compose.yaml",
        "workspace/compose.yml",
        "workspace/compose.yaml",
        "**/docker-compose.yml",
    ];

    for pattern in patterns {
        if let Ok(paths) = glob(pattern) {
            for entry in paths.flatten() {
                let path_str = entry.to_string_lossy().to_string();
                // Safety: Skip currently managed root compose
                if path_str != "compose.yaml" && path_str != "compose.yml" {
                    legacy_files.push(path_str);
                }
            }
        }
    }

    // 2. Orphaned backups
    if let Ok(paths) = glob(".airis/backups/*") {
        for entry in paths.flatten() {
            legacy_files.push(entry.to_string_lossy().to_string());
        }
    }

    legacy_files.sort();
    legacy_files.dedup();

    if legacy_files.is_empty() {
        return Ok(json!({
            "content": [
                {
                    "type": "text",
                    "text": "Workspace is already clean. No legacy artifacts found."
                }
            ]
        }));
    }

    let list = legacy_files.join("\n");
    let response = format!(
        "The following legacy artifacts and unneeded files were found:\n\n{}\n\nYou can use 'migration_execute' to remove these files, or run 'airis clean --purge --force' from the shell.",
        list
    );

    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": response
            }
        ]
    }))
}

fn handle_workspace_discover() -> Result<Value> {
    // Run full discovery using the actual discovery engine
    let discovered = crate::commands::discover::run()?;

    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": serde_json::to_string_pretty(&discovered)?
            }
        ]
    }))
}

fn handle_manifest_validate(arguments: &Value) -> Result<Value> {
    let content = arguments["manifest"]
        .as_str()
        .context("Missing manifest content")?;

    match Manifest::parse(content) {
        Ok(manifest) => match manifest.validate() {
            Ok(_) => Ok(json!({
                "content": [
                    {
                        "type": "text",
                        "text": "Manifest is valid."
                    }
                ]
            })),
            Err(e) => Ok(json!({
                "content": [
                    {
                        "type": "text",
                        "text": format!("Validation failed:\n{}", e)
                    }
                ],
                "isError": true
            })),
        },
        Err(e) => Ok(json!({
            "content": [
                {
                    "type": "text",
                    "text": format!("Parsing failed:\n{}", e)
                }
            ],
            "isError": true
        })),
    }
}

fn handle_manifest_apply(arguments: &Value) -> Result<Value> {
    let content = arguments["manifest"]
        .as_str()
        .context("Missing manifest content")?;
    let run_gen = arguments["run_gen"].as_bool().unwrap_or(true);

    // 1. Parse + validate before touching disk so an invalid manifest can
    //    never corrupt the workspace. `Manifest::parse` performs both TOML
    //    parsing and consistency validation.
    if let Err(e) = Manifest::parse(content) {
        return Ok(json!({
            "content": [
                {
                    "type": "text",
                    "text": format!("Invalid manifest, manifest.toml not modified:\n{:#}", e)
                }
            ],
            "isError": true
        }));
    }

    // 2. Write to disk
    std::fs::write("manifest.toml", content)?;

    let mut response_text = "Manifest written to manifest.toml.".to_string();

    // 3. Optionally run gen
    if run_gen {
        // Load the manifest we just wrote to ensure we're using the latest
        let _manifest = Manifest::load(Path::new("manifest.toml"))?;
        crate::commands::generate::run(false, false, false)?;
        response_text.push_str("\nEnvironment updated with 'airis gen'.");
    } else {
        response_text.push_str("\nRun 'airis gen' to update the environment.");
    }

    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": response_text
            }
        ]
    }))
}

fn handle_migration_execute(arguments: &Value) -> Result<Value> {
    let tasks: Vec<MigrationTask> = serde_json::from_value(arguments["tasks"].clone())?;

    // Create a plan from tasks. Discovery results are empty here as we are executing
    // pre-defined tasks from the AI.
    let plan = MigrationPlan {
        tasks,
        discovery: crate::commands::discover::DiscoveryResult {
            apps: vec![],
            libs: vec![],
            compose_files: vec![],
            catalog: indexmap::IndexMap::new(),
        },
    };

    let report = crate::commands::migrate::execute(&plan, false)?;

    let mut text = format!("Migration completed with {} steps.", report.completed.len());
    if !report.errors.is_empty() {
        text.push_str(&format!("\nErrors:\n{}", report.errors.join("\n")));
    }

    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "isError": !report.errors.is_empty()
    }))
}

/// Run the current airis binary as a subprocess so stdout from CLI handlers
/// is captured and cannot corrupt the stdio MCP protocol on our own stdout.
fn run_airis_subprocess(args: &[&str]) -> Result<Value> {
    let bin = std::env::current_exe().context("Failed to resolve airis binary path")?;
    let output = Command::new(&bin)
        .args(args)
        .output()
        .with_context(|| format!("Failed to execute '{} {}'", bin.display(), args.join(" ")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let success = output.status.success();

    let text = match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => format!(
            "airis {} exited with status {}",
            args.join(" "),
            output.status
        ),
        (false, true) => stdout,
        (true, false) => stderr,
        (false, false) => format!("{stdout}\n--- stderr ---\n{stderr}"),
    };

    Ok(json!({
        "content": [
            { "type": "text", "text": text }
        ],
        "isError": !success
    }))
}

fn run_airis_subprocess_dyn(args: Vec<String>) -> Result<Value> {
    let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_airis_subprocess(&str_args)
}

fn handle_workspace_gen(arguments: &Value) -> Result<Value> {
    let dry_run = arguments["dry_run"].as_bool().unwrap_or(false);
    let mut args: Vec<&str> = vec!["gen"];
    if dry_run {
        args.push("--dry-run");
    }
    run_airis_subprocess(&args)
}

fn handle_workspace_up() -> Result<Value> {
    run_airis_subprocess(&["up"])
}

fn handle_workspace_down() -> Result<Value> {
    run_airis_subprocess(&["down"])
}

fn handle_workspace_restart(arguments: &Value) -> Result<Value> {
    let mut args = vec!["restart".to_string()];
    if let Some(service) = arguments["service"].as_str() {
        args.push(service.to_string());
    }
    run_airis_subprocess_dyn(args)
}

fn handle_workspace_logs(arguments: &Value) -> Result<Value> {
    let tail = arguments["tail"].as_u64().unwrap_or(50);
    let tail_str = tail.to_string();
    let mut args = vec!["logs", "--tail", &tail_str];
    let service_owned;
    if let Some(service) = arguments["service"].as_str() {
        service_owned = service.to_string();
        args.push(&service_owned);
    }
    run_airis_subprocess(&args)
}

fn handle_workspace_install() -> Result<Value> {
    run_airis_subprocess(&["install"])
}

fn handle_workspace_run(arguments: &Value) -> Result<Value> {
    let task = arguments["task"].as_str().context("Missing task name")?;
    let mut args = vec!["run".to_string(), task.to_string()];
    if let Some(extra) = arguments["extra_args"].as_array() {
        for v in extra {
            if let Some(s) = v.as_str() {
                args.push(s.to_string());
            }
        }
    }
    run_airis_subprocess_dyn(args)
}

fn handle_workspace_exec(arguments: &Value) -> Result<Value> {
    let cmd = arguments["cmd"].as_array().context("Missing cmd array")?;
    let mut args = vec!["exec".to_string()];
    for v in cmd {
        if let Some(s) = v.as_str() {
            args.push(s.to_string());
        }
    }
    run_airis_subprocess_dyn(args)
}

fn handle_workspace_test(arguments: &Value) -> Result<Value> {
    let mut args = vec!["test".to_string()];
    if let Some(extra) = arguments["extra_args"].as_array() {
        for v in extra {
            if let Some(s) = v.as_str() {
                args.push(s.to_string());
            }
        }
    }
    run_airis_subprocess_dyn(args)
}

fn handle_workspace_lint() -> Result<Value> {
    run_airis_subprocess(&["lint"])
}

fn handle_workspace_typecheck() -> Result<Value> {
    run_airis_subprocess(&["typecheck"])
}

fn handle_workspace_clean(arguments: &Value) -> Result<Value> {
    let force = arguments["force"].as_bool().unwrap_or(false);
    let purge = arguments["purge"].as_bool().unwrap_or(false);
    let mut args = vec!["clean".to_string()];
    if force {
        args.push("--force".to_string());
    }
    if purge {
        args.push("--purge".to_string());
    }
    run_airis_subprocess_dyn(args)
}

fn handle_guards_install(arguments: &Value) -> Result<Value> {
    let global = arguments["global"].as_bool().unwrap_or(false);
    if global {
        run_airis_subprocess(&["guards", "install", "--global"])
    } else {
        run_airis_subprocess(&["guards", "install"])
    }
}

fn handle_guards_status(arguments: &Value) -> Result<Value> {
    let global = arguments["global"].as_bool().unwrap_or(false);
    if global {
        run_airis_subprocess(&["guards", "status", "--global"])
    } else {
        run_airis_subprocess(&["guards", "status"])
    }
}

fn handle_guards_uninstall(arguments: &Value) -> Result<Value> {
    let global = arguments["global"].as_bool().unwrap_or(false);
    if global {
        run_airis_subprocess(&["guards", "uninstall", "--global"])
    } else {
        run_airis_subprocess(&["guards", "uninstall"])
    }
}

fn handle_workspace_validate_all() -> Result<Value> {
    run_airis_subprocess(&["validate", "all"])
}

fn handle_workspace_doctor() -> Result<Value> {
    run_airis_subprocess(&["doctor"])
}

fn handle_workspace_verify() -> Result<Value> {
    run_airis_subprocess(&["verify"])
}

fn handle_workspace_status() -> Result<Value> {
    let output = Command::new("docker")
        .args(["compose", "ps"])
        .output()
        .context("Failed to run 'docker compose ps'")?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let success = output.status.success();

    let text = match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => format!("docker compose ps exited with status {}", output.status),
        (false, true) => stdout,
        (true, false) => stderr,
        (false, false) => format!("{stdout}\n--- stderr ---\n{stderr}"),
    };

    Ok(json!({
        "content": [
            { "type": "text", "text": text }
        ],
        "isError": !success
    }))
}

/// Workspace resources advertised over MCP. Each entry is a project-relative
/// path; `resources/list` filters out anything that doesn't currently exist so
/// clients only see real files. Order is the suggested reading order: manifest
/// first, then shared docs, then the Claude adapter, then the generated
/// compose, then the Cargo manifest. Update this list when adding new
/// always-relevant workspace files.
const WORKSPACE_RESOURCES: &[(&str, &str, &str)] = &[
    (
        "manifest.toml",
        "Workspace manifest — Docker-first orchestration source of truth",
        "application/toml",
    ),
    (
        "docs/ai/PROJECT_RULES.md",
        "Project rules for AI agents",
        "text/markdown",
    ),
    (
        "docs/ai/WORKFLOW.md",
        "Default contributor workflow and operational notes",
        "text/markdown",
    ),
    ("docs/ai/REVIEW.md", "Review checklist", "text/markdown"),
    (
        "docs/ai/STACK.md",
        "Stack overview and common commands",
        "text/markdown",
    ),
    (
        "CLAUDE.md",
        "Generated Claude adapter (thin pointer to docs/ai/*.md)",
        "text/markdown",
    ),
    (
        "compose.yaml",
        "Generated docker-compose file",
        "application/yaml",
    ),
    ("Cargo.toml", "Rust crate manifest", "application/toml"),
];

fn handle_resources_list() -> Result<Value> {
    let mut resources: Vec<Value> = Vec::new();
    for (rel_path, description, mime_type) in WORKSPACE_RESOURCES {
        if !Path::new(rel_path).exists() {
            continue;
        }
        resources.push(json!({
            "uri": format!("file:///{}", rel_path),
            "name": *rel_path,
            "description": *description,
            "mimeType": *mime_type,
        }));
    }
    Ok(json!({ "resources": resources }))
}

fn handle_resources_read(uri: &str) -> Result<Value> {
    let rel_path = parse_workspace_uri(uri).with_context(|| {
        format!("Invalid resource URI: {uri}. Expected file:///<workspace-relative-path>")
    })?;

    // Reject anything that isn't on the advertised list. Prevents using
    // resources/read as a generic file-exfiltration primitive.
    let entry = WORKSPACE_RESOURCES
        .iter()
        .find(|(path, _, _)| *path == rel_path)
        .with_context(|| format!("Resource not advertised: {rel_path}"))?;

    let text =
        std::fs::read_to_string(rel_path).with_context(|| format!("Failed to read {rel_path}"))?;

    Ok(json!({
        "contents": [
            {
                "uri": uri,
                "mimeType": entry.2,
                "text": text,
            }
        ]
    }))
}

/// Strip the `file:///` prefix and reject paths that try to escape the
/// workspace root via `..` or absolute components.
fn parse_workspace_uri(uri: &str) -> Option<&str> {
    let path = uri.strip_prefix("file:///")?;
    if path.is_empty() || path.starts_with('/') {
        return None;
    }
    if path.split('/').any(|seg| seg == "..") {
        return None;
    }
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_workspace_uri_accepts_relative_paths() {
        assert_eq!(
            parse_workspace_uri("file:///manifest.toml"),
            Some("manifest.toml")
        );
        assert_eq!(
            parse_workspace_uri("file:///docs/ai/STACK.md"),
            Some("docs/ai/STACK.md")
        );
    }

    #[test]
    fn parse_workspace_uri_rejects_traversal_and_absolute() {
        assert_eq!(parse_workspace_uri("file:///"), None);
        assert_eq!(parse_workspace_uri("file:////etc/passwd"), None);
        assert_eq!(parse_workspace_uri("file:///../etc/passwd"), None);
        assert_eq!(parse_workspace_uri("file:///docs/../etc/passwd"), None);
        assert_eq!(parse_workspace_uri("https://example.com/x"), None);
    }

    #[test]
    fn handle_resources_read_rejects_unadvertised_paths() {
        // `Cargo.lock` is a real file in this workspace but not on the
        // advertised list, so reads should be refused.
        let err = handle_resources_read("file:///Cargo.lock").unwrap_err();
        assert!(err.to_string().contains("not advertised"));
    }

    #[test]
    fn handle_manifest_apply_rejects_invalid_toml() {
        // Syntactically broken TOML must be refused with an error response
        // before any disk write. `run_gen` is off so the handler never reaches
        // the write/gen path and never shells out to `airis gen`.
        let args = json!({ "manifest": "this = = not valid toml", "run_gen": false });
        let result = handle_manifest_apply(&args).unwrap();

        assert_eq!(result["isError"], json!(true));
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("not modified")
        );
    }

    #[test]
    fn handle_manifest_apply_rejects_manifest_failing_validation() {
        // An empty document is valid TOML but fails consistency validation
        // (project.id is required). It must still be rejected before writing.
        let args = json!({ "manifest": "", "run_gen": false });
        let result = handle_manifest_apply(&args).unwrap();

        assert_eq!(result["isError"], json!(true));
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("not modified")
        );
    }
}
