-- ============================================
-- Focus Today - Initial Schema
-- ============================================

-- ============================================
-- Profiles
-- ============================================
create table public.profiles (
  id uuid references auth.users on delete cascade primary key,
  email text,
  full_name text,
  avatar_url text,
  google_access_token text,
  google_refresh_token text,
  created_at timestamptz default now() not null,
  updated_at timestamptz default now() not null
);

alter table public.profiles enable row level security;

create policy "Users can view own profile"
  on public.profiles for select
  using (auth.uid() = id);

create policy "Users can update own profile"
  on public.profiles for update
  using (auth.uid() = id);

create policy "Users can insert own profile"
  on public.profiles for insert
  with check (auth.uid() = id);

-- Auto-create profile on signup
create or replace function public.handle_new_user()
returns trigger as $$
begin
  insert into public.profiles (id, email, full_name, avatar_url)
  values (
    new.id,
    new.email,
    coalesce(new.raw_user_meta_data->>'full_name', new.raw_user_meta_data->>'name'),
    new.raw_user_meta_data->>'avatar_url'
  );
  return new;
end;
$$ language plpgsql security definer;

create trigger on_auth_user_created
  after insert on auth.users
  for each row execute procedure public.handle_new_user();

-- ============================================
-- Categories
-- ============================================
create table public.categories (
  id uuid default gen_random_uuid() primary key,
  user_id uuid references auth.users on delete cascade not null,
  name text not null,
  color text default '#6366f1' not null,
  icon text,
  sort_order integer default 0 not null,
  created_at timestamptz default now() not null,
  updated_at timestamptz default now() not null
);

alter table public.categories enable row level security;

create policy "Users can view own categories"
  on public.categories for select
  using (auth.uid() = user_id);

create policy "Users can insert own categories"
  on public.categories for insert
  with check (auth.uid() = user_id);

create policy "Users can update own categories"
  on public.categories for update
  using (auth.uid() = user_id);

create policy "Users can delete own categories"
  on public.categories for delete
  using (auth.uid() = user_id);

create index idx_categories_user_id on public.categories(user_id);

-- ============================================
-- Tasks
-- ============================================
create table public.tasks (
  id uuid default gen_random_uuid() primary key,
  user_id uuid references auth.users on delete cascade not null,
  category_id uuid references public.categories on delete set null,
  parent_id uuid references public.tasks on delete cascade,
  title text not null,
  description text,
  status text default 'todo' not null check (status in ('todo', 'in_progress', 'done')),
  priority integer default 0 not null check (priority between 0 and 3),
  due_date date,
  due_time time,
  estimated_minutes integer,
  is_backlog boolean default false not null,
  sort_order integer default 0 not null,
  completed_at timestamptz,
  google_task_id text,
  google_calendar_event_id text,
  created_at timestamptz default now() not null,
  updated_at timestamptz default now() not null
);

alter table public.tasks enable row level security;

create policy "Users can view own tasks"
  on public.tasks for select
  using (auth.uid() = user_id);

create policy "Users can insert own tasks"
  on public.tasks for insert
  with check (auth.uid() = user_id);

create policy "Users can update own tasks"
  on public.tasks for update
  using (auth.uid() = user_id);

create policy "Users can delete own tasks"
  on public.tasks for delete
  using (auth.uid() = user_id);

create index idx_tasks_user_id on public.tasks(user_id);
create index idx_tasks_category_id on public.tasks(category_id);
create index idx_tasks_parent_id on public.tasks(parent_id);
create index idx_tasks_due_date on public.tasks(due_date);
create index idx_tasks_status on public.tasks(status);

-- Auto-set completed_at when status changes to 'done'
create or replace function public.handle_task_completion()
returns trigger as $$
begin
  if new.status = 'done' and (old.status is null or old.status != 'done') then
    new.completed_at = now();
  elsif new.status != 'done' then
    new.completed_at = null;
  end if;
  new.updated_at = now();
  return new;
end;
$$ language plpgsql;

create trigger on_task_update
  before update on public.tasks
  for each row execute procedure public.handle_task_completion();

-- Also handle on insert
create or replace function public.handle_task_insert()
returns trigger as $$
begin
  if new.status = 'done' then
    new.completed_at = now();
  end if;
  return new;
end;
$$ language plpgsql;

create trigger on_task_insert
  before insert on public.tasks
  for each row execute procedure public.handle_task_insert();

-- ============================================
-- Chat Sessions
-- ============================================
create table public.chat_sessions (
  id uuid default gen_random_uuid() primary key,
  user_id uuid references auth.users on delete cascade not null,
  title text,
  created_at timestamptz default now() not null,
  updated_at timestamptz default now() not null
);

alter table public.chat_sessions enable row level security;

create policy "Users can view own chat sessions"
  on public.chat_sessions for select
  using (auth.uid() = user_id);

create policy "Users can insert own chat sessions"
  on public.chat_sessions for insert
  with check (auth.uid() = user_id);

create policy "Users can update own chat sessions"
  on public.chat_sessions for update
  using (auth.uid() = user_id);

create policy "Users can delete own chat sessions"
  on public.chat_sessions for delete
  using (auth.uid() = user_id);

-- ============================================
-- Chat Messages
-- ============================================
create table public.chat_messages (
  id uuid default gen_random_uuid() primary key,
  session_id uuid references public.chat_sessions on delete cascade not null,
  user_id uuid references auth.users on delete cascade not null,
  role text not null check (role in ('user', 'assistant', 'system')),
  content text not null,
  tool_invocations jsonb,
  created_at timestamptz default now() not null
);

alter table public.chat_messages enable row level security;

create policy "Users can view own chat messages"
  on public.chat_messages for select
  using (auth.uid() = user_id);

create policy "Users can insert own chat messages"
  on public.chat_messages for insert
  with check (auth.uid() = user_id);

create index idx_chat_messages_session_id on public.chat_messages(session_id);

-- ============================================
-- Updated at trigger helper
-- ============================================
create or replace function public.update_updated_at()
returns trigger as $$
begin
  new.updated_at = now();
  return new;
end;
$$ language plpgsql;

create trigger update_profiles_updated_at
  before update on public.profiles
  for each row execute procedure public.update_updated_at();

create trigger update_categories_updated_at
  before update on public.categories
  for each row execute procedure public.update_updated_at();

create trigger update_chat_sessions_updated_at
  before update on public.chat_sessions
  for each row execute procedure public.update_updated_at();
