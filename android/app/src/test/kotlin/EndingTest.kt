package io.github.wallpaper_rs

import androidx.media3.common.Player
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class EndingTest {
    @Test
    fun onlyAnEndedPlayerTellsItsEnd() {
        assertTrue(Ending.over(Player.STATE_ENDED))
        assertFalse(Ending.over(Player.STATE_READY))
        assertFalse(Ending.over(Player.STATE_BUFFERING))
    }

    @Test
    fun anEndedVideoMovedToAnotherSurfaceDrawsItsLastFrameAgain() {
        assertEquals(12_000L, Ending.redraw(Player.STATE_ENDED, 12_000L))
        assertNull("a playing video gives its next frame on its own", Ending.redraw(Player.STATE_READY, 4_000L))
        assertNull(Ending.redraw(Player.STATE_BUFFERING, 0L))
    }
}
