# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Environment

This project uses **Docker-first development** via airis-monorepo. Do not install dependencies on the host.

### Commands (all run inside Docker)

```bash
# Start workspace container
docker compose up -d

# Install dependencies
docker compose exec workspace pnpm install

# Development server (http://localhost:3000)
docker compose exec workspace pnpm dev

# Type checking
docker compose exec workspace pnpm typecheck

# Linting
docker compose exec workspace pnpm lint

# Enter container shell
docker compose exec workspace bash

# Stop workspace
docker compose down
```

Or use `airis` CLI shortcuts: `airis up`, `airis install`, `airis dev`, `airis shell`

### Environment Variables

Copy `.env.local.example` to `.env.local` and set:
- `NEXT_PUBLIC_SUPABASE_URL` / `NEXT_PUBLIC_SUPABASE_ANON_KEY` - Supabase project
- `ANTHROPIC_API_KEY` - For AI chat (or `OPENAI_API_KEY` with `AI_PROVIDER=openai`)

## Architecture

### Tech Stack
- **Next.js 16** (App Router, Turbopack)
- **Supabase** (PostgreSQL + Google OAuth)
- **Vercel AI SDK v5** (streaming chat with tool calling)
- **shadcn/ui** + Tailwind CSS v4

### Project Structure

```
src/
├── app/
│   ├── (auth)/           # Login, OAuth callback
│   ├── (dashboard)/      # Protected routes (chat, today, tasks, calendar)
│   └── api/
│       ├── chat/         # LLM streaming endpoint
│       ├── tasks/        # CRUD + bulk creation
│       └── categories/   # CRUD
├── components/
│   ├── chat/             # ChatPanel, ConfirmTasksDialog
│   ├── tasks/            # TaskList, TaskCard
│   └── layout/           # Sidebar
├── lib/
│   ├── ai/               # LLM provider, prompts, schemas
│   └── supabase/         # Client/server helpers, middleware
└── types/
    └── database.ts       # Supabase types (generated via `supabase gen types`)
```

### Core Data Flow: AI Chat → Task Creation

1. User sends text in `ChatPanel` (`@ai-sdk/react` `useChat`)
2. `POST /api/chat` streams response using Vercel AI SDK's `streamText`
3. LLM calls `parse_tasks` tool (client-side tool, no execute function)
4. `onToolCall` callback in `ChatPanel` extracts parsed tasks
5. `ConfirmTasksDialog` lets user edit/confirm tasks
6. `POST /api/tasks/bulk` creates tasks + auto-creates categories

### Database Tables (Supabase)

- **profiles** - User info, auto-created on signup via trigger
- **categories** - Task categories with color/icon
- **tasks** - Core entity (title, status, priority, due_date, category_id, parent_id for subtasks)
- **chat_sessions** / **chat_messages** - AI conversation history

All tables have RLS enabled (user_id based).

### Key Patterns

**Supabase Types**: Generated from remote schema. Regenerate with:
```bash
npx supabase gen types typescript --project-id wjqzivpclivjjbzacezo > src/types/supabase.ts
```

**AI Provider Switching**: Set `AI_PROVIDER=openai` in `.env.local` to use OpenAI instead of Anthropic.

**Auth Flow**: Middleware at `src/middleware.ts` redirects unauthenticated users to `/login`. Dashboard routes are protected via server-side check in `(dashboard)/layout.tsx`.
