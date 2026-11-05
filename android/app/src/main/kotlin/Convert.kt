package io.github.wallpaper_rs

import android.content.Context
import android.media.MediaCodecList
import android.media.MediaExtractor
import android.media.MediaFormat
import android.os.PowerManager
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

class Decoder(val hardware: Boolean, val fits: (Int, Int, Double) -> Boolean)

object Convert {
    const val SETTING = "video_convert"
    const val KEEP = "keep"
    const val OFF = "off"
    const val SAVER = "saver"
    const val STARTED = "started"
    private const val AVC = "video/avc"
    private const val RATE = 30.0
    private const val TOP_RATE = 60.0
    private val BOXES = listOf(1920 to 1080, 1280 to 720)

    fun fitted(width: Int, height: Int, box: Pair<Int, Int>): Pair<Int, Int> {
        val scale = minOf(1.0, box.first.toDouble() / max(width, height), box.second.toDouble() / min(width, height))
        val even = { side: Int -> ((side * scale / 2).roundToInt() * 2).coerceAtLeast(2) }
        return even(width) to even(height)
    }

    fun target(source: List<Decoder>, avc: List<Decoder>, width: Int, height: Int, fps: Double): Pair<Int, Int>? {
        if (width <= 0 || height <= 0 || source.any { it.hardware && it.fits(width, height, fps) }) return null
        val rate = fps.coerceAtMost(TOP_RATE)
        return (listOf(width to height) + BOXES.map { fitted(width, height, it) }).distinct()
            .firstOrNull { (w, h) -> avc.any { it.hardware && it.fits(w, h, rate) } }
    }

    private fun decoders(format: MediaFormat): List<Decoder> {
        val mime = format.getString(MediaFormat.KEY_MIME).orEmpty()
        val profile = if (format.containsKey(MediaFormat.KEY_PROFILE)) format.getInteger(MediaFormat.KEY_PROFILE) else null
        return MediaCodecList(MediaCodecList.REGULAR_CODECS).codecInfos
            .filter { info -> !info.isEncoder && info.supportedTypes.any { it.equals(mime, true) } }
            .mapNotNull { info ->
                val caps = runCatching { info.getCapabilitiesForType(mime) }.getOrNull() ?: return@mapNotNull null
                val video = caps.videoCapabilities ?: return@mapNotNull null
                val profiled = profile == null || caps.profileLevels.any { it.profile == profile }
                Decoder(info.isHardwareAccelerated) { w, h, r -> profiled && runCatching { video.areSizeAndRateSupported(w, h, r) }.getOrDefault(false) }
            }
    }

    fun plan(path: String): Pair<Int, Int>? = runCatching {
        val extractor = MediaExtractor().apply { setDataSource(path) }
        try {
            val format = (0 until extractor.trackCount).map(extractor::getTrackFormat)
                .firstOrNull { it.getString(MediaFormat.KEY_MIME).orEmpty().startsWith("video/") } ?: return null
            val fps = if (format.containsKey(MediaFormat.KEY_FRAME_RATE)) format.getNumber(MediaFormat.KEY_FRAME_RATE, RATE).toDouble() else RATE
            val avc = decoders(MediaFormat.createVideoFormat(AVC, 2, 2))
            target(decoders(format), avc, format.getInteger(MediaFormat.KEY_WIDTH), format.getInteger(MediaFormat.KEY_HEIGHT), fps)
        } finally {
            extractor.release()
        }
    }.getOrNull()

    fun saving(context: Context) = context.getSystemService(PowerManager::class.java)?.isPowerSaveMode == true

    fun chosen(): String = runCatching { Native.setting(SETTING, null) }.getOrDefault(OFF)
}
