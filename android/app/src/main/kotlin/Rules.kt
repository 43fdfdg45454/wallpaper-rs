package io.github.wallpaper_rs

import android.app.TimePickerDialog
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.InputChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLocale
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import java.time.DayOfWeek
import java.time.format.TextStyle
import kotlinx.coroutines.delay
import org.json.JSONArray
import org.json.JSONObject

val WEEK = listOf("Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun")

fun toggled(days: List<String>, day: String): List<String> = WEEK.filter { (it in days) != (it == day) }

fun timed(times: List<String>, hour: Int, minute: Int): List<String> =
    (times + "%02d:%02d".format(hour, minute)).distinct().sorted()

fun landing(from: Int, shift: Float, sizes: List<Int>): Int {
    var (at, left) = from to shift
    while (left > 0 && at + 1 < sizes.size && left > sizes[at + 1] / 2f) left -= sizes[++at]
    while (left < 0 && at > 0 && -left > sizes[at - 1] / 2f) left += sizes[--at]
    return at
}

fun JSONArray?.strings(): List<String> = this?.let { list -> List(list.length(), list::getString) }.orEmpty()

fun Playlist.edited(key: String, value: Any?): JSONObject = JSONObject(info.toString()).put(key, value ?: JSONObject.NULL)

@Composable
fun Schedule(rules: JSONObject, save: (String, Any) -> Unit) = Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
    val days = rules.optJSONArray("days").strings()
    val times = rules.optJSONArray("at").strings()
    val locale = LocalLocale.current.platformLocale
    val context = LocalContext.current
    Text(stringResource(R.string.days), style = MaterialTheme.typography.titleSmall)
    Row(Modifier.horizontalScroll(rememberScrollState()), Arrangement.spacedBy(8.dp)) {
        WEEK.forEachIndexed { index, day ->
            val label = DayOfWeek.of(index + 1).getDisplayName(TextStyle.SHORT, locale)
            FilterChip(day in days, { save("days", JSONArray(toggled(days, day))) }, { Text(label) })
        }
    }
    if (days.isEmpty()) Text(stringResource(R.string.every_day), style = MaterialTheme.typography.bodySmall)
    Text(stringResource(R.string.hours), Modifier.padding(top = 8.dp), style = MaterialTheme.typography.titleSmall)
    Row(Modifier.horizontalScroll(rememberScrollState()), Arrangement.spacedBy(8.dp)) {
        times.forEach { time ->
            InputChip(true, { save("at", JSONArray(times - time)) }, { Text(time) },
                trailingIcon = { Icon(Icons.Default.Close, stringResource(R.string.remove_hour)) })
        }
        AssistChip({
            TimePickerDialog(context, { _, hour, minute -> save("at", JSONArray(timed(times, hour, minute))) }, 8, 0, true).show()
        }, { Text(stringResource(R.string.add_hour)) }, leadingIcon = { Icon(Icons.Default.Add, null) })
    }
    if (times.isNotEmpty()) Text(stringResource(R.string.hours_hint), style = MaterialTheme.typography.bodySmall)
}

val FADES = listOf(0, 500, 1000, 2000)

fun faded(rules: JSONObject): String = rules.opt("transition_ms").takeIf { it is Number }?.toString().orEmpty()

@Composable
fun Fade(rules: JSONObject, save: (String, Any) -> Unit) {
    val lasting = FADES.map { ms -> ms.toString() to if (ms == 0) stringResource(R.string.fade_none) else stringResource(R.string.fade_ms, ms) }
    val options = listOf("" to stringResource(R.string.fade_usual)) + lasting
    Choices(stringResource(R.string.list_fade), options, faded(rules)) { save("transition_ms", it.toIntOrNull() ?: JSONObject.NULL) }
}

@Composable
fun Smart(list: Playlist, save: (JSONObject) -> Unit) {
    val saved = list.info.optString("query").takeUnless { list.info.isNull("query") }
    var text by remember(list.uid, saved) { mutableStateOf(saved.orEmpty()) }
    var matches by remember { mutableIntStateOf(-1) }
    LaunchedEffect(text) {
        delay(DEBOUNCE_MS)
        matches = if (text.isBlank()) -1 else runCatching { ask("count", "query" to text) as Int }.getOrDefault(-1)
    }
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
        OutlinedTextField(
            text, { text = it }, Modifier.fillMaxWidth(), label = { Text(stringResource(R.string.smart_query)) },
            placeholder = { Text(stringResource(R.string.manual_playlist)) }, singleLine = true,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions { save(list.edited("query", text.ifBlank { null })) },
        )
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (matches >= 0) Text(stringResource(R.string.matches, matches), Modifier.padding(vertical = 12.dp), style = MaterialTheme.typography.bodySmall)
            if (text != saved.orEmpty()) TextButton({ save(list.edited("query", text.ifBlank { null })) }) { Text(stringResource(R.string.save_search)) }
            if (saved != null) TextButton({ save(list.edited("query", null)) }) { Text(stringResource(R.string.make_manual)) }
        }
    }
}

@Composable
fun Named(title: String, name: String, close: () -> Unit, done: (String) -> Unit) {
    var typed by remember(name) { mutableStateOf(name) }
    AlertDialog(
        onDismissRequest = close,
        confirmButton = { TextButton({ done(typed.trim()) }, enabled = typed.isNotBlank()) { Text(stringResource(R.string.save_name)) } },
        dismissButton = { TextButton(close) { Text(stringResource(R.string.cancel)) } },
        title = { Text(title) },
        text = { OutlinedTextField(typed, { typed = it }, label = { Text(stringResource(R.string.name)) }, singleLine = true) },
    )
}

private const val DEBOUNCE_MS = 250L
