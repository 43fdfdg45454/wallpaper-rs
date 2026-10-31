package io.github.wallpaper_rs

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject

val PICTURE_FIT = FIT + ("tile" to R.string.tile)

@Composable
fun fitChoices(): List<Pair<String, String>> =
    listOf("" to stringResource(R.string.like_settings)) + PICTURE_FIT.map { (value, label) -> value to stringResource(label) }

@Composable
fun Framing(main: Main, uid: String, state: JSONObject) {
    val fit = state.optString("fit").takeIf { chosen -> PICTURE_FIT.any { it.first == chosen } }.orEmpty()
    Choices(stringResource(R.string.fit), fitChoices(), fit) { chosen -> main.fit(listOf(uid), chosen) }
    Row(Modifier.padding(start = 16.dp, end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(stringResource(R.string.frame_hint), Modifier.weight(1f), style = MaterialTheme.typography.bodySmall)
        TextButton({ main.act { withContext(Dispatchers.IO) { depth("unframe", "uid" to uid) } } }, enabled = !state.isNull("crop")) {
            Text(stringResource(R.string.reset_frame))
        }
    }
}

fun Main.fit(uids: List<String>, fit: String, done: () -> Unit = {}) = act {
    val set = withContext(Dispatchers.IO) { depth("fit", "uids" to uids, "fit" to fit.ifEmpty { null }) } as? Int ?: 0
    if (set < uids.size) toast(getString(R.string.fit_pictures, set, uids.size))
    done()
}

fun Modifier.framing(main: Main, uid: String, screen: IntSize): Modifier = pointerInput(uid, screen) {
    awaitEachGesture {
        awaitFirstDown(requireUnconsumed = false)
        var moved = false
        try {
            do {
                val event = awaitPointerEvent()
                val (pan, zoom) = event.calculatePan() to event.calculateZoom()
                if (size.width > 0 && size.height > 0 && (pan != Offset.Zero || zoom != 1f)) {
                    moved = true
                    main.frame(uid, screen, floatArrayOf(pan.x / size.width, pan.y / size.height), zoom, false)
                    event.changes.forEach { it.consume() }
                }
            } while (event.changes.any { it.pressed })
        } finally {
            if (moved) main.frame(uid, screen, floatArrayOf(0f, 0f), 1f, true)
        }
    }
}

private fun Main.frame(uid: String, screen: IntSize, pan: FloatArray, zoom: Float, done: Boolean) = Render.post {
    val size = intArrayOf(screen.width, screen.height)
    val failed = runCatching { depth("frame", "uid" to uid, "pan" to pan, "zoom" to zoom, "screen" to size, "done" to done) }
    Wallpaper.wake()
    if (done || failed.isFailure) runOnUiThread {
        failed.exceptionOrNull()?.let(::toast)
        if (done) changed()
    }
}
