package io.github.wallpaper_rs

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.repeatOnLifecycle
import java.text.DateFormat
import java.util.Date
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import org.json.JSONObject

typealias Run = (String, suspend () -> Unit) -> Unit

@Composable
fun Account(main: Main) {
    var changes by remember { mutableIntStateOf(0) }
    var task by remember { mutableStateOf<String?>(null) }
    var approving by remember { mutableStateOf(false) }
    val first = remember { Sync.status() }
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    val status by produceState(first, changes, task, lifecycle) {
        lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
            while (true) {
                value = withContext(Dispatchers.IO) { Sync.status() }
                if (task == null && value.isNull("link") && value.isNull("pairing")) break
                delay(1500)
            }
        }
    }
    val run: Run = { waiting, work ->
        main.act(false) {
            task = waiting
            try {
                work()
            } finally {
                task = null
                changes++
                Sync.schedule(main)
            }
        }
    }
    val state = status.optString("state")
    LazyColumn {
        item { Header(stringResource(R.string.devices)) }
        when {
            state == "none" -> item { Server(run) }
            !status.isNull("link") -> item { Code(main, status.getJSONObject("link")) }
            state == "unlinked" -> item { Link(status, run) }
            else -> {
                item { Summary(main, status, run) { approving = true } }
                items(devices(status).filterNot { it.optBoolean("current") }) { Device(it, state == "needs_key", run) }
            }
        }
        task?.takeIf { status.isNull("link") && status.isNull("pairing") }?.let { waiting ->
            item { Waiting(main, waiting) }
        }
    }
    Pairing(main, status, run)
    if (approving) Approve(main, run) { approving = false }
}

fun devices(status: JSONObject) =
    status.optJSONArray("devices")?.let { list -> List(list.length()) { list.getJSONObject(it) } }.orEmpty()

fun JSONObject.text(key: String): String? = if (isNull(key)) null else getString(key)

fun JSONObject.server() = text("name")?.ifEmpty { null } ?: getString("url")

@Composable
fun Waiting(main: Main, text: String) = Column(Modifier.padding(16.dp), Arrangement.spacedBy(8.dp)) {
    if (text.isNotEmpty()) Text(text)
    LinearProgressIndicator(Modifier.fillMaxWidth())
    TextButton({ main.act(false) { account("cancel") } }) { Text(stringResource(R.string.cancel)) }
}

@Composable
private fun Summary(main: Main, status: JSONObject, run: Run, approve: () -> Unit) =
    Column(Modifier.padding(16.dp), Arrangement.spacedBy(8.dp)) {
        var removing by remember { mutableStateOf(false) }
        Text(stringResource(R.string.server_named, status.server()))
        Text(stringResource(R.string.this_device, status.optString("label"), status.optString("device")))
        if (status.optString("state") == "needs_key") Text(stringResource(R.string.needs_key)) else {
            val last = status.optLong("last_sync")
            val time = DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT).format(Date(last * 1000))
            Text(if (last > 0) stringResource(R.string.last_sync, time) else stringResource(R.string.never_synced))
            Text(stringResource(if (status.optBoolean("encrypted")) R.string.encrypted else R.string.not_encrypted))
            if (status.optBoolean("encrypted")) Text(stringResource(R.string.names_key), style = MaterialTheme.typography.bodySmall)
            val outdated = stringResource(R.string.outdated).takeIf { status.optBoolean("outdated") }
            val revoked = stringResource(R.string.recovery_revoked).takeIf { status.optBoolean("recovery_stale") }
            val failed = status.optJSONObject("last_reply")?.let(::refusal)?.let(main::told) ?: status.text("last_error")
            listOfNotNull(outdated ?: failed, status.text("warning"), revoked).forEach {
                Text(it, color = MaterialTheme.colorScheme.error)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                val conflict = main.getString(R.string.conflict, "%1")
                Button({ run("") { account("sync", "conflict" to conflict) } }) { Text(stringResource(R.string.sync_now)) }
                OutlinedButton(approve) { Text(stringResource(R.string.approve_device)) }
            }
            Toggle(stringResource(R.string.pause_sync), status.optBoolean("paused")) {
                run("") { account("pause", "paused" to it) }
            }
            Toggle(stringResource(R.string.mobile_data), status.optString("network") == "any") {
                run("") { withContext(Dispatchers.IO) { Native.setting("sync_network", if (it) "any" else "unmetered") } }
            }
        }
        TextButton({ removing = true }) { Text(stringResource(R.string.remove_server)) }
        if (removing) AlertDialog(
            onDismissRequest = { removing = false },
            confirmButton = {
                TextButton({
                    removing = false
                    run("") { account("remove") }
                }) { Text(stringResource(R.string.remove)) }
            },
            dismissButton = { TextButton({ removing = false }) { Text(stringResource(R.string.cancel)) } },
            text = { Text(stringResource(R.string.remove_server_question)) },
        )
    }

@Composable
private fun Device(device: JSONObject, join: Boolean, run: Run) {
    val name = device.text("label") ?: device.getString("name")
    val waiting = stringResource(if (join) R.string.waiting_join else R.string.waiting_share, name)
    ListItem(
        headlineContent = { Text(name) },
        supportingContent = { Text(device.getString("name") + " · " + device.optString("platform")) },
        trailingContent = {
            TextButton({ run(waiting) { account(if (join) "join" else "share", "device" to device.getString("id")) } }) {
                Text(stringResource(if (join) R.string.get_key else R.string.share_key))
            }
        },
    )
}
