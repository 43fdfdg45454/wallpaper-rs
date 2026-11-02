package io.github.wallpaper_rs

import android.graphics.PixelFormat
import android.hardware.HardwareBuffer
import android.media.ImageReader
import android.os.Handler
import android.os.HandlerThread

const val READ = HardwareBuffer.USAGE_GPU_COLOR_OUTPUT or HardwareBuffer.USAGE_CPU_READ_OFTEN

class Readers(name: String) {
    private val thread = HandlerThread(name).apply { start() }
    private val handler = Handler(thread.looper)
    private val made = mutableListOf<ImageReader>()
    private var open = true

    fun make(side: Int, format: Int = PixelFormat.RGBA_8888, usage: Long = READ): ImageReader =
        ImageReader.newInstance(side, side, format, 3, usage).also { made += it }

    fun listen(reader: ImageReader, read: (ImageReader) -> Unit) =
        reader.setOnImageAvailableListener({ if (open) read(it) }, handler)

    fun close() {
        handler.post {
            open = false
            made.forEach(ImageReader::close)
        }
        thread.quitSafely()
        thread.join()
    }
}
