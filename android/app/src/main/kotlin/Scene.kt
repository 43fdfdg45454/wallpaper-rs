package io.github.wallpaper_rs

import android.Manifest.permission.RECORD_AUDIO
import android.content.pm.PackageManager.PERMISSION_GRANTED
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts.CreateDocument
import androidx.activity.result.contract.ActivityResultContracts.OpenDocument
import androidx.activity.result.contract.ActivityResultContracts.RequestPermission
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
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
import androidx.compose.ui.platform.LocalLocale
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

@Composable
fun Scene(main: Main, uid: String, modifier: Modifier = Modifier) {
    val languages = listOf(LocalLocale.current.toLanguageTag().replace('-', '_'))
    val form by loaded(JSONObject(), uid, main.revision.intValue) {
        ask("scene", "uid" to uid, "languages" to languages) as JSONObject
    }
    val context = LocalContext.current
    val (refused, skipped) = stringResource(R.string.microphone_needed) to stringResource(R.string.skipped_settings)
    var flipped by remember(uid) { mutableStateOf(setOf<String>()) }
    var naming by remember(uid) { mutableStateOf<Pair<String, String>?>(null) }
    var exporting by remember(uid) { mutableStateOf("") }
    var imported by remember(uid) { mutableStateOf<String?>(null) }
    val set: (String, Any?) -> Unit = { key, value -> main.act { ask("property", "uid" to uid, "key" to key, "value" to value) } }
    val grant = { input: String, on: Boolean -> main.act(false) { ask("grant", "uid" to uid, "input" to input, "on" to on) } }
    val microphone = rememberLauncherForActivityResult(RequestPermission()) { if (it) grant("audio", true) else main.toast(refused) }
    val writing = rememberLauncherForActivityResult(CreateDocument(JSON)) { target ->
        target?.let { main.act(false) {
            val text = ask("export", "uid" to uid, "preset" to exporting) as String
            withContext(Dispatchers.IO) { context.contentResolver.openOutputStream(it)?.use { out -> out.write(text.toByteArray()) } }
        } }
    }
    val read: suspend (Uri) -> String = { source ->
        withContext(Dispatchers.IO) { context.contentResolver.openInputStream(source)?.use { it.readBytes().decodeToString() }.orEmpty() }
    }
    val bring: suspend (String, String) -> Unit = { text, name ->
        val warnings = ask("import", "uid" to uid, "text" to text, "name" to name) as JSONArray
        if (warnings.length() > 0) main.toast(skipped.format((0 until warnings.length()).joinToString { warnings.getString(it) }))
    }
    val reading = rememberLauncherForActivityResult(OpenDocument()) { source -> source?.let { main.act { bring(read(it), "") } } }
    val keeping = rememberLauncherForActivityResult(OpenDocument()) { source -> source?.let { main.act(false) { imported = read(it) } } }
    val small = MaterialTheme.typography.bodySmall
    Column(modifier.verticalScroll(rememberScrollState())) {
        val (uses, granted) = form.optJSONObject("uses") to form.optJSONObject("granted")
        if (uses?.optBoolean("audio") == true) Toggle(stringResource(R.string.react_sound), granted?.optBoolean("audio") == true) { on ->
            if (on && main.checkSelfPermission(RECORD_AUDIO) != PERMISSION_GRANTED) microphone.launch(RECORD_AUDIO) else grant("audio", on)
        }
        if (uses?.optBoolean("touch") == true) {
            Toggle(stringResource(R.string.react_touch), granted?.optBoolean("touch") == true) { grant("touch", it) }
        }
        if (uses?.optBoolean("taps") == true) {
            Toggle(stringResource(R.string.react_taps), granted?.optBoolean("taps") == true) { grant("taps", it) }
        }
        if (form.optBoolean("sounds")) Text(stringResource(R.string.sounds_off), Modifier.padding(16.dp, 4.dp), style = small)
        val saved = options(form.optJSONArray("saved"))
        val presets = options(form.optJSONArray("presets")) + saved
        if (presets.isNotEmpty()) Choices(stringResource(R.string.presets), presets, "") { preset ->
            main.act { ask("preset", "uid" to uid, "preset" to preset) }
        }
        saved.forEach { (preset, name) ->
            Saved(name, { naming = preset to name }, { exporting = preset; writing.launch("$name.json") }) { main.act { ask("forget", "preset" to preset) } }
        }
        Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 8.dp)) {
            TextButton({ naming = "" to "" }) { Text(stringResource(R.string.save_preset)) }
            TextButton({ exporting = ""; writing.launch(FILE) }) { Text(stringResource(R.string.export_settings)) }
            TextButton({ reading.launch(arrayOf(JSON, "text/plain")) }) { Text(stringResource(R.string.import_settings)) }
            TextButton({ keeping.launch(arrayOf(JSON, "text/plain")) }) { Text(stringResource(R.string.import_preset)) }
        }
        val fields = form.optJSONArray("fields") ?: JSONArray()
        val all = (0 until fields.length()).map(fields::getJSONObject)
        val folded = all.filter { it.getString("kind") == "group" && it.optBoolean("collapsed") != (it.getString("key") in flipped) }.map { it.getString("key") }
        all.filter { it.optBoolean("visible") && it.optString("group") !in folded }.forEach { field ->
            val key = field.getString("key")
            Field(field, key in folded, { flipped = if (key in flipped) flipped - key else flipped + key }, set, { main.act { ask("press", "uid" to uid, "key" to key) } }) { text ->
                main.act { ask("secret", "uid" to uid, "key" to key, "text" to text) }
            }
        }
        if (all.isNotEmpty()) TextButton({ main.act { ask("reset", "uid" to uid) } }, Modifier.padding(horizontal = 8.dp)) {
            Text(stringResource(R.string.reset))
        }
    }
    imported?.let { text ->
        Naming("", { imported = null }) { chosen ->
            imported = null
            main.act { bring(text, chosen) }
        }
    }
    naming?.let { (preset, name) ->
        Naming(name, { naming = null }) { chosen ->
            naming = null
            main.act { if (preset.isEmpty()) ask("save", "uid" to uid, "name" to chosen) else ask("rename", "preset" to preset, "name" to chosen) }
        }
    }
}

fun options(list: JSONArray?) =
    (0 until (list?.length() ?: 0)).map { list!!.getJSONObject(it) }.map { it.get("value").toString() to it.getString("label") }

private const val JSON = "application/json"
private const val FILE = "settings.json"
