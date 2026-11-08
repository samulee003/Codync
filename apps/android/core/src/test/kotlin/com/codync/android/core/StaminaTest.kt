package com.codync.android.core

import kotlinx.serialization.json.*
import org.junit.Assert.*
import org.junit.Test

class StaminaTest {
    @Test fun `each agent's stamina reads by its backend and picks its color`() {
        val usage = buildJsonObject {
            putJsonObject("stamina") {
                putJsonObject("codex") { put("left", 33); put("label", "Weekly"); put("resetsAt", 5) }
                putJsonObject("claude") { put("left", 80); put("label", "5h") }
                putJsonObject("cursor") { put("left", 10); put("label", "Weekly") }
            }
        }
        assertEquals(Stamina(33, "Weekly", 5), Stamina.of(usage, "codex"))
        assertEquals(Stamina.Level.Low, Stamina.of(usage, "codex")?.level)
        assertEquals(Stamina.Level.Full, Stamina.of(usage, "claude")?.level)
        assertEquals(Stamina.Level.Out, Stamina.of(usage, "cursor")?.level)
        assertNull("no reading: no bar", Stamina.of(usage, "gemini"))
        assertNull("a host before the bars", Stamina.of(JsonObject(emptyMap()), "codex"))
    }
}
