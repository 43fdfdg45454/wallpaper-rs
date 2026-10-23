package io.github.wallpaper_rs

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts.OpenDocument
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
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

@Composable
fun Relief(main: Main, uid: String) {
    val context = LocalContext.current
    val busy = Depth.working.value
    val state by loaded(JSONObject(), uid, main.revision.intValue, busy) { depth("item", "uid" to uid) as JSONObject }
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
    Toggle(stringResource(R.string.parallax), state.optBoolean("parallax")) { on ->
        main.act { withContext(Dispatchers.IO) { depth("parallax", "uid" to uid, "on" to on) } }
    }
    Row(Modifier.padding(horizontal = 8.dp), Arrangement.spacedBy(8.dp), Alignment.CenterVertically) {
        val map = if (state.optBoolean("map")) R.string.remake_map else R.string.make_map
        TextButton(make, enabled = busy == null) { Text(stringResource(map)) }
        TextButton({ open.launch(arrayOf(PNG)) }, enabled = busy == null) { Text(stringResource(R.string.import_map)) }
    }
    if (busy == uid) Row(Modifier.padding(horizontal = 16.dp), Arrangement.spacedBy(8.dp), Alignment.CenterVertically) {
        val share = Depth.progress.floatValue
        val bar = Modifier.weight(1f)
        if (share < 0f) LinearProgressIndicator(bar) else LinearProgressIndicator({ share }, bar)
        if (share >= 0f) TextButton({ Depth.cancelled = true }) { Text(stringResource(R.string.cancel)) }
    }
    Depth.failure.value?.takeIf { it.first == uid && busy == null }?.let { (_, text) ->
        val small = MaterialTheme.typography.bodySmall
        Text(text, Modifier.padding(16.dp, 4.dp), MaterialTheme.colorScheme.error, style = small)
    }
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

private const val PNG = "image/png"
