package io.github.wallpaper_rs

import android.content.Context
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.json.JSONArray
import org.json.JSONObject

private val PARTS = mapOf(
    "zone" to R.string.web_zone,
    "language" to R.string.web_language,
    "gpu" to R.string.web_gpu,
    "storage" to R.string.web_storage,
    "network" to R.string.web_network,
)

private val SHOWN = listOf(
    "zone" to R.string.web_show_zone,
    "language" to R.string.web_show_language,
    "gpu" to R.string.web_show_gpu,
    "storage" to R.string.web_show_storage,
)

fun strings(list: JSONArray?) = (0 until (list?.length() ?: 0)).map { list!!.getString(it) }

fun Context.notice(exposes: List<String>): String? {
    val parts = exposes.mapNotNull { PARTS[it] }.map(::getString)
    if (parts.isEmpty()) return null
    val joined = if (parts.size == 1) parts[0] else getString(R.string.web_and, parts.dropLast(1).joinToString(", "), parts.last())
    return getString(R.string.web_notice, joined)
}

@Composable
fun Exposes(exposes: List<String>) {
    LocalContext.current.notice(exposes)?.let { Note(it, MaterialTheme.colorScheme.tertiary) }
}

@Composable
fun WebPrivacy(main: Main, uid: String, web: JSONObject?) {
    web ?: return
    val hidden = web.optJSONObject("hidden") ?: JSONObject()
    val cleared = stringResource(R.string.web_cleared)
    Text(stringResource(R.string.web_privacy), Modifier.padding(16.dp, 8.dp), style = MaterialTheme.typography.titleSmall)
    Exposes(strings(web.optJSONArray("exposes")))
    SHOWN.filter { (what, _) -> what != "storage" || web.optBoolean("storage") }.forEach { (what, label) ->
        Toggle(stringResource(label), !hidden.optBoolean(what)) { on ->
            main.act { ask(if (on) "unconfirm" else "confirm", "uid" to uid, "what" to "hide_$what") }
        }
    }
    if (web.optBoolean("network")) Toggle(stringResource(R.string.web_reach), web.optBoolean("online")) { on ->
        main.act(false) { ask("grant", "uid" to uid, "input" to "network", "on" to on) }
    }
    if (web.optBoolean("storage")) TextButton({
        main.act(false) {
            ask("clear_data", "uid" to uid)
            main.toast(cleared)
        }
    }, Modifier.padding(horizontal = 8.dp)) { Text(stringResource(R.string.web_clear)) }
}
