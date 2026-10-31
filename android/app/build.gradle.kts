import javax.inject.Inject

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

val rustLibs = layout.buildDirectory.dir("rustJniLibs").get().asFile.path
val workspace = rootDir.parentFile
val keystore = providers.environmentVariable("WPRS_KEYSTORE")
val keystorePassword = providers.environmentVariable("WPRS_KEYSTORE_PASSWORD")
val abis = providers.gradleProperty("abis").getOrElse("arm64-v8a,x86_64").split(',')
val home = providers.systemProperty("user.home").get()
val remaps = listOf(
    providers.environmentVariable("CARGO_HOME").getOrElse("$home/.cargo") to "/cargo",
    providers.environmentVariable("RUSTUP_HOME").getOrElse("$home/.rustup") to "/rustup",
    home to "/build",
    workspace.path to "/src",
)

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
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
    splits.abi {
        isEnable = true
        reset()
        include(*abis.toTypedArray())
        isUniversalApk = abis.size > 1
    }
    val signing = if (keystore.isPresent) signingConfigs.create("release") {
        storeFile = file(keystore.get())
        storePassword = keystorePassword.get()
        keyAlias = "wallpaper-rs"
        keyPassword = keystorePassword.get()
    } else signingConfigs.getByName("debug")
    buildTypes.getByName("debug") { signingConfig = signing }
    buildTypes.getByName("release") {
        signingConfig = signing
        isMinifyEnabled = true
        isShrinkResources = true
        proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "keep.pro")
    }
    buildFeatures { compose = true }
    sourceSets.named("main") {
        jniLibs.directories += rustLibs
        assets.directories += workspace.resolve("crates/wprs-depth/licenses").path
    }
    sourceSets.named("androidTest") { assets.directories += workspace.resolve("tests/fixtures").path }
    packaging {
        jniLibs {
            keepDebugSymbols += "**/*.so"
            excludes += (listOf("armeabi-v7a", "x86", "arm64-v8a", "x86_64", "riscv64") - abis).map { "lib/$it/**" }
            useLegacyPackaging = true
        }
    }
}

dependencies {
    implementation(platform(libs.compose.bom))
    implementation(libs.activity.compose)
    implementation(libs.compose.material3)
    implementation(libs.compose.icons)
    implementation(libs.media3.exoplayer)
    implementation(libs.work.runtime)
    testImplementation(libs.junit)
    androidTestImplementation(libs.test.runner)
    androidTestImplementation(libs.test.junit)
}

val cargo = tasks.register<Exec>("cargo") {
    workingDir = workspace
    environment("CARGO_ENCODED_RUSTFLAGS", remaps.joinToString("\u001f") { (from, to) -> "--remap-path-prefix=$from=$to" })
    environment("CFLAGS", remaps.joinToString(" ") { (from, to) -> "-ffile-prefix-map=$from=$to" })
    commandLine(
        listOf("cargo", "ndk") + abis.flatMap { listOf("-t", it) } +
            listOf("-P", "31", "-o", rustLibs, "build", "-p", "wprs-android", "--release", "--locked"),
    )
}

abstract class Examples : DefaultTask() {
    @get:InputDirectory
    abstract val scenes: DirectoryProperty

    @get:OutputDirectory
    abstract val assets: DirectoryProperty

    @get:Inject
    abstract val files: FileSystemOperations

    @TaskAction
    fun copy() {
        files.sync {
            from(scenes) {
                into("scenes")
                exclude { it.isDirectory && it.file.resolve("index.html").isFile }
                exclude { it.relativePath.segments.size > 1 && it.file.parentFile.resolve("index.html").isFile }
            }
            into(assets)
        }
    }
}

val examples = tasks.register<Examples>("examples") { scenes.set(workspace.resolve("scenes")) }

androidComponents.onVariants { it.sources.assets?.addGeneratedSourceDirectory(examples, Examples::assets) }

tasks.named("preBuild") { dependsOn(cargo) }
