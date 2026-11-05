package io.github.wallpaper_rs

import android.graphics.Color
import android.graphics.ImageFormat
import android.hardware.HardwareBuffer
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.util.concurrent.atomic.AtomicInteger
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class VideoTest {
    private val readers = Readers("frames")
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val videos = Videos(context)
    private val tilt = Tilt(context)
    private val lock = Screen()
    private val home = Screen()

    private inner class Screen {
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
            painter.attach(reader.surface)
            painter.resize(SIDE, SIDE, 0)
        }

        fun show(visible: Boolean) = Render.call { painter.show(visible) }

        fun plays() = frames.get().let { start -> waitFor { frames.get() >= start + FRAMES } }

        fun stopped() = settled(frames) == quiet(frames)
    }

    @After
    fun detach() {
        listOf(lock, home).forEach { Render.call { it.painter.detach() } }
        readers.close()
    }

    @Test
    fun videosTakeTheSurfaceAndGiveItBack() {
        assertEquals(IMAGE, install(still(Color.RED)))
        home.attach()
        home.show(true)
        assertTrue(waitFor { home.frames.get() > 0 })
        assertEquals(VIDEO, install(fixture(CLIP)))
        Render.call { home.painter.refresh() }
        assertTrue(home.plays())
        home.show(false)
        assertTrue(home.stopped())
        assertEquals(IMAGE, install(still(Color.BLUE)))
        val before = home.frames.get()
        Render.call { home.painter.refresh() }
        home.show(true)
        assertTrue(waitFor { home.frames.get() > before })
        assertTrue(home.stopped())
    }

    @Test
    fun oneVideoPlaysWhereItWasShownLast() {
        assertEquals(VIDEO, install(fixture(CLIP)))
        lock.attach()
        home.attach()
        lock.show(true)
        assertTrue(lock.plays())
        home.show(true)
        assertTrue(home.plays())
        assertTrue(lock.stopped())
        lock.show(false)
        lock.show(true)
        assertTrue(lock.plays())
        assertTrue(home.stopped())
    }

    @Test
    fun aVideoLeftOnOneScreenFadesOutFromWhereItWas() {
        Native.setting("transition.kind", "fade")
        assertEquals(VIDEO, install(fixture(CLIP)))
        listOf(lock, home).forEach { it.attach() }
        listOf(lock, home).forEach { it.show(true) }
        assertTrue(home.plays())
        assertTrue(waitFor { Render.call { videos.positions().values.single() } in 300L..700L })
        assertEquals(IMAGE, install(still(Color.RED)))
        val (before, after) = Render.call { videos.positions().also { home.painter.refresh() } to videos.positions() }
        val shared = before.values.single()
        val lent = after.filterKeys { it !in before }.values.single()
        assertTrue("the video went from $shared to $lent", kotlin.math.abs(lent - shared) < 100)
    }

    @Test
    fun eachScreenPlaysASharedVideoWithItsOwnSettings() {
        val uri = "file://${fixture(CLIP).path}"
        val reel = { rate: Double -> JSONObject().put("rate", rate).put("loop", true).put("volume", 1.0).put("start", 0).put("end", -1).toString() }
        val speeds = Render.call {
            val speed = { videos.videos.getValue(uri).speed }
            listOf(lock, home).forEach { videos.join(uri, it.painter) }
            videos.cut(uri, reel(2.0), lock.painter)
            videos.cut(uri, reel(0.5), home.painter)
            lock.painter.show(true)
            videos.show(uri, lock.painter)
            val shownOnLock = speed()
            home.painter.show(true)
            videos.show(uri, home.painter)
            val shownAtHome = speed()
            videos.leave(uri, home.painter)
            listOf(shownOnLock, shownAtHome, speed()).also { videos.leave(uri, lock.painter) }
        }
        assertEquals(listOf(2f, 0.5f, 2f), speeds)
    }

    @Test
    fun importsKeepOnlyVideosThePhoneDecodes() {
        assertTrue(playable { setDataSource(fixture(CLIP).path) })
        assertFalse(playable { setDataSource(still(Color.RED).path) })
    }

    private companion object {
        const val SIDE = 64
        const val FRAMES = 5
        const val CLIP = "magenta-64x32.webm"
    }
}
