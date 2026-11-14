package com.codync.android.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.decodeFromJsonElement

/**
 * A task on the computer's team board (`host/src/board/`): the user and the bots add tasks, a bot claims one, notes what
 * it finds and finishes it. A task in a git project gets its own worktree and branch when claimed.
 */
@Serializable data class BoardTask(
    val id: String,
    val title: String,
    val detail: String = "",
    val project: String? = null,
    val status: TaskStatus,
    val createdBy: String? = null,
    val claimedBy: String? = null,
    val branch: String? = null,
    val worktree: String? = null,
    val notes: List<TaskNote> = emptyList(),
    val summary: String? = null,
    /** The task this one is a part of (a subtask). */
    val parent: String? = null,
    /** The bot that checks (or checked) the finished work. */
    val reviewer: String? = null,
    val createdAt: Long = 0,
    val updatedAt: Long = 0,
    val rev: Long = 0,
    val deleted: Boolean = false,
    /** What must hold for the result to count as done; the checker marks each criterion pass or fail. */
    val acceptance: List<String> = emptyList(),
    /** The plan its claimer proposed, or the approved one. */
    val plan: String? = null,
    val approval: PlanApproval = PlanApproval.NotNeeded,
    /** The checker's latest verdict on each criterion. */
    val checks: List<CriterionCheck> = emptyList(),
    /** The questions the claimer asks the user, with the answers given. */
    val questions: List<TaskQuestion> = emptyList(),
    /** The tasks it waits for: it starts once they are finished. */
    val after: List<String> = emptyList(),
    /** The bot it goes to once nothing it waits for is left. */
    val assignee: String? = null,
    /** Milliseconds since the epoch, when the claimer started and finished it. */
    val startedAt: Long? = null,
    val finishedAt: Long? = null,
    val turns: Int = 0,
) {
    /** The actions its status allows, in menu order. */
    val actions: List<TaskAction> get() = when (status) {
        TaskStatus.Open -> listOf(TaskAction.Assign, TaskAction.Done, TaskAction.Cancel, TaskAction.Delete)
        TaskStatus.Claimed -> listOf(TaskAction.Assign, TaskAction.Reopen, TaskAction.Cancel)
        TaskStatus.Checking -> listOf(TaskAction.Done, TaskAction.Reopen, TaskAction.Cancel)
        TaskStatus.Review -> if (branch == null) listOf(TaskAction.Done, TaskAction.Reopen, TaskAction.Cancel)
            else listOf(TaskAction.Merge, TaskAction.Reopen, TaskAction.Cancel)
        TaskStatus.Done, TaskStatus.Cancelled -> listOf(TaskAction.Reopen, TaskAction.Delete)
    }

    /** The plan waits for the user's approval. */
    val awaitsApproval: Boolean get() = status == TaskStatus.Claimed && approval == PlanApproval.Proposed

    /** The claimer has questions for the user. */
    val awaitsAnswers: Boolean get() = status == TaskStatus.Claimed && approval == PlanApproval.Asking

    /** The user has to act on it: approve the plan or answer the questions. */
    val needsUser: Boolean get() = awaitsApproval || awaitsAnswers

    /** Done, cancelled, or finished with no branch left to merge: out of the way of a task that waits for it. */
    val finishedForOthers: Boolean
        get() = status == TaskStatus.Done || status == TaskStatus.Cancelled || (status == TaskStatus.Review && branch == null)

    /** Each acceptance criterion with the latest check on it. */
    val criteria: List<CriterionResult> get() = acceptance.mapIndexed { index, criterion ->
        CriterionResult(index, criterion, checks.firstOrNull { it.criterion == criterion })
    }

    /** The titles of the unfinished tasks an open task still waits for. */
    fun waitingFor(find: (String) -> BoardTask?): List<String> =
        if (status != TaskStatus.Open) emptyList()
        else after.mapNotNull(find).filter { !it.deleted && !it.finishedForOthers }.map { it.title }

    /** The work spent on it: its turns and the minutes from its start to its finish; null when there is none. */
    val effort: TaskEffort? get() {
        val minutes = if (startedAt != null && finishedAt != null && finishedAt >= startedAt) ((finishedAt - startedAt) / 60_000).toInt() else 0
        return if (turns > 0 || minutes > 0) TaskEffort(turns, minutes) else null
    }

    /** The answers to send: the given ones, and what the user typed for each open question; null while one is blank. */
    fun answers(typed: Map<Int, String>): List<String>? {
        val result = mutableListOf<String>()
        for ((index, question) in questions.withIndex()) {
            if (question.isAnswered) {
                result += question.answer.orEmpty()
            } else {
                val text = typed[index]?.trim().orEmpty()
                if (text.isEmpty()) return null
                result += text
            }
        }
        return result
    }

    /**
     * Its byline, as on the desktop: what it waits for, its assignee, who holds it, what the user has to do, who checks it,
     * the task it is part of, its branch and how many notes it has.
     */
    fun byline(name: (String) -> String, find: (String) -> BoardTask?, tr: (String) -> String): String = listOfNotNull(
        waitingFor(find).takeIf { it.isNotEmpty() }?.let { tr("Waits for {titles}").replace("{titles}", it.joinToString(", ")) },
        assignee?.takeIf { status == TaskStatus.Open }?.let { tr("For {name}").replace("{name}", name(it)) },
        claimedBy?.let(name),
        tr("Plan waits for you").takeIf { awaitsApproval },
        tr("Has questions for you").takeIf { awaitsAnswers },
        reviewer?.takeIf { status == TaskStatus.Checking }?.let { tr("Checked by {name}").replace("{name}", name(it)) },
        parent?.let(find)?.let { "↳ ${it.title}" },
        branch,
        notes.size.takeIf { it > 0 }?.let { tr("{count} notes").replace("{count}", it.toString()) },
    ).joinToString(" · ")

    companion object {
        fun decode(value: JsonElement): BoardTask = WireJson.decodeFromJsonElement<BoardTask>(value).also {
            require(it.id.isNotBlank()) { "The computer sent an invalid task." }
        }
    }
}

