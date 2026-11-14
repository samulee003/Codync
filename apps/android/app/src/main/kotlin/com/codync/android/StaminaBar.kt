package com.codync.android

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.codync.android.core.Stamina
import com.codync.android.design.*
import com.kyant.shapes.Capsule

/**
 * A game-style stamina bar under a bot's avatar (iOS `StaminaBar`): how much of its agent's plan is left (the tightest
 * limit), green while plenty, amber under half, red when nearly out.
 */
@Composable internal fun StaminaBar(stamina: Stamina, width: Dp) {
    val fill by animateFloatAsState(stamina.left / 100f, Motion.hover(), label = "Stamina")
    val color = when (stamina.level) {
        Stamina.Level.Full -> Palette.added
        Stamina.Level.Low -> Palette.warning
        Stamina.Level.Out -> Palette.danger
    }
    val label = "${tr(stamina.label)}: ${tr("{percent}% left").replace("{percent}", stamina.left.toString())}"
    Box(Modifier.width(width).height(4.dp).clip(Capsule()).background(Palette.text.copy(alpha = 0.12f))
        .semantics { contentDescription = label }) {
        Box(Modifier.fillMaxHeight().fillMaxWidth(fill).clip(Capsule()).background(color))
    }
}
