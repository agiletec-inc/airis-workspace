'use client'

import { useState } from 'react'
import { useRouter } from 'next/navigation'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { TaskPreviewCard } from './task-preview-card'
import { Loader2, CheckCircle2 } from 'lucide-react'
import type { ParsedTask } from '@/lib/ai/schemas'

interface ConfirmTasksDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  tasks: ParsedTask[]
  summary: string
}

export function ConfirmTasksDialog({
  open,
  onOpenChange,
  tasks: initialTasks,
  summary,
}: ConfirmTasksDialogProps) {
  const [tasks, setTasks] = useState<ParsedTask[]>(initialTasks)
  const [isSaving, setIsSaving] = useState(false)
  const [saved, setSaved] = useState(false)
  const router = useRouter()

  // Reset state when dialog opens with new tasks
  const handleOpenChange = (open: boolean) => {
    if (open) {
      setTasks(initialTasks)
      setSaved(false)
    }
    onOpenChange(open)
  }

  // Keep tasks in sync when initialTasks changes
  if (open && initialTasks.length > 0 && tasks !== initialTasks && !isSaving && !saved) {
    setTasks(initialTasks)
  }

  const updateTask = (index: number, updates: Partial<ParsedTask>) => {
    setTasks((prev) =>
      prev.map((t, i) => (i === index ? { ...t, ...updates } : t))
    )
  }

  const removeTask = (index: number) => {
    setTasks((prev) => prev.filter((_, i) => i !== index))
  }

  const handleSave = async () => {
    if (tasks.length === 0) return

    setIsSaving(true)
    try {
      const res = await fetch('/api/tasks/bulk', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ tasks }),
      })

      if (res.ok) {
        setSaved(true)
        setTimeout(() => {
          onOpenChange(false)
          router.refresh()
        }, 1000)
      }
    } finally {
      setIsSaving(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>タスクの確認</DialogTitle>
          <DialogDescription>{summary}</DialogDescription>
        </DialogHeader>

        {saved ? (
          <div className="flex flex-col items-center justify-center py-8">
            <CheckCircle2 className="h-12 w-12 text-green-500" />
            <p className="mt-3 text-sm font-medium">
              {tasks.length}件のタスクを登録しました
            </p>
          </div>
        ) : (
          <>
            <ScrollArea className="max-h-[60vh]">
              <div className="space-y-2 pr-4">
                {tasks.map((task, i) => (
                  <TaskPreviewCard
                    key={i}
                    task={task}
                    index={i}
                    onUpdate={updateTask}
                    onRemove={removeTask}
                  />
                ))}
              </div>
            </ScrollArea>

            <DialogFooter>
              <Button variant="outline" onClick={() => onOpenChange(false)}>
                キャンセル
              </Button>
              <Button onClick={handleSave} disabled={isSaving || tasks.length === 0}>
                {isSaving && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
                {tasks.length}件のタスクを登録
              </Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  )
}
