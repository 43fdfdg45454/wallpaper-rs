package io.github.wallpaper_rs

import android.graphics.Color
import android.graphics.ImageFormat
import android.hardware.HardwareBuffer
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioTrack
import android.net.Uri
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class DuckingTest {
    private val readers = Readers("ducking")
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val audio = context.getSystemService(AudioManager::class.java)
    private val videos = Videos(context)
    private val painter = Painter(false, videos, Tilt(context), Hearing(context), Pages(context))
    private var ours: Video? = null
    private var other: AudioTrack? = null
    private var muted: String? = null

    @After
    fun stop() {
        other?.release()
        Render.call {
            ours?.release()
            painter.detach()
        }
        videos.close()
        readers.close()
        Native.setting(DUCK, "false")
        muted?.let { Native.setting(MUTED, it) }
        assertTrue(waitFor { Duck.gain == 1f })
    }

    @Test
    fun ourOwnSoundKeepsPlayingAndAnotherPlayerSilencesIt() {
        assertEquals(IMAGE, install(still(Color.GRAY)))
        muted = Native.setting(MUTED, null)
        Native.setting(DUCK, "true")
        Native.setting(MUTED, "false")
        val reader = readers.make(SIDE, ImageFormat.PRIVATE, HardwareBuffer.USAGE_GPU_SAMPLED_IMAGE)
        readers.listen(reader) { it.acquireLatestImage()?.close() }
        val video = Video(context, Uri.fromFile(silence()).toString())
        ours = video
        Render.call {
            painter.attach(reader.surface)
            painter.resize(SIDE, SIDE, 0)
            painter.show(true)
            video.viewers += painter
            video.update()
        }
        assertTrue("our video plays its sound", waitFor { audio.activePlaybackConfigurations.isNotEmpty() })
        Thread.sleep(QUIET_MS)
        assertEquals("our own sound does not silence us", 1f, Duck.gain)
        other = track().apply { play() }
        assertTrue("another player silences the wallpaper", waitFor { Duck.gain == 0f })
        other?.stop()
        assertTrue("the sound comes back when it stops", waitFor { Duck.gain == 1f })
    }

    private fun silence(): File = File(storage(), "silence.wav").apply {
        val samples = RATE * SECONDS
        val header = ByteBuffer.allocate(HEADER).order(ByteOrder.LITTLE_ENDIAN)
        header.put("RIFF".toByteArray()).putInt(HEADER - 8 + samples * 2).put("WAVEfmt ".toByteArray())
        header.putInt(16).putShort(1).putShort(1).putInt(RATE).putInt(RATE * 2).putShort(2).putShort(16)
        header.put("data".toByteArray()).putInt(samples * 2)
        writeBytes(header.array() + ByteArray(samples * 2))
    }

    private fun track(): AudioTrack {
        val frames = RATE / 10
        return AudioTrack.Builder()
            .setAudioAttributes(AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA).build())
            .setAudioFormat(
                AudioFormat.Builder().setEncoding(AudioFormat.ENCODING_PCM_16BIT).setSampleRate(RATE)
                    .setChannelMask(AudioFormat.CHANNEL_OUT_MONO).build(),
            )
            .setTransferMode(AudioTrack.MODE_STATIC).setBufferSizeInBytes(frames * 2).build().apply {
                write(ByteArray(frames * 2), 0, frames * 2)
                setLoopPoints(0, frames, -1)
            }
    }

    private companion object {
        const val SIDE = 32
        const val RATE = 8000
        const val SECONDS = 20
        const val HEADER = 44
        const val QUIET_MS = 1500L
        const val DUCK = "playback.duck"
        const val MUTED = "playback.muted"
    }
}
