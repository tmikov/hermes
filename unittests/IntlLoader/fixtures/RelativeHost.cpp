/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Loads libhermesvm with dlopen by the path in argv[1] (relative, in the
// test), RTLD_LOCAL, changes to the directory in argv[2], then creates a
// runtime through the public factory and reports whether the (fake) Intl
// plugin was installed by default resolution. With "delete" as argv[3] it
// removes the library's file after loading it. dladdr reports the path a
// library was loaded by, so a relative one no longer names the library
// after the change of directory.

#include "hermes/hermes.h"

#include <dlfcn.h>
#include <unistd.h>
#include <cstdio>
#include <cstring>

/// facebook::hermes::makeHermesRuntime(const hermes::vm::RuntimeConfig &),
/// mangled per the Itanium C++ ABI.
static const char kFactory[] =
    "_ZN8facebook6hermes17makeHermesRuntimeERKN6hermes2vm13RuntimeConfigE";

/// Usage: intl_relative_host <libhermesvm> <dir> [delete]. Prints "plugin"
/// or "no plugin".
int main(int argc, char **argv) {
  if (argc != 3 && !(argc == 4 && std::strcmp(argv[3], "delete") == 0))
    return 2;
  void *vm = ::dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
  if (!vm) {
    std::fprintf(stderr, "%s\n", ::dlerror());
    return 2;
  }
  auto make = reinterpret_cast<decltype(&facebook::hermes::makeHermesRuntime)>(
      ::dlsym(vm, kFactory));
  if (!make) {
    std::fprintf(stderr, "%s\n", ::dlerror());
    return 2;
  }
  if (argc == 4 && ::unlink(argv[1]) != 0)
    return 2;
  if (::chdir(argv[2]) != 0)
    return 2;
  auto rt = make(::hermes::vm::RuntimeConfig());
  bool installed = rt->global().hasProperty(*rt, "__fakeIntlPlugin");
  std::printf("%s\n", installed ? "plugin" : "no plugin");
  return 0;
}
