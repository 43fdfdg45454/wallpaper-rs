package io.github.wallpaper_rs

import android.app.WallpaperColors
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.net.Uri
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import org.json.JSONArray
import org.json.JSONObject

object Shortcuts {
    var missed by mutableStateOf<Pair<String, String>?>(null)
        private set

    fun forget() {
        missed = null
    }

    fun open(context: Context, target: JSONObject): Boolean {
        val (uri, app) = target.optString("uri") to target.optString("app")
        val intent = if (uri.isNotEmpty()) Intent(Intent.ACTION_VIEW, Uri.parse(uri)).addCategory(Intent.CATEGORY_BROWSABLE)
        else context.packageManager.getLaunchIntentForPackage(app)
        val thrown = intent?.let { runCatching { context.startActivity(it.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }.exceptionOrNull() }
        val reason = when {
            intent == null || thrown is ActivityNotFoundException && uri.isEmpty() -> "app"
            thrown is ActivityNotFoundException -> "web"
            thrown != null -> "denied"
            else -> null
        }
        val shown = uri.ifEmpty { app }
        missed = reason?.let { it to shown }
        return reason == null
    }

    fun each(list: String?, block: (JSONObject) -> Unit) = list?.let(::JSONArray)?.let { all ->
        (0 until all.length()).map(all::getJSONObject).forEach { block(it.optJSONObject("open") ?: it) }
    }

    fun apps(context: Context): List<Pair<String, String>> {
        val launcher = Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER)
        val found = context.packageManager.queryIntentActivities(launcher, 0)
        return found.map { it.activityInfo.packageName to it.loadLabel(context.packageManager).toString() }
            .filter { it.first != context.packageName }.distinctBy { it.first }.sortedBy { it.second.lowercase() }
    }

    fun web(text: String) = text.trim().takeIf { it.startsWith("https://") && it.length <= 2048 && it.none(Char::isISOControl) }
}

fun Painter.colors(): WallpaperColors? = Native.colors(preview, lock)?.let { (primary, secondary, tertiary, hints) ->
    val color = { argb: Int -> if (argb == 0) null else Color.valueOf(argb) }
    WallpaperColors(Color.valueOf(primary), color(secondary), color(tertiary), hints)
}

@Composable
internal fun Shortcut(field: JSONObject, modifier: Modifier, enabled: Boolean, set: (Any?) -> Unit) {
    val context = LocalContext.current
    val chosen = field.opt("value") as? JSONObject
    var picking by remember { mutableStateOf(false) }
    var typing by remember { mutableStateOf<String?>(null) }
    val apps = remember(picking) { if (picking) Shortcuts.apps(context) else emptyList() }
    val shown = chosen?.optString("uri")?.takeIf { it.isNotEmpty() }
        ?: chosen?.optString("app")?.let { app -> apps.firstOrNull { it.first == app }?.second ?: app }
    Row(modifier, verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(field.getString("label"), style = MaterialTheme.typography.titleSmall)
            Text(shown ?: stringResource(R.string.shortcut_none), style = MaterialTheme.typography.bodySmall)
        }
        TextButton({ picking = true }, enabled = enabled) { Text(stringResource(R.string.shortcut_app)) }
        TextButton({ typing = chosen?.optString("uri").orEmpty() }, enabled = enabled) { Text(stringResource(R.string.shortcut_web)) }
        if (chosen != null) TextButton({ set(null) }, enabled = enabled) { Text(stringResource(R.string.remove)) }
    }
    if (picking) AlertDialog(
        onDismissRequest = { picking = false },
        confirmButton = { TextButton({ picking = false }) { Text(stringResource(R.string.cancel)) } },
        title = { Text(stringResource(R.string.shortcut_app)) },
        text = {
            LazyColumn {
                items(apps) { (app, label) ->
                    ListItem(headlineContent = { Text(label) }, modifier = Modifier.clickable {
                        picking = false
                        set(JSONObject().put("app", app))
                    })
                }
            }
        },
    )
    typing?.let { text ->
        val web = Shortcuts.web(text)
        AlertDialog(
            onDismissRequest = { typing = null },
            confirmButton = { TextButton({ typing = null; web?.let { set(JSONObject().put("uri", it)) } }, enabled = web != null) { Text(stringResource(R.string.shortcut_keep)) } },
            dismissButton = { TextButton({ typing = null }) { Text(stringResource(R.string.cancel)) } },
            title = { Text(stringResource(R.string.shortcut_web)) },
            text = { OutlinedTextField(text, { typing = it }, singleLine = true, isError = web == null, supportingText = { Text(stringResource(R.string.shortcut_https)) }) },
        )
    }
}

@Composable
internal fun ShortcutMissed() {
    val (reason, shown) = Shortcuts.missed ?: return
    val text = when (reason) {
        "app" -> R.string.shortcut_missed_app
        "web" -> R.string.shortcut_missed_web
        else -> R.string.shortcut_missed_denied
    }
    ListItem(
        headlineContent = { Text(stringResource(text, shown)) },
        trailingContent = { TextButton(Shortcuts::forget) { Text(stringResource(R.string.close)) } },
    )
}
