import { createClient } from '@/lib/supabase/server'
import { format } from 'date-fns'
import { ja } from 'date-fns/locale'
import { TaskList } from '@/components/tasks/task-list'

export default async function TodayPage() {
  const supabase = await createClient()
  const today = format(new Date(), 'yyyy-MM-dd')

  const { data: tasks } = await supabase
    .from('tasks')
    .select('*, category:categories(*)')
    .eq('due_date', today)
    .eq('is_backlog', false)
    .order('sort_order')

  const dateLabel = format(new Date(), 'M月d日(E)', { locale: ja })

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center justify-between border-b px-6 py-4">
        <div>
          <h1 className="text-xl font-semibold">Today</h1>
          <p className="text-sm text-muted-foreground">{dateLabel}</p>
        </div>
      </header>
      <div className="flex-1 overflow-auto p-6">
        <TaskList tasks={tasks ?? []} />
      </div>
    </div>
  )
}
