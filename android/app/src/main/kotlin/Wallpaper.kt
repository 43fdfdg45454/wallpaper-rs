package io.github.wallpaper_rs

import android.app.WallpaperColors
import android.service.wallpaper.WallpaperService
import android.view.MotionEvent
import android.view.SurfaceHolder

class Wallpaper : WallpaperService() {
    private val power = Power(this)
    private val videos = Videos(this)
    private val tilt by lazy { Tilt(this) }
    private val hearing by lazy { Hearing(this) }

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
            painter = Painter(isPreview, videos, tilt, hearing) { on -> mainExecutor.execute { setTouchEventsEnabled(on) } }
            if (!isPreview) live++
            if (!isPreview && commitWhenLive) Render.call {
                commitWhenLive = false
                Wallpaper.reload(Native.commit())
            }
            colors = Render.call { painter.colors() }
        }

        override fun onDestroy() {
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
                if (visible && !isPreview && Native.rotate()) Wallpaper.reload(Native.setting(COLORS, null) == ALWAYS)
                painter.show(visible)
            }
        }

        override fun onOffsetsChanged(x: Float, y: Float, xStep: Float, yStep: Float, xPixels: Int, yPixels: Int) =
            Render.post { painter.offset(if (xStep > 0f && xStep <= 1f) x else 0.5f) }

        override fun onTouchEvent(event: MotionEvent) {
            val (x, y, action) = Triple(event.x, event.y, event.actionMasked)
            Render.post { painter.touch(x, y, action != MotionEvent.ACTION_UP && action != MotionEvent.ACTION_CANCEL) }
        }

        override fun onSurfaceDestroyed(holder: SurfaceHolder) = Render.call {
            scenes -= this
            painter.detach()
        }

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
        private val scenes = mutableSetOf<Scene>()
        val previews = mutableSetOf<Painter>()
        var live = 0

        @Volatile
        var commitWhenLive = false

        fun reload(notify: Boolean) = scenes.forEach { it.reload(notify) }

        fun wake() {
            scenes.forEach(Scene::wake)
            previews.forEach(Painter::wake)
        }
    }
}
