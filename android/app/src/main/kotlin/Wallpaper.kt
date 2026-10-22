package io.github.wallpaper_rs

import android.service.wallpaper.WallpaperService
import android.view.SurfaceHolder

class Wallpaper : WallpaperService() {
    override fun onCreate() {
        super.onCreate()
        Native.prepare(noBackupFilesDir.path)
    }

    override fun onCreateEngine(): Engine = Scene()

    inner class Scene : Engine() {
        private var handle = 0L
        private var shown = false
        private val frames = Frames { Native.draw(handle, false) }

        override fun onSurfaceCreated(holder: SurfaceHolder) = Render.call {
            handle = Native.attach(holder.surface)
            scenes += this
        }

        override fun onSurfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) =
            Render.call { Native.resize(handle, width, height) }

        override fun onSurfaceRedrawNeeded(holder: SurfaceHolder) = Render.call { redraw(true) }

        override fun onVisibilityChanged(visible: Boolean) = Render.post {
            shown = visible
            if (visible) redraw(false) else frames.stop()
        }

        override fun onSurfaceDestroyed(holder: SurfaceHolder) = Render.call {
            scenes -= this
            frames.stop()
            Native.detach(handle)
            handle = 0L
        }

        fun reload() {
            if (shown) redraw(false)
        }

        private fun redraw(force: Boolean) {
            if (Native.draw(handle, force) && shown) frames.start()
        }
    }

    companion object {
        private val scenes = mutableSetOf<Scene>()

        fun reload() = scenes.forEach(Scene::reload)
    }
}
