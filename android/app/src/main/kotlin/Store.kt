package io.github.wallpaper_rs

import android.content.Context
import android.content.res.AssetManager
import android.graphics.BitmapFactory
import android.media.MediaMetadataRetriever
import android.media.MediaMetadataRetriever.METADATA_KEY_DURATION
import android.media.MediaMetadataRetriever.METADATA_KEY_HAS_AUDIO
import android.media.MediaMetadataRetriever.METADATA_KEY_VIDEO_HEIGHT
import android.media.MediaMetadataRetriever.METADATA_KEY_VIDEO_ROTATION
import android.media.MediaMetadataRetriever.METADATA_KEY_VIDEO_WIDTH
import android.media.MediaMetadataRetriever.OPTION_CLOSEST_SYNC
import android.util.LruCache
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.produceState
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File
import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONTokener

class Item(json: JSONObject) {
    val uid: String = json.getString("uid")
    val title: String = json.getString("title")
    val path: String = json.getString("path")
    val video = json.getString("kind") == "video"
    val scene = json.getString("kind") == "package"
    val thumb: String? = if (json.isNull("thumb")) null else json.getString("thumb")
    val width = json.optInt("width")
    val height = json.optInt("height")
    val millis = json.optInt("duration_ms")
    val favorite = json.optBoolean("favorite")
    private val details = json.optJSONObject("about")
    val about: String = details?.optString("description").orEmpty()
    val animated: String? = details?.takeUnless { it.isNull("animated") }?.optString("animated")?.takeIf { it.isNotEmpty() }
    val refused = (details?.optJSONArray("refused")?.length() ?: 0) > 0
    val lacks = json.optJSONArray("lacks")?.let { list -> List(list.length(), list::getString) }.orEmpty()
    private val pack = json.optJSONObject("package")
    private val confirmed = pack?.optJSONArray("confirmed")?.let { list -> List(list.length(), list::getString) }.orEmpty()
    val mature = pack?.optString("content") == "mature" && "mature" !in confirmed
    val flashing = pack?.optBoolean("photosensitive") == true && "photosensitive" !in confirmed
}

class Playlist(json: JSONObject) {
    val info: JSONObject = json.getJSONObject("playlist")
    val count = json.getInt("count")
    val current = json.getBoolean("current")
    val uid: String = info.getString("uid")
    val name: String = info.getString("name")
    val rules: JSONObject = info.getJSONObject("rules")
}

fun Playlist.ruled(key: String, value: Any): JSONObject =
    JSONObject(info.toString()).apply { getJSONObject("rules").put(key, value) }

class Refused(message: String) : Exception(message)

private fun answer(native: (String) -> String, op: String, args: Array<out Pair<String, Any?>>): Any? {
    val answer = JSONTokener(native(JSONObject(mapOf("op" to op, *args)).toString())).nextValue()
    if (answer is JSONObject && answer.length() == 1 && answer.opt("error") is String) throw Refused(answer.getString("error"))
    return answer.takeUnless { it == JSONObject.NULL }
}

suspend fun ask(op: String, vararg args: Pair<String, Any?>): Any? =
    withContext(Dispatchers.IO) { answer(Native::library, op, args) }

fun depth(op: String, vararg args: Pair<String, Any?>): Any? = answer(Native::depth, op, args)

fun examples(context: Context) {
    val root = File(context.cacheDir, "examples").apply { deleteRecursively() }
    unpack(context.assets, "scenes", root)
    try {
        answer(Native::library, "examples", arrayOf("dir" to root.path))
    } finally {
        root.deleteRecursively()
    }
}

private fun unpack(assets: AssetManager, path: String, to: File) {
    val names = assets.list(path).orEmpty()
    if (names.isEmpty()) assets.open(path).use { input -> to.outputStream().use { input.copyTo(it) } }
    else names.forEach { unpack(assets, "$path/$it", File(to.apply { mkdirs() }, it)) }
}

suspend fun account(op: String, vararg args: Pair<String, Any?>): Any? =
    withContext(Dispatchers.IO) { answer(Native::account, op, args) }

suspend fun fetch(op: String, vararg args: Pair<String, Any?>) =
    (ask(op, *args) as? JSONArray)?.let { list -> List(list.length()) { Item(list.getJSONObject(it)) } }.orEmpty()

suspend fun playlists() =
    (ask("playlists") as? JSONArray)?.let { list -> List(list.length()) { Playlist(list.getJSONObject(it)) } }.orEmpty()

fun choice(kind: String, uid: String): JSONObject = JSONObject().put(kind, uid)

@Composable
fun <T> loaded(initial: T, vararg keys: Any?, load: suspend () -> T): State<T> =
    produceState(initial, *keys) {
        value = runCatching { withContext(Dispatchers.IO) { load() } }.getOrDefault(initial)
    }

private val thumbs = object : LruCache<String, ImageBitmap>(BYTES) {
    override fun sizeOf(key: String, value: ImageBitmap) = value.width * value.height * 4
}

fun forget() = thumbs.evictAll()

@Composable
fun thumbnail(item: Item): State<ImageBitmap?> = produceState(thumbs[item.uid], item.uid, item.thumb) {
    value = value ?: runCatching { withContext(Dispatchers.IO) { picture(item) } }.getOrNull()
}

private suspend fun picture(item: Item): ImageBitmap? {
    val made = suspend { ask("thumb", "uid" to item.uid) as? String }
    val path = item.thumb ?: made() ?: poster(item.uid) { setDataSource(item.path) }?.let { made() }
    return path?.let { BitmapFactory.decodeFile(it)?.asImageBitmap() }?.also { thumbs.put(item.uid, it) }
}

fun poster(uid: String, open: MediaMetadataRetriever.() -> Unit) = runCatching {
    MediaMetadataRetriever().use { retriever ->
        retriever.open()
        val frame = checkNotNull(retriever.getScaledFrameAtTime(0, OPTION_CLOSEST_SYNC, EDGE, EDGE))
        val pixels = IntArray(frame.width * frame.height)
        frame.getPixels(pixels, 0, frame.width, 0, 0, frame.width, frame.height)
        fun read(key: Int) = retriever.extractMetadata(key)?.toIntOrNull() ?: 0
        val size = intArrayOf(read(METADATA_KEY_VIDEO_WIDTH), read(METADATA_KEY_VIDEO_HEIGHT))
        if (read(METADATA_KEY_VIDEO_ROTATION) % 180 != 0) size.reverse()
        val audio = retriever.extractMetadata(METADATA_KEY_HAS_AUDIO) == "yes"
        Native.poster(uid, pixels, frame.width, size + read(METADATA_KEY_DURATION), audio)
    }
}.getOrNull()

private const val EDGE = 512
private const val BYTES = 32 shl 20
