package io.github.wallpaper_rs

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class HeardTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    @Test
    fun theChannelStaysOpenWhileTheAppOrAWallpaperIsShown() {
        storage()
        val (app, home) = Any() to Any()
        Heard.want(context, app, true)
        Heard.want(context, home, true)
        assertTrue(Heard.listening)
        Heard.want(context, app, false)
        assertTrue("the shown wallpaper keeps listening", Heard.listening)
        Heard.want(context, home, false)
        assertFalse("nothing listens while nothing is shown", Heard.listening)
        Heard.want(context, home, true)
        assertTrue(waitFor { !Heard.listening })
        Heard.want(context, home, false)
    }

    @Test
    fun listeningWhileShownIsTheUsersChoice() {
        storage()
        assertEquals(Heard.SHOWN, Native.setting(Heard.SETTING, null))
        assertTrue(Heard.shown())
        Native.setting(Heard.SETTING, "app")
        assertFalse(Heard.shown())
        Native.setting(Heard.SETTING, Heard.SHOWN)
    }
}
