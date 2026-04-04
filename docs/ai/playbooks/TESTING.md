# Testing Playbook

This playbook defines the testing strategy enforced by `manifest.toml [testing]`.

## Test Levels

| Level | Purpose | Runs against | When |
|-------|---------|-------------|------|
| **unit** | Pure logic, no I/O | In-memory | Every commit |
| **integration** | Real DB, real schema | Local Supabase / test project | Pre-push, CI |
| **e2e** | Full user flow | Staging environment | Post-deploy |
| **smoke** | Critical path alive? | Production / staging | Post-deploy, monitoring |

## Mock Policy

The `[testing].mock_policy` field controls what is allowed:

- **`forbidden`** (default) — Never mock external services. Use real instances or local emulators.
- **`unit-only`** — Mocks allowed in `*.test.*` files only. Forbidden in `*.integration.*`, `*.e2e.*`, `*.spec.*`.
- **`allowed`** — No restrictions (opt-in escape hatch).

### Why mocks are dangerous

Mocks simulate what you **think** the service does, not what it **actually** does. When the real service changes (schema migration, API version bump, RLS policy update), mocked tests keep passing while production breaks.

## Type Enforcement

When `[testing.type_enforcement]` is configured:

1. All DB-touching test files must import from the generated types path.
2. `airis policy check` scans for `required_imports` patterns and fails if missing.

### Correct pattern

```typescript
// Import generated types — always up to date with real schema
import type { Database } from '@workspace/database';

const supabase = createClient<Database>(url, key);
const { data } = await supabase.from('users').select('*');
// TypeScript catches schema mismatches at compile time
```

### Wrong pattern

```typescript
// Hand-typed interface — will drift from real schema
interface User {
  id: string;
  name: string;
}

// No type safety, no schema validation
const { data } = await supabase.from('users').select('*');
const user = data as User; // unsafe cast
```

## Writing Integration Tests

1. **Start with the schema.** Run `supabase gen types` to get the latest types.
2. **Use a test database.** Either `supabase start` (local) or a dedicated test project.
3. **Clean up after yourself.** Each test should create its own data and delete it, or use transactions that roll back.
4. **Test RLS policies.** Create clients with different roles and verify access control.
5. **Test triggers and functions.** If the DB has server-side logic, test it through the client.

### Example

```typescript
import { createClient } from '@supabase/supabase-js';
import type { Database } from '@workspace/database';
import { describe, it, expect, beforeAll, afterAll } from 'vitest';

const supabase = createClient<Database>(
  process.env.SUPABASE_URL!,
  process.env.SUPABASE_SERVICE_KEY!,
);

describe('users table', () => {
  const testUserId = crypto.randomUUID();

  afterAll(async () => {
    await supabase.from('users').delete().eq('id', testUserId);
  });

  it('inserts and reads back', async () => {
    const { error: insertError } = await supabase
      .from('users')
      .insert({ id: testUserId, name: 'test-user' });
    expect(insertError).toBeNull();

    const { data, error } = await supabase
      .from('users')
      .select('*')
      .eq('id', testUserId)
      .single();
    expect(error).toBeNull();
    expect(data?.name).toBe('test-user');
  });
});
```

## Forbidden Patterns

`[testing].forbidden_patterns` defines regex patterns that `airis policy check` scans for in test files. Common patterns to forbid:

```toml
forbidden_patterns = [
    "vi\\.mock.*supabase",
    "vi\\.mock.*database",
    "jest\\.mock.*supabase",
    "jest\\.mock.*database",
    "createClient.*mock",
]
```

## Smoke Tests

`[[testing.smoke]]` entries define critical-path checks that run post-deploy:

```toml
[[testing.smoke]]
name = "api-health"
command = "curl -sf $API_URL/health"
timeout = 10

[[testing.smoke]]
name = "auth-flow"
command = "vitest run --filter smoke-auth"
timeout = 60
```

Keep smoke tests fast (under 60s) and focused on "is the service alive and can users do the most important thing?"

## Workflow

```
Code change
  → unit tests (local, fast, mocks OK for pure logic)
  → integration tests (local Supabase, real schema, pre-push)
  → CI runs airis policy check (forbidden patterns + type enforcement)
  → deploy to staging
  → e2e tests (staging environment, real services)
  → smoke tests (post-deploy health check)
```
