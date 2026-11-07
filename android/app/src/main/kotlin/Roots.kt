package io.github.wallpaper_rs

import java.io.ByteArrayOutputStream
import java.io.DataOutputStream
import java.io.File
import java.security.KeyStore

// The native side trusts only its built-in roots, so it reads the certificate authorities that the
// user installed (a home server with its own CA) from this file, each after its length.
fun saveUserRoots(file: File) {
    val bytes = runCatching {
        val store = KeyStore.getInstance("AndroidCAStore").apply { load(null) }
        val out = ByteArrayOutputStream()
        DataOutputStream(out).use { data ->
            store.aliases().toList().filter { it.startsWith("user:") }.mapNotNull { store.getCertificate(it)?.encoded }
                .forEach { data.writeInt(it.size); data.write(it) }
        }
        out.toByteArray()
    }.getOrDefault(ByteArray(0))
    runCatching { file.writeBytes(bytes) }
}
