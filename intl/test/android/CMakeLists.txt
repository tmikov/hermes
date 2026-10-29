# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# The Android smoke test (smoke.cpp): it links libhermesvm.so and libjsi.so,
# as an app does, and is run on a device or emulator (intl/README.md).
set(HERMES_ENABLE_EH ON)
set(HERMES_ENABLE_RTTI ON)

add_hermes_executable(hermes-intl-android-smoke smoke.cpp)
# fbjni too, although the smoke test calls none of it: libhermesvm.so needs
# it, and the ARM translation of an x86_64 image ignores LD_LIBRARY_PATH,
# so only the executable's RUNPATH below can find it there.
target_link_libraries(hermes-intl-android-smoke PRIVATE hermesvm jsi)
if(TARGET fbjni::fbjni)
  target_link_libraries(hermes-intl-android-smoke PRIVATE fbjni::fbjni)
endif()
# The libraries are looked up next to the executable, then in ../lib.
# CMake sets no RPATH for Android, so pass it to the linker.
target_link_options(hermes-intl-android-smoke PRIVATE
  "LINKER:-rpath,$ORIGIN:$ORIGIN/../lib")
set_target_properties(hermes-intl-android-smoke PROPERTIES
  RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)
