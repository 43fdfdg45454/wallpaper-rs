package io.github.wallpaper_rs

import android.app.WallpaperManager
import android.content.ComponentName
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.view.Gravity
import android.widget.Button
import android.widget.LinearLayout
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts.PickVisualMedia

class Main : ComponentActivity() {
    private val pick = registerForActivityResult(PickVisualMedia()) { uri -> if (uri != null) store(uri) }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Native.prepare(noBackupFilesDir.path)
        setContentView(LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            addView(button(R.string.choose) { pick.launch(PickVisualMediaRequest(PickVisualMedia.ImageOnly)) })
            addView(button(R.string.apply) { useAsWallpaper() })
        })
    }

    private fun button(label: Int, click: () -> Unit) = Button(this).apply {
        setText(label)
        setOnClickListener { click() }
    }

    private fun useAsWallpaper() {
        val wallpaper = ComponentName(this, Wallpaper::class.java)
        val intent = Intent(WallpaperManager.ACTION_CHANGE_LIVE_WALLPAPER)
            .putExtra(WallpaperManager.EXTRA_LIVE_WALLPAPER_COMPONENT, wallpaper)
        runCatching { startActivity(intent) }.onFailure { toast(R.string.unsupported) }
    }

    private fun store(uri: Uri) {
        val file = runCatching { contentResolver.openFileDescriptor(uri, "r") }.getOrNull()
        Render.post {
            val stored = file?.use { Native.store(it.fd) } == true
            if (stored) Wallpaper.reload()
            runOnUiThread { toast(if (stored) R.string.stored else R.string.unreadable) }
        }
    }

    private fun toast(text: Int) = Toast.makeText(this, text, Toast.LENGTH_SHORT).show()
}
