package io.github.wallpaper_rs

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.hardware.display.DisplayManager
import android.os.Build
import android.view.Display
import android.widget.Toast
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.util.Locale

object Perf {
    @Volatile
    var on = false
        private set
    private val spent = FloatArray(SAMPLES)
    private var count = 0

    fun record(start: Boolean) {
        Native.perf(start)
        synchronized(this) { count = 0 }
        on = start
    }

    fun frame(ms: Float) = synchronized(this) {
        spent[count % SAMPLES] = ms
        count++
    }

    fun report(context: Context): String {
        val loop = synchronized(this) { spent.copyOf(minOf(count, SAMPLES)) }.sorted()
        val at = { q: Float -> loop.getOrElse(((loop.size - 1) * q).toInt()) { 0f } }
        val display = context.getSystemService(DisplayManager::class.java)?.getDisplay(Display.DEFAULT_DISPLAY)
        return buildString {
            append(Native.performance())
            val mean = loop.average().takeUnless { it.isNaN() } ?: 0.0
            append(String.format(Locale.ROOT, LOOP, mean, at(0.5f), at(0.9f), at(0.99f), at(1f)))
            append(String.format(Locale.ROOT, SCREEN, display?.refreshRate ?: 0f, Build.VERSION.SDK_INT))
            append(Native.diagnostics())
            append('\n')
        }
    }

    private const val SAMPLES = 4096
    private const val LOOP = "frame loop   mean %.2f, p50 %.2f, p90 %.2f, p99 %.2f, max %.2f ms (Kotlin, JNI and draw)\n"
    private const val SCREEN = "display      %.1f Hz, Android %d\n"
}

@Composable
fun PerfControls() {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var recording by remember { mutableStateOf(Perf.on) }
    val copied = stringResource(R.string.perf_copied)
    Toggle(stringResource(R.string.perf_record), recording) {
        Perf.record(it)
        recording = it
    }
    Note(stringResource(R.string.perf_hint))
    TextButton({
        scope.launch {
            val text = withContext(Dispatchers.IO) { Perf.report(context) }
            context.getSystemService(ClipboardManager::class.java)?.setPrimaryClip(ClipData.newPlainText("wallpaper-rs", text))
            Toast.makeText(context, copied, Toast.LENGTH_SHORT).show()
        }
    }, Modifier.padding(8.dp, 0.dp), enabled = recording) { Text(stringResource(R.string.perf_copy)) }
}
