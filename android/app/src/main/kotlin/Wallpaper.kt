package io.github.wallpaper_rs

import android.app.WallpaperColors
import android.app.WallpaperManager
import android.os.Build
import android.service.wallpaper.WallpaperService
import android.view.MotionEvent
import android.view.SurfaceHolder

class Wallpaper : WallpaperService() {
    private val power = Power(this)
    private val videos = Videos(this)
    private val tilt by lazy { Tilt(this) }
    private val hearing by lazy { Hearing(this) }
    private val pages by lazy { Pages(this) }

    override fun onCreate() {
        super.onCreate()
        Native.prepare(noBackupFilesDir.path)
    }

    override fun onCreateEngine(): Engine = Scene()

    override fun onDestroy() {
        Render.call { Native.finish() }
        super.onDestroy()
    }

    override fun onTrimMemory(level: Int) {
        super.onTrimMemory(level)
        Render.post {
            Native.trim()
            videos.trim()
        }
    }

    inner class Scene : Engine() {
        private lateinit var painter: Painter

        @Volatile
        private var colors: WallpaperColors? = null

        override fun onCreate(surfaceHolder: SurfaceHolder) {
            super.onCreate(surfaceHolder)
            painter = Painter(isPreview, videos, tilt, hearing, pages) { on -> mainExecutor.execute { setTouchEventsEnabled(on) } }
            if (!isPreview) live++
            if (!isPreview) screens[this] = flags()
            val lock = lock()
            Render.call { painter.screen(lock) }
            commitWhenLive?.takeIf { !isPreview }?.let { target ->
                commitWhenLive = null
                Render.call { Wallpaper.reload(Native.commit(target)) }
            }
            colors = Render.call { painter.colors() }
        }

        override fun onWallpaperFlagsChanged(which: Int) {
            super.onWallpaperFlagsChanged(which)
            if (isPreview) return
            screens[this] = which
            val lock = lock()
            Render.post {
                painter.screen(lock)
                reload(true)
            }
        }

        private fun flags() = if (Build.VERSION.SDK_INT >= 34) wallpaperFlags else BOTH_FLAGS

        private fun lock() = screens[this] == WallpaperManager.FLAG_LOCK

        override fun onDestroy() {
            screens -= this
            if (!isPreview) live--
            super.onDestroy()
        }

        override fun onSurfaceCreated(holder: SurfaceHolder) = Render.call {
            painter.attach(holder.surface)
            scenes += this
        }

        override fun onSurfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
            val rotation = displayContext?.display?.rotation ?: 0
            Render.call { painter.resize(width, height, rotation) }
        }

        override fun onSurfaceRedrawNeeded(holder: SurfaceHolder) = Render.call { painter.redraw() }

        override fun onVisibilityChanged(visible: Boolean) {
            power.show(this, visible)
            Render.post {
                if (visible && !isPreview && Native.rotate(painter.lock)) Wallpaper.reload(Native.setting(COLORS, null) == ALWAYS)
                painter.show(visible)
            }
        }

        override fun onOffsetsChanged(x: Float, y: Float, xStep: Float, yStep: Float, xPixels: Int, yPixels: Int) =
            Render.post { painter.offset(if (xStep > 0f && xStep <= 1f) x else 0.5f) }

        override fun onTouchEvent(event: MotionEvent) {
            if (pages.press(painter, event)) return
            val (x, y, action) = Triple(event.x, event.y, event.actionMasked)
            Render.post { painter.touch(x, y, action != MotionEvent.ACTION_UP && action != MotionEvent.ACTION_CANCEL) }
        }

        override fun onSurfaceDestroyed(holder: SurfaceHolder) = pages.drop(painter).let { _ -> Render.call {
            scenes -= this
            painter.detach()
        } }

        override fun onComputeColors(): WallpaperColors? = colors

        fun reload(notify: Boolean) {
            colors = painter.colors()
            painter.refresh()
            if (notify && !isPreview) mainExecutor.execute { notifyColorsChanged() }
        }

        fun wake() = painter.wake()
    }

    companion object {
        private const val COLORS = "system_colors"
        private const val ALWAYS = "always"
        private const val BOTH_FLAGS = WallpaperManager.FLAG_SYSTEM or WallpaperManager.FLAG_LOCK
        private val scenes = mutableSetOf<Scene>()
        private val screens = mutableMapOf<Scene, Int>()
        val previews = mutableSetOf<Painter>()
        var live = 0

        @Volatile
        var commitWhenLive: String? = null

        fun joined(target: String) = target != BOTH && Build.VERSION.SDK_INT >= 34 && screens.values.any { it == BOTH_FLAGS }

        fun reload(notify: Boolean) = scenes.forEach { it.reload(notify) }

        fun wake() {
            scenes.forEach(Scene::wake)
            previews.forEach(Painter::wake)
        }
    }
}