@Serializable enum class TaskStatus(val title: String, val symbol: String) {
    @SerialName("open") Open("Open", "circle"),
    @SerialName("claimed") Claimed("In progress", "circle.dotted"),
    @SerialName("checking") Checking("Being checked", "checkmark.seal"),
    @SerialName("review") Review("Ready to merge", "arrow.triangle.merge"),
    @SerialName("done") Done("Done", "checkmark.circle.fill"),
    @SerialName("cancelled") Cancelled("Cancelled", "xmark.circle");

    /** The wire value `setTaskStatus` takes. */
    val wire: String get() = name.lowercase()
}

/** A note on a task; `by` is the bot that wrote it, null for the user. */
@Serializable data class TaskNote(val by: String? = null, val text: String, val at: Long = 0)

enum class TaskAction(val title: String, val icon: String, val destructive: Boolean = false) {
    Assign("Assign to {name}", "person.crop.circle.badge.plus"),
    Merge("Merge branch", "arrow.triangle.merge"),
    Reopen("Reopen", "arrow.uturn.backward"),
    Done("Mark done", "checkmark"),
    Cancel("Cancel task", "xmark"),
    Delete("Delete task", "trash", destructive = true),
}

/** The board's columns: each status with its tasks, newest first; empty ones left out. Tasks that wait for the user are shown by [waitingForUser]. */
fun boardColumns(tasks: Collection<BoardTask>): List<Pair<TaskStatus, List<BoardTask>>> {
    val live = tasks.filterNot { it.deleted || it.needsUser }.sortedByDescending { it.updatedAt }
    return TaskStatus.entries.mapNotNull { status -> live.filter { it.status == status }.takeIf { it.isNotEmpty() }?.let { status to it } }
}

/** The tasks that wait for the user (a plan to approve, questions to answer), newest first. */
fun waitingForUser(tasks: Collection<BoardTask>): List<BoardTask> =
    tasks.filter { !it.deleted && it.needsUser }.sortedByDescending { it.updatedAt }

/** A task from the computer joins the board unless an older copy (a lower rev); a deleted one leaves. */
fun Map<String, BoardTask>.upsertTask(task: BoardTask): Map<String, BoardTask> {
    val known = this[task.id]
    if (known != null && known.rev > task.rev) return this
    return if (task.deleted) this - task.id else this + (task.id to task)
}

/** The first host with the team board. */
const val BOARD_HOST = "2.41.0"

/** Whether the computer's host has the board; an older one has no `tasks` method, so the app leaves it out. */
fun hasBoard(hostVersion: String?): Boolean = hostVersion != null && !AppVersion.isBelow(hostVersion, BOARD_HOST)
