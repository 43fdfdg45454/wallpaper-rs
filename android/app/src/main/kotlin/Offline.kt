package io.github.wallpaper_rs

import android.os.Handler
import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.TransferListener
import androidx.media3.exoplayer.upstream.BandwidthMeter

@OptIn(UnstableApi::class)
object Offline : BandwidthMeter {
    override fun getBitrateEstimate() = 0L

    override fun getTransferListener(): TransferListener? = null

    override fun addEventListener(eventHandler: Handler, eventListener: BandwidthMeter.EventListener) = Unit

    override fun removeEventListener(eventListener: BandwidthMeter.EventListener) = Unit
}
