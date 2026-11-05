package io.github.wallpaper_rs

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
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
class KeptTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val context = instrumentation.targetContext
    private val readers = Readers("kept")
    private val reader = readers.make(SIDE)
    private val pages = Pages(context)
    private val painter = Painter(false, Videos(context), Tilt(context), Hearing(context), pages)
    private lateinit var uid: String

    @Before
    fun prepare() = runBlocking {
        val dir = File(storage(), "kept-web/kept").apply { deleteRecursively(); mkdirs() }
        File(dir, "manifest.json").writeText(MANIFEST.replace("org.example.kept", "org.example.kept${System.nanoTime()}"))
        File(dir, "index.html").writeText(PAGE)
        uid = (ask("examples", "dir" to dir.parent) as org.json.JSONArray).getString(0)
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
        readers.close()
    }

    private fun probe(): JSONObject? {
        var result: String? = null
        val done = CountDownLatch(1)
        instrumentation.runOnMainSync {
            val web = pages.page(painter)?.web
            if (web == null) done.countDown() else web.evaluateJavascript("JSON.stringify(window.probe || null)") { result = it; done.countDown() }
        }
        done.await(5, TimeUnit.SECONDS)
        val text = result?.let { JSONTokener(it).nextValue() as? String } ?: return null
        return runCatching { JSONObject(text) }.getOrNull()
    }

    private fun until(what: String, check: (JSONObject) -> Boolean): JSONObject {
        assertTrue("$what: ${probe()}", waitFor { probe()?.let(check) == true })
        return probe()!!
    }

    private fun full() = runBlocking { (ask("scene", "uid" to uid, "languages" to listOf("en")) as JSONObject).getJSONObject("web").getBoolean("full") }

    @Test
    fun aPageKeepsNoMoreThanItDeclared() {
        val first = until("the page tries to keep data") { it.has("big") && it.has("stored") }
        assertEquals("kept", first.getString("small"))
        assertEquals("QuotaExceededError", first.getString("big"))
        assertEquals("QuotaExceededError", first.getString("named"))
        assertEquals(true, first.get("stored"))
        assertTrue(!first.getString("origin").endsWith(".e.wpak.invalid"))
        val passing = until("a page that grows past its cap while it runs starts again without its data") {
            it.optString("origin").endsWith(".e.wpak.invalid")
        }
        assertEquals("kept", passing.getString("small"))
        assertTrue(full())
        runBlocking { ask("clear_data", "uid" to uid) }
        Render.call { painter.refresh() }
        until("cleared data keeps again") { !it.optString("origin").endsWith(".e.wpak.invalid") && it.has("stored") }
        assertTrue(!full())
    }

    private companion object {
        const val SIDE = 64
        const val MANIFEST = """{ "schema": "wallpaper-rs.wpak/2", "id": "org.example.kept", "version": "1.0.0",
            "title": "Kept", "license": "CC0-1.0", "kind": "web", "entry": "index.html", "origin": { "kind": "original" },
            "content": { "rating": "general" }, "photosensitive": false, "web": { "storage_kib": 4 } }"""
        const val PAGE = """<!doctype html><html><body><script>
const probe = (window.probe = { origin: location.origin });
const keep = (name, value) => { try { localStorage.setItem(name, value); probe[name] = "kept"; } catch (e) { probe[name] = e.name; } };
keep("small", "s");
keep("big", "x".repeat(600000));
try { localStorage.named = "x".repeat(600000); probe.named = "kept"; } catch (e) { probe.named = e.name; }
const data = new Uint8Array(3 << 20);
for (let i = 0; i < data.length; i += 65536) crypto.getRandomValues(data.subarray(i, i + 65536));
const open = indexedDB.open("kept", 1);
open.onupgradeneeded = () => open.result.createObjectStore("s");
open.onsuccess = () => {
  const tx = open.result.transaction("s", "readwrite");
  tx.objectStore("s").put(data, "blob");
  tx.oncomplete = () => { probe.stored = true; };
  tx.onerror = () => { probe.stored = String(tx.error); };
};
open.onerror = () => { probe.stored = String(open.error); };
</script></body></html>"""
    }
}
