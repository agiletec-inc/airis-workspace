'use client'

import { useState } from 'react'
import { createClient } from '@/lib/supabase/client'
import { cn } from '@/lib/utils'
import { Badge } from '@/components/ui/badge'
import { CheckCircle2, Circle, Clock, AlertTriangle } from 'lucide-react'
import type { Task, Category } from '@/types/database'
import { useRouter } from 'next/navigation'

type TaskWithCategory = Task & { category: Category | null }

const priorityConfig = [
  { label: '', icon: null, className: '' },
  { label: 'Low', icon: null, className: 'text-blue-500' },
  { label: 'Medium', icon: AlertTriangle, className: 'text-amber-500' },
  { label: 'High', icon: AlertTriangle, className: 'text-red-500' },
]

export function TaskCard({ task }: { task: TaskWithCategory }) {
  const [isUpdating, setIsUpdating] = useState(false)
  const supabase = createClient()
  const router = useRouter()

  const toggleStatus = async (e: React.MouseEvent) => {
    e.stopPropagation()
    if (isUpdating) return
    setIsUpdating(true)

    const newStatus = task.status === 'done' ? 'todo' : 'done'
    await supabase
      .from('tasks')
      .update({ status: newStatus })
      .eq('id', task.id)

    setIsUpdating(false)
    router.refresh()
  }

  const priority = priorityConfig[task.priority]
  const isDone = task.status === 'done'

  return (
    <div
      className={cn(
        'group flex items-start gap-3 rounded-lg border px-3 py-2.5 transition-colors hover:bg-muted/50',
        isDone && 'opacity-60'
      )}
    >
      <button
        onClick={toggleStatus}
        className="mt-0.5 shrink-0"
        disabled={isUpdating}
      >
        {isDone ? (
          <CheckCircle2 className="h-5 w-5 text-green-500" />
        ) : (
          <Circle className="h-5 w-5 text-muted-foreground hover:text-foreground" />
        )}
      </button>

      <div className="min-w-0 flex-1">
        <p className={cn('text-sm', isDone && 'line-through')}>
          {task.title}
        </p>
        {task.description && (
          <p className="mt-0.5 truncate text-xs text-muted-foreground">
            {task.description}
          </p>
        )}
        <div className="mt-1 flex items-center gap-2">
          {task.category && (
            <Badge
              variant="secondary"
              className="text-xs"
              style={{
                backgroundColor: `${task.category.color}20`,
                color: task.category.color,
              }}
            >
              {task.category.name}
            </Badge>
          )}
          {task.estimated_minutes && (
            <span className="flex items-center gap-1 text-xs text-muted-foreground">
              <Clock className="h-3 w-3" />
              {task.estimated_minutes}min
            </span>
          )}
          {task.priority > 0 && (
            <span className={cn('flex items-center gap-1 text-xs', priority.className)}>
              {priority.icon && <priority.icon className="h-3 w-3" />}
              {priority.label}
            </span>
          )}
        </div>
      </div>
    </div>
  )
}
