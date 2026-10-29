/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The CocoaPods consumer app of the Intl plugin (run-podapp.sh). It gets
// hermesvm.framework and hermesintl.framework from the hermes-engine pod,
// creates a runtime with the default configuration, writes what it finds to
// stdout and to tmp/podapp-result.txt in its data container, and exits.
// "Intl.Segmenter: function" means Intl comes from the plugin; "typeof Intl:
// undefined" means there is no plugin (Hermes has no Intl of its own), and
// the report then holds the results of Hermes's own locale fallbacks.

#import <Foundation/Foundation.h>

#include <hermes/hermes.h>
#include <jsi/jsi.h>

#include <mach-o/dyld.h>

#include <cstdio>
#include <cstring>
#include <exception>
#include <memory>
#include <string>

using namespace facebook;

namespace {

/// The script the app runs; its value is the report's JavaScript part.
const char kScript[] = R"JS(
var r = ["typeof Intl: " + typeof Intl];
if (typeof Intl === "undefined") {
  var d = new Date(Date.UTC(2026, 9, 27, 12, 0, 0));
  r.push('"a".localeCompare("b"): ' + "a".localeCompare("b"));
  r.push('"I".toLocaleLowerCase(): ' + "I".toLocaleLowerCase());
  r.push("toLocaleString: " + d.toLocaleString());
  r.push("toLocaleDateString: " + d.toLocaleDateString());
  r.push("toLocaleTimeString: " + d.toLocaleTimeString());
  r.push("Infinity.toLocaleString(): " + Infinity.toLocaleString());
  r.push("[1, 2].toLocaleString(): " + [1, 2].toLocaleString());
} else {
  r.push("Intl.Segmenter: " + typeof Intl.Segmenter);
}
if (typeof Intl !== "undefined" && typeof Intl.Segmenter === "function") {
  var ro = new Intl.DateTimeFormat().resolvedOptions();
  r.push("default locale: " + ro.locale);
  r.push("default time zone: " + ro.timeZone);
  r.push("number: " + new Intl.NumberFormat("de-DE").format(1234567.891));
}
r.join("\n");
)JS";

/// The architecture this process runs as.
const char *arch() {
#if defined(__arm64__)
  return "arm64";
#elif defined(__x86_64__)
  return "x86_64";
#else
  return "unknown";
#endif
}

} // namespace

int main(int argc, char *argv[]) {
  @autoreleasepool {
    std::string report = std::string("arch: ") + arch() + "\n";
    try {
      std::unique_ptr<jsi::Runtime> rt = facebook::hermes::makeHermesRuntime();
      jsi::Value result = rt->evaluateJavaScript(
          std::make_shared<jsi::StringBuffer>(kScript), "podapp.js");
      report += result.toString(*rt).utf8(*rt) + "\n";
    } catch (const std::exception &e) {
      report += std::string("error: ") + e.what() + "\n";
    }
    // Where the frameworks were loaded from.
    for (uint32_t i = 0, n = _dyld_image_count(); i < n; ++i) {
      const char *name = _dyld_get_image_name(i);
      if (name &&
          (std::strstr(name, "hermesvm.framework") ||
           std::strstr(name, "hermesintl"))) {
        report += std::string("image: ") + name + "\n";
      }
    }
    std::fputs(report.c_str(), stdout);
    std::fflush(stdout);
    NSString *path = [NSTemporaryDirectory()
        stringByAppendingPathComponent:@"podapp-result.txt"];
    [[NSString stringWithUTF8String:report.c_str()]
        writeToFile:path
         atomically:YES
           encoding:NSUTF8StringEncoding
              error:nil];
  }
  return 0;
}
