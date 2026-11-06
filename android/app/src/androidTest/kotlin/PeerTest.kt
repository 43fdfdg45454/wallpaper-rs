package io.github.wallpaper_rs

import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.webkit.WebViewCompat
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class PeerTest : PageBase() {
    private fun frames(atStart: Boolean): JSONObject? {
        lateinit var bare: WebView
        instrumentation.runOnMainSync {
            val base = pages.page(painter)!!.web.url!!
            bare = WebView(context).apply {
                settings.javaScriptEnabled = true
                settings.blockNetworkLoads = true
                webViewClient = object : WebViewClient() {
                    override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? =
                        Page.answer(Native.serve(painter.handle, request.url.toString()))
                }
                if (atStart) WebViewCompat.addDocumentStartJavaScript(this, JSONObject(Native.serve(painter.handle, base + ":wprs/guard.js")).getString("text"), setOf(base.trimEnd('/')))
                loadUrl(base + "peers.html")
            }
        }
        var seen: JSONObject? = null
        waitFor { read("peers") { bare }.also { seen = it }?.length() == ROUTES }
        instrumentation.runOnMainSync { bare.destroy() }
        return seen
    }

    @Test
    fun framesBornBlankCannotOpenPeers() {
        until("the page runs") { it.has("own") }
        listOf(true, false).forEach { atStart ->
            val seen = frames(atStart)
            assertTrue("$atStart $seen", seen != null && seen.length() == ROUTES && seen.keys().asSequence().all { seen.getString(it) == "blocked" })
        }
        assertEquals(0, reached.get())
    }

    private companion object {
        const val ROUTES = 8
    }
}
