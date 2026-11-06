package io.github.wallpaper_rs

import android.os.ParcelFileDescriptor
import android.os.SystemClock
import android.view.InputDevice
import android.view.MotionEvent
import androidx.test.ext.junit.runners.AndroidJUnit4
import java.util.TimeZone
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class WebPageTest : PageBase() {
    @Test
    fun pagesSeeTheirValuesAndNothingElse() {
        val seen = until("the probe answers") { probe -> listOf("own", "manifest", "escape", "other", "file", "wpak", "local").all(probe::has) }
        assertEquals("preloaded", seen.getString("early"))
        assertTrue(seen.getString("zone"), seen.getString("zone") in setOf(TimeZone.getDefault().id) + if (TimeZone.getDefault().rawOffset == 0) UTC else emptySet())
        assertEquals(android.content.res.Resources.getSystem().configuration.locales[0].toLanguageTag(), seen.getString("language"))
        assertEquals("undefined", seen.getString("peer"))
        assertTrue(seen.getString("own").startsWith("read "))
        listOf("manifest", "escape", "other", "file", "wpak", "local").forEach { assertEquals(it, "blocked", seen.getString(it)) }
        assertEquals(0, reached.get())
        assertTrue("the page draws its tint", waitFor { near(0xff8000, pixel.get(), 24) })
        runBlocking { ask("property", "uid" to uid, "key" to "tint", "value" to "#0000ff") }
        reload()
        assertTrue("a changed value reaches the page", waitFor { near(0x0000ff, pixel.get(), 24) })
        assertEquals(1, probe()!!.getInt("visits"))
    }

    @Test
    fun hiddenPagesPauseAndDrawAtTheCap() {
        val seen = until("the page counts its frames for a few seconds") { it.getJSONArray("rates").length() >= 4 }
        val cap = seen.getJSONObject("settings").getInt("fps")
        val rates = seen.getJSONArray("rates").let { list -> (1 until list.length()).map(list::getInt) }
        assertTrue("rAF stays under the cap of $cap: $rates", rates.all { it in 1..cap + 3 })
        Render.call { painter.show(false) }
        until("the page is told it is hidden") { it.getJSONArray("paused").let { p -> p.length() > 0 && p.getBoolean(p.length() - 1) } }
        Render.call { painter.show(true) }
        until("the page is told it is shown") { it.getJSONArray("paused").let { p -> !p.getBoolean(p.length() - 1) } }
    }

    @Test
    fun privacyAndStorageArePerPackage() = zoned(ZONE) { runBlocking {
        reload()
        val shown = until("the page sees the zone of the phone") { it.has("worker") && it.optString("zone") == ZONE }
        assertEquals(ZONE, shown.getJSONObject("routes").getString("frame"))
        assertEquals(listOf(ZONE, 21, ZONE, -TOKYO), shown.getJSONArray("worker").let { listOf(it.getString(0), it.getInt(1), it.getString(3), it.getInt(4)) })
        val leaks = shown.getJSONArray("leaks").let { all -> (0..1).map(all::getJSONArray) }
        leaks.forEach { assertEquals(listOf(ZONE, -TOKYO, 0), listOf(it.getString(0), it.getInt(1), it.getInt(4))) }
        listOf("hide_zone", "hide_language", "hide_gpu").forEach { ask("confirm", "uid" to uid, "what" to it) }
        reload()
        val hidden = until("the page loads again hiding what it may not see") { it.optString("zone") == "UTC" && it.has("worker") }
        assertEquals("en-US", hidden.getString("language"))
        assertEquals(12, hidden.getInt("hour"))
        val routes = hidden.getJSONObject("routes")
        listOf("frame", "made").forEach { assertEquals(it, "UTC", routes.getString(it)) }
        listOf("frameHour", "madeHour").forEach { assertEquals(it, 12, routes.getInt(it)) }
        assertEquals("en-US", routes.getString("frameLanguage"))
        assertEquals("undefined", routes.getString("framePeer"))
        assertEquals("[\"UTC\",12,\"en-US\",\"UTC\",0]", hidden.getJSONArray("worker").toString())
        assertEquals(List(2) { "[\"UTC\",0,\"en-US\",\"en-US\",0]" }.joinToString(",", "[", "]"), hidden.getJSONArray("leaks").toString())
        assertTrue(hidden.getString("webgl"), hidden.isNull("webgl") || hidden.getString("webgl") == "WebKit WebGL")
        assertEquals(2, hidden.getInt("visits"))
        ask("clear_data", "uid" to uid)
        reload()
        until("cleared data starts again") { it.optInt("visits") == 1 && it.has("own") }
        ask("confirm", "uid" to uid, "what" to "hide_storage")
        reload()
        until("hidden storage starts empty") { it.optInt("visits") == 1 && it.has("own") }
        reload()
        assertEquals(1, probe()!!.getInt("visits"))
    } }

    @Test
    fun clicksAndChosenFilesNeedTheUser() = runBlocking {
        val photo = fixture("green-64x32.png")
        ParcelFileDescriptor.open(photo, ParcelFileDescriptor.MODE_READ_ONLY).use {
            ask("choose", "uid" to uid, "key" to "photo", "fd" to it.fd, "name" to photo.name)
        }
        reload()
        val seen = until("the chosen photo is served") { it.has("photo") }
        assertTrue(seen.getString("photo").startsWith("read "))
        tap()
        assertEquals(0, probe()!!.getJSONArray("clicks").length())
        ask("grant", "uid" to uid, "input" to "taps", "on" to true)
        reload()
        until("the page draws") { it.getJSONArray("rates").length() > 0 }
        tap()
        until("a real tap is a click") { it.getJSONArray("clicks").let { c -> c.length() == 1 && c.getBoolean(0) } }
        assertEquals(0, reached.get())
    }

    private fun tap() {
        val now = SystemClock.uptimeMillis()
        val events = listOf(MotionEvent.ACTION_DOWN to now, MotionEvent.ACTION_UP to now + 50)
        events.forEach { (action, at) ->
            val event = MotionEvent.obtain(now, at, action, SIDE / 2f, SIDE / 2f, 0).apply { source = InputDevice.SOURCE_TOUCHSCREEN }
            instrumentation.runOnMainSync { pages.press(painter, event) }
            event.recycle()
        }
    }

    private companion object {
        val UTC = setOf("UTC", "GMT", "Etc/UTC", "Etc/GMT")
        const val ZONE = "Asia/Tokyo"
        const val TOKYO = 9 * 3_600_000
    }
}
