# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Tests for the Intl plugin loader (API/hermes/IntlPluginLoader.cpp) with
# in-process plugins. The fake plugin libraries for the lit tests in
# test/hermes/intl-plugin follow.
set(HERMES_ENABLE_EH ON)
set(HERMES_ENABLE_RTTI ON)
set(LLVM_ENABLE_RTTI ON)

add_hermes_unittest(IntlLoaderTests IntlLoaderTest.cpp)
target_link_libraries(IntlLoaderTests hermesvm_a)

# Fake plugin libraries for test/hermes/intl-plugin. Like the Node-API test
# addons they leave JSI undefined and bind to the host's exported copy, which
# Windows linkers do not allow.
if(NOT WIN32)
  function(add_fake_intl_plugin name)
    add_hermes_library(${name} SHARED fixtures/FakeIntlPlugin.cpp)
    target_compile_definitions(${name} PRIVATE ${ARGN})
    target_include_directories(${name} PRIVATE
      ${HERMES_JSI_DIR} ${PROJECT_SOURCE_DIR}/public
      ${PROJECT_SOURCE_DIR}/include)
    set_target_properties(${name} PROPERTIES SUFFIX ".so")
    if(APPLE)
      target_link_options(${name} PRIVATE -undefined dynamic_lookup)
    endif()
    set_property(GLOBAL APPEND PROPERTY HERMES_INTL_FIXTURE_TARGETS ${name})
  endfunction()

  add_fake_intl_plugin(fake_intl_good)
  add_fake_intl_plugin(fake_intl_no_entry FAKE_NO_ENTRY=1)
  add_fake_intl_plugin(fake_intl_null_entry FAKE_NULL_ENTRY=1)
  add_fake_intl_plugin(fake_intl_bad_interface FAKE_INTERFACE_DELTA=1)
  add_fake_intl_plugin(fake_intl_bad_bytecode FAKE_BYTECODE_DELTA=1)
  add_fake_intl_plugin(fake_intl_bad_flags FAKE_FLIP_FLAGS=1)
  add_fake_intl_plugin(fake_intl_throwing FAKE_THROWING_INSTALL=1)
  add_fake_intl_plugin(fake_intl_rejects_defaults FAKE_REJECT_DEFAULTS=1)

  # A plugin whose dependency cannot be found: found but broken, never
  # "not found". It has no RPATH, so the dependency is missing wherever the
  # plugin is copied.
  add_hermes_library(fake_intl_dep SHARED fixtures/FakeIntlDep.cpp)
  set_target_properties(fake_intl_dep PROPERTIES SUFFIX ".so")
  add_fake_intl_plugin(fake_intl_needs_dep FAKE_NEEDS_DEP=1)
  target_link_libraries(fake_intl_needs_dep PRIVATE fake_intl_dep)
  set_target_properties(fake_intl_needs_dep PROPERTIES SKIP_BUILD_RPATH ON)

  # An embedder that changes directory before creating its first runtime,
  # for default resolution of a relatively launched executable.
  add_hermes_executable(intl_chdir_host fixtures/ChdirHost.cpp
    LINK_LIBS hermesvm_a)
  hermes_export_dynamic_symbols(intl_chdir_host)
  # A host that gets Hermes and JSI from libhermesvm (shared), exporting no
  # JSI of its own, so the fake plugin binds to libhermesvm's copy.
  add_hermes_executable(intl_shared_host fixtures/SharedHost.cpp)
  target_link_libraries(intl_shared_host PRIVATE hermesvm)

  # A host that loads libhermesvm with dlopen by a relative path and then
  # changes directory. It links no VM code: it reaches the factory with
  # dlsym, and links JSI and hermesPublic for the code RuntimeConfig and the
  # JSI calls it makes need.
  add_hermes_executable(intl_relative_host fixtures/RelativeHost.cpp)
  target_include_directories(intl_relative_host PRIVATE
    ${PROJECT_SOURCE_DIR}/API ${HERMES_JSI_DIR} ${PROJECT_SOURCE_DIR}/public)
  target_link_libraries(intl_relative_host PRIVATE hermesPublic jsi
    ${CMAKE_DL_LIBS})

  set_property(GLOBAL APPEND PROPERTY HERMES_INTL_FIXTURE_TARGETS
    fake_intl_dep intl_chdir_host intl_shared_host intl_relative_host)
endif()
