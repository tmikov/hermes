/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Usage: intl_shared_host <plugin> <script>. Creates a runtime from
// libhermesvm with IntlPluginPath = <plugin>, evaluates <script>, and prints
// its completion value. Unlike the hermes CLI it does not export JSI, so the
// plugin binds to libhermesvm's copy.

#include "hermes/hermes.h"

#include <fstream>
#include <iostream>
#include <sstream>

using namespace facebook;

/// Run the script in argv[2] with the plugin in argv[1].
int main(int argc, char **argv) {
  if (argc != 3)
    return 2;
  std::ifstream in(argv[2]);
  std::stringstream source;
  source << in.rdbuf();
  auto config = ::hermes::vm::RuntimeConfig::Builder()
                    .withIntlPluginPath(argv[1])
                    .build();
  auto rt = facebook::hermes::makeHermesRuntime(config);
  try {
    jsi::Value result = rt->evaluateJavaScript(
        std::make_shared<jsi::StringBuffer>(source.str()), argv[2]);
    std::cout << result.toString(*rt).utf8(*rt) << std::endl;
  } catch (const jsi::JSIException &e) {
    std::cout << "exception: " << e.what() << std::endl;
    return 1;
  }
  return 0;
}
