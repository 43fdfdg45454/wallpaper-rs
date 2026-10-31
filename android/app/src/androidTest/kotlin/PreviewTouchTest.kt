package io.github.wallpaper_rs

import android.os.SystemClock
import android.view.MotionEvent
import androidx.test.ext.junit.runners.AndroidJUnit4
import java.io.File
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class PreviewTouchTest : SceneBase() {
    private fun press(feel: Feel, action: Int, x: Float): Boolean {
        val at = SystemClock.uptimeMillis()
        val event = MotionEvent.obtain(at, at, action, x, SIDE / 2f, 0)
        return feel.onTouch(null, event).also { event.recycle() }
    }

    @Test
    fun touchesOnThePreviewReachTheSceneOnlyWhenItTakesThem() = runBlocking {
        val dir = File(storage(), "preview-taps/taps").apply { mkdirs() }
        File(dir, "scene.json").writeText(TAPS_SCENE)
        File(dir, "tap.js").writeText(TAPS_SCRIPT)
        val own = TAPS_MANIFEST.replace("org.example.taps", "org.example.preview-taps").replace("\"Taps\"", "\"$TITLE\"")
        File(dir, "manifest.json").writeText(own)
        Native.library(JSONObject().put("op", "examples").put("dir", dir.parent).toString())
        val uid = show(TITLE)
        assertTrue(waitFor { frames.get() > 0 })
        val feel = Feel().apply { painter = this@PreviewTouchTest.painter }
        assertFalse("a preview without permission keeps its touches for the page", press(feel, MotionEvent.ACTION_DOWN, SIDE / 4f))
        ask("grant", "uid" to uid, "input" to "taps", "on" to true)
        Render.call { painter.refresh() }
        settled(frames)
        feel.on = true
        val red = { lamp.get() shr 8 }
        val before = frames.get()
        assertTrue(press(feel, MotionEvent.ACTION_DOWN, SIDE / 4f))
        assertTrue(waitFor { frames.get() > before })
        assertTrue(press(feel, MotionEvent.ACTION_UP, SIDE / 4f))
        assertTrue("the touch on the preview did not reach the scene: ${lamp.get()}", waitFor { red() > 192 })
    }

    private companion object {
        const val TITLE = "Preview taps"
    }
}
