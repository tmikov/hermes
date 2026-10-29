# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# The ICU4X-based Intl plugin: a separate shared library, libhermesintl, that
# Hermes loads when it creates a runtime. See
# doc/superpowers/specs/2026-10-22-intl-plugin-design.md.

# HERMES_INTL_UNSUPPORTED_REASON, HERMES_INTL_RUST_TRIPLES,
# HERMES_INTL_DEPLOYMENT_VAR, HERMES_INTL_CARGO, HERMES_INTL_CARGO_SEARCHED
# and HERMES_INTL_CARGO_HOST are set in the top-level CMakeLists.txt (see
# cmake/modules/HermesIntl.cmake).
if(EMSCRIPTEN)
  message(FATAL_ERROR
    "HERMES_BUILD_INTL is ON, but the plugin is not built for Emscripten. "
    "Configure with -DHERMES_BUILD_INTL=OFF; an embedder can still register "
    "a plugin with registerIntlPlugin.")
elseif(HERMES_INTL_UNSUPPORTED_REASON)
  message(FATAL_ERROR
    "HERMES_BUILD_INTL is ON, but ${HERMES_INTL_UNSUPPORTED_REASON}, or "
    "configure with -DHERMES_BUILD_INTL=OFF.")
endif()
# The plugin's JavaScript is compiled by hermesc at build time, so a build
# whose own hermesc cannot run here needs the host's.
hermes_intl_foreign_hermesc(_foreign_hermesc)
if(_foreign_hermesc AND NOT IMPORT_HOST_COMPILERS)
  message(FATAL_ERROR
    "HERMES_BUILD_INTL is ON in ${_foreign_hermesc}, whose hermesc cannot "
    "run on this machine to compile the plugin's JavaScript. Build Hermes "
    "for the host and configure with "
    "-DIMPORT_HOST_COMPILERS=<host build>/ImportHostCompilers.cmake, or "
    "with -DHERMES_BUILD_INTL=OFF.")
endif()

if(NOT HERMES_INTL_CARGO)
  message(FATAL_ERROR
    "HERMES_BUILD_INTL is ON, which requires a Rust toolchain, but cargo was "
    "not found ${HERMES_INTL_CARGO_SEARCHED}. Install Rust "
    "(https://rustup.rs) or configure with -DHERMES_BUILD_INTL=OFF.")
endif()
# A host build builds for this triple, and check-intl-rust always does.
if(NOT HERMES_INTL_CARGO_HOST)
  message(FATAL_ERROR
    "HERMES_BUILD_INTL: '${HERMES_INTL_CARGO} -vV' printed no host triple. "
    "Check the Rust installation or configure with -DHERMES_BUILD_INTL=OFF.")
endif()
list(LENGTH HERMES_INTL_RUST_TRIPLES _rust_triple_count)
hermes_intl_check_rust_triples("${HERMES_INTL_RUST_TRIPLES}" _triples_error)
if(_triples_error)
  message(FATAL_ERROR
    "HERMES_BUILD_INTL: cannot build the Rust core for "
    "${HERMES_INTL_RUST_TRIPLES} together: ${_triples_error}. Several "
    "triples are combined with lipo into one archive. Set "
    "HERMES_INTL_RUST_TARGET to one triple, or to the architectures of one "
    "Apple platform.")
endif()

# The MSRV of icu 2.3. rustup ships the standard library for the tvOS and
# visionOS targets from Rust 1.95.
set(HERMES_INTL_RUST_MIN_VERSION 1.88)
if(HERMES_INTL_RUST_TRIPLES MATCHES "-apple-(tvos|visionos)")
  set(HERMES_INTL_RUST_MIN_VERSION 1.95)
endif()
execute_process(
  COMMAND ${HERMES_INTL_CARGO} --version
  WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/core
  OUTPUT_VARIABLE _cargo_version_out
  OUTPUT_STRIP_TRAILING_WHITESPACE
  RESULT_VARIABLE _cargo_rc)
string(REGEX MATCH "[0-9]+\\.[0-9]+(\\.[0-9]+)?" _cargo_version
  "${_cargo_version_out}")
if(NOT _cargo_rc EQUAL 0 OR
   _cargo_version VERSION_LESS HERMES_INTL_RUST_MIN_VERSION)
  message(FATAL_ERROR
    "HERMES_BUILD_INTL needs Rust ${HERMES_INTL_RUST_MIN_VERSION} or newer "
    "for ${HERMES_INTL_RUST_TRIPLES}; ${HERMES_INTL_CARGO} --version printed "
    "'${_cargo_version_out}'. Run 'rustup update' or configure with "
    "-DHERMES_BUILD_INTL=OFF.")
