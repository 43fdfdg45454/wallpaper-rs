package io.github.wallpaper_rs

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource

private val REASONS = listOf(
    "it is too slow for this graphics card" to R.string.reason_slow,
    "it stopped the graphics card or the program twice" to R.string.reason_blocked,
    "the graphics card or the program stopped twice" to R.string.reason_unbuilt,
    "the graphics card or the program stopped" to R.string.reason_stopped,
    "it kept the graphics card busy" to R.string.reason_busy,
    "its shader could not be checked" to R.string.reason_unchecked,
    "the effects of the scene would do more than" to R.string.reason_work,
    "the scene has more than" to R.string.reason_states,
    "its state textures would take" to R.string.reason_memory,
    "its textures would take more than" to R.string.reason_textures,
)

@Composable
fun leftOutReason(reason: String): String {
    val found = REASONS.firstOrNull { reason.startsWith(it.first) } ?: return reason
    val text = stringResource(found.second)
    return if (text.startsWith(found.first)) reason else text
}

fun levelReason(reason: String): Int? = when (reason) {
    "device" -> R.string.level_device
    "cost" -> R.string.level_cost
    "battery" -> R.string.level_battery
    "saver" -> R.string.level_saver
    "heat" -> R.string.level_heat
    "slow" -> R.string.level_slow
    else -> null
}
