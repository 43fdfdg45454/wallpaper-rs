package io.github.wallpaper_rs

import android.app.WallpaperManager
import android.content.ComponentName
import android.content.Intent
import android.os.Build
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ListItem
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource

const val BOTH = "both"
const val HOME = "home"
const val LOCK = "lock"

private val TARGETS = listOf(HOME to R.string.home_screen, LOCK to R.string.lock_screen, BOTH to R.string.both_screens)

fun separate() = Build.VERSION.SDK_INT >= 34

fun Main.setWallpaper() {
    if (separate()) placing.value = true else place(BOTH)
}

fun Main.place(target: String) {
    placing.value = false
    if (active(target) && !Wallpaper.joined(target)) commit(target) else useAsWallpaper(target)
}

fun Main.placed(applied: Boolean) {
    val target = Wallpaper.commitWhenLive ?: return
    if (active(target)) commit(target) else if (!applied) Wallpaper.commitWhenLive = null
}

private fun Main.active(target: String): Boolean {
    val manager = WallpaperManager.getInstance(this)
    val home = manager.wallpaperInfo?.packageName == packageName
    if (!separate()) return Wallpaper.live > 0 || home
    val lock = (manager.getWallpaperInfo(WallpaperManager.FLAG_LOCK) ?: manager.wallpaperInfo)?.packageName == packageName
    return when (target) {
        HOME -> home
        LOCK -> lock
        else -> home && lock
    }
}

private fun Main.useAsWallpaper(target: String) {
    val wallpaper = ComponentName(this, Wallpaper::class.java)
    val intent = Intent(WallpaperManager.ACTION_CHANGE_LIVE_WALLPAPER)
        .putExtra(WallpaperManager.EXTRA_LIVE_WALLPAPER_COMPONENT, wallpaper)
    Wallpaper.commitWhenLive = target
    runCatching { preview.launch(intent) }.onFailure {
        Wallpaper.commitWhenLive = null
        toast(getString(R.string.unsupported))
    }
}

private fun Main.commit(target: String) = Render.post {
    Wallpaper.commitWhenLive = null
    Wallpaper.reload(Native.commit(target))
    runOnUiThread {
        revision.intValue++
        toast(getString(R.string.applied))
    }
}

@Composable
fun Placing(main: Main) = AlertDialog(
    onDismissRequest = { main.placing.value = false },
    confirmButton = {},
    dismissButton = { TextButton({ main.placing.value = false }) { Text(stringResource(R.string.cancel)) } },
    title = { Text(stringResource(R.string.place_where)) },
    text = {
        Column {
            TARGETS.forEach { (target, label) ->
                ListItem(headlineContent = { Text(stringResource(label)) }, modifier = Modifier.clickable { main.place(target) })
            }
        }
    },
)
