package io.github.wallpaper_rs

import android.net.Uri
import android.provider.MediaStore
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
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

    private fun plan(vararg kinds: String) = JSONObject().put("tree", tree).put("accept", JSONArray(kinds.toList()))

    @Test
    fun aChosenFolderWakesTheCopyOnlyWhenItsFilesChange() {
        val triggers = Mirrors.triggers(listOf(plan("image")))
        assertEquals(Uri.parse("$tree/document/primary%3APictures/children"), triggers.first())
        assertTrue(MediaStore.Images.Media.EXTERNAL_CONTENT_URI in triggers)
        assertFalse("pictures only do not wake for videos", MediaStore.Video.Media.EXTERNAL_CONTENT_URI in triggers)
        assertEquals(3, Mirrors.triggers(listOf(plan("image", "video"), plan("image"))).size)
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
    fun withoutChosenFoldersNothingWaitsForChanges() {
        storage()
        val work = WorkManager.getInstance(context)
        val triggers = listOf(MediaStore.Images.Media.EXTERNAL_CONTENT_URI)
        work.enqueueUniqueWork(Mirrors.WORK, androidx.work.ExistingWorkPolicy.REPLACE, Mirrors.request(triggers)).result.get()
        Mirrors.watch(context)
        assertTrue(waitFor { work.getWorkInfosForUniqueWork(Mirrors.WORK).get().all { it.state == WorkInfo.State.CANCELLED } })
        assertFalse(context.remirror())
    }

    private companion object {
        const val TEST = "mirror-test"
    }
}
