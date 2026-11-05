package io.github.wallpaper_rs

import android.content.Context
import android.view.Surface
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import org.json.JSONObject

@OptIn(UnstableApi::class)
class Videos(private val context: Context) : Ducked {
    private val videos = mutableMapOf<String, Video>()

    init {
        Duck.listen(this)
    }

    override fun ducked() = videos.values.forEach(Video::duck)

    fun close() = Duck.unlisten(this)

    fun join(uri: String, painter: Painter) = videos.getOrPut(uri) { Video(context, uri) }.run {
        if (painter !in viewers) viewers += painter
        configure()
        update()
    }

    fun clip(key: String, path: String, surface: Surface, painter: Painter, audible: Boolean = false) =
        videos.getOrPut(key) { Video(context, "file://$path", surface, audible) }.run {
            if (fixed == null) fixed = surface
            if (painter !in viewers) viewers += painter
            update()
        }

    fun adopt(key: String, uri: String) = videos.remove(key)?.let { video ->
        if (uri in videos) return@let video.release()
        video.detach()
        video.fixed = null
        videos[uri] = video
    }

    fun lend(uri: String, key: String, painter: Painter) {
        val video = videos[uri] ?: return
        if (video.viewers.any { it !== painter } || key in videos) {
            if (key !in videos) videos[key] = Video(context, uri, from = video.at())
            leave(uri, painter)
            return
        }
        videos -= uri
        video.detach()
        video.fixed = null
        videos[key] = video
    }

    fun leave(uri: String, painter: Painter) = videos[uri]?.run {
        viewers -= painter
        if (output === painter.surface) detach()
        if (viewers.isNotEmpty()) update() else release().also { videos -= uri }
    }

    fun show(uri: String, painter: Painter) = videos[uri]?.run {
        viewers -= painter
        viewers += painter
        update()
    }

    fun update(uri: String) = videos[uri]?.update()

    fun steer(key: String, wanted: JSONObject) = videos[key]?.run {
        held = wanted.getBoolean("playing")
        speed = wanted.getDouble("rate").toFloat()
        looped = wanted.getBoolean("loop")
        val seek = wanted.getLong("at").takeIf { wanted.getBoolean("seek") }
        val range = wanted.getLong("start") to (wanted.getLong("end").takeIf { it >= 0 } ?: C.TIME_END_OF_SOURCE)
        val cut = range != clip
        clip = range
        seek?.let { position = it }
        player?.let { steer(it, seek, cut) }
        update()
    }

    fun cut(key: String?, reel: String?) {
        val (video, told) = (videos[key ?: return] ?: return) to JSONObject(reel ?: return)
        video.speed = told.getDouble("rate").toFloat()
        video.looped = told.getBoolean("loop")
        val loudness = told.getDouble("volume").toFloat()
        val hushed = (loudness <= 0f) != (video.loudness <= 0f)
        video.loudness = loudness
        if (hushed) video.configure()
        val range = told.getLong("start") to (told.getLong("end").takeIf { it >= 0 } ?: C.TIME_END_OF_SOURCE)
        val cut = range != video.clip
        video.clip = range
        video.player?.let { video.steer(it, null, cut) }
    }

    fun positions() = videos.mapValues { it.value.at() }

    fun trim() = videos.values.filter { it.player?.playWhenReady == false }.forEach { it.release() }
}
