'use client'

import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Button } from '@/components/ui/button'
import { Clock, Trash2, ChevronDown, ChevronRight } from 'lucide-react'
import { useState } from 'react'
import type { ParsedTask } from '@/lib/ai/schemas'

const priorityLabels = ['なし', 'Low', 'Medium', 'High']
const priorityColors = ['', 'text-blue-500', 'text-amber-500', 'text-red-500']

interface TaskPreviewCardProps {
  task: ParsedTask
  index: number
  onUpdate: (index: number, updates: Partial<ParsedTask>) => void
  onRemove: (index: number) => void
}

export function TaskPreviewCard({ task, index, onUpdate, onRemove }: TaskPreviewCardProps) {
  const [showSubtasks, setShowSubtasks] = useState(false)

  return (
    <div className="rounded-lg border bg-card p-3">
      <div className="flex items-start gap-2">
        <div className="flex-1 space-y-2">
          <Input
            value={task.title}
            onChange={(e) => onUpdate(index, { title: e.target.value })}
            className="h-8 text-sm font-medium"
          />

          <div className="flex flex-wrap items-center gap-2">
            <Input
              value={task.category ?? ''}
              onChange={(e) => onUpdate(index, { category: e.target.value || undefined })}
              placeholder="カテゴリ"
              className="h-7 w-28 text-xs"
            />

            <Select
              value={String(task.priority ?? 0)}
              onValueChange={(v) => onUpdate(index, { priority: Number(v) })}
            >
              <SelectTrigger className="h-7 w-24 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {priorityLabels.map((label, i) => (
                  <SelectItem key={i} value={String(i)}>
                    <span className={priorityColors[i]}>{label}</span>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>

            <div className="flex items-center gap-1">
              <Clock className="h-3 w-3 text-muted-foreground" />
              <Input
                type="number"
                value={task.estimated_minutes ?? ''}
                onChange={(e) =>
                  onUpdate(index, {
                    estimated_minutes: e.target.value ? Number(e.target.value) : undefined,
                  })
                }
                placeholder="分"
                className="h-7 w-16 text-xs"
              />
            </div>

            <Input
              type="date"
              value={task.due_date ?? ''}
              onChange={(e) => onUpdate(index, { due_date: e.target.value || undefined })}
              className="h-7 w-36 text-xs"
            />
          </div>

          {task.subtasks && task.subtasks.length > 0 && (
            <div>
              <button
                onClick={() => setShowSubtasks(!showSubtasks)}
                className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
              >
                {showSubtasks ? (
                  <ChevronDown className="h-3 w-3" />
                ) : (
                  <ChevronRight className="h-3 w-3" />
                )}
                サブタスク ({task.subtasks.length})
              </button>
              {showSubtasks && (
                <div className="mt-1 space-y-1 pl-4">
                  {task.subtasks.map((st, i) => (
                    <div key={i} className="flex items-center gap-2 text-xs">
                      <span className="text-muted-foreground">-</span>
                      <span>{st.title}</span>
                      {st.estimated_minutes && (
                        <Badge variant="secondary" className="text-[10px]">
                          {st.estimated_minutes}min
                        </Badge>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>

        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 shrink-0"
          onClick={() => onRemove(index)}
        >
          <Trash2 className="h-3.5 w-3.5 text-muted-foreground" />
        </Button>
      </div>
    </div>
  )
}
