/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The Android smoke test of the Intl plugin: an executable that gets Hermes
// from libhermesvm.so and JSI from libjsi.so, as an app does, creates a
// runtime with the default configuration (so default resolution finds the
// plugin and the defaults come from the device), and prints a few Intl
// results. intl/README.md describes the layouts it is run in.
//
// It prints "Intl.Segmenter: undefined" and stops when no plugin was
// loaded: the Android build keeps the old, Java-backed Intl, which cannot
// run in a process without a JVM. With --maps it then prints the files the
// code of libhermesvm.so and libhermesintl.so was mapped from, which shows
// where the plugin was found (an APK is one file).

#include <hermes/hermes.h>
#include <jsi/jsi.h>

#include <cstdio>
#include <cstring>
#include <fstream>
#include <memory>
#include <string>

using namespace facebook;

namespace {

/// The script the smoke test runs; print() is the host function below.
const char kScript[] = R"JS(
print("Intl.Segmenter:", typeof Intl.Segmenter);
if (typeof Intl.Segmenter === "function") {
  var ro = new Intl.DateTimeFormat().resolvedOptions();
  print("default locale:", ro.locale);
  print("default time zone:", ro.timeZone);
  print("number:", new Intl.NumberFormat("de").format(1234.5));
  print("date:", new Intl.DateTimeFormat("en-US", {
    dateStyle: "medium", timeStyle: "long", timeZone: "Asia/Kolkata"
  }).format(Date.UTC(2026, 9, 27, 12, 0, 0)));
  print("list:", new Intl.ListFormat("en").format(["a", "b", "c"]));
  print("segments:", Array.from(new Intl.Segmenter("en", {
    granularity: "word"
  }).segment("Hello, world!")).length);
}
)JS";

/// print(...args): write the arguments, converted to strings and separated
/// by spaces, and a newline to stdout.
jsi::Value print(
    jsi::Runtime &rt,
    const jsi::Value &,
    const jsi::Value *args,
    size_t count) {
  std::string line;
  for (size_t i = 0; i < count; ++i) {
    if (i)
      line += ' ';
    line += args[i].toString(rt).utf8(rt);
  }
  std::printf("%s\n", line.c_str());
  return jsi::Value::undefined();
}

/// Print each executable mapping in /proc/self/maps of a file whose path
/// contains "libhermesvm.so", "libhermesintl.so" or ".apk".
void printMaps() {
  std::ifstream maps("/proc/self/maps");
  std::string line;
  while (std::getline(maps, line)) {
    if (line.find(" r-xp ") == std::string::npos)
      continue;
    if (line.find("libhermesvm.so") != std::string::npos ||
        line.find("libhermesintl.so") != std::string::npos ||
        line.find(".apk") != std::string::npos) {
      std::printf("mapped: %s\n", line.c_str());
    }
  }
}

} // namespace

int main(int argc, char **argv) {
  bool maps = argc > 1 && !std::strcmp(argv[1], "--maps");
  // No JVM here, so the finalizer thread must not attach to one (the
  // Android default does).
  auto config = ::hermes::vm::RuntimeConfig::Builder()
                    .withFinalizerThreadRunner(::hermes::vm::ThreadRunner{})
                    .build();
  std::unique_ptr<jsi::Runtime> rt =
      facebook::hermes::makeHermesRuntime(config);
  rt->global().setProperty(
      *rt,
      "print",
      jsi::Function::createFromHostFunction(
          *rt, jsi::PropNameID::forAscii(*rt, "print"), 1, print));
  try {
    rt->evaluateJavaScript(
        std::make_shared<jsi::StringBuffer>(kScript), "smoke.js");
  } catch (const jsi::JSIException &e) {
    std::printf("error: %s\n", e.what());
    return 1;
  }
  if (maps)
    printMaps();
  return 0;
}
