package io.github.wallpaper_rs

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.os.BatteryManager
import androidx.core.content.ContextCompat
import java.util.concurrent.atomic.AtomicLong
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import org.json.JSONObject

object Listening {
    const val FIRST_MS = 30_000L
    const val LAST_MS = 900_000L

    fun allowed(online: Boolean, unmetered: Boolean, any: Boolean, low: Boolean) = online && !low && (unmetered || any)

    fun goes(active: Boolean, outdated: Boolean) = active && !outdated

    fun after(wait: Long, failed: Boolean) = if (failed) (wait * 2).coerceAtMost(LAST_MS) else FIRST_MS
}

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
    private var watching: Watch? = null
    private var online = false
    private var unmetered = false
    private var low = false
    private const val SETTLE_MS = 2_000L
    private const val AGAIN_MS = 5_000L
    private const val NETWORK = "sync_network"
    private const val ANY = "any"

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
        watch(context.applicationContext, wanted.isNotEmpty())
        settle()
    }

    @Synchronized
    fun recheck() = settle()

    private fun fits() = Listening.allowed(online, unmetered, runCatching { Native.setting(NETWORK, null) }.getOrNull() == ANY, low)

    private fun settle() = if (wanted.isNotEmpty() && fits()) start() else stop()

    private class Watch(val context: Context, val manager: ConnectivityManager) : ConnectivityManager.NetworkCallback() {
        val battery = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) = drained(intent.action == Intent.ACTION_BATTERY_LOW)
        }

        override fun onAvailable(network: Network) = again()

        override fun onCapabilitiesChanged(network: Network, capabilities: NetworkCapabilities) =
            reached(true, capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED))

        override fun onLost(network: Network) = reached(false, false)
    }

    private fun watch(context: Context, on: Boolean) {
        if (on == (watching != null)) return
        watching?.let { watch ->
            runCatching { watch.manager.unregisterNetworkCallback(watch) }
            runCatching { watch.context.unregisterReceiver(watch.battery) }
        }
        watching = null
        val manager = context.getSystemService(ConnectivityManager::class.java) ?: return
        if (!on) return
        val watch = Watch(context, manager).also { watching = it }
        val capabilities = manager.getNetworkCapabilities(manager.activeNetwork)
        online = capabilities != null
        unmetered = capabilities?.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED) == true
        low = context.registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
            ?.getBooleanExtra(BatteryManager.EXTRA_BATTERY_LOW, false) == true
        val filter = IntentFilter(Intent.ACTION_BATTERY_LOW).apply { addAction(Intent.ACTION_BATTERY_OKAY) }
        runCatching { ContextCompat.registerReceiver(context, watch.battery, filter, ContextCompat.RECEIVER_NOT_EXPORTED) }
        runCatching { manager.registerDefaultNetworkCallback(watch) }
    }

    @Synchronized
    private fun reached(on: Boolean, free: Boolean) {
        if (on == online && free == unmetered) return
        online = on
        unmetered = free
        settle()
    }

    @Synchronized
    private fun drained(drained: Boolean) {
        if (drained == low) return
        low = drained
        settle()
    }

    @Synchronized
    private fun again() {
        if (!resting || wanted.isEmpty() || !fits()) return
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
            var wait = Listening.FIRST_MS
            while (isActive && goes()) {
                val heard = ask("listen", mine) ?: break
                val failed = when {
                    heard.optBoolean("changed") -> !sync()
                    heard.optBoolean("opened") -> false.also { delay(AGAIN_MS) }
                    heard.has("error") || heard.optBoolean("ended") -> true
                    else -> break
                }
                if (failed) {
                    resting = true
                    delay(wait)
                    resting = false
                }
                wait = Listening.after(wait, failed)
            }
        }
    }

    private fun goes() = runCatching { Sync.status() }.getOrNull()?.let { Listening.goes(it.optBoolean("active"), it.optBoolean("outdated")) } == true

    private fun sync(): Boolean {
        val request = JSONObject().put("op", "sync").put("conflict", conflict)
        val synced = runCatching { JSONObject(Native.account(request.toString())) }.getOrNull()
        if (synced == null || synced.has("error")) return false
        if (synced.optInt("applied") > 0) {
            watching?.context?.let(Updates::schedule)
            applied?.invoke() ?: Render.post { Wallpaper.reload(false) }
        }
        return true
    }

    private fun stop() {
        job?.cancel() ?: return
        job = null
        ask("hush", token)
    }
}
