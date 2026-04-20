use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::path::Path;

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

fn handle_request(request: McpRequest) -> Result<McpResponse> {
    let result = match request.method.as_str() {
        "initialize" => Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {
                    "listChanged": false
                }
            },
            "serverInfo": {
                "name": "airis-workspace-mcp",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        "notifications/initialized" => None,
        "tools/list" => Some(json!({
            "tools": [
                {
                    "name": "workspace_init",
                    "description": "Initialize or sync manifest.toml with the current repository state. Detects existing apps, libs, and legacy docker-compose files (v1), proposing a normalized manifest.toml that follows the latest airis best practices and standardizes on compose.yaml (v2). After applying the proposed manifest with 'manifest_apply', it is highly recommended to run 'airis clean --purge --force' via shell to remove the legacy configuration files and complete the consolidation.",
                    "input_schema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "workspace_discover",
                    "description": "Scan the workspace to detect current structural facts. Useful for gathering context before proposing manual manifest changes.",
                    "input_schema": {
                        "type": "object",
                        "properties": {}
                    }
                },

                {
                    "name": "manifest_validate",
                    "description": "Validate a proposed manifest.toml content without writing it to disk.",
                    "input_schema": {
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
                    "input_schema": {
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
                    "input_schema": {
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
                }
            ]
        })),
        "tools/call" => {
            let params = request.params.as_ref().context("Missing params")?;
            let name = params["name"].as_str().context("Missing tool name")?;
            let arguments = &params["arguments"];

            let tool_result = match name {
                "workspace_init" => handle_workspace_init()?,
                "workspace_discover" => handle_workspace_discover()?,
                "manifest_validate" => handle_manifest_validate(arguments)?,
                "manifest_apply" => handle_manifest_apply(arguments)?,
                "migration_execute" => handle_migration_execute(arguments)?,
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

    // 1. Write to disk
    std::fs::write("manifest.toml", content)?;

    let mut response_text = "Manifest written to manifest.toml.".to_string();

    // 2. Optionally run gen
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
