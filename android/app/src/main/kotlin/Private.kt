package io.github.wallpaper_rs

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import org.json.JSONArray
import org.json.JSONObject

@Composable
fun Phrase(title: String, close: () -> Unit, done: (String) -> Unit) {
    var phrase by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = close,
        confirmButton = { TextButton({ done(phrase) }, enabled = phrase.isNotBlank()) { Text(title) } },
        dismissButton = { TextButton(close) { Text(stringResource(R.string.cancel)) } },
        title = { Text(stringResource(R.string.recovery_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.recovery_warning))
                OutlinedTextField(
                    phrase, { phrase = it }, Modifier.fillMaxWidth(), singleLine = true,
                    label = { Text(stringResource(R.string.passphrase)) }, visualTransformation = PasswordVisualTransformation(),
                )
            }
        },
    )
}

@Composable
fun Recovery(status: JSONObject, run: Run) = Column(Modifier.padding(horizontal = 16.dp), Arrangement.spacedBy(4.dp)) {
    var asking by remember { mutableStateOf<String?>(null) }
    val waiting = stringResource(R.string.restoring)
    val rotating = stringResource(R.string.rotating)
    if (!status.optBoolean("encrypted")) return@Column
    Text(stringResource(R.string.recovery_title), style = MaterialTheme.typography.titleSmall)
    if (status.optString("state") == "needs_key") {
        TextButton({ asking = "restore" }) { Text(stringResource(R.string.restore)) }
    } else {
        val shown = if (status.optString("recovery") == "off") R.string.recovery_off else R.string.recovery_current
        Text(stringResource(shown), style = MaterialTheme.typography.bodyMedium)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            TextButton({ asking = "escrow" }) { Text(stringResource(R.string.recovery_set)) }
            if (status.optString("recovery") != "off") TextButton({ run("") { account("escrow", "passphrase" to "") } }) {
                Text(stringResource(R.string.recovery_remove))
            }
        }
        if (status.optString("state") == "ready") TextButton({ run(rotating) { account("rotate") } }) {
            Text(stringResource(R.string.rotate_key))
        }
    }
    val shared = status.optJSONArray("secrets")?.let { list -> List(list.length(), list::getString) }.orEmpty()
    if (shared.isNotEmpty()) Text(stringResource(R.string.secrets_shared, shared.joinToString()), style = MaterialTheme.typography.bodySmall)
    asking?.let { op ->
        val label = stringResource(if (op == "restore") R.string.restore else R.string.recovery_set)
        Phrase(label, { asking = null }) { phrase ->
            asking = null
            run(if (op == "restore") waiting else "") { account(op, "passphrase" to phrase) }
        }
    }
}

@Composable
fun PrivateFiles(main: Main, status: JSONObject, run: Run) {
    if (!status.optBoolean("encrypted") || status.optString("state") == "needs_key") return
    val files by loaded(JSONArray(), status.optLong("last_sync"), main.revision.intValue) {
        (account("files") as? JSONObject)?.optJSONArray("files") ?: JSONArray()
    }
    Column(Modifier.padding(horizontal = 16.dp)) {
        Text(stringResource(R.string.private_files), style = MaterialTheme.typography.titleSmall)
        if (files.length() == 0) Text(stringResource(R.string.private_none), style = MaterialTheme.typography.bodySmall)
    }
    List(files.length(), files::getJSONObject).forEach { file ->
        val name = file.optString("name")
        val note = when {
            !file.optBoolean("stored") -> stringResource(R.string.private_waiting)
            !file.isNull("item") -> stringResource(R.string.private_here)
            else -> null
        }
        val fetching = stringResource(R.string.private_fetching, name)
        ListItem(
            headlineContent = { Text(name) },
            supportingContent = { Text(listOfNotNull(size(file.optLong("size")), note).joinToString(" · ")) },
            trailingContent = {
                if (file.isNull("item")) TextButton({ run(fetching) { account("fetch", "id" to file.getString("id")) } }) {
                    Text(stringResource(R.string.private_fetch))
                }
            },
        )
    }
}

fun size(bytes: Long) = if (bytes >= 1_000_000) "%.1f MB".format(bytes / 1e6) else "%d KB".format((bytes + 999) / 1000)

@Composable
fun Kept(main: Main, uid: String) {
    val shown by loaded(false to false, uid, main.revision.intValue) {
        val status = Sync.status()
        val files = (account("files") as? JSONObject)?.optJSONArray("files") ?: JSONArray()
        val shared = List(files.length(), files::getJSONObject).any { it.optString("item") == uid && it.optBoolean("shared") }
        (status.optBoolean("encrypted") && status.optString("state") == "ready") to shared
    }
    val (ready, shared) = shown
    if (ready) Toggle(stringResource(R.string.private_share), shared) {
        main.act { account("share_files", "targets" to listOf(uid), "share" to it) }
    }
}
