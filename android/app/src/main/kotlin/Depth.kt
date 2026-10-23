package io.github.wallpaper_rs

import android.os.ParcelFileDescriptor
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.net.URI
import java.util.concurrent.Executors
import javax.net.ssl.HttpsURLConnection
import org.json.JSONObject

object Depth {
    val working = mutableStateOf<String?>(null)
    val progress = mutableFloatStateOf(-1f)
    val failure = mutableStateOf<Pair<String, String>?>(null)

    @Volatile
    var cancelled = false
    private val worker = Executors.newSingleThreadExecutor { Thread(it, "wprs-depth") }

    fun make(uid: String, library: String) = run(uid) {
        val model = depth("model") as JSONObject
        if (!model.getBoolean("installed")) {
            check(model.getBoolean("allowed")) { "downloading the model is turned off" }
            fetch(model)
            progress.floatValue = -1f
            depth("install")
        }
        depth("make", "uid" to uid, "library" to library)
    }

    fun import(uid: String, file: ParcelFileDescriptor) =
        run(uid) { file.use { depth("import", "uid" to uid, "fd" to it.fd) } }

    private fun run(uid: String, work: () -> Unit) {
        working.value = uid
        progress.floatValue = -1f
        cancelled = false
        failure.value = null
        worker.execute {
            val failed = runCatching(work).exceptionOrNull()?.takeUnless { cancelled }
            failure.value = failed?.let { uid to (it.message ?: it.javaClass.simpleName) }
            working.value = null
            Render.post { Wallpaper.reload(false) }
        }
    }

    private fun fetch(model: JSONObject) {
        val urls = model.getJSONArray("urls")
        val part = File(model.getString("part")).apply { parentFile?.mkdirs() }
        var failed = IOException("no address to download from")
        repeat(urls.length()) {
            try {
                return get(urls.getString(it), part, model.getLong("bytes"), model.getString("agent"))
            } catch (e: IOException) {
                failed = e
            }
        }
        throw failed
    }

    private fun get(url: String, part: File, total: Long, agent: String) {
        if (part.length() > total) part.delete()
        val have = part.length()
        if (have == total) return
        val http = URI(url).toURL().openConnection() as HttpsURLConnection
        try {
            http.connectTimeout = TIMEOUT_MS
            http.readTimeout = TIMEOUT_MS
            http.setRequestProperty("User-Agent", agent)
            http.setRequestProperty("Accept-Encoding", "identity")
            if (have > 0) http.setRequestProperty("Range", "bytes=$have-")
            val code = http.responseCode
            if (code !in 200..299) throw IOException("${http.url.host} answered $code")
            val resumed = code == 206 && http.getHeaderField("Content-Range").orEmpty().startsWith("bytes $have-")
            var done = if (resumed) have else 0L
            FileOutputStream(part, resumed).use { out ->
                http.inputStream.use { input ->
                    val chunk = ByteArray(1 shl 16)
                    while (!cancelled) {
                        val read = input.read(chunk).takeIf { it >= 0 } ?: break
                        done += read
                        if (done > total) {
                            part.delete()
                            throw IOException("${http.url.host} sent more than $total bytes")
                        }
                        out.write(chunk, 0, read)
                        progress.floatValue = done.toFloat() / total
                    }
                }
                out.fd.sync()
            }
            if (done != total) throw IOException("the download stopped at $done of $total bytes; it resumes next time")
        } finally {
            http.disconnect()
        }
    }

    private const val TIMEOUT_MS = 60_000
}
