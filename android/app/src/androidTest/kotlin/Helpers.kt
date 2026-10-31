package io.github.wallpaper_rs

import android.graphics.Bitmap
import android.os.ParcelFileDescriptor
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.util.concurrent.atomic.AtomicInteger
import org.json.JSONObject

const val IMAGE = "image"
const val VIDEO = "video"

fun storage(): File = InstrumentationRegistry.getInstrumentation().targetContext.cacheDir.resolve("render-test")
    .apply { mkdirs() }
    .also { Native.prepare(it.path) }

fun still(color: Int): File = File(storage(), "still.png").apply {
    outputStream().use { out ->
        Bitmap.createBitmap(8, 8, Bitmap.Config.ARGB_8888).apply { eraseColor(color) }
            .compress(Bitmap.CompressFormat.PNG, 100, out)
    }
}

fun fixture(name: String): File = File(storage(), name).apply {
    InstrumentationRegistry.getInstrumentation().context.assets.open(name).use { input ->
        outputStream().use { input.copyTo(it) }
    }
}

fun install(file: File): String {
    val item = ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
        .use { JSONObject(Native.store(it.fd, file.name)!!) }
    Native.library(JSONObject(mapOf("op" to "pick", "choice" to choice("item", item.getString("uid")))).toString())
    Render.call { Native.commit(BOTH) }
    return item.getString("kind")
}

fun settled(frames: AtomicInteger) = Thread.sleep(300).let { frames.get() }

fun quiet(frames: AtomicInteger) = Thread.sleep(1500).let { frames.get() }

fun waitFor(done: () -> Boolean): Boolean {
    val end = System.currentTimeMillis() + 20_000
    while (!done() && System.currentTimeMillis() < end) Thread.sleep(20)
    return done()
}
