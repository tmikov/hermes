/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The Apple smoke test of the Intl plugin: an executable that gets Hermes
// and JSI from hermesvm.framework, as an app does, creates a runtime with
// the default configuration (so default resolution looks for
// hermesintl.framework next to hermesvm.framework, and the defaults come
// from the system's preferences), and prints a few Intl results. It is run
// on the simulators with `xcrun simctl spawn` and natively on macOS and Mac
// Catalyst, from the build tree (bin/ next to lib/) and from an app-style
// layout (Frameworks/ next to the executable).
//
// It prints "Intl.Segmenter: undefined" when no plugin was loaded: Apple
// framework builds keep the old Intl, which has no Segmenter. It then
// prints the paths of the hermesvm and hermesintl images in the process,
// which shows where the plugin was found.

#include <hermes/hermes.h>
#include <jsi/jsi.h>

#include <mach-o/dyld.h>

#include <cstdio>
#include <cstring>
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
  print("number:", new Intl.NumberFormat("de-DE").format(1234567.891));
  print("date:", new Intl.DateTimeFormat("ja-JP", {
    timeZone: "Asia/Tokyo", dateStyle: "full", timeStyle: "long"
  }).format(0));
  print("collation:", ["z", "ä", "a"].sort(new Intl.Collator("sv").compare)
    .join());
  print("segments:", Array.from(new Intl.Segmenter("th", {
    granularity: "word"
  }).segment("สวัสดีครับ"))
    .length);
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

/// Print the path of each loaded image whose path contains
/// "hermesvm.framework" or "hermesintl".
void printImages() {
  for (uint32_t i = 0, n = _dyld_image_count(); i < n; ++i) {
    const char *name = _dyld_get_image_name(i);
    if (name &&
        (std::strstr(name, "hermesvm.framework") ||
         std::strstr(name, "hermesintl"))) {
      std::printf("image: %s\n", name);
    }
  }
}

} // namespace

int main() {
  std::unique_ptr<jsi::Runtime> rt = facebook::hermes::makeHermesRuntime();
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
  printImages();
  return 0;
}
