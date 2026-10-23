package io.github.wallpaper_rs

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalLocale
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp

private val FILTERS = listOf(
    R.string.all to "",
    R.string.images to "kind:image",
    R.string.videos to "kind:video",
    R.string.scenes to SCENES,
    R.string.favorites to "fav:",
)

@Composable
fun Library(main: Main, open: (String) -> Unit) = Box(Modifier.fillMaxSize()) {
    var filter by rememberSaveable { mutableIntStateOf(0) }
    val chosen = rememberSelection()
    val query = FILTERS[filter].second
    val languages = listOf(LocalLocale.current.toLanguageTag().replace('-', '_'))
    val items by loaded<List<Item>?>(null, main.revision.intValue, query, languages) {
        fetch("items", "query" to query, "languages" to languages)
    }
    chosen.follow(items)
    Column {
        if (chosen.isEmpty()) Header(stringResource(R.string.library)) else Selecting(main, chosen, items.orEmpty().map(Item::uid))
        Choices(null, FILTERS.map { (label, query) -> query to stringResource(label) }, query) { chosen ->
            filter = FILTERS.indexOfFirst { it.second == chosen }
        }
        if (query == SCENES) Row(Modifier.padding(horizontal = 8.dp), Arrangement.spacedBy(8.dp)) {
            TextButton(main::examples) { Text(stringResource(R.string.examples)) }
            TextButton(main::openPackage) { Text(stringResource(R.string.open_package)) }
        }
        Importing(main)
        if (items?.isEmpty() == true) Text(stringResource(R.string.empty), Modifier.padding(16.dp))
        LazyVerticalGrid(
            columns = GridCells.Adaptive(112.dp),
            contentPadding = PaddingValues(16.dp, 8.dp, 16.dp, 96.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items(items.orEmpty(), key = Item::uid) { item ->
                val tap = { if (chosen.isEmpty()) open(item.uid) else chosen.toggle(item.uid) }
                val press = Modifier.combinedClickable(onLongClick = { chosen.toggle(item.uid) }, onClick = tap)
                Thumb(item, Modifier.aspectRatio(0.75f).then(press), item.uid in chosen)
            }
        }
    }
    if (chosen.isEmpty()) Adding(main, Modifier.align(Alignment.BottomEnd).padding(16.dp))
}

@Composable
fun Thumb(item: Item, modifier: Modifier = Modifier, picked: Boolean = false) {
    val bitmap by thumbnail(item)
    val shape = MaterialTheme.shapes.medium
    val outline = if (picked) Modifier.border(3.dp, MaterialTheme.colorScheme.primary, shape) else Modifier
    Box(modifier.then(outline).clip(shape).background(MaterialTheme.colorScheme.surfaceVariant)) {
        val hidden = if (item.mature) Modifier.blur(24.dp) else Modifier
        bitmap?.let { Image(it, item.title, Modifier.fillMaxSize().then(hidden), contentScale = ContentScale.Crop) }
        if (item.mature) Icon(Icons.Default.Lock, stringResource(R.string.mature), Modifier.align(Alignment.Center), Color.White)
        if (item.flashing) Icon(Icons.Default.Warning, stringResource(R.string.flashing), Modifier.align(Alignment.TopEnd).padding(4.dp), Color.White)
        val corner = Modifier.align(Alignment.BottomStart).padding(4.dp)
        if (item.video) Icon(Icons.Default.PlayArrow, null, corner, Color.White)
        if (item.scene) Icon(Icons.Default.Star, null, corner, Color.White)
        if (picked) Icon(Icons.Default.CheckCircle, null, Modifier.align(Alignment.TopStart).padding(4.dp), MaterialTheme.colorScheme.primary)
    }
}

private const val SCENES = "kind:scene"
