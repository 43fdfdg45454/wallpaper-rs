package io.github.wallpaper_rs

import androidx.compose.foundation.combinedClickable
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
import androidx.compose.material.icons.filled.KeyboardArrowUp
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.json.JSONObject

@Composable
fun Entries(main: Main, uid: String, open: (String) -> Unit, close: () -> Unit) = Column {
    val list by loaded<Playlist?>(null, main.revision.intValue, uid) { playlists().find { it.uid == uid } }
    val found by loaded<List<Item>?>(null, main.revision.intValue, uid) { fetch("entries", "playlist" to uid) }
    val items = found.orEmpty()
    val rules = list?.rules ?: JSONObject()
    val save = { key: String, value: Any -> list?.let { main.act { ask("save", "playlist" to it.ruled(key, value)) } } }
    val chosen = rememberSelection()
    chosen.follow(found)
    DisposableEffect(uid) { onDispose { main.unpick(choice("playlist", uid)) } }
    if (chosen.isEmpty()) Header(list?.name.orEmpty(), close) else Selecting(main, chosen, items.map(Item::uid), uid)
    LazyColumn(contentPadding = PaddingValues(bottom = 16.dp)) {
        if (chosen.isEmpty()) item {
            val intervals = INTERVALS.map { (value, label) -> value to stringResource(label) }
            val interval = rules.optString("interval", "30m")
            Choices(stringResource(R.string.change), intervals, interval) { save("interval", it) }
            Toggle(stringResource(R.string.shuffle), rules.optString("order") == "shuffle") {
                save("order", if (it) "shuffle" else "sequential")
            }
            Toggle(stringResource(R.string.skip_videos), rules.optBoolean("skip_videos_on_battery")) {
                save("skip_videos_on_battery", it)
            }
            Row(Modifier.padding(16.dp), Arrangement.spacedBy(8.dp)) {
                val use = {
                    main.act(false) {
                        ask("pick", "choice" to choice("playlist", uid))
                        main.apply()
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
            ListItem(
                headlineContent = { Text(item.title) },
                leadingContent = { Thumb(item, Modifier.size(56.dp), item.uid in chosen) },
                trailingContent = {
                    if (chosen.isNotEmpty()) return@ListItem Icon(
                        Icons.Default.CheckCircle,
                        null,
                        tint = if (item.uid in chosen) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outlineVariant,
                    )
                    Row {
                        val up = { main.act { ask("move", "playlist" to uid, "from" to index, "to" to index - 1) } }
                        if (index > 0) IconButton({ up() }) {
                            Icon(Icons.Default.KeyboardArrowUp, stringResource(R.string.earlier))
                        }
                        IconButton({ main.act { ask("take", "playlist" to uid, "items" to listOf(item.uid)) } }) {
                            Icon(Icons.Default.Close, stringResource(R.string.take_out))
                        }
                    }
                },
                modifier = Modifier.combinedClickable(onLongClick = { chosen.toggle(item.uid) }) {
                    if (chosen.isEmpty()) open(item.uid) else chosen.toggle(item.uid)
                },
            )
        }
    }
}
