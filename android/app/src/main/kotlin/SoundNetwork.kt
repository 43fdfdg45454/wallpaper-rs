package io.github.wallpaper_rs

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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.json.JSONObject

@Composable
internal fun SoundNetworkGrants(form: JSONObject, grant: (String, Boolean) -> Unit) {
    val (uses, granted) = form.optJSONObject("uses") to form.optJSONObject("granted")
    if (uses?.optBoolean("audio") != true || !uses.optBoolean("network") || granted?.optBoolean("audio") != true) return
    var confirming by remember { mutableStateOf(false) }
    Toggle(stringResource(R.string.audio_network_title), granted.optBoolean("audio_network")) { wanted ->
        if (MediaConsent.asks("audio_network", wanted)) confirming = true else grant("audio_network", wanted)
    }
    Text(stringResource(R.string.audio_network_note), Modifier.padding(16.dp, 4.dp), style = MaterialTheme.typography.bodySmall)
    if (confirming) AlertDialog(
        onDismissRequest = { confirming = false },
        title = { Text(stringResource(R.string.audio_network_title)) },
        text = { Text(stringResource(R.string.audio_network_why)) },
        confirmButton = {
            TextButton({
                confirming = false
                grant("audio_network", true)
            }) { Text(stringResource(R.string.media_network_allow)) }
        },
        dismissButton = { TextButton({ confirming = false }) { Text(stringResource(android.R.string.cancel)) } },
    )
}
