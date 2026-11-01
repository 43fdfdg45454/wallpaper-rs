package io.github.wallpaper_rs

import android.Manifest
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
class SceneTest : SceneBase() {
    @Test
    fun animatedScenesDrawUntilHidden() {
        show("Snowfall")
        assertTrue(waitFor { frames.get() > FRAMES })
        Render.call { painter.show(false) }
        assertEquals(settled(frames), quiet(frames))
    }

    @Test
    fun theFlashGuardIsOffUntilChosen() {
        assertEquals("false", Native.setting("playback.flash_guard", null))
        assertEquals("true", Native.setting("playback.flash_guard", "true"))
        show("Pulse")
        assertTrue(waitFor { frames.get() > 0 })
        assertEquals("false", Native.setting("playback.flash_guard", "false"))
    }

    @Test
    fun soundScenesRestWhileTheMicrophoneHearsNothing() = runBlocking {
        val automation = InstrumentationRegistry.getInstrumentation().uiAutomation
        automation.grantRuntimePermission(context.packageName, Manifest.permission.RECORD_AUDIO)
        val uid = show("Pulse")
        try {
            ask("grant", "uid" to uid, "input" to "audio", "on" to true)
            Render.call { painter.refresh() }
            assertTrue(waitFor { frames.get() > 0 })
            assertEquals(settled(frames), quiet(frames))
        } finally {
            ask("grant", "uid" to uid, "input" to "audio", "on" to false)
        }
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
    fun videoRangesLoopWithinTheirPart() {
        val dir = File(storage(), "parts/part").apply { mkdirs() }
        fixture("magenta-64x32.webm").copyTo(File(dir, "clip.webm"), true)
        File(dir, "scene.json").writeText(PART_SCENE)
        File(dir, "manifest.json").writeText(PART_MANIFEST)
        val reply = Native.library(JSONObject().put("op", "examples").put("dir", dir.parent).toString())
        assertTrue(reply, reply.startsWith("["))
        show("Part")
        assertTrue(waitFor { frames.get() > FRAMES })
        val seen = (0 until 40).map { Thread.sleep(50).let { Render.call { videos.positions().values.maxOrNull() ?: -1L } } }
        assertTrue("$seen", seen.count { it > 400 } <= 2 && seen.distinct().size > 2)
    }

    @Test
    fun scenesThatMixLayersWithTheNewerModesAndEffectsDraw() {
        val dir = File(storage(), "mixes/mixes").apply { mkdirs() }
        File(dir, "scene.json").writeText(MIXES_SCENE)
        File(dir, "manifest.json").writeText(MIXES_MANIFEST)
        val reply = Native.library(JSONObject().put("op", "examples").put("dir", dir.parent).toString())
        assertTrue(reply, reply.startsWith("["))
        show("Mixes")
        assertTrue(waitFor { frames.get() > 0 })
        assertEquals(settled(frames), quiet(frames))
    }

    @Test
    fun cameraTiltCaptureBloomStyledTextsAndKeptFramesDrawAndSettle() {
        val dir = File(storage(), "leans/leans").apply { mkdirs() }
        File(dir, "scene.json").writeText(LEANS_SCENE)
        File(dir, "manifest.json").writeText(LEANS_MANIFEST)
        val reply = Native.library(JSONObject().put("op", "examples").put("dir", dir.parent).toString())
        assertTrue(reply, reply.startsWith("["))
        show("Leans")
        assertTrue(waitFor { frames.get() > 0 })
        assertEquals(settled(frames), quiet(frames))
    }

    @Test
    fun meshLayersBendTheirBonesAndSettleOnceTheirClipEnds() {
        val dir = File(storage(), "rigs/rig").apply { mkdirs() }
        File(dir, "scene.json").writeText(RIG_SCENE)
        File(dir, "rig.glb").writeBytes(java.util.Base64.getDecoder().decode(RIG_GLB))
        File(dir, "manifest.json").writeText(RIG_MANIFEST)
        val reply = Native.library(JSONObject().put("op", "examples").put("dir", dir.parent).toString())
        assertTrue(reply, reply.startsWith("["))
        show("Rig")
        assertTrue("the clip did not move the mesh", waitFor { frames.get() > 3 })
        Thread.sleep(700)
        assertEquals(settled(frames), quiet(frames))
    }

    @Test
    fun bonesWithPhysicsAndChainsDrawAndSettle() {
        val dir = File(storage(), "rigs/swing").apply { mkdirs() }
        File(dir, "scene.json").writeText(SWING_SCENE)
        File(dir, "rig.glb").writeBytes(java.util.Base64.getDecoder().decode(RIG_GLB))
        File(dir, "manifest.json").writeText(SWING_MANIFEST)
        val reply = Native.library(JSONObject().put("op", "examples").put("dir", dir.parent).toString())
        assertTrue(reply, reply.startsWith("["))
        show("Swing")
        assertTrue("the mesh was not drawn", waitFor { frames.get() > 0 })
        assertTrue("the bones did not settle", waitFor { frames.get().let { before -> quiet(frames) == before } })
    }

    @Test
    fun particlesWithTrailsOrdersGradientsAndSoundRangesDrawAndTellTheirPath() {
        val dir = File(storage(), "rains/rain").apply { mkdirs() }
        File(dir, "scene.json").writeText(RAIN_SCENE)
        File(dir, "manifest.json").writeText(RAIN_MANIFEST)
        val reply = Native.library(JSONObject().put("op", "examples").put("dir", dir.parent).toString())
        assertTrue(reply, reply.startsWith("["))
        show("Rain")
        assertTrue(waitFor { frames.get() > FRAMES })
        assertTrue(JSONObject(Native.diagnostics()).optString("particles") in listOf("gpu", "cpu"))
    }

    @Test
    fun scriptsRunAndTapsReachTheScene() = runBlocking {
        val dir = File(storage(), "taps/taps").apply { mkdirs() }
        File(dir, "scene.json").writeText(TAPS_SCENE)
        File(dir, "tap.js").writeText(TAPS_SCRIPT)
        File(dir, "manifest.json").writeText(TAPS_MANIFEST)
        val reply = Native.library(JSONObject().put("op", "examples").put("dir", dir.parent).toString())
        assertTrue(reply, reply.startsWith("["))
        val uid = show("Taps")
        assertTrue(waitFor { frames.get() > 0 })
        val form = ask("scene", "uid" to uid, "languages" to listOf("en")) as JSONObject
        assertTrue(form.getJSONObject("uses").getBoolean("taps") && !form.getJSONObject("granted").getBoolean("taps"))
        ask("grant", "uid" to uid, "input" to "taps", "on" to true)
        Render.call { painter.refresh() }
        settled(frames)
        val red = { lamp.get() shr 8 }
        val green = { lamp.get() and 0xff }
        tap(SIDE * 3 / 4f)
        quiet(frames)
        assertTrue("a tap beside the button lit the lamp: ${lamp.get()}", red() < 64)
        tap(SIDE / 4f)
        assertTrue("the tap did not reach the script and the timeline: ${lamp.get()}", waitFor { red() > 192 && green() < 64 })
        assertEquals("", JSONObject(Native.diagnostics()).optString("scripts"))
    }
}
