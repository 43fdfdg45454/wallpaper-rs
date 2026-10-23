package io.github.wallpaper_rs

import android.net.Uri
import android.provider.DocumentsContract
import android.provider.DocumentsContract.Document
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlin.concurrent.thread

@Composable
fun Adding(main: Main, modifier: Modifier) = Box(modifier) {
    var open by remember { mutableStateOf(false) }
    ExtendedFloatingActionButton(onClick = { open = true }, icon = { Icon(Icons.Default.Add, null) }, text = { Text(stringResource(R.string.choose)) })
    DropdownMenu(open, { open = false }) {
        listOf(R.string.photos to main::choose, R.string.files to main::chooseFiles, R.string.folder to main::chooseFolder).forEach { (label, pick) ->
            DropdownMenuItem({ Text(stringResource(label)) }, {
                open = false
                pick()
            })
        }
    }
}

@Composable
fun Importing(main: Main) {
    val (left, total) = main.importing.intValue to main.imports.intValue
    if (left <= 0) return
    val bar = Modifier.fillMaxWidth().padding(16.dp, 8.dp)
    if (total <= 0) LinearProgressIndicator(bar) else {
        LinearProgressIndicator({ (total - left).toFloat() / total }, bar)
        Note(stringResource(R.string.imported, total - left, total))
    }
}

private val COLUMNS = arrayOf(Document.COLUMN_DOCUMENT_ID, Document.COLUMN_MIME_TYPE)
private const val MAX_FILES = 5000

fun Main.importFolder(tree: Uri) {
    importing.intValue++
    thread {
        val found = runCatching { media(tree) }.getOrDefault(emptyList())
        runOnUiThread {
            importing.intValue--
            if (found.isEmpty()) toast(getString(R.string.folder_empty)) else store(found)
        }
    }
}

private fun Main.media(tree: Uri): List<Uri> {
    val (found, pending) = mutableListOf<Uri>() to ArrayDeque(listOf(DocumentsContract.getTreeDocumentId(tree)))
    while (pending.isNotEmpty() && found.size < MAX_FILES) {
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, pending.removeFirst())
        contentResolver.query(children, COLUMNS, null, null, null)?.use { row ->
            while (row.moveToNext()) {
                val (id, type) = row.getString(0) to row.getString(1).orEmpty()
                when {
                    type == Document.MIME_TYPE_DIR -> pending += id
                    type.startsWith("image/") || type.startsWith("video/") -> found += DocumentsContract.buildDocumentUriUsingTree(tree, id)
                }
            }
        }
    }
    return found.take(MAX_FILES)
}
