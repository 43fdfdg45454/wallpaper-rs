package io.github.wallpaper_rs

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.List
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.FavoriteBorder
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalLocale
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.json.JSONObject

@Composable
fun Detail(main: Main, uid: String, close: () -> Unit) {
    val languages = listOf(LocalLocale.current.toLanguageTag().replace('-', '_'))
    val item by loaded<Item?>(null, uid, main.revision.intValue) {
        (ask("item", "uid" to uid, "languages" to languages) as? JSONObject)?.let(::Item)
    }
    val asks = item?.let { if (it.flashing) "photosensitive" else if (it.mature) "mature" else null }
    var ready by remember(uid) { mutableStateOf(false) }
    var adding by remember { mutableStateOf(false) }
    var removing by remember { mutableStateOf(false) }
    LaunchedEffect(uid, item == null, asks) {
        if (item == null || asks != null) return@LaunchedEffect
        runCatching { ask("pick", "choice" to choice("item", uid)) }
        ready = true
    }
    DisposableEffect(uid) { onDispose { main.unpick(choice("item", uid)) } }
    val picture = item?.run { !video && !scene } == true
    val state by loaded(JSONObject(), uid, picture, main.revision.intValue, Depth.working.value, powered.intValue) {
        if (picture) depth("item", "uid" to uid) as JSONObject else JSONObject()
    }
    val window = LocalWindowInfo.current.containerSize
    val screen = remember(window) { main.wallpaperSize() }
    val preview = @Composable { modifier: Modifier ->
        Box(modifier.padding(horizontal = 16.dp), Alignment.Center) {
            if (asks != null) Caution(asks) { main.act { ask("confirm", "uid" to uid, "what" to asks) } }
            else if (ready && (item?.refused != true || item?.animated == null)) {
                val shape = Modifier.aspectRatio(screen.width.coerceAtLeast(1) / screen.height.coerceAtLeast(1).toFloat(), true)
                Live(main, main.revision.intValue, Depth.working.value, modifier = if (picture) shape.framing(main, uid, screen) else shape)
            } else item?.animated?.let { Animated(it, Modifier.fillMaxSize()) }
        }
    }
    val about = @Composable { modifier: Modifier ->
        Column(modifier.verticalScroll(rememberScrollState())) {
            item?.let { Text(facts(it), Modifier.padding(16.dp, 8.dp), style = MaterialTheme.typography.bodyMedium) }
            item?.about?.takeIf { it.isNotEmpty() }?.let { Text(it, Modifier.padding(16.dp, 0.dp), style = MaterialTheme.typography.bodySmall, maxLines = 4) }
            item?.lacks?.takeIf { it.isNotEmpty() }?.let { lacks ->
                val notes = lacks.mapNotNull { lack ->
                    when (lack) {
                        "sound" -> R.string.lacks_sound
                        "video_properties" -> R.string.lacks_video_properties
                        "playback2" -> R.string.lacks_playback2
                        "gpu" -> R.string.lacks_gpu
                        "display2" -> R.string.lacks_display2
                        "spanned_scenes" -> R.string.lacks_spanned_scenes
                        "sound2" -> R.string.lacks_sound2
                        "script_geometry" -> R.string.lacks_script_geometry
                        "script_storage" -> R.string.lacks_script_storage
                        "script_sounds" -> R.string.lacks_script_sounds
                        "script_animations" -> R.string.lacks_script_animations
                        "script_camera" -> R.string.lacks_script_camera
                        "script_shortcuts" -> R.string.lacks_script_shortcuts
                        "shortcut" -> R.string.lacks_shortcut
                        "script_hierarchy" -> R.string.lacks_script_hierarchy
                        "rgb" -> R.string.lacks_rgb
                        else -> null
                    }
                }.map { stringResource(it) }
                if (notes.isNotEmpty()) Note(notes.joinToString(" "))
            }
            if (picture && state.optBoolean("changed")) Note(stringResource(R.string.picture_changed), MaterialTheme.colorScheme.error)
            if (picture) {
                Framing(main, uid, state)
                Relief(main, uid, state)
            }
            if (item?.scene == false) Kept(main, uid)
            item?.takeIf { it.scene && !it.web && !it.flashes }?.let { shown ->
                Toggle(stringResource(R.string.keep_moving), shown.moving) {
                    main.act { ask(if (it) "confirm" else "unconfirm", "uid" to uid, "what" to "motion") }
                }
            }
        }
    }
    Column(Modifier.fillMaxSize()) {
        Header(item?.title.orEmpty(), close)
        if (window.width > window.height) Row(Modifier.weight(1f)) {
            preview(Modifier.weight(1f).fillMaxHeight())
            Column(Modifier.weight(1f)) {
                about(Modifier.weight(1f, false))
                if (item?.scene == true) Scene(main, uid, Modifier.weight(1f))
            }
        } else {
            preview(Modifier.weight(1f).fillMaxWidth())
            about(Modifier.heightIn(max = 360.dp))
            if (item?.scene == true) Scene(main, uid, Modifier.weight(1f))
        }
        item?.exposes?.let { Exposes(it) }
        Row(Modifier.padding(16.dp), Arrangement.spacedBy(8.dp), Alignment.CenterVertically) {
            Button(main::setWallpaper, Modifier.weight(1f), enabled = ready && asks == null) { Text(stringResource(R.string.apply)) }
            val favorite = item?.favorite == true
            IconButton({ main.act { ask("favorite", "uids" to listOf(uid), "favorite" to !favorite) } }) {
                val icon = if (favorite) Icons.Default.Favorite else Icons.Default.FavoriteBorder
                Icon(icon, stringResource(R.string.favorite))
            }
            IconButton({ adding = true }) {
                Icon(Icons.AutoMirrored.Filled.List, stringResource(R.string.add_to_playlist))
            }
            IconButton({ removing = true }) { Icon(Icons.Default.Delete, stringResource(R.string.remove)) }
        }
    }
    if (adding) AddTo({ adding = false }) { playlist -> main.act { ask("add", "playlist" to playlist, "items" to listOf(uid)) } }
    if (removing) Removing(1, { removing = false }) {
        main.act {
            ask("remove", "uids" to listOf(uid))
            close()
        }
    }
}

@Composable
private fun Caution(asks: String, confirm: () -> Unit) = Column(Modifier.fillMaxSize(), Arrangement.Center, Alignment.CenterHorizontally) {
    val flashing = asks == "photosensitive"
    Icon(if (flashing) Icons.Default.Warning else Icons.Default.Lock, null, Modifier.padding(8.dp), MaterialTheme.colorScheme.error)
    Text(stringResource(if (flashing) R.string.flashing_warning else R.string.mature_warning), Modifier.padding(16.dp), style = MaterialTheme.typography.bodyLarge)
    OutlinedButton(confirm) { Text(stringResource(R.string.show_anyway)) }
}

@Composable
private fun facts(item: Item): String = listOfNotNull(
    stringResource(R.string.size, item.width, item.height).takeIf { item.width > 0 },
    stringResource(R.string.duration, item.millis / 60_000, item.millis / 1000 % 60).takeIf { item.millis > 0 },
).joinToString(" · ")
