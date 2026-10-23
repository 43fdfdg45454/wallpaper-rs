package io.github.wallpaper_rs

import android.Manifest.permission.RECORD_AUDIO
import android.content.pm.PackageManager.PERMISSION_GRANTED
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts.RequestPermission
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalLocale
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import java.text.NumberFormat
import org.json.JSONArray
import org.json.JSONObject

@Composable
fun Scene(main: Main, uid: String, modifier: Modifier = Modifier) {
    val languages = listOf(LocalLocale.current.toLanguageTag().replace('-', '_'))
    val form by loaded(JSONObject(), uid, main.revision.intValue) {
        ask("scene", "uid" to uid, "languages" to languages) as JSONObject
    }
    val refused = stringResource(R.string.microphone_needed)
    val set: (String, Any) -> Unit = { key, value -> main.act { ask("property", "uid" to uid, "key" to key, "value" to value) } }
    val grant = { input: String, on: Boolean -> main.act(false) { ask("grant", "uid" to uid, "input" to input, "on" to on) } }
    val microphone = rememberLauncherForActivityResult(RequestPermission()) { if (it) grant("audio", true) else main.toast(refused) }
    val small = MaterialTheme.typography.bodySmall
    Column(modifier.verticalScroll(rememberScrollState())) {
        val (uses, granted) = form.optJSONObject("uses") to form.optJSONObject("granted")
        if (uses?.optBoolean("audio") == true) Toggle(stringResource(R.string.react_sound), granted?.optBoolean("audio") == true) { on ->
            if (on && main.checkSelfPermission(RECORD_AUDIO) != PERMISSION_GRANTED) microphone.launch(RECORD_AUDIO) else grant("audio", on)
        }
        if (uses?.optBoolean("touch") == true) {
            Toggle(stringResource(R.string.react_touch), granted?.optBoolean("touch") == true) { grant("touch", it) }
        }
        if (form.optBoolean("scripts")) Text(stringResource(R.string.scripts_off), Modifier.padding(16.dp, 4.dp), style = small)
        val presets = options(form.optJSONArray("presets"))
        if (presets.isNotEmpty()) Choices(stringResource(R.string.presets), presets, "") { preset ->
            main.act { ask("preset", "uid" to uid, "preset" to preset) }
        }
        val fields = form.optJSONArray("fields") ?: JSONArray()
        (0 until fields.length()).map(fields::getJSONObject).filter { it.optBoolean("visible") }.forEach { Field(it, set) }
        if (fields.length() > 0) TextButton({ main.act { ask("reset", "uid" to uid) } }, Modifier.padding(horizontal = 8.dp)) {
            Text(stringResource(R.string.reset))
        }
    }
}

@Composable
private fun Field(field: JSONObject, set: (String, Any) -> Unit) {
    val (key, label) = field.getString("key") to field.getString("label")
    val value = field.opt("value")
    val padded = Modifier.fillMaxWidth().padding(16.dp, 4.dp)
    when (field.getString("kind")) {
        "slider" -> Column(padded) {
            val (low, high) = field.optDouble("min", 0.0).toFloat() to field.optDouble("max", 1.0).toFloat()
            val step = field.optDouble("step", 0.0).toFloat()
            val steps = if (step > 0f) ((high - low) / step).toInt() - 1 else 0
            var at by remember(key, value) { mutableFloatStateOf((value as? Number)?.toFloat() ?: low) }
            Text("$label ${NumberFormat.getInstance().apply { maximumFractionDigits = 2 }.format(at)}${field.optString("unit")}")
            Slider(
                value = at,
                onValueChange = { at = it },
                valueRange = low..high,
                steps = steps.coerceIn(0, MAX_STEPS),
                onValueChangeFinished = { set(key, if (field.optBoolean("integer")) at.toInt() else at.toDouble()) },
            )
        }
        "bool" -> Toggle(label, value == true) { set(key, it) }
        "combo" -> Choices(label, options(field.optJSONArray("options")), value as? String ?: "") { set(key, it) }
        "color", "text" -> {
            var typed by remember(key, value) { mutableStateOf(value as? String ?: "") }
            OutlinedTextField(
                value = typed,
                onValueChange = { typed = it },
                modifier = padded,
                label = { Text(label) },
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { set(key, typed) }),
            )
        }
        "group" -> Text(label, padded, style = MaterialTheme.typography.titleSmall)
        "label" -> Text(label, padded, style = MaterialTheme.typography.bodyMedium)
        else -> Text(stringResource(R.string.not_on_phone, label), padded, style = MaterialTheme.typography.bodySmall)
    }
}

private fun options(list: JSONArray?) =
    (0 until (list?.length() ?: 0)).map { list!!.getJSONObject(it) }.map { it.getString("value") to it.getString("label") }

private const val MAX_STEPS = 100
