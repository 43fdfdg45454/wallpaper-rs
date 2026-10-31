package io.github.wallpaper_rs

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject

val LEVELS = listOf(
    Triple("near", R.string.depth_near, 0f..1f),
    Triple("far", R.string.depth_far, 0f..1f),
    Triple("gamma", R.string.depth_gamma, 0.2f..5f),
    Triple("smooth", R.string.depth_smooth, 0f..16f),
    Triple("dilate", R.string.depth_dilate, 0f..16f),
)

fun level(key: String, value: Float): Any = if (key == "smooth" || key == "dilate") Math.round(value) else value

@Composable
fun Levels(main: Main, uid: String, state: JSONObject) {
    LEVELS.forEach { (key, label, range) ->
        Level(stringResource(label), state.optDouble(key, 0.0).toFloat(), range) {
            main.act { withContext(Dispatchers.IO) { depth("levels", "uid" to uid, key to level(key, it)) } }
        }
    }
    TextButton({ main.act { withContext(Dispatchers.IO) { depth("unmap", "uid" to uid) } } }, Modifier.padding(horizontal = 8.dp)) {
        Text(stringResource(R.string.depth_remove))
    }
}
