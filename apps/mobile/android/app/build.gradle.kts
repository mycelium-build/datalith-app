plugins { id("com.android.application") version "9.1.0" }

android {
    namespace = "build.mycelium.datalith"
    compileSdk = 34
    ndkVersion = "27.3.13750724"
    defaultConfig {
        applicationId = "build.mycelium.datalith"
        testInstrumentationRunner = "build.mycelium.datalith.FolderInstrumentation"
        minSdk = 31
        targetSdk = 34
        versionCode = 1
        versionName = "0.1"
    }
    // These dependencies are statically linked into the NativeActivity library.
    packaging.jniLibs.excludes += setOf("**/libgpui_mobile.so", "**/libturso_sdk_kit.so", "**/libturso_sync_sdk_kit.so")
    sourceSets.getByName("main") {
        java.srcDir("build/gpui-java")
        jniLibs.srcDir("build/native")
    }
}
