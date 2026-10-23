package io.github.wallpaper_rs

import android.graphics.Color
import android.graphics.ImageFormat
import android.hardware.HardwareBuffer
import android.media.ImageReader
import android.os.Handler
import android.os.HandlerThread
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.util.concurrent.atomic.AtomicInteger
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class VideoTest {
    private val thread = HandlerThread("frames").apply { start() }
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val videos = Videos(context)
    private val tilt = Tilt(context)
    private val lock = Screen()
    private val home = Screen()

    private inner class Screen {
        val frames = AtomicInteger()
        val painter = Painter(false, videos, tilt, Hearing(context))
        val reader: ImageReader =
            ImageReader.newInstance(SIDE, SIDE, ImageFormat.PRIVATE, 3, HardwareBuffer.USAGE_GPU_SAMPLED_IMAGE)

        init {
            reader.setOnImageAvailableListener({ source ->
                source.acquireLatestImage()?.close()
                frames.incrementAndGet()
            }, Handler(thread.looper))
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
        listOf(lock, home).forEach {
            Render.call { it.painter.detach() }
            it.reader.close()
        }
        thread.quitSafely()
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

    private companion object {
        const val SIDE = 64
        const val FRAMES = 5
        const val CLIP = "magenta-64x32.webm"
    }
}
