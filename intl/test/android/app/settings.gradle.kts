/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// A test app that loads Hermes from the hermes-android AAR the way React
// Native does (Java, SoLoader, prefab), to check that default resolution
// finds the Intl plugin in an app. See intl/README.md, "On an Android
// device".

pluginManagement {
  repositories {
    google()
    mavenCentral()
    gradlePluginPortal()
  }
}

dependencyResolutionManagement {
  repositories {
    // A local Maven repository holding the hermes-android AAR under test:
    // -PhermesMavenRepo=<dir>.
    maven { url = uri(providers.gradleProperty("hermesMavenRepo").get()) }
    google()
    mavenCentral()
  }
}

rootProject.name = "hermes-intl-test-app"
