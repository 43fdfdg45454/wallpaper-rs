package io.github.wallpaper_rs

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts.OpenDocumentTree
import androidx.activity.result.contract.ActivityResultContracts.OpenMultipleDocuments
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Checkbox
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlin.coroutines.cancellation.CancellationException
import kotlinx.coroutines.launch
import org.json.JSONObject

class Asked(val text: String, val noting: Boolean, val run: (String) -> Unit)

fun titled(title: Any?): String = when (title) {
    is JSONObject -> title.keys().asSequence().firstOrNull()?.let(title::optString).orEmpty()
    else -> title?.toString().orEmpty()
}

fun Main.stallSaid(reply: JSONObject): String = when {
    reply.has("published") -> reply.getJSONObject("published").let {
        getString(R.string.stall_published, it.getLong("version"), it.getInt("packages"), it.getString("expires"))
    }
    reply.has("added") -> getString(R.string.stall_added, reply.getJSONArray("added").strings().joinToString(", "))
    else -> getString(R.string.stall_changed)
}

@Composable
fun Stall(main: Main) {
    var tree by rememberSaveable { mutableStateOf<String?>(null) }
    var shown by remember(tree) { mutableStateOf<JSONObject?>(null) }
    var said by remember(tree) { mutableStateOf("") }
    var asked by remember { mutableStateOf<Asked?>(null) }
    var publishing by remember { mutableStateOf(false) }
    var unlisted by rememberSaveable { mutableStateOf(false) }
    var busy by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    fun act(op: String, vararg args: Pair<String, Any?>, wpaks: List<Uri> = emptyList()) {
        val at = Uri.parse(tree ?: return)
        busy = true
        scope.launch {
            runCatching { main.stalled(at, op, *args, wpaks = wpaks) }.onFailure { if (it !is CancellationException) main.toast(it) }.onSuccess { reply ->
                shown = reply
                said = if (op == "open") "" else main.stallSaid(reply)
            }
            busy = false
        }
    }
    val folder = rememberLauncherForActivityResult(OpenDocumentTree()) { picked -> picked?.let { tree = it.toString() } }
    val wpaks = rememberLauncherForActivityResult(OpenMultipleDocuments()) { files -> if (files.isNotEmpty()) act("add", "unlisted" to unlisted, wpaks = files) }
    LaunchedEffect(tree) { if (tree != null) act("open") }
    Column(Modifier.padding(horizontal = 16.dp), Arrangement.spacedBy(8.dp)) {
        Text(stringResource(R.string.stall), style = MaterialTheme.typography.titleSmall)
        Text(shown?.let { it.getString("name") + " · " + it.getString("repo_id").take(16) } ?: stringResource(R.string.stall_hint))
        Row(verticalAlignment = Alignment.CenterVertically) {
            TextButton({ folder.launch(null) }, enabled = !busy) { Text(stringResource(R.string.stall_open)) }
            if (shown != null) {
                TextButton({ wpaks.launch(arrayOf("*/*")) }, enabled = !busy) { Text(stringResource(R.string.stall_add)) }
                TextButton({ publishing = true }, enabled = !busy) { Text(stringResource(R.string.stall_publish)) }
            }
        }
        if (shown != null) Row(verticalAlignment = Alignment.CenterVertically) {
            Checkbox(unlisted, { unlisted = it })
            Column {
                Text(stringResource(R.string.stall_unlisted))
                Text(stringResource(R.string.stall_unlisted_note), style = MaterialTheme.typography.bodySmall)
            }
        }
        if (busy) Text(stringResource(R.string.stall_busy))
        if (busy) LinearProgressIndicator(Modifier.fillMaxWidth())
        if (said.isNotEmpty()) Text(said, color = MaterialTheme.colorScheme.primary)
    }
    shown?.optJSONArray("packages").objects().forEach { stocked ->
        Stocked(stocked, busy, { text, noting, then -> asked = Asked(text, noting, then) }) { op, args -> act(op, *args) }
    }
    asked?.let { Asking(it) { asked = null } }
    if (publishing) Publishing({ publishing = false }) { key, password, days ->
        act("publish", "key" to key, "password" to password.ifEmpty { null }, "days" to days)
    }
}
