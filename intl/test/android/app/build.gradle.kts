/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Properties:
// - hermesMavenRepo (settings.gradle.kts): the Maven repository with the AAR;
// - hermesVersion: its version (default 0.0.0);
// - hermesJniLibs: a directory with <abi>/libjsi.so from the CMake build
//   that made the AAR, such as its build/intermediates/cmake/release/obj.
//   The AAR leaves libjsi.so out, since React Native supplies it, so this
//   app packages it and links against it. Nothing else is taken from there:
//   every other Hermes library must come from the AAR;
// - hermesAbis: the ABIs to build, separated by commas (default x86_64).

plugins { id("com.android.application") version "9.2.1" }

val hermesVersion = providers.gradleProperty("hermesVersion").getOrElse("0.0.0")
val hermesJniLibs = providers.gradleProperty("hermesJniLibs").get()
val hermesAbis = providers.gradleProperty("hermesAbis").getOrElse("x86_64").split(",")
// Only libjsi.so from hermesJniLibs, as jniLibs.
val jsiLibsDir = layout.buildDirectory.dir("jsi-libs").get().asFile
val copyJsiLibs by
    tasks.registering(Sync::class) {
      from(hermesJniLibs) { include("*/libjsi.so") }
      into(jsiLibsDir)
    }

tasks.matching { it.name == "preBuild" }.configureEach { dependsOn(copyJsiLibs) }

// The Hermes checkout this app is in, for the JSI headers, which the AAR's
// prefab headers leave out (React Native supplies those too).
val hermesSrc = rootDir.resolve("../../../..").canonicalPath

android {
  namespace = "com.facebook.hermes.intlapp"
  compileSdk = 36
  ndkVersion = "28.2.13676358"

  defaultConfig {
    applicationId = "com.facebook.hermes.intlapp"
    minSdk = 24
    targetSdk = 36
    versionCode = 1
    versionName = "1.0"
    ndk { abiFilters += hermesAbis }
    externalNativeBuild {
      cmake {
        arguments(
            "-DANDROID_STL=c++_shared",
            "-DHERMES_SRC=$hermesSrc",
            "-DHERMES_JNI_LIBS=$hermesJniLibs",
        )
      }
    }
  }

  externalNativeBuild {
    cmake {
      path = file("src/main/cpp/CMakeLists.txt")
      version = "3.31.6"
    }
  }

  buildFeatures { prefab = true }

  sourceSets { getByName("main") { jniLibs.directories.add(jsiLibsDir.path) } }

  packaging {
    // Keep the libraries uncompressed in the APK and load them from there
    // (extractNativeLibs=false), as AGP does by default for minSdk 23+.
    jniLibs.useLegacyPackaging = false
  }

  compileOptions {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
  }
}

dependencies {
  implementation("com.facebook.hermes:hermes-android:$hermesVersion")
  implementation("com.facebook.fbjni:fbjni:0.7.0")
  implementation("com.facebook.soloader:soloader:0.12.1")
}
