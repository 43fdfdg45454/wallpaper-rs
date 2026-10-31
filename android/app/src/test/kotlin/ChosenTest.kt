package io.github.wallpaper_rs

import org.junit.Assert.assertEquals
import org.junit.Test

class ChosenTest {
    @Test
    fun copiedNamesStaySafeAndApart() {
        assertEquals("sea_shore.jpg", safeName("sea shore.jpg", emptySet()))
        assertEquals("2-sea.jpg", safeName("sea.jpg", setOf("sea.jpg")))
        assertEquals("file", safeName("..", emptySet()))
        assertEquals("_etc_passwd", safeName("/etc/passwd", emptySet()))
    }
}
