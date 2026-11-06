package io.github.wallpaper_rs

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class MediaConsentTest {
    @Test
    fun onlyTurningOnWhatPlaysWithTheNetworkAsksASecondTime() {
        assertTrue(MediaConsent.asks("media_network", true))
        assertFalse("taking it back never asks", MediaConsent.asks("media_network", false))
        assertFalse("the first permission has its own switch and note", MediaConsent.asks("media", true))
        assertFalse(MediaConsent.asks("audio", true))
    }
}
