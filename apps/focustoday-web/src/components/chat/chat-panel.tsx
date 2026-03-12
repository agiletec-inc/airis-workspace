'use client'

import { useChat } from '@ai-sdk/react'
import { useRef, useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Send, Loader2 } from 'lucide-react'
import { MessageBubble } from './message-bubble'
import { ConfirmTasksDialog } from './confirm-tasks-dialog'
import type { ParsedTask } from '@/lib/ai/schemas'

export function ChatPanel() {
  const [pendingTasks, setPendingTasks] = useState<ParsedTask[] | null>(null)
  const [pendingSummary, setPendingSummary] = useState('')
  const [input, setInput] = useState('')
  const scrollRef = useRef<HTMLDivElement>(null)

  const { messages, sendMessage, status } = useChat({
    id: 'focus-today-chat',
    onToolCall({ toolCall }) {
      if (toolCall.toolName === 'parse_tasks') {
        const result = toolCall.input as { tasks: ParsedTask[]; summary: string }
        setPendingTasks(result.tasks)
        setPendingSummary(result.summary)
      }
    },
  })

  const isLoading = status === 'submitted' || status === 'streaming'

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollIntoView({ behavior: 'smooth' })
    }
  }, [messages])

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!input.trim() || isLoading) return
    const text = input
    setInput('')
    await sendMessage({ text })
  }

  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      if (input.trim() && !isLoading) {
        handleSubmit(e)
      }
    }
  }

  return (
    <div className="flex h-full flex-col">
      <ScrollArea className="flex-1 px-4">
        <div className="mx-auto max-w-2xl space-y-4 py-4">
          {messages.length === 0 && (
            <div className="flex flex-col items-center justify-center pt-20 text-center">
              <h2 className="text-lg font-semibold">Focus Today AI Chat</h2>
              <p className="mt-2 max-w-md text-sm text-muted-foreground">
                テキストを貼り付けるとタスクに変換します。
                会議メモ、チャットログ、TODOリストなど何でもOK。
              </p>
              <div className="mt-6 space-y-2 text-left text-sm text-muted-foreground">
                <p>例:</p>
                <pre className="rounded-lg bg-muted p-3 text-xs">{`- LP制作のデザインレビュー（火曜まで）
- APIのエラーハンドリング修正
- 週次定例の議事録まとめ
- ユーザーインタビューのアポ取り`}</pre>
              </div>
            </div>
          )}

          {messages.map((message) => (
            <MessageBubble key={message.id} message={message} />
          ))}

          {isLoading && (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Loader2 className="h-4 w-4 animate-spin" />
              考え中...
            </div>
          )}

          <div ref={scrollRef} />
        </div>
      </ScrollArea>

      <div className="border-t bg-background p-4">
        <form
          onSubmit={handleSubmit}
          className="mx-auto flex max-w-2xl items-end gap-2"
        >
          <Textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder="タスクを入力、またはテキストを貼り付け..."
            className="min-h-[44px] max-h-[200px] resize-none"
            rows={1}
          />
          <Button
            type="submit"
            size="icon"
            disabled={!input.trim() || isLoading}
            className="shrink-0"
          >
            <Send className="h-4 w-4" />
          </Button>
        </form>
      </div>

      <ConfirmTasksDialog
        open={pendingTasks !== null}
        onOpenChange={(open) => {
          if (!open) setPendingTasks(null)
        }}
        tasks={pendingTasks ?? []}
        summary={pendingSummary}
      />
    </div>
  )
}
