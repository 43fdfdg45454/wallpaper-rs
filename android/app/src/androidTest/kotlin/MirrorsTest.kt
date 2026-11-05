package io.github.wallpaper_rs

import android.net.Uri
import android.provider.MediaStore
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.work.NetworkType
import androidx.work.WorkInfo
import androidx.work.WorkManager
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class MirrorsTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val tree = "content://com.android.externalstorage.documents/tree/primary%3APictures"

    private val cloud = "content://org.example.cloud.documents/tree/root%3APhotos"

    private fun plan(vararg kinds: String, at: String = tree) = JSONObject().put("tree", at).put("accept", JSONArray(kinds.toList()))

    @Test
    fun aChosenFolderWakesTheCopyOnlyWhenItsFilesChange() {
        val triggers = Mirrors.triggers(listOf(plan("image")))
        assertEquals(Uri.parse("$tree/document/primary%3APictures/children"), triggers.first())
        assertTrue(MediaStore.Images.Media.EXTERNAL_CONTENT_URI in triggers)
        assertFalse("pictures only do not wake for videos", MediaStore.Video.Media.EXTERNAL_CONTENT_URI in triggers)
        assertEquals(3, Mirrors.triggers(listOf(plan("image", "video"), plan("image"))).size)
        val elsewhere = Mirrors.triggers(listOf(plan("image", "video", at = cloud)))
        assertEquals("a folder of another provider wakes only for its own files", listOf(Uri.parse("$cloud/document/root%3APhotos/children")), elsewhere)
        val work = WorkManager.getInstance(context)
        work.enqueueUniqueWork(TEST, androidx.work.ExistingWorkPolicy.REPLACE, Mirrors.request(triggers)).result.get()
        val info = work.getWorkInfosForUniqueWork(TEST).get().single()
        val watched = info.constraints.contentUriTriggers.map { it.uri to it.isTriggeredForDescendants }
        assertEquals(triggers.map { it to true }.toSet(), watched.toSet())
        assertTrue(info.constraints.requiresBatteryNotLow())
        assertEquals(WorkInfo.State.ENQUEUED, info.state)
        work.cancelUniqueWork(TEST).result.get()
    }

    @Test
    fun foldersOfACloudWaitForAnUnmeteredNetworkInTheBackground() {
        val (phone, far) = plan("image") to plan("image", at = cloud)
        assertEquals("mobile data copies only the folders of the phone", listOf(phone), Mirrors.now(listOf(phone, far), unmetered = false))
        assertEquals(listOf(phone, far), Mirrors.now(listOf(phone, far), unmetered = true))
        val work = WorkManager.getInstance(context)
        work.enqueueUniqueWork(TEST, androidx.work.ExistingWorkPolicy.REPLACE, Mirrors.waiting()).result.get()
        val info = work.getWorkInfosForUniqueWork(TEST).get().single()
        assertEquals(NetworkType.UNMETERED, info.constraints.requiredNetworkType)
        assertTrue(info.constraints.requiresBatteryNotLow())
        work.cancelUniqueWork(TEST).result.get()
    }

    @Test
    fun withoutChosenFoldersNothingWaitsForChanges() {
        storage()
        val work = WorkManager.getInstance(context)
        val triggers = listOf(MediaStore.Images.Media.EXTERNAL_CONTENT_URI)
        work.enqueueUniqueWork(Mirrors.WORK, androidx.work.ExistingWorkPolicy.REPLACE, Mirrors.request(triggers)).result.get()
        work.enqueueUniqueWork(Mirrors.CLOUD, androidx.work.ExistingWorkPolicy.REPLACE, Mirrors.waiting()).result.get()
        Mirrors.watch(context)
        listOf(Mirrors.WORK, Mirrors.CLOUD).forEach { name ->
            assertTrue(waitFor { work.getWorkInfosForUniqueWork(name).get().all { it.state == WorkInfo.State.CANCELLED } })
        }
        assertFalse(context.remirror())
    }

    private companion object {
        const val TEST = "mirror-test"
    }
}
