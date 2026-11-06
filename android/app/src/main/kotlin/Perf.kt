package io.github.wallpaper_rs

import android.content.ClipData
import android.content.ClipboardManager
import android.os.Build
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

object Perf {
    @Volatile
    var on = false
        private set

    fun record(start: Boolean) {
        Native.perf(start)
        on = start
    }

    fun report(): String = buildString {
        append(Native.performance())
        append("android      ").append(Build.VERSION.SDK_INT).append('\n')
        append(Native.diagnostics())
        append('\n')
    }
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
            val text = withContext(Dispatchers.IO) { Perf.report() }
            context.getSystemService(ClipboardManager::class.java)?.setPrimaryClip(ClipData.newPlainText("wallpaper-rs", text))
            Toast.makeText(context, copied, Toast.LENGTH_SHORT).show()
        }
    }, Modifier.padding(8.dp, 0.dp), enabled = recording) { Text(stringResource(R.string.perf_copy)) }
}
