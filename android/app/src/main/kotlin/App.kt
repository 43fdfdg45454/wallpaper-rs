package io.github.wallpaper_rs

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.List
import androidx.compose.material.icons.filled.AccountCircle
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp

private val TABS = listOf(
    R.string.library to Icons.Default.Home,
    R.string.playlists to Icons.AutoMirrored.Filled.List,
    R.string.devices to Icons.Default.AccountCircle,
    R.string.settings to Icons.Default.Settings,
)

@Composable
fun App(main: Main) {
    val context = LocalContext.current
    val colors = if (isSystemInDarkTheme()) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
    var tab by rememberSaveable { mutableIntStateOf(0) }
    var item by rememberSaveable { mutableStateOf<String?>(null) }
    var playlist by rememberSaveable { mutableStateOf<String?>(null) }
    BackHandler(item != null || playlist != null) { if (item != null) item = null else playlist = null }
    MaterialTheme(colorScheme = colors) {
        Scaffold(bottomBar = {
            if (item == null) NavigationBar {
                TABS.forEachIndexed { index, (label, icon) ->
                    NavigationBarItem(
                        selected = tab == index,
                        onClick = {
                            tab = index
                            playlist = null
                        },
                        icon = { Icon(icon, null) },
                        label = { Text(stringResource(label)) },
                    )
                }
            }
        }) { padding ->
            Box(Modifier.padding(padding)) {
                val (shown, list) = item to playlist
                when {
                    shown != null -> Detail(main, shown) { item = null }
                    tab == 0 -> Library(main) { item = it }
                    tab == 1 && list != null -> Entries(main, list, { item = it }) { playlist = null }
                    tab == 1 -> Playlists(main) { playlist = it }
                    tab == 2 -> Account(main)
                    else -> Settings(main)
                }
            }
        }
    }
}

@Composable
fun Header(title: String, back: (() -> Unit)? = null) = Row(verticalAlignment = Alignment.CenterVertically) {
    back?.let { IconButton(it) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } }
    Text(title, Modifier.padding(16.dp), style = MaterialTheme.typography.headlineSmall)
}

@Composable
fun Choices(label: String?, options: List<Pair<String, String>>, chosen: String, choose: (String) -> Unit) =
    Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        label?.let { Text(it, style = MaterialTheme.typography.titleSmall) }
        Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            options.forEach { (value, text) -> FilterChip(chosen == value, { choose(value) }, { Text(text) }) }
        }
    }

@Composable
fun Toggle(label: String, checked: Boolean, change: (Boolean) -> Unit) = ListItem(
    headlineContent = { Text(label) },
    trailingContent = { Switch(checked, change) },
    modifier = Modifier.clickable { change(!checked) },
)

@Composable
fun Note(text: String, color: Color = Color.Unspecified) =
    Text(text, Modifier.padding(16.dp, 4.dp), color, style = MaterialTheme.typography.bodySmall)
