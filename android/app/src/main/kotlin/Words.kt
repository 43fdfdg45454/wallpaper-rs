package io.github.wallpaper_rs

import android.annotation.SuppressLint
import android.content.Context
import org.json.JSONObject

class Refused(message: String, val id: String = "", val args: List<String> = emptyList()) : Exception(message)

private val REPLY = setOf("error", "id", "args")

fun refusal(json: JSONObject): Refused? {
    if (json.opt("error") !is String || !json.keys().asSequence().all { it in REPLY }) return null
    val args = json.optJSONArray("args")?.let { list -> List(list.length(), list::getString) }.orEmpty()
    return Refused(json.getString("error"), json.optString("id"), args)
}

fun resourceName(id: String): String? =
    id.takeIf { it.isNotEmpty() && it.all { c -> c.isLetterOrDigit() || c == '.' || c == '_' } }?.let { "error_" + it.replace('.', '_') }

@SuppressLint("DiscouragedApi")
fun Context.told(error: Throwable): String {
    val refused = error as? Refused ?: return error.message.orEmpty()
    val found = resourceName(refused.id)?.let { resources.getIdentifier(it, "string", packageName) } ?: 0
    return if (found == 0) refused.message.orEmpty() else getString(found, *refused.args.toTypedArray())
}
