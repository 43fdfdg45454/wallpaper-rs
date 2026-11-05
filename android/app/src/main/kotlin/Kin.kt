package io.github.wallpaper_rs

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalLocale
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.json.JSONObject

@Composable
fun Kin(main: Main, uid: String, open: (String) -> Unit) {
    val languages = listOf(LocalLocale.current.toLanguageTag().replace('-', '_'))
    val kin by loaded<JSONObject?>(null, uid, main.revision.intValue) {
        ask("kin", "uid" to uid, "languages" to languages) as? JSONObject
    }
    kin?.optJSONObject("base")?.let { base ->
        val found = base.optJSONObject("item")
        Note(stringResource(R.string.preset_note))
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
            val about = found?.let { stringResource(R.string.preset_of, it.getString("title")) }
                ?: stringResource(R.string.preset_of_missing, base.getString("id"))
            Text(about, Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium)
            if (found != null) TextButton({ open(found.getString("uid")) }) { Text(stringResource(R.string.preset_show)) }
            else if (!base.isNull("repo")) Button({ main.act { account("install_base", "uid" to uid) } }) {
                Text(stringResource(R.string.preset_install))
            }
        }
    }
    val presets = kin?.optJSONArray("presets") ?: return
    if (presets.length() == 0) return
    Text(stringResource(R.string.presets_of), Modifier.padding(16.dp, 8.dp, 16.dp, 0.dp), style = MaterialTheme.typography.titleSmall)
    (0 until presets.length()).map(presets::getJSONObject).forEach { preset ->
        ListItem(headlineContent = { Text(preset.getString("title")) }, modifier = Modifier.clickable { open(preset.getString("uid")) })
    }
}
