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

private fun optedOut(answer: JsonObject): Set<String>? =
    answer["optedOut"]?.let { it as? JsonArray }?.mapNotNull { (it as? JsonPrimitive)?.contentOrNull }?.toSet()

fun AppStore.loadBoard() = action("board") {
    val answer = ready().call("tasks").jsonObject
    val tasks = answer.getValue("tasks").jsonArray.mapNotNull { runCatching { BoardTask.decode(it) }.getOrNull() }
    mutableState.update { state ->
        state.copy(tasks = tasks.fold(state.tasks) { board, task -> board.upsertTask(task) }, boardOptedOut = optedOut(answer) ?: emptySet())
    }
}

fun AppStore.setBoardClaims(botId: String, on: Boolean) = action("board-claims:$botId") {
    val answer = ready().call("setBoardClaims", buildJsonObject { put("botId", botId); put("on", on) }).jsonObject
    mutableState.update { it.copy(boardOptedOut = optedOut(answer) ?: it.boardOptedOut) }
}

/** Hands the goal to the lead bot, which splits it into tasks; `handed` gets its name. */
fun AppStore.splitGoal(goal: String, handed: (String) -> Unit) = action("split-goal") {
    val answer = ready().call("splitGoal", body("goal" to goal)).jsonObject
    handed(answer["name"]?.jsonPrimitive?.contentOrNull ?: "")
}

/** A new task; with `approvePlan`, its claimer's plan waits for the user before any work starts. */
fun AppStore.createTask(title: String, detail: String, project: String?, acceptance: List<String>, approvePlan: Boolean, onDone: () -> Unit) =
    action("create-task") {
        val fields = buildJsonObject {
            put("title", title)
            put("detail", detail)
            project?.ifBlank { null }?.let { put("project", it) }
            put("acceptance", buildJsonArray { acceptance.forEach { add(JsonPrimitive(it)) } })
            put("approvePlan", approvePlan)
        }
        keepTask(ready().call("createTask", fields))
        onDone()
    }

fun AppStore.approvePlan(task: BoardTask) = action("task:${task.id}") {
    keepTask(ready().call("approvePlan", body("taskId" to task.id)))
}

/** Sends the plan back with what to change. */
fun AppStore.revisePlan(task: BoardTask, note: String, onDone: () -> Unit) = action("task:${task.id}") {
    keepTask(ready().call("revisePlan", body("taskId" to task.id, "note" to note)))
    onDone()
}

/** One answer per question, in order. */
fun AppStore.answerQuestions(task: BoardTask, answers: List<String>, onDone: () -> Unit) = action("task:${task.id}") {
    val list = buildJsonArray { answers.forEach { add(JsonPrimitive(it)) } }
    keepTask(ready().call("answerQuestions", buildJsonObject { put("taskId", task.id); put("answers", list) }))
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
