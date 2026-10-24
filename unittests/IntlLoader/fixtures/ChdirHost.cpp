/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Changes to the directory in argv[1], then creates a runtime and reports
// whether the (fake) Intl plugin was installed by default resolution.

#include "hermes/hermes.h"

#include <unistd.h>
#include <cstdio>

/// Usage: intl_chdir_host [<dir>]. Prints "plugin" or "no plugin".
int main(int argc, char **argv) {
  if (argc > 1 && ::chdir(argv[1]) != 0)
    return 2;
  auto rt = facebook::hermes::makeHermesRuntime();
  bool installed = rt->global().hasProperty(*rt, "__fakeIntlPlugin");
  std::printf("%s\n", installed ? "plugin" : "no plugin");
  return 0;
}
