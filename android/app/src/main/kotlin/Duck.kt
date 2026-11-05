package io.github.wallpaper_rs

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioManager
import android.media.AudioPlaybackConfiguration
import kotlin.math.abs
import kotlin.math.sign

object Ducking {
    const val STEP = 0.1f
    const val STEP_MS = 50L

    fun others(ours: List<Boolean>, pages: Int) = ours.count { !it } > pages

    fun next(gain: Float, target: Float) = if (abs(target - gain) <= STEP) target else gain + sign(target - gain) * STEP
}

object Duck : AudioManager.AudioPlaybackCallback() {
    @Volatile
    var gain = 1f
        private set
    private var audio: AudioManager? = null
    private var target = 1f
    private val wanting = mutableSetOf<Any>()
    private val listeners = mutableListOf<() -> Unit>()
    private val ramp = Runnable { step() }

    fun listen(listener: () -> Unit) = Render.post { listeners += listener }

    fun want(context: Context, who: Any, on: Boolean) = Render.post {
        if (on) wanting += who else wanting -= who
        val listening = wanting.isNotEmpty() && Native.setting(KEY, null) == "true"
        val manager = context.applicationContext.getSystemService(AudioManager::class.java) ?: return@post
        if (listening && audio == null) {
            audio = manager
            manager.registerAudioPlaybackCallback(this, Render.handler)
            onPlaybackConfigChanged(manager.activePlaybackConfigurations)
        } else if (!listening && audio != null) {
            audio?.unregisterAudioPlaybackCallback(this)
            audio = null
            aim(1f)
        }
    }

    override fun onPlaybackConfigChanged(configs: List<AudioPlaybackConfiguration>) {
        val ours = configs.map { mine(it.audioAttributes) }
        aim(if (Ducking.others(ours, wanting.count { it is Painter })) 0f else 1f)
    }

    private fun mine(attributes: AudioAttributes) =
        attributes.contentType == AudioAttributes.CONTENT_TYPE_MOVIE && attributes.allowedCapturePolicy == AudioAttributes.ALLOW_CAPTURE_BY_SYSTEM

    private fun aim(goal: Float) {
        target = goal
        Render.cancel(ramp)
        step()
    }

    private fun step() {
        if (gain == target) return
        gain = Ducking.next(gain, target)
        Native.duck(gain)
        listeners.forEach { it() }
        if (gain != target) Render.later(Ducking.STEP_MS, ramp)
    }

    private const val KEY = "playback.duck"
}
