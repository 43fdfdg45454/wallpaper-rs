package io.github.wallpaper_rs

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
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
    val query = FILTERS[filter].second
    val items by loaded<List<Item>?>(null, main.revision.intValue, query) { fetch("items", "query" to query) }
    Column {
        Header(stringResource(R.string.library))
        Choices(null, FILTERS.map { (label, query) -> query to stringResource(label) }, query) { chosen ->
            filter = FILTERS.indexOfFirst { it.second == chosen }
        }
        if (query == SCENES) Row(Modifier.padding(horizontal = 8.dp), Arrangement.spacedBy(8.dp)) {
            TextButton(main::examples) { Text(stringResource(R.string.examples)) }
            TextButton(main::openPackage) { Text(stringResource(R.string.open_package)) }
        }
        if (main.importing.intValue > 0) LinearProgressIndicator(Modifier.fillMaxWidth().padding(16.dp))
        if (items?.isEmpty() == true) Text(stringResource(R.string.empty), Modifier.padding(16.dp))
        LazyVerticalGrid(
            columns = GridCells.Adaptive(112.dp),
            contentPadding = PaddingValues(16.dp, 8.dp, 16.dp, 96.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items(items.orEmpty(), key = Item::uid) { item ->
                Thumb(item, Modifier.aspectRatio(0.75f).clickable { open(item.uid) })
            }
        }
    }
    ExtendedFloatingActionButton(
        onClick = main::choose,
        icon = { Icon(Icons.Default.Add, null) },
        text = { Text(stringResource(R.string.choose)) },
        modifier = Modifier.align(Alignment.BottomEnd).padding(16.dp),
    )
}

@Composable
fun Thumb(item: Item, modifier: Modifier = Modifier) {
    val bitmap by thumbnail(item)
    Box(modifier.clip(MaterialTheme.shapes.medium).background(MaterialTheme.colorScheme.surfaceVariant)) {
        bitmap?.let { Image(it, item.title, Modifier.fillMaxSize(), contentScale = ContentScale.Crop) }
        val corner = Modifier.align(Alignment.BottomStart).padding(4.dp)
        if (item.video) Icon(Icons.Default.PlayArrow, null, corner, Color.White)
        if (item.scene) Icon(Icons.Default.Star, null, corner, Color.White)
    }
}

private const val SCENES = "kind:scene"
