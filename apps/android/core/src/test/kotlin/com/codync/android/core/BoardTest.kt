package com.codync.android.core

import kotlinx.serialization.json.*
import org.junit.Assert.*
import org.junit.Test

class BoardTest {
    private fun task(id: String, status: String, updatedAt: Long, branch: String? = null, deleted: Boolean = false) = buildJsonObject {
        put("id", id); put("title", id); put("status", status); put("updatedAt", updatedAt); put("rev", updatedAt)
        put("branch", branch); put("deleted", deleted)
        putJsonArray("notes") { addJsonObject { put("by", JsonNull); put("text", "hi"); put("at", 1) } }
    }

    private fun decode(json: String): BoardTask = BoardTask.decode(Json.parseToJsonElement(json))

    @Test fun `task events join the board and an unreadable one is passed over`() {
        val event = MirrorEvent.decode(buildJsonObject { put("type", "task"); put("task", task("t", "review", 3, branch = "codync/task-t")) })
        val task = (event as MirrorEvent.TaskChanged).task
        assertEquals(TaskStatus.Review, task.status)
        assertEquals("hi", task.notes.single().text)
        assertEquals(listOf(TaskAction.Merge, TaskAction.Reopen, TaskAction.Cancel), task.actions)
        assertEquals(MirrorEvent.Ignored, MirrorEvent.decode(buildJsonObject { put("type", "task"); putJsonObject("task") { put("id", "t") } }))
    }

    @Test fun `columns follow the status order newest first and older copies never win`() {
        val tasks = listOf(task("a", "open", 1), task("b", "open", 3), task("c", "done", 2), task("d", "open", 4, deleted = true)).map(BoardTask::decode)
        val columns = boardColumns(tasks)
        assertEquals(listOf(TaskStatus.Open, TaskStatus.Done), columns.map { it.first })
        assertEquals(listOf("b", "a"), columns.first().second.map { it.id })

        var board = emptyMap<String, BoardTask>().upsertTask(BoardTask.decode(task("a", "claimed", 5)))
        board = board.upsertTask(BoardTask.decode(task("a", "open", 4)))
        assertEquals(TaskStatus.Claimed, board.getValue("a").status)
        board = board.upsertTask(BoardTask.decode(task("a", "open", 6, deleted = true)))
        assertTrue(board.isEmpty())
        assertFalse(hasBoard("2.40.3"))
        assertTrue(hasBoard("2.41.0"))
    }

    @Test fun `a task being checked names its checker and parent`() {
        val task = BoardTask.decode(buildJsonObject {
            put("id", "s"); put("title", "Slides"); put("status", "checking"); put("claimedBy", "ada"); put("reviewer", "bo"); put("parent", "p")
        })
        val parent = BoardTask.decode(buildJsonObject { put("id", "p"); put("title", "Workshop"); put("status", "open") })
        assertEquals(listOf(TaskAction.Done, TaskAction.Reopen, TaskAction.Cancel), task.actions)
        val byline = task.byline({ it.uppercase() }, { if (it == "p") parent else null }, { it })
        assertEquals("ADA · Checked by BO · ↳ Workshop", byline)
        val claimed = task.copy(status = TaskStatus.Claimed)
        assertEquals(TaskAction.Assign, claimed.actions.first())
        assertEquals("ada", claimed.byline({ it }, { null }, { it }))
    }

    @Test fun `a task from an older host decodes with the new fields at their defaults`() {
        val task = decode("""{"id":"o","title":"Old","status":"claimed","notes":[],"createdAt":0,"updatedAt":1,"rev":1}""")
        assertEquals(PlanApproval.NotNeeded, task.approval)
        assertTrue(task.acceptance.isEmpty() && task.checks.isEmpty() && task.questions.isEmpty() && task.after.isEmpty())
        assertNull(task.plan)
        assertNull(task.assignee)
        assertNull(task.effort)
        assertFalse(task.needsUser)
        assertTrue(task.criteria.isEmpty())
    }

