package io.github.wallpaper_rs

import android.view.InputDevice
import android.view.MotionEvent
import android.view.ViewConfiguration
import android.webkit.WebView
import org.json.JSONObject

class Touches(private val web: WebView) {
    private var pointer = false
    private var clicks = false
    private var down: MotionEvent? = null
    private val slop = ViewConfiguration.get(web.context).scaledTouchSlop

    @Volatile
    var drawn = false

    fun follow(spec: JSONObject) {
        pointer = spec.getBoolean("pointer")
        clicks = spec.getBoolean("clicks")
    }

    fun press(event: MotionEvent) {
        when {
            !drawn -> return
            pointer && clicks -> web.dispatchTouchEvent(event)
            pointer -> hover(event)
            clicks -> tap(event)
        }
    }

    private fun hover(event: MotionEvent) = MotionEvent.obtain(event).run {
        val up = event.actionMasked == MotionEvent.ACTION_UP || event.actionMasked == MotionEvent.ACTION_CANCEL
        action = if (up) MotionEvent.ACTION_HOVER_EXIT else MotionEvent.ACTION_HOVER_MOVE
        source = InputDevice.SOURCE_MOUSE
        web.dispatchGenericMotionEvent(this)
        recycle()
    }

    private fun tap(event: MotionEvent) {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> down = MotionEvent.obtain(event)
            MotionEvent.ACTION_UP -> down?.let { start ->
                val near = kotlin.math.hypot(event.x - start.x, event.y - start.y) < slop
                if (near && event.eventTime - start.downTime < ViewConfiguration.getLongPressTimeout()) {
                    web.dispatchTouchEvent(start)
                    val up = MotionEvent.obtain(start.downTime, event.eventTime, MotionEvent.ACTION_UP, start.x, start.y, 0)
                    up.source = start.source
                    web.dispatchTouchEvent(up)
                    up.recycle()
                }
            }
            else -> return
        }
        if (event.actionMasked == MotionEvent.ACTION_UP) down = down?.let { it.recycle(); null }
    }
}
