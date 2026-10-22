package io.github.wallpaper_rs

import android.os.Handler
import android.os.HandlerThread
import android.view.Choreographer
import android.view.Surface
import java.util.concurrent.Callable
import java.util.concurrent.FutureTask

object Native {
    init {
        System.loadLibrary("wprs_android")
    }

    external fun prepare(files: String)

    external fun attach(surface: Surface): Long

    external fun resize(engine: Long, width: Int, height: Int)

    external fun draw(engine: Long, force: Boolean): Boolean

    external fun detach(engine: Long)

    external fun store(fd: Int): Boolean
}

object Render {
    private val handler = Handler(HandlerThread("wprs-render").apply { start() }.looper)

    fun post(block: () -> Unit) {
        handler.post { block() }
    }

    fun <T> call(block: () -> T): T = FutureTask(Callable(block)).also { handler.post(it) }.get()
}

fun interface Draw {
    fun frame(): Boolean
}

class Frames(private val draw: Draw) : Choreographer.FrameCallback {
    private var running = false

    fun start() {
        if (!running) Choreographer.getInstance().postFrameCallback(this)
        running = true
    }

    fun stop() {
        Choreographer.getInstance().removeFrameCallback(this)
        running = false
    }

    override fun doFrame(frameTimeNanos: Long) {
        running = draw.frame()
        if (running) Choreographer.getInstance().postFrameCallback(this)
    }
}
