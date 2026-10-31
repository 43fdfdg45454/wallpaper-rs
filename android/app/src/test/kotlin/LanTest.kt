package io.github.wallpaper_rs

import java.net.InetAddress
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class LanTest {
    private fun ip(text: String) = InetAddress.getByName(text)

    @Test
    fun aFoundServerIsReachedByItsFirstUsableAddress() {
        assertEquals("https://192.168.1.20:8443", address(listOf(ip("fd00::20"), ip("192.168.1.20")), 8443))
        assertEquals("https://[fd00:0:0:0:0:0:0:20]:8443", address(listOf(ip("fe80::1"), ip("fd00::20")), 8443))
        assertNull(address(listOf(ip("127.0.0.1"), ip("fe80::1")), 8443))
    }

    @Test
    fun depthLevelsInPixelsAreWholeNumbers() {
        assertEquals(3, level("smooth", 2.6f))
        assertEquals(0.5f, level("near", 0.5f))
    }
}
