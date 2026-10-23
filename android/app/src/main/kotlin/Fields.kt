package io.github.wallpaper_rs

import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.DateRange
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Share
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
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
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.json.JSONObject

@Composable
fun Field(field: JSONObject, folded: Boolean, fold: () -> Unit, set: (String, Any?) -> Unit, press: () -> Unit, secret: (String) -> Unit) {
    val (key, label) = field.getString("key") to field.getString("label")
    val (value, enabled) = field.opt("value") to field.optBoolean("enabled", true)
    val padded = Modifier.fillMaxWidth().padding(16.dp, 4.dp)
    when (field.getString("kind")) {
        "slider" -> Slide(field, padded, enabled) { set(key, it) }
        "bool" -> Toggle(label, value == true, enabled) { set(key, it) }
        "combo" -> Column(padded) {
            Text(label, style = MaterialTheme.typography.titleSmall)
            val options = field.optJSONArray("options")
            Row(Modifier.horizontalScroll(rememberScrollState()), Arrangement.spacedBy(8.dp)) {
                (0 until (options?.length() ?: 0)).map { options!!.getJSONObject(it) }.forEach { option ->
                    val chosen = option.get("value").toString() == value.toString()
                    val shown = icon(option.optString("icon"))
                    FilterChip(chosen, { set(key, option.get("value")) }, { Text(option.getString("label")) }, enabled = enabled,
                        leadingIcon = shown?.let { { Icon(it, null) } })
                }
            }
        }
        "color", "text" -> Entry(label, value as? String ?: "", padded, enabled, field.optBoolean("secret"), field.optBoolean("filled")) {
            if (field.optBoolean("secret")) secret(it) else set(key, it)
        }
        "group" -> Row(Modifier.fillMaxWidth().clickable(onClick = fold).padding(16.dp, 8.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(if (folded) Icons.AutoMirrored.Filled.KeyboardArrowRight else Icons.Default.KeyboardArrowDown, null)
            Text(label, Modifier.padding(start = 8.dp), style = MaterialTheme.typography.titleSmall)
        }
        "label" -> Text(label, padded, style = MaterialTheme.typography.bodyMedium)
        "divider" -> Column(padded) {
            HorizontalDivider()
            if (label.isNotEmpty()) Text(label, Modifier.padding(top = 4.dp), style = MaterialTheme.typography.labelMedium)
        }
        "button" -> OutlinedButton(press, Modifier.padding(16.dp, 4.dp), enabled = enabled) {
            icon(field.optString("icon"))?.let { Icon(it, null, Modifier.padding(end = 8.dp)) }
            Text(label)
        }
        else -> Text(stringResource(R.string.not_on_phone, label), padded, style = MaterialTheme.typography.bodySmall)
    }
    field.optString("help").takeIf { it.isNotEmpty() }?.let { Text(it, Modifier.padding(16.dp, 0.dp), style = MaterialTheme.typography.bodySmall) }
}

@Composable
fun Saved(name: String, rename: () -> Unit, export: () -> Unit, forget: () -> Unit) = Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
    Text(name, Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium)
    IconButton(rename) { Icon(Icons.Default.Edit, stringResource(R.string.rename_preset)) }
    IconButton(export) { Icon(Icons.Default.Share, stringResource(R.string.export_preset)) }
    IconButton(forget) { Icon(Icons.Default.Delete, stringResource(R.string.delete_preset)) }
}

@Composable
fun Naming(name: String, close: () -> Unit, done: (String) -> Unit) {
    var typed by remember(name) { mutableStateOf(name) }
    AlertDialog(
        onDismissRequest = close,
        confirmButton = { TextButton({ done(typed) }, enabled = typed.isNotBlank()) { Text(stringResource(R.string.save_preset)) } },
        dismissButton = { TextButton(close) { Text(stringResource(R.string.cancel)) } },
        title = { Text(stringResource(R.string.preset_name)) },
        text = { OutlinedTextField(typed, { typed = it }, singleLine = true) },
    )
}

private fun icon(name: String): ImageVector? = when (name) {
    "appointment-soon", "x-office-calendar" -> Icons.Default.DateRange
    "dialog-information" -> Icons.Default.Info
    "edit-find" -> Icons.Default.Search
    "emblem-favorite" -> Icons.Default.Favorite
    "go-home" -> Icons.Default.Home
    "media-playback-start" -> Icons.Default.PlayArrow
    "media-playlist-repeat", "view-refresh" -> Icons.Default.Refresh
    "preferences-system", "preferences-desktop-theme" -> Icons.Default.Settings
    "zoom-in" -> Icons.Default.Add
    else -> null
}
