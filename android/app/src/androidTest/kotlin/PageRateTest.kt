package io.github.wallpaper_rs

import android.graphics.PixelFormat
import android.media.ImageReader
import android.os.Build
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class PageRateTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    @Test
    fun aPageDisplayAsksForNoMoreFramesThanItsCap() {
        assumeTrue("the requested refresh rate needs Android 14", Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
        val reader = ImageReader.newInstance(64, 64, PixelFormat.RGBA_8888, 2)
        val dpi = context.resources.displayMetrics.densityDpi
        val capped = Displays.make(context, "wprs-rate-test", 64 to 64, dpi, reader.surface, 15)
        val rate = capped.display.refreshRate
        capped.release()
        val free = Displays.make(context, "wprs-rate-free", 64 to 64, dpi, reader.surface, 0)
        val whole = free.display.refreshRate
        free.release()
        reader.close()
        assertTrue("a page capped at 15 frames runs its display at $rate", rate <= 15.5f)
        assertTrue("an uncapped page runs at $whole", whole > 15.5f)
    }
}
