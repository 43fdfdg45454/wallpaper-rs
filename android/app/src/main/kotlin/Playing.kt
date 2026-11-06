package io.github.wallpaper_rs

import android.content.Intent
import android.provider.Settings
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.json.JSONObject

internal object MediaConsent {
    fun asks(input: String, wanted: Boolean): Boolean = wanted && input == "media_network"
}

@Composable
internal fun MediaGrants(form: JSONObject, grant: (String, Boolean) -> Unit) {
    val (uses, granted) = form.optJSONObject("uses") to form.optJSONObject("granted")
    if (uses?.optBoolean("media") != true) return
    val context = LocalContext.current
    val access = Players.allowed(context)
    var asking by remember { mutableStateOf(false) }
    val on = granted?.optBoolean("media") == true
    Toggle(stringResource(R.string.show_media), on) { wanted ->
        grant("media", wanted)
        if (wanted && !access) asking = true
    }
    val note = if (access) R.string.media_note else R.string.media_access_off
    Text(stringResource(note), Modifier.padding(16.dp, 4.dp), style = MaterialTheme.typography.bodySmall)
    if (on && access) {
        val shown = Players.status
        val word = { yes: Boolean -> context.getString(if (yes) R.string.media_yes else R.string.media_no) }
        val line = if (!shown.listening) stringResource(R.string.media_status_off)
        else stringResource(R.string.media_status, shown.players, word(shown.track), word(shown.playing), word(shown.cover))
        Text(line, Modifier.padding(16.dp, 4.dp), style = MaterialTheme.typography.bodySmall)
    }
    if (on && !access) TextButton({ asking = true }, Modifier.padding(horizontal = 8.dp)) { Text(stringResource(R.string.media_access_open)) }
    var confirming by remember { mutableStateOf(false) }
    if (on && uses.optBoolean("network")) {
        Toggle(stringResource(R.string.show_media_network), granted.optBoolean("media_network")) { wanted ->
            if (MediaConsent.asks("media_network", wanted)) confirming = true else grant("media_network", wanted)
        }
        Text(stringResource(R.string.media_network_note), Modifier.padding(16.dp, 4.dp), style = MaterialTheme.typography.bodySmall)
    }
    if (confirming) AlertDialog(
        onDismissRequest = { confirming = false },
        title = { Text(stringResource(R.string.show_media_network)) },
        text = { Text(stringResource(R.string.media_network_why)) },
        confirmButton = {
            TextButton({
                confirming = false
                grant("media_network", true)
            }) { Text(stringResource(R.string.media_network_allow)) }
        },
        dismissButton = { TextButton({ confirming = false }) { Text(stringResource(android.R.string.cancel)) } },
    )
    if (asking) AlertDialog(
        onDismissRequest = { asking = false },
        title = { Text(stringResource(R.string.media_access_title)) },
        text = { Text(stringResource(R.string.media_access_why)) },
        confirmButton = {
            TextButton({
                asking = false
                context.startActivity(Intent(Settings.ACTION_NOTIFICATION_LISTENER_SETTINGS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            }) { Text(stringResource(R.string.media_access_open)) }
        },
        dismissButton = { TextButton({ asking = false }) { Text(stringResource(android.R.string.cancel)) } },
    )
}
