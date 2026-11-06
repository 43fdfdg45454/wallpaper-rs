package io.github.wallpaper_rs

import org.junit.Assert.assertEquals
import org.junit.Test

class ShownTest {
    @Test
    fun theStatusComesFromTheReaderThatKnowsMost() {
        assertEquals("nothing reads the players", MediaShown(), MediaShown.of(emptyList()))
        assertEquals(MediaShown(), MediaShown.of(listOf(MediaShown(players = 3, track = true))))
        val preview = MediaShown(listening = true, players = 2)
        val wallpaper = MediaShown(listening = true, players = 2, track = true, playing = true, cover = true)
        assertEquals(wallpaper, MediaShown.of(listOf(preview, wallpaper)))
        assertEquals(preview, MediaShown.of(listOf(preview, MediaShown())))
    }
}
