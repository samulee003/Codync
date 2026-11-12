import assert from 'node:assert/strict'
import { test } from 'node:test'
import { awaitsAnswers, awaitsApproval, boardColumns, criteriaWithChecks, hasBoard, needsUser, newerTask, parseCriteria, taskActions, taskByline, taskColumn, taskEffort, waitingFor, type BoardTask } from './board.ts'

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
  const look = { name: (b: string) => b.toUpperCase(), find: (id: string) => (id === 'p' ? task('p', 'open', 1, { title: 'Workshop' }) : undefined), tr: (k: string) => k }
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
  assert.match(taskByline(waiting, { name: (b) => b, find: () => undefined, tr: (k) => k }), /Plan waits for you/)

  assert.deepEqual(parseCriteria('- 180 minutes\n\n  * Cantonese \n• Offline'), ['180 minutes', 'Cantonese', 'Offline'])
  const checked = criteriaWithChecks({ acceptance: ['180 minutes', 'Cantonese'], checks: [{ criterion: 'Cantonese', pass: false, note: 'Part 3' }] })
  assert.deepEqual(checked, [{ criterion: '180 minutes', check: undefined }, { criterion: 'Cantonese', check: { criterion: 'Cantonese', pass: false, note: 'Part 3' } }])
})

test('an open task shows what it still waits for, and who it goes to', () => {
  const base = { detail: '', project: null, createdBy: null, claimedBy: null, branch: null, worktree: null, notes: [], summary: null, createdAt: 1, updatedAt: 1, rev: 1 }
  const helpers: BoardTask = { ...base, id: 'h', title: 'Text helpers', status: 'claimed' }
  const merged: BoardTask = { ...base, id: 'm', title: 'Models', status: 'review', branch: 'codync/task-m' }
  const api: BoardTask = { ...base, id: 'a', title: 'Public API', status: 'open', after: ['h', 'm', 'gone'], assignee: 'bo' }
  const find = (id: string) => [helpers, merged, api].find((t) => t.id === id)
  assert.deepEqual(waitingFor(api, find), ['Text helpers', 'Models'], 'a branch waiting to merge still holds it')
  assert.equal(taskByline(api, { name: (b) => b.toUpperCase(), find, tr: (k) => k }), 'Waits for Text helpers, Models · For BO')
  assert.deepEqual(waitingFor({ ...api, status: 'claimed' }, find), [], 'started: nothing to wait for')
  assert.deepEqual(waitingFor(api, (id) => (id === 'h' ? { ...helpers, status: 'done' } : id === 'm' ? { ...merged, branch: null } : undefined)), [])
})

test('a task that asks questions first waits for the user, like a plan to approve', () => {
  const asking = task('q', 'claimed', 1, { approval: 'asking', questions: [{ text: 'Which folder?' }] })
  const proposed = task('p', 'claimed', 1, { approval: 'proposed' })
  const working = task('w', 'claimed', 1, { approval: 'approved' })
  assert.ok(awaitsAnswers(asking) && !awaitsAnswers(proposed) && !awaitsAnswers(working))
  assert.ok(needsUser(asking) && needsUser(proposed) && !needsUser(working))
  assert.equal(taskColumn(asking), 'plan')
  assert.equal(taskColumn(working), 'claimed')
  assert.equal(taskColumn({ status: 'open', approval: 'asking' }), 'open', 'only a claimed task is asking')
  assert.equal(boardColumns([asking])[0]!.title, 'Waiting for you')
  assert.match(taskByline(asking, { name: (b) => b, find: () => undefined, tr: (k) => k }), /Has questions for you/)
})

test('a task shows its effort once a bot spent a turn on it', () => {
  assert.equal(taskEffort({}), null)
  assert.equal(taskEffort({ turns: 0, startedAt: null, finishedAt: null }), null)
  assert.deepEqual(taskEffort({ turns: 3, startedAt: null, finishedAt: null }), { turns: 3, minutes: 0 })
  assert.deepEqual(taskEffort({ turns: 2, startedAt: 1_000, finishedAt: 1_000 + 9 * 60_000 + 20_000 }), { turns: 2, minutes: 9 })
  assert.equal(taskEffort({ startedAt: 0, finishedAt: 90_000 }), null, 'no turn yet')
  assert.deepEqual(taskEffort({ turns: 4, startedAt: 5 * 60_000, finishedAt: null }), { turns: 4, minutes: 0 }, 'not finished: no minutes yet')
})
