package io.github.wallpaper_rs

import android.animation.ValueAnimator
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.display.DisplayManager
import android.os.BatteryManager
import android.os.Build
import android.os.PowerManager
import android.os.SystemClock
import android.view.Display
import androidx.compose.runtime.mutableIntStateOf
import androidx.core.content.ContextCompat

val powered = mutableIntStateOf(0)

class Power(private val context: Context) : BroadcastReceiver(), PowerManager.OnThermalStatusChangedListener {
    private val visible = mutableSetOf<Any>()
    private var headroom = 0f
    private var sampled = -HEADROOM_MS

    fun show(engine: Any, shown: Boolean) {
        val before = visible.isNotEmpty()
        if (shown) visible += engine else visible -= engine
        when {
            visible.isEmpty() -> if (before) stop()
            before -> update()
            else -> start()
        }
    }

    private fun start() {
        val filter = IntentFilter(Intent.ACTION_POWER_CONNECTED)
        filter.addAction(Intent.ACTION_POWER_DISCONNECTED)
        filter.addAction(Intent.ACTION_BATTERY_LOW)
        filter.addAction(Intent.ACTION_BATTERY_OKAY)
        filter.addAction(PowerManager.ACTION_POWER_SAVE_MODE_CHANGED)
        ContextCompat.registerReceiver(context, this, filter, ContextCompat.RECEIVER_NOT_EXPORTED)
        manager().addThermalStatusListener(context.mainExecutor, this)
        update()
    }

    private fun stop() {
        context.unregisterReceiver(this)
        manager().removeThermalStatusListener(this)
    }

    override fun onReceive(context: Context, intent: Intent) = update()

    override fun onThermalStatusChanged(status: Int) = update()

    private fun manager() = context.getSystemService(PowerManager::class.java)!!

    private fun update() {
        val power = manager()
        val battery = context.registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
        val now = SystemClock.elapsedRealtime()
        if (now - sampled >= HEADROOM_MS) {
            sampled = now
            headroom = power.getThermalHeadroom(FORECAST_SECONDS).takeUnless { it.isNaN() } ?: headroom
        }
        Native.power(
            (battery?.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) ?: 0) != 0,
            power.isPowerSaveMode,
            battery?.getBooleanExtra(BatteryManager.EXTRA_BATTERY_LOW, false) == true,
            power.currentThermalStatus,
            headroom,
            !ValueAnimator.areAnimatorsEnabled(),
        )
        display()
        powered.intValue++
        Render.post { Wallpaper.wake() }
    }

    private fun display() {
        val shown = context.getSystemService(DisplayManager::class.java)?.getDisplay(Display.DEFAULT_DISPLAY) ?: return
        val ratio = if (Build.VERSION.SDK_INT >= 34 && shown.isHdrSdrRatioAvailable) shown.hdrSdrRatio else 0f
        @Suppress("DEPRECATION")
        val peak = shown.hdrCapabilities?.desiredMaxLuminance ?: 0f
        Native.display(shown.isHdr, peak, ratio, shown.refreshRate)
    }

    private companion object {
        const val HEADROOM_MS = 10_000L
        const val FORECAST_SECONDS = 10
    }
}
