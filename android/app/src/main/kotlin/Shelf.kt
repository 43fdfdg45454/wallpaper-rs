package io.github.wallpaper_rs

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.DocumentsContract.Document
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

private val CHILDREN = arrayOf(Document.COLUMN_DOCUMENT_ID, Document.COLUMN_MIME_TYPE, Document.COLUMN_DISPLAY_NAME)
private val LEFT = setOf("packages", "previews")
private val SHELVING = Mutex()
private const val KEPT = "stall"
private const val TREE = "tree"
private const val GRANTED = Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION

fun safe(path: String) = path.split('/').none { it.isEmpty() || it == "." || it == ".." }

fun shelved(path: String) = safe(path) && path.substringBefore('/') !in LEFT

private class Shelf(context: Context, private val tree: Uri) {
    private val resolver = context.contentResolver
    val dir = File(context.cacheDir, "stall")
    val bring = File(context.cacheDir, "stall-in")
    private val top = DocumentsContract.getTreeDocumentId(tree)

    private fun children(parent: String) =
        resolver.query(DocumentsContract.buildChildDocumentsUriUsingTree(tree, parent), CHILDREN, null, null, null)?.use { row ->
            buildList { while (row.moveToNext()) add(Triple(row.getString(0), row.getString(1).orEmpty(), row.getString(2).orEmpty())) }
        }.orEmpty()

    private fun uri(id: String) = DocumentsContract.buildDocumentUriUsingTree(tree, id)

    fun mirror() {
        listOf(dir, bring).forEach { it.deleteRecursively() }
        val pending = ArrayDeque(listOf(top to ""))
        while (pending.isNotEmpty()) {
            val (parent, prefix) = pending.removeFirst()
            children(parent).forEach { (id, type, name) ->
                val path = prefix + name
                when {
                    !shelved(path) -> {}
                    type == Document.MIME_TYPE_DIR -> pending += id to "$path/"
                    else -> resolver.openInputStream(uri(id))?.use { input ->
                        File(dir, path).apply { parentFile?.mkdirs() }.outputStream().use { input.copyTo(it) }
                    }
                }
            }
        }
    }

    fun take(files: List<Uri>): List<String> = files.mapIndexed { index, source ->
        val file = File(bring.apply { mkdirs() }, "$index.wpak")
        resolver.openInputStream(source)?.use { input -> file.outputStream().use { input.copyTo(it) } }
        file.path
    }

    private fun made(parent: String, name: String, type: String): String {
        children(parent).firstOrNull { it.third == name }?.let { return it.first }
        val made = DocumentsContract.createDocument(resolver, uri(parent), type, name) ?: error(name)
        return DocumentsContract.getDocumentId(made)
    }

    fun write(changed: List<String>) = changed.filter(::safe).forEach { path ->
        val parts = path.split('/')
        val parent = parts.dropLast(1).fold(top) { at, name -> made(at, name, Document.MIME_TYPE_DIR) }
        val target = uri(made(parent, parts.last(), "application/octet-stream"))
        resolver.openOutputStream(target, "wt")?.use { out -> File(dir, path).inputStream().use { it.copyTo(out) } }
    }

    fun clear() = listOf(dir, bring).forEach { it.deleteRecursively() }
}

suspend fun Context.stalled(tree: Uri, op: String, vararg args: Pair<String, Any?>, wpaks: List<Uri> = emptyList()): JSONObject =
    withContext(Dispatchers.IO) {
        SHELVING.withLock {
            val shelf = Shelf(this@stalled, tree)
            try {
                shelf.mirror()
                val files = JSONArray(shelf.take(wpaks))
                val reply = stall(op, "dir" to shelf.dir.path, "files" to files, *args)
                shelf.write(reply.optJSONArray("changed").strings())
                reply
            } finally {
                shelf.clear()
            }
        }
    }

fun Context.keptStall(): String? {
    val kept = getSharedPreferences(KEPT, Context.MODE_PRIVATE)
    val tree = kept.getString(TREE, null) ?: return null
    val granted = contentResolver.persistedUriPermissions.any { it.uri.toString() == tree && it.isReadPermission && it.isWritePermission }
    if (!granted) kept.edit().remove(TREE).apply()
    return tree.takeIf { granted }
}

fun Context.keepStall(tree: Uri) {
    val kept = getSharedPreferences(KEPT, Context.MODE_PRIVATE)
    kept.getString(TREE, null)?.takeIf { it != tree.toString() }?.let { old ->
        runCatching { contentResolver.releasePersistableUriPermission(Uri.parse(old), GRANTED) }
    }
    runCatching { contentResolver.takePersistableUriPermission(tree, GRANTED) }
    kept.edit().putString(TREE, tree.toString()).apply()
}
