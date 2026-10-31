package io.github.wallpaper_rs

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.DocumentsContract.Document
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts.OpenDocument
import androidx.activity.result.contract.ActivityResultContracts.OpenDocumentTree
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import java.io.File
import kotlin.concurrent.thread
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

private val COLUMNS = arrayOf(Document.COLUMN_DOCUMENT_ID, Document.COLUMN_MIME_TYPE, Document.COLUMN_DISPLAY_NAME, Document.COLUMN_SIZE)

fun accepted(field: JSONObject): List<String> = field.optJSONArray("accept").strings().ifEmpty { listOf("image") }

fun safeName(name: String, taken: Set<String>): String {
    val clean = name.map { if (it.isLetterOrDigit() || it == '.' || it == '-') it else '_' }.joinToString("").trimStart('.').ifEmpty { "file" }
    return generateSequence(1) { it + 1 }.map { if (it == 1) clean else "$it-$clean" }.first { it !in taken }
}

@Composable
fun rememberChooser(main: Main, uid: String): (JSONObject) -> Unit {
    var key by remember(uid) { mutableStateOf("") }
    val file = rememberLauncherForActivityResult(OpenDocument()) { picked -> picked?.let { main.act { main.chooseFile(uid, key, it) } } }
    val folder = rememberLauncherForActivityResult(OpenDocumentTree()) { picked -> picked?.let { main.act { main.chooseFolder(uid, key, it) } } }
    return { field ->
        key = field.getString("key")
        if (field.getString("kind") == "folder") folder.launch(null) else file.launch(accepted(field).map { "$it/*" }.toTypedArray())
    }
}

private fun Context.keep(uri: Uri) = runCatching { contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION) }

suspend fun Main.chooseFile(uid: String, key: String, uri: Uri) = withContext(Dispatchers.IO) {
    contentResolver.openFileDescriptor(uri, "r")?.use { ask("choose", "uid" to uid, "key" to key, "fd" to it.fd, "name" to name(uri)) }
}

suspend fun Main.chooseFolder(uid: String, key: String, tree: Uri) = withContext(Dispatchers.IO) {
    keep(tree)
    val plan = ask("folder", "uid" to uid, "key" to key, "tree" to tree.toString()) as JSONObject
    mirror(tree, plan)
    ask("folded", "uid" to uid, "key" to key)
}

suspend fun Main.unchoose(uid: String, key: String) = withContext(Dispatchers.IO) {
    (ask("unchoose", "uid" to uid, "key" to key) as? String)?.let { tree ->
        runCatching { contentResolver.releasePersistableUriPermission(Uri.parse(tree), Intent.FLAG_GRANT_READ_URI_PERMISSION) }
    }
}

fun Context.mirror(tree: Uri, plan: JSONObject): Boolean {
    val (dir, accept) = File(plan.getString("dir")).apply { mkdirs() } to plan.getJSONArray("accept").strings()
    val (max, depth) = plan.getInt("max") to plan.getInt("depth")
    val wanted = mutableMapOf<String, Pair<Uri, Long>>()
    val pending = ArrayDeque(listOf(DocumentsContract.getTreeDocumentId(tree) to 0))
    while (pending.isNotEmpty() && wanted.size < max) {
        val (parent, level) = pending.removeFirst()
        contentResolver.query(DocumentsContract.buildChildDocumentsUriUsingTree(tree, parent), COLUMNS, null, null, null)?.use { row ->
            while (row.moveToNext() && wanted.size < max) {
                val (id, type) = row.getString(0) to row.getString(1).orEmpty()
                when {
                    type == Document.MIME_TYPE_DIR -> if (level < depth) pending += id to level + 1
                    accept.any { type.startsWith("$it/") } ->
                        wanted[safeName(row.getString(2).orEmpty(), wanted.keys)] = DocumentsContract.buildDocumentUriUsingTree(tree, id) to row.getLong(3)
                }
            }
        }
    }
    val stale = dir.listFiles().orEmpty().filter { it.name !in wanted || it.length() != wanted.getValue(it.name).second }
    stale.forEach { it.delete() }
    val missing = wanted.filter { (name, _) -> !File(dir, name).exists() }
    missing.forEach { (name, source) ->
        contentResolver.openInputStream(source.first)?.use { input -> File(dir, name).outputStream().use { input.copyTo(it) } }
    }
    return stale.isNotEmpty() || missing.isNotEmpty()
}

fun Main.remirror() = thread(isDaemon = true) {
    val mirrors = runCatching { Native.library(JSONObject().put("op", "mirrors").toString()) }.getOrNull()?.let(::JSONArray) ?: return@thread
    val kept = contentResolver.persistedUriPermissions.map { it.uri.toString() }.toSet()
    val changed = (0 until mirrors.length()).map { mirrors.getJSONObject(it).getJSONObject("plan") }
        .filter { it.getString("tree") in kept }
        .count { plan -> runCatching { mirror(Uri.parse(plan.getString("tree")), plan) }.getOrDefault(false) }
    if (changed > 0) {
        Native.library(JSONObject().put("op", "mirrored").toString())
        Render.post { Wallpaper.reload(false) }
    }
}
