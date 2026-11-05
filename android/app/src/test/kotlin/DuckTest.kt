package io.github.wallpaper_rs

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class DuckTest {
    @Test
    fun onlySoundOfOtherAppsSilencesTheWallpaper() {
        assertFalse(Ducking.others(0, 0))
        assertFalse("our own video plays alone", Ducking.others(1, 1))
        assertFalse("a video and a page with sound", Ducking.others(2, 2))
        assertTrue(Ducking.others(1, 0))
        assertTrue(Ducking.others(3, 2))
        assertFalse("our player was not reported yet", Ducking.others(0, 1))
    }

    @Test
    fun aVideoAtNoVolumeDecodesNoSound() {
        assertFalse(Audible.muted(silent = false, sound = "false", loudness = 0.5f))
        assertTrue("a muted package or a volume of 0", Audible.muted(silent = false, sound = "false", loudness = 0f))
        assertTrue("video sound off", Audible.muted(silent = false, sound = "true", loudness = 1f))
        assertTrue("a clip of a scene without sound", Audible.muted(silent = true, sound = "false", loudness = 1f))
    }

    @Test
    fun theGainFadesInTenStepsAndStopsAtItsGoal() {
        var gain = 1f
        var steps = 0
        while (gain != 0f) {
            gain = Ducking.next(gain, 0f)
            steps++
        }
        assertEquals(10, steps)
        assertEquals(0f, Ducking.next(0.05f, 0f))
        assertEquals(1f, Ducking.next(0.95f, 1f))
        assertEquals(0.5f, Ducking.next(0.4f, 1f), 1e-6f)
    }
}
