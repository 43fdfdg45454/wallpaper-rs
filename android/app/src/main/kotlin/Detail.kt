package io.github.wallpaper_rs

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.List
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.FavoriteBorder
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.json.JSONObject

@Composable
fun Detail(main: Main, uid: String, close: () -> Unit) {
    val item by loaded<Item?>(null, uid, main.revision.intValue) {
        (ask("item", "uid" to uid) as? JSONObject)?.let(::Item)
    }
    var ready by remember(uid) { mutableStateOf(false) }
    var adding by remember { mutableStateOf(false) }
    var removing by remember { mutableStateOf(false) }
    LaunchedEffect(uid) {
        runCatching { ask("pick", "choice" to choice("item", uid)) }
        ready = true
    }
    DisposableEffect(uid) { onDispose { main.unpick(choice("item", uid)) } }
    Column(Modifier.fillMaxSize()) {
        Header(item?.title.orEmpty(), close)
        Box(Modifier.weight(1f).fillMaxWidth().padding(horizontal = 16.dp)) {
            if (ready) Live(main, main.revision.intValue, Depth.working.value)
        }
        item?.let { Text(facts(it), Modifier.padding(16.dp, 8.dp), style = MaterialTheme.typography.bodyMedium) }
        item?.let { shown -> if (shown.scene) Scene(main, uid, Modifier.weight(1f)) else if (!shown.video) Relief(main, uid) }
        Row(Modifier.padding(16.dp), Arrangement.spacedBy(8.dp), Alignment.CenterVertically) {
            Button(main::apply, Modifier.weight(1f)) { Text(stringResource(R.string.apply)) }
            val favorite = item?.favorite == true
            IconButton({ main.act { ask("favorite", "uid" to uid, "favorite" to !favorite) } }) {
                val icon = if (favorite) Icons.Default.Favorite else Icons.Default.FavoriteBorder
                Icon(icon, stringResource(R.string.favorite))
            }
            IconButton({ adding = true }) {
                Icon(Icons.AutoMirrored.Filled.List, stringResource(R.string.add_to_playlist))
            }
            IconButton({ removing = true }) { Icon(Icons.Default.Delete, stringResource(R.string.remove)) }
        }
    }
    if (adding) AddTo({ adding = false }) { playlist -> main.act { ask("add", "playlist" to playlist, "item" to uid) } }
    if (removing) AlertDialog(
        onDismissRequest = { removing = false },
        confirmButton = {
            TextButton({
                main.act {
                    ask("remove", "uid" to uid)
                    close()
                }
            }) { Text(stringResource(R.string.remove)) }
        },
        dismissButton = { TextButton({ removing = false }) { Text(stringResource(R.string.cancel)) } },
        text = { Text(stringResource(R.string.remove_question)) },
    )
}

@Composable
private fun facts(item: Item): String = listOfNotNull(
    stringResource(R.string.size, item.width, item.height).takeIf { item.width > 0 },
    stringResource(R.string.duration, item.millis / 60_000, item.millis / 1000 % 60).takeIf { item.millis > 0 },
).joinToString(" · ")

@Composable
private fun AddTo(close: () -> Unit, add: (String) -> Unit) {
    val lists by loaded(emptyList<Playlist>()) { playlists() }
    AlertDialog(
        onDismissRequest = close,
        confirmButton = { TextButton(close) { Text(stringResource(R.string.cancel)) } },
        title = { Text(stringResource(R.string.add_to_playlist)) },
        text = {
            Column {
                if (lists.isEmpty()) Text(stringResource(R.string.no_playlists))
                lists.forEach { list ->
                    TextButton({
                        add(list.uid)
                        close()
                    }) { Text(list.name) }
                }
            }
        },
    )
}
