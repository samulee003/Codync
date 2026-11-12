package com.codync.android

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import com.codync.android.core.*
import com.codync.android.design.*

/**
 * A task's work order in its card (iOS `TaskWorkOrder`): the acceptance criteria, each with the latest check's verdict; the
 * plan, and while it waits for the user, approving it or sending it back with what to change; the questions the claimer
 * asks, answered here.
 */
@Composable internal fun TaskWorkOrder(task: BoardTask, store: AppStore) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        if (task.criteria.isNotEmpty()) CriteriaList(task.criteria)
        task.plan?.let { plan ->
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(tr(if (task.approval == PlanApproval.Approved) "Approved plan" else "Proposed plan"),
                    style = Type.caption.semibold, color = Palette.secondary)
                Text(plan, style = Type.callout, color = Palette.text)
            }
        }
        if (task.awaitsApproval) PlanDecision(task, store)
        if (task.questions.isNotEmpty()) QuestionList(task, store)
    }
}

/** Each criterion with its mark (passed, failed, not checked yet) and the checker's note. */
@Composable private fun CriteriaList(criteria: List<CriterionResult>) {
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(tr("Acceptance criteria"), style = Type.caption.semibold, color = Palette.secondary)
        for (item in criteria) Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Symbol(item.symbol, size = 13.dp, tint = criterionTint(item.check), contentDescription = tr(item.title))
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(item.criterion, style = Type.callout, color = Palette.text)
                item.check?.note?.takeIf { it.isNotEmpty() }?.let { Text(it, style = Type.caption, color = Palette.secondary) }
            }
        }
    }
}

@Composable private fun criterionTint(check: CriterionCheck?): Color = when {
    check == null -> Palette.secondary
    check.pass -> Palette.added
    else -> Palette.danger
}

/** Approve the proposed plan, or send it back saying what to change. */
@Composable private fun PlanDecision(task: BoardTask, store: AppStore) {
    var change by remember(task.id) { mutableStateOf("") }
    val trimmed = change.trim()
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        FieldBox(change, { change = it }, tr("What to change in the plan"), imeAction = ImeAction.Send, actions = KeyboardActions(onSend = {
            if (trimmed.isNotEmpty()) store.revisePlan(task, trimmed) { change = "" }
        }))
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
            SecondaryButton("Ask for changes", enabled = trimmed.isNotEmpty()) { store.revisePlan(task, trimmed) { change = "" } }
            PrimaryButton("Approve plan") { store.approvePlan(task) }
        }
    }
}

/** The questions the claimer asks: answered ones read-only; while the task waits, a field for each open one. */
@Composable private fun QuestionList(task: BoardTask, store: AppStore) {
    val typed = remember(task.id) { mutableStateMapOf<Int, String>() }
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(tr("Questions for you"), style = Type.caption.semibold, color = Palette.secondary)
        for ((index, question) in task.questions.withIndex()) Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(question.text, style = Type.callout, color = Palette.text)
            if (question.isAnswered) {
                Text(question.answer.orEmpty(), style = Type.callout, color = Palette.secondary)
            } else if (task.awaitsAnswers) {
                FieldBox(typed[index].orEmpty(), { typed[index] = it }, tr("Your answer"))
            }
        }
        if (task.awaitsAnswers) {
            val answers = task.answers(typed)
            PrimaryButton("Send answers", enabled = answers != null) {
                if (answers != null) store.answerQuestions(task, answers) { typed.clear() }
            }
        }
    }
}
