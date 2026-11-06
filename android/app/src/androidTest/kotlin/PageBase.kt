package io.github.wallpaper_rs

import android.webkit.WebView
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.net.ServerSocket
import java.net.SocketTimeoutException
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.json.JSONTokener
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Before

abstract class PageBase {
    protected val instrumentation = InstrumentationRegistry.getInstrumentation()
    protected val context = instrumentation.targetContext
    protected val pixel = AtomicInteger()
    protected val readers = Readers("page")
    protected val reader = readers.make(SIDE)
    protected val pages = Pages(context)
    protected val painter = Painter(false, Videos(context), Tilt(context), Hearing(context), pages)
    protected val server = ServerSocket(0, 1, java.net.InetAddress.getByName("127.0.0.1")).apply { soTimeout = 100 }
    protected val reached = AtomicInteger()
    protected lateinit var uid: String

    @Before
    fun prepare() = runBlocking {
        val dir = File(storage(), "web/probe").apply { deleteRecursively(); mkdirs() }
        File(dir, "manifest.json").writeText(PROBE_MANIFEST)
        File(dir, "index.html").writeText(PROBE_PAGE)
        File(dir, "probe.js").writeText(PROBE_SCRIPT)
        File(dir, "peers.html").writeText(PEERS_PAGE)
        File(dir, "early.js").writeText("window.early = \"preloaded\";")
        File(dir, "manifest.json").writeText(PROBE_MANIFEST.replace("org.example.probe", "org.example.probe${System.nanoTime()}"))
        uid = (ask("examples", "dir" to dir.parent) as org.json.JSONArray).getString(0)
        ask("property", "uid" to uid, "key" to "port", "value" to server.localPort)
        readers.listen(reader) { source ->
            source.acquireLatestImage()?.use { image ->
                val plane = image.planes[0]
                val at = SIDE / 2 * plane.rowStride + SIDE / 2 * plane.pixelStride
                pixel.set((0..2).fold(0) { argb, i -> argb shl 8 or (plane.buffer.get(at + i).toInt() and 0xff) })
            }
        }
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
        readers.close()
    }

    protected fun probe(): JSONObject? = read("probe") { pages.page(painter)?.web }

    protected fun read(name: String, web: () -> WebView?): JSONObject? {
        var result: String? = null
        val done = java.util.concurrent.CountDownLatch(1)
        instrumentation.runOnMainSync {
            val view = web()
            if (view == null) done.countDown() else view.evaluateJavascript("JSON.stringify(window.$name || null)") { result = it; done.countDown() }
        }
        done.await(5, java.util.concurrent.TimeUnit.SECONDS)
        val text = result?.let { JSONTokener(it).nextValue() as? String } ?: return null
        return runCatching { JSONObject(text) }.getOrNull()
    }

    protected fun until(what: String, check: (JSONObject) -> Boolean): JSONObject {
        assertTrue("$what: ${probe()} at ${pages.page(painter)?.key}", waitFor { probe()?.let(check) == true })
        return probe()!!
    }

    protected fun reload() = Render.call { painter.refresh() }

    protected companion object {
        const val SIDE = 256
    }
}
