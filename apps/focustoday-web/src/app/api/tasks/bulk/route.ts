import { NextResponse } from 'next/server'
import { createClient } from '@/lib/supabase/server'
import type { ParsedTask } from '@/lib/ai/schemas'

const CATEGORY_COLORS = [
  '#6366f1', '#8b5cf6', '#ec4899', '#f43f5e',
  '#f97316', '#eab308', '#22c55e', '#14b8a6',
  '#06b6d4', '#3b82f6',
]

export async function POST(req: Request) {
  const supabase = await createClient()
  const { data: { user } } = await supabase.auth.getUser()

  if (!user) {
    return NextResponse.json({ error: 'Unauthorized' }, { status: 401 })
  }

  const { tasks }: { tasks: ParsedTask[] } = await req.json()

  // Get or create categories
  const { data: existingCategories } = await supabase
    .from('categories')
    .select('*')
    .eq('user_id', user.id)

  const categoryMap = new Map(
    (existingCategories ?? []).map((c) => [c.name, c.id])
  )

  const uniqueNewCategories = [
    ...new Set(
      tasks
        .map((t) => t.category)
        .filter((c): c is string => !!c && !categoryMap.has(c))
    ),
  ]

  if (uniqueNewCategories.length > 0) {
    const { data: newCategories } = await supabase
      .from('categories')
      .insert(
        uniqueNewCategories.map((name, i) => ({
          user_id: user.id,
          name,
          color: CATEGORY_COLORS[(existingCategories?.length ?? 0 + i) % CATEGORY_COLORS.length],
          sort_order: (existingCategories?.length ?? 0) + i,
        }))
      )
      .select()

    newCategories?.forEach((c) => categoryMap.set(c.name, c.id))
  }

  // Get max sort_order
  const { data: maxOrderResult } = await supabase
    .from('tasks')
    .select('sort_order')
    .eq('user_id', user.id)
    .order('sort_order', { ascending: false })
    .limit(1)

  let sortOrder = (maxOrderResult?.[0]?.sort_order ?? -1) + 1

  // Insert tasks
  const insertedTasks = []

  for (const task of tasks) {
    const categoryId = task.category ? categoryMap.get(task.category) ?? null : null

    const { data: parentTask } = await supabase
      .from('tasks')
      .insert({
        user_id: user.id,
        title: task.title,
        description: task.description ?? null,
        category_id: categoryId,
        priority: task.priority ?? 0,
        due_date: task.due_date ?? null,
        due_time: task.due_time ?? null,
        estimated_minutes: task.estimated_minutes ?? null,
        sort_order: sortOrder++,
      })
      .select('*, category:categories(*)')
      .single()

    if (parentTask && task.subtasks && task.subtasks.length > 0) {
      const { data: subtasks } = await supabase
        .from('tasks')
        .insert(
          task.subtasks.map((st, i) => ({
            user_id: user.id,
            title: st.title,
            parent_id: parentTask.id,
            category_id: categoryId,
            estimated_minutes: st.estimated_minutes ?? null,
            sort_order: i,
          }))
        )
        .select()

      insertedTasks.push({ ...parentTask, subtasks: subtasks ?? [] })
    } else if (parentTask) {
      insertedTasks.push({ ...parentTask, subtasks: [] })
    }
  }

  return NextResponse.json({ tasks: insertedTasks })
}
