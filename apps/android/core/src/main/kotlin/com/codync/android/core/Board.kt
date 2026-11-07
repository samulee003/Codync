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
    val createdAt: Long = 0,
    val updatedAt: Long = 0,
    val rev: Long = 0,
    val deleted: Boolean = false,
) {
    /** The actions its status allows, in menu order. */
    val actions: List<TaskAction> get() = when (status) {
        TaskStatus.Open -> listOf(TaskAction.Assign, TaskAction.Done, TaskAction.Cancel, TaskAction.Delete)
        TaskStatus.Claimed -> listOf(TaskAction.Reopen, TaskAction.Cancel)
        TaskStatus.Review -> if (branch == null) listOf(TaskAction.Done, TaskAction.Reopen, TaskAction.Cancel)
            else listOf(TaskAction.Merge, TaskAction.Reopen, TaskAction.Cancel)
        TaskStatus.Done, TaskStatus.Cancelled -> listOf(TaskAction.Reopen, TaskAction.Delete)
    }

    companion object {
        fun decode(value: JsonElement): BoardTask = WireJson.decodeFromJsonElement<BoardTask>(value).also {
            require(it.id.isNotBlank()) { "The computer sent an invalid task." }
        }
    }
}

@Serializable enum class TaskStatus(val title: String, val symbol: String) {
    @SerialName("open") Open("Open", "circle"),
    @SerialName("claimed") Claimed("In progress", "circle.dotted"),
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

/** The board's columns: each status with its tasks, newest first; empty ones left out. */
fun boardColumns(tasks: Collection<BoardTask>): List<Pair<TaskStatus, List<BoardTask>>> {
    val live = tasks.filterNot { it.deleted }.sortedByDescending { it.updatedAt }
    return TaskStatus.entries.mapNotNull { status -> live.filter { it.status == status }.takeIf { it.isNotEmpty() }?.let { status to it } }
}

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
