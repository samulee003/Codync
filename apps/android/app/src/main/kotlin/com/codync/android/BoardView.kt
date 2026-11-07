package com.codync.android

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.codync.android.core.*
import com.codync.android.design.*

/**
 * The computer's team board (iOS `BoardView`, docs/features/team-board.md): tasks the user and the bots share, by status.
 * The user adds tasks, hands one to a bot, merges a finished task's branch, and reopens, closes or deletes tasks; bots claim
 * and finish them through their own tools.
 */
@Composable internal fun BoardView(state: AppState, store: AppStore) {
    val online = state.actualConnection is LinkState.Ready
    AnalyticsOnAppear(AnalyticsEvent.ScreenViewed(AnalyticsScreen.Board))
    LaunchedEffect(online) { if (online) store.loadBoard() }
    var adding by remember { mutableStateOf(false) }
    var expanded by remember { mutableStateOf<String?>(null) }
    val columns = boardColumns(state.tasks.values)
    Column(Modifier.fillMaxSize().background(Palette.background)) {
        ModalHeader("Team board") { IconButton("New task", "plus") { adding = true } }
        CardForm {
            Text(tr("Tasks your bots split, claim and finish together"), style = Type.callout, color = Palette.secondary)
            state.error?.let { Text(tr(it), style = Type.caption, color = Palette.danger) }
            when {
                columns.isNotEmpty() -> for ((status, tasks) in columns) Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    SectionHeading("${tr(status.title)} · ${tasks.size}", Modifier.padding(start = 2.dp))
                    for (task in tasks) TaskRow(task, state, store, task.id == expanded) { expanded = if (expanded == task.id) null else task.id }
                }
                "board" in state.busy -> Box(Modifier.fillMaxWidth().padding(top = 40.dp), contentAlignment = Alignment.Center) { Spinner(20.dp) }
                else -> Column(Modifier.outlinedCard(), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(tr("No tasks yet"), style = Type.callout.semibold, color = Palette.text)
                    Text(tr("Add a task with +, or ask a bot to split a job into tasks."), style = Type.callout, color = Palette.secondary)
                }
            }
        }
    }
    CodyncSheet(adding, { adding = false }) { NewTaskSheet(store) { adding = false } }
}

@Composable private fun TaskRow(task: BoardTask, state: AppState, store: AppStore, expanded: Boolean, toggle: () -> Unit) {
    Column(Modifier.outlinedCard(vertical = 10.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Row(Modifier.weight(1f).plainButton(action = toggle), verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Symbol(task.status.symbol, size = 16.dp, tint = Palette.secondary)
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(task.title, style = Type.body.medium, color = Palette.text, maxLines = 2, overflow = TextOverflow.Ellipsis)
                    val subtitle = subtitle(task, state)
                    if (subtitle.isNotEmpty()) Text(subtitle, style = Type.caption, color = Palette.secondary, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
            }
            val assignTo = tr(TaskAction.Assign.title)
            DropdownMenu({ menuItems(task, state, store, assignTo) }, Modifier.size(32.dp), label = tr("Task actions")) {
                Symbol("ellipsis", size = 16.dp, tint = Palette.secondary)
            }
        }
        AnimatedVisibility(expanded) { TaskDetail(task, state, store) }
    }
}

/** Who holds it, its branch, and how many notes it has. */
@Composable private fun subtitle(task: BoardTask, state: AppState): String = listOfNotNull(
    task.claimedBy?.let { id -> state.bots.firstOrNull { it.id == id }?.name ?: id },
    task.branch,
    task.notes.size.takeIf { it > 0 }?.let { tr("{count} notes").replace("{count}", it.toString()) },
).joinToString(" · ")

/** A task's menu: one row per action; assigning lists this computer's bots, each its own row (`assignTo`: "Assign to {name}"). */
private fun menuItems(task: BoardTask, state: AppState, store: AppStore, assignTo: String): List<MenuItem> =
    task.actions.flatMap { action ->
        if (action == TaskAction.Assign) roster(state.bots).filter { it.kind != "group" }.sortedBy { it.name.lowercase() }.map { bot ->
            MenuItem(assignTo.replace("{name}", bot.name), icon = action.icon, verbatim = true) { store.assignTask(task, bot.id) }
        } else listOf(MenuItem(action.title, icon = action.icon, destructive = action.destructive) { store.performTask(action, task) })
    }

/** An unfolded task: its detail, where its work is, its summary and notes, and a field to add a note. */
@Composable private fun TaskDetail(task: BoardTask, state: AppState, store: AppStore) {
    var note by remember(task.id) { mutableStateOf("") }
    val you = tr("You")
    Column(Modifier.padding(start = 26.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        if (task.detail.isNotEmpty()) Text(task.detail, style = Type.callout, color = Palette.text)
        (task.worktree ?: task.project)?.let {
            Text(it, style = Type.caption.copy(fontFamily = FontFamily.Monospace), color = Palette.secondary, maxLines = 1, overflow = TextOverflow.MiddleEllipsis)
        }
        task.summary?.let { Text(it, style = Type.callout, color = Palette.text) }
        for (n in task.notes) {
            val by = n.by?.let { id -> state.bots.firstOrNull { it.id == id }?.name ?: id } ?: you
            Text("$by: ${n.text}", style = Type.callout, color = Palette.secondary)
        }
        FieldBox(note, { note = it }, tr("Add a note for the team"), imeAction = ImeAction.Send, actions = KeyboardActions(onSend = {
            val text = note.trim()
            if (text.isNotEmpty()) store.noteTask(task, text) { note = "" }
        }))
    }
}

/** A new task: its title, what to do, and the folder it's in (a git project gives each claim its own worktree). */
@Composable private fun NewTaskSheet(store: AppStore, done: () -> Unit) {
    var title by remember { mutableStateOf("") }
    var detail by remember { mutableStateOf("") }
    var project by remember { mutableStateOf("") }
    val trimmed = title.trim()
    Column(Modifier.fillMaxSize().background(Palette.background)) {
        ModalHeader("New task") {
            IconButton("Add task", "checkmark", enabled = trimmed.isNotEmpty()) { store.createTask(trimmed, detail, project.trim(), done) }
        }
        CardForm {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                FieldBox(title, { title = it }, tr("Task title"))
                FieldBox(detail, { detail = it }, tr("What to do, and how to check it is done"), minLines = 3, maxLines = 8)
                FieldBox(project, { project = it }, tr("Project folder (optional)"), style = Type.body.copy(fontFamily = FontFamily.Monospace),
                    capitalize = false)
            }
        }
    }
}
