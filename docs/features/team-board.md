# Team board

Each computer has one board of tasks that the user and its bots share, modeled on the claim board of
[Agensh](https://github.com/microsoft/Agensh): bots split a job into tasks, each claims one, notes what it finds for the
others, and finishes it. A task in a git project gets its own worktree and branch when claimed, so bots working in one
repository side by side never edit the same checkout. Everything lives in the host (`host/src/board/`); the apps show the
board and send intents.

## Tasks

A task has a title, a detail, an optional project folder (absolute, an existing directory), a status, who added it (a
bot or the user), who claimed it, its branch and worktree, notes (at most 50, oldest dropped first) and the claimer's
summary.

| Status | Means | Next |
|---|---|---|
| `open` | Waiting for a claimer | claimed by a bot, or assigned to one by the user |
| `claimed` | A bot holds it (at most 3 per bot) | `finish_task`, `release_task`, or the user reopens/cancels it |
| `review` | Finished on its branch, waiting for the user's merge | `mergeTask`, or reopen/cancel |
| `done` | Finished (merged, or no branch) | reopen, delete |
| `cancelled` | Dropped | reopen, delete |

Claims run under the board's lock (`board::Board`), so the first claimer wins and a second gets "X already claimed this
task". A computer keeps at most 500 live tasks.

## Git worktrees

When a task's project is the top of a git repository with a commit, `claim` runs
`git worktree add <data dir>/worktrees/<task id>` on branch `codync/task-<first 8 of the id>` (new from `HEAD`, or the
branch an earlier claim left). The claimer is told to work and commit only there. `finish_task` refuses while the
worktree has uncommitted changes, so the branch is the whole result; the task then waits in `review`.

`mergeTask` merges the branch into the project's checked-out branch with `--no-ff` and a "Merge task: <title>" commit. It
refuses while the project has uncommitted changes to tracked files, and aborts a conflicting merge, leaving the project
as it was. After a merge the worktree is removed and the branch deleted (`git branch -d`). Cancelling or deleting a
task removes its worktree and keeps its branch, so no commit is lost. `GIT_*` variables are left out of every git call.

## Bots' tools

On every bot's `team` MCP server, next to `list_bots` and `message_bot`: `list_tasks` (`status`, `mine`), `read_task`,
`create_task` (project defaults to the bot's folder), `claim_task` (answers `workIn` with the worktree), `note_task`,
`finish_task` (`summary` required) and `release_task` (`reason` becomes a note).

## App methods and sync

`tasks`, `createTask {title, detail?, project?}`, `editTask {taskId, title?, detail?}`, `noteTask {taskId, text}`,
`assignTask {taskId, botId}` (claims the task for the bot and starts a turn in its own chat), `setTaskStatus {taskId,
status: open|done|cancelled}`, `mergeTask {taskId}`, `deleteTask {taskId}`. Tasks are kept in kv `board.task.<id>`,
each change stamped with a rev and sent as `{"type": "task", "task": …}`; the catch-up replays changes after `since`,
and a deleted task stays as a tombstone (`deleted: true`) so clients drop it. Older apps ignore the event.

## Clients

| Client | Where | Actions |
|---|---|---|
| Desktop | `checklist` button in the sidebar header (and rail) → sheet (`views/board/`) | `+` new task, unfold a task for detail and notes, add a note, menu: assign to a bot, merge, reopen, mark done, cancel, delete |
| iPhone | `checklist` toolbar button on the roster → sheet (`CodyncUI/Board/BoardView.swift`) | same |
| Android | `checklist` button in the roster header → sheet (`BoardView.kt`) | same |
| Terminal UI | `B` or the palette's *Team board* (`tui/manage/board.rs`) | `n` new, `↵` unfold, `c` note, `a` assign, `m` merge, `o` reopen, `d` done, `x` cancel, `X` delete |

The board button shows only for a computer whose host is 2.41.0 or newer. Analytics: `task_changed` (`action`, `by`)
and the `board` screen ([analytics events](../reference/analytics-events.md)).

## Remote computer use

The board pairs with the `agent-desktop` driver ([computer use](computer-use.md)), which lets several bots use the
computer at once: split GUI work into tasks so each bot claims a different app or window instead of two driving the
same one.
