package io.github.wallpaper_rs

import java.util.concurrent.atomic.AtomicLong
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import org.json.JSONObject

class Heard(private val main: Main, private val scope: CoroutineScope) {
    private var token = 0L
    private var job: Job? = null

    private fun ask(op: String, mine: Long) = runCatching {
        JSONObject(Native.account(JSONObject().put("op", op).put("token", mine).toString()))
    }.getOrNull()

    fun start() {
        if (job?.isActive == true) return
        val mine = TOKENS.incrementAndGet().also { token = it }
        job = scope.launch(Dispatchers.IO) {
            var wait = FIRST_MS
            while (isActive && Sync.active()) {
                val heard = ask("listen", mine) ?: break
                when {
                    heard.optBoolean("changed") -> {
                        wait = FIRST_MS
                        val request = JSONObject().put("op", "sync").put("conflict", main.getString(R.string.conflict, "%1"))
                        val synced = runCatching { JSONObject(Native.account(request.toString())) }.getOrNull()
                        if ((synced?.optInt("applied") ?: 0) > 0) main.runOnUiThread { main.changed(false) }
                    }
                    heard.optBoolean("opened") -> {
                        wait = FIRST_MS
                        delay(AGAIN_MS)
                    }
                    heard.has("error") || heard.optBoolean("ended") -> {
                        delay(wait)
                        wait = (wait * 2).coerceAtMost(LAST_MS)
                    }
                    else -> break
                }
            }
        }
    }

    fun stop() {
        job?.cancel()
        job = null
        ask("hush", token)
    }

    private companion object {
        val TOKENS = AtomicLong()
        const val FIRST_MS = 30_000L
        const val AGAIN_MS = 5_000L
        const val LAST_MS = 900_000L
    }
}
