package io.github.wallpaper_rs

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.List
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.FavoriteBorder
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.runtime.toMutableStateList
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp

@Composable
fun rememberSelection(): SnapshotStateList<String> =
    rememberSaveable(saver = listSaver({ it.toList() }, { it.toMutableStateList() })) { mutableStateListOf() }

fun SnapshotStateList<String>.toggle(uid: String) {
    if (!remove(uid)) add(uid)
}

@Composable
fun SnapshotStateList<String>.follow(items: List<Item>?) = LaunchedEffect(items) {
    items?.map(Item::uid)?.toSet()?.let { shown -> retainAll(shown) }
}

@Composable
fun Selecting(main: Main, chosen: SnapshotStateList<String>, all: List<String>, playlist: String? = null) {
    var dialog by remember { mutableStateOf("") }
    val uids = chosen.toList()
    val done = { work: suspend () -> Unit -> main.act { work().also { chosen.clear() } } }
    BackHandler(chosen.isNotEmpty()) { chosen.clear() }
    Row(verticalAlignment = Alignment.CenterVertically) {
        IconButton(chosen::clear) { Icon(Icons.Default.Close, stringResource(R.string.cancel)) }
        Text(pluralStringResource(R.plurals.selected, uids.size, uids.size), Modifier.weight(1f), style = MaterialTheme.typography.titleLarge)
        TextButton({ chosen.addAll(all.filter { it !in chosen }) }) { Text(stringResource(R.string.select_all)) }
    }
    Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        if (playlist == null) IconButton({ dialog = "add" }) { Icon(Icons.AutoMirrored.Filled.List, stringResource(R.string.add_to_playlist)) }
        else TextButton({ done { ask("take", "playlist" to playlist, "items" to uids) } }) { Text(stringResource(R.string.take_out)) }
        IconButton({ done { ask("favorite", "uids" to uids, "favorite" to true) } }) { Icon(Icons.Default.Favorite, stringResource(R.string.favorite)) }
        IconButton({ done { ask("favorite", "uids" to uids, "favorite" to false) } }) {
            Icon(Icons.Default.FavoriteBorder, stringResource(R.string.unfavorite))
        }
        TextButton({ dialog = "fit" }) { Text(stringResource(R.string.fit)) }
        if (playlist == null) IconButton({ dialog = "remove" }) { Icon(Icons.Default.Delete, stringResource(R.string.remove)) }
    }
    val close = { dialog = "" }
    when (dialog) {
        "add" -> AddTo(close) { list -> done { ask("add", "playlist" to list, "items" to uids) } }
        "remove" -> Removing(uids.size, close) { done { ask("remove", "uids" to uids) } }
        "fit" -> Fits(close) { fit -> main.fit(uids, fit, chosen::clear) }
    }
}

@Composable
fun AddTo(close: () -> Unit, add: (String) -> Unit) {
    val lists by loaded(emptyList<Playlist>()) { playlists() }
    Choose(stringResource(R.string.add_to_playlist), close, lists.map { it.uid to it.name }, stringResource(R.string.no_playlists), add)
}

@Composable
private fun Fits(close: () -> Unit, fit: (String) -> Unit) = Choose(stringResource(R.string.fit), close, fitChoices(), "", fit)

@Composable
private fun Choose(title: String, close: () -> Unit, choices: List<Pair<String, String>>, empty: String, pick: (String) -> Unit) = AlertDialog(
    onDismissRequest = close,
    confirmButton = { TextButton(close) { Text(stringResource(R.string.cancel)) } },
    title = { Text(title) },
    text = {
        Column {
            if (choices.isEmpty()) Text(empty)
            choices.forEach { (value, label) ->
                TextButton({
                    pick(value)
                    close()
                }) { Text(label) }
            }
        }
    },
)

@Composable
fun Removing(count: Int, close: () -> Unit, remove: () -> Unit) = AlertDialog(
    onDismissRequest = close,
    confirmButton = {
        TextButton({
            close()
            remove()
        }) { Text(stringResource(R.string.remove)) }
    },
    dismissButton = { TextButton(close) { Text(stringResource(R.string.cancel)) } },
    text = { Text(pluralStringResource(R.plurals.remove_question, count, count)) },
)
