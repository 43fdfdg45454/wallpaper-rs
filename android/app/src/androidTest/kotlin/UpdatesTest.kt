package io.github.wallpaper_rs

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
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

    @Test
    fun theDailyCheckFollowsTheMobileDataSwitch() {
        storage()
        val work = WorkManager.getInstance(context)
        val network = Native.setting(NETWORK, null)
        val needed = { work.getWorkInfosForUniqueWork(Updates.WORK).get().single().constraints.requiredNetworkType }
        try {
            Native.setting(NETWORK, "any")
            Updates.schedule(context, repos = 1)
            assertTrue(waitFor { needed() == NetworkType.CONNECTED })
            Native.setting(NETWORK, "unmetered")
            Updates.schedule(context, repos = 1)
            assertTrue("turning mobile data off reaches the waiting check", waitFor { needed() == NetworkType.UNMETERED })
        } finally {
            Native.setting(NETWORK, network)
            work.cancelUniqueWork(Updates.WORK).result.get()
        }
    }

    private companion object {
        const val NETWORK = "sync_network"
    }
}
