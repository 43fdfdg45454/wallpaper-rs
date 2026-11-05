package io.github.wallpaper_rs

import android.content.Context
import android.os.Handler
import android.os.Looper
import androidx.compose.runtime.mutableIntStateOf
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.PeriodicWorkRequest
import androidx.work.WorkManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import java.util.concurrent.TimeUnit
import org.json.JSONArray
import org.json.JSONObject

class RepoCheck(context: Context, params: WorkerParameters) : Worker(context, params) {
    override fun doWork(): Result {
        val found = runCatching { JSONArray(Native.account(JSONObject().put("op", "refresh_repos").put("force", false).toString())) }
        Updates.found(found.getOrNull()?.length() ?: return Result.retry())
        return Result.success()
    }
}

object Updates {
    const val WORK = "repos"
    val count = mutableIntStateOf(0)
    private val main = Handler(Looper.getMainLooper())

    fun found(updates: Int) = main.post { count.intValue = updates }

    fun count() = runCatching { JSONArray(Native.account("{\"op\": \"updates\"}")).length() }.getOrDefault(0)

    fun repos() = runCatching { JSONObject(Native.account("{\"op\": \"repos\"}")).optJSONArray("repos")?.length() }.getOrNull() ?: 0

    fun schedule(context: Context, repos: Int = repos()) {
        val work = WorkManager.getInstance(context)
        if (repos == 0) {
            work.cancelUniqueWork(WORK)
            return
        }
        val request = PeriodicWorkRequest.Builder(RepoCheck::class.java, 1, TimeUnit.DAYS).setConstraints(Sync.constraints()).build()
        work.enqueueUniquePeriodicWork(WORK, ExistingPeriodicWorkPolicy.UPDATE, request)
    }
}
