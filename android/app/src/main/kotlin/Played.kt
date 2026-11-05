package io.github.wallpaper_rs

import android.content.Context
import android.view.Surface
import androidx.annotation.OptIn
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer

private const val FITTING = "fit"
private const val STRETCH = "stretch"
private const val SOUND = "playback.muted"
private const val IDLE_MS = 30_000L
internal val WHOLE = 0L to C.TIME_END_OF_SOURCE
private val MARK = AudioAttributes.Builder().setUsage(C.USAGE_MEDIA).setContentType(C.AUDIO_CONTENT_TYPE_MOVIE)
    .setAllowedCapturePolicy(C.ALLOW_CAPTURE_BY_SYSTEM).build()

@OptIn(UnstableApi::class)
internal class Video(
    private val context: Context,
    private val uri: String,
    var fixed: Surface? = null,
    private val audible: Boolean = false,
    from: Long = 0L,
) : Player.Listener {
    val viewers = mutableListOf<Painter>()
    var held = true
    var speed = 1f
    var looped = true
    var loudness = 1f
    var player: ExoPlayer? = null
    var output: Surface? = null
    var position = from
    var clip = WHOLE
    private var muted = true
    private var retried = false
    private val idle = Runnable { release() }

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
                if (output !== target) active.setVideoSurface(target)
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
        muted = (fixed != null && !audible) || Native.setting(SOUND, null) != "false"
        active.trackSelectionParameters =
            active.trackSelectionParameters.buildUpon().setTrackTypeDisabled(C.TRACK_TYPE_AUDIO, muted).build()
        heard()
    }

    fun duck() {
        player?.volume = loudness.coerceIn(0f, 1f) * Duck.gain
    }

    private fun heard() = Duck.want(context, this, player?.playWhenReady == true && !muted && loudness > 0f)

    override fun onPlayerError(error: PlaybackException) = Render.post {
        if (!retried && player?.playerError != null) {
            retried = true
            clip = WHOLE
            release()
            update()
        }
    }

    private fun start(target: Surface) {
        player = ExoPlayer.Builder(context).setBandwidthMeter(Offline).setUsePlatformDiagnostics(false)
            .setAudioAttributes(MARK, false).build().apply {
                addListener(this@Video)
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
