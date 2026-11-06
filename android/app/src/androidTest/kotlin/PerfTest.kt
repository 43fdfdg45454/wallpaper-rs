package io.github.wallpaper_rs

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class PerfTest : SceneBase() {
    @Test
    fun theReportCoversTheShownWallpaperAndStartsAnotherRunAfterItWasHidden() {
        Perf.record(true)
        try {
            show("Snowfall")
            assertTrue(waitFor { frames.get() > FRAMES })
            Render.call { painter.show(false) }
            Thread.sleep(HIDDEN_MS)
            val start = frames.get()
            Render.call { painter.show(true) }
            assertTrue(waitFor { frames.get() > start + FRAMES })
            val text = Perf.report()
            assertTrue(text, text.contains("screen       home, the one shown last"))
            assertTrue(text, number(text, "\\((\\d+) runs\\)") >= 2)
            assertTrue(text, number(text, "Kotlin, JNI and draw, (\\d+) frames") > 0)
        } finally {
            Perf.record(false)
        }
    }

    private fun number(text: String, pattern: String) =
        Regex(pattern).find(text)?.groupValues?.get(1)?.toLong() ?: -1L

    private companion object {
        const val HIDDEN_MS = 1_500L
    }
}
