package io.github.wallpaper_rs

import android.graphics.Color
import android.graphics.PixelFormat
import android.hardware.HardwareBuffer
import android.media.ImageReader
import android.os.Handler
import android.os.HandlerThread
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.atomic.AtomicInteger
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class RenderTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val frames = AtomicInteger()
    private val pixel = AtomicInteger()
    private val seen = CopyOnWriteArrayList<Int>()
    private val edges = CopyOnWriteArrayList<Int>()
    private val thread = HandlerThread("reader").apply { start() }
    private val usage = HardwareBuffer.USAGE_GPU_COLOR_OUTPUT or HardwareBuffer.USAGE_CPU_READ_OFTEN
    private val reader = ImageReader.newInstance(SIDE, SIDE, PixelFormat.RGBA_8888, 3, usage)
    private val painter = Painter(false, Videos(context), Tilt(blind(context)), Hearing(context))

    @Before
    fun attach() {
        choose(CHOSEN)
        reader.setOnImageAvailableListener({ source ->
            generateSequence { runCatching { source.acquireNextImage() }.getOrNull() }.forEach { image ->
                image.use {
                    val plane = it.planes[0]
                    val at = SIDE / 2 * plane.rowStride + SIDE / 2 * plane.pixelStride
                    val color = { from: Int -> (0..2).map { i -> plane.buffer.get(from + i).toInt() and 0xff }.let { (r, g, b) -> Color.rgb(r, g, b) } }
                    edges += color(2 * plane.rowStride + SIDE / 2 * plane.pixelStride)
                    seen += color(at).also(pixel::set)
                    frames.incrementAndGet()
                }
            }
        }, Handler(thread.looper))
        Render.call {
            painter.attach(reader.surface)
            painter.resize(SIDE, SIDE, 0)
        }
    }

    @After
    fun detach() {
        Render.call { painter.detach() }
        reader.close()
        thread.quitSafely()
    }

    @Test
    fun drawsTheChosenImage() {
        Render.call { painter.show(true) }
        assertTrue(waitFor { frames.get() > 0 })
        same(CHOSEN, pixel.get())
    }

    @Test
    fun aNewSurfaceShowsAChangedChoiceWithoutFading() {
        Native.setting("transition.kind", "fade")
        Render.call { painter.show(true) }
        assertTrue(waitFor { frames.get() > 0 })
        Render.call { painter.detach() }
        choose(OTHER)
        val start = seen.size
        Render.call {
            painter.attach(reader.surface)
            painter.resize(SIDE, SIDE, 0)
            painter.show(true)
        }
        assertTrue(waitFor { seen.size > start })
        same(OTHER, seen[start])
    }

    @Test
    fun heicAndAvifPhotosDraw() = listOf("blue-64x32.heic", "blue-64x32.avif").forEach { name ->
        assertEquals(name, IMAGE, install(fixture(name)))
        val start = seen.size
        Render.call {
            Wallpaper.reload(false)
            painter.show(true)
            painter.wake()
        }
        assertTrue(name, waitFor { seen.size > start && near(OTHER, seen.last(), LOSSY) })
    }

    @Test
    fun wholePictureVideosKeepTheirBars() {
        Native.setting("fit", "fit")
        try {
            assertEquals(VIDEO, install(fixture(CLIP)))
            Render.call {
                Wallpaper.reload(false)
                painter.refresh()
                painter.show(true)
            }
            assertTrue(waitFor { seen.isNotEmpty() && near(MAGENTA, seen.last(), LOSSY) })
            assertTrue(near(Color.BLACK, edges.last(), LOSSY))
        } finally {
            Native.setting("fit", "fill")
        }
    }

    @Test
    fun videosFadeInOverPictures() {
        Native.setting("transition.kind", "fade")
        Render.call { painter.show(true) }
        assertTrue(waitFor { seen.isNotEmpty() && near(CHOSEN, seen.last(), TOLERANCE.toInt()) })
        assertEquals(VIDEO, install(fixture(CLIP)))
        Render.call {
            Wallpaper.reload(false)
            painter.refresh()
        }
        assertTrue("a frame mixes the picture and the video", waitFor { seen.any { (it and 0xff) in 0x48..0xd0 } })
    }

    private fun near(expected: Int, drawn: Int, tolerance: Int) =
        listOf(16, 8, 0).all { shift -> kotlin.math.abs((expected shr shift and 0xff) - (drawn shr shift and 0xff)) <= tolerance }

    private fun same(expected: Int, drawn: Int) = listOf(16, 8, 0).forEach { shift ->
        assertEquals((expected shr shift and 0xff).toFloat(), (drawn shr shift and 0xff).toFloat(), TOLERANCE)
    }

    @Test
    fun staticImagesDrawNothingMore() {
        Render.call { painter.show(true) }
        assertTrue(waitFor { frames.get() > 0 })
        assertEquals(settled(), quiet())
    }

    @Test
    fun hiddenWallpapersStopDrawing() {
        Render.call { painter.show(true) }
        assertTrue(waitFor { frames.get() > 0 })
        choose(Color.rgb(20, 60, 180))
        Render.call {
            Wallpaper.reload(false)
            painter.wake()
        }
        assertTrue(waitFor { frames.get() > 1 })
        Render.call { painter.show(false) }
        assertEquals(settled(), quiet())
        Render.call { painter.offset(0f) }
        assertEquals(settled(), quiet())
    }

    private fun choose(color: Int) = assertEquals(IMAGE, install(still(color)))

    private fun settled() = settled(frames)

    private fun quiet() = quiet(frames)

    private companion object {
        const val SIDE = 64
        const val CHOSEN = 0xffc86428.toInt()
        const val OTHER = 0xff2864c8.toInt()
        const val MAGENTA = 0xffff00ff.toInt()
        const val CLIP = "magenta-64x32.webm"
        const val TOLERANCE = 3f
        const val LOSSY = 16
    }
}
