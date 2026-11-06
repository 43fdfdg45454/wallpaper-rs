package io.github.wallpaper_rs

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class StallTest {
    private fun unpack(path: String, to: File) {
        val assets = InstrumentationRegistry.getInstrumentation().context.assets
        val names = assets.list(path).orEmpty()
        if (names.isEmpty()) assets.open(path).use { input -> to.outputStream().use { input.copyTo(it) } }
        else names.forEach { unpack("$path/$it", File(to.apply { mkdirs() }, it)) }
    }

    @Test
    fun aStaticRepositoryFolderIsOpenedFilledUnlistedAndFeaturedFromThePhone() {
        val dir = File(storage(), "stall").apply { deleteRecursively() }
        unpack("stall", dir)
        Anchor.renew(fixture("stall-anchor.json").copyTo(File(dir, "root/1.json"), true))
        val wpak = fixture("stall-dawn.wpak")
        fun ask(op: String, vararg args: Pair<String, Any?>) = JSONObject(Native.stall(JSONObject(mapOf("op" to op, "dir" to dir.path, *args)).toString()))
        val opened = ask("open")
        assertEquals("Example wallpapers", opened.getString("name"))
        assertEquals(0, opened.getJSONArray("packages").length())
        assertEquals(0, opened.getJSONArray("changed").length())
        val added = ask("add", "files" to JSONArray(listOf(wpak.path)), "unlisted" to true)
        val changed = added.getJSONArray("changed").strings()
        assertTrue(changed.toString(), "catalog.json" in changed && "packages/org.example.dawn/1.0.0.wpak" in changed)
        assertTrue(changed.toString(), changed.all(::safe))
        val dawn = added.getJSONArray("packages").getJSONObject(0)
        assertEquals("Dawn", titled(dawn.getJSONArray("versions").getJSONObject(0).opt("title")))
        assertFalse(dawn.getJSONArray("versions").getJSONObject(0).getBoolean("listed"))
        val listed = ask("list", "id" to "org.example.dawn", "listed" to true)
        assertTrue(listed.getJSONArray("packages").getJSONObject(0).getJSONArray("versions").getJSONObject(0).getBoolean("listed"))
        assertEquals(listOf("catalog.json"), listed.getJSONArray("changed").strings())
        val featured = ask("feature", "id" to "org.example.dawn", "note" to "Calm morning", "on" to true)
        assertEquals("Calm morning", featured.getJSONArray("packages").getJSONObject(0).getString("note"))
        val unfeatured = ask("feature", "id" to "org.example.dawn", "on" to false)
        assertFalse(unfeatured.getJSONArray("packages").getJSONObject(0).getBoolean("featured"))
        assertTrue(ask("feature", "id" to "org.example.none", "on" to true).has("error"))
        assertTrue(ask("publish", "key" to "not a key", "days" to 7).has("error"))
        dir.deleteRecursively()
    }
}
