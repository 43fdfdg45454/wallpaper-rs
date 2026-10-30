package io.github.wallpaper_rs

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import org.json.JSONObject

val FIT =
    listOf("fill" to R.string.fill, "fit" to R.string.whole, "stretch" to R.string.stretch, "center" to R.string.center)
private val COLORS = listOf("chosen" to R.string.when_chosen, "always" to R.string.always, "never" to R.string.never)
private val BACKENDS = listOf("auto" to R.string.automatic, "vulkan" to R.string.vulkan, "gl" to R.string.gl)
private val CHOICES = listOf(
    Triple(R.string.fit, "fit", FIT),
    Triple(R.string.battery_fps, "playback.max_fps", listOf("30", "15", "10").map { it to R.string.fps }),
    Triple(R.string.charger_fps, "charger_fps", listOf("60", "30").map { it to R.string.fps }),
    Triple(R.string.system_colors, "system_colors", COLORS),
    Triple(R.string.backend, "backend", BACKENDS),
)
private val TOGGLES = listOf(
    Triple(R.string.fade, "transition.kind", "fade" to "none"),
    Triple(R.string.video_sound, "playback.muted", "false" to "true"),
    Triple(R.string.allow_download, "models.allow_download", "true" to "false"),
    Triple(R.string.flash_guard, "playback.flash_guard", "true" to "false"),
)
private val KEYS = CHOICES.map { it.second } + TOGGLES.map { it.second }
private val NOTICES = listOf("onnxruntime", "onnxruntime-third-party-notices", "depth-anything-v2")

@Composable
fun Settings(main: Main) {
    var changes by remember { mutableIntStateOf(0) }
    var notice by remember { mutableStateOf<String?>(null) }
    val values by loaded(emptyMap<String, String>(), changes) { KEYS.associateWith { Native.setting(it, null) } }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    val report by produceState(JSONObject(), lifecycle) {
        lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
            while (true) {
                value = withContext(Dispatchers.IO) { JSONObject(Native.diagnostics()) }
                delay(1000)
            }
        }
    }
    val change = { work: () -> Any? ->
        main.act {
            withContext(Dispatchers.IO) { work() }
            changes++
        }
    }
    val set = { key: String, value: String -> change { Native.setting(key, value) } }
    LazyColumn {
        item { Header(stringResource(R.string.settings)) }
        items(CHOICES) { (label, key, options) ->
            val texts = options.map { (value, text) -> value to stringResource(text, value.toIntOrNull() ?: 0) }
            Choices(stringResource(label), texts, values[key].orEmpty()) { set(key, it) }
        }
        items(TOGGLES) { (label, key, states) ->
            val (on, off) = states
            Toggle(stringResource(label), values[key] == on) { set(key, if (it) on else off) }
        }
        item {
            val model by loaded(JSONObject(), changes, Depth.working.value) { depth("model") as JSONObject }
            if (model.optBoolean("installed")) ListItem(
                headlineContent = { Text(stringResource(R.string.depth_model, model.getString("size"))) },
                trailingContent = {
                    TextButton({ change { depth("forget") } }, enabled = Depth.working.value == null) {
                        Text(stringResource(R.string.delete_model))
                    }
                },
            )
        }
        item {
            Header(stringResource(R.string.diagnostics))
            val answers = stringResource(R.string.no) to stringResource(R.string.yes)
            val yes = { flag: String -> if (report.optBoolean(flag)) answers.second else answers.first }
            listOfNotNull(
                stringResource(R.string.gpu, report.optString("gpu").ifEmpty { stringResource(R.string.not_started) }),
                stringResource(R.string.frames, report.optLong("frames"), report.optInt("fps")),
                stringResource(R.string.cap, report.optInt("cap")),
                stringResource(R.string.power, yes("charging"), yes("saver"), report.optInt("thermal")),
                stringResource(R.string.scene_level, report.optString("tier"), report.optString("quality"))
                    .takeIf { report.optString("tier").isNotEmpty() },
                stringResource(R.string.particles_cpu).takeIf { report.optString("particles") == "cpu" },
            ).plus(
                report.optJSONArray("left_out").let { left ->
                    (0 until (left?.length() ?: 0)).map {
                        val effect = left!!.getJSONObject(it)
                        stringResource(R.string.left_out, effect.optString("id"), leftOutReason(effect.optString("reason")))
                    }
                },
            ).forEach { Text(it, Modifier.padding(16.dp, 4.dp), style = MaterialTheme.typography.bodyMedium) }
            if ((report.optJSONArray("left_out")?.length() ?: 0) > 0) {
                TextButton({ change { Native.retry() } }, Modifier.padding(8.dp, 0.dp)) {
                    Text(stringResource(R.string.retry_effects))
                }
            }
        }
        item { Header(stringResource(R.string.licenses)) }
        items(NOTICES) { ListItem(headlineContent = { Text(it) }, modifier = Modifier.clickable { notice = it }) }
    }
    notice?.let { Notice(it) { notice = null } }
}

@Composable
private fun Notice(name: String, close: () -> Unit) {
    val assets = LocalContext.current.assets
    val lines by loaded(emptyList<String>(), name) { assets.open("$name.txt").bufferedReader().use { it.readLines() } }
    AlertDialog(
        onDismissRequest = close,
        confirmButton = { TextButton(close) { Text(stringResource(R.string.close)) } },
        title = { Text(name) },
        text = { LazyColumn { items(lines) { Text(it, style = MaterialTheme.typography.bodySmall) } } },
    )
}
