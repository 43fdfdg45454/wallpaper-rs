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

    @Test
    fun ratedScenesWaitForConsent() = runBlocking {
        val dir = File(storage(), "rated/flash").apply { mkdirs() }
        val id = "org.example.flash${System.currentTimeMillis()}"
        File(dir, "scene.json").writeText(FLASH_SCENE)
        File(dir, "manifest.json").writeText(FLASH_MANIFEST.replace("org.example.flash", id))
        Native.library(JSONObject().put("op", "examples").put("dir", dir.parent).toString())
        val uid = find("Flash")
        suspend fun item() = Item(ask("item", "uid" to uid, "languages" to listOf("es")) as JSONObject)
        assertTrue(item().mature && item().flashing && item().about == "Destellos rápidos.")
        assertTrue(item().title == "Destello" && item().lacks.isEmpty())
        val pick = JSONObject(mapOf("op" to "pick", "choice" to choice("item", uid))).toString()
        assertTrue(JSONObject(Native.library(pick)).getString("error").contains("photosensitive"))
        ask("confirm", "uid" to uid, "what" to "photosensitive")
        assertTrue(item().mature && !item().flashing)
        ask("confirm", "uid" to uid, "what" to "mature")
        assertTrue(!item().mature)
        assertEquals("null", Native.library(pick))
    }

    @Test
    fun buttonsRedrawAndSettingsTravelAsFiles() = runBlocking {
        val dir = File(storage(), "knobs/knobs").apply { mkdirs() }
        File(dir, "scene.json").writeText(KNOBS_SCENE)
        File(dir, "manifest.json").writeText(KNOBS_MANIFEST)
        Native.library(JSONObject().put("op", "examples").put("dir", dir.parent).toString())
        val uid = show("Knobs")
        assertTrue(waitFor { frames.get() > 0 })
        val before = settled(frames)
        ask("press", "uid" to uid, "key" to "again")
        Render.post(painter::refresh)
        assertTrue(quiet(frames) > before)
        ask("secret", "uid" to uid, "key" to "token", "text" to "hunter2")
        ask("property", "uid" to uid, "key" to "mood", "value" to 2)
        ask("save", "uid" to uid, "name" to "Storm")
        val text = ask("export", "uid" to uid, "preset" to "Storm") as String
        assertTrue(!text.contains("hunter2") && JSONObject(text).getJSONObject("values").getInt("mood") == 2)
        ask("reset", "uid" to uid)
        assertEquals(0, (ask("import", "uid" to uid, "text" to text, "name" to "") as JSONArray).length())
        val form = ask("scene", "uid" to uid, "languages" to listOf("en")) as JSONObject
        val fields = form.getJSONArray("fields").let { list -> (0 until list.length()).map(list::getJSONObject) }
        assertTrue(fields.first { it.getString("key") == "token" }.getBoolean("filled"))
        assertEquals(1, form.getJSONArray("saved").length())
        assertEquals(0, (ask("import", "uid" to uid, "text" to text, "name" to "Copy") as JSONArray).length())
        val named = (ask("scene", "uid" to uid, "languages" to listOf("en")) as JSONObject).getJSONArray("saved")
        assertEquals(listOf("Copy", "Storm"), (0 until named.length()).map { named.getJSONObject(it).getString("label") }.sorted())
        assertEquals(listOf("folder"), Item(ask("item", "uid" to uid) as JSONObject).lacks)
    }

    private fun find(title: String): String {
        val query = JSONObject().put("op", "items").put("query", "kind:scene")
        val items = JSONArray(Native.library(query.toString()))
        return (0 until items.length()).map(items::getJSONObject).first { it.getString("title") == title }.getString("uid")
    }

    private fun show(title: String): String {
        val uid = find(title)
        Native.library(JSONObject().put("op", "confirm").put("uid", uid).put("what", "photosensitive").toString())
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
        const val KNOBS_SCENE = """{ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [64, 32] },
            "timelines": [{ "id": "glow", "duration": 1, "autoplay": false, "trigger": "prop:again" }],
            "layers": [{ "id": "fill", "type": "solid", "color": "#ff0000", "opacity": { "timeline": "glow", "keys": [[0, 0], [1, 1]] } }] }"""
        const val KNOBS_MANIFEST = """{ "schema": "wallpaper-rs.wpak/2", "id": "org.example.knobs", "version": "1.0.0",
            "title": "Knobs", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "origin": { "kind": "original" },
            "content": { "rating": "general" }, "photosensitive": false, "requires": { "features": ["folder", "properties2"] },
            "properties": { "mood": { "type": "combo", "default": 1, "options": [1, 2] }, "again": { "type": "button" },
                            "token": { "type": "text", "secret": true }, "photos": { "type": "folder" } } }"""
        const val FLASH_SCENE = """{ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [64, 32] },
            "layers": [{ "id": "fill", "type": "solid", "color": "#ffffff" }] }"""
        const val FLASH_MANIFEST = """{ "schema": "wallpaper-rs.wpak/2", "id": "org.example.flash", "version": "1.0.0",
            "default_locale": "en", "title": { "en": "Flash", "es": "Destello" }, "description": { "en": "Fast **flashes**.", "es": "Destellos **rápidos**." },
            "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "origin": { "kind": "original" },
            "content": { "rating": "mature", "descriptors": ["violence"] }, "photosensitive": true }"""
    }
}
