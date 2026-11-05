package io.github.wallpaper_rs

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.json.JSONObject

fun costWords(stops: Boolean, chosen: Boolean, over: Boolean): Int? = when {
    over -> if (stops) R.string.auto_would_stop else R.string.auto_would_lower
    stops && chosen -> R.string.level_triangles
    stops -> R.string.level_stopped
    else -> null
}

@Composable
fun costNote(report: JSONObject): String? {
    val stops = report.optString("auto") == "stop"
    val left = report.optJSONArray("left_out")?.length() ?: 0
    val kept = if (left > 0) stringResource(R.string.level_kept, left) else null
    val over = report.optJSONArray("over")?.let {
        stringResource(R.string.level_over, "%.1f".format(it.optDouble(0)), "%.0f".format(it.optDouble(1)))
    }
    val words = costWords(stops, report.optString("reason") == "user", over != null) ?: return null
    return listOfNotNull(over, stringResource(words), kept).joinToString(" ")
}

@Composable
fun CostWarning(report: JSONObject) {
    costNote(report)?.let {
        Text(it, Modifier.padding(16.dp, 4.dp), MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
    }
}