endif()

# Each triple's standard library must be installed; otherwise cargo fails
# late, with "can't find crate for `core`".
get_filename_component(_cargo_bin_dir ${HERMES_INTL_CARGO} DIRECTORY)
find_program(HERMES_INTL_RUSTC rustc HINTS ${_cargo_bin_dir})
execute_process(
  COMMAND ${HERMES_INTL_RUSTC} --print sysroot
  WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/core
  OUTPUT_VARIABLE _rust_sysroot
  OUTPUT_STRIP_TRAILING_WHITESPACE
  RESULT_VARIABLE _rustc_rc)
if(NOT HERMES_INTL_RUSTC OR NOT _rustc_rc EQUAL 0)
  message(FATAL_ERROR
    "HERMES_BUILD_INTL: could not run '${HERMES_INTL_RUSTC} --print "
    "sysroot'. Check the Rust installation or configure with "
    "-DHERMES_BUILD_INTL=OFF.")
endif()
foreach(_t IN LISTS HERMES_INTL_RUST_TRIPLES)
  if(NOT IS_DIRECTORY ${_rust_sysroot}/lib/rustlib/${_t}/lib)
    message(FATAL_ERROR
      "HERMES_BUILD_INTL: the Rust standard library for ${_t} is not "
      "installed (no ${_rust_sysroot}/lib/rustlib/${_t}/lib). Run "
      "'rustup target add ${_t}', or configure with "
      "-DHERMES_BUILD_INTL=OFF.")
  endif()
endforeach()

# The plugin binds JSI to the one copy Hermes itself uses. On Android that is
# libjsi.so, which libhermesvm.so also links (an app gets it from React
# Native); a static JSI would put a second copy of jsi.cpp in the process.
if(ANDROID AND NOT HERMES_BUILD_SHARED_JSI)
  message(FATAL_ERROR
    "HERMES_BUILD_INTL on Android needs HERMES_BUILD_SHARED_JSI=ON: the "
    "plugin links libjsi.so, the copy of JSI libhermesvm.so uses. Configure "
    "with -DHERMES_BUILD_SHARED_JSI=ON, or with -DHERMES_BUILD_INTL=OFF.")
endif()

# Windows: Hermes exports no JSI from any module, so the plugin links its own
# copy, the static jsi target. A shared jsi target is a jsi.dll that exports
# nothing, which the plugin could not link against.
if(WIN32 AND HERMES_BUILD_SHARED_JSI)
  message(FATAL_ERROR
    "HERMES_BUILD_INTL on Windows needs HERMES_BUILD_SHARED_JSI=OFF: the "
    "plugin links its own static copy of JSI. Configure with "
    "-DHERMES_BUILD_SHARED_JSI=OFF, or with -DHERMES_BUILD_INTL=OFF.")
endif()

# MSVC: the plugin interface passes std::string and std::vector, which one
# module allocates and the other frees, so the plugin and Hermes must share
# one C runtime heap: the C runtime DLL (/MD, or /MDd in Debug). With a
# static runtime (/MT, /MTd) each module would have its own heap.
if(MSVC)
  # The flags of every configuration this build can select.
  set(_crt_flag_vars CMAKE_C_FLAGS CMAKE_CXX_FLAGS)
  set(_crt_configs ${CMAKE_CONFIGURATION_TYPES} ${CMAKE_BUILD_TYPE}
    Debug Release RelWithDebInfo MinSizeRel)
  list(TRANSFORM _crt_configs TOUPPER)
  list(REMOVE_DUPLICATES _crt_configs)
  foreach(_config IN LISTS _crt_configs)
    list(APPEND _crt_flag_vars
      CMAKE_C_FLAGS_${_config} CMAKE_CXX_FLAGS_${_config})
  endforeach()
  hermes_intl_static_crt("${CMAKE_MSVC_RUNTIME_LIBRARY}" _static_crt
    ${_crt_flag_vars})
  if(_static_crt)
    message(FATAL_ERROR
      "HERMES_BUILD_INTL on Windows needs the C runtime DLL (/MD or /MDd), "
      "but the build selects a static C runtime (${_static_crt}). The plugin "
      "and Hermes pass C++ strings and vectors to each other, so they must "
      "share one runtime heap. Use CMAKE_MSVC_RUNTIME_LIBRARY="
      "MultiThreaded$<$<CONFIG:Debug>:Debug>DLL (the default; generator "
      "expressions are accepted only in that form), or configure with "
      "-DHERMES_BUILD_INTL=OFF.")
  endif()
  # The plugin's strings are UTF-8, as on every other platform. Without
  # /utf-8, cl encodes narrow literals in the build machine's ANSI code
  # page, so " " becomes the single byte 0xA0 and a character outside
  # that code page becomes "?". This applies to the plugin and its tests.
  add_compile_options(/utf-8)
