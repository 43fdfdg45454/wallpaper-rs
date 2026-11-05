package io.github.wallpaper_rs

import android.graphics.ImageFormat
import android.hardware.HardwareBuffer
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.util.concurrent.atomic.AtomicInteger
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ReelTest {
    private val readers = Readers("reels")
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val videos = Videos(context)
    private val tilt = Tilt(context)
    private val home = Screen(false)
    private val lock = Screen(true)

    private inner class Screen(private val locked: Boolean) {
        val frames = AtomicInteger()
        val painter = Painter(false, videos, tilt, Hearing(context), Pages(context))
        val reader = readers.make(SIDE, ImageFormat.PRIVATE, HardwareBuffer.USAGE_GPU_SAMPLED_IMAGE)

        init {
            readers.listen(reader) { source ->
                source.acquireLatestImage()?.close()
                frames.incrementAndGet()
            }
        }

        fun attach() = Render.call {
            painter.screen(locked)
            painter.attach(reader.surface)
            painter.resize(SIDE, SIDE, 0)
            painter.show(true)
        }

        fun plays() = frames.get().let { start -> waitFor { frames.get() >= start + FRAMES } }

        fun direct() = Render.call { Native.video(painter.handle) } != null

        fun filmed() = Render.call { Native.film(painter.handle) } != null
    }

    @After
    fun detach() {
        filter(true, null)
        Native.setting("fit", "fill")
        listOf(lock, home).forEach { Render.call { it.painter.detach() } }
        readers.close()
    }

    private fun filter(lock: Boolean, chosen: JSONObject?) =
        JSONObject(Native.library(JSONObject().put("op", "set_filter").put("lock", lock).put("filter", chosen ?: JSONObject.NULL).toString()))

    @Test
    fun aFilterOfTheLockScreenSendsOnlyItsVideoThroughTheGraphicsCard() {
        assertEquals(VIDEO, install(fixture(CLIP)))
        val shown = filter(true, JSONObject().put("look", "warm"))
        assertEquals("warm", shown.getJSONObject("filter").getString("look"))
        listOf(home, lock).forEach { it.attach() }
        assertTrue(home.plays())
        assertTrue(lock.plays())
        assertTrue(home.direct())
        assertTrue(!lock.direct() && lock.filmed())
        filter(true, null)
        Render.call { lock.painter.refresh() }
        assertTrue(waitFor { lock.direct() })
    }

    @Test
    fun hdrVideosThroughTheGraphicsCardAreDrawnWithoutFailures() {
        Native.setting("fit", "fit")
        assertEquals(VIDEO, install(fixture("pq-64x32.mkv")))
        home.attach()
        assertTrue(home.filmed())
        assertTrue(home.plays())
        val clips = JSONObject(Native.diagnostics()).getJSONObject("clips")
        assertTrue("clips $clips", clips.getLong("frames") > 0)
        assertEquals("clips $clips", 0, clips.getInt("failed"))
    }

    @Test
    fun aShortcutOpensOnlyWhatTheUserChose() {
        val target = JSONObject().put("uri", "https://example.org/")
        Shortcuts.each(JSONObject().put("open", target).let { "[$it]" }) { assertEquals(target.toString(), it.toString()) }
        assertNull(Shortcuts.web("http://example.org"))
        assertNotNull(Shortcuts.web(" https://example.org/a "))
        assertTrue(Shortcuts.apps(context).none { it.first == context.packageName })
    }

    private companion object {
        const val SIDE = 64
        const val FRAMES = 5
        const val CLIP = "magenta-64x32.webm"
    }
}
