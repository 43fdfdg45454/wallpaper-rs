package io.github.wallpaper_rs

import android.annotation.SuppressLint
import android.app.Presentation
import android.content.Context
import android.graphics.Color
import android.graphics.drawable.ColorDrawable
import android.hardware.display.DisplayManager
import android.hardware.display.VirtualDisplay
import android.view.MotionEvent
import android.view.Surface
import android.webkit.CookieManager
import android.webkit.JavascriptInterface
import android.webkit.RenderProcessGoneDetail
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebStorage
import android.webkit.WebView
import android.webkit.WebViewClient
import java.io.FileInputStream
import org.json.JSONObject

@SuppressLint("SetJavaScriptEnabled")
class Page(context: Context, private val handle: Long, spec: JSONObject, val surface: Surface, size: Pair<Int, Int>, private val gone: () -> Unit, private val heard: () -> Unit) {
    val key: String = spec.getString("key")
    private val origin = spec.getString("origin")
    private val host = spec.getString("url").removePrefix("https://").trimEnd('/')
    private val storage = spec.getBoolean("storage")
    private val dpi = context.resources.displayMetrics.densityDpi
    private val display: VirtualDisplay =
        context.getSystemService(DisplayManager::class.java).createVirtualDisplay(NAME, size.first, size.second, dpi, surface, 0)!!
    private val presentation = Presentation(context, display.display)
    val web = WebView(presentation.context)
    private val touches = Touches(web)

    init {
        Refusing.workers()
        runCatching { Pages.forget(true) }
        val background = Color.parseColor(spec.getString("background"))
        CookieManager.getInstance().setAcceptThirdPartyCookies(web, false)
        web.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            allowFileAccess = false
            allowContentAccess = false
            blockNetworkLoads = !spec.getBoolean("network")
            cacheMode = WebSettings.LOAD_NO_CACHE
            mediaPlaybackRequiresUserGesture = false
            javaScriptCanOpenWindowsAutomatically = false
            safeBrowsingEnabled = false
            textZoom = 100
            setGeolocationEnabled(false)
            setSupportMultipleWindows(false)
            setSupportZoom(false)
        }
        web.setBackgroundColor(background)
        web.isLongClickable = false
        web.isHapticFeedbackEnabled = false
        web.setOnLongClickListener { true }
        web.isVerticalScrollBarEnabled = false
        web.isHorizontalScrollBarEnabled = false
        web.overScrollMode = WebView.OVER_SCROLL_NEVER
        web.setDownloadListener { _, _, _, _, _ -> }
        web.webViewClient = Client()
        web.webChromeClient = Refusing()
        web.addJavascriptInterface(Bridge(), BRIDGE)
        presentation.window?.setBackgroundDrawable(ColorDrawable(background))
        presentation.setContentView(web)
        presentation.show()
        follow(spec)
        val url = spec.getString("url")
        web.post { web.loadUrl(url) }
    }

    fun follow(spec: JSONObject) = touches.follow(spec)

    fun tell(script: String) = web.evaluateJavascript(script, null)

    fun visible(on: Boolean) = if (on) web.onResume() else web.onPause()

    fun resize(width: Int, height: Int) = display.resize(width, height, dpi)

    fun press(event: MotionEvent) = touches.press(event)

    fun close() {
        presentation.dismiss()
        web.destroy()
        display.release()
        if (!storage) WebStorage.getInstance().deleteOrigin(origin)
    }

    private inner class Client : WebViewClient() {
        override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? =
            answer(Native.serve(handle, request.url.toString()))

        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest) = request.url.host != host

        override fun onPageCommitVisible(view: WebView, url: String) {
            touches.drawn = true
        }

        override fun onRenderProcessGone(view: WebView, detail: RenderProcessGoneDetail): Boolean {
            gone()
            return true
        }
    }

    private inner class Bridge {
        @JavascriptInterface
        fun post(message: String) {
            if (message.startsWith(AUDIO)) heard()
            val reply = Native.heard(handle, message) ?: return
            web.post { web.evaluateJavascript(reply, null) }
        }
    }

    companion object {
        private const val NAME = "wprs-page"
        private const val BRIDGE = "wprsHost"
        private const val AUDIO = "{\"audio\""

        fun answer(text: String): WebResourceResponse? {
            val reply = JSONObject(text)
            if (reply.optBoolean("network")) return null
            val mime = reply.optString("mime")
            if (mime.isEmpty()) return WebResourceResponse("text/plain", "utf-8", 403, "Forbidden", emptyMap(), ByteArray(0).inputStream())
            val body = if (reply.has("text")) reply.getString("text").byteInputStream() else FileInputStream(reply.getString("path"))
            val headers = mapOf("Content-Security-Policy" to reply.getString("csp"), "X-Content-Type-Options" to "nosniff", "X-DNS-Prefetch-Control" to "off")
            val encoding = if (mime.startsWith("text/") || mime.endsWith("json") || mime.endsWith("xml")) "utf-8" else null
            return WebResourceResponse(mime, encoding, 200, "OK", headers, body)
        }
    }
}