endif()

# --- The Rust core -----------------------------------------------------------

# cargo builds the core once per triple, into
# <cargo>/<triple>/<debug|release>/, with the profile chosen per build
# configuration (hermes_intl_cargo_profile_dir), so multi-configuration
# generators get the right library per configuration. Several Apple triples
# are combined with lipo into <cargo>/universal/<debug|release>/.
set(HERMES_INTL_CORE_DIR ${CMAKE_CURRENT_SOURCE_DIR}/core)
set(_cargo_target_dir ${CMAKE_CURRENT_BINARY_DIR}/cargo)
set(HERMES_INTL_CARGO_PROFILE_ARGS $<$<NOT:$<CONFIG:Debug>>:--release>)
set(_cargo_profile_dir $<IF:$<CONFIG:Debug>,debug,release>)
set(_core_lib_name
  ${CMAKE_STATIC_LIBRARY_PREFIX}hermes_intl_core${CMAKE_STATIC_LIBRARY_SUFFIX})
set(_core_include_dir ${CMAKE_CURRENT_BINARY_DIR}/include)
set(_core_header ${_core_include_dir}/hermes_intl_core.h)
# Imported targets require their include directories to exist at configure
# time.
file(MAKE_DIRECTORY ${_core_include_dir})
file(GLOB_RECURSE _core_sources CONFIGURE_DEPENDS
  ${HERMES_INTL_CORE_DIR}/src/*.rs ${HERMES_INTL_CORE_DIR}/data/*)
set(_core_deps ${_core_sources}
  ${HERMES_INTL_CORE_DIR}/Cargo.toml ${HERMES_INTL_CORE_DIR}/Cargo.lock
  ${HERMES_INTL_CORE_DIR}/build.rs ${HERMES_INTL_CORE_DIR}/cbindgen.toml)
set(HERMES_INTL_CARGO_ENV
  ${CMAKE_COMMAND} -E env CARGO_TARGET_DIR=${_cargo_target_dir})
# The Rust objects' minimum OS version, so the Apple linker does not warn
# that they are newer than the C++ code.
set(_cargo_deployment_env "")
if(HERMES_INTL_DEPLOYMENT_VAR AND CMAKE_OSX_DEPLOYMENT_TARGET)
  set(_cargo_deployment_env
    ${HERMES_INTL_DEPLOYMENT_VAR}=${CMAKE_OSX_DEPLOYMENT_TARGET})
endif()

# The C toolchain of an Android build, for a crate that compiles C through
# the cc crate. The core has none today, and cargo never links a staticlib,
# so this only keeps such a crate on the NDK and API level the C++ uses.
set(_cargo_cc_env "")
if(ANDROID)
  foreach(_t IN LISTS HERMES_INTL_RUST_TRIPLES)
    string(REPLACE "-" "_" _t_ ${_t})
    set(_cflags
      "--target=${CMAKE_C_COMPILER_TARGET} --sysroot=${CMAKE_SYSROOT}")
    list(APPEND _cargo_cc_env
      CC_${_t_}=${CMAKE_C_COMPILER}
      "CFLAGS_${_t_}=${_cflags}"
      AR_${_t_}=${CMAKE_AR})
  endforeach()
endif()

# One command per triple: build.rs writes the cbindgen header, so only the
# first command asks for it, and two builds never write it at once. Cargo
# runs from intl/core, as at configure, so that a rustup toolchain override
# there applies to the build too (--manifest-path does not select it).
set(_core_libs "")
set(_core_header_env HERMES_INTL_CORE_HEADER=${_core_header})
set(_core_header_output ${_core_header})
foreach(_t IN LISTS HERMES_INTL_RUST_TRIPLES)
  set(_lib ${_cargo_target_dir}/${_t}/${_cargo_profile_dir}/${_core_lib_name})
  list(APPEND _core_libs ${_lib})
  add_custom_command(
    OUTPUT ${_lib} ${_core_header_output}
    COMMAND ${HERMES_INTL_CARGO_ENV} ${_core_header_env}
      ${_cargo_deployment_env} ${_cargo_cc_env}
      ${HERMES_INTL_CARGO} build --locked --lib
      ${HERMES_INTL_CARGO_PROFILE_ARGS} --target ${_t}
      --manifest-path ${HERMES_INTL_CORE_DIR}/Cargo.toml
    WORKING_DIRECTORY ${HERMES_INTL_CORE_DIR}
    DEPENDS ${_core_deps}
    COMMENT "Building the Intl plugin's Rust core for ${_t}"
    USES_TERMINAL
    VERBATIM
    # Drops the profile argument when it is empty.
    COMMAND_EXPAND_LISTS)
  set(_core_header_env "")
  set(_core_header_output "")
endforeach()
if(_rust_triple_count GREATER 1)
  find_program(HERMES_INTL_LIPO NAMES lipo llvm-lipo)
  if(NOT HERMES_INTL_LIPO)
    message(FATAL_ERROR
      "HERMES_BUILD_INTL: lipo, which combines the Rust core of "
      "${HERMES_INTL_RUST_TRIPLES} into one archive, was not found. Install "
      "the Xcode command line tools (xcode-select --install), pass "
      "-DHERMES_INTL_LIPO=<path to lipo>, build for a single architecture "
      "(CMAKE_OSX_ARCHITECTURES), or configure with -DHERMES_BUILD_INTL=OFF.")
  endif()
  set(_core_lib_dir ${_cargo_target_dir}/universal/${_cargo_profile_dir})
  set(_core_lib ${_core_lib_dir}/${_core_lib_name})
  add_custom_command(
    OUTPUT ${_core_lib}
    COMMAND ${CMAKE_COMMAND} -E make_directory ${_core_lib_dir}
    COMMAND ${HERMES_INTL_LIPO} -create -output ${_core_lib} ${_core_libs}
    DEPENDS ${_core_libs}
    COMMENT "Combining the Intl plugin's Rust core for each architecture"
    VERBATIM)
  set(_core_lib_dir_rel universal)
else()
  set(_core_lib ${_core_libs})
  set(_core_lib_dir_rel ${HERMES_INTL_RUST_TRIPLES})
endif()
add_custom_target(hermesIntlCoreBuild DEPENDS ${_core_lib} ${_core_header})

add_library(hermesIntlCore STATIC IMPORTED GLOBAL)
set_target_properties(hermesIntlCore PROPERTIES
  INTERFACE_INCLUDE_DIRECTORIES ${_core_include_dir})
# IMPORTED_LOCATION takes no generator expressions, so give each
# configuration its own.
if(CMAKE_CONFIGURATION_TYPES)
  set(_configs ${CMAKE_CONFIGURATION_TYPES})
else()
  set(_configs ${CMAKE_BUILD_TYPE})
endif()
foreach(_config IN LISTS _configs)
  string(TOUPPER ${_config} _uconfig)
  hermes_intl_cargo_profile_dir(${_config} _profile)
  set_property(TARGET hermesIntlCore APPEND PROPERTY
    IMPORTED_CONFIGURATIONS ${_uconfig})
  set_target_properties(hermesIntlCore PROPERTIES
    IMPORTED_LOCATION_${_uconfig}
    ${_cargo_target_dir}/${_core_lib_dir_rel}/${_profile}/${_core_lib_name})
endforeach()
# What the Rust standard library needs from the system on the target.
list(GET HERMES_INTL_RUST_TRIPLES 0 _first_triple)
hermes_intl_rust_native_libs(${_first_triple} HERMES_INTL_RUST_NATIVE_LIBS)
if("Threads::Threads" IN_LIST HERMES_INTL_RUST_NATIVE_LIBS)
  find_package(Threads REQUIRED)
endif()
target_link_libraries(hermesIntlCore INTERFACE ${HERMES_INTL_RUST_NATIVE_LIBS})
add_dependencies(hermesIntlCore hermesIntlCoreBuild)
message(VERBOSE "Intl plugin: Rust targets ${HERMES_INTL_RUST_TRIPLES}; "
  "system libraries ${HERMES_INTL_RUST_NATIVE_LIBS}")

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
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/DateTimeFormatHelpers.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/DisplayNamesHelpers.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/StringHelpers.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/Defaults.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/IntlBytecode.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/ListFormatHelpers.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/LocaleHelpers.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/NativeHelpers.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/NumberHelpers.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/PluralRulesHelpers.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/RelativeTimeFormatHelpers.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/SegmenterHelpers.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/Plugin.cpp)
add_hermes_library(hermesintl SHARED ${HERMES_INTL_PLUGIN_SOURCES})
target_include_directories(hermesintl PRIVATE
  ${HERMES_JSI_DIR} ${PROJECT_SOURCE_DIR}/public
  ${CMAKE_CURRENT_BINARY_DIR}/js)
target_compile_definitions(hermesintl PRIVATE
  HERMES_INTL_BYTECODE_VERSION=${HERMES_INTL_BYTECODE_VERSION})
target_link_libraries(hermesintl PRIVATE hermesIntlCore)
add_dependencies(hermesintl hermesIntlBytecode)
# Where the loader's default resolution finds it: next to hermesvm.framework
# in a framework build, else in lib/ or, under a multi-configuration
# generator and on Windows, next to the tools.
hermes_intl_plugin_output_dir(hermesintl hermesvm "${HERMES_TOOLS_OUTPUT_DIR}")
# Drop unreferenced sections, as hermesvm does (add_hermes_library does not
# apply these flags to a shared library). Most of ICU4X's compiled data is
# never referenced, so this removes most of the library's size. The exported
# entry point below is a root, so the export rules are unaffected.
target_link_options(hermesintl PRIVATE ${HERMES_EXTRA_LINKER_FLAGS})
# Export only hermes_intl_get_entry: nothing from the Rust core, and no C++
# inline or template instances. The JSI types' typeinfo stays undefined and
# binds to the host's copy, so exceptions still cross the boundary (except
# on Windows; see below).
if(APPLE)
  target_link_options(hermesintl PRIVATE
    -Wl,-exported_symbol,_hermes_intl_get_entry)
  target_link_libraries(hermesintl PRIVATE "-framework CoreFoundation")
  if(HERMES_BUILD_APPLE_FRAMEWORK)
    # hermesintl.framework, which an app embeds next to hermesvm.framework,
    # where the loader's default resolution finds it. It links its JSI
    # provider, hermesvm.framework (which does not pass JSI on, so this adds
    # no second copy), and finds it through an LC_RPATH relative to itself.
    # The rpath replaces CMake's absolute build-tree one, since the build
    # tree's framework is the one that ships.
    hermes_apple_framework(hermesintl
      dev.hermesengine.intl.${HERMES_APPLE_TARGET_PLATFORM})
    target_link_libraries(hermesintl PRIVATE hermesvm)
    hermes_intl_framework_rpath("${HERMES_APPLE_TARGET_PLATFORM}" _rpath)
    set_target_properties(hermesintl PROPERTIES
      BUILD_WITH_INSTALL_RPATH TRUE
      INSTALL_RPATH ${_rpath})
  else()
    # JSI binds to the host's exported copy when the library is loaded.
    target_link_options(hermesintl PRIVATE -undefined dynamic_lookup)
  endif()
  # The plugin is built for the architectures its Rust core has, which in
  # the tvOS and visionOS simulator builds leave out x86_64. An x86_64
  # process there finds an arm64-only plugin, which the loader treats as no
  # plugin.
  hermes_intl_plugin_osx_architectures("${HERMES_INTL_RUST_TRIPLES}"
    "${CMAKE_OSX_ARCHITECTURES}" _plugin_archs)
  if(_plugin_archs)
    set_target_properties(hermesintl PROPERTIES
      OSX_ARCHITECTURES "${_plugin_archs}")
    message(VERBOSE "Intl plugin: architectures ${_plugin_archs}")
  endif()
elseif(WIN32)
  # A DLL exports only what is __declspec(dllexport), which Plugin.cpp
  # applies to hermes_intl_get_entry alone. Its imports name their DLLs at
  # link time, and no Hermes module exports JSI, so the plugin links its own
  # copy: each module then has its own JSI type information, and MSVC's
  # runtime matches C++ types across modules by decorated name, so
  # exceptions and NativeState casts still cross the boundary.
  target_link_libraries(hermesintl PRIVATE jsi)
  # Drop unreferenced sections in every configuration: a /DEBUG link turns
  # /OPT:REF off by default, and ICU4X's unreferenced data is most of the
  # library.
  target_link_options(hermesintl PRIVATE /OPT:REF)
else()
  set(_hermesintl_version_script
    ${CMAKE_CURRENT_SOURCE_DIR}/jsi/hermesintl.version)
  target_link_options(hermesintl PRIVATE
    -Wl,--version-script=${_hermesintl_version_script})
  set_property(TARGET hermesintl APPEND PROPERTY LINK_DEPENDS
    ${_hermesintl_version_script})
  if(ANDROID)
    # Link the JSI provider, libjsi.so (NEEDED), as libhermesvm.so does, so
    # the plugin binds to the app's one libjsi.so in any process. Every
    # reference must bind at link time.
    target_link_libraries(hermesintl PRIVATE jsi)
    target_link_options(hermesintl PRIVATE -Wl,--no-undefined)
  endif()
endif()

# The Apple smoke test (test/apple), run from the build tree and from an
# app-style layout on the simulators and on macOS. Framework builds have no
# test suite, so it is built only when asked for.
if(APPLE AND HERMES_BUILD_APPLE_FRAMEWORK)
  add_subdirectory(test/apple)
endif()

# --- Tests -------------------------------------------------------------------

if(HERMES_ENABLE_TEST_SUITE)
  set(HERMES_INTL_LIT_PARAMS
    intl_test_exec_root=${CMAKE_CURRENT_BINARY_DIR}/test/lit
    intl_plugin=$<TARGET_FILE:hermesintl>
    intl_js_dir=${CMAKE_CURRENT_SOURCE_DIR}/js
    hermes=$<TARGET_FILE:hermes>
    hermesc=$<TARGET_FILE:hermesc>
    hvm=$<TARGET_FILE:hvm>
    hermes_jsi_shared=$<TARGET_FILE:hermes-jsi-shared>
    shermes=$<TARGET_FILE:shermes>
    hermesvm_lib=$<TARGET_FILE:hermesvm>
    FileCheck=$<TARGET_FILE:FileCheck>
    cmake=${CMAKE_COMMAND}
    tools_dir=$<TARGET_FILE_DIR:hermes>
    jsi_unstable=${JSI_UNSTABLE})
  set(HERMES_INTL_LIT_SUITES ${CMAKE_CURRENT_SOURCE_DIR}/test/lit)
  set(HERMES_INTL_LIT_DEPENDS hermesintl hermes hermesc hvm
    hermes-jsi-shared shermes-dep)

  add_custom_target(HermesIntlUnitTests)
  add_subdirectory(test/unit)
  if(ANDROID)
    add_subdirectory(test/android)
  endif()
  list(APPEND HERMES_INTL_LIT_PARAMS
    intl_unittests_dir=$<TARGET_FILE_DIR:IntlPluginUnitTests>)
  list(APPEND HERMES_INTL_LIT_SUITES ${CMAKE_CURRENT_SOURCE_DIR}/test/unit)
  list(APPEND HERMES_INTL_LIT_DEPENDS HermesIntlUnitTests)

  add_lit_testsuite(check-intl-lit "Running the Intl plugin lit tests"
    ${HERMES_INTL_LIT_SUITES}
    PARAMS ${HERMES_INTL_LIT_PARAMS}
    DEPENDS ${HERMES_INTL_LIT_DEPENDS})

  # The tests run on the host, so they build for the host triple, even in
  # a cross build. Passing the header too keeps the build script's inputs
  # those of the host build of the library, which then shares their output.
  # From intl/core, like the build.
  add_custom_target(check-intl-rust
    COMMAND ${HERMES_INTL_CARGO_ENV} HERMES_INTL_CORE_HEADER=${_core_header}
      ${HERMES_INTL_CARGO} test --locked ${HERMES_INTL_CARGO_PROFILE_ARGS}
      --target ${HERMES_INTL_CARGO_HOST}
      --manifest-path ${HERMES_INTL_CORE_DIR}/Cargo.toml
    WORKING_DIRECTORY ${HERMES_INTL_CORE_DIR}
    COMMENT "Running the Intl plugin's Rust tests"
    USES_TERMINAL
    VERBATIM
    COMMAND_EXPAND_LISTS)

  set(HERMES_INTL_TEST262_DIR "" CACHE PATH
    "A tc39/test262 checkout for check-intl, at the commit named in intl/test/test262-expectations.txt")
  set(_t262 ${HERMES_INTL_TEST262_DIR}/test)
  if(HERMES_INTL_TEST262_DIR)
    add_custom_target(check-intl-test262
      COMMAND ${Python_EXECUTABLE} ${PROJECT_SOURCE_DIR}/utils/test_runner.py
        --intl-plugin $<TARGET_FILE:hermesintl>
        --intl-expectations ${CMAKE_CURRENT_SOURCE_DIR}/test/test262-expectations.txt
        -b $<TARGET_FILE_DIR:hermes>
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
