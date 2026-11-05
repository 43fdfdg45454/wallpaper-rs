package io.github.wallpaper_rs

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PicturedTest {
    @Test
    fun theCoverIsReadOnlyForNewTrackDataOrANewPlayer() {
        val pictured = Pictured()
        assertTrue(pictured.read(false))
        assertFalse(pictured.read(false))
        assertFalse(pictured.read(false))
        assertTrue(pictured.read(true))
        assertFalse(pictured.read(false))
        pictured.forget()
        assertTrue(pictured.read(false))
    }
}
