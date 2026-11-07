package io.github.wallpaper_rs

import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

class RepoPreviewTest {
    @Before
    fun prepare() {
        storage()
    }

    @Test
    fun aPreviewOfARepositoryTheUserNeverAddedIsNeitherFetchedNorCached() = runBlocking {
        assertNull(remote("0000/org.example.none/1.0.0"))
        assertNull(remote("garbled"))
        val reply = JSONObject(Native.account(JSONObject().put("op", "thumb").put("spec", "0000/org.example.none/1.0.0").toString()))
        assertTrue(reply.has("error"))
    }
}
