package io.github.wallpaper_rs

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import org.json.JSONObject

@Composable
fun hdrState(hdr: JSONObject?): String? {
    hdr ?: return null
    val why = when (hdr.optString("reason")) {
        "battery" -> R.string.hdr_why_battery
        "quality" -> R.string.hdr_why_quality
        "off" -> R.string.hdr_why_off
        else -> R.string.hdr_why_display
    }
    val state = if (hdr.optBoolean("active")) {
        stringResource(R.string.hdr_state_on, "%.1f".format(hdr.optDouble("headroom")))
    } else {
        stringResource(R.string.hdr_state_off, stringResource(why))
    }
    val answer = stringResource(if (hdr.optBoolean("display")) R.string.yes else R.string.no)
    return "$state. " + stringResource(R.string.hdr_screen, answer, "%.0f".format(hdr.optDouble("peak_nits")))
}
