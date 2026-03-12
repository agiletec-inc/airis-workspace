import { streamText, tool } from 'ai'
import { getModel } from '@/lib/ai/provider'
import { SYSTEM_PROMPT } from '@/lib/ai/prompts'
import { parseTasksToolSchema } from '@/lib/ai/schemas'
import { createClient } from '@/lib/supabase/server'

export const maxDuration = 60

export async function POST(req: Request) {
  const supabase = await createClient()
  const { data: { user } } = await supabase.auth.getUser()

  if (!user) {
    return new Response('Unauthorized', { status: 401 })
  }

  const { messages } = await req.json()

  const result = streamText({
    model: getModel(),
    system: SYSTEM_PROMPT,
    messages,
    tools: {
      parse_tasks: tool({
        description: 'ユーザーの入力からタスクをパースして構造化する。タスクリスト、TODO、会議メモなどのタスク的な内容を受け取った場合に使用する。',
        inputSchema: parseTasksToolSchema,
      }),
    },
  })

  return result.toUIMessageStreamResponse()
}
