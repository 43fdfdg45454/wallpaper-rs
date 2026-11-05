package io.github.wallpaper_rs

import android.content.Context
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.MediaStore
import androidx.work.Constraints
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequest
import androidx.work.WorkManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import java.util.concurrent.TimeUnit
import org.json.JSONArray
import org.json.JSONObject

class Remirror(context: Context, params: WorkerParameters) : Worker(context, params) {
    override fun doWork(): Result {
        runCatching { applicationContext.remirror() }
        Mirrors.watch(applicationContext, ExistingWorkPolicy.APPEND_OR_REPLACE)
        return Result.success()
    }
}

object Mirrors {
    const val WORK = "mirror"
    private const val SETTLE_S = 10L
    private const val LONGEST_S = 120L
    private val LOCAL = setOf("com.android.externalstorage.documents", "com.android.providers.media.documents")

    fun plans(): List<JSONObject> {
        val mirrors = runCatching { Native.library(JSONObject().put("op", "mirrors").toString()) }.getOrNull()?.let(::JSONArray) ?: return emptyList()
        return (0 until mirrors.length()).map { mirrors.getJSONObject(it).getJSONObject("plan") }
    }

    fun kept(context: Context, plans: List<JSONObject>): List<JSONObject> {
        val kept = context.contentResolver.persistedUriPermissions.map { it.uri.toString() }.toSet()
        return plans.filter { it.getString("tree") in kept }
    }

    fun triggers(plans: List<JSONObject>): List<Uri> {
        val trees = plans.map { Uri.parse(it.getString("tree")) }.distinct()
        val local = plans.filter { Uri.parse(it.getString("tree")).authority in LOCAL }
        val kinds = local.flatMap { it.getJSONArray("accept").strings() }.toSet()
        val shared = listOfNotNull(
            MediaStore.Images.Media.EXTERNAL_CONTENT_URI.takeIf { "image" in kinds },
            MediaStore.Video.Media.EXTERNAL_CONTENT_URI.takeIf { "video" in kinds },
        )
        return trees.map { DocumentsContract.buildChildDocumentsUriUsingTree(it, DocumentsContract.getTreeDocumentId(it)) } + shared
    }

    fun request(triggers: List<Uri>): OneTimeWorkRequest {
        val constraints = Constraints.Builder().setRequiresBatteryNotLow(true)
            .setTriggerContentUpdateDelay(SETTLE_S, TimeUnit.SECONDS)
            .setTriggerContentMaxDelay(LONGEST_S, TimeUnit.SECONDS)
        triggers.forEach { constraints.addContentUriTrigger(it, true) }
        return OneTimeWorkRequest.Builder(Remirror::class.java).setConstraints(constraints.build()).build()
    }

    fun watch(context: Context, policy: ExistingWorkPolicy = ExistingWorkPolicy.REPLACE) {
        val triggers = triggers(kept(context, plans()))
        val work = WorkManager.getInstance(context)
        if (triggers.isEmpty()) work.cancelUniqueWork(WORK) else work.enqueueUniqueWork(WORK, policy, request(triggers))
    }
}

fun Context.remirror(): Boolean {
    val changed = Mirrors.kept(this, Mirrors.plans()).count { plan -> runCatching { mirror(Uri.parse(plan.getString("tree")), plan) }.getOrDefault(false) }
    if (changed > 0) {
        Native.library(JSONObject().put("op", "mirrored").toString())
        Render.post { Wallpaper.reload(false) }
    }
    return changed > 0
}
