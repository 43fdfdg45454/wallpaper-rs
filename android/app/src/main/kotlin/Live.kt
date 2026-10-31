package io.github.wallpaper_rs

import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView

class Feel(private val pages: Pages) : View.OnTouchListener {
    @Volatile
    var on = false
    var painter: Painter? = null

    override fun onTouch(view: View?, event: MotionEvent): Boolean {
        painter?.let { if (pages.press(it, event)) return true }
        val target = painter?.takeIf { on } ?: return false
        val (x, y, action) = Triple(event.x, event.y, event.actionMasked)
        Render.post { target.touch(x, y, action != MotionEvent.ACTION_UP && action != MotionEvent.ACTION_CANCEL) }
        if (action == MotionEvent.ACTION_UP) view?.performClick()
        return true
    }
}

@Composable
fun Live(main: Main, vararg changes: Any?, modifier: Modifier = Modifier.fillMaxSize()) = Box(modifier) {
    val feel = remember { Feel(main.pages) }
    val painter = remember { Painter(true, main.videos, main.tilt, main.hearing, main.pages) { feel.on = it }.also { feel.painter = it } }
    LaunchedEffect(*changes) { Render.post(painter::refresh) }
    AndroidView({ SurfaceView(it).apply { holder.addCallback(Preview(this, painter, main.pages)); setOnTouchListener(feel) } }, Modifier.fillMaxSize())
}

private class Preview(private val view: SurfaceView, private val painter: Painter, private val pages: Pages) : SurfaceHolder.Callback {
    override fun surfaceCreated(holder: SurfaceHolder) = Render.call {
        painter.attach(holder.surface)
        painter.show(true)
        Wallpaper.previews += painter
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) = Render.call {
        painter.resize(width, height, view.display?.rotation ?: 0)
        painter.redraw()
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) = pages.drop(painter).let { _ -> Render.call {
        Wallpaper.previews -= painter
        painter.detach()
    } }
}
