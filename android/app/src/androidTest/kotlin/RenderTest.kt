package io.github.wallpaper_rs

import android.content.Context
import android.content.ContextWrapper
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.PixelFormat
import android.hardware.HardwareBuffer
import android.media.ImageReader
import android.os.Handler
import android.os.HandlerThread
import android.os.ParcelFileDescriptor
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.util.concurrent.atomic.AtomicInteger
import org.json.JSONObject
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
    private val thread = HandlerThread("reader").apply { start() }
    private val usage = HardwareBuffer.USAGE_GPU_COLOR_OUTPUT or HardwareBuffer.USAGE_CPU_READ_OFTEN
    private val reader = ImageReader.newInstance(SIDE, SIDE, PixelFormat.RGBA_8888, 3, usage)
    private val blind = object : ContextWrapper(context) {
        override fun getSystemService(name: String): Any? =
            if (name == Context.SENSOR_SERVICE) null else super.getSystemService(name)
    }
    private val painter = Painter(false, Videos(context), Tilt(blind), Hearing(context))

    @Before
    fun attach() {
        choose(CHOSEN)
        reader.setOnImageAvailableListener({ source ->
            source.acquireLatestImage()?.use { image ->
                val plane = image.planes[0]
                val at = SIDE / 2 * plane.rowStride + SIDE / 2 * plane.pixelStride
                val (r, g, b) = (0..2).map { plane.buffer.get(at + it).toInt() and 0xff }
                pixel.set(Color.rgb(r, g, b))
                frames.incrementAndGet()
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
        val drawn = pixel.get()
        listOf(16, 8, 0).forEach { shift ->
            assertEquals((CHOSEN shr shift and 0xff).toFloat(), (drawn shr shift and 0xff).toFloat(), TOLERANCE)
        }
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

    @Test
    fun tiltMovesParallaxWallpapersOnlyWhileShown() {
        val uid = JSONObject(Native.library(JSONObject().put("op", "current").toString())).getString("image")
        depth("parallax", "uid" to uid, "on" to true)
        Render.call {
            painter.refresh()
            painter.show(true)
        }
        assertTrue(waitFor { frames.get() > 0 })
        val still = settled()
        lean(0.6f)
        assertTrue(waitFor { frames.get() > still })
        Thread.sleep(2000)
        assertEquals(settled(), quiet())
        Render.call { painter.show(false) }
        val hidden = settled()
        lean(-0.6f)
        assertEquals(hidden, quiet())
        depth("parallax", "uid" to uid, "on" to false)
    }

    private fun lean(side: Float) = (1..10).forEach { step ->
        val x = side * step / 10
        Render.call { if (Native.tilt(x, 0.8f, 0.6f, 0)) painter.move() }
        Thread.sleep(20)
    }

    private fun choose(color: Int) = assertEquals(IMAGE, install(still(color)))

    private fun settled() = settled(frames)

    private fun quiet() = quiet(frames)

    private companion object {
        const val SIDE = 64
        const val CHOSEN = 0xffc86428.toInt()
        const val TOLERANCE = 3f
    }
}

const val IMAGE = "image"
const val VIDEO = "video"

fun storage(): File = InstrumentationRegistry.getInstrumentation().targetContext.cacheDir.resolve("render-test")
    .apply { mkdirs() }
    .also { Native.prepare(it.path) }

fun still(color: Int): File = File(storage(), "still.png").apply {
    outputStream().use { out ->
        Bitmap.createBitmap(8, 8, Bitmap.Config.ARGB_8888).apply { eraseColor(color) }
            .compress(Bitmap.CompressFormat.PNG, 100, out)
    }
}

fun fixture(name: String): File = File(storage(), name).apply {
    InstrumentationRegistry.getInstrumentation().context.assets.open(name).use { input ->
        outputStream().use { input.copyTo(it) }
    }
}

fun install(file: File): String {
    val item = ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
        .use { JSONObject(Native.store(it.fd, file.name)!!) }
    Native.library(JSONObject(mapOf("op" to "pick", "choice" to choice("item", item.getString("uid")))).toString())
    Render.call { Native.commit() }
    return item.getString("kind")
}

fun settled(frames: AtomicInteger) = Thread.sleep(300).let { frames.get() }

fun quiet(frames: AtomicInteger) = Thread.sleep(1500).let { frames.get() }

fun waitFor(done: () -> Boolean): Boolean {
    val end = System.currentTimeMillis() + 20_000
    while (!done() && System.currentTimeMillis() < end) Thread.sleep(20)
    return done()
}
