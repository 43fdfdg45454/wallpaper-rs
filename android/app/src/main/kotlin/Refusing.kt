package io.github.wallpaper_rs

import android.net.Uri
import android.os.Message
import android.view.View
import android.webkit.ConsoleMessage
import android.webkit.GeolocationPermissions
import android.webkit.JsPromptResult
import android.webkit.JsResult
import android.webkit.PermissionRequest
import android.webkit.ServiceWorkerClient
import android.webkit.ServiceWorkerController
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView

class Refusing : WebChromeClient() {
    override fun onPermissionRequest(request: PermissionRequest) = request.deny()

    override fun onGeolocationPermissionsShowPrompt(origin: String, callback: GeolocationPermissions.Callback) =
        callback.invoke(origin, false, false)

    override fun onJsAlert(view: WebView, url: String, message: String, result: JsResult) = true.also { result.cancel() }

    override fun onJsConfirm(view: WebView, url: String, message: String, result: JsResult) = true.also { result.cancel() }

    override fun onJsBeforeUnload(view: WebView, url: String, message: String, result: JsResult) = true.also { result.confirm() }

    override fun onJsPrompt(view: WebView, url: String, message: String, value: String?, result: JsPromptResult) =
        true.also { result.cancel() }

    override fun onCreateWindow(view: WebView, dialog: Boolean, gesture: Boolean, result: Message) = false

    override fun onShowFileChooser(view: WebView, callback: ValueCallback<Array<Uri>>, params: FileChooserParams) = false

    override fun onShowCustomView(view: View, callback: CustomViewCallback) = callback.onCustomViewHidden()

    override fun onConsoleMessage(message: ConsoleMessage) = true

    companion object {
        @Volatile
        private var workers = false

        fun workers() {
            if (workers) return
            workers = true
            ServiceWorkerController.getInstance().apply {
                serviceWorkerWebSettings.apply {
                    blockNetworkLoads = true
                    allowContentAccess = false
                    allowFileAccess = false
                }
                setServiceWorkerClient(object : ServiceWorkerClient() {
                    override fun shouldInterceptRequest(request: WebResourceRequest): WebResourceResponse =
                        Page.answer("{}")!!
                })
            }
        }
    }
}
