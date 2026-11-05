package io.github.wallpaper_rs

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.json.JSONArray

@Composable
fun RepoUpdates(main: Main, run: Run) {
    var checked by remember { mutableIntStateOf(0) }
    val updates by loaded(JSONArray(), checked, main.revision.intValue) { account("updates") as? JSONArray ?: JSONArray() }
    LaunchedEffect(updates) { Updates.found(updates.length()) }
    val listed = List(updates.length(), updates::getJSONObject)
    if (listed.isNotEmpty()) Text(stringResource(R.string.repo_updates), Modifier.padding(horizontal = 16.dp), style = MaterialTheme.typography.titleSmall)
    listed.forEach { update ->
        ListItem(
            headlineContent = { Text(update.getString("title").ifEmpty { update.getString("id") }) },
            supportingContent = { Text(stringResource(R.string.repo_update_from, update.getString("installed"), update.getString("available"))) },
            trailingContent = {
                TextButton({
                    run("") {
                        account("update", "uid" to update.getString("uid"))
                        checked++
                        main.changed()
                    }
                }) { Text(stringResource(R.string.repo_update)) }
            },
        )
    }
    TextButton({ run("") { account("refresh_repos", "force" to true).also { checked++ } } }, Modifier.padding(horizontal = 8.dp)) {
        Text(stringResource(R.string.repo_check))
    }
}
