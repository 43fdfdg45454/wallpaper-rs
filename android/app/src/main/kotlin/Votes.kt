package io.github.wallpaper_rs

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import org.json.JSONArray
import org.json.JSONObject

suspend fun votes(ids: List<String>): JSONObject =
    account("votes", "ids" to JSONArray(ids.take(1000))) as? JSONObject ?: JSONObject()

@Composable
fun Votes(id: String, counts: JSONObject?, shown: Boolean, run: Run, voted: (JSONObject) -> Unit) {
    val mine = counts?.optString("mine")?.ifEmpty { null } ?: "none"
    fun cast(vote: String) = run("") {
        val found = account("vote", "id" to id, "vote" to if (mine == vote) "none" else vote) as? JSONObject
        found?.let(voted)
    }
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        listOf(
            Triple("up", R.string.vote_up, R.string.votes_up),
            Triple("down", R.string.vote_down, R.string.votes_down),
        ).forEach { (vote, plain, counted) ->
            val text = if (shown && counts != null) stringResource(counted, counts.optInt(vote)) else stringResource(plain)
            FilterChip(mine == vote, { cast(vote) }, { Text(text) })
        }
    }
}
