// The team board on the wire (`host/src/board/`): tasks the user and the bots share.
import { isBelow } from './compat.ts'

export type TaskStatus = 'open' | 'claimed' | 'checking' | 'review' | 'done' | 'cancelled'

export interface TaskNote {
  /** The bot that wrote it; null: the user. */
  by: string | null
  text: string
  at: number
}

export interface BoardTask {
  id: string
  title: string
  detail: string
  /** The folder the work is in; a git repository gives each claim its own worktree. */
  project: string | null
  status: TaskStatus
  /** The bot that added it; null: the user. */
  createdBy: string | null
  claimedBy: string | null
  branch: string | null
  worktree: string | null
  notes: TaskNote[]
  summary: string | null
  /** The task this one is a part of (a subtask). */
  parent?: string | null
  /** The bot that checks (or checked) the finished work. */
  reviewer?: string | null
  createdAt: number
  updatedAt: number
  rev: number
  deleted?: boolean
}

/** The board's columns, in the order they show. */
export const TASK_COLUMNS: { status: TaskStatus; title: string }[] = [
  { status: 'open', title: 'Open' },
  { status: 'claimed', title: 'In progress' },
  { status: 'checking', title: 'Being checked' },
  { status: 'review', title: 'Ready to merge' },
  { status: 'done', title: 'Done' },
  { status: 'cancelled', title: 'Cancelled' },
]

/** Tasks by status, each column newest first. */
export function boardColumns(tasks: Iterable<BoardTask>): { status: TaskStatus; title: string; tasks: BoardTask[] }[] {
  const live = [...tasks].filter((t) => !t.deleted).sort((a, b) => b.updatedAt - a.updatedAt)
  return TASK_COLUMNS.map((column) => ({ ...column, tasks: live.filter((t) => t.status === column.status) }))
}

/** What the user can do with a task in its status. */
export type TaskAction = 'assign' | 'merge' | 'reopen' | 'done' | 'cancel' | 'delete'

export function taskActions(task: Pick<BoardTask, 'status' | 'branch'>): TaskAction[] {
  switch (task.status) {
    case 'open': return ['assign', 'done', 'cancel', 'delete']
    case 'claimed': return ['assign', 'reopen', 'cancel']
    case 'checking': return ['done', 'reopen', 'cancel']
    case 'review': return task.branch ? ['merge', 'reopen', 'cancel'] : ['done', 'reopen', 'cancel']
    case 'done':
    case 'cancelled': return ['reopen', 'delete']
  }
}

/** A task's byline: who holds it, who checks it, the task it is part of, its branch and how many notes it has. */
export function taskByline(
  task: BoardTask,
  look: { name: (bot: string) => string; parentTitle: (id: string) => string | undefined; tr: (key: string) => string },
): string {
  const { name, parentTitle, tr } = look
  const parent = task.parent ? parentTitle(task.parent) : undefined
  return [
    task.claimedBy ? name(task.claimedBy) : null,
    task.status === 'checking' && task.reviewer ? tr('Checked by {name}').replace('{name}', name(task.reviewer)) : null,
    parent ? `↳ ${parent}` : null,
    task.branch,
    task.notes.length ? tr('{count} notes').replace('{count}', String(task.notes.length)) : null,
  ].filter(Boolean).join(' · ')
}

/** Keeps the newer of two copies of a task (by rev). */
export function newerTask(known: BoardTask | undefined, incoming: BoardTask): boolean {
  return !known || incoming.rev >= known.rev
}

/** The first host with the team board. */
export const BOARD_HOST = '2.41.0'

export function hasBoard(hostVersion: string | undefined): boolean {
  return !!hostVersion && !isBelow(hostVersion, BOARD_HOST)
}
