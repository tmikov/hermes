/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "llvh/Support/ErrorHandling.h"

extern "C" void __cxa_throw(void* thrown_exception,
    std::type_info* tinfo,
    void (*dest)(void*)) {
  llvh::report_fatal_error("C++ exceptions not supported on Wasi");
}

extern "C" void* __cxa_allocate_exception(size_t) {
  llvh::report_fatal_error("C++ exceptions not supported on Wasi");
}
