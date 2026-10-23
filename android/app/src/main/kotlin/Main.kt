package io.github.wallpaper_rs

import android.app.Activity
import android.app.WallpaperManager
import android.content.ComponentName
import android.content.Intent
import android.net.Uri
import android.os.Build
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
import androidx.compose.ui.unit.IntSize
import kotlin.concurrent.thread
import kotlinx.coroutines.MainScope
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
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
    private val pick = registerForActivityResult(PickMultipleVisualMedia()) { store(it) }
    private val files = registerForActivityResult(OpenMultipleDocuments()) { store(it) }
    private val folder = registerForActivityResult(OpenDocumentTree()) { uri -> uri?.let(::importFolder) }
    private val packages = registerForActivityResult(OpenDocument()) { uri -> uri?.let { store(listOf(it)) } }
    private val preview = registerForActivityResult(StartActivityForResult()) {
        if (active()) commit() else if (it.resultCode != Activity.RESULT_OK) Wallpaper.commitWhenLive = false
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        Native.prepare(noBackupFilesDir.path)
        if (savedInstanceState == null) Sync.soon(this, 0)
        setContent { App(this) }
    }

    override fun onStart() {
        super.onStart()
        power.show(this, true)
    }

    override fun onStop() {
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
                failure?.let { toast(it.message.orEmpty()) }
            }
        }
    }

    fun changed(edit: Boolean = true) {
        revision.intValue++
        Render.post { Wallpaper.reload(false) }
        if (edit) Sync.soon(this, EDITS)
    }

    fun act(edit: Boolean = true, work: suspend () -> Unit) = scope.launch {
        runCatching { work() }.onFailure { toast(it.message.orEmpty()) }
        changed(edit)
    }

    fun apply() = if (active()) commit() else useAsWallpaper()

    fun unpick(choice: JSONObject) {
        if (!isChangingConfigurations && !Wallpaper.commitWhenLive) Render.post {
            Native.library(JSONObject().put("op", "unpick").put("choice", choice).toString())
            Wallpaper.reload(false)
        }
    }

    fun toast(text: String) = Toast.makeText(this, text, Toast.LENGTH_LONG).show()

    private fun active() = Wallpaper.live > 0 || WallpaperManager.getInstance(this).let { manager ->
        val lock = if (Build.VERSION.SDK_INT >= 34) manager.getWallpaperInfo(WallpaperManager.FLAG_LOCK) else null
        listOfNotNull(manager.wallpaperInfo, lock).any { it.packageName == packageName }
    }

    private fun useAsWallpaper() {
        val wallpaper = ComponentName(this, Wallpaper::class.java)
        val intent = Intent(WallpaperManager.ACTION_CHANGE_LIVE_WALLPAPER)
            .putExtra(WallpaperManager.EXTRA_LIVE_WALLPAPER_COMPONENT, wallpaper)
        Wallpaper.commitWhenLive = true
        runCatching { preview.launch(intent) }.onFailure {
            Wallpaper.commitWhenLive = false
            toast(getString(R.string.unsupported))
        }
    }

    private fun commit() = Render.post {
        Wallpaper.commitWhenLive = false
        Wallpaper.reload(Native.commit())
        runOnUiThread {
            revision.intValue++
            toast(getString(R.string.applied))
        }
    }

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
        if (item.getString("kind") == "video") open(uri)?.use { poster(uid) { setDataSource(it.fileDescriptor) } }
        return true
    }

    private fun open(uri: Uri) = runCatching { contentResolver.openFileDescriptor(uri, "r") }.getOrNull()

    private fun name(uri: Uri) = runCatching {
        contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { row ->
            if (row.moveToFirst()) row.getString(0) else null
        }
    }.getOrNull().orEmpty()
}

private const val EDITS = 30L
private const val REFRESH = 10
