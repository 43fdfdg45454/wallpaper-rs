package io.github.wallpaper_rs

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class DuckTest {
    @Test
    fun onlySoundOfOtherAppsSilencesTheWallpaper() {
        assertFalse(Ducking.others(emptyList(), 0))
        assertFalse(Ducking.others(listOf(true, true), 0))
        assertTrue(Ducking.others(listOf(true, false), 0))
        assertFalse(Ducking.others(listOf(false), 1))
        assertTrue(Ducking.others(listOf(false, false, true), 1))
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
