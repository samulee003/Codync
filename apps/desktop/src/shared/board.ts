// The team board on the wire (`host/src/board/`): tasks the user and the bots share.
import { isBelow } from './compat.ts'

export type TaskStatus = 'open' | 'claimed' | 'checking' | 'review' | 'done' | 'cancelled'

export interface TaskNote {
  /** The bot that wrote it; null: the user. */
  by: string | null
  text: string
  at: number
}

/** Whether the user approves the claimer's plan before work starts; `asking`: the bot has questions before it plans. */
export type PlanApproval = 'notNeeded' | 'waiting' | 'proposed' | 'approved' | 'asking'

/** A question the claimer asks the user before planning, with the user's answer once given. */
export interface TaskQuestion {
  text: string
  answer?: string | null
}

/** A saved task: its fields copied into the new-task form. */
export interface TaskTemplate {
  id: string
  name: string
  title: string
  detail: string
  acceptance: string[]
  approvePlan: boolean
  project: string | null
}

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
  /** The questions the claimer asked before planning. */
  questions?: TaskQuestion[]
  /** When work started and finished (ms), and how many turns the bots spent on it. */
  startedAt?: number | null
  finishedAt?: number | null
  turns?: number
  /** The latest check, criterion by criterion. */
  checks?: CriterionCheck[]
  /** The tasks it waits for: it starts once they are finished. */
  after?: string[]
  /** The bot it goes to once nothing it waits for is left. */
  assignee?: string | null
  createdAt: number
  updatedAt: number
  rev: number
  deleted?: boolean
}

/** A board column: a status, or the tasks waiting for the user (claimed tasks, shown first). */
export type BoardColumn = TaskStatus | 'plan'

/** The board's columns, in the order they show. */
export const TASK_COLUMNS: { status: BoardColumn; title: string }[] = [
  { status: 'plan', title: 'Waiting for you' },
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

/** Whether a claimed task waits for the user's answers to the questions it asked before planning. */
export function awaitsAnswers(task: Pick<BoardTask, 'status' | 'approval'>): boolean {
  return task.status === 'claimed' && task.approval === 'asking'
}

/** Whether a task waits for the user: a plan to approve, or questions to answer. */
export function needsUser(task: Pick<BoardTask, 'status' | 'approval'>): boolean {
  return awaitsApproval(task) || awaitsAnswers(task)
}

/** The column a task shows in. */
export function taskColumn(task: Pick<BoardTask, 'status' | 'approval'>): BoardColumn {
  return needsUser(task) ? 'plan' : task.status
}

/** How much work a task took, once a bot spent a turn on it: the turns and the minutes from start to finish (0 until it finishes). */
export function taskEffort(task: Pick<BoardTask, 'turns' | 'startedAt' | 'finishedAt'>): { turns: number; minutes: number } | null {
  const { turns = 0, startedAt, finishedAt } = task
  if (!turns) return null
  const timed = startedAt != null && finishedAt != null
  const minutes = timed ? Math.max(0, Math.round((finishedAt - startedAt) / 60_000)) : 0
  return { turns, minutes }
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

/** Whether a task another waits for is out of the way: done, cancelled, or finished with no branch left to merge. */
export function finishedForOthers(task: Pick<BoardTask, 'status' | 'branch'>): boolean {
  return task.status === 'done' || task.status === 'cancelled' || (task.status === 'review' && !task.branch)
}

/** The titles of the tasks an open task still waits for. */
export function waitingFor(task: Pick<BoardTask, 'status' | 'after'>, find: (id: string) => BoardTask | undefined): string[] {
  if (task.status !== 'open') return []
  return (task.after ?? []).map(find).filter((t): t is BoardTask => !!t && !t.deleted && !finishedForOthers(t)).map((t) => t.title)
}

/** A task's byline: who holds it, who checks it, the task it is part of, its branch and how many notes it has. */
export function taskByline(
  task: BoardTask,
  look: { name: (bot: string) => string; find: (id: string) => BoardTask | undefined; tr: (key: string) => string },
): string {
  const { name, find, tr } = look
  const parent = task.parent ? find(task.parent)?.title : undefined
  const waiting = waitingFor(task, find)
  return [
    waiting.length ? tr('Waits for {titles}').replace('{titles}', waiting.join(', ')) : null,
    task.status === 'open' && task.assignee ? tr('For {name}').replace('{name}', name(task.assignee)) : null,
    task.claimedBy ? name(task.claimedBy) : null,
    awaitsApproval(task) ? tr('Plan waits for you') : null,
    awaitsAnswers(task) ? tr('Has questions for you') : null,
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
