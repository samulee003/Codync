package com.codync.android.core

import kotlinx.serialization.KSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull

/** Whether the user approves the claimer's plan before work starts; `Asking`: the claimer has questions instead. */
@Serializable(with = PlanApprovalSerializer::class)
enum class PlanApproval(val wire: String) {
    NotNeeded("notNeeded"), Waiting("waiting"), Asking("asking"), Proposed("proposed"), Approved("approved"),
}

/** Reads the wire value; an unknown or unreadable one is `NotNeeded`, so a newer host's task still shows. */
object PlanApprovalSerializer : KSerializer<PlanApproval> {
    override val descriptor: SerialDescriptor = PrimitiveSerialDescriptor("PlanApproval", PrimitiveKind.STRING)

    override fun serialize(encoder: Encoder, value: PlanApproval) = encoder.encodeString(value.wire)

    override fun deserialize(decoder: Decoder): PlanApproval {
        val wire = (decoder as? JsonDecoder)?.decodeJsonElement()?.let { (it as? JsonPrimitive)?.contentOrNull }
        return PlanApproval.entries.firstOrNull { it.wire == wire } ?: PlanApproval.NotNeeded
    }
}

/** The checker's verdict on one acceptance criterion. */
@Serializable data class CriterionCheck(val criterion: String, val pass: Boolean, val note: String = "")

/** A question the claimer asks the user; `answer` is set once the user has answered it. */
@Serializable data class TaskQuestion(val text: String, val answer: String? = null) {
    /** Answered with something to show (an empty answer still waits). */
    val isAnswered: Boolean get() = !answer.isNullOrEmpty()
}

/** One acceptance criterion with the latest check on it, if it was checked. */
data class CriterionResult(val index: Int, val criterion: String, val check: CriterionCheck?) {
    /** The mark it shows: passed, failed, or not checked yet. */
    val symbol: String get() = when (check?.pass) {
        null -> "circle"
        true -> "checkmark.circle.fill"
        false -> "xmark.circle.fill"
    }

    /** Its label, matching the mark (translated by the view). */
    val title: String get() = when (check?.pass) {
        null -> "Not checked yet"
        true -> "Passed"
        false -> "Failed"
    }
}

/** The work spent on a task: its turns, and the minutes from its start to its finish. */
data class TaskEffort(val turns: Int, val minutes: Int) {
    /** Its line on a task card, translated with `tr`. */
    fun line(tr: (String) -> String): String = tr("{turns} turns · {minutes} min")
        .replace("{turns}", turns.toString())
        .replace("{minutes}", minutes.toString())
}

/** Acceptance criteria typed one per line: list marks and leading spaces dropped, blank lines left out. */
fun parseCriteria(text: String): List<String> = text.lines()
    .map { it.trimStart { c -> c.isWhitespace() || c in "-*•" }.trim() }
    .filter { it.isNotEmpty() }
