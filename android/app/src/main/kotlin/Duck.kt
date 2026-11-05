package io.github.wallpaper_rs

import android.content.Context
import android.media.AudioManager
import android.media.AudioPlaybackConfiguration
import java.util.Collections
import java.util.WeakHashMap
import kotlin.math.abs
import kotlin.math.sign

interface Ducked {
    fun ducked()
}

object Ducking {
    const val STEP = 0.1f
    const val STEP_MS = 50L

    fun others(active: Int, ours: Int) = active > ours

    fun next(gain: Float, target: Float) = if (abs(target - gain) <= STEP) target else gain + sign(target - gain) * STEP
}

object Duck : AudioManager.AudioPlaybackCallback() {
    @Volatile
    var gain = 1f
        private set
    private var audio: AudioManager? = null
    private var target = 1f
    private val wanting = mutableSetOf<Any>()
    private val listeners = Collections.newSetFromMap(WeakHashMap<Ducked, Boolean>())
    private val ramp = Runnable { step() }

    fun listen(owner: Ducked) = Render.post { listeners += owner }

    fun unlisten(owner: Ducked) = Render.post { listeners -= owner }

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

    override fun onPlaybackConfigChanged(configs: List<AudioPlaybackConfiguration>) =
        aim(if (Ducking.others(configs.size, wanting.size)) 0f else 1f)

    private fun aim(goal: Float) {
        target = goal
        Render.cancel(ramp)
        step()
    }

    private fun step() {
        if (gain == target) return
        gain = Ducking.next(gain, target)
        Native.duck(gain)
        listeners.toList().forEach(Ducked::ducked)
        if (gain != target) Render.later(Ducking.STEP_MS, ramp)
    }

    private const val KEY = "playback.duck"
}
