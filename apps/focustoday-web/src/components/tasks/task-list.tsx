'use client'

import { TaskCard } from './task-card'
import type { Task, Category } from '@/types/database'

type TaskWithCategory = Task & { category: Category | null }

export function TaskList({ tasks }: { tasks: TaskWithCategory[] }) {
  if (tasks.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-center">
        <p className="text-muted-foreground">タスクがありません</p>
        <p className="mt-1 text-sm text-muted-foreground">
          AI Chatでタスクを追加してみましょう
        </p>
      </div>
    )
  }

  // Group by category
  const grouped = tasks.reduce<Record<string, { category: Category | null; tasks: TaskWithCategory[] }>>(
    (acc, task) => {
      const key = task.category_id ?? 'uncategorized'
      if (!acc[key]) {
        acc[key] = { category: task.category, tasks: [] }
      }
      acc[key].tasks.push(task)
      return acc
    },
    {}
  )

  return (
    <div className="space-y-6">
      {Object.entries(grouped).map(([key, group]) => (
        <div key={key}>
          <div className="mb-2 flex items-center gap-2">
            {group.category && (
              <div
                className="h-3 w-3 rounded-full"
                style={{ backgroundColor: group.category.color }}
              />
            )}
            <h3 className="text-sm font-medium text-muted-foreground">
              {group.category?.name ?? '未分類'}
            </h3>
            <span className="text-xs text-muted-foreground">
              ({group.tasks.length})
            </span>
          </div>
          <div className="space-y-1">
            {group.tasks.map((task) => (
              <TaskCard key={task.id} task={task} />
            ))}
          </div>
        </div>
      ))}
    </div>
  )
}
