package io.github.wallpaper_rs

import android.content.Context
import android.media.MediaFormat
import android.view.Surface
import androidx.annotation.OptIn
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.video.VideoFrameMetadataListener

private const val FITTING = "fit"
private const val STRETCH = "stretch"
private const val SOUND = "playback.muted"
private const val IDLE_MS = 30_000L
private const val FILE = "file://"
internal val WHOLE = 0L to C.TIME_END_OF_SOURCE
private val MOVIE = AudioAttributes.Builder().setUsage(C.USAGE_MEDIA).setContentType(C.AUDIO_CONTENT_TYPE_MOVIE).build()

@OptIn(UnstableApi::class)
internal class Video(
    private val context: Context,
    private val uri: String,
    var fixed: Surface? = null,
    private val audible: Boolean = false,
    from: Long = 0L,
) : Player.Listener {
    val viewers = mutableListOf<Painter>()
    val reels = mutableMapOf<Painter, Reel>()
    var held = true
    var speed = 1f
    var looped = true
    var capped = false
    val frames = FrameCap()
    var loudness = 1f
    var player: ExoPlayer? = null
    var output: Surface? = null
    var position = from
    var clip = WHOLE
    private var muted = true
    private var retried = false
    private var transfer = -1
    private val idle = Runnable { release() }
    private val decoded = VideoFrameMetadataListener { _, _, _, format ->
        val told = format?.takeIf { it.containsKey(MediaFormat.KEY_COLOR_TRANSFER) }?.getInteger(MediaFormat.KEY_COLOR_TRANSFER)
        if (told != null && told != transfer) {
            transfer = told
            Native.decoded(uri.removePrefix(FILE), told)
        }
    }

    fun update() {
        Render.cancel(idle)
        val reader = fixed
        val target = if (reader != null) reader.takeIf { held && viewers.any(Painter::playing) }
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
                if (output !== target) {
                    active.setVideoSurface(target)
                    Ending.redraw(active.playbackState, active.currentPosition)?.let(active::seekTo)
                }
                output = target
                active.play()
            }
        }
        heard()
    }

    fun configure() {
        val active = player ?: return
        val stretch = Native.setting(FITTING, null) == STRETCH
        active.videoScalingMode =
            if (stretch) C.VIDEO_SCALING_MODE_SCALE_TO_FIT else C.VIDEO_SCALING_MODE_SCALE_TO_FIT_WITH_CROPPING
        muted = Audible.muted(fixed != null && !audible, Native.setting(SOUND, null), loudness)
        active.trackSelectionParameters =
            active.trackSelectionParameters.buildUpon().setTrackTypeDisabled(C.TRACK_TYPE_AUDIO, muted).build()
        heard()
    }

    fun lead() = viewers.lastOrNull { it.shown } ?: viewers.lastOrNull()

    fun follow() {
        val reel = lead()?.let(reels::get) ?: return
        val cut = reel.clip != clip
        val hushed = (reel.loudness <= 0f) != (loudness <= 0f)
        speed = reel.speed
        looped = reel.looped
        capped = reel.fps > 0
        frames.fps = reel.fps
        loudness = reel.loudness
        clip = reel.clip
        if (hushed) configure()
        player?.let { steer(it, null, cut) }
    }

    fun duck() {
        player?.volume = loudness.coerceIn(0f, 1f) * Duck.gain
    }

    private fun heard() = Duck.want(context, this, player?.playWhenReady == true && !muted && loudness > 0f)

    fun told() = player?.let { onPlaybackStateChanged(it.playbackState) }

    override fun onPlaybackStateChanged(state: Int) {
        val path = uri.removePrefix(FILE)
        viewers.toList().forEach { it.ended(path, Ending.over(state)) }
    }

    override fun onPlayerError(error: PlaybackException) = Render.post {
        if (!retried && player?.playerError != null) {
            retried = true
            clip = WHOLE
            release()
            update()
        }
    }

    private fun start(target: Surface) {
        player = ExoPlayer.Builder(context, CappedRenderers(context, frames)).setBandwidthMeter(Offline).setUsePlatformDiagnostics(false)
            .setAudioAttributes(MOVIE, false).build().apply {
                addListener(this@Video)
                if (fixed != null) setVideoFrameMetadataListener(decoded)
                steer(this, null)
                setVideoSurface(target)
                setMediaItem(item(), position)
                prepare()
                play()
            }
        configure()
        output = target
    }

    fun steer(player: ExoPlayer, seek: Long?, cut: Boolean = false) {
        player.repeatMode = if (looped) Player.REPEAT_MODE_ONE else Player.REPEAT_MODE_OFF
        player.setVideoChangeFrameRateStrategy(
            if (capped) C.VIDEO_CHANGE_FRAME_RATE_STRATEGY_OFF else C.VIDEO_CHANGE_FRAME_RATE_STRATEGY_ONLY_IF_SEAMLESS,
        )
        player.setPlaybackSpeed(speed.coerceIn(0.1f, 4f))
        duck()
        heard()
        if (cut) player.setMediaItem(item(), seek ?: 0L) else seek?.let(player::seekTo)
    }

    private fun item() = MediaItem.Builder().setUri(uri).setClippingConfiguration(
        MediaItem.ClippingConfiguration.Builder().setStartPositionMs(clip.first).setEndPositionMs(clip.second).build(),
    ).build()

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
        heard()
    }
}

internal object Ending {
    fun over(state: Int) = state == Player.STATE_ENDED

    fun redraw(state: Int, at: Long) = at.takeIf { over(state) }
}
