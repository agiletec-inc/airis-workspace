import { z } from 'zod'

export const parsedTaskSchema = z.object({
  title: z.string().describe('タスクのタイトル'),
  description: z.string().optional().describe('タスクの詳細説明'),
  category: z.string().optional().describe('カテゴリ名（例: 開発, デザイン, ミーティング）'),
  priority: z.number().min(0).max(3).default(0).describe('優先度 0=なし, 1=Low, 2=Medium, 3=High'),
  due_date: z.string().optional().describe('期限日 (YYYY-MM-DD形式)'),
  due_time: z.string().optional().describe('期限時刻 (HH:MM形式)'),
  estimated_minutes: z.number().optional().describe('見積もり時間（分）'),
  subtasks: z.array(z.object({
    title: z.string(),
    estimated_minutes: z.number().optional(),
  })).optional().describe('サブタスクの配列'),
})

export const parseTasksToolSchema = z.object({
  tasks: z.array(parsedTaskSchema).describe('パースされたタスクの配列'),
  summary: z.string().describe('パース結果の要約メッセージ'),
})

export type ParsedTask = z.infer<typeof parsedTaskSchema>
export type ParseTasksResult = z.infer<typeof parseTasksToolSchema>
