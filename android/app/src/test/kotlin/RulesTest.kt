package io.github.wallpaper_rs

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class RulesTest {
    @Test
    fun daysToggleInWeekOrder() {
        assertEquals(listOf("Mon", "Sat"), toggled(listOf("Sat"), "Mon"))
        assertEquals(listOf("Sat"), toggled(listOf("Mon", "Sat"), "Mon"))
        assertEquals(WEEK, WEEK.fold(emptyList<String>(), ::toggled))
    }

    @Test
    fun hoursAreSortedAndKeptOnce() {
        assertEquals(listOf("07:05", "18:30"), timed(listOf("18:30"), 7, 5))
        assertEquals(listOf("18:30"), timed(listOf("18:30"), 18, 30))
    }

    @Test
    fun aDraggedRowLandsWhereItsMiddleIs() {
        val rows = List(5) { 100 }
        assertEquals(2, landing(2, 40f, rows))
        assertEquals(3, landing(2, 60f, rows))
        assertEquals(4, landing(2, 900f, rows))
        assertEquals(0, landing(2, -160f, rows))
        assertEquals(1, landing(2, -140f, rows))
    }

    @Test
    fun messageIdsNameTheirStrings() {
        assertEquals("error_android_no_preset", resourceName("android.no_preset"))
        assertEquals("error_format_newer", resourceName("format.newer"))
        assertNull(resourceName(""))
        assertNull(resourceName("../x"))
    }
}
