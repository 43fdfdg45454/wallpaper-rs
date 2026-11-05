package io.github.wallpaper_rs

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import java.util.concurrent.atomic.AtomicLong
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import org.json.JSONObject

object Heard {
    const val SETTING = "sync_listen"
    const val SHOWN = "shown"
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val tokens = AtomicLong()
    private val wanted = mutableSetOf<Any>()
    private var token = 0L
    private var job: Job? = null
    @Volatile
    private var resting = false
    private var watching: Pair<ConnectivityManager, ConnectivityManager.NetworkCallback>? = null
    private const val SETTLE_MS = 2_000L
    private const val FIRST_MS = 30_000L
    private const val AGAIN_MS = 5_000L
    private const val LAST_MS = 900_000L

    @Volatile
    var conflict = "%1"

    @Volatile
    var applied: (() -> Unit)? = null

    val listening get() = synchronized(this) { job?.isActive == true }

    fun shown() = runCatching { Native.setting(SETTING, null) }.getOrNull() == SHOWN

    private fun ask(op: String, mine: Long) = runCatching {
        JSONObject(Native.account(JSONObject().put("op", op).put("token", mine).toString()))
    }.getOrNull()

    @Synchronized
    fun want(context: Context, who: Any, on: Boolean) {
        if (on) wanted += who else wanted -= who
        if (wanted.isEmpty()) stop() else start()
        watch(context, wanted.isNotEmpty())
    }

    private fun watch(context: Context, on: Boolean) {
        if (on == (watching != null)) return
        watching?.let { (manager, callback) -> runCatching { manager.unregisterNetworkCallback(callback) } }
        watching = null
        val manager = context.getSystemService(ConnectivityManager::class.java) ?: return
        if (on) watching = manager to object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) = again()
        }.also { callback -> runCatching { manager.registerDefaultNetworkCallback(callback) } }
    }

    @Synchronized
    private fun again() {
        if (!resting || wanted.isEmpty()) return
        job?.cancel()
        job = null
        start()
    }

    private fun start() {
        if (job?.isActive == true) return
        val mine = tokens.incrementAndGet().also { token = it }
        resting = false
        job = scope.launch {
            delay(SETTLE_MS)
            var wait = FIRST_MS
            while (isActive && Sync.active()) {
                val heard = ask("listen", mine) ?: break
                when {
                    heard.optBoolean("changed") -> {
                        wait = FIRST_MS
                        sync()
                    }
                    heard.optBoolean("opened") -> {
                        wait = FIRST_MS
                        delay(AGAIN_MS)
                    }
                    heard.has("error") || heard.optBoolean("ended") -> {
                        resting = true
                        delay(wait)
                        resting = false
                        wait = (wait * 2).coerceAtMost(LAST_MS)
                    }
                    else -> break
                }
            }
        }
    }

    private fun sync() {
        val request = JSONObject().put("op", "sync").put("conflict", conflict)
        val synced = runCatching { JSONObject(Native.account(request.toString())) }.getOrNull()
        if ((synced?.optInt("applied") ?: 0) == 0) return
        applied?.invoke() ?: Render.post { Wallpaper.reload(false) }
    }

    private fun stop() {
        job?.cancel() ?: return
        job = null
        ask("hush", token)
    }
}
