package io.github.wallpaper_rs

import android.graphics.ImageFormat
import android.hardware.HardwareBuffer
import android.net.Uri
import androidx.annotation.OptIn
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.After
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@OptIn(UnstableApi::class)
@RunWith(AndroidJUnit4::class)
class CapTest {
    private val readers = Readers("cap")
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val context = instrumentation.targetContext

    @After
    fun close() = readers.close()

    private fun shown(cap: Int): Int {
        val limit = FrameCap().apply { fps = cap }
        val reader = readers.make(SIDE, ImageFormat.PRIVATE, HardwareBuffer.USAGE_GPU_SAMPLED_IMAGE)
        readers.listen(reader) { it.acquireLatestImage()?.close() }
        val clip = MediaItem.fromUri(Uri.fromFile(fixture("quadrants-64x32.webm")))
        lateinit var player: ExoPlayer
        instrumentation.runOnMainSync {
            player = ExoPlayer.Builder(context, CappedRenderers(context, limit)).build().apply {
                repeatMode = Player.REPEAT_MODE_ONE
                setVideoSurface(reader.surface)
                setMediaItem(clip)
                prepare()
                play()
            }
        }
        val count = { var n = 0; instrumentation.runOnMainSync { n = player.videoDecoderCounters?.renderedOutputBufferCount ?: 0 }; n }
        assertTrue("the video never played", waitFor { count() > 0 })
        val before = count()
        Thread.sleep(WINDOW_MS)
        val seen = count() - before
        instrumentation.runOnMainSync { player.release() }
        return seen
    }

    @Test
    fun theFramesShownFollowTheCapAndAllOfThemShowWithoutOne() {
        val free = shown(0)
        val capped = shown(10)
        assertTrue("20 fps video, no cap: $free", free >= 45)
        assertTrue("capped to 10 fps over 3 s: $capped", capped in 22..38)
    }

    private companion object {
        const val SIDE = 64
        const val WINDOW_MS = 3_000L
    }
}
