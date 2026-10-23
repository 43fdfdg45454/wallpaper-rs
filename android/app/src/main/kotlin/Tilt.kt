package io.github.wallpaper_rs

import android.content.Context
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager

class Tilt(context: Context) : SensorEventListener {
    private val manager = context.getSystemService(SensorManager::class.java)
    private val sensor = manager?.run {
        getDefaultSensor(Sensor.TYPE_GAME_ROTATION_VECTOR) ?: getDefaultSensor(Sensor.TYPE_ROTATION_VECTOR)
    }
    private val rates = mutableMapOf<Painter, Int>()
    private val matrix = FloatArray(9)
    private var rate = 0
    var rotation = 0

    fun follow(painter: Painter, wanted: Int) {
        if (wanted > 0) rates[painter] = wanted.coerceAtMost(MAX_HZ) else rates -= painter
        val next = rates.values.maxOrNull() ?: 0
        if (next == rate || sensor == null) return
        if (rate > 0) manager?.unregisterListener(this)
        rate = next
        if (rate > 0) manager?.registerListener(this, sensor, 1_000_000 / rate, Render.handler)
    }

    override fun onSensorChanged(event: SensorEvent) {
        SensorManager.getRotationMatrixFromVector(matrix, event.values)
        if (Native.tilt(matrix[6], matrix[7], matrix[8], rotation)) rates.keys.forEach(Painter::move)
    }

    override fun onAccuracyChanged(sensor: Sensor, accuracy: Int) = Unit

    private companion object {
        const val MAX_HZ = 60
    }
}
