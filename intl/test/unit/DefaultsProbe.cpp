/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Prints the default locale and time zone that detection finds on the real
// host, one per line, for apple-defaults.js. It reads no arguments of its
// own, so CoreFoundation sees an argument-domain override such as
// -AppleLanguages '(sr-Latn-RS, de)'; the gtest binary's parser would reject
// it.

#include "Defaults.h"

#include <cstdio>

int main() {
  hermes_intl::HostEnvironment env;
  std::printf(
      "%s\n%s\n",
      hermes_intl::detectDefaultLocale(env).c_str(),
      hermes_intl::detectDefaultTimeZone(env).c_str());
  return 0;
}
