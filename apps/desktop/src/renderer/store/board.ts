import { newerTask, type BoardTask, type TaskStatus } from '@shared/board'
import type { HostClient } from '../client/host-client'
import { trNow } from '../lib/l10n.ts'

/**
 * One computer's team board (`host/src/board/`): its tasks by id, from `tasks` and the `task` events. Every change goes
 * through the host, which answers the task as saved; the event that follows carries the same copy.
 */
export class Board {
  readonly byId = new Map<string, BoardTask>()
  private changed: () => void
  constructor(changed: () => void) { this.changed = changed }

  upsert(task: BoardTask) {
    if (!newerTask(this.byId.get(task.id), task)) return
    if (task.deleted) this.byId.delete(task.id)
    else this.byId.set(task.id, task)
    this.changed()
  }

  async load(client: HostClient | null) {
    for (const task of (await connected(client).tasks()).tasks) this.upsert(task)
  }

  async create(client: HostClient | null, title: string, detail: string, project: string | null) {
    this.upsert((await connected(client).createTask(title, detail, project)).task)
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

function connected(client: HostClient | null): HostClient {
  if (!client) throw new Error(trNow('Connect to this computer to use its board'))
  return client
}
