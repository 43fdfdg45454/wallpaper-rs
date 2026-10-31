package io.github.wallpaper_rs

import android.content.Context
import android.content.ContextWrapper
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.PixelFormat
import android.hardware.HardwareBuffer
import android.hardware.Sensor
import android.media.ImageReader
import android.os.Handler
import android.os.HandlerThread
import android.os.ParcelFileDescriptor
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicIntegerArray
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.sin
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

fun blind(context: Context): Context = object : ContextWrapper(context) {
    override fun getSystemService(name: String): Any? = if (name == Context.SENSOR_SERVICE) null else super.getSystemService(name)
}

@RunWith(AndroidJUnit4::class)
class MotionTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val frames = AtomicInteger()
    private val reds = AtomicIntegerArray(3)
    private val thread = HandlerThread("motion").apply { start() }
    private val usage = HardwareBuffer.USAGE_GPU_COLOR_OUTPUT or HardwareBuffer.USAGE_CPU_READ_OFTEN
    private val reader = ImageReader.newInstance(SIDE, SIDE, PixelFormat.RGBA_8888, 3, usage)
    private val tilt = Tilt(blind(context))
    private val painter = Painter(false, Videos(context), tilt, Hearing(context))
    private var uid = ""

    @Before
    fun attach() {
        assertEquals(IMAGE, install(gradient()))
        uid = JSONObject(Native.library(JSONObject().put("op", "current").toString())).getString("image")
        reader.setOnImageAvailableListener({ source ->
            source.acquireLatestImage()?.use { image ->
                val plane = image.planes[0]
                (0..2).forEach { reds.set(it, plane.buffer.get(SIDE / 2 * plane.rowStride + SIDE * (it + 1) / 4 * plane.pixelStride).toInt() and 0xff) }
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
        Native.library(JSONObject(mapOf("op" to "remove", "uids" to listOf(uid))).toString())
        reader.close()
        thread.quitSafely()
    }

    @Test
    fun turningThePhoneMovesTheParallaxPictureOnlyWhileShown() = moves(Sensor.TYPE_GAME_ROTATION_VECTOR) { angle ->
        floatArrayOf(0f, sin(angle / 2), 0f, cos(angle / 2))
    }

    @Test
    fun tiltingAPhoneWithoutGyroscopeMovesItToo() = moves(Sensor.TYPE_ACCELEROMETER) { angle ->
        floatArrayOf(-9.8f * sin(angle), 0f, 9.8f * cos(angle))
    }

    @Test
    fun nearThingsMoveMoreThanTheBackground() {
        val map = File(storage(), "halves.png").apply {
            val pixels = IntArray(SIDE * SIDE) { if (it % SIDE < SIDE / 2) Color.WHITE else Color.BLACK }
            outputStream().use { Bitmap.createBitmap(pixels, SIDE, SIDE, Bitmap.Config.ARGB_8888).compress(Bitmap.CompressFormat.PNG, 100, it) }
        }
        ParcelFileDescriptor.open(map, ParcelFileDescriptor.MODE_READ_ONLY).use { depth("import", "uid" to uid, "fd" to it.fd) }
        assertTrue((depth("item", "uid" to uid) as JSONObject).getBoolean("map"))
        val (near, far) = moved(Sensor.TYPE_GAME_ROTATION_VECTOR) { angle -> floatArrayOf(0f, sin(angle / 2), 0f, cos(angle / 2)) }
        assertTrue("near moved $near and far $far", near * far < 0 && abs(near) > abs(far))
    }

    @Test
    fun framingMovesAndZoomsThePicture() {
        Render.call { painter.show(true) }
        assertTrue(waitFor { frames.get() > 0 })
        val (still, before) = settled(frames) to reds.get(1)
        val pan = floatArrayOf(0.25f, 0f)
        depth("frame", "uid" to uid, "pan" to pan, "zoom" to 2f, "screen" to intArrayOf(SIDE, SIDE), "done" to true)
        Render.call { painter.wake() }
        assertTrue(waitFor { frames.get() > still && abs(reds.get(1) - before) >= MOVED })
        depth("unframe", "uid" to uid)
        Render.call { painter.wake() }
        assertTrue(waitFor { abs(reds.get(1) - before) <= TOLERANCE })
    }

    private fun moves(type: Int, reading: (Float) -> FloatArray) {
        val (near, far) = moved(type, reading)
        assertTrue("the whole picture moved $near and $far", abs(near) >= MOVED && abs(far) >= MOVED)
        Thread.sleep(2000)
        assertEquals("a still phone draws nothing", settled(frames), quiet(frames))
        Render.call { painter.show(false) }
        val hidden = settled(frames)
        hold(type, reading(-0.2f), 300)
        assertEquals(hidden, quiet(frames))
    }

    private fun moved(type: Int, reading: (Float) -> FloatArray): Pair<Int, Int> {
        depth("parallax", "uid" to uid, "on" to true)
        Render.call {
            painter.refresh()
            painter.show(true)
        }
        assertTrue(waitFor { frames.get() > 0 })
        hold(type, reading(0f), 300)
        val (still, before) = settled(frames) to IntArray(3, reds::get)
        (1..10).forEach { hold(type, reading(0.02f * it), 20) }
        assertTrue(waitFor { frames.get() > still })
        val turned = waitFor {
            hold(type, reading(0.2f), 100)
            abs(reds.get(1) - before[1]) >= MOVED
        }
        assertTrue("red went from ${before[1]} to ${reds.get(1)}", turned)
        assertTrue("the picture kept moving", waitFor { frames.get().let { seen -> settled(frames) == seen } })
        return reds.get(0) - before[0] to reds.get(2) - before[2]
    }

    private fun hold(type: Int, values: FloatArray, millis: Long) = repeat((millis / STEP_MS).toInt().coerceAtLeast(1)) {
        Render.call { tilt.sense(type, values) }
        Thread.sleep(STEP_MS)
    }

    private fun gradient(): File = File(storage(), "gradient.png").apply {
        val pixels = IntArray(SIDE * SIDE) { Color.rgb(it % SIDE * 4, 0, 0) }
        outputStream().use { Bitmap.createBitmap(pixels, SIDE, SIDE, Bitmap.Config.ARGB_8888).compress(Bitmap.CompressFormat.PNG, 100, it) }
    }

    private companion object {
        const val SIDE = 64
        const val MOVED = 4
        const val TOLERANCE = 3
        const val STEP_MS = 20L
    }
}
