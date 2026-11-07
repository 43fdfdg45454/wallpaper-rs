package io.github.wallpaper_rs

import android.media.AudioManager
import androidx.test.ext.junit.runners.AndroidJUnit4
import java.io.File
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class SoundTest : SceneBase() {
    private val audio = context.getSystemService(AudioManager::class.java)
    private val kept = Native.setting(MUTED, null)

    @After
    fun restore() {
        Native.setting(MUTED, kept)
    }

    private fun output() = JSONObject(Native.diagnostics()).getJSONObject("sound")

    @Test
    fun aSceneWithSoundLayersOpensItsAudioSessionWhileShownAndReleasesIt() {
        Native.setting(MUTED, "true")
        val (opened, released) = output().getInt("opened") to output().getInt("released")
        show("Snowfall")
        assertTrue(waitFor { frames.get() > FRAMES })
        assertEquals("scenes start muted", opened, output().getInt("opened"))
        Native.setting(MUTED, "false")
        assertTrue("the sound opens an audio session", waitFor { output().getInt("session") > 0 })
        assertTrue("and plays", waitFor { audio.activePlaybackConfigurations.isNotEmpty() })
        assertTrue("the codecs of the phone decode its sound", waitFor { output().getInt("decoded") > 0 })
        assertEquals(0, output().getInt("undecodable"))
        assertEquals(opened + 1, output().getInt("opened"))
        Render.call { painter.show(false) }
        assertTrue("hiding the wallpaper releases it", waitFor { output().getInt("session") == -1 })
        assertEquals(released + 1, output().getInt("released"))
        assertTrue("nothing plays any more", waitFor { audio.activePlaybackConfigurations.isEmpty() })
    }

    @Test
    fun theLockScreenStaysSilent() {
        Native.setting(MUTED, "false")
        val opened = output().getInt("opened")
        Render.call { painter.screen(true) }
        show("Snowfall")
        assertTrue(waitFor { frames.get() > FRAMES })
        Thread.sleep(SETTLE_MS)
        assertEquals(opened, output().getInt("opened"))
    }

    @Test
    fun aStreamedSoundIsReadAsItPlaysEvenInAStillScene() {
        Native.setting(MUTED, "false")
        val dir = File(storage(), "streams/stream").apply { mkdirs() }
        context.assets.open("scenes/snowfall/assets/wind.ogg").use { wind -> File(dir, "wind.ogg").outputStream().use(wind::copyTo) }
        File(dir, "scene.json").writeText(STREAM_SCENE)
        File(dir, "manifest.json").writeText(STREAM_MANIFEST)
        Native.library(JSONObject().put("op", "examples").put("dir", dir.parent).toString())
        val before = output().getInt("streamed")
        show("Stream")
        assertTrue(waitFor { frames.get() > 0 })
        assertTrue("the stream opens its session", waitFor { output().getInt("session") > 0 })
        assertTrue("and its blocks arrive", waitFor { output().getInt("streamed") > before })
        assertEquals(0, output().getInt("undecodable"))
    }

    private companion object {
        const val MUTED = "playback.muted"
        const val SETTLE_MS = 1_000L
        const val STREAM_SCENE = """{ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [64, 64] },
            "assets": { "wind": { "path": "wind.ogg" } },
            "layers": [{ "id": "fill", "type": "solid", "color": "#204080" },
                       { "id": "gusts", "type": "sound", "sound": "wind", "stream": true }] }"""
        const val STREAM_MANIFEST = """{ "schema": "wallpaper-rs.wpak/2", "id": "org.example.stream", "version": "1.0.0",
            "title": "Stream", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "origin": { "kind": "original" },
            "content": { "rating": "general" }, "photosensitive": false, "requires": { "features": ["sound", "sound2"] } }"""
    }
}
