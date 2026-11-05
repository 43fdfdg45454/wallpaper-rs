package io.github.wallpaper_rs

import android.media.MediaExtractor
import android.media.MediaFormat
import android.os.ParcelFileDescriptor
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
class ConvertingTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val context = instrumentation.targetContext

    private fun shell(command: String) =
        ParcelFileDescriptor.AutoCloseInputStream(instrumentation.uiAutomation.executeShellCommand(command)).use { it.readBytes() }

    private fun stored(name: String): String {
        val file = fixture(name)
        return ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY).use { JSONObject(Native.store(it.fd, "clip-${System.nanoTime()}.webm")!!) }.getString("uid")
    }

    private fun item(uid: String) = JSONObject(Native.library(JSONObject().put("op", "item").put("uid", uid).toString()))

    private fun mime(path: String) = MediaExtractor().run {
        setDataSource(path)
        val found = (0 until trackCount).map { getTrackFormat(it).getString(MediaFormat.KEY_MIME).orEmpty() }.first { it.startsWith("video/") }
        release()
        found
    }

    private fun count() = JSONArray(Native.library(JSONObject().put("op", "items").put("query", "kind:video").toString())).length()

    @Test
    fun aConvertedVideoTakesThePlaceOfItsOriginal() {
        storage()
        val uid = stored("quadrants-64x32.webm")
        val before = item(uid).getString("path")
        Converting.offer(context, Conversion(uid, before, "clip", false, 64 to 32))
        assertTrue("the conversion ends", waitFor { item(uid).getString("path").endsWith(".mp4") })
        assertTrue(waitFor { uid !in Converting.progress })
        val after = item(uid).getString("path")
        assertTrue(after, after.endsWith(".mp4") && !File(before).exists())
        assertEquals("video/avc", mime(after))
        assertTrue(item(uid).optInt("width") > 0)
    }

    @Test
    fun keepingTheOriginalAddsTheConvertedCopy() {
        storage()
        val uid = stored("magenta-64x32.webm")
        val (path, videos) = item(uid).getString("path") to count()
        Converting.offer(context, Conversion(uid, path, "clip", true, 64 to 32))
        assertTrue("the converted copy is added", waitFor { count() == videos + 1 })
        assertTrue(waitFor { uid !in Converting.progress })
        assertEquals(path, item(uid).getString("path"))
        assertEquals(videos + 1, count())
    }

    @Test
    fun aStoppedConversionLeavesTheOriginal() {
        storage()
        val uid = stored("magenta-64x32.webm")
        val path = item(uid).getString("path")
        Converting.offer(context, Conversion(uid, path, "clip", false, 64 to 32))
        Converting.stop(uid)
        assertTrue(waitFor { uid !in Converting.progress })
        assertEquals(path, item(uid).getString("path"))
        assertFalse(File(File(path).parentFile?.parentFile, "converting/$uid.mp4").exists())
    }

    @Test
    fun batterySaverHoldsBackOnlyTheConversionsNobodyAskedFor() {
        storage()
        val (asked, automatic) = stored("magenta-64x32.webm") to stored("quadrants-64x32.webm")
        val (first, kept) = item(asked).getString("path") to item(automatic).getString("path")
        try {
            shell("dumpsys battery unplug")
            assertTrue("battery saver is on", waitFor { shell("cmd power set-mode 1").let { Convert.saving(context) } })
            Converting.offer(context, Conversion(asked, first, "asked", false, 64 to 32))
            Converting.offer(context, Conversion(automatic, kept, "automatic", false, 64 to 32, asked = false))
            assertTrue("the asked conversion runs", waitFor { asked in Converting.progress })
            Converting.stop(asked)
            assertTrue(waitFor { automatic !in Converting.progress && asked !in Converting.progress })
            Thread.sleep(SETTLE_MS)
            assertEquals("nothing converts on its own while saving battery", kept, item(automatic).getString("path"))
            assertFalse(File(File(kept).parentFile?.parentFile, "converting/$automatic.mp4").exists())
        } finally {
            shell("cmd power set-mode 0")
            shell("dumpsys battery reset")
        }
    }

    private companion object {
        const val SETTLE_MS = 1500L
    }
}
