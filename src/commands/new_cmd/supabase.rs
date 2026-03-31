//! Supabase trigger and realtime scaffolding

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Generate a Supabase database trigger migration
pub fn generate_supabase_trigger(project_dir: &Path, name: &str) -> Result<()> {
    // For triggers, we create a migration file instead of a function directory
    let migrations_dir = Path::new("supabase/migrations");
    if !migrations_dir.exists() {
        fs::create_dir_all(migrations_dir).context("Failed to create migrations directory")?;
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
    fs::create_dir_all(project_dir).context("Failed to create function directory")?;

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
pub fn generate_supabase_realtime(project_dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(project_dir).context("Failed to create realtime function directory")?;

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
