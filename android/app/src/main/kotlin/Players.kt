package io.github.wallpaper_rs

import android.content.ComponentName
import android.content.Context
import android.graphics.Bitmap
import android.media.MediaMetadata
import android.media.session.MediaController
import android.media.session.MediaSessionManager
import android.media.session.PlaybackState
import android.os.SystemClock
import android.provider.Settings
import org.json.JSONObject

class Pictured {
    private var sent = false

    fun read(fresh: Boolean): Boolean = (fresh || !sent).also { sent = true }

    fun forget() {
        sent = false
    }
}

class Players(private val context: Context) : MediaSessionManager.OnActiveSessionsChangedListener {
    private val listeners = mutableSetOf<Painter>()
    private val component = ComponentName(context, Listener::class.java)
    private var manager: MediaSessionManager? = null
    private var controller: MediaController? = null
    private val pictured = Pictured()
    private val callback = object : MediaController.Callback() {
        override fun onMetadataChanged(metadata: MediaMetadata?) = send(true)
        override fun onPlaybackStateChanged(state: PlaybackState?) = send(false)
        override fun onSessionDestroyed() = pick(null)
    }

    fun follow(painter: Painter, wanted: Boolean) {
        if (wanted) listeners += painter else listeners -= painter
        val on = listeners.isNotEmpty() && allowed(context)
        if (on == (manager != null)) return
        if (!on) {
            manager?.removeOnActiveSessionsChangedListener(this)
            manager = null
            return pick(null)
        }
        val sessions = context.getSystemService(MediaSessionManager::class.java) ?: return
        runCatching {
            sessions.addOnActiveSessionsChangedListener(this, component, Render.handler)
            manager = sessions
            onActiveSessionsChanged(sessions.getActiveSessions(component))
        }.onFailure { Native.media(null, null, 0, false) }
    }

    override fun onActiveSessionsChanged(controllers: List<MediaController>?) {
        val playing = controllers?.firstOrNull { it.playbackState?.state == PlaybackState.STATE_PLAYING }
        pick(playing ?: controllers?.firstOrNull())
    }

    private fun pick(next: MediaController?) {
        if (next?.sessionToken == controller?.sessionToken && next != null) return send(false)
        controller?.unregisterCallback(callback)
        controller = next
        pictured.forget()
        next?.registerCallback(callback, Render.handler)
        send(true)
    }

    private fun send(fresh: Boolean) {
        val chosen = controller
        val metadata = chosen?.metadata
        if (chosen == null || metadata == null || manager == null) {
            pictured.forget()
            Native.media(null, null, 0, manager != null && allowed(context))
        } else {
            val state = chosen.playbackState
            val same = !pictured.read(fresh)
            val picture = if (same) null else metadata.getBitmap(MediaMetadata.METADATA_KEY_ALBUM_ART) ?: metadata.getBitmap(MediaMetadata.METADATA_KEY_ART)
            val text = { key: String -> metadata.getString(key).orEmpty() }
            val told = JSONObject()
                .put("title", text(MediaMetadata.METADATA_KEY_TITLE))
                .put("artist", text(MediaMetadata.METADATA_KEY_ARTIST))
                .put("album", text(MediaMetadata.METADATA_KEY_ALBUM))
                .put("album_artist", text(MediaMetadata.METADATA_KEY_ALBUM_ARTIST))
                .put("genres", text(MediaMetadata.METADATA_KEY_GENRE))
                .put("playing", state?.state == PlaybackState.STATE_PLAYING)
                .put("position", position(state) / 1000.0)
                .put("duration", metadata.getLong(MediaMetadata.METADATA_KEY_DURATION).coerceAtLeast(0) / 1000.0)
                .put("rate", state?.playbackSpeed?.toDouble() ?: 1.0)
                .put("keep_cover", same)
            val small = picture?.let(::shrink)
            val pixels = small?.let { IntArray(it.width * it.height).also { all -> it.getPixels(all, 0, it.width, 0, 0, it.width, it.height) } }
            Native.media(told.toString(), pixels, small?.width ?: 0, true)
        }
        listeners.forEach(Painter::move)
    }

    private fun position(state: PlaybackState?): Long {
        state ?: return 0
        if (state.state != PlaybackState.STATE_PLAYING) return state.position.coerceAtLeast(0)
        val spent = SystemClock.elapsedRealtime() - state.lastPositionUpdateTime
        return (state.position + spent * state.playbackSpeed).toLong().coerceAtLeast(0)
    }

    private fun shrink(picture: Bitmap): Bitmap {
        val side = maxOf(picture.width, picture.height)
        if (side <= SIDE) return picture.copy(Bitmap.Config.ARGB_8888, false)
        val (w, h) = maxOf(1, picture.width * SIDE / side) to maxOf(1, picture.height * SIDE / side)
        return Bitmap.createScaledBitmap(picture.copy(Bitmap.Config.ARGB_8888, false), w, h, true)
    }

    companion object {
        private const val SIDE = 256

        fun allowed(context: Context): Boolean {
            val enabled = Settings.Secure.getString(context.contentResolver, "enabled_notification_listeners").orEmpty()
            return enabled.split(':').any { ComponentName.unflattenFromString(it) == ComponentName(context, Listener::class.java) }
        }
    }
}
