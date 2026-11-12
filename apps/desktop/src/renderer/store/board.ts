import { newerTask, type BoardTask, type TaskStatus, type TaskTemplate } from '@shared/board'
import type { HostClient } from '../client/host-client'
import { trNow } from '../lib/l10n.ts'

/**
 * One computer's team board (`host/src/board/`): its tasks by id, from `tasks` and the `task` events. Every change goes
 * through the host, which answers the task as saved; the event that follows carries the same copy.
 */
export class Board {
  readonly byId = new Map<string, BoardTask>()
  /** Bots the user turned off from claiming open tasks on their own; the rest take part. */
  optedOut = new Set<string>()
  /** The saved task templates, for filling the new-task form. */
  templates: TaskTemplate[] = []
  private changed: () => void
  constructor(changed: () => void) { this.changed = changed }

  upsert(task: BoardTask) {
    if (!newerTask(this.byId.get(task.id), task)) return
    if (task.deleted) this.byId.delete(task.id)
    else this.byId.set(task.id, task)
    this.changed()
  }

  async load(client: HostClient | null) {
    const { tasks, optedOut } = await connected(client).tasks()
    for (const task of tasks) this.upsert(task)
    this.optedOut = new Set(optedOut ?? [])
    this.templates = await loadTemplates(client)
    this.changed()
  }

  async setClaims(client: HostClient | null, botId: string, on: boolean) {
    this.optedOut = new Set((await connected(client).setBoardClaims(botId, on)).optedOut)
    this.changed()
  }

  /** Answers the name of the bot splitting it. */
  async split(client: HostClient | null, goal: string): Promise<string> {
    return (await connected(client).splitGoal(goal)).name
  }

  /** A new task with its acceptance criteria; `approvePlan`: its claimer's plan waits for the user first. */
  async create(client: HostClient | null, title: string, detail: string, project: string | null, acceptance: string[], approvePlan: boolean) {
    this.upsert((await connected(client).createTask(title, detail, project, acceptance, approvePlan)).task)
  }

  async approvePlan(client: HostClient | null, id: string) {
    this.upsert((await connected(client).approvePlan(id)).task)
  }

  /** Sends a proposed plan back with what to change. */
  async revisePlan(client: HostClient | null, id: string, note: string) {
    this.upsert((await connected(client).revisePlan(id, note)).task)
  }

  /** The user's answers to the questions a claimed task asked before planning, one per question, in order. */
  async answerQuestions(client: HostClient | null, id: string, answers: string[]) {
    this.upsert((await connected(client).answerQuestions(id, answers)).task)
  }

  async saveTemplate(client: HostClient | null, taskId: string, name?: string) {
    const { template } = await connected(client).saveTaskTemplate(taskId, name)
    this.templates = [...this.templates.filter((t) => t.id !== template.id), template]
    this.changed()
  }

  async deleteTemplate(client: HostClient | null, templateId: string) {
    await connected(client).deleteTaskTemplate(templateId)
    this.templates = this.templates.filter((t) => t.id !== templateId)
    this.changed()
  }

  async note(client: HostClient | null, id: string, text: string) {
    this.upsert((await connected(client).noteTask(id, text)).task)
  }

  async assign(client: HostClient | null, id: string, botId: string) {
    this.upsert((await connected(client).assignTask(id, botId)).task)
  }

  async setStatus(client: HostClient | null, id: string, status: TaskStatus) {
    this.upsert((await connected(client).setTaskStatus(id, status)).task)
  }

  async merge(client: HostClient | null, id: string) {
    this.upsert((await connected(client).mergeTask(id)).task)
  }

  async delete(client: HostClient | null, id: string) {
    await connected(client).deleteTask(id)
    this.byId.delete(id)
    this.changed()
  }
}

/** Hosts before task templates have no such method: the board still loads, without templates. */
async function loadTemplates(client: HostClient | null): Promise<TaskTemplate[]> {
  try {
    return (await connected(client).taskTemplates()).templates
  } catch {
    return []
  }
}

function connected(client: HostClient | null): HostClient {
  if (!client) throw new Error(trNow('Connect to this computer to use its board'))
  return client
}
