package io.github.wallpaper_rs

import org.junit.Assert.assertEquals
import org.junit.Test

class CostTest {
    @Test
    fun aChosenLevelOnASceneHeavyOnlyInModelTrianglesStillWarns() {
        assertEquals(R.string.level_triangles, costWords("stop", over = false))
        assertEquals(R.string.level_stopped, costWords("still", over = false))
        assertEquals(R.string.auto_would_stop, costWords("stop", over = true))
        assertEquals(R.string.auto_would_lower, costWords("lower", over = true))
        assertEquals(null, costWords("", over = false))
    }
}
