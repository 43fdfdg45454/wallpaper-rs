package io.github.wallpaper_rs

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp

val INTERVALS = listOf(
    "15m" to R.string.every_15m,
    "30m" to R.string.every_30m,
    "1h" to R.string.every_1h,
    "6h" to R.string.every_6h,
    "1d" to R.string.every_1d,
    "0s" to R.string.never,
)

@Composable
fun Playlists(main: Main, open: (String) -> Unit) = Box(Modifier.fillMaxSize()) {
    val lists by loaded(emptyList<Playlist>(), main.revision.intValue) { playlists() }
    var naming by remember { mutableStateOf<String?>(null) }
    LazyColumn(contentPadding = PaddingValues(bottom = 96.dp)) {
        item { Header(stringResource(R.string.playlists)) }
        if (lists.isEmpty()) item { Text(stringResource(R.string.no_playlists), Modifier.padding(16.dp)) }
        items(lists, key = Playlist::uid) { list ->
            ListItem(
                headlineContent = { Text(list.name) },
                supportingContent = { Text(summary(list)) },
                trailingContent = { if (list.current) Icon(Icons.Default.Check, stringResource(R.string.in_use)) },
                modifier = Modifier.clickable { open(list.uid) },
            )
        }
    }
    ExtendedFloatingActionButton(
        onClick = { naming = "" },
        icon = { Icon(Icons.Default.Add, null) },
        text = { Text(stringResource(R.string.new_playlist)) },
        modifier = Modifier.align(Alignment.BottomEnd).padding(16.dp),
    )
    naming?.let { name ->
        AlertDialog(
            onDismissRequest = { naming = null },
            confirmButton = {
                TextButton({
                    naming = null
                    main.act { ask("create", "name" to name) }
                }, enabled = name.isNotBlank()) { Text(stringResource(R.string.create)) }
            },
            title = { Text(stringResource(R.string.new_playlist)) },
            text = {
                val label = stringResource(R.string.name)
                OutlinedTextField(name, { naming = it }, label = { Text(label) }, singleLine = true)
            },
        )
    }
}

@Composable
private fun summary(list: Playlist): String {
    val every = INTERVALS.find { it.first == list.rules.optString("interval") }?.second ?: R.string.every_30m
    return pluralStringResource(R.plurals.wallpapers, list.count, list.count) + " · " + stringResource(every)
}
