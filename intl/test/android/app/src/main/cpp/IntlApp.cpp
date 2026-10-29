/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// MainActivity.nativeRun: create a Hermes runtime with the default
// configuration, as React Native does, and report what the Intl plugin
// gives it, and where the plugin's code was mapped from.

#include <android/log.h>
#include <fbjni/fbjni.h>
#include <hermes/hermes.h>
#include <jsi/jsi.h>

#include <fstream>
#include <memory>
#include <string>

using namespace facebook;

namespace {

/// The script whose result nativeRun reports. Intl.Segmenter exists only in
/// the plugin: the AAR still has the old Intl, which has Intl but not it.
const char kScript[] = R"JS(
var lines = ["Intl.Segmenter: " + typeof Intl.Segmenter];
if (typeof Intl.Segmenter === "function") {
  lines.push("number: " + new Intl.NumberFormat("de").format(1234.5));
  var ro = new Intl.DateTimeFormat().resolvedOptions();
  lines.push("default locale: " + ro.locale);
  lines.push("default time zone: " + ro.timeZone);
}
lines.join("\n");
)JS";

/// The executable mappings in /proc/self/maps of libhermesvm.so and
/// libhermesintl.so, by name or (when mapped from the APK) by offset in it.
std::string hermesMaps() {
  std::ifstream maps("/proc/self/maps");
  std::string line, out;
  while (std::getline(maps, line)) {
    if (line.find(" r-xp ") != std::string::npos &&
        (line.find("libhermes") != std::string::npos ||
         line.find(".apk") != std::string::npos)) {
      out += "\nmapped: " + line;
    }
  }
  return out;
}

/// Create a runtime and return the script's result and the mappings.
jstring nativeRun(JNIEnv *env, jclass) {
  std::string result;
  try {
    std::unique_ptr<jsi::Runtime> rt = facebook::hermes::makeHermesRuntime();
    result = rt->evaluateJavaScript(
                   std::make_shared<jsi::StringBuffer>(kScript), "app.js")
                 .asString(*rt)
                 .utf8(*rt);
  } catch (const std::exception &e) {
    result = std::string("error: ") + e.what();
  }
  result += hermesMaps();
  return env->NewStringUTF(result.c_str());
}

} // namespace

/// Initialize fbjni with the JavaVM, as React Native does (the default
/// finalizer thread of a Hermes runtime attaches to it), and register
/// nativeRun.
JNIEXPORT jint JNI_OnLoad(JavaVM *vm, void *) {
  return jni::initialize(vm, [] {
    JNIEnv *env = jni::Environment::current();
    jclass cls = env->FindClass("com/facebook/hermes/intlapp/MainActivity");
    if (!cls) {
      // FindClass left a NoClassDefFoundError pending, which
      // System.loadLibrary throws.
      __android_log_print(
          ANDROID_LOG_ERROR, "HermesIntlApp", "MainActivity not found");
      return;
    }
    JNINativeMethod method = {
        "nativeRun",
        "()Ljava/lang/String;",
        reinterpret_cast<void *>(nativeRun)};
    if (env->RegisterNatives(cls, &method, 1) != JNI_OK) {
      __android_log_print(
          ANDROID_LOG_ERROR,
          "HermesIntlApp",
          "cannot register MainActivity.nativeRun");
    }
  });
}
