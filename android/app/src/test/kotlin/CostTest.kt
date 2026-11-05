package io.github.wallpaper_rs

import org.junit.Assert.assertEquals
import org.junit.Test

class CostTest {
    @Test
    fun aChosenLevelOnASceneHeavyOnlyInModelTrianglesStillWarns() {
        assertEquals(R.string.level_triangles, costWords(stops = true, chosen = true, over = false))
        assertEquals(R.string.level_stopped, costWords(stops = true, chosen = false, over = false))
        assertEquals(R.string.auto_would_stop, costWords(stops = true, chosen = true, over = true))
        assertEquals(R.string.auto_would_lower, costWords(stops = false, chosen = true, over = true))
        assertEquals(null, costWords(stops = false, chosen = true, over = false))
    }
}
