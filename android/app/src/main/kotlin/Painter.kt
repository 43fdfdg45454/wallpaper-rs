package io.github.wallpaper_rs

import android.app.WallpaperColors
import android.graphics.Color
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView
import org.json.JSONArray

class Painter(
    private val preview: Boolean,
    private val videos: Videos,
    private val tilt: Tilt,
    private val hearing: Hearing,
    private val touchable: (Boolean) -> Unit = {},
) {
    private var handle = 0L
    private var video: String? = null
    private var clips = emptyList<String>()
    private var hears = false
    private var touches = false
    private val handover = Runnable { refresh() }
    private val frames = Frames { nanos ->
        if (hears) hearing.poll()
        Native.draw(handle, nanos, false).also { if (it < 0L) Render.handler.post(handover) else if (it == 0L && playing) follow() }
    }

    var shown = false
        private set

    var lock = false
        private set

    var playing = false
        private set

    var surface: Surface? = null
        private set

    fun attach(surface: Surface) {
        this.surface = surface
        handle = Native.attach(surface, preview, lock)
        refresh()
    }

    fun refresh() {
        val uri = Native.video(handle)
        val wanted = wanted()
        if (uri != video) {
            if (uri != null) clips.firstOrNull { "file://$it" == uri }?.let { videos.adopt(key(it), uri) }
            video?.let { old -> if (old.removePrefix(FILE) in wanted) videos.lend(old, key(old.removePrefix(FILE))) else videos.leave(old, this) }
        }
        video = uri
        clip(wanted)
        if (uri == null) return wake()
        frames.stop()
        videos.join(uri, this)
        follow()
    }

    fun screen(lock: Boolean) {
        if (lock == this.lock) return
        this.lock = lock
        Native.screen(handle, lock)
        if (handle != 0L) refresh()
    }

    fun resize(width: Int, height: Int, rotation: Int) {
        tilt.rotation = rotation
        Native.resize(handle, width, height)
    }

    fun redraw() = Native.draw(handle, System.nanoTime(), true).let { if (shown) frames.schedule(it) }

    fun show(visible: Boolean) {
        shown = visible
        Native.visible(handle, visible)
        follow()
        val uri = video
        when {
            uri != null -> videos.show(uri, this)
            visible -> frames.start()
            else -> frames.stop()
        }
    }

    fun wake() {
        follow()
        val uri = video
        if (uri != null) videos.update(uri) else if (shown) frames.start()
    }

    fun move() {
        if (shown && video == null) frames.start()
    }

    fun offset(x: Float) {
        if (Native.offset(handle, x)) wake()
    }

    fun touch(x: Float, y: Float, down: Boolean) {
        if (Native.touch(handle, x, y, down)) move()
    }

    fun colors(): WallpaperColors? = Native.colors(preview, lock)?.let { (primary, secondary, tertiary, hints) ->
        WallpaperColors(Color.valueOf(primary), color(secondary), color(tertiary), hints)
    }

    fun detach() {
        frames.stop()
        tilt.follow(this, 0)
        hearing.follow(this, false)
        clips.forEach { videos.leave(key(it), this) }
        clips = emptyList()
        video?.let { videos.leave(it, this) }
        video = null
        Native.detach(handle)
        handle = 0L
        surface = null
    }

    private fun wanted() = Native.clips(handle)?.let(::JSONArray)?.let { list -> List(list.length(), list::getString) }.orEmpty()

    private fun clip(wanted: List<String>) {
        val film = Native.film(handle)
        clips.filter { it !in wanted }.forEach { path ->
            videos.leave(key(path), this)
            Native.clip(handle, path, false)
        }
        wanted.filter { it !in clips }.forEach { path ->
            Native.clip(handle, path, true)?.let { videos.clip(key(path), path, it, this, path == film) }
        }
        clips = wanted
    }

    private fun key(path: String) = "$path#$handle"

    private fun follow() {
        val inputs = if (video == null) Native.inputs(handle) else 0
        hears = shown && inputs and AUDIO != 0
        hearing.follow(this, hears)
        playing = shown && inputs and CLIPS != 0
        clips.forEach { videos.update(key(it)) }
        val touch = inputs and TOUCH != 0
        if (touch != touches) touchable(touch)
        touches = touch
        tilt.follow(this, if (shown && video == null) Native.parallax(handle) else 0)
    }

    private fun color(argb: Int) = if (argb == 0) null else Color.valueOf(argb)

    private companion object {
        const val AUDIO = 1
        const val TOUCH = 2
        const val CLIPS = 4
        const val FILE = "file://"
    }
}

@Composable
fun Live(main: Main, vararg changes: Any?, modifier: Modifier = Modifier.fillMaxSize()) = Box(modifier) {
    val painter = remember { Painter(true, main.videos, main.tilt, main.hearing) }
    LaunchedEffect(*changes) { Render.post(painter::refresh) }
    AndroidView({ SurfaceView(it).apply { holder.addCallback(Preview(this, painter)) } }, Modifier.fillMaxSize())
}

private class Preview(private val view: SurfaceView, private val painter: Painter) : SurfaceHolder.Callback {
    override fun surfaceCreated(holder: SurfaceHolder) = Render.call {
        painter.attach(holder.surface)
        painter.show(true)
        Wallpaper.previews += painter
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) = Render.call {
        painter.resize(width, height, view.display?.rotation ?: 0)
        painter.redraw()
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) = Render.call {
        Wallpaper.previews -= painter
        painter.detach()
    }
}
