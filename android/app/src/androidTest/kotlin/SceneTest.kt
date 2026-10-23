package io.github.wallpaper_rs

import android.graphics.PixelFormat
import android.hardware.HardwareBuffer
import android.media.ImageReader
import android.os.Handler
import android.os.HandlerThread
import androidx.test.ext.junit.runners.AndroidJUnit4
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
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class SceneTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val frames = AtomicInteger()
    private val thread = HandlerThread("scene").apply { start() }
    private val usage = HardwareBuffer.USAGE_GPU_COLOR_OUTPUT or HardwareBuffer.USAGE_CPU_READ_OFTEN
    private val reader = ImageReader.newInstance(SIDE, SIDE, PixelFormat.RGBA_8888, 3, usage)
    private val painter = Painter(false, Videos(context), Tilt(context), Hearing(context))

    @Before
    fun prepare() {
        storage()
        examples(context)
        reader.setOnImageAvailableListener({ source ->
            source.acquireLatestImage()?.close()
            frames.incrementAndGet()
        }, Handler(thread.looper))
    }

    @After
    fun detach() {
        Render.call { painter.detach() }
        reader.close()
        thread.quitSafely()
    }

    @Test
    fun animatedScenesDrawUntilHidden() {
        show("Snowfall")
        assertTrue(waitFor { frames.get() > FRAMES })
        Render.call { painter.show(false) }
        assertEquals(settled(frames), quiet(frames))
    }

    @Test
    fun silentSoundScenesDrawOnce() {
        show("Pulse")
        assertTrue(waitFor { frames.get() > 0 })
        assertEquals(settled(frames), quiet(frames))
    }

    @Test
    fun timedScenesRedrawAtOnceWhenAPropertyChanges() {
        val uid = show("Orbits")
        assertTrue(waitFor { frames.get() > 0 })
        val before = settled(frames)
        val change = JSONObject().put("op", "property").put("uid", uid).put("key", "clock").put("value", false)
        Native.library(change.toString())
        Render.post(painter::refresh)
        assertTrue(quiet(frames) > before)
    }

    @Test
    fun videoLayersKeepDrawing() {
        val dir = File(storage(), "clips/clip").apply { mkdirs() }
        fixture("magenta-64x32.webm").copyTo(File(dir, "clip.webm"), true)
        File(dir, "scene.json").writeText(CLIP_SCENE)
        File(dir, "manifest.json").writeText(CLIP_MANIFEST)
        Native.library(JSONObject().put("op", "examples").put("dir", dir.parent).toString())
        show("Clip")
        assertTrue(waitFor { frames.get() > FRAMES })
        val start = frames.get()
        assertTrue(quiet(frames) > start + FRAMES)
    }

    @Test
    fun pagesGetTheirItemAndProperties() = runBlocking {
        val uid = show("Snowfall")
        assertEquals("Snowfall", Item(ask("item", "uid" to uid) as JSONObject).title)
        assertTrue((ask("scene", "uid" to uid, "languages" to listOf("en")) as JSONObject).has("fields"))
    }

    private fun show(title: String): String {
        val query = JSONObject().put("op", "items").put("query", "kind:scene")
        val items = JSONArray(Native.library(query.toString()))
        val uid = (0 until items.length()).map(items::getJSONObject).first { it.getString("title") == title }.getString("uid")
        Native.library(JSONObject(mapOf("op" to "pick", "choice" to choice("item", uid))).toString())
        Render.call {
            Native.commit()
            painter.attach(reader.surface)
            painter.resize(SIDE, SIDE, 0)
            painter.show(true)
        }
        return uid
    }

    private companion object {
        const val SIDE = 64
        const val FRAMES = 3
        const val CLIP_SCENE = """{ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [64, 32] },
            "assets": { "clip": { "path": "clip.webm" } }, "layers": [{ "id": "clip", "type": "video", "video": "clip" }] }"""
        const val CLIP_MANIFEST = """{ "schema": "wallpaper-rs.wpak/1", "id": "org.example.clip", "version": "1.0.0",
            "title": "Clip", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json" }"""
    }
}
