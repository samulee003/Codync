# Team board

Each computer has one board of tasks that the user and its bots share, modeled on the claim board of
[Agensh](https://github.com/microsoft/Agensh): bots split a job into tasks, each claims one, notes what it finds for the
others, and finishes it. A task in a git project gets its own worktree and branch when claimed, so bots working in one
repository side by side never edit the same checkout. Everything lives in the host (`host/src/board/`); the apps show the
board and send intents.

## Tasks

A task has a title, a detail, an optional project folder (absolute, an existing directory), a status, who added it (a
bot or the user), who claimed it, its branch and worktree, notes (at most 50, oldest dropped first), the claimer's
summary, the task it is a part of (`parent`, a subtask), who checks it (`reviewer`) and how often it was sent back
(`sendBacks`).

| Status | Means | Next |
|---|---|---|
| `open` | Waiting for a claimer | claimed by a bot, or assigned to one by the user |
| `claimed` | A bot holds it (at most 3 per bot) | `finish_task`, `release_task`, or the user reopens/cancels it |
| `checking` | Finished; another bot checks the result | `review_task` accepts it (`review`/`done`) or sends it back (`claimed`) |
| `review` | Finished on its branch, waiting for the user's merge | `mergeTask`, or reopen/cancel |
| `done` | Finished (merged, or no branch) | reopen, delete |
| `cancelled` | Dropped | reopen, delete |

Claims run under the board's lock (`board::Board`), so the first claimer wins and a second gets "X already claimed this
task". A computer keeps at most 500 live tasks.

## Bots come for tasks on their own

Like Agensh's workers, bots claim from the board without being assigned (`board/dispatch.rs`). Every 5 seconds, while
the board has open tasks, each bot that takes part and is idle (not working, waiting on the user or failing) and holds
fewer than 3 tasks gets a host-started turn in its own chat listing the open tasks (at most 10): claim one that fits with
`claim_task`, do it and `finish_task`, or reply in one line that none fits. A bot hears again only when the set of open
tasks changes, or after 30 minutes. Every bot takes part unless the user turns it off on the board
(`setBoardClaims {botId, on}`, saved in kv `board.optedOut`; `tasks` answers `optedOut`).

Each nudge also lists the bot's teammates (name, agent, model, description) and asks it to claim only work that fits it
better than them, and to split a task that turns out to hold parallel parts into subtasks instead of doing it alone.

## The lead plans, bots check each other (`board/flow.rs`)

A user's work never sits for the fastest bot to grab whole:

- **Planning.** A task the user adds (`createTask`) is claimed for the lead bot (else the first bot), without a worktree,
  in a host-started turn: split it into subtasks if it has parts others could do side by side, else do it if it fits, or
  `release_task` it (naming who fits) for the board. **Split a goal** (`splitGoal {goal}`) makes the goal such a task
  (titled by its first line) and asks the lead to split it. Both answer the planner (`{botId, name}`); the user can
  still hand the task to another bot (`assignTask` takes it from the lead).
- **Subtasks.** `create_task` with `parentId` adds a part. The parent's claimer coordinates: it can't `finish_task`
  while a subtask is `open`, `claimed` or `checking`, and when the last one is finished (or the user closes it) it gets
  a turn listing the parts and their summaries, to check and assemble them and finish the parent.
- **Checking.** `finish_task` puts the task in `checking` with a checker: another bot that takes part, idle ones first,
  then the one holding fewest tasks. The checker gets a turn to look at the result itself and answer `review_task
  {taskId, approve, feedback}`: accepted, the task goes to `review` (a branch to merge) or `done`; sent back, the
  feedback becomes a note ("Sent back: …") and the claimer gets the task again with it. After two send-backs, or with
  no other bot, a finish settles without a check.

## Character sheets (`board/sheet.rs`)

Each bot has a sheet, shown like a unit's status screen in a tactics game: the **class** the user gives it, a **level**,
and five **abilities** (0–100). Kept in kv `board.sheet.<botId>` as the class and the bot's deeds, counted as they happen
so deleting a task takes nothing back.

| Class (wire) | For | Abilities start at (might, intellect, command, accuracy, insight) |
|---|---|---|
| `strategist` | Plans and splits work | 30, 75, 70, 50, 55 |
| `warrior` | Builds code and systems | 75, 40, 35, 55, 35 |
| `scholar` | Writes documents and material | 45, 65, 40, 65, 45 |
| `scout` | Researches | 50, 55, 35, 50, 60 |
| `inspector` | Checks finished work | 35, 55, 40, 70, 80 |

No class starts every ability at 40. Deeds: `finished` (accepted work), `firstTry` (accepted without a send-back),
`sentBack`, `checked` (tasks it reviewed), `caught` (reviews that sent work back), `planned` (subtasks it added) and
`notes`. An ability grows by `min(25, 5·√count)`: might with `finished`, intellect with `planned + notes/2`, command
with `planned`, insight with `checked + caught`; accuracy moves from the class's start to the bot's own first-try rate
over its first 10 finished tasks. Experience is `10·finished + 5·firstTry + 4·checked + 3·caught + 6·planned + notes`,
and level `n` starts at `20·(n−1)²`.

The class also steers the board: nudges and the planner's prompt list each teammate's class, and an inspector is
picked first to check finished work. Methods: `botSheets` (every bot's sheet: `class`, `level`, `xp`, `levelXp`,
`nextLevelXp`, `abilities`, `deeds`) and `setBotClass {botId, class|null}`. Stamina is the usage reading
([usage limits](usage-limits.md)), not part of the sheet.

Clients: the desktop app only for now, by the maintainer's choice to try it there first: the *Character* card at the
top of a bot's details (class picker, level and experience, stamina, ability chart, deeds), a class picker in *New
bot*, and the level beside the name in the roster from level 2. iPhone, Android and the terminal UI follow once the
design has settled.

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
`create_task` (project defaults to the bot's folder; `parentId` for a subtask), `claim_task` (answers `workIn` with the
worktree), `note_task`, `finish_task` (`summary` required), `release_task` (`reason` becomes a note) and `review_task`
(`approve`, `feedback`). `read_task` includes `checkedBy`, `parentId` and the `subtasks` with their summaries.

## App methods and sync

`tasks`, `createTask {title, detail?, project?}` (the lead plans it), `editTask {taskId, title?, detail?}`, `noteTask {taskId, text}`,
`assignTask {taskId, botId}` (claims the task for the bot and starts a turn in its own chat), `setTaskStatus {taskId,
status: open|done|cancelled}`, `mergeTask {taskId}`, `deleteTask {taskId}`. Tasks are kept in kv `board.task.<id>`,
each change stamped with a rev and sent as `{"type": "task", "task": …}`; the catch-up replays changes after `since`,
and a deleted task stays as a tombstone (`deleted: true`) so clients drop it. Older apps ignore the event.

## Clients

Every client shows the `checking` column (*Being checked*), who checks a task (*Checked by {name}*) and the task a
subtask is part of (*↳ title*).

| Client | Where | Actions |
|---|---|---|
| Desktop | `checklist` button in the sidebar header (and rail) → sheet (`views/board/`) | `+` new task (folder from the workspace picker), wand: split a goal, unfold a task for detail and notes, add a note, menu: assign to a bot, merge, reopen, mark done, cancel, delete; a switch per bot for claiming on its own |
| iPhone | `checklist` toolbar button on the roster → sheet (`CodyncUI/Board/`) | same |
| Android | `checklist` button in the roster header → sheet (`BoardView.kt`, `BoardSheets.kt`) | same |
| Terminal UI | `B` or the palette's *Team board* (`tui/manage/board.rs`) | `n` new (folder typed as a path), `s` split a goal, `p` bots that claim, `↵` unfold, `c` note, `a` assign, `m` merge, `o` reopen, `d` done, `x` cancel, `X` delete |

The board button shows only for a computer whose host is 2.41.0 or newer. Analytics: `task_changed` (`action`, `by`)
and the `board` screen ([analytics events](../reference/analytics-events.md)).

## Remote computer use

The board pairs with the `agent-desktop` driver ([computer use](computer-use.md)), which lets several bots use the
computer at once: split GUI work into tasks so each bot claims a different app or window instead of two driving the
same one.
