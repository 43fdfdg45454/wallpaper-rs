package io.github.wallpaper_rs

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts.OpenDocument
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ListItem
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import org.json.JSONObject

typealias Ask = (String, Boolean, (String) -> Unit) -> Unit

typealias Go = (String, Array<out Pair<String, Any?>>) -> Unit

const val NOTE = 200

@Composable
fun Stocked(stocked: JSONObject, busy: Boolean, ask: Ask, go: Go) {
    val id = stocked.getString("id")
    val versions = stocked.optJSONArray("versions").objects()
    val (listed, featured) = versions.any { it.optBoolean("listed") } to stocked.optBoolean("featured")
    val listing = stringResource(if (listed) R.string.stall_ask_unlist else R.string.stall_ask_list, id)
    val picking = stringResource(if (featured) R.string.stall_ask_unfeature else R.string.stall_ask_feature, id)
    val (revoked, hidden) = stringResource(R.string.stall_revoked) to stringResource(R.string.stall_unlisted_mark)
    ListItem(
        headlineContent = { Text((if (featured) "★ " else "") + id) },
        supportingContent = {
            Column {
                val by = stocked.optString("author").takeUnless { stocked.isNull("author") }?.let { stringResource(R.string.stall_by, it) }
                (stocked.optString("note").takeUnless { stocked.isNull("note") } ?: by)?.let { Text(it) }
                Row {
                    TextButton({ ask(listing, false) { go("list", arrayOf("id" to id, "listed" to !listed)) } }, enabled = !busy) {
                        Text(stringResource(if (listed) R.string.stall_unlist else R.string.stall_list_again))
                    }
                    TextButton({ ask(picking, !featured) { note -> go("feature", arrayOf("id" to id, "note" to note.ifBlank { null }, "on" to !featured)) } }, enabled = !busy) {
                        Text(stringResource(if (featured) R.string.stall_unfeature else R.string.stall_feature))
                    }
                }
                versions.forEach { release ->
                    val (version, shown) = release.getString("version") to release.optBoolean("listed")
                    val marks = listOfNotNull(revoked.takeIf { release.optBoolean("revoked") }, hidden.takeUnless { shown })
                    val asking = stringResource(if (shown) R.string.stall_ask_unlist_version else R.string.stall_ask_list_version, id, version)
                    Row(Modifier.padding(start = 16.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text((listOf(version, titled(release.opt("title"))) + marks).joinToString(" · "), Modifier.weight(1f))
                        TextButton({ ask(asking, false) { go("list", arrayOf("id" to id, "version" to version, "listed" to !shown)) } }, enabled = !busy) {
                            Text(stringResource(if (shown) R.string.stall_unlist_version else R.string.stall_list_version))
                        }
                    }
                }
            }
        },
    )
}

@Composable
fun Asking(asked: Asked, close: () -> Unit) {
    var note by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = close,
        title = { Text(stringResource(R.string.stall_change)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(asked.text)
                if (asked.noting) OutlinedTextField(note, { note = it.take(NOTE).replace("\n", " ") }, Modifier.fillMaxWidth(), singleLine = true, label = { Text(stringResource(R.string.stall_note)) })
            }
        },
        confirmButton = {
            TextButton({
                close()
                asked.run(note)
            }) { Text(stringResource(R.string.stall_confirm)) }
        },
        dismissButton = { TextButton(close) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
fun Publishing(close: () -> Unit, publish: (String, String, Int) -> Unit) {
    val context = LocalContext.current
    var key by remember { mutableStateOf<String?>(null) }
    var password by remember { mutableStateOf("") }
    var days by remember { mutableStateOf("7") }
    val chosen = rememberLauncherForActivityResult(OpenDocument()) { uri ->
        uri?.let { key = runCatching { context.contentResolver.openInputStream(it)?.use { input -> input.reader().readText() } }.getOrNull() }
    }
    val lasting = days.toIntOrNull()?.takeIf { it in 1..365 }
    AlertDialog(
        onDismissRequest = close,
        title = { Text(stringResource(R.string.stall_publish_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(if (key == null) R.string.stall_key_hint else R.string.stall_key_chosen))
                TextButton({ chosen.launch(arrayOf("*/*")) }) { Text(stringResource(R.string.stall_key)) }
                OutlinedTextField(password, { password = it }, Modifier.fillMaxWidth(), singleLine = true, visualTransformation = PasswordVisualTransformation(),
                    label = { Text(stringResource(R.string.stall_password)) }, supportingText = { Text(stringResource(R.string.stall_password_hint)) })
                OutlinedTextField(days, { days = it.filter(Char::isDigit).take(3) }, Modifier.fillMaxWidth(), singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), label = { Text(stringResource(R.string.stall_days)) })
            }
        },
        confirmButton = {
            TextButton({
                close()
                publish(key.orEmpty(), password, lasting ?: 7)
            }, enabled = key != null && lasting != null) { Text(stringResource(R.string.stall_publish_now)) }
        },
        dismissButton = { TextButton(close) { Text(stringResource(R.string.cancel)) } },
    )
}
