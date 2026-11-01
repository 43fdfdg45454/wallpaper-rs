package io.github.wallpaper_rs

import android.app.WallpaperColors
import android.graphics.Color
import android.view.Surface
import org.json.JSONArray
import org.json.JSONObject

class Painter(
    private val preview: Boolean,
    private val videos: Videos,
    private val tilt: Tilt,
    private val hearing: Hearing,
    private val pages: Pages,
    private val touchable: (Boolean) -> Unit = {},
) {
    var handle = 0L
        private set
    private var page: JSONObject? = null
    private var size = 1 to 1
    private var video: String? = null
    private var clips = emptyList<String>()
    private var hears = false
    private var touches = false
    private val handover = Runnable { refresh() }
    private val frames = Frames { nanos ->
        if (hears) hearing.poll()
        if (page != null) return@Frames listen()
        Native.draw(handle, nanos, false).also {
            Native.steer(handle)?.let { list -> Render.post { steer(JSONArray(list)) } }
            if (it < 0L) Render.handler.post(handover) else if (it == 0L && playing) follow()
        }
    }

    var shown = false
        private set

    var lock = false
        private set

    var playing = false
        private set

    @Volatile
    var surface: Surface? = null
        private set

    fun attach(surface: Surface) {
        this.surface = surface
        handle = Native.attach(surface, preview, lock, shown)
        refresh()
    }

    fun refresh() {
        val uri = Native.video(handle)
        val wanted = wanted()
        if (uri != video) {
            if (uri != null) clips.firstOrNull { "file://$it" == uri }?.let { videos.adopt(key(it), uri) }
            video?.let { old -> if (old.removePrefix(FILE) in wanted) videos.lend(old, key(old.removePrefix(FILE)), this) else videos.leave(old, this) }
        }
        video = uri
        clip(wanted)
        val spec = Native.page(handle)?.let(::JSONObject)
        if (page != null && spec == null) pages.leave(this)
        page = spec
        if (spec != null) {
            frames.stop()
            pages.show(this, surface, spec, size, shown && Native.moving())
            return follow()
        }
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
        size = width to height
        Native.resize(handle, width, height)
        if (page != null) refresh()
    }

    fun redraw() = Native.draw(handle, System.nanoTime(), true).let { if (shown) frames.schedule(it) }

    fun show(visible: Boolean) {
        shown = visible
        Native.visible(handle, visible)
        if (page != null) pages.visible(this, visible && Native.moving())
        follow()
        val uri = video
        when {
            uri != null -> videos.show(uri, this)
            visible -> frames.start()
            else -> frames.stop()
        }
    }

    fun wake() {
        if (page != null) pages.visible(this, shown && Native.moving())
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
        if (page != null) pages.leave(this)
        page = null
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

    private fun steer(list: JSONArray) = (0 until list.length()).map(list::getJSONObject).forEach {
        videos.steer(key(it.getString("path")), it)
    }

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
        if (page != null && hears) frames.start()
    }

    private fun listen(): Long {
        Native.listen(handle)?.let { pages.tell(this, it) }
        return if (hears) AUDIO_MS else 0L
    }

    private fun color(argb: Int) = if (argb == 0) null else Color.valueOf(argb)

    private companion object {
        const val AUDIO = 1
        const val TOUCH = 2
        const val CLIPS = 4
        const val FILE = "file://"
        const val AUDIO_MS = 33L
    }
}
