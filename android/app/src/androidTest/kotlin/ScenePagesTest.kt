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
class ScenePagesTest : SceneBase() {
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
        assertTrue(Item(ask("item", "uid" to uid) as JSONObject).lacks.isEmpty())
    }

    @Test
    fun webPagesAreAddedAndShownAsPages() = runBlocking {
        val dir = File(storage(), "pages/page").apply { mkdirs() }
        File(dir, "index.html").writeText("<!doctype html><title>Page</title>")
        File(dir, "manifest.json").writeText(PAGE_MANIFEST)
        val uid = JSONArray(Native.library(JSONObject().put("op", "examples").put("dir", dir.parent).toString())).getString(0)
        val item = Item(ask("item", "uid" to uid) as JSONObject)
        assertTrue(item.web && item.scene && item.lacks.isEmpty())
        assertEquals(listOf("zone", "language", "gpu"), item.exposes)
        assertTrue(context.notice(item.exposes)!!.contains("your time zone, your language and the model of your graphics chip"))
        ask("remove", "uids" to listOf(uid))
        val webs = JSONArray(Native.library(JSONObject().put("op", "items").put("query", "kind:web").toString()))
        val lights = (0 until webs.length()).map(webs::getJSONObject).first { it.getString("title") == "Lights" }
        assertEquals(listOf("zone", "language", "gpu", "storage"), Item(lights).exposes)
    }

    @Test
    fun scenesWithLightsAreRefusedUntilTheEngineDrawsThem() {
        val dir = File(storage(), "lamps/lamp").apply { mkdirs() }
        File(dir, "scene.json").writeText(LAMP_SCENE)
        File(dir, "manifest.json").writeText(LAMP_MANIFEST)
        val reply = JSONObject(Native.library(JSONObject().put("op", "examples").put("dir", dir.parent).toString()))
        assertTrue(reply.getString("error").contains("it uses the feature lights (lights its layers)"))
    }

    @Test
    fun scenesWithModelsAreRefusedUntilTheEngineDrawsThem() {
        val dir = File(storage(), "stages/stage").apply { mkdirs() }
        File(dir, "scene.json").writeText(STAGE_SCENE)
        File(dir, "manifest.json").writeText(STAGE_MANIFEST)
        val reply = JSONObject(Native.library(JSONObject().put("op", "examples").put("dir", dir.parent).toString()))
        assertTrue(reply.getString("error").contains("camera3d (moves a 3D camera) and model3d (shows 3D models)"))
    }

    @Test
    fun scenesWithTheNewerEffectsAndParametersAreAccepted() {
        val dir = File(storage(), "shafts/shafts").apply { mkdirs() }
        File(dir, "scene.json").writeText(SHAFTS_SCENE)
        File(dir, "manifest.json").writeText(SHAFTS_MANIFEST)
        val reply = Native.library(JSONObject().put("op", "examples").put("dir", dir.parent).toString())
        assertTrue(reply, reply.startsWith("["))
    }
}
