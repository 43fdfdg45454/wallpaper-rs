package io.github.wallpaper_rs

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts.OpenDocument
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject

private val STILL = mapOf("saver" to R.string.still_saver, "battery" to R.string.still_battery, "heat" to R.string.still_heat)

@Composable
fun Relief(main: Main, uid: String, state: JSONObject) {
    val context = LocalContext.current
    val busy = Depth.working.value
    var consent by remember { mutableStateOf<JSONObject?>(null) }
    val library = context.applicationInfo.nativeLibraryDir + "/libonnxruntime.so"
    val off = stringResource(R.string.downloads_off)
    val open = rememberLauncherForActivityResult(OpenDocument()) { uri ->
        val file = uri?.let { runCatching { context.contentResolver.openFileDescriptor(it, "r") }.getOrNull() }
        file?.let { Depth.import(uid, it) }
    }
    val make: () -> Unit = {
        main.act {
            val model = withContext(Dispatchers.IO) { depth("model") as JSONObject }
            when {
                model.getBoolean("installed") -> Depth.make(uid, library)
                model.getBoolean("allowed") -> consent = model
                else -> main.toast(off)
            }
        }
    }
    val tune = { change: Pair<String, Any> -> main.act { withContext(Dispatchers.IO) { depth("parallax", "uid" to uid, change) } } }
    val moving = state.optBoolean("parallax")
    Toggle(stringResource(R.string.parallax), moving) { tune("on" to it) }
    if (moving) {
        Level(stringResource(R.string.strength), state.optDouble("strength", 0.02).toFloat(), MIN_STRENGTH..MAX_STRENGTH) { tune("strength" to it) }
        Level(stringResource(R.string.focus), state.optDouble("focus", 0.3).toFloat(), 0f..1f) { tune("focus" to it) }
        STILL[state.optString("paused")]?.let { Note(stringResource(it), MaterialTheme.colorScheme.error) }
    }
    val map = when {
        busy == uid -> R.string.depth_making
        state.optBoolean("map") -> if (moving) R.string.depth_ready else R.string.depth_kept
        else -> if (moving) R.string.depth_none else R.string.depth_missing
    }
    Note(stringResource(map))
    Row(Modifier.padding(horizontal = 8.dp), Arrangement.spacedBy(8.dp), Alignment.CenterVertically) {
        val again = if (state.optBoolean("map")) R.string.remake_map else R.string.make_map
        TextButton(make, enabled = busy == null) { Text(stringResource(again)) }
        TextButton({ open.launch(arrayOf(PNG)) }, enabled = busy == null) { Text(stringResource(R.string.import_map)) }
    }
    if (busy == uid) Row(Modifier.padding(horizontal = 16.dp), Arrangement.spacedBy(8.dp), Alignment.CenterVertically) {
        val share = Depth.progress.floatValue
        val bar = Modifier.weight(1f)
        if (share < 0f) LinearProgressIndicator(bar) else LinearProgressIndicator({ share }, bar)
        if (share >= 0f) TextButton({ Depth.cancelled = true }) { Text(stringResource(R.string.cancel)) }
    }
    Depth.failure.value?.takeIf { it.first == uid && busy == null }?.let { (_, text) -> Note(text, MaterialTheme.colorScheme.error) }
    consent?.let { model ->
        AlertDialog(
            onDismissRequest = { consent = null },
            confirmButton = {
                TextButton({
                    consent = null
                    Depth.make(uid, library)
                }) { Text(stringResource(R.string.download)) }
            },
            dismissButton = { TextButton({ consent = null }) { Text(stringResource(R.string.cancel)) } },
            title = { Text(stringResource(R.string.download_model)) },
            text = {
                val terms = listOf("title", "size", "hosts", "license", "license_url").map(model::getString)
                Text(stringResource(R.string.model_terms, *terms.toTypedArray()))
            },
        )
    }
}

@Composable
private fun Level(label: String, saved: Float, range: ClosedFloatingPointRange<Float>, save: (Float) -> Unit) = Column(Modifier.padding(horizontal = 16.dp)) {
    var value by remember(saved) { mutableFloatStateOf(saved.coerceIn(range)) }
    Text(label, style = MaterialTheme.typography.bodyMedium)
    Slider(value, { value = it }, valueRange = range, onValueChangeFinished = { save(value) })
}

private const val PNG = "image/png"
private const val MIN_STRENGTH = 0.002f
private const val MAX_STRENGTH = 0.03f