    @Test fun `an unknown approval value reads as not needed`() {
        val task = decode("""{"id":"u","title":"New","status":"claimed","approval":"someday","notes":[],"createdAt":0,"updatedAt":1,"rev":1}""")
        assertEquals(PlanApproval.NotNeeded, task.approval)
        assertFalse(task.needsUser)
    }

    @Test fun `a plan waits for the user with its criteria paired to their checks`() {
        val plan = decode("""
            {"id":"p1","title":"Plan","status":"claimed","approval":"proposed","plan":"Do X","acceptance":["A","B"],
             "checks":[{"criterion":"A","pass":true,"note":"ok"}],"notes":[],"createdAt":0,"updatedAt":2,"rev":2}
        """)
        assertTrue(plan.awaitsApproval && plan.needsUser && !plan.awaitsAnswers)
        assertEquals(listOf(true, null), plan.criteria.map { it.check?.pass })
        assertEquals("checkmark.circle.fill", plan.criteria.first().symbol)
        assertEquals("circle", plan.criteria.last().symbol)
        assertTrue(plan.byline({ it }, { null }, { it }).contains("Plan waits for you"))
        assertTrue(boardColumns(listOf(plan)).isEmpty())
        assertEquals(listOf("p1"), waitingForUser(listOf(plan)).map { it.id })
    }

    @Test fun `questions wait for the user and the answers are complete only when every open one is typed`() {
        val task = decode("""
            {"id":"q","title":"Ask","status":"claimed","approval":"asking",
             "questions":[{"text":"Which?","answer":null},{"text":"Done?","answer":"yes"}],"notes":[],"createdAt":0,"updatedAt":1,"rev":1}
        """)
        assertTrue(task.awaitsAnswers && task.needsUser && !task.awaitsApproval)
        assertTrue(task.questions.last().isAnswered)
        assertFalse(task.questions.first().isAnswered)
        assertEquals(listOf("blue", "yes"), task.answers(mapOf(0 to "blue")))
        assertNull(task.answers(mapOf(0 to "  ")))
        assertNull(task.answers(emptyMap()))
    }

    @Test fun `an open task waits for unfinished tasks and names its assignee`() {
        val waiting = decode("""{"id":"w","title":"Wait","status":"open","assignee":"bo","after":["x","d"],"notes":[],"createdAt":0,"updatedAt":1,"rev":1}""")
        val build = decode("""{"id":"x","title":"Build","status":"open","notes":[],"createdAt":0,"updatedAt":1,"rev":1}""")
        val done = decode("""{"id":"d","title":"Old","status":"done","notes":[],"createdAt":0,"updatedAt":1,"rev":1}""")
        val board = mapOf("x" to build, "d" to done)
        assertEquals(listOf("Build"), waiting.waitingFor { board[it] })
        val line = waiting.byline({ it.uppercase() }, { board[it] }, { it })
        assertEquals("Waits for Build · For BO", line)
        assertEquals(listOf(TaskStatus.Open), boardColumns(listOf(waiting)).map { it.first })
    }

    @Test fun `effort is turns and the minutes from start to finish`() {
        val task = decode("""{"id":"e","title":"Work","status":"done","startedAt":0,"finishedAt":420000,"turns":3,"notes":[],"createdAt":0,"updatedAt":1,"rev":1}""")
        assertEquals(TaskEffort(3, 7), task.effort)
        assertEquals("3 turns · 7 min", task.effort!!.line { it })
        assertNull(decode("""{"id":"n","title":"Work","status":"done","turns":0,"notes":[],"createdAt":0,"updatedAt":1,"rev":1}""").effort)
    }

    @Test fun `criteria typed one per line are parsed`() {
        assertEquals(listOf("Tests pass", "Lint clean", "Docs"), parseCriteria("- Tests pass\n\n* Lint clean\n  • Docs  \n"))
    }
}
