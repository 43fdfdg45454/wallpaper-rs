plugins {
    alias(libs.plugins.android.application)
}

val rustLibs = layout.buildDirectory.dir("rustJniLibs").get().asFile.path
val workspace = rootDir.parentFile
val keystore = providers.environmentVariable("WPRS_KEYSTORE")
val keystorePassword = providers.environmentVariable("WPRS_KEYSTORE_PASSWORD")

android {
    namespace = "io.github.wallpaper_rs"
    compileSdk = 37
    defaultConfig {
        applicationId = "io.github.wallpaper_rs"
        minSdk = 31
        targetSdk = 36
        versionCode = providers.gradleProperty("versionCode").getOrElse("1").toInt()
        versionName = providers.gradleProperty("versionName").orNull ?: workspace.resolve("Cargo.toml").useLines { lines ->
            lines.first { it.startsWith("version = ") }.substringAfter('"').substringBefore('"')
        }
    }
    if (keystore.isPresent) signingConfigs.getByName("debug") {
        storeFile = file(keystore.get())
        storePassword = keystorePassword.get()
        keyAlias = "wallpaper-rs"
        keyPassword = keystorePassword.get()
    }
    sourceSets.named("main") { jniLibs.directories += rustLibs }
    packaging { jniLibs { keepDebugSymbols += "**/*.so" } }
}

dependencies {
    implementation(libs.activity)
}

val cargo = tasks.register<Exec>("cargo") {
    workingDir = workspace
    commandLine(
        "cargo", "ndk", "-t", "arm64-v8a", "-t", "x86_64", "-P", "31", "-o", rustLibs,
        "build", "-p", "wprs-android", "--release", "--locked",
    )
}

tasks.named("preBuild") { dependsOn(cargo) }
