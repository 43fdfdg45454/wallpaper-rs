package io.github.wallpaper_rs

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.json.JSONArray
import org.json.JSONObject

private fun JSONArray?.objects() = this?.let { list -> List(list.length(), list::getJSONObject) }.orEmpty()

@Composable
fun Repos(main: Main, status: JSONObject, run: Run) {
    var changed by remember { mutableIntStateOf(0) }
    var exploring by remember { mutableStateOf(false) }
    var url by rememberSaveable { mutableStateOf("") }
    val shown by loaded(JSONObject(), changed, status.optInt("offers"), status.optLong("last_sync")) {
        account("repos") as? JSONObject ?: JSONObject()
    }
    fun act(work: suspend () -> Unit) = run("") {
        work()
        changed++
    }
    Text(stringResource(R.string.repos), Modifier.padding(horizontal = 16.dp), style = MaterialTheme.typography.titleSmall)
    shown.optJSONArray("offers").objects().forEach { offer ->
        val key = offer.optJSONArray("fingerprints")?.optString(0).orEmpty().take(16)
        ListItem(
            headlineContent = { Text(stringResource(R.string.repo_offer, offer.getString("name"), key)) },
            supportingContent = { Text(offer.getString("url").substringBefore('?')) },
            trailingContent = {
                Row {
                    TextButton({ act { account("offer", "repo" to offer.getString("id"), "add" to true) } }) { Text(stringResource(R.string.repo_add)) }
                    TextButton({ act { account("offer", "repo" to offer.getString("id"), "add" to false) } }) { Text(stringResource(R.string.repo_decline)) }
                }
            },
        )
    }
    shown.optJSONArray("repos").objects().forEach { repo ->
        ListItem(
            headlineContent = { Text(repo.getString("name")) },
            supportingContent = { Text(stringResource(R.string.repo_packages, repo.optInt("packages"))) },
            trailingContent = {
                TextButton({ act { account("remove_repo", "repo_id" to repo.getString("repo_id")) } }) { Text(stringResource(R.string.remove)) }
            },
        )
    }
    Column(Modifier.padding(horizontal = 16.dp), Arrangement.spacedBy(8.dp)) {
        OutlinedTextField(url, { url = it }, Modifier.fillMaxWidth(), singleLine = true, label = { Text(stringResource(R.string.repo_url)) })
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            TextButton({ act { account("add_repo", "url" to url.trim()); url = "" } }, enabled = url.isNotBlank()) {
                Text(stringResource(R.string.repo_add_url))
            }
            if (shown.optJSONArray("repos").objects().isNotEmpty()) TextButton({ exploring = true }) { Text(stringResource(R.string.repo_browse)) }
        }
    }
    if (exploring) Explore(main, run) { exploring = false }
}

@Composable
private fun Explore(main: Main, run: Run, close: () -> Unit) {
    var query by rememberSaveable { mutableStateOf("") }
    var installed by remember { mutableIntStateOf(0) }
    val found by loaded(JSONArray(), query, installed) { account("browse", "query" to query) as? JSONArray ?: JSONArray() }
    AlertDialog(
        onDismissRequest = close,
        confirmButton = { TextButton(close) { Text(stringResource(R.string.close)) } },
        title = { Text(stringResource(R.string.repo_browse)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(query, { query = it }, Modifier.fillMaxWidth(), singleLine = true, label = { Text(stringResource(R.string.repo_search)) })
                LazyColumn(Modifier.heightIn(max = 360.dp)) {
                    items(found.objects()) { item ->
                        ListItem(
                            headlineContent = { Text(item.optString("title").ifEmpty { item.getString("id") }) },
                            supportingContent = { Text(listOf(item.getString("repo"), item.getString("version"), size(item.optLong("size"))).joinToString(" · ")) },
                            trailingContent = {
                                if (item.optBoolean("installed")) Text(stringResource(R.string.repo_installed))
                                else TextButton({
                                    run("") {
                                        account("install", "repo_id" to item.getString("repo_id"), "id" to item.getString("id"))
                                        installed++
                                        main.changed()
                                    }
                                }) { Text(stringResource(R.string.repo_install)) }
                            },
                        )
                    }
                }
            }
        },
    )
}
