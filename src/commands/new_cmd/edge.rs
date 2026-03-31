//! Supabase Edge Function scaffolding

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Generate a Supabase Edge Function
pub fn generate_edge_function(project_dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(project_dir).context("Failed to create edge function directory")?;

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
