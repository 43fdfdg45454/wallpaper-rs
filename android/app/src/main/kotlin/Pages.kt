package io.github.wallpaper_rs

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.view.MotionEvent
import android.view.Surface
import android.webkit.WebStorage
import org.json.JSONArray
import org.json.JSONObject

class Pages(private val context: Context) {
    private val shown = mutableMapOf<Painter, Shown>()

    private inner class Shown(val painter: Painter, var spec: JSONObject, val surface: Surface, var size: Pair<Int, Int>) {
        var page: Page? = null
        var visible = false
        var crashes = 0
        val sleep = Runnable { page?.close(); page = null }

        fun open() {
            if (page != null || crashes > MAX_CRASHES || !surface.isValid) return
            page = Page(context, painter.handle, spec, surface, size, ::crashed) { Render.post(painter::wake) }.also { it.visible(visible) }
        }

        fun crashed() = main.post {
            page?.close()
            page = null
            crashes++
            if (visible) open()
        }
    }

    fun show(painter: Painter, surface: Surface?, spec: JSONObject, size: Pair<Int, Int>, visible: Boolean) = main.post {
        if (surface == null || painter.surface !== surface) return@post
        val old = shown[painter]
        val same = old != null && old.surface === surface && old.spec.getString("key") == spec.getString("key")
        if (!same) drop(painter)
        val entry = shown.getOrPut(painter) { Shown(painter, spec, surface, size) }
        entry.spec = spec
        entry.page?.follow(spec)
        if (!spec.isNull("tell")) entry.page?.tell(spec.getString("tell"))
        if (entry.size != size) entry.page?.resize(size.first, size.second)
        entry.size = size
        show(entry, visible)
    }

    private fun show(entry: Shown, visible: Boolean) {
        entry.visible = visible
        main.removeCallbacks(entry.sleep)
        if (visible) entry.open() else main.postDelayed(entry.sleep, SLEEP_MS)
        entry.page?.visible(visible)
    }

    fun visible(painter: Painter, visible: Boolean) = main.post { shown[painter]?.let { show(it, visible) } }

    fun tell(painter: Painter, script: String) = main.post { shown[painter]?.page?.tell(script) }

    fun leave(painter: Painter) = main.post { drop(painter) }

    fun drop(painter: Painter) {
        val entry = shown.remove(painter) ?: return
        main.removeCallbacks(entry.sleep)
        entry.page?.close()
    }

    fun press(painter: Painter, event: MotionEvent): Boolean {
        val page = shown[painter]?.page ?: return shown.containsKey(painter)
        page.press(event)
        return true
    }

    fun page(painter: Painter) = shown[painter]?.page

    companion object {
        private val main = Handler(Looper.getMainLooper())
        private const val SLEEP_MS = 5 * 60_000L
        private const val MAX_CRASHES = 3
        private const val PASSING = ".e.wpak.invalid"

        private var swept = false

        fun forget(sweep: Boolean = false) {
            val origins = JSONArray(Native.library(JSONObject().put("op", "cleared").toString()))
            val sweeping = sweep && !swept
            if (origins.length() == 0 && !sweeping) return
            val storage = WebStorage.getInstance()
            (0 until origins.length()).forEach { storage.deleteOrigin(origins.getString(it)) }
            if (sweeping) storage.getOrigins { found: Map<*, *> ->
                found.keys.map { it.toString() }.filter { it.endsWith(PASSING) }.forEach(storage::deleteOrigin)
            }
            swept = swept || sweep
        }
    }
}
