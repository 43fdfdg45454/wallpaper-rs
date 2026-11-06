package io.github.wallpaper_rs

import android.content.Context
import android.hardware.display.DisplayManager
import android.hardware.display.VirtualDisplay
import android.hardware.display.VirtualDisplayConfig
import android.os.Build
import android.view.Surface

object Displays {
    fun make(context: Context, name: String, size: Pair<Int, Int>, dpi: Int, surface: Surface, fps: Int): VirtualDisplay {
        val manager = context.getSystemService(DisplayManager::class.java)
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.UPSIDE_DOWN_CAKE || fps <= 0) return manager.createVirtualDisplay(name, size.first, size.second, dpi, surface, 0)!!
        val config = VirtualDisplayConfig.Builder(name, size.first, size.second, dpi).setSurface(surface).setRequestedRefreshRate(fps.toFloat()).build()
        return manager.createVirtualDisplay(config, null, null)!!
    }
}
