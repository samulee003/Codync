package com.codync.android

import com.codync.android.core.*
import kotlinx.coroutines.flow.update
import kotlinx.serialization.json.*

// The computer's team board (docs/features/team-board.md): its tasks come from `tasks` and the `task` events; every change
// goes through the computer, which answers the task as saved.

private fun AppStore.keepTask(answer: JsonElement) {
    val task = BoardTask.decode(answer.jsonObject.getValue("task"))
    mutableState.update { it.copy(tasks = it.tasks.upsertTask(task)) }
}

/** The `task` events in a batch. */
internal fun AppStore.receiveTasks(events: List<MirrorEvent>) {
    val tasks = events.filterIsInstance<MirrorEvent.TaskChanged>().map { it.task }
    if (tasks.isNotEmpty()) mutableState.update { state -> state.copy(tasks = tasks.fold(state.tasks) { board, task -> board.upsertTask(task) }) }
}

fun AppStore.loadBoard() = action("board") {
    val tasks = ready().call("tasks").jsonObject.getValue("tasks").jsonArray.mapNotNull { runCatching { BoardTask.decode(it) }.getOrNull() }
    mutableState.update { state -> state.copy(tasks = tasks.fold(state.tasks) { board, task -> board.upsertTask(task) }) }
}

fun AppStore.createTask(title: String, detail: String, project: String?, onDone: () -> Unit) = action("create-task") {
    keepTask(ready().call("createTask", body("title" to title, "detail" to detail, "project" to project?.ifBlank { null })))
    onDone()
}

fun AppStore.noteTask(task: BoardTask, text: String, onDone: () -> Unit) = action("note-task:${task.id}") {
    keepTask(ready().call("noteTask", body("taskId" to task.id, "text" to text)))
    onDone()
}

fun AppStore.assignTask(task: BoardTask, botId: String) = action("task:${task.id}") {
    keepTask(ready().call("assignTask", body("taskId" to task.id, "botId" to botId)))
}

/** A menu action on a task; `Assign` goes through [assignTask], with the bot. */
fun AppStore.performTask(action: TaskAction, task: BoardTask) = action("task:${task.id}") {
    val channel = ready()
    when (action) {
        TaskAction.Assign -> Unit
        // Merging runs git in the project, which can take a while on a large repository.
        TaskAction.Merge -> keepTask(channel.call("mergeTask", body("taskId" to task.id), timeoutMillis = 120_000))
        TaskAction.Reopen -> keepTask(channel.call("setTaskStatus", body("taskId" to task.id, "status" to TaskStatus.Open.wire)))
        TaskAction.Done -> keepTask(channel.call("setTaskStatus", body("taskId" to task.id, "status" to TaskStatus.Done.wire)))
        TaskAction.Cancel -> keepTask(channel.call("setTaskStatus", body("taskId" to task.id, "status" to TaskStatus.Cancelled.wire)))
        TaskAction.Delete -> {
            channel.call("deleteTask", body("taskId" to task.id))
            mutableState.update { it.copy(tasks = it.tasks - task.id) }
        }
    }
}
