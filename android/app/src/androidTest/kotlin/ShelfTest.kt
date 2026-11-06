package io.github.wallpaper_rs

import android.provider.DocumentsContract
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

private const val REPOSITORY = "repo"
private const val INCOMING = "in/dawn.wpak"

@RunWith(AndroidJUnit4::class)
class ShelfTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val app = instrumentation.targetContext
    private val authority = instrumentation.context.packageName + ".folder"
    private val tree = DocumentsContract.buildTreeDocumentUri(authority, REPOSITORY)

    private fun seed() {
        app.contentResolver.call(instrumentation.context.packageName + ".seeder", "seed", app.packageName, null)
    }

    private fun files(parent: String = REPOSITORY): List<String> {
        val columns = arrayOf(DocumentsContract.Document.COLUMN_DOCUMENT_ID, DocumentsContract.Document.COLUMN_MIME_TYPE)
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, parent)
        val found = app.contentResolver.query(children, columns, null, null, null)!!.use { row ->
            buildList { while (row.moveToNext()) add(row.getString(0) to row.getString(1)) }
        }
        return found.flatMap { (id, type) -> if (type == DocumentsContract.Document.MIME_TYPE_DIR) files(id) else listOf(id) }
    }

    private fun read(path: String) =
        app.contentResolver.openInputStream(DocumentsContract.buildDocumentUriUsingTree(tree, path))!!.use { it.readBytes().decodeToString() }

    @Test
    fun aStaticRepositoryInAPickedFolderIsCopiedInAndItsChangesAreWrittenBack() {
        storage()
        seed()
        val opened = runBlocking { app.stalled(tree, "open") }
        assertEquals(opened.toString(), "Example wallpapers", opened.getString("name"))
        val wpak = DocumentsContract.buildDocumentUri(authority, INCOMING)
        val added = runBlocking { app.stalled(tree, "add", "unlisted" to true, wpaks = listOf(wpak)) }
        val changed = added.getJSONArray("changed").strings()
        assertTrue(changed.toString(), "packages/org.example.dawn/1.0.0.wpak" in changed)
        val written = files()
        assertTrue(written.toString(), "$REPOSITORY/packages/org.example.dawn/1.0.0.wpak" in written && File(REPOSITORY, "root/1.json").path in written)
        assertTrue(read("$REPOSITORY/catalog.json"), "org.example.dawn" in read("$REPOSITORY/catalog.json"))
        val listed = runBlocking { app.stalled(tree, "list", "id" to "org.example.dawn", "listed" to true) }
        val version = listed.getJSONArray("packages").getJSONObject(0).getJSONArray("versions").getJSONObject(0)
        assertTrue("a package left in the folder was not read back: $listed", version.getBoolean("listed"))
        assertEquals(written.toSet(), files().toSet())
        assertFalse("the copy of the folder stayed in the cache", File(app.cacheDir, "stall").exists() || File(app.cacheDir, "stall-in").exists())
    }

    @Test
    fun aPickedRepositoryFolderIsKeptWhileItsGrantLastsAndForgottenWhenAnotherIsPicked() {
        seed()
        app.keepStall(tree)
        assertEquals(tree.toString(), app.keptStall())
        assertTrue(app.contentResolver.persistedUriPermissions.any { it.uri == tree && it.isWritePermission })
        val other = DocumentsContract.buildTreeDocumentUri(authority, "other")
        app.keepStall(other)
        assertNull("a folder without a lasting grant was kept", app.keptStall())
        assertFalse("the grant of the folder picked before stayed", app.contentResolver.persistedUriPermissions.any { it.uri == tree })
        assertNull(app.keptStall())
    }

    @Test
    fun aRepositoryFolderAlsoMirroredForAWallpaperKeepsItsReadGrant() {
        seed()
        val read = { uri: android.net.Uri -> app.contentResolver.persistedUriPermissions.filter { it.uri == uri } }
        val mirrored = { it: String -> it == tree.toString() }
        app.keepStall(tree, mirrored)
        app.keepStall(DocumentsContract.buildTreeDocumentUri(authority, "other"), mirrored)
        assertTrue("the read grant a mirror uses went with the repository", read(tree).any { it.isReadPermission && !it.isWritePermission })
        app.keepStall(tree, mirrored)
        app.release(tree.toString(), android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION) { false }
        assertEquals("unchoosing the folder of a wallpaper dropped the repository", tree.toString(), app.keptStall())
        app.keepStall(DocumentsContract.buildTreeDocumentUri(authority, "other")) { false }
        assertTrue(read(tree).isEmpty())
    }
}
