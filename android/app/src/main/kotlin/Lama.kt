package io.github.wallpaper_rs

import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.res.stringResource
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject

@Composable
fun LamaNote(main: Main, fill: String?, changes: Int) {
    Note(stringResource(R.string.fill_note))
    if (fill != "lama") return
    var consent by remember { mutableStateOf<JSONObject?>(null) }
    val lama by loaded(JSONObject(), changes, Depth.working.value) { depth("lama") as JSONObject }
    val busy = Depth.working.value != null
    val off = stringResource(R.string.downloads_off)
    if (lama.optBoolean("installed")) {
        ListItem(
            headlineContent = { Text(stringResource(R.string.lama_model, lama.optString("size"))) },
            trailingContent = {
                TextButton({ main.act { withContext(Dispatchers.IO) { depth("forget_lama") } } }, enabled = !busy) {
                    Text(stringResource(R.string.delete_model))
                }
            },
        )
    } else if (lama.has("files")) {
        Note(stringResource(R.string.lama_missing), MaterialTheme.colorScheme.error)
        TextButton({ if (lama.optBoolean("allowed")) consent = lama else main.toast(off) }, enabled = !busy) {
            Text(stringResource(R.string.download_lama))
        }
    }
    consent?.let { model ->
        AlertDialog(
            onDismissRequest = { consent = null },
            confirmButton = {
                TextButton({
                    consent = null
                    Depth.fetchLama()
                }) { Text(stringResource(R.string.download)) }
            },
            dismissButton = { TextButton({ consent = null }) { Text(stringResource(R.string.cancel)) } },
            title = { Text(stringResource(R.string.download_lama)) },
            text = {
                val terms = listOf("title", "size", "hosts", "license", "license_url").map(model::getString)
                Text(stringResource(R.string.lama_terms, *terms.toTypedArray()))
            },
        )
    }
}
