import { createClient } from '@/lib/supabase/server'
import { TaskList } from '@/components/tasks/task-list'

export default async function TasksPage() {
  const supabase = await createClient()

  const { data: tasks } = await supabase
    .from('tasks')
    .select('*, category:categories(*)')
    .eq('is_backlog', false)
    .neq('status', 'done')
    .order('sort_order')

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center justify-between border-b px-6 py-4">
        <h1 className="text-xl font-semibold">Tasks</h1>
      </header>
      <div className="flex-1 overflow-auto p-6">
        <TaskList tasks={tasks ?? []} />
      </div>
    </div>
  )
}
