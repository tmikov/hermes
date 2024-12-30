# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

link_libraries(shermes_console_a hermesvmlean_a jsi)
link_libraries(setjmp)
link_libraries(wasi-emulated-mman)

add_executable(sh-demo demo.c cxa.cpp)
