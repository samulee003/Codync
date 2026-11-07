import assert from 'node:assert/strict'
import { test } from 'node:test'
import { boardColumns, hasBoard, newerTask, taskActions, type BoardTask } from './board.ts'

const task = (id: string, status: BoardTask['status'], updatedAt: number, extra: Partial<BoardTask> = {}): BoardTask => ({
  id, title: id, detail: '', project: null, status, createdBy: null, claimedBy: null, branch: null, worktree: null,
  notes: [], summary: null, createdAt: 0, updatedAt, rev: updatedAt, ...extra,
})

test('columns follow the status order, newest first, without deleted tasks', () => {
  const columns = boardColumns([
    task('a', 'open', 1), task('b', 'open', 3), task('c', 'review', 2), task('d', 'open', 4, { deleted: true }),
  ])
  assert.deepEqual(columns.map((c) => c.status), ['open', 'claimed', 'review', 'done', 'cancelled'])
  assert.deepEqual(columns[0]!.tasks.map((t) => t.id), ['b', 'a'])
  assert.deepEqual(columns[2]!.tasks.map((t) => t.id), ['c'])
})

test('a task waiting for review merges only with a branch', () => {
  assert.deepEqual(taskActions({ status: 'review', branch: 'codync/task-1' }), ['merge', 'reopen', 'cancel'])
  assert.deepEqual(taskActions({ status: 'review', branch: null }), ['done', 'reopen', 'cancel'])
  assert.ok(!taskActions({ status: 'claimed', branch: null }).includes('delete'), 'a claimed task is released first')
})

test('an older copy never replaces a newer one', () => {
  assert.ok(newerTask(undefined, task('a', 'open', 1)))
  assert.ok(!newerTask(task('a', 'open', 5), task('a', 'claimed', 4)))
  assert.ok(newerTask(task('a', 'open', 5), task('a', 'claimed', 6)))
})

test('the board needs a host that has it', () => {
  assert.ok(!hasBoard(undefined))
  assert.ok(!hasBoard('2.40.3'))
  assert.ok(hasBoard('2.41.0'))
})
