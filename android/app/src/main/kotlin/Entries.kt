package io.github.wallpaper_rs

import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import org.json.JSONObject

@Composable
fun Entries(main: Main, uid: String, open: (String) -> Unit, close: () -> Unit) = Column {
    val list by loaded<Playlist?>(null, main.revision.intValue, uid) { playlists().find { it.uid == uid } }
    val found by loaded<List<Item>?>(null, main.revision.intValue, uid) { fetch("entries", "playlist" to uid) }
    val items = found.orEmpty()
    val rules = list?.rules ?: JSONObject()
    val manual = list?.info?.isNull("query") != false
    val keep: (JSONObject) -> Unit = { info -> main.act { ask("save", "playlist" to info) } }
    val save: (String, Any) -> Unit = { key, value -> list?.let { keep(it.ruled(key, value)) } }
    val chosen = rememberSelection()
    var naming by remember { mutableStateOf(false) }
    var dragged by remember { mutableStateOf<String?>(null) }
    var shift by remember { mutableFloatStateOf(0f) }
    var row by remember { mutableIntStateOf(1) }
    chosen.follow(found)
    DisposableEffect(uid) { onDispose { main.unpick(choice("playlist", uid)) } }
    if (!chosen.isEmpty()) Selecting(main, chosen, items.map(Item::uid), uid)
    else Row(verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) { Header(list?.name.orEmpty(), close) }
        IconButton({ naming = true }, enabled = list != null) { Icon(Icons.Default.Edit, stringResource(R.string.rename_playlist)) }
    }
    LazyColumn(contentPadding = PaddingValues(bottom = 16.dp)) {
        if (chosen.isEmpty()) item {
            val intervals = INTERVALS.map { (value, label) -> value to stringResource(label) }
            Choices(stringResource(R.string.change), intervals, rules.optString("interval", "30m")) { save("interval", it) }
            Schedule(rules, save)
            Toggle(stringResource(R.string.shuffle), rules.optString("order") == "shuffle") {
                save("order", if (it) "shuffle" else "sequential")
            }
            Toggle(stringResource(R.string.skip_videos), rules.optBoolean("skip_videos_on_battery")) {
                save("skip_videos_on_battery", it)
            }
            list?.let { Smart(it, keep) }
            Row(Modifier.padding(16.dp), Arrangement.spacedBy(8.dp)) {
                val use = {
                    main.act(false) {
                        ask("pick", "choice" to choice("playlist", uid))
                        main.setWallpaper()
                    }
                }
                Button({ use() }, enabled = items.isNotEmpty()) { Text(stringResource(R.string.apply)) }
                OutlinedButton({
                    main.act {
                        ask("delete", "playlist" to uid)
                        close()
                    }
                }) { Text(stringResource(R.string.delete)) }
            }
            if (found?.isEmpty() == true) Text(stringResource(R.string.empty_playlist), Modifier.padding(16.dp))
        }
        itemsIndexed(items, key = { _, item -> item.uid }) { index, item ->
            val moving = dragged == item.uid
            val drop = {
                val to = landing(index, shift, List(items.size) { row })
                dragged = null
                shift = 0f
                if (to != index) main.act { ask("move", "playlist" to uid, "from" to index, "to" to to) }
            }
            ListItem(
                headlineContent = { Text(item.title) },
                leadingContent = { Thumb(item, Modifier.size(56.dp), item.uid in chosen) },
                trailingContent = {
                    if (chosen.isNotEmpty()) return@ListItem Icon(
                        Icons.Default.CheckCircle,
                        null,
                        tint = if (item.uid in chosen) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outlineVariant,
                    )
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        if (manual) IconButton({ take(main, uid, item.uid) }) {
                            Icon(Icons.Default.Close, stringResource(R.string.take_out))
                        }
                        if (manual) Icon(Icons.Default.Menu, stringResource(R.string.drag_to_move), Modifier.padding(12.dp).pointerInput(item.uid, index) {
                            detectVerticalDragGestures(
                                onDragStart = { dragged = item.uid },
                                onDragEnd = drop,
                                onDragCancel = {
                                    dragged = null
                                    shift = 0f
                                },
                            ) { change, amount ->
                                change.consume()
                                shift += amount
                            }
                        })
                    }
                },
                modifier = Modifier.onSizeChanged { row = it.height.coerceAtLeast(1) }.zIndex(if (moving) 1f else 0f)
                    .graphicsLayer { translationY = if (moving) shift else 0f }
                    .combinedClickable(onLongClick = { chosen.toggle(item.uid) }) {
                        if (chosen.isEmpty()) open(item.uid) else chosen.toggle(item.uid)
                    },
            )
        }
    }
    if (naming) list?.let { Named(stringResource(R.string.rename_playlist), it.name, { naming = false }) { name ->
        naming = false
        keep(it.edited("name", name))
    } }
}

private fun take(main: Main, playlist: String, item: String) = main.act { ask("take", "playlist" to playlist, "items" to listOf(item)) }
