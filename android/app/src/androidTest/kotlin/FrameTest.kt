package io.github.wallpaper_rs

import android.os.Debug
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class FrameTest : SceneBase() {
    @Suppress("DEPRECATION")
    @Test
    fun drawingFramesAllocatesNothingOnTheRenderThread() {
        show("Snowfall")
        assertTrue(waitFor { frames.get() > FRAMES })
        val counted = Render.call {
            Debug.startAllocCounting()
            Debug.resetThreadAllocSize()
            val probe = ByteArray(PROBE)
            Debug.getThreadAllocSize() + probe.size * 0
        }
        assumeTrue("this runtime does not count allocations", counted >= PROBE)
        try {
            Render.call { Debug.resetThreadAllocSize() }
            val start = frames.get()
            assertTrue(waitFor { frames.get() > start + MEASURED })
            val bytes = Render.call { Debug.getThreadAllocSize() }
            assertTrue("the render thread allocated $bytes bytes in $MEASURED frames", bytes <= NOISE)
        } finally {
            Render.call { Debug.stopAllocCounting() }
        }
    }

    private companion object {
        const val PROBE = 4096
        const val MEASURED = 60
        const val NOISE = 4096
    }
}
