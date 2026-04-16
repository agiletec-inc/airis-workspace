use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::manifest::Manifest;
use crate::commands::discover::discover_from_workspaces;
use crate::commands::migrate::{MigrationPlan, MigrationTask};

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
                    "name": "workspace_analyze",
                    "description": "Analyze the current physical workspace and return detected frameworks and project structures.",
                    "input_schema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "manifest_propose",
                    "description": "Propose a new manifest.toml structure. Validates and writes to disk if valid.",
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
                    "name": "migration_plan_execute",
                    "description": "Execute a list of migration tasks (file moves, directory creation).",
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
                "workspace_analyze" => handle_workspace_analyze()?,
                "manifest_propose" => handle_manifest_propose(arguments)?,
                "migration_plan_execute" => handle_migration_plan_execute(arguments)?,
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

fn handle_workspace_analyze() -> Result<Value> {
    let workspace_root = std::env::current_dir()?;
    let manifest_path = Path::new("manifest.toml");
    
    let mut analysis = json!({
        "root": workspace_root.display().to_string(),
        "has_manifest": manifest_path.exists(),
    });

    // Basic discovery logic
    let patterns = vec!["apps/*".to_string(), "libs/*".to_string(), "packages/*".to_string()];
    let discovered = discover_from_workspaces(&patterns, &workspace_root)?;
    
    analysis["projects"] = serde_json::to_value(discovered)?;

    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": serde_json::to_string_pretty(&analysis)?
            }
        ]
    }))
}

fn handle_manifest_propose(arguments: &Value) -> Result<Value> {
    let content = arguments["manifest"].as_str().context("Missing manifest content")?;
    
    // Attempt to parse and validate
    match Manifest::parse(content) {
        Ok(manifest) => {
            match manifest.validate() {
                Ok(_) => {
                    // Valid manifest, write it to disk
                    std::fs::write("manifest.toml", content)?;
                    Ok(json!({
                        "content": [
                            {
                                "type": "text",
                                "text": "Manifest is valid and has been written to manifest.toml. Run `airis gen` to apply changes to the environment."
                            }
                        ]
                    }))
                }
                Err(e) => {
                    Ok(json!({
                        "content": [
                            {
                                "type": "text",
                                "text": format!("Validation failed:\n{}", e)
                            }
                        ],
                        "isError": true
                    }))
                }
            }
        }
        Err(e) => {
            Ok(json!({
                "content": [
                    {
                        "type": "text",
                        "text": format!("Parsing failed:\n{}", e)
                    }
                ],
                "isError": true
            }))
        }
    }
}

fn handle_migration_plan_execute(arguments: &Value) -> Result<Value> {
    let tasks: Vec<MigrationTask> = serde_json::from_value(arguments["tasks"].clone())?;
    
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
