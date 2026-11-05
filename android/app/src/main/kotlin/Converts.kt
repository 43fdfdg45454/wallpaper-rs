package io.github.wallpaper_rs

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject

fun Main.convert(uid: String, name: String, asked: Boolean, keep: Boolean = Convert.chosen() == Convert.KEEP): String? {
    if (!asked && Convert.chosen() == Convert.OFF) return null
    val path = JSONObject(Native.library(JSONObject().put("op", "item").put("uid", uid).toString())).optString("path")
    val size = path.takeIf { it.isNotEmpty() }?.let(Convert::plan) ?: return null
    if (!asked && Convert.saving(this)) return Convert.SAVER
    Converting.offer(this, Conversion(uid, path, File(name).nameWithoutExtension.ifEmpty { name }, keep, size, asked))
    return Convert.STARTED
}

@Composable
fun Conversions(only: String? = null) {
    Converting.progress.toList().filter { only == null || it.first == only }.forEach { (uid, done) ->
        Row(Modifier.fillMaxWidth().padding(16.dp, 4.dp), Arrangement.spacedBy(8.dp), Alignment.CenterVertically) {
            Text(stringResource(R.string.converting, Converting.names[uid].orEmpty(), (done * 100).toInt()), Modifier.weight(1f))
            TextButton({ Converting.stop(uid) }) { Text(stringResource(R.string.convert_stop)) }
        }
        LinearProgressIndicator({ done }, Modifier.fillMaxWidth().padding(16.dp, 0.dp))
    }
}

@Composable
fun ConvertVideo(main: Main, item: Item) {
    val converting = item.uid in Converting.progress
    val plan by loaded<Pair<Int, Int>?>(null, item.path, converting) { Convert.plan(item.path) }
    if (converting) return Conversions(item.uid)
    plan ?: return
    val start = { keep: Boolean -> main.act(false) { withContext(Dispatchers.IO) { main.convert(item.uid, item.title, asked = true, keep = keep) } }.let { } }
    Note(stringResource(R.string.convert_note))
    Row(Modifier.padding(16.dp, 4.dp), Arrangement.spacedBy(8.dp)) {
        Button({ start(false) }) { Text(stringResource(R.string.convert_now)) }
        OutlinedButton({ start(true) }) { Text(stringResource(R.string.convert_keep)) }
    }
}
