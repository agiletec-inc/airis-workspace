import type { Tables as SupabaseTables, TablesInsert as SupabaseTablesInsert, TablesUpdate as SupabaseTablesUpdate } from './supabase'
export type { Database } from './supabase'

export type Tables<T extends keyof {
  profiles: unknown
  categories: unknown
  tasks: unknown
  chat_sessions: unknown
  chat_messages: unknown
}> = SupabaseTables<T>

export type TablesInsert<T extends keyof {
  profiles: unknown
  categories: unknown
  tasks: unknown
  chat_sessions: unknown
  chat_messages: unknown
}> = SupabaseTablesInsert<T>

export type TablesUpdate<T extends keyof {
  profiles: unknown
  categories: unknown
  tasks: unknown
  chat_sessions: unknown
  chat_messages: unknown
}> = SupabaseTablesUpdate<T>

// Convenience type aliases
export type Profile = SupabaseTables<'profiles'>
export type Category = SupabaseTables<'categories'>
export type Task = SupabaseTables<'tasks'>
export type ChatSession = SupabaseTables<'chat_sessions'>
export type ChatMessage = SupabaseTables<'chat_messages'>

export type TaskWithCategory = Task & {
  category: Category | null
}

export type TaskWithSubtasks = TaskWithCategory & {
  subtasks: Task[]
}
