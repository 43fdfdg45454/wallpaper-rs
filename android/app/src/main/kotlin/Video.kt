package io.github.wallpaper_rs

import android.content.Context
import android.os.Handler
import android.view.Surface
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.TransferListener
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.upstream.BandwidthMeter

@OptIn(UnstableApi::class)
class Videos(private val context: Context) {
    private val videos = mutableMapOf<String, Video>()

    fun join(uri: String, painter: Painter) = videos.getOrPut(uri) { Video(uri) }.run {
        if (painter !in viewers) viewers += painter
        player?.let { configure(it) }
        update()
    }

    fun clip(key: String, path: String, surface: Surface, painter: Painter, audible: Boolean = false) =
        videos.getOrPut(key) { Video("file://$path", surface, audible) }.run {
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
            if (key !in videos) videos[key] = Video(uri, from = video.at())
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

    fun positions() = videos.mapValues { it.value.at() }

    fun trim() = videos.values.filter { it.player?.playWhenReady == false }.forEach { it.release() }

    private fun configure(player: ExoPlayer, silent: Boolean = false) {
        val stretch = Native.setting(FIT, null) == STRETCH
        player.videoScalingMode =
            if (stretch) C.VIDEO_SCALING_MODE_SCALE_TO_FIT else C.VIDEO_SCALING_MODE_SCALE_TO_FIT_WITH_CROPPING
        val muted = silent || Native.setting(SOUND, null) != "false"
        player.trackSelectionParameters =
            player.trackSelectionParameters.buildUpon().setTrackTypeDisabled(C.TRACK_TYPE_AUDIO, muted).build()
    }

    private inner class Video(
        private val uri: String,
        var fixed: Surface? = null,
        private val audible: Boolean = false,
        from: Long = 0L,
    ) : Player.Listener {
        val viewers = mutableListOf<Painter>()
        var player: ExoPlayer? = null
        var output: Surface? = null
        private var position = from
        private var retried = false
        private val idle = Runnable { release() }

        fun update() {
            Render.cancel(idle)
            val reader = fixed
            val target = if (reader != null) reader.takeIf { viewers.any(Painter::playing) }
            else viewers.lastOrNull { it.shown }?.surface?.takeIf { Native.moving() }
            val active = player
            when {
                target == null -> {
                    retried = false
                    active?.pause()
                    if (active != null) Render.later(IDLE_MS, idle)
                }
                active == null -> start(target)
                else -> {
                    if (output !== target) active.setVideoSurface(target)
                    output = target
                    active.play()
                }
            }
        }

        override fun onPlayerError(error: PlaybackException) = Render.post {
            if (!retried && player?.playerError != null) {
                retried = true
                release()
                update()
            }
        }

        private fun start(target: Surface) {
            player = ExoPlayer.Builder(context).setBandwidthMeter(Offline).setUsePlatformDiagnostics(false).build().apply {
                addListener(this@Video)
                repeatMode = Player.REPEAT_MODE_ONE
                configure(this, fixed != null && !audible)
                setVideoSurface(target)
                setMediaItem(MediaItem.fromUri(uri), position)
                prepare()
                play()
            }
            output = target
        }

        fun at() = player?.currentPosition ?: position

        fun detach() {
            player?.clearVideoSurface()
            output = null
        }

        fun release() {
            Render.cancel(idle)
            player?.let {
                position = it.currentPosition
                it.release()
            }
            player = null
            output = null
        }
    }

    private object Offline : BandwidthMeter {
        override fun getBitrateEstimate() = 0L

        override fun getTransferListener(): TransferListener? = null

        override fun addEventListener(eventHandler: Handler, eventListener: BandwidthMeter.EventListener) = Unit

        override fun removeEventListener(eventListener: BandwidthMeter.EventListener) = Unit
    }

    private companion object {
        const val FIT = "fit"
        const val STRETCH = "stretch"
        const val SOUND = "playback.muted"
        const val IDLE_MS = 30_000L
    }
}
