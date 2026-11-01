package io.github.wallpaper_rs

import android.graphics.PixelFormat
import android.hardware.HardwareBuffer
import android.media.ImageReader
import android.os.Handler
import android.os.HandlerThread
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before

abstract class SceneBase {
    protected val context = InstrumentationRegistry.getInstrumentation().targetContext
    protected val frames = AtomicInteger()
    protected val lamp = AtomicInteger()
    protected val thread = HandlerThread("scene").apply { start() }
    protected val usage = HardwareBuffer.USAGE_GPU_COLOR_OUTPUT or HardwareBuffer.USAGE_CPU_READ_OFTEN
    protected val reader = ImageReader.newInstance(SIDE, SIDE, PixelFormat.RGBA_8888, 3, usage)
    protected val videos = Videos(context)
    protected val painter = Painter(false, videos, Tilt(context), Hearing(context), Pages(context))

    @Before
    fun prepare() {
        storage()
        examples(context)
        Native.library(JSONObject().put("op", "items").put("query", "").put("languages", JSONArray(listOf("en"))).toString())
        reader.setOnImageAvailableListener({ source ->
            source.acquireLatestImage()?.use { image ->
                val plane = image.planes[0]
                val at = SIDE / 2 * plane.rowStride + SIDE * 3 / 4 * plane.pixelStride
                lamp.set((plane.buffer.get(at).toInt() and 0xff shl 8) or (plane.buffer.get(at + 1).toInt() and 0xff))
            }
            frames.incrementAndGet()
        }, Handler(thread.looper))
    }

    @After
    fun detach() {
        Render.call { painter.detach() }
        reader.close()
        thread.quitSafely()
    }

    protected fun tap(x: Float) {
        val before = frames.get()
        Render.call { painter.touch(x, SIDE / 2f, true) }
        assertTrue(waitFor { frames.get() > before })
        Render.call { painter.touch(x, SIDE / 2f, false) }
    }

    protected fun find(title: String): String {
        val query = JSONObject().put("op", "items").put("query", "kind:scene")
        val items = JSONArray(Native.library(query.toString()))
        return (0 until items.length()).map(items::getJSONObject).first { it.getString("title") == title }.getString("uid")
    }

    protected fun show(title: String): String {
        val uid = find(title)
        Native.library(JSONObject().put("op", "confirm").put("uid", uid).put("what", "photosensitive").toString())
        Native.library(JSONObject(mapOf("op" to "pick", "choice" to choice("item", uid))).toString())
        Render.call {
            Native.commit(BOTH)
            painter.attach(reader.surface)
            painter.resize(SIDE, SIDE, 0)
            painter.show(true)
        }
        return uid
    }

    companion object {
        const val SIDE = 64
        const val FRAMES = 3
    }
}
