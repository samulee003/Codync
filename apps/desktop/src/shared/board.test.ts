import assert from 'node:assert/strict'
import { test } from 'node:test'
import { awaitsApproval, boardColumns, criteriaWithChecks, hasBoard, newerTask, parseCriteria, taskActions, taskByline, type BoardTask } from './board.ts'

const task = (id: string, status: BoardTask['status'], updatedAt: number, extra: Partial<BoardTask> = {}): BoardTask => ({
  id, title: id, detail: '', project: null, status, createdBy: null, claimedBy: null, branch: null, worktree: null,
  notes: [], summary: null, createdAt: 0, updatedAt, rev: updatedAt, ...extra,
})

test('columns follow the status order, newest first, without deleted tasks', () => {
  const columns = boardColumns([
    task('a', 'open', 1), task('b', 'open', 3), task('c', 'review', 2), task('d', 'open', 4, { deleted: true }),
  ])
  assert.deepEqual(columns.map((c) => c.status), ['plan', 'open', 'claimed', 'checking', 'review', 'done', 'cancelled'])
  assert.deepEqual(columns[1]!.tasks.map((t) => t.id), ['b', 'a'])
  assert.deepEqual(columns[4]!.tasks.map((t) => t.id), ['c'])
})

test('a task waiting for review merges only with a branch', () => {
  assert.deepEqual(taskActions({ status: 'review', branch: 'codync/task-1' }), ['merge', 'reopen', 'cancel'])
  assert.deepEqual(taskActions({ status: 'review', branch: null }), ['done', 'reopen', 'cancel'])
  assert.ok(!taskActions({ status: 'claimed', branch: null }).includes('delete'), 'a claimed task is released first')
  assert.ok(taskActions({ status: 'claimed', branch: null }).includes('assign'), 'the user can hand a planned task on')
  assert.deepEqual(taskActions({ status: 'checking', branch: null }), ['done', 'reopen', 'cancel'])
})

test('a byline names the claimer, the checker and the task it is part of', () => {
  const look = { name: (b: string) => b.toUpperCase(), parentTitle: (id: string) => (id === 'p' ? 'Workshop' : undefined), tr: (k: string) => k }
  const checking = task('a', 'checking', 1, { claimedBy: 'ada', reviewer: 'bo', parent: 'p', notes: [{ by: null, text: 'x', at: 1 }] })
  assert.equal(taskByline(checking, look), 'ADA · Checked by BO · ↳ Workshop · 1 notes')
  assert.equal(taskByline(task('b', 'done', 1, { claimedBy: 'ada', reviewer: 'bo' }), look), 'ADA', 'checked: the checker no longer shows')
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

test('a plan waiting for the user shows first, and criteria pair with their latest check', () => {
  const base = { id: 'a', title: 'Workshop', detail: '', project: null, createdBy: null, claimedBy: 'bot', branch: null, worktree: null, notes: [], summary: null, createdAt: 1, updatedAt: 1, rev: 1 }
  const waiting: BoardTask = { ...base, status: 'claimed', approval: 'proposed', plan: 'Slides, guide' }
  const working: BoardTask = { ...base, id: 'b', status: 'claimed', approval: 'approved' }
  const columns = boardColumns([waiting, working])
  assert.equal(columns[0].status, 'plan')
  assert.deepEqual(columns[0].tasks.map((t) => t.id), ['a'])
  assert.deepEqual(columns.find((c) => c.status === 'claimed')?.tasks.map((t) => t.id), ['b'])
  assert.ok(awaitsApproval(waiting) && !awaitsApproval(working))
  assert.match(taskByline(waiting, { name: (b) => b, parentTitle: () => undefined, tr: (k) => k }), /Plan waits for you/)

  assert.deepEqual(parseCriteria('- 180 minutes\n\n  * Cantonese \n• Offline'), ['180 minutes', 'Cantonese', 'Offline'])
  const checked = criteriaWithChecks({ acceptance: ['180 minutes', 'Cantonese'], checks: [{ criterion: 'Cantonese', pass: false, note: 'Part 3' }] })
  assert.deepEqual(checked, [{ criterion: '180 minutes', check: undefined }, { criterion: 'Cantonese', check: { criterion: 'Cantonese', pass: false, note: 'Part 3' } }])
})
