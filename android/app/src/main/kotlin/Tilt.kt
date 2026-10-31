package io.github.wallpaper_rs

import android.content.Context
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager

class Tilt(context: Context) : SensorEventListener {
    private val manager = context.getSystemService(SensorManager::class.java)
    private val sensor = manager?.let { sensors -> KINDS.firstNotNullOfOrNull { sensors.getDefaultSensor(it) } }
    private val rates = mutableMapOf<Painter, Int>()
    private val turn = FloatArray(4)
    private var rate = 0
    var rotation = 0

    fun follow(painter: Painter, wanted: Int) {
        if (wanted > 0) rates[painter] = wanted.coerceAtMost(MAX_HZ) else rates -= painter
        val next = rates.values.maxOrNull() ?: 0
        if (next == rate || sensor == null) return
        if (rate > 0) manager?.unregisterListener(this) else Native.still()
        rate = next
        if (rate > 0) manager?.registerListener(this, sensor, 1_000_000 / rate, Render.handler)
    }

    override fun onSensorChanged(event: SensorEvent) = sense(event.sensor.type, event.values)

    fun sense(type: Int, values: FloatArray) {
        val moved = if (type == Sensor.TYPE_ACCELEROMETER) Native.tilt(values[0], values[1], values[2], rotation) else {
            SensorManager.getQuaternionFromVector(turn, values)
            Native.turn(turn[0], turn[1], turn[2], turn[3], rotation)
        }
        if (moved) rates.keys.forEach(Painter::move)
    }

    override fun onAccuracyChanged(sensor: Sensor, accuracy: Int) = Unit

    private companion object {
        const val MAX_HZ = 60
        val KINDS = listOf(Sensor.TYPE_GAME_ROTATION_VECTOR, Sensor.TYPE_ROTATION_VECTOR, Sensor.TYPE_ACCELEROMETER)
    }
}
