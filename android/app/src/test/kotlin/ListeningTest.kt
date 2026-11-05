package io.github.wallpaper_rs

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ListeningTest {
    @Test
    fun theChannelFollowsTheRulesOfTheHourlySync() {
        assertTrue(Listening.allowed(online = true, unmetered = true, any = false, low = false))
        assertFalse("mobile data only when the user allows it", Listening.allowed(online = true, unmetered = false, any = false, low = false))
        assertTrue(Listening.allowed(online = true, unmetered = false, any = true, low = false))
        assertFalse("never on a low battery", Listening.allowed(online = true, unmetered = true, any = true, low = true))
        assertFalse(Listening.allowed(online = false, unmetered = false, any = true, low = false))
    }

    @Test
    fun aSyncThatFailsWaitsLongerEachTimeUntilOneWorks() {
        var wait = Listening.FIRST_MS
        val waits = List(7) { wait.also { wait = Listening.after(it, failed = true) } }
        assertEquals(listOf(30_000L, 60_000L, 120_000L, 240_000L, 480_000L, 900_000L, 900_000L), waits)
        assertEquals("a sync that works starts over", Listening.FIRST_MS, Listening.after(wait, failed = false))
    }

    @Test
    fun theChannelStopsWhileANewerAppSyncsTheAccount() {
        assertTrue(Listening.goes(active = true, outdated = false))
        assertFalse("a newer app holds the account until this one is updated", Listening.goes(active = true, outdated = true))
        assertFalse(Listening.goes(active = false, outdated = false))
    }
}
