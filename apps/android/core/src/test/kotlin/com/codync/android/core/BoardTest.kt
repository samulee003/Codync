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
}
