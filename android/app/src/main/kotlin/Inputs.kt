package io.github.wallpaper_rs

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import java.text.NumberFormat
import kotlin.math.ln
import kotlin.math.pow
import org.json.JSONObject

@Composable
internal fun Slide(field: JSONObject, modifier: Modifier, enabled: Boolean, set: (Any) -> Unit) = Column(modifier) {
    val (low, high) = field.optDouble("min", 0.0) to field.optDouble("max", 1.0)
    val log = field.optBoolean("log") && low > 0.0
    val step = if (log) 0.0 else field.optDouble("step", 0.0)
    val places = field.getInt("decimals")
    val toValue = { p: Float -> if (log) low * (high / low).pow(p.toDouble()) else p.toDouble() }
    val toPosition = { v: Double -> if (log) (ln(v / low) / ln(high / low)).toFloat() else v.toFloat() }
    val current = (field.opt("value") as? Number)?.toDouble() ?: low
    var at by remember(field.toString()) { mutableFloatStateOf(toPosition(current)) }
    val format = NumberFormat.getInstance().apply { maximumFractionDigits = places }
    val send = { v: Double -> set(if (places == 0) Math.round(v) else v) }
    Text("${field.getString("label")} ${format.format(toValue(at))}${field.optString("unit").let { if (it.isEmpty()) "" else " $it" }}")
    val range = if (log) 0f..1f else low.toFloat()..high.toFloat()
    val steps = if (step > 0.0) ((high - low) / step).toInt() - 1 else 0
    Slider(at, { at = it }, enabled = enabled, valueRange = range, steps = steps.coerceIn(0, MAX_STEPS), onValueChangeFinished = { send(toValue(at)) })
    if (field.optBoolean("editable")) {
        var typed by remember(field.toString()) { mutableStateOf(format.format(current)) }
        OutlinedTextField(typed, { typed = it }, enabled = enabled, singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { runCatching { format.parse(typed)?.toDouble() }.getOrNull()?.let(send) }))
    }
}

@Composable
internal fun Entry(label: String, value: String, modifier: Modifier, enabled: Boolean, hidden: Boolean, filled: Boolean, keep: (String) -> Unit) {
    var typed by remember(label, value) { mutableStateOf(if (hidden) "" else value) }
    OutlinedTextField(
        value = typed,
        onValueChange = { typed = it },
        modifier = modifier,
        enabled = enabled,
        label = { Text(label) },
        placeholder = { if (hidden && filled) Text(stringResource(R.string.stored_secret)) },
        singleLine = true,
        visualTransformation = if (hidden) PasswordVisualTransformation() else VisualTransformation.None,
        keyboardOptions = KeyboardOptions(keyboardType = if (hidden) KeyboardType.Password else KeyboardType.Text, imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { keep(typed) }),
    )
}

private const val MAX_STEPS = 100
