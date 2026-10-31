package io.github.wallpaper_rs

import android.app.Activity
import android.net.Uri
import android.os.Bundle
import android.provider.OpenableColumns
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts.OpenDocument
import androidx.activity.result.contract.ActivityResultContracts.OpenDocumentTree
import androidx.activity.result.contract.ActivityResultContracts.OpenMultipleDocuments
import androidx.activity.result.contract.ActivityResultContracts.PickMultipleVisualMedia
import androidx.activity.result.contract.ActivityResultContracts.PickVisualMedia
import androidx.activity.result.contract.ActivityResultContracts.StartActivityForResult
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.unit.IntSize
import kotlin.concurrent.thread
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject

class Main : ComponentActivity() {
    val revision = mutableIntStateOf(0)
    val importing = mutableIntStateOf(0)
    val imports = mutableIntStateOf(0)
    val videos by lazy { Videos(applicationContext) }
    val tilt by lazy { Tilt(applicationContext) }
    val hearing by lazy { Hearing(applicationContext) }
    private val power by lazy { Power(applicationContext) }
    private val scope = MainScope()
    private val heard by lazy { Heard(this, scope) }
    private val pick = registerForActivityResult(PickMultipleVisualMedia()) { store(it) }
    private val files = registerForActivityResult(OpenMultipleDocuments()) { store(it) }
    private val folder = registerForActivityResult(OpenDocumentTree()) { uri -> uri?.let(::importFolder) }
    private val packages = registerForActivityResult(OpenDocument()) { uri -> uri?.let { store(listOf(it)) } }
    val placing = mutableStateOf(false)
    val preview = registerForActivityResult(StartActivityForResult()) { placed(it.resultCode == Activity.RESULT_OK) }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        Native.prepare(noBackupFilesDir.path)
        Unveil.load()
        if (savedInstanceState == null) {
            Sync.soon(this, 0)
            remirror()
        }
        setContent { App(this) }
    }

    override fun onStart() {
        super.onStart()
        power.show(this, true)
        heard.start()
    }

    override fun onStop() {
        heard.stop()
        power.show(this, false)
        super.onStop()
    }

    override fun onDestroy() {
        scope.cancel()
        forget()
        super.onDestroy()
    }

    fun choose() = pick.launch(PickVisualMediaRequest(PickVisualMedia.ImageAndVideo))

    fun chooseFiles() = files.launch(arrayOf("image/*", "video/*"))

    fun chooseFolder() = folder.launch(null)

    fun openPackage() = packages.launch(arrayOf("*/*"))

    fun wallpaperSize(): IntSize {
        val bounds = windowManager.maximumWindowMetrics.bounds
        val turned = display.rotation % 2 == 1
        return if (turned) IntSize(bounds.height(), bounds.width()) else IntSize(bounds.width(), bounds.height())
    }

    fun examples() {
        importing.intValue++
        thread {
            val failure = runCatching { examples(this) }.exceptionOrNull()
            runOnUiThread {
                importing.intValue--
                revision.intValue++
                failure?.let(::toast)
            }
        }
    }

    fun changed(edit: Boolean = true) {
        revision.intValue++
        Render.post { Wallpaper.reload(false) }
        if (edit) Sync.soon(this, EDITS)
    }

    fun act(edit: Boolean = true, work: suspend () -> Unit) = scope.launch {
        runCatching { work() }.onFailure(::toast)
        changed(edit)
    }

    fun unpick(choice: JSONObject) {
        if (!isChangingConfigurations && Wallpaper.commitWhenLive == null) Render.post {
            Native.library(JSONObject().put("op", "unpick").put("choice", choice).toString())
            Wallpaper.reload(false)
        }
    }

    fun toast(text: String) =
        Toast.makeText(this, text.ifBlank { getString(R.string.failed_without_reason) }, Toast.LENGTH_LONG).show()

    fun toast(error: Throwable) = toast(told(error))

    fun store(uris: List<Uri>) {
        if (uris.isEmpty()) return
        importing.intValue += uris.size
        imports.intValue += uris.size
        thread {
            val failed = uris.withIndex().count { (index, uri) ->
                val stored = runCatching { store(uri) }.getOrDefault(false)
                runOnUiThread {
                    importing.intValue--
                    if (index % REFRESH == 0 || index == uris.lastIndex) revision.intValue++
                    if (importing.intValue == 0) imports.intValue = 0
                }
                !stored
            }
            if (failed > 0) runOnUiThread { toast(resources.getQuantityString(R.plurals.unreadable, failed, failed)) }
        }
    }

    private fun store(uri: Uri): Boolean {
        val item = open(uri)?.use { Native.store(it.fd, name(uri)) }?.let(::JSONObject) ?: return false
        val uid = item.getString("uid")
        val played = when {
            item.getString("kind") == "video" -> open(uri)?.use { file ->
                playable { setDataSource(file.fileDescriptor) } && poster(uid) { setDataSource(file.fileDescriptor) } != null
            } == true
            !item.isNull("poster") -> item.getString("poster").let { file -> playable { setDataSource(file) } && poster(uid) { setDataSource(file) } != null }
            else -> true
        }
        if (!played) Native.library(JSONObject().put("op", "remove").put("uids", JSONArray(listOf(uid))).toString())
        return played
    }

    private fun open(uri: Uri) = runCatching { contentResolver.openFileDescriptor(uri, "r") }.getOrNull()

    fun name(uri: Uri) = runCatching {
        contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { row ->
            if (row.moveToFirst()) row.getString(0) else null
        }
    }.getOrNull().orEmpty()
}

private const val EDITS = 30L
private const val REFRESH = 10
