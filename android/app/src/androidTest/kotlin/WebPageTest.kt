package io.github.wallpaper_rs

import android.graphics.PixelFormat
import android.hardware.HardwareBuffer
import android.media.ImageReader
import android.os.Handler
import android.os.HandlerThread
import android.os.ParcelFileDescriptor
import android.os.SystemClock
import android.view.InputDevice
import android.view.MotionEvent
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.net.ServerSocket
import java.net.SocketTimeoutException
import java.util.TimeZone
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.json.JSONTokener
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class WebPageTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val context = instrumentation.targetContext
    private val pixel = AtomicInteger()
    private val thread = HandlerThread("page").apply { start() }
    private val usage = HardwareBuffer.USAGE_GPU_COLOR_OUTPUT or HardwareBuffer.USAGE_CPU_READ_OFTEN
    private val reader = ImageReader.newInstance(SIDE, SIDE, PixelFormat.RGBA_8888, 3, usage)
    private val pages = Pages(context)
    private val painter = Painter(false, Videos(context), Tilt(context), Hearing(context), pages)
    private val server = ServerSocket(0, 1, java.net.InetAddress.getByName("127.0.0.1")).apply { soTimeout = 100 }
    private val reached = AtomicInteger()
    private lateinit var uid: String

    @Before
    fun prepare() = runBlocking {
        val dir = File(storage(), "web/probe").apply { deleteRecursively(); mkdirs() }
        File(dir, "manifest.json").writeText(PROBE_MANIFEST)
        File(dir, "index.html").writeText(PROBE_PAGE)
        File(dir, "probe.js").writeText(PROBE_SCRIPT)
        File(dir, "early.js").writeText("window.early = \"preloaded\";")
        File(dir, "manifest.json").writeText(PROBE_MANIFEST.replace("org.example.probe", "org.example.probe${System.nanoTime()}"))
        uid = (ask("examples", "dir" to dir.parent) as org.json.JSONArray).getString(0)
        ask("property", "uid" to uid, "key" to "port", "value" to server.localPort)
        reader.setOnImageAvailableListener({ source ->
            source.acquireLatestImage()?.use { image ->
                val plane = image.planes[0]
                val at = SIDE / 2 * plane.rowStride + SIDE / 2 * plane.pixelStride
                pixel.set((0..2).fold(0) { argb, i -> argb shl 8 or (plane.buffer.get(at + i).toInt() and 0xff) })
            }
        }, Handler(thread.looper))
        Thread { while (!server.isClosed) runCatching { server.accept().close(); reached.incrementAndGet() }.exceptionOrNull()?.takeIf { it !is SocketTimeoutException && !server.isClosed }?.let { throw it } }.start()
        Native.library(JSONObject(mapOf("op" to "pick", "choice" to choice("item", uid))).toString())
        Render.call {
            Native.commit(BOTH)
            painter.attach(reader.surface)
            painter.resize(SIDE, SIDE, 0)
            painter.show(true)
        }
    }

    @After
    fun detach() {
        instrumentation.runOnMainSync { pages.drop(painter) }
        Render.call { painter.detach() }
        runBlocking { ask("remove", "uids" to listOf(uid)) }
        server.close()
        reader.close()
        thread.quitSafely()
    }

    private fun probe(): JSONObject? {
        var result: String? = null
        val done = java.util.concurrent.CountDownLatch(1)
        instrumentation.runOnMainSync {
            val web = pages.page(painter)?.web
            if (web == null) done.countDown() else web.evaluateJavascript("JSON.stringify(window.probe || null)") { result = it; done.countDown() }
        }
        done.await(5, java.util.concurrent.TimeUnit.SECONDS)
        val text = result?.let { JSONTokener(it).nextValue() as? String } ?: return null
        return runCatching { JSONObject(text) }.getOrNull()
    }

    private fun until(what: String, check: (JSONObject) -> Boolean): JSONObject {
        assertTrue("$what: ${probe()} at ${pages.page(painter)?.key}", waitFor { probe()?.let(check) == true })
        return probe()!!
    }

    private fun reload() = Render.call { painter.refresh() }

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
        assertEquals(listOf(ZONE, 21), shown.getJSONArray("worker").let { listOf(it.getString(0), it.getInt(1)) })
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
        assertEquals("[\"UTC\",12,\"en-US\"]", hidden.getJSONArray("worker").toString())
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
        const val SIDE = 256
        val UTC = setOf("UTC", "GMT", "Etc/UTC", "Etc/GMT")
        const val ZONE = "Asia/Tokyo"
    }
}
