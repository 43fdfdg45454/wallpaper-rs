package io.github.wallpaper_rs

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
}
