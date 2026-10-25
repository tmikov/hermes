# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# The ICU4X-based Intl plugin: a separate shared library, libhermesintl, that
# Hermes loads when it creates a runtime. See
# doc/superpowers/specs/2026-10-22-intl-plugin-design.md.

# HERMES_INTL_UNSUPPORTED_REASON is set in the top-level CMakeLists.txt.
if(HERMES_INTL_UNSUPPORTED_REASON)
  message(FATAL_ERROR
    "HERMES_BUILD_INTL: the Intl plugin builds only for the host on Linux "
    "and macOS for now, not for ${HERMES_INTL_UNSUPPORTED_REASON}. "
    "Configure with -DHERMES_BUILD_INTL=OFF.")
endif()
if(CMAKE_CONFIGURATION_TYPES)
  message(FATAL_ERROR
    "HERMES_BUILD_INTL: multi-configuration generators are not supported "
    "yet. Use Ninja or Makefiles, or configure with -DHERMES_BUILD_INTL=OFF.")
endif()

# The MSRV of icu 2.3.
set(HERMES_INTL_RUST_MIN_VERSION 1.88)
find_program(HERMES_INTL_CARGO cargo)
if(NOT HERMES_INTL_CARGO)
  message(FATAL_ERROR
    "HERMES_BUILD_INTL is ON, which requires a Rust toolchain, but cargo was "
    "not found on PATH. Install Rust (https://rustup.rs) or configure with "
    "-DHERMES_BUILD_INTL=OFF.")
endif()
execute_process(
  COMMAND ${HERMES_INTL_CARGO} --version
  OUTPUT_VARIABLE _cargo_version_out
  OUTPUT_STRIP_TRAILING_WHITESPACE
  RESULT_VARIABLE _cargo_rc)
string(REGEX MATCH "[0-9]+\\.[0-9]+(\\.[0-9]+)?" _cargo_version
  "${_cargo_version_out}")
if(NOT _cargo_rc EQUAL 0 OR
   _cargo_version VERSION_LESS HERMES_INTL_RUST_MIN_VERSION)
  message(FATAL_ERROR
    "HERMES_BUILD_INTL needs Rust ${HERMES_INTL_RUST_MIN_VERSION} or newer; "
    "${HERMES_INTL_CARGO} --version printed '${_cargo_version_out}'. Run "
    "'rustup update' or configure with -DHERMES_BUILD_INTL=OFF.")
endif()

# --- The Rust core -----------------------------------------------------------

set(HERMES_INTL_CORE_DIR ${CMAKE_CURRENT_SOURCE_DIR}/core)
set(_cargo_target_dir ${CMAKE_CURRENT_BINARY_DIR}/cargo)
if(CMAKE_BUILD_TYPE STREQUAL "Debug")
  set(HERMES_INTL_CARGO_PROFILE_ARGS "")
  set(_cargo_profile_dir debug)
else()
  set(HERMES_INTL_CARGO_PROFILE_ARGS --release)
  set(_cargo_profile_dir release)
endif()
set(_core_lib
  ${_cargo_target_dir}/${_cargo_profile_dir}/${CMAKE_STATIC_LIBRARY_PREFIX}hermes_intl_core${CMAKE_STATIC_LIBRARY_SUFFIX})
