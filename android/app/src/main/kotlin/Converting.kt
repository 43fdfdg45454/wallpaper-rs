package io.github.wallpaper_rs

import android.content.Context
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.os.ParcelFileDescriptor
import androidx.annotation.OptIn
import androidx.compose.runtime.mutableStateMapOf
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.util.UnstableApi
import androidx.media3.effect.Presentation
import androidx.media3.transformer.Composition
import androidx.media3.transformer.EditedMediaItem
import androidx.media3.transformer.Effects
import androidx.media3.transformer.ExportException
import androidx.media3.transformer.ExportResult
import androidx.media3.transformer.ProgressHolder
import androidx.media3.transformer.Transformer
import java.io.File
import kotlin.concurrent.thread
import org.json.JSONObject
import org.json.JSONTokener

class Conversion(val uid: String, val path: String, val title: String, val keep: Boolean, val size: Pair<Int, Int>) {
    var target = ""
}

@OptIn(UnstableApi::class)
object Converting {
    val progress = mutableStateMapOf<String, Float>()
    val names = mutableMapOf<String, String>()
    private val main = Handler(Looper.getMainLooper())
    private val queue = ArrayDeque<Conversion>()
    private var running: Pair<Conversion, Transformer>? = null
    private var app: Context? = null
    private const val POLL_MS = 500L

    @Volatile
    var finished: ((String) -> Unit)? = null

    private fun library(op: String, uid: String): String? =
        JSONTokener(Native.library(JSONObject().put("op", op).put("uid", uid).toString())).nextValue() as? String

    fun offer(context: Context, conversion: Conversion) = main.post {
        if (conversion.uid in progress) return@post
        app = context.applicationContext
        names[conversion.uid] = conversion.title
        progress[conversion.uid] = 0f
        queue += conversion
        next()
    }

    fun stop(uid: String) = main.post {
        if (queue.removeAll { it.uid == uid }) progress.remove(uid)
        val (conversion, transformer) = running?.takeIf { it.first.uid == uid } ?: return@post
        transformer.cancel()
        settle(conversion, false)
    }

    private fun next() {
        val context = app ?: return
        if (running != null) return
        val conversion = queue.removeFirstOrNull() ?: return
        val target = runCatching { library("convert_to", conversion.uid) }.getOrNull() ?: return settle(conversion, false)
        val transformer = Transformer.Builder(context).setVideoMimeType(MimeTypes.VIDEO_H264).addListener(object : Transformer.Listener {
            override fun onCompleted(composition: Composition, result: ExportResult) = settle(conversion, true)

            override fun onError(composition: Composition, result: ExportResult, error: ExportException) = settle(conversion, false)
        }).build()
        val (width, height) = conversion.size
        val effects = Effects(emptyList(), listOf(Presentation.createForWidthAndHeight(width, height, Presentation.LAYOUT_SCALE_TO_FIT)))
        val item = EditedMediaItem.Builder(MediaItem.fromUri(Uri.fromFile(File(conversion.path)))).setEffects(effects).build()
        running = conversion to transformer
        conversion.target = target
        transformer.start(item, target)
        main.postDelayed(poll, POLL_MS)
    }

    private val poll = object : Runnable {
        override fun run() {
            val (conversion, transformer) = running ?: return
            val holder = ProgressHolder()
            if (transformer.getProgress(holder) == Transformer.PROGRESS_STATE_AVAILABLE) progress[conversion.uid] = holder.progress / 100f
            main.postDelayed(this, POLL_MS)
        }
    }

    private fun settle(conversion: Conversion, made: Boolean) {
        if (running?.first === conversion) running = null
        main.removeCallbacks(poll)
        thread(isDaemon = true) {
            val placed = made && runCatching { place(conversion) }.getOrDefault(false)
            if (placed) Render.post { Wallpaper.reload(false) } else runCatching { library("unconverted", conversion.uid) }
            main.post {
                progress.remove(conversion.uid)
                finished?.invoke(conversion.uid)
                next()
            }
        }
    }

    private fun place(conversion: Conversion): Boolean {
        if (!conversion.keep) return library("converted", conversion.uid)?.let { path -> poster(conversion.uid) { setDataSource(path) } } != null
        val made = File(conversion.target)
        val name = app?.getString(R.string.converted_name, conversion.title) + ".mp4"
        val stored = ParcelFileDescriptor.open(made, ParcelFileDescriptor.MODE_READ_ONLY).use { JSONObject(Native.store(it.fd, name)!!) }
        poster(stored.getString("uid")) { setDataSource(made.path) }
        library("unconverted", conversion.uid)
        return true
    }
}
