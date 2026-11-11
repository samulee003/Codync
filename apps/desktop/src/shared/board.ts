// The team board on the wire (`host/src/board/`): tasks the user and the bots share.
import { isBelow } from './compat.ts'

export type TaskStatus = 'open' | 'claimed' | 'checking' | 'review' | 'done' | 'cancelled'

export interface TaskNote {
  /** The bot that wrote it; null: the user. */
  by: string | null
  text: string
  at: number
}

/** Whether the user approves the claimer's plan before work starts. */
export type PlanApproval = 'notNeeded' | 'waiting' | 'proposed' | 'approved'

/** The checker's verdict on one acceptance criterion. */
export interface CriterionCheck {
  criterion: string
  pass: boolean
  note: string
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
  /** What must hold for the result to count as done; the checker marks each pass or fail. */
  acceptance?: string[]
  /** The plan its claimer proposed for the user's approval. */
  plan?: string | null
  approval?: PlanApproval
  /** The latest check, criterion by criterion. */
  checks?: CriterionCheck[]
  createdAt: number
  updatedAt: number
  rev: number
  deleted?: boolean
}

/** A board column: a status, or the plans waiting for the user's approval (claimed tasks, shown first). */
export type BoardColumn = TaskStatus | 'plan'

/** The board's columns, in the order they show. */
export const TASK_COLUMNS: { status: BoardColumn; title: string }[] = [
  { status: 'plan', title: 'Plans to approve' },
  { status: 'open', title: 'Open' },
  { status: 'claimed', title: 'In progress' },
  { status: 'checking', title: 'Being checked' },
  { status: 'review', title: 'Ready to merge' },
  { status: 'done', title: 'Done' },
  { status: 'cancelled', title: 'Cancelled' },
]

/** Whether a task's plan waits for the user's approval. */
export function awaitsApproval(task: Pick<BoardTask, 'status' | 'approval'>): boolean {
  return task.status === 'claimed' && task.approval === 'proposed'
}

/** The column a task shows in. */
export function taskColumn(task: Pick<BoardTask, 'status' | 'approval'>): BoardColumn {
  return awaitsApproval(task) ? 'plan' : task.status
}

/** Tasks by column, each newest first. */
export function boardColumns(tasks: Iterable<BoardTask>): { status: BoardColumn; title: string; tasks: BoardTask[] }[] {
  const live = [...tasks].filter((t) => !t.deleted).sort((a, b) => b.updatedAt - a.updatedAt)
  return TASK_COLUMNS.map((column) => ({ ...column, tasks: live.filter((t) => taskColumn(t) === column.status) }))
}

/** Acceptance criteria typed one per line: trimmed, list marks dropped, blank lines left out. */
export function parseCriteria(text: string): string[] {
  return text.split('\n').map((line) => line.replace(/^[\s\-*•]+/, '').trim()).filter(Boolean)
}

/** Each criterion with its verdict from the latest check, if it was checked. */
export function criteriaWithChecks(task: Pick<BoardTask, 'acceptance' | 'checks'>): { criterion: string; check?: CriterionCheck }[] {
  const checks = task.checks ?? []
  return (task.acceptance ?? []).map((criterion) => ({ criterion, check: checks.find((c) => c.criterion === criterion) }))
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
    awaitsApproval(task) ? tr('Plan waits for you') : null,
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
