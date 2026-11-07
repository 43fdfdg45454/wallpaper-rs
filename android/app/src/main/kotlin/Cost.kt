package io.github.wallpaper_rs

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.json.JSONObject

fun costWords(auto: String, over: Boolean): Int? = when {
    over -> if (auto == "stop") R.string.auto_would_stop else R.string.auto_would_lower
    auto == "stop" -> R.string.level_triangles
    auto == "still" -> R.string.level_stopped
    else -> null
}

fun stepWords(step: String): Int? = when (step) {
    "model" -> R.string.step_model
    "steady" -> R.string.step_steady
    "climbed" -> R.string.step_climbed
    "dropped" -> R.string.step_dropped
    "top" -> R.string.step_top
    "held" -> R.string.step_held
    "still" -> R.string.step_still
    "halved" -> R.string.step_halved
    "doubled" -> R.string.step_doubled
    else -> null
}

fun levelWords(pace: Int): Int = if (pace > 1) R.string.scene_level_paced else R.string.scene_level

@Composable
fun PaceNote() {
    Text(stringResource(R.string.auto_pace_note), Modifier.padding(16.dp, 4.dp), style = MaterialTheme.typography.bodySmall)
}

@Composable
fun frameNote(report: JSONObject): String? {
    val step = stringResource(stepWords(report.optString("step")) ?: return null)
    if (report.isNull("frame_ms")) return stringResource(R.string.frame_unmeasured, step)
    val ms = { key: String -> "%.1f".format(report.optDouble(key)) }
    return stringResource(R.string.frame_measured, ms("frame_ms"), ms("period_ms"), step)
}

@Composable
fun costNote(report: JSONObject): String? {
    val left = report.optJSONArray("left_out")?.length() ?: 0
    val kept = if (left > 0) stringResource(R.string.level_kept, left) else null
    val over = report.optJSONArray("over")?.let {
        stringResource(R.string.level_over, "%.1f".format(it.optDouble(0)), "%.0f".format(it.optDouble(1)))
    }
    val words = costWords(report.optString("auto"), over != null) ?: return null
    return listOfNotNull(over, stringResource(words), kept).joinToString(" ")
}

@Composable
fun CostWarning(report: JSONObject) {
    costNote(report)?.let {
        Text(it, Modifier.padding(16.dp, 4.dp), MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
    }
}
