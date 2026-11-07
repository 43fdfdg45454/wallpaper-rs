package io.github.wallpaper_rs

import android.content.Context
import android.os.Handler
import androidx.annotation.OptIn
import androidx.media3.common.Format
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.Renderer
import androidx.media3.exoplayer.mediacodec.MediaCodecAdapter
import androidx.media3.exoplayer.mediacodec.MediaCodecSelector
import androidx.media3.exoplayer.video.MediaCodecVideoRenderer
import androidx.media3.exoplayer.video.VideoRendererEventListener
import java.nio.ByteBuffer

class FrameCap {
    @Volatile
    var fps = 0

    @Volatile
    var speed = 1f
    private var due = 0L
    private var last = NONE
    private var verdict = true

    fun reset() {
        last = NONE
    }

    fun keeps(pts: Long): Boolean {
        if (pts == last) return verdict
        val gap = if (last == NONE) NONE else pts - last
        last = pts
        val step = (MICROS * speed / fps.coerceAtLeast(1)).toLong()
        val fresh = fps <= 0 || gap !in 0..JUMP
        verdict = fresh || pts + gap / 2 >= due
        if (fresh) due = pts + step else if (verdict) due = maxOf(due, pts - step) + step
        return verdict
    }

    private companion object {
        const val NONE = -1L
        const val MICROS = 1_000_000L
        const val JUMP = MICROS
    }
}

@OptIn(UnstableApi::class)
class Capped(builder: MediaCodecVideoRenderer.Builder, private val cap: FrameCap) : MediaCodecVideoRenderer(builder) {
    override fun onPositionReset(positionUs: Long, joining: Boolean, resetToKeyFrame: Boolean) {
        cap.reset()
        super.onPositionReset(positionUs, joining, resetToKeyFrame)
    }

    override fun setPlaybackSpeed(current: Float, target: Float) {
        cap.speed = current
        super.setPlaybackSpeed(current, target)
    }

    override fun processOutputBuffer(
        positionUs: Long,
        elapsedRealtimeUs: Long,
        codec: MediaCodecAdapter?,
        buffer: ByteBuffer?,
        index: Int,
        flags: Int,
        sampleCount: Int,
        presentationUs: Long,
        decodeOnly: Boolean,
        last: Boolean,
        format: Format,
    ): Boolean {
        if (codec != null && !decodeOnly && !last && !cap.keeps(presentationUs)) {
            skipOutputBuffer(codec, index, presentationUs)
            return true
        }
        return super.processOutputBuffer(positionUs, elapsedRealtimeUs, codec, buffer, index, flags, sampleCount, presentationUs, decodeOnly, last, format)
    }
}

@OptIn(UnstableApi::class)
class CappedRenderers(context: Context, private val cap: FrameCap) : DefaultRenderersFactory(context) {
    override fun buildVideoRenderers(
        context: Context,
        mode: Int,
        selector: MediaCodecSelector,
        fallback: Boolean,
        handler: Handler,
        listener: VideoRendererEventListener,
        joining: Long,
        out: ArrayList<Renderer>,
    ) {
        out += Capped(
            MediaCodecVideoRenderer.Builder(context).setCodecAdapterFactory(codecAdapterFactory).setMediaCodecSelector(selector)
                .setAllowedJoiningTimeMs(joining).setEnableDecoderFallback(fallback).setEventHandler(handler)
                .setEventListener(listener).setMaxDroppedFramesToNotify(MAX_DROPPED_VIDEO_FRAME_COUNT_TO_NOTIFY),
            cap,
        )
    }
}
