package io.github.wallpaper_rs

import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.os.ParcelFileDescriptor
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class HeardTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val context = instrumentation.targetContext
    private val network = storage().let { Native.setting(NETWORK, null) }

    private fun shell(command: String) = ParcelFileDescriptor.AutoCloseInputStream(instrumentation.uiAutomation.executeShellCommand(command)).use { it.readBytes() }

    private fun unmetered() = context.getSystemService(ConnectivityManager::class.java).run {
        getNetworkCapabilities(activeNetwork)?.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED) == true
    }

    @After
    fun restore() {
        shell("dumpsys battery reset")
        Native.setting(NETWORK, network)
    }

    @Test
    fun theChannelStaysOpenWhileTheAppOrAWallpaperIsShown() {
        Native.setting(NETWORK, ANY)
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
    fun theChannelOpensOnMobileDataOnlyWhenTheUserAllowsIt() {
        val home = Any()
        Native.setting(NETWORK, "unmetered")
        Heard.want(context, home, true)
        assertEquals("the channel follows the kind of network", unmetered(), Heard.listening)
        Native.setting(NETWORK, ANY)
        Heard.recheck()
        assertTrue(Heard.listening)
        Heard.want(context, home, false)
    }

    @Test
    fun theChannelClosesOnALowBatteryAndOpensWhenItIsFine() {
        Native.setting(NETWORK, ANY)
        val home = Any()
        Heard.want(context, home, true)
        assertTrue(Heard.listening)
        shell("dumpsys battery unplug")
        shell("dumpsys battery set level 5")
        assertTrue("a low battery closes the channel", waitFor { Heard.recheck().let { !Heard.listening } })
        shell("dumpsys battery reset")
        assertTrue("the channel opens again", waitFor { Heard.recheck().let { Heard.listening } })
        Heard.want(context, home, false)
    }

    @Test
    fun listeningWhileShownIsTheUsersChoice() {
        assertEquals(Heard.SHOWN, Native.setting(Heard.SETTING, null))
        assertTrue(Heard.shown())
        Native.setting(Heard.SETTING, "app")
        assertFalse(Heard.shown())
        Native.setting(Heard.SETTING, Heard.SHOWN)
    }

    private companion object {
        const val NETWORK = "sync_network"
        const val ANY = "any"
    }
}
