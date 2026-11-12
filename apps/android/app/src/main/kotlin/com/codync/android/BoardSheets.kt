package com.codync.android

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.codync.android.core.*
import com.codync.android.design.*
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonPrimitive

/**
 * A new task (iOS `NewTaskSheet`): its title, what to do, and the folder it's in, picked like a bot's workspace (a git
 * project gives each claim its own worktree).
 */
@Composable internal fun NewTaskSheet(state: AppState, store: AppStore, done: () -> Unit) {
    var title by remember { mutableStateOf("") }
    var detail by remember { mutableStateOf("") }
    var criteria by remember { mutableStateOf("") }
    var approvePlan by remember { mutableStateOf(true) }
    var project by remember { mutableStateOf<String?>(null) }
    var picking by remember { mutableStateOf(false) }
    val trimmed = title.trim()
    Column(Modifier.fillMaxSize().background(Palette.background)) {
        ModalHeader("New task") {
            IconButton("Add task", "checkmark", enabled = trimmed.isNotEmpty()) {
                store.createTask(trimmed, detail, project, parseCriteria(criteria), approvePlan, done)
            }
        }
        CardForm {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                FieldBox(title, { title = it }, tr("Task title"))
                FieldBox(detail, { detail = it }, tr("What to do: the result, the materials to use, and what not to do"), minLines = 3, maxLines = 8)
                FieldBox(criteria, { criteria = it }, tr("Acceptance criteria, one per line: what must hold for it to count as done"),
                    minLines = 3, maxLines = 8)
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Row(Modifier.weight(1f).pill().plainButton { picking = true }, verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Symbol("folder", size = 16.dp, tint = Palette.secondary)
                        Text(project ?: tr("Project folder (optional)"), Modifier.weight(1f),
                            style = if (project == null) Type.body else Type.body.copy(fontFamily = FontFamily.Monospace),
                            color = if (project == null) Palette.secondary else Palette.text, maxLines = 1, overflow = TextOverflow.MiddleEllipsis)
                    }
                    if (project != null) IconButton("Clear", "xmark") { project = null }
                }
                CodyncSwitch(approvePlan, { approvePlan = it }, Modifier.fillMaxWidth()) {
                    Text(tr("Approve the plan before work starts"), Modifier.weight(1f), style = Type.body, color = Palette.text)
                }
            }
        }
    }
    CodyncSheet(picking, { picking = false }) {
        val hello = state.hello
        FolderPicker(project ?: hello?.get("home")?.jsonPrimitive?.contentOrNull, state.actualConnection is LinkState.Ready,
            hello?.get("name")?.jsonPrimitive?.contentOrNull ?: "Computer", store::listDirs, store::selectWorkspace) {
            project = it; picking = false
        }
    }
}

/** A whole goal (iOS `SplitGoalSheet`), for the lead bot to split into tasks the bots then claim. */
@Composable internal fun SplitGoalSheet(store: AppStore, handed: (String) -> Unit) {
    var goal by remember { mutableStateOf("") }
    val trimmed = goal.trim()
    Column(Modifier.fillMaxSize().background(Palette.background)) {
        ModalHeader("Split a goal into tasks") {
            IconButton("Split into tasks", "checkmark", enabled = trimmed.isNotEmpty()) { store.splitGoal(trimmed, handed) }
        }
        CardForm {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                FieldBox(goal, { goal = it }, tr("The goal, e.g. prepare both kids’ exam review papers"), minLines = 3, maxLines = 10)
                Text(tr("Your lead bot splits it into tasks, and the bots claim them."), style = Type.caption, color = Palette.secondary)
            }
        }
    }
}

/** Which bots come for open tasks on their own (iOS `BoardMembers`): all, unless turned off here. */
@Composable internal fun BoardMembers(state: AppState, store: AppStore) {
    val bots = roster(state.bots).filter { it.kind != "group" }.sortedBy { it.name.lowercase() }
    if (bots.isEmpty()) return
    CardSection("Bots that claim tasks on their own") {
        for (bot in bots) row {
            CodyncSwitch(bot.id !in state.boardOptedOut, { on -> store.setBoardClaims(bot.id, on) }, Modifier.fillMaxWidth()) {
                Text(bot.name, Modifier.weight(1f).padding(end = 8.dp), style = Type.body, color = Palette.text, maxLines = 1,
                    overflow = TextOverflow.Ellipsis)
            }
        }
    }
}
