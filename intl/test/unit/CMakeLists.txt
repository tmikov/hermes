# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Unit tests of the plugin's C++, compiled into the test rather than loaded
# from the plugin library. NativeHelpersTest and SetupObjectsTest need a
# runtime, from hermesvm_a.
set(HERMES_ENABLE_EH ON)
set(HERMES_ENABLE_RTTI ON)
set(LLVM_ENABLE_RTTI ON)

add_unittest(HermesIntlUnitTests IntlPluginUnitTests
  DefaultsTest.cpp
  NativeHelpersTest.cpp
  SetupObjectsTest.cpp
  ${PROJECT_SOURCE_DIR}/intl/jsi/CollatorHelpers.cpp
  ${PROJECT_SOURCE_DIR}/intl/jsi/StringHelpers.cpp
  ${PROJECT_SOURCE_DIR}/intl/jsi/Defaults.cpp
  ${PROJECT_SOURCE_DIR}/intl/jsi/LocaleHelpers.cpp
  ${PROJECT_SOURCE_DIR}/intl/jsi/NativeHelpers.cpp)
target_include_directories(IntlPluginUnitTests PRIVATE
  ${PROJECT_SOURCE_DIR}/intl/jsi ${PROJECT_SOURCE_DIR}/public
  ${HERMES_JSI_DIR})
target_link_libraries(IntlPluginUnitTests hermesIntlCore hermesvm_a)
if(APPLE)
  # Defaults.cpp's Apple branch calls CoreFoundation.
  target_link_libraries(IntlPluginUnitTests "-framework CoreFoundation")

  # A plain executable, not a gtest, so that it can take CoreFoundation's
  # argument-domain overrides; apple-defaults.js in test/lit runs it.
  add_hermes_executable(IntlDefaultsProbe
    DefaultsProbe.cpp
    ${PROJECT_SOURCE_DIR}/intl/jsi/Defaults.cpp
    LINK_LIBS hermesIntlCore "-framework CoreFoundation")
  target_include_directories(IntlDefaultsProbe PRIVATE
    ${PROJECT_SOURCE_DIR}/intl/jsi ${PROJECT_SOURCE_DIR}/public)
  add_dependencies(HermesIntlUnitTests IntlDefaultsProbe)
endif()

# Kept apart from IntlPluginUnitTests: it replaces the global operator new and
# delete to make an allocation fail, which must not touch the other tests.
add_unittest(HermesIntlUnitTests IntlRustCoreAllocTests
  RustCoreAllocTest.cpp)
target_include_directories(IntlRustCoreAllocTests PRIVATE
  ${PROJECT_SOURCE_DIR}/intl/jsi)
target_link_libraries(IntlRustCoreAllocTests hermesIntlCore)
