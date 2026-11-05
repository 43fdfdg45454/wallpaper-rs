package io.github.wallpaper_rs

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ConvertTest {
    private fun up(hardware: Boolean, width: Int, height: Int, fps: Double = 60.0) =
        Decoder(hardware) { w, h, r -> maxOf(w, h) <= maxOf(width, height) && minOf(w, h) <= minOf(width, height) && r <= fps }

    private val avc = listOf(up(true, 1920, 1080), up(false, 4096, 2160))

    @Test
    fun aVideoThePhoneDecodesInHardwareStaysAsItIs() {
        assertNull(Convert.target(listOf(up(true, 3840, 2160)), avc, 3840, 2160, 30.0))
        assertNull(Convert.target(listOf(up(true, 640, 480)), avc, 0, 480, 30.0))
    }

    @Test
    fun aVideoDecodedOnlyInSoftwareBecomesAvcAtAFittingSize() {
        assertEquals(1280 to 720, Convert.target(listOf(up(false, 4096, 2160)), avc, 1280, 720, 30.0))
        assertEquals(1920 to 1080, Convert.target(listOf(up(false, 4096, 2160)), avc, 3840, 2160, 30.0))
        assertEquals(1080 to 1920, Convert.target(emptyList(), avc, 2160, 3840, 30.0))
        assertEquals(1920 to 1080, Convert.target(listOf(up(true, 1920, 1080)), avc, 3840, 2160, 60.0))
    }

    @Test
    fun tooFastOrWithoutAHardwareTargetNothingIsConverted() {
        assertNull(Convert.target(emptyList(), listOf(up(false, 4096, 2160)), 1280, 720, 30.0))
        assertNull(Convert.target(emptyList(), listOf(up(true, 1920, 1080, 30.0)), 1280, 720, 60.0))
        assertEquals(1280 to 720, Convert.target(emptyList(), listOf(up(true, 1920, 1080, 60.0)), 1280, 720, 120.0))
    }

    @Test
    fun fittedSizesKeepTheShapeWithEvenSides() {
        assertEquals(1920 to 818, Convert.fitted(4096, 1744, 1920 to 1080))
        assertEquals(64 to 32, Convert.fitted(64, 32, 1920 to 1080))
        assertEquals(864 to 1920, Convert.fitted(1440, 3200, 1920 to 1080))
    }
}
