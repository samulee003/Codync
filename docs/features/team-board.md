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
  in a host-started turn that asks it to **size the task first** and note its call ("Size: small, one bot" / "Size:
  large, N parts"). Small (one bot does it well in about 15 minutes, or its parts would take minutes each): no split,
  since a team costs a planning turn plus a turn and a check per part; the lead does it if it fits, or `release_task`s
  it naming who fits. Large (independent parts that each take real work, or need different strengths): split into
  subtasks and coordinate. A same-task trial showed why: six small modules took one solo bot one turn and about a
  minute, the team of four about fifteen turns and two and a half minutes, both scoring full marks. **Split a goal** (`splitGoal {goal}`) makes the goal such a task
  (titled by its first line) and asks the lead to split it if it is big enough. Both answer the planner (`{botId, name}`); the user can
  still hand the task to another bot (`assignTask` takes it from the lead).
- **Subtasks.** `create_task` with `parentId` adds a part; with `assignee` (a teammate's name) the lead hands it straight to that bot, which gets a turn to start on it (a trial showed bots told to claim only what fits them better than their teammates all deferred and nobody started). Nudges now say every task needs doing: claim the one that names you, else one that fits, and take a task that has waited rather than hold out for a better fit. The parent's claimer coordinates: it can't `finish_task`
  while a subtask is `open`, `claimed` or `checking`, and when the last one is finished (or the user closes it) it gets
  a turn listing the parts and their summaries, to check and assemble them and finish the parent.
- **Checking.** `finish_task` puts the task in `checking` with a checker: another bot that takes part, idle ones first,
  then the one holding fewest tasks. The checker gets a turn to look at the result itself and answer `review_task
  {taskId, approve, feedback}`: accepted, the task goes to `review` (a branch to merge) or `done`; sent back, the
  feedback becomes a note ("Sent back: …") and the claimer gets the task again with it. A task with acceptance criteria
  is checked criterion by criterion (`review_task {taskId, checks: [{criterion, pass, note}]}`, every criterion once;
  a failing one needs a note): it is accepted only when all pass, the verdicts are kept on the task (`checks`), and the
  failures go back as the note. After two send-backs the next finish goes to the user (`review`, with a note) instead
  of counting unchecked; with no other bot, a finish settles without a check.

## Work orders (`board/brief.rs`)

A trial on real work (a three-hour workshop's materials) found what decides quality is the brief, not the number of
bots: a checker noted an inconsistency and still passed the work. So a task is a work order:

- **Acceptance criteria** (`acceptance`, up to 12 one-line criteria): what must hold for the result to count as done.
  The user writes them when adding a task (`createTask {acceptance}`, a list or one per line), the lead writes or
  sharpens them in its plan, and every subtask carries its own (`create_task {acceptance}`).
- **Plan first** (`approval`: `notNeeded` / `waiting` / `proposed` / `approved`). A task the user adds waits for its
  claimer's plan unless they turn it off (`createTask {approvePlan: false}`): the claimer calls `propose_plan {taskId,
  plan, acceptance?}` (criteria required by then) and stops; the user gets a *needs you* alert and answers
  `approvePlan {taskId}` (the claimer carries it out) or `revisePlan {taskId, note}` (the note says what to change, the
  claimer proposes again). Until it is approved the task can't be finished or split. Reopening a task asks for a plan
  again. The status stays `claimed`, so older apps show it as in progress.

## A fresh session per job (`board/focus.rs`)

Before a bot is handed board work from a job it wasn't on (planning, an assignment or a check; a job is a task with
its subtasks), its agent session starts afresh (`Cmd::NewSession`), unless it is mid-turn. A trial showed why: a lead
that had built a task once read a new task with the same title as a repeat and stopped. Parts of one goal keep the
session; the user's chat stays as it was.

## Character sheets (`board/sheet.rs`)

Each bot has a sheet, shown like a unit's status screen in a tactics game: the **class** the user gives it, a **level**,
a **rank** (the class is promoted at levels 5 and 10, each promotion adding 6 to every ability), and five **abilities**
(0–100). Kept in kv `board.sheet.<botId>` as the class and the bot's deeds, counted as they happen
so deleting a task takes nothing back.

| Class (wire) | Titles by rank | For | Abilities start at (might, intellect, command, accuracy, insight) |
|---|---|---|---|
| `strategist` | Strategist, Chief Strategist, Grand Strategist | Plans and splits work | 30, 80, 60, 50, 55 |
| `ruler` | Ruler, Overlord, Emperor | Leads, coordinates and assembles | 50, 65, 85, 50, 45 |
| `general` | Commander, High Commander, Grand Commander | Builds code and systems | 80, 45, 55, 55, 35 |
| `infantry` | Infantry, Elite Infantry, Imperial Guard | Steady, well-defined tasks | 60, 40, 35, 70, 40 |
| `martialArtist` | Martial Artist, Martial Master, Martial Saint | Checks finished work | 55, 50, 30, 60, 85 |

No class starts every ability at 40. Deeds: `finished` (accepted work), `firstTry` (accepted without a send-back),
`sentBack`, `checked` (tasks it reviewed), `caught` (reviews that sent work back), `planned` (subtasks it added) and
`notes`. An ability grows by `min(25, 5·√count)`: might with `finished`, intellect with `planned + notes/2`, command
with `planned`, insight with `checked + caught`; accuracy moves from the class's start to the bot's own first-try rate
over its first 10 finished tasks. Experience is `10·finished + 5·firstTry + 4·checked + 3·caught + 6·planned + notes`,
and level `n` starts at `20·(n−1)²`.

The class also steers the board: nudges and the planner's prompt list each teammate's class, and a martial artist is
picked first to check finished work. Methods: `botSheets` (every bot's sheet: `class`, `level`, `rank`,
`nextPromotion`, `xp`, `levelXp`, `nextLevelXp`, `abilities`, `deeds`) and `setBotClass {botId, class|null}`. Stamina is the usage reading
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
`create_task` (project defaults to the bot's folder; `parentId` for a subtask; `assignee` hands it to a teammate;
`acceptance`), `claim_task` (answers `workIn` with the worktree and `workOrder` with the criteria and whether to propose a
plan first), `propose_plan` (`plan`, `acceptance`), `note_task`, `finish_task` (`summary` required), `release_task`
(`reason` becomes a note) and `review_task` (`checks` per criterion, or `approve`; `feedback`). `read_task` includes
`checkedBy`, `parentId`, the `subtasks` with their summaries, `acceptance`, `approval`, `plan` and `checks`.

## App methods and sync

`tasks`, `createTask {title, detail?, project?, acceptance?, approvePlan?}` (the lead plans it), `approvePlan {taskId}`,
`revisePlan {taskId, note}`, `editTask {taskId, title?, detail?}`, `noteTask {taskId, text}`,
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
