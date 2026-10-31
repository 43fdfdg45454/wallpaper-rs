package io.github.wallpaper_rs

import android.os.Handler
import android.os.HandlerThread
import android.os.SystemClock
import android.view.Choreographer
import android.view.Surface
import java.util.concurrent.Callable
import java.util.concurrent.FutureTask

object Native {
    init {
        System.loadLibrary("wprs_android")
    }

    external fun prepare(files: String)

    external fun attach(surface: Surface, preview: Boolean, lock: Boolean): Long

    external fun resize(engine: Long, width: Int, height: Int)

    external fun offset(engine: Long, x: Float): Boolean

    external fun video(engine: Long): String?

    external fun draw(engine: Long, nanos: Long, force: Boolean): Long

    external fun detach(engine: Long)

    external fun power(charging: Boolean, saver: Boolean, low: Boolean, thermal: Int, headroom: Float, still: Boolean)

    external fun moving(): Boolean

    external fun parallax(engine: Long): Int

    external fun tilt(x: Float, y: Float, z: Float, rotation: Int): Boolean

    external fun turn(w: Float, x: Float, y: Float, z: Float, rotation: Int): Boolean

    external fun device(lowRam: Boolean, performance: Int)

    external fun screen(engine: Long, lock: Boolean)

    external fun visible(engine: Long, shown: Boolean)

    external fun inputs(engine: Long): Int

    external fun touch(engine: Long, x: Float, y: Float, down: Boolean): Boolean

    external fun hear(fft: ByteArray, rate: Int)

    external fun playback(active: Boolean)

    external fun clips(engine: Long): String?

    external fun clip(engine: Long, path: String, open: Boolean): Surface?

    external fun depth(request: String): String

    external fun store(fd: Int, name: String): String?

    external fun poster(item: String, pixels: IntArray, width: Int, size: IntArray, audio: Boolean)

    external fun library(request: String): String

    external fun account(request: String): String

    external fun commit(target: String): Boolean

    external fun rotate(lock: Boolean): Boolean

    external fun colors(preview: Boolean, lock: Boolean): IntArray?

    external fun setting(key: String, value: String?): String

    external fun diagnostics(): String

    external fun trim()

    external fun retry()

    external fun finish()
}

object Render {
    val handler = Handler(HandlerThread("wprs-render").apply { start() }.looper)

    fun post(block: () -> Unit) {
        handler.post { block() }
    }

    fun <T> call(block: () -> T): T = FutureTask(Callable(block)).also { handler.post(it) }.get()

    fun later(delay: Long, task: Runnable) {
        handler.postDelayed(task, delay)
    }

    fun cancel(task: Runnable) = handler.removeCallbacks(task)
}

fun interface Draw {
    fun frame(nanos: Long): Long
}

class Frames(private val draw: Draw) : Choreographer.FrameCallback {
    private var at = Long.MAX_VALUE

    fun start() {
        if (at > SystemClock.uptimeMillis() + SOON_MS) post(0L)
    }

    fun stop() {
        Choreographer.getInstance().removeFrameCallback(this)
        at = Long.MAX_VALUE
    }

    fun schedule(delay: Long) = if (delay > 0L) post(delay) else stop()

    private fun post(delay: Long) {
        stop()
        Choreographer.getInstance().postFrameCallbackDelayed(this, delay)
        at = SystemClock.uptimeMillis() + delay
    }

    override fun doFrame(frameTimeNanos: Long) = schedule(draw.frame(frameTimeNanos))
}

private const val SOON_MS = 100L