set(_core_include_dir ${CMAKE_CURRENT_BINARY_DIR}/include)
set(_core_header ${_core_include_dir}/hermes_intl_core.h)
# Imported targets require their include directories to exist at configure
# time.
file(MAKE_DIRECTORY ${_core_include_dir})
file(GLOB_RECURSE _core_sources CONFIGURE_DEPENDS
  ${HERMES_INTL_CORE_DIR}/src/*.rs ${HERMES_INTL_CORE_DIR}/data/*)
set(HERMES_INTL_CARGO_ENV
  ${CMAKE_COMMAND} -E env
  CARGO_TARGET_DIR=${_cargo_target_dir}
  HERMES_INTL_CORE_HEADER=${_core_header})

add_custom_command(
  OUTPUT ${_core_lib} ${_core_header}
  COMMAND ${HERMES_INTL_CARGO_ENV} ${HERMES_INTL_CARGO} build --locked --lib
    ${HERMES_INTL_CARGO_PROFILE_ARGS}
    --manifest-path ${HERMES_INTL_CORE_DIR}/Cargo.toml
  DEPENDS ${_core_sources}
    ${HERMES_INTL_CORE_DIR}/Cargo.toml ${HERMES_INTL_CORE_DIR}/Cargo.lock
    ${HERMES_INTL_CORE_DIR}/build.rs ${HERMES_INTL_CORE_DIR}/cbindgen.toml
  COMMENT "Building the Intl plugin's Rust core"
  USES_TERMINAL
  VERBATIM)
add_custom_target(hermesIntlCoreBuild DEPENDS ${_core_lib} ${_core_header})

find_package(Threads REQUIRED)
add_library(hermesIntlCore STATIC IMPORTED GLOBAL)
set_target_properties(hermesIntlCore PROPERTIES
  IMPORTED_LOCATION ${_core_lib}
  INTERFACE_INCLUDE_DIRECTORIES ${_core_include_dir})
# What the Rust standard library needs from the system (from
# `cargo rustc -- --print native-static-libs`).
target_link_libraries(hermesIntlCore INTERFACE Threads::Threads
  ${CMAKE_DL_LIBS})
if(NOT APPLE)
  target_link_libraries(hermesIntlCore INTERFACE m)
endif()
add_dependencies(hermesIntlCore hermesIntlCoreBuild)

# --- The plugin library ------------------------------------------------------

# The bytecode version of the hermesc that compiles intl/js. The loader
# rejects a plugin whose version differs from the engine's. Reconfigure when
# the header changes, so the plugin never carries a stale version.
set(_bytecode_version_header
  ${PROJECT_SOURCE_DIR}/include/hermes/BCGen/HBC/BytecodeVersion.h)
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
  ${_bytecode_version_header})
file(STRINGS ${_bytecode_version_header}
  _bytecode_version_line REGEX "BYTECODE_VERSION = [0-9]+")
# Take the number after "=", not the first in the line (uint32_t's 32).
string(REGEX REPLACE ".*BYTECODE_VERSION = ([0-9]+).*" "\\1"
  HERMES_INTL_BYTECODE_VERSION "${_bytecode_version_line}")
if(NOT HERMES_INTL_BYTECODE_VERSION MATCHES "^[0-9]+$")
  message(FATAL_ERROR "Could not read BYTECODE_VERSION from "
    "${_bytecode_version_header}")
endif()

add_subdirectory(js)

set(HERMES_ENABLE_EH ON)
set(HERMES_ENABLE_RTTI ON)
set(HERMES_INTL_PLUGIN_SOURCES
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/CollatorHelpers.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/StringHelpers.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/Defaults.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/IntlBytecode.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/LocaleHelpers.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/NativeHelpers.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/NumberHelpers.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/PluralRulesHelpers.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/Plugin.cpp)
add_hermes_library(hermesintl SHARED ${HERMES_INTL_PLUGIN_SOURCES})
target_include_directories(hermesintl PRIVATE
  ${HERMES_JSI_DIR} ${PROJECT_SOURCE_DIR}/public
  ${CMAKE_CURRENT_BINARY_DIR}/js)
target_compile_definitions(hermesintl PRIVATE
  HERMES_INTL_BYTECODE_VERSION=${HERMES_INTL_BYTECODE_VERSION})
target_link_libraries(hermesintl PRIVATE hermesIntlCore)
add_dependencies(hermesintl hermesIntlBytecode)
# Next to libhermesvm, where the loader's default resolution looks.
set_target_properties(hermesintl PROPERTIES
  LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib)
# Drop unreferenced sections, as hermesvm does (add_hermes_library does not
# apply these flags to a shared library). Most of ICU4X's compiled data is
# never referenced, so this removes most of the library's size. The exported
# entry point below is a root, so the export rules are unaffected.
target_link_options(hermesintl PRIVATE ${HERMES_EXTRA_LINKER_FLAGS})
# Export only hermes_intl_get_entry: nothing from the Rust core, and no C++
# inline or template instances. The JSI types' typeinfo stays undefined and
# binds to the host's copy, so exceptions still cross the boundary.
if(APPLE)
  # JSI binds to the host's exported copy when the library is loaded.
  target_link_options(hermesintl PRIVATE -undefined dynamic_lookup
    -Wl,-exported_symbol,_hermes_intl_get_entry)
  target_link_libraries(hermesintl PRIVATE "-framework CoreFoundation")
else()
  set(_hermesintl_version_script
    ${CMAKE_CURRENT_SOURCE_DIR}/jsi/hermesintl.version)
  target_link_options(hermesintl PRIVATE
    -Wl,--version-script=${_hermesintl_version_script})
  set_property(TARGET hermesintl APPEND PROPERTY LINK_DEPENDS
    ${_hermesintl_version_script})
endif()

# --- Tests -------------------------------------------------------------------

if(HERMES_ENABLE_TEST_SUITE)
  set(HERMES_INTL_LIT_PARAMS
    intl_test_exec_root=${CMAKE_CURRENT_BINARY_DIR}/test/lit
    intl_plugin=$<TARGET_FILE:hermesintl>
    intl_js_dir=${CMAKE_CURRENT_SOURCE_DIR}/js
    hermes=${HERMES_TOOLS_OUTPUT_DIR}/hermes
    hermesc=${HERMES_TOOLS_OUTPUT_DIR}/hermesc
    hvm=${HERMES_TOOLS_OUTPUT_DIR}/hvm
    hermes_jsi_shared=${HERMES_TOOLS_OUTPUT_DIR}/hermes-jsi-shared
    shermes=${HERMES_TOOLS_OUTPUT_DIR}/shermes
    hermesvm_lib=$<TARGET_FILE:hermesvm>
    FileCheck=${HERMES_TOOLS_OUTPUT_DIR}/FileCheck
    cmake=${CMAKE_COMMAND}
    tools_dir=${HERMES_TOOLS_OUTPUT_DIR}
    jsi_unstable=${JSI_UNSTABLE})
  set(HERMES_INTL_LIT_SUITES ${CMAKE_CURRENT_SOURCE_DIR}/test/lit)
  set(HERMES_INTL_LIT_DEPENDS hermesintl hermes hermesc hvm
    hermes-jsi-shared shermes-dep)

  add_custom_target(HermesIntlUnitTests)
  add_subdirectory(test/unit)
  list(APPEND HERMES_INTL_LIT_PARAMS
    intl_unittests_dir=${CMAKE_CURRENT_BINARY_DIR}/test/unit)
  list(APPEND HERMES_INTL_LIT_SUITES ${CMAKE_CURRENT_SOURCE_DIR}/test/unit)
  list(APPEND HERMES_INTL_LIT_DEPENDS HermesIntlUnitTests)

  add_lit_testsuite(check-intl-lit "Running the Intl plugin lit tests"
    ${HERMES_INTL_LIT_SUITES}
    PARAMS ${HERMES_INTL_LIT_PARAMS}
    DEPENDS ${HERMES_INTL_LIT_DEPENDS})

  add_custom_target(check-intl-rust
    COMMAND ${HERMES_INTL_CARGO_ENV} ${HERMES_INTL_CARGO} test --locked
      ${HERMES_INTL_CARGO_PROFILE_ARGS}
      --manifest-path ${HERMES_INTL_CORE_DIR}/Cargo.toml
    COMMENT "Running the Intl plugin's Rust tests"
    USES_TERMINAL
    VERBATIM)

  set(HERMES_INTL_TEST262_DIR "" CACHE PATH
    "A tc39/test262 checkout for check-intl, at the commit named in intl/test/test262-expectations.txt")
  set(_t262 ${HERMES_INTL_TEST262_DIR}/test)
  if(HERMES_INTL_TEST262_DIR)
    add_custom_target(check-intl-test262
      COMMAND ${Python_EXECUTABLE} ${PROJECT_SOURCE_DIR}/utils/test_runner.py
        --intl-plugin $<TARGET_FILE:hermesintl>
        --intl-expectations ${CMAKE_CURRENT_SOURCE_DIR}/test/test262-expectations.txt
        -b ${HERMES_TOOLS_OUTPUT_DIR}
        ${_t262}/intl402
        ${_t262}/built-ins/String/prototype/localeCompare
        ${_t262}/built-ins/String/prototype/toLocaleLowerCase
        ${_t262}/built-ins/String/prototype/toLocaleUpperCase
        ${_t262}/built-ins/Number/prototype/toLocaleString
        ${_t262}/built-ins/BigInt/prototype/toLocaleString
        ${_t262}/built-ins/Array/prototype/toLocaleString
        ${_t262}/built-ins/TypedArray/prototype/toLocaleString
        ${_t262}/built-ins/Date/prototype/toLocaleString
        ${_t262}/built-ins/Date/prototype/toLocaleDateString
        ${_t262}/built-ins/Date/prototype/toLocaleTimeString
      DEPENDS hermesintl hermes hermesc
      COMMENT "Running test262 intl402 against the Intl plugin"
      USES_TERMINAL
      VERBATIM)
  else()
    add_custom_target(check-intl-test262
      COMMAND ${CMAKE_COMMAND} -E echo
        "check-intl needs test262: configure with -DHERMES_INTL_TEST262_DIR=<a tc39/test262 checkout at the commit in intl/test/test262-expectations.txt>"
      COMMAND ${CMAKE_COMMAND} -E false
      VERBATIM)
  endif()

  add_custom_target(check-intl-runner
    COMMAND ${CMAKE_COMMAND} -E env PYTHONPATH=${PROJECT_SOURCE_DIR}/utils
      ${Python_EXECUTABLE} -m unittest testsuite.test_intl_plugin
    COMMENT "Running the test262 plugin-mode unit tests"
    VERBATIM)

  add_custom_target(check-intl-data
    COMMAND ${Python_EXECUTABLE} -B
      ${HERMES_INTL_CORE_DIR}/data/test_update_cldr_lists.py
    COMMENT "Running the unit tests of the CLDR list generator"
    VERBATIM)

  add_custom_target(check-intl)
  add_dependencies(check-intl check-intl-rust check-intl-lit check-intl-runner
    check-intl-data check-intl-test262)
endif()
