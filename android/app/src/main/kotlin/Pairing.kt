package io.github.wallpaper_rs

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import java.text.DateFormat
import java.util.Date
import org.json.JSONObject

@Composable
fun Pairing(main: Main, status: JSONObject, run: Run) {
    val pairing = status.optJSONObject("pairing") ?: return
    val (name, sharing) = pairing.getString("name") to pairing.getBoolean("sharing")
    var digits by remember(pairing.getString("code")) { mutableStateOf("") }
    val waiting = if (sharing) "" else stringResource(R.string.waiting_key, name)
    val done = stringResource(if (sharing) R.string.key_sent else R.string.key_received, name)
    AlertDialog(
        onDismissRequest = {},
        title = { Text(stringResource(if (sharing) R.string.share_key else R.string.get_key)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.compare, pairing.getString("code"), name))
                OutlinedTextField(
                    digits, { typed -> digits = typed.filter(Char::isDigit).take(6) }, singleLine = true,
                    label = { Text(stringResource(R.string.digits)) },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                )
            }
        },
        confirmButton = {
            TextButton({
                run(waiting) {
                    account("confirm", "code" to digits)
                    main.toast(done)
                }
            }, enabled = digits.length == 6) { Text(stringResource(R.string.confirm)) }
        },
        dismissButton = { TextButton({ main.act(false) { account("cancel") } }) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
fun Approve(main: Main, run: Run, close: () -> Unit) {
    var code by remember { mutableStateOf("") }
    var found by remember { mutableStateOf<JSONObject?>(null) }
    val approved = stringResource(R.string.approved)
    AlertDialog(
        onDismissRequest = close,
        title = { Text(stringResource(R.string.approve_device)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.approve_warning))
                OutlinedTextField(
                    code, { code = it }, readOnly = found != null, singleLine = true,
                    label = { Text(stringResource(R.string.device_code)) },
                )
                found?.let { device ->
                    val client = device.text("client")?.ifEmpty { null } ?: stringResource(R.string.unnamed)
                    val time = DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT).format(Date(device.optLong("requested_at")))
                    Text(stringResource(R.string.asked, device.optString("platform"), client, time))
                }
            }
        },
        confirmButton = {
            if (found == null) TextButton({ main.act(false) { found = account("preview", "code" to code) as? JSONObject } }, enabled = code.isNotBlank()) {
                Text(stringResource(R.string.look_up))
            } else TextButton({
                close()
                run("") {
                    account("approve", "code" to code, "approve" to true)
                    main.toast(approved)
                }
            }) { Text(stringResource(R.string.approve)) }
        },
        dismissButton = { TextButton(close) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
fun Server(run: Run) = Column(Modifier.padding(16.dp), Arrangement.spacedBy(12.dp)) {
    var url by rememberSaveable { mutableStateOf("") }
    Text(stringResource(R.string.sync_intro))
    OutlinedTextField(url, { url = it }, Modifier.fillMaxWidth(), label = { Text(stringResource(R.string.server)) }, singleLine = true)
    Button({ run("") { account("add", "url" to url.trim()) } }, enabled = url.isNotBlank()) {
        Text(stringResource(R.string.add_server))
    }
}

@Composable
fun Link(status: JSONObject, run: Run) = Column(Modifier.padding(16.dp), Arrangement.spacedBy(12.dp)) {
    val phone = stringResource(R.string.this_phone)
    var name by rememberSaveable { mutableStateOf("") }
    var plain by rememberSaveable { mutableStateOf(false) }
    Text(stringResource(R.string.server_named, status.server()))
    OutlinedTextField(
        name, { name = it }, Modifier.fillMaxWidth(), singleLine = true,
        label = { Text(stringResource(R.string.phone_label)) }, placeholder = { Text(phone) },
    )
    Toggle(stringResource(R.string.plain_text), plain) { plain = it }
    Button({
        run("") {
            account("link", "label" to name.ifBlank { phone }, "plaintext" to plain)
            account("wait")
        }
    }) { Text(stringResource(R.string.link)) }
    TextButton({ run("") { account("remove") } }) { Text(stringResource(R.string.remove_server)) }
}

@Composable
fun Code(main: Main, link: JSONObject) = Column(Modifier.padding(16.dp), Arrangement.spacedBy(12.dp)) {
    Text(stringResource(R.string.approve_here, link.getString("uri")))
    Text(link.getString("code"), style = MaterialTheme.typography.displaySmall)
    Waiting(main, "")
}
