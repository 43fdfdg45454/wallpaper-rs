package io.github.wallpaper_rs

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.PeriodicWorkRequest
import androidx.work.WorkInfo
import androidx.work.WorkManager
import java.util.concurrent.TimeUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class UpdatesTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    @Test
    fun withoutRepositoriesNothingChecksForUpdates() {
        storage()
        val work = WorkManager.getInstance(context)
        val waiting = PeriodicWorkRequest.Builder(RepoCheck::class.java, 1, TimeUnit.DAYS).build()
        work.enqueueUniquePeriodicWork(Updates.WORK, ExistingPeriodicWorkPolicy.KEEP, waiting).result.get()
        Updates.schedule(context)
        assertTrue(waitFor { work.getWorkInfosForUniqueWork(Updates.WORK).get().all { it.state == WorkInfo.State.CANCELLED } })
        assertEquals(0, Updates.count())
    }
}
