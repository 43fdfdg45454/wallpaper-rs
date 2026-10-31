package io.github.wallpaper_rs

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.os.Build
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import java.net.Inet4Address
import java.net.InetAddress

const val SERVICE = "_wprs._tcp"

fun address(hosts: List<InetAddress>, port: Int): String? {
    val host = hosts.sortedBy { it !is Inet4Address }.firstOrNull { !it.isLinkLocalAddress && !it.isLoopbackAddress } ?: return null
    val text = host.hostAddress ?: return null
    return if (host is Inet4Address) "https://$text:$port" else "https://[$text]:$port"
}

private class Finder(context: Context, val found: MutableMap<String, String>) : NsdManager.DiscoveryListener {
    private val nsd = context.getSystemService(NsdManager::class.java)
    private val executor = context.mainExecutor

    fun start() = runCatching { nsd.discoverServices(SERVICE, NsdManager.PROTOCOL_DNS_SD, this) }

    fun stop() = runCatching { nsd.stopServiceDiscovery(this) }

    override fun onServiceFound(info: NsdServiceInfo) {
        val show = { seen: NsdServiceInfo -> address(seen.hosts(), seen.port)?.let { executor.execute { found[info.serviceName] = it } } }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            nsd.registerServiceInfoCallback(info, executor, object : NsdManager.ServiceInfoCallback {
                override fun onServiceUpdated(seen: NsdServiceInfo) {
                    show(seen)
                    runCatching { nsd.unregisterServiceInfoCallback(this) }
                }
                override fun onServiceLost() {}
                override fun onServiceInfoCallbackRegistrationFailed(code: Int) {}
                override fun onServiceInfoCallbackUnregistered() {}
            })
        } else {
            @Suppress("DEPRECATION")
            nsd.resolveService(info, object : NsdManager.ResolveListener {
                override fun onServiceResolved(seen: NsdServiceInfo) {
                    show(seen)
                }
                override fun onResolveFailed(seen: NsdServiceInfo, code: Int) {}
            })
        }
    }

    override fun onServiceLost(info: NsdServiceInfo) = executor.execute { found.remove(info.serviceName) }
    override fun onDiscoveryStarted(type: String) {}
    override fun onDiscoveryStopped(type: String) {}
    override fun onStartDiscoveryFailed(type: String, code: Int) {}
    override fun onStopDiscoveryFailed(type: String, code: Int) {}
}

@Suppress("DEPRECATION")
private fun NsdServiceInfo.hosts(): List<InetAddress> =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) hostAddresses else listOfNotNull(host)

@Composable
fun Nearby(pick: (String) -> Unit) {
    val context = LocalContext.current
    val found = remember { mutableStateMapOf<String, String>() }
    DisposableEffect(Unit) {
        val finder = Finder(context, found).apply { start() }
        onDispose { finder.stop() }
    }
    if (found.isNotEmpty()) Text(stringResource(R.string.lan_found), style = MaterialTheme.typography.titleSmall)
    found.forEach { (name, url) ->
        ListItem(headlineContent = { Text(name) }, supportingContent = { Text(url) }, trailingContent = {
            TextButton({ pick(url) }) { Text(stringResource(R.string.repo_add)) }
        })
    }
}
