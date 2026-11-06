package io.github.wallpaper_rs

import android.content.Context
import android.content.Intent
import android.net.Uri

private const val READ = Intent.FLAG_GRANT_READ_URI_PERMISSION
private const val WRITE = Intent.FLAG_GRANT_WRITE_URI_PERMISSION

fun mirrored(tree: String) = Mirrors.plans().any { it.getString("tree") == tree }

fun Context.release(tree: String, modes: Int, mirrored: (String) -> Boolean = ::mirrored) {
    val stall = stallTree() == tree
    val kept = (if (stall || mirrored(tree)) READ else 0) or (if (stall) WRITE else 0)
    val gone = modes and kept.inv()
    if (gone != 0) runCatching { contentResolver.releasePersistableUriPermission(Uri.parse(tree), gone) }
}
