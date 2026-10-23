package io.github.wallpaper_rs

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioManager
import android.media.AudioPlaybackConfiguration
import android.media.audiofx.Visualizer

class Hearing(private val context: Context) : AudioManager.AudioPlaybackCallback() {
    private val listeners = mutableSetOf<Painter>()
    private var visualizer: Visualizer? = null
    private var fft = ByteArray(0)
    private var rate = 0

    fun follow(painter: Painter, wanted: Boolean) {
        if (wanted) listeners += painter else listeners -= painter
        val on = listeners.isNotEmpty() && allowed()
        if (on == (visualizer != null)) return
        visualizer?.release()
        visualizer = if (on) open() else null
        val audio = context.getSystemService(AudioManager::class.java) ?: return
        if (visualizer == null) return audio.unregisterAudioPlaybackCallback(this)
        audio.registerAudioPlaybackCallback(this, Render.handler)
        Native.playback(audio.activePlaybackConfigurations.isNotEmpty())
    }

    fun poll() {
        val active = visualizer ?: return
        if (active.getFft(fft) == Visualizer.SUCCESS) Native.hear(fft, rate)
    }

    override fun onPlaybackConfigChanged(configs: List<AudioPlaybackConfiguration>) {
        Native.playback(configs.isNotEmpty())
        if (configs.isNotEmpty()) listeners.forEach(Painter::move)
    }

    private fun allowed() =
        context.checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED

    private fun open(): Visualizer? {
        val made = runCatching { Visualizer(0) }.getOrNull() ?: return null
        return runCatching {
            made.apply {
                setCaptureSize(Visualizer.getCaptureSizeRange()[1])
                fft = ByteArray(captureSize)
                rate = samplingRate / 1000
                setEnabled(true)
            }
        }.onFailure { made.release() }.getOrNull()
    }
}
