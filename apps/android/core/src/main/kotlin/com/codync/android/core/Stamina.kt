package com.codync.android.core

import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.longOrNull

/**
 * How much of a bot's agent's plan is left (`host/src/usage/stamina.rs`): the game-style bar under the bot's avatar.
 * [left] is the share of the agent's tightest limit still left, 0–100.
 */
data class Stamina(val left: Int, val label: String, val resetsAt: Long? = null, val resetsText: String? = null) {
    enum class Level { Full, Low, Out }

    /** The bar's color: full, under half, nearly out. */
    val level: Level get() = when { left >= 50 -> Level.Full; left >= 20 -> Level.Low; else -> Level.Out }

    companion object {
        /** The stamina of agent [backend] in the host's `usage`, or null (no reading, or a host before the bars). */
        fun of(usage: JsonObject, backend: String): Stamina? {
            val entry = (usage["stamina"] as? JsonObject)?.get(backend) as? JsonObject ?: return null
            val left = entry["left"]?.jsonPrimitive?.intOrNull ?: return null
            return Stamina(left.coerceIn(0, 100), entry["label"]?.jsonPrimitive?.contentOrNull ?: "",
                entry["resetsAt"]?.jsonPrimitive?.longOrNull, entry["resetsText"]?.jsonPrimitive?.contentOrNull)
        }
    }
}
