package io.github.wallpaper_rs

import android.content.Intent
import android.content.IntentFilter
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ShortcutTest : SceneBase() {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()

    @After
    fun forgotten() = Shortcuts.forget()

    private fun tapped(target: JSONObject) = runBlocking {
        val dir = File(storage(), "shortcuts/shortcuts").apply { mkdirs() }
        File(dir, "scene.json").writeText(SHORTCUT_SCENE)
        File(dir, "open.js").writeText(SHORTCUT_SCRIPT)
        File(dir, "manifest.json").writeText(SHORTCUT_MANIFEST)
        Native.library(JSONObject().put("op", "examples").put("dir", dir.parent).toString())
        painter.opened = { Shortcuts.open(context, it) }
        val uid = show("Shortcuts")
        assertTrue(waitFor { frames.get() > 0 })
        ask("grant", "uid" to uid, "input" to "taps", "on" to true)
        ask("property", "uid" to uid, "key" to "open", "value" to target)
        Render.call { painter.refresh() }
        settled(frames)
        Thread.sleep(APART_MS)
        tap(SIDE / 4f)
    }

    @Test
    fun aTapOnTheWallpaperOpensTheWebAddressTheUserChose() {
        val filter = IntentFilter(Intent.ACTION_VIEW).apply { addDataScheme("https"); addCategory(Intent.CATEGORY_BROWSABLE) }
        val monitor = instrumentation.addMonitor(filter, null, true)
        try {
            tapped(JSONObject().put("uri", "https://example.org/a"))
            assertTrue("the tap did not reach the browser", waitFor { monitor.hits == 1 })
            assertNull(Shortcuts.missed)
        } finally {
            instrumentation.removeMonitor(monitor)
        }
    }

    @Test
    fun anAppThatIsNotThereIsReportedInsteadOfSwallowed() {
        tapped(JSONObject().put("app", "org.example.absent"))
        assertTrue("nothing was reported", waitFor { Shortcuts.missed != null })
        assertEquals("app" to "org.example.absent", Shortcuts.missed)
    }

    @Test
    fun theFailuresAreToldApartAndAGoodOpeningClearsThem() {
        val filter = IntentFilter(Intent.ACTION_VIEW).apply { addDataScheme("https"); addCategory(Intent.CATEGORY_BROWSABLE) }
        val monitor = instrumentation.addMonitor(filter, null, true)
        try {
            assertTrue(!Shortcuts.open(context, JSONObject().put("app", "org.example.absent")))
            assertEquals("app", Shortcuts.missed?.first)
            assertTrue(Shortcuts.open(context, JSONObject().put("uri", "https://example.org/")))
            assertNull(Shortcuts.missed)
        } finally {
            instrumentation.removeMonitor(monitor)
        }
    }

    private companion object {
        const val APART_MS = 1_100L
        const val SHORTCUT_SCENE = """{ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [64, 64] },
            "scripts": [{ "path": "open.js", "events": ["init", "click"], "uses": ["layers", "clicks", "shortcuts"] }],
            "layers": [{ "id": "button", "type": "solid", "color": "#2040c0", "size": [32, 64], "solid": true,
                        "transform": { "position": [16, 32] } }] }"""
        const val SHORTCUT_SCRIPT = """import { scene, openShortcut } from "wprs";
export function init() { scene.layer("button").on("click", () => { openShortcut("open"); }); }
"""
        const val SHORTCUT_MANIFEST = """{ "schema": "wallpaper-rs.wpak/2", "id": "org.example.shortcuts", "version": "1.0.0",
            "title": "Shortcuts", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "origin": { "kind": "original" },
            "content": { "rating": "general" }, "photosensitive": false, "inputs": ["pointer", "clicks"],
            "requires": { "features": ["properties2", "script_shortcuts"] },
            "properties": { "open": { "type": "shortcut", "label": "Open" } } }"""
    }
}
