package io.github.wallpaper_rs

import android.app.ActivityManager
import android.app.Application
import android.content.Context
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import androidx.annotation.OptIn
import androidx.media3.common.util.Log
import androidx.media3.common.util.UnstableApi
import androidx.work.BackoffPolicy
import androidx.work.Configuration
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequest
import androidx.work.PeriodicWorkRequest
import androidx.work.WorkManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import java.security.KeyStore
import java.util.concurrent.TimeUnit
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import org.json.JSONObject

class Wprs : Application(), Configuration.Provider {
    override val workManagerConfiguration: Configuration get() = Configuration.Builder().build()

    @OptIn(UnstableApi::class)
    override fun onCreate() {
        super.onCreate()
        Log.setLogLevel(Log.LOG_LEVEL_WARNING)
        Native.prepare(noBackupFilesDir.path)
        val small = getSystemService(ActivityManager::class.java)?.isLowRamDevice == true
        Native.device(small, Build.VERSION.MEDIA_PERFORMANCE_CLASS)
        if (Sync.active()) Sync.schedule(this)
    }
}

object Vault {
    private const val ALIAS = "wprs-account"
    private const val CIPHER = "AES/GCM/NoPadding"
    private const val IV = 12

    private fun key(): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        return store.getKey(ALIAS, null) as SecretKey? ?: KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").run {
            val purposes = KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
            init(
                KeyGenParameterSpec.Builder(ALIAS, purposes)
                    .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                    .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                    .setKeySize(256)
                    .build(),
            )
            generateKey()
        }
    }

    @JvmStatic
    fun seal(plain: ByteArray): ByteArray = Cipher.getInstance(CIPHER).run {
        init(Cipher.ENCRYPT_MODE, key())
        iv + doFinal(plain)
    }

    @JvmStatic
    fun open(sealed: ByteArray): ByteArray = Cipher.getInstance(CIPHER).run {
        init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, sealed, 0, IV))
        doFinal(sealed, IV, sealed.size - IV)
    }
}

class Syncer(context: Context, params: WorkerParameters) : Worker(context, params) {
    override fun doWork(): Result {
        if (!Sync.active()) return Result.success()
        val request = JSONObject(mapOf("op" to "sync", "conflict" to applicationContext.getString(R.string.conflict, "%1")))
        val answer = JSONObject(Native.account(request.toString()))
        if (answer.optInt("applied") > 0) Render.post { Wallpaper.reload(false) }
        return if (answer.optInt("unsent") > 0) Result.retry() else Result.success()
    }
}

object Sync {
    private const val PERIODIC = "sync"
    private const val SOON = "sync-soon"

    fun status() = JSONObject(Native.account("{\"op\": \"status\"}"))

    fun active() = status().optBoolean("active")

    private fun constraints() = Constraints.Builder()
        .setRequiredNetworkType(if (Native.setting("sync_network", null) == "any") NetworkType.CONNECTED else NetworkType.UNMETERED)
        .setRequiresBatteryNotLow(true)
        .build()

    fun schedule(context: Context) = WorkManager.getInstance(context).run {
        if (active()) {
            val request = PeriodicWorkRequest.Builder(Syncer::class.java, 1, TimeUnit.HOURS).setConstraints(constraints())
                .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS).build()
            enqueueUniquePeriodicWork(PERIODIC, ExistingPeriodicWorkPolicy.UPDATE, request)
        } else {
            cancelUniqueWork(PERIODIC)
            cancelUniqueWork(SOON)
        }
    }

    fun soon(context: Context, seconds: Long) {
        if (!active()) return
        val request = OneTimeWorkRequest.Builder(Syncer::class.java)
            .setConstraints(constraints())
            .setInitialDelay(seconds, TimeUnit.SECONDS)
            .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 30, TimeUnit.SECONDS)
            .build()
        WorkManager.getInstance(context).enqueueUniqueWork(SOON, ExistingWorkPolicy.KEEP, request)
    }
}
