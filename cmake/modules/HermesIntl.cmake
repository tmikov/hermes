# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# How the Intl plugin (intl/) chooses what cargo builds its Rust core for,
# used by the top-level CMakeLists.txt and intl/CMakeLists.txt.
# intl/test/lit/rust-targets.js and osx-architectures.js run the pure
# functions here through cmake -P.

# Set HERMES_INTL_CARGO (a cache variable) to cargo: the one on PATH, else
# the one in $CARGO_HOME/bin or ~/.cargo/bin, where rustup installs it, since
# a build started outside a login shell (a Gradle daemon or an Xcode build
# phase launched from the GUI) may not have those on PATH. Set
# HERMES_INTL_CARGO_SEARCHED to where it was looked for, a phrase such as
# "on PATH or in /home/u/.cargo/bin", and HERMES_INTL_CARGO_HOST to the host
# triple its `cargo -vV` names. The host is empty when cargo is not found;
# intl/CMakeLists.txt reports that.
macro(hermes_intl_find_cargo)
  set(_hermes_intl_cargo_dirs "")
  if(NOT "$ENV{CARGO_HOME}" STREQUAL "")
    file(TO_CMAKE_PATH "$ENV{CARGO_HOME}/bin" _hermes_intl_dir)
    list(APPEND _hermes_intl_cargo_dirs "${_hermes_intl_dir}")
  endif()
  if(NOT "$ENV{HOME}" STREQUAL "")
    file(TO_CMAKE_PATH "$ENV{HOME}/.cargo/bin" _hermes_intl_dir)
    list(APPEND _hermes_intl_cargo_dirs "${_hermes_intl_dir}")
  endif()
  list(REMOVE_DUPLICATES _hermes_intl_cargo_dirs)
  # PATHS are searched after PATH.
  find_program(HERMES_INTL_CARGO cargo PATHS ${_hermes_intl_cargo_dirs})
  set(HERMES_INTL_CARGO_SEARCHED "on PATH")
  if(_hermes_intl_cargo_dirs)
    list(JOIN _hermes_intl_cargo_dirs " or " _hermes_intl_dir)
    string(APPEND HERMES_INTL_CARGO_SEARCHED " or in ${_hermes_intl_dir}")
  endif()
  set(HERMES_INTL_CARGO_HOST "")
  if(HERMES_INTL_CARGO)
    # From intl/core, so that a rustup toolchain override there applies.
    execute_process(
      COMMAND ${HERMES_INTL_CARGO} -vV
      WORKING_DIRECTORY ${PROJECT_SOURCE_DIR}/intl/core
      OUTPUT_VARIABLE _hermes_intl_cargo_vv
      ERROR_QUIET)
    hermes_intl_cargo_host("${_hermes_intl_cargo_vv}" HERMES_INTL_CARGO_HOST)
  endif()
endmacro()

# Set \p out to the host triple in \p cargo_vv, the output of `cargo -vV`
# ("host: x86_64-unknown-linux-gnu"), or to "" if it has no host line.
function(hermes_intl_cargo_host cargo_vv out)
  set(host "")
  if(cargo_vv MATCHES "(^|\n)host: ([^\n]+)")
    string(STRIP "${CMAKE_MATCH_2}" host)
  endif()
  set(${out} "${host}" PARENT_SCOPE)
endfunction()

# Set \p out to the Rust triple for Apple platform \p platform (a
# HERMES_APPLE_TARGET_PLATFORM value; macosx for plain macOS) and
# architecture \p arch (a CMAKE_OSX_ARCHITECTURES entry), or to "" if Rust
# has no supported target for the pair. Rust has no supported x86_64 target
# for the tvOS and visionOS simulators, so those pairs give SKIP: the plugin
# is arm64-only there, and the loader treats an x86_64 process that finds
# the arm64-only plugin as having no plugin.
function(_hermes_intl_apple_triple platform arch out)
  set(t "")
  if(arch STREQUAL "arm64")
    set(rarch aarch64)
  elseif(arch STREQUAL "x86_64")
    set(rarch x86_64)
  else()
    set(rarch "")
  endif()
  if(NOT rarch)
    # No triple.
  elseif(platform STREQUAL "macosx")
    set(t ${rarch}-apple-darwin)
  elseif(platform STREQUAL "iphoneos")
    if(rarch STREQUAL "aarch64")
      set(t aarch64-apple-ios)
    endif()
  elseif(platform STREQUAL "iphonesimulator")
    if(rarch STREQUAL "aarch64")
      set(t aarch64-apple-ios-sim)
    else()
      set(t x86_64-apple-ios)
    endif()
  elseif(platform STREQUAL "catalyst")
    set(t ${rarch}-apple-ios-macabi)
  elseif(platform STREQUAL "appletvos")
    if(rarch STREQUAL "aarch64")
      set(t aarch64-apple-tvos)
    endif()
  elseif(platform STREQUAL "appletvsimulator")
    if(rarch STREQUAL "aarch64")
      set(t aarch64-apple-tvos-sim)
    else()
      set(t SKIP)
    endif()
  elseif(platform STREQUAL "xros")
    if(rarch STREQUAL "aarch64")
      set(t aarch64-apple-visionos)
    endif()
  elseif(platform STREQUAL "xrsimulator")
    if(rarch STREQUAL "aarch64")
      set(t aarch64-apple-visionos-sim)
    else()
      set(t SKIP)
    endif()
  endif()
  set(${out} "${t}" PARENT_SCOPE)
endfunction()

# Set \p out to the environment variable that gives cargo the minimum OS
# version for Apple triple \p triple, or to "" for other triples. Catalyst
# (-ios-macabi) and the iOS simulator read the iOS one.
function(hermes_intl_deployment_var triple out)
  set(v "")
  if(triple MATCHES "-apple-darwin$")
    set(v MACOSX_DEPLOYMENT_TARGET)
  elseif(triple MATCHES "-apple-ios")
    set(v IPHONEOS_DEPLOYMENT_TARGET)
  elseif(triple MATCHES "-apple-tvos")
    set(v TVOS_DEPLOYMENT_TARGET)
  elseif(triple MATCHES "-apple-visionos")
    set(v XROS_DEPLOYMENT_TARGET)
  endif()
  set(${out} "${v}" PARENT_SCOPE)
endfunction()

# Map the build, described by the variables in scope, to the Rust target
# triples cargo builds the core for:
# - \p out_triples: the list of triples. There are several only for an
#   Apple build of several architectures; intl/CMakeLists.txt combines
#   their archives with lipo. For a host build it is HERMES_INTL_CARGO_HOST,
#   which is empty when cargo was not found.
# - \p out_env_var: the deployment-target variable of the triples (see
#   hermes_intl_deployment_var), or "".
# - \p out_reason: why the plugin cannot build here, a clause that can stand
#   as a sentence, or "" if it can. The triples are empty when it is set.
# The rules, in order:
# - Emscripten: never built (the loader loads no libraries there), whatever
#   HERMES_INTL_RUST_TARGET says;
# - HERMES_INTL_RUST_TARGET (a list), if set, wins over every rule below;
# - Android: from ANDROID_ABI (or CMAKE_ANDROID_ARCH_ABI);
# - Apple target platforms (HERMES_APPLE_TARGET_PLATFORM): one triple per
#   CMAKE_OSX_ARCHITECTURES entry, or for the host's architecture if it is
#   empty;
# - Windows with the MSVC ABI (MSVC, or a compiler that simulates it): from
#   the first of CMAKE_CXX_COMPILER_ARCHITECTURE_ID, CMAKE_VS_PLATFORM_NAME
#   and CMAKE_SYSTEM_PROCESSOR that is set; other Windows builds (MinGW)
#   get a reason;
# - macOS: the macosx row per CMAKE_OSX_ARCHITECTURES entry, or the host
#   triple (HERMES_INTL_CARGO_HOST) if it is empty;
# - any other native build: the host triple;
# - any other cross build: a reason naming HERMES_INTL_RUST_TARGET.
function(hermes_intl_rust_targets out_triples out_env_var out_reason)
  set(triples "")
  set(reason "")
  set(fix "set HERMES_INTL_RUST_TARGET to the Rust target triple")
  if(EMSCRIPTEN)
    string(CONCAT reason
      "the plugin is not built for Emscripten; embedders can register a "
      "plugin with registerIntlPlugin")
  elseif(HERMES_INTL_RUST_TARGET)
    set(triples ${HERMES_INTL_RUST_TARGET})
  elseif(ANDROID OR HERMES_IS_ANDROID OR ANDROID_ABI OR CMAKE_ANDROID_ARCH_ABI)
    set(abi "${ANDROID_ABI}")
    if(NOT abi)
      set(abi "${CMAKE_ANDROID_ARCH_ABI}")
    endif()
    if(abi STREQUAL "arm64-v8a")
      set(triples aarch64-linux-android)
    elseif(abi STREQUAL "armeabi-v7a")
      set(triples armv7-linux-androideabi)
    elseif(abi STREQUAL "x86")
      set(triples i686-linux-android)
    elseif(abi STREQUAL "x86_64")
      set(triples x86_64-linux-android)
    else()
      set(reason "Android ABI '${abi}' has no Rust target mapping; ${fix}")
    endif()
  elseif(HERMES_APPLE_TARGET_PLATFORM OR
         (APPLE AND (NOT CMAKE_SYSTEM_NAME OR
                     CMAKE_SYSTEM_NAME STREQUAL "Darwin")))
    if(HERMES_APPLE_TARGET_PLATFORM)
      set(platform ${HERMES_APPLE_TARGET_PLATFORM})
      set(what "HERMES_APPLE_TARGET_PLATFORM=${platform}")
    else()
      set(platform macosx)
      set(what "macOS")
    endif()
    set(archs "${CMAKE_OSX_ARCHITECTURES}")
    if(NOT archs AND NOT HERMES_INTL_CARGO_HOST)
      # Cargo was not found: no triples, and intl/CMakeLists.txt says so.
    elseif(NOT archs AND NOT HERMES_APPLE_TARGET_PLATFORM)
      # A plain macOS build for the host.
      set(triples ${HERMES_INTL_CARGO_HOST})
    else()
      if(NOT archs)
        if(HERMES_INTL_CARGO_HOST MATCHES "^aarch64-")
          set(archs arm64)
        elseif(HERMES_INTL_CARGO_HOST MATCHES "^x86_64-")
          set(archs x86_64)
        else()
          set(archs "'${HERMES_INTL_CARGO_HOST}'")
        endif()
      endif()
      foreach(arch IN LISTS archs)
        _hermes_intl_apple_triple("${platform}" "${arch}" t)
        if(t STREQUAL "SKIP")
          # Built for the other architectures only.
        elseif(t)
          list(APPEND triples ${t})
        else()
          set(reason
            "${what} has no Rust target for architecture ${arch}; ${fix}")
          break()
        endif()
      endforeach()
      if(NOT reason AND NOT triples)
        set(reason "${what} has no Rust target for ${archs}; ${fix}")
      endif()
    endif()
  elseif(WIN32 AND NOT (MSVC OR CMAKE_CXX_SIMULATE_ID STREQUAL "MSVC"))
    set(reason
      "Windows builds without the MSVC ABI have no Rust target mapping; ${fix}")
  elseif(WIN32)
    set(arch "${CMAKE_CXX_COMPILER_ARCHITECTURE_ID}")
    if(NOT arch)
      set(arch "${CMAKE_VS_PLATFORM_NAME}")
    endif()
    if(NOT arch)
      set(arch "${CMAKE_SYSTEM_PROCESSOR}")
    endif()
    string(TOLOWER "${arch}" larch)
    if(larch MATCHES "^(x64|amd64|x86_64)$")
      set(triples x86_64-pc-windows-msvc)
    elseif(larch MATCHES "^(arm64|aarch64)$")
      set(triples aarch64-pc-windows-msvc)
    else()
      set(reason
        "Windows architecture '${arch}' has no Rust target mapping; ${fix}")
    endif()
  elseif(NOT CMAKE_CROSSCOMPILING)
    set(triples ${HERMES_INTL_CARGO_HOST})
  else()
    string(CONCAT reason
      "a cross build for ${CMAKE_SYSTEM_NAME} ${CMAKE_SYSTEM_PROCESSOR} has "
      "no Rust target mapping; ${fix}")
  endif()
  if(reason)
    set(triples "")
  endif()
  set(env_var "")
  if(triples)
    list(GET triples 0 first)
    hermes_intl_deployment_var(${first} env_var)
  endif()
  set(${out_triples} "${triples}" PARENT_SCOPE)
  set(${out_env_var} "${env_var}" PARENT_SCOPE)
  set(${out_reason} "${reason}" PARENT_SCOPE)
endfunction()

# Set \p out to why this build's own hermesc cannot run on the build machine,
# a phrase such as "a cross build", or to "" if it can. The plugin's
# JavaScript is compiled at build time, so such a build needs the host's
# hermesc through IMPORT_HOST_COMPILERS. That is the case for a cross build,
# for every Apple target platform other than macosx (Catalyst included),
# and for a macOS build whose CMAKE_OSX_ARCHITECTURES leaves out the build
# machine's architecture (CMAKE_HOST_SYSTEM_PROCESSOR).
function(hermes_intl_foreign_hermesc out)
  set(what "")
  set(host_arch "${CMAKE_HOST_SYSTEM_PROCESSOR}")
  if(host_arch STREQUAL "aarch64")
    set(host_arch arm64)
  endif()
  if(CMAKE_CROSSCOMPILING)
    set(what "a cross build")
  elseif(HERMES_APPLE_TARGET_PLATFORM AND
         NOT HERMES_APPLE_TARGET_PLATFORM STREQUAL "macosx")
    set(what "HERMES_APPLE_TARGET_PLATFORM=${HERMES_APPLE_TARGET_PLATFORM}")
  elseif((APPLE OR HERMES_APPLE_TARGET_PLATFORM) AND CMAKE_OSX_ARCHITECTURES
         AND NOT host_arch IN_LIST CMAKE_OSX_ARCHITECTURES)
    string(CONCAT what
      "a build for CMAKE_OSX_ARCHITECTURES=${CMAKE_OSX_ARCHITECTURES} on "
      "${CMAKE_HOST_SYSTEM_PROCESSOR}")
  endif()
  set(${out} "${what}" PARENT_SCOPE)
endfunction()

# Set \p out_platform and \p out_arch to the Apple platform (a
# HERMES_APPLE_TARGET_PLATFORM value) and the architecture (the triple's
# first component) of Apple triple \p triple. The platform is the triple's
# OS part for a triple that is not one of Hermes's slices.
function(hermes_intl_apple_slice triple out_platform out_arch)
  string(REGEX REPLACE "-.*" "" arch "${triple}")
  string(REGEX REPLACE "^[^-]*-" "" os "${triple}")
  if(triple MATCHES "-apple-darwin$")
    set(platform macosx)
  elseif(triple MATCHES "-apple-ios-macabi$")
    set(platform catalyst)
  elseif(triple MATCHES "-apple-ios-sim$" OR triple STREQUAL "x86_64-apple-ios")
    set(platform iphonesimulator)
  elseif(triple MATCHES "-apple-ios$")
    set(platform iphoneos)
  elseif(triple MATCHES "-apple-tvos-sim$" OR
         triple STREQUAL "x86_64-apple-tvos")
    set(platform appletvsimulator)
  elseif(triple MATCHES "-apple-tvos$")
    set(platform appletvos)
  elseif(triple MATCHES "-apple-visionos-sim$")
    set(platform xrsimulator)
  elseif(triple MATCHES "-apple-visionos$")
    set(platform xros)
  else()
    set(platform "${os}")
  endif()
  set(${out_platform} "${platform}" PARENT_SCOPE)
  set(${out_arch} "${arch}" PARENT_SCOPE)
endfunction()

# Set \p out to the architectures (OSX_ARCHITECTURES) the plugin of an Apple
# build of CMAKE_OSX_ARCHITECTURES \p osx_archs is built for, when they
# differ from \p osx_archs, or to "" when they do not. They are those of its
# Rust core's triples \p triples (arm64, x86_64), which in the tvOS and
# visionOS simulator builds leave out x86_64 (see _hermes_intl_apple_triple).
# A build with no CMAKE_OSX_ARCHITECTURES, or with a triple that is not
# Apple's, gives "".
function(hermes_intl_plugin_osx_architectures triples osx_archs out)
  set(archs "")
  foreach(t IN LISTS triples)
    if(NOT t MATCHES "-apple-")
      set(archs "")
      break()
    endif()
    hermes_intl_apple_slice(${t} platform arch)
    if(arch STREQUAL "aarch64")
      set(arch arm64)
    endif()
    list(APPEND archs ${arch})
  endforeach()
  if(NOT osx_archs OR "${archs}" STREQUAL "${osx_archs}")
    set(archs "")
  endif()
  set(${out} "${archs}" PARENT_SCOPE)
endfunction()

# Set \p out to the LC_RPATH that lets hermesintl.framework of Apple platform
# \p platform (a HERMES_APPLE_TARGET_PLATFORM value; empty for a plain macOS
# framework build) find hermesvm.framework when both are in one directory,
# such as an app's Frameworks/: @loader_path/.. from the shallow bundle of
# the embedded SDKs (hermesintl.framework/hermesintl), @loader_path/../../..
# from the deep bundle of macOS and Mac Catalyst
# (hermesintl.framework/Versions/<v>/hermesintl).
function(hermes_intl_framework_rpath platform out)
  if(NOT platform OR platform STREQUAL "macosx" OR platform STREQUAL "catalyst")
    set(${out} "@loader_path/../../.." PARENT_SCOPE)
  else()
    set(${out} "@loader_path/.." PARENT_SCOPE)
  endif()
endfunction()

# Set \p out to why the list \p triples cannot be built together, or to ""
# if it can. One triple always can. Several are combined with lipo into one
# archive, so they must be Apple triples of one platform (one deployment
# variable) and of different architectures.
function(hermes_intl_check_rust_triples triples out)
  set(error "")
  list(LENGTH triples count)
  if(count GREATER 1)
    set(first_platform "")
    set(archs "")
    foreach(t IN LISTS triples)
      hermes_intl_apple_slice(${t} platform arch)
      if(NOT t MATCHES "-apple-")
        set(error "only Apple targets can be built together, not ${t}")
      elseif(first_platform AND NOT platform STREQUAL first_platform)
        string(CONCAT error
          "${t} is for ${platform}, not ${first_platform}; one build "
          "combines the architectures of one Apple platform")
      elseif(arch IN_LIST archs)
        set(error "two of the triples are for ${arch}")
      endif()
      if(error)
        break()
      endif()
      if(NOT first_platform)
        set(first_platform ${platform})
      endif()
      list(APPEND archs ${arch})
    endforeach()
  endif()
  set(${out} "${error}" PARENT_SCOPE)
endfunction()

# Set the HERMES_BUILD_INTL cache variable from \p default, the value the
# build would take by default, and print why it is OFF by default (\p
# off_why) when it is. HERMES_INTL_DEFAULTED_TO records the default that
# a value was taken from:
# - on the first configure, unless the value came from the command line;
# - a value given on the command line (-DHERMES_BUILD_INTL=...) is the
#   user's, and clears the record for good;
# - while the cached value still equals the recorded default, it follows
#   a new default, such as when an update supports a build it refused.
# Any other value is the user's and stays.
macro(hermes_intl_build_option default off_why)
  if(DEFINED CACHE{HERMES_BUILD_INTL})
    get_property(_hermes_intl_help CACHE HERMES_BUILD_INTL PROPERTY HELPSTRING)
    if(_hermes_intl_help STREQUAL
       "No help, variable specified on the command line.")
      unset(HERMES_INTL_DEFAULTED_TO CACHE)
    elseif(DEFINED CACHE{HERMES_INTL_DEFAULTED_TO} AND
           "$CACHE{HERMES_BUILD_INTL}" STREQUAL
             "$CACHE{HERMES_INTL_DEFAULTED_TO}" AND
           NOT "${default}" STREQUAL "$CACHE{HERMES_INTL_DEFAULTED_TO}")
      message(STATUS "HERMES_BUILD_INTL follows its new default, ${default}")
      set(HERMES_BUILD_INTL ${default} CACHE BOOL
        "Build the ICU4X-based Intl plugin (libhermesintl); requires Rust"
        FORCE)
      set(HERMES_INTL_DEFAULTED_TO ${default} CACHE INTERNAL
        "The value HERMES_BUILD_INTL took by default")
    endif()
  else()
    set(HERMES_INTL_DEFAULTED_TO ${default} CACHE INTERNAL
      "The value HERMES_BUILD_INTL took by default")
  endif()
  set(HERMES_BUILD_INTL ${default} CACHE BOOL
    "Build the ICU4X-based Intl plugin (libhermesintl); requires Rust")
  if(NOT HERMES_BUILD_INTL AND NOT ${default} AND
     DEFINED CACHE{HERMES_INTL_DEFAULTED_TO} AND
     "$CACHE{HERMES_BUILD_INTL}" STREQUAL "$CACHE{HERMES_INTL_DEFAULTED_TO}")
    message(STATUS "Intl plugin off by default: ${off_why}")
  endif()
endmacro()

# Set \p out to the system libraries the Rust standard library needs on
# \p triple, from `cargo rustc --target <triple> -- --print
# native-static-libs`, leaving out what every C++ link of that target adds
# anyway (libc, libSystem, libunwind, the MSVC CRT). Linux names the
# Threads::Threads target, which needs find_package(Threads). Apple, and
# targets not listed, need none.
function(hermes_intl_rust_native_libs triple out)
  set(libs "")
  if(triple MATCHES "-android")
    set(libs dl log m)
  elseif(triple MATCHES "-linux-")
    set(libs Threads::Threads dl m)
  elseif(triple MATCHES "-windows-msvc$")
    set(libs kernel32 ntdll userenv ws2_32 dbghelp)
  endif()
  set(${out} "${libs}" PARENT_SCOPE)
endfunction()

# Set \p out to the cargo profile directory of build configuration
# \p config: debug for Debug (cargo's dev profile), release for every other
# configuration (cargo --release). intl/CMakeLists.txt writes the same rule
# as generator expressions.
function(hermes_intl_cargo_profile_dir config out)
  string(TOUPPER "${config}" uconfig)
  if(uconfig STREQUAL "DEBUG")
    set(${out} debug PARENT_SCOPE)
  else()
    set(${out} release PARENT_SCOPE)
  endif()
endfunction()

# Set \p out to what makes an MSVC build use a static C runtime (/MT, /MTd)
# in some configuration, or to "" if every configuration uses the C runtime
# DLL. \p runtime is CMAKE_MSVC_RUNTIME_LIBRARY (empty means CMake's
# default, the DLL); the remaining arguments name the flag variables to
# scan for /MT and /MTd. A generator expression in \p runtime cannot be
# evaluated here, so it is accepted only in the form CMake's default uses:
# with the generator expressions removed the value is MultiThreadedDLL or
# MultiThreadedDebugDLL, and it ends with DLL, so no configuration can drop
# the DLL suffix.
function(hermes_intl_static_crt runtime out)
  set(result "")
  if(NOT runtime STREQUAL "")
    set(plain "${runtime}")
    set(previous "")
    while(NOT plain STREQUAL previous)
      set(previous "${plain}")
      string(REGEX REPLACE "\\$<[^$<>]*>" "" plain "${plain}")
    endwhile()
    if(NOT plain MATCHES "^MultiThreaded(Debug)?DLL$" OR
       NOT runtime MATCHES "DLL$")
      set(result "CMAKE_MSVC_RUNTIME_LIBRARY=${runtime}")
    endif()
  endif()
  foreach(var IN LISTS ARGN)
    if(NOT result AND " ${${var}} " MATCHES "[ \t]([/-]MTd?)[ \t]")
      set(result "${CMAKE_MATCH_1} in ${var}")
    endif()
  endforeach()
  set(${out} "${result}" PARENT_SCOPE)
endfunction()

# Set where the Intl plugin target \p plugin is built, so that the loader's
# default resolution finds it; \p vm is the hermesvm target and \p tools_dir
# is HERMES_TOOLS_OUTPUT_DIR. intl/test/lit/plugin-output-dir.js runs it
# under single- and multi-configuration generators.
# - When \p vm is an Apple framework, hermesintl.framework goes next to
#   hermesvm.framework, in every configuration, where the loader's sibling
#   framework candidate and the plugin's LC_RPATH expect it: \p plugin gets
#   the LIBRARY_OUTPUT_DIRECTORY and LIBRARY_OUTPUT_DIRECTORY_<CONFIG>
#   settings of \p vm, or the directory \p vm is defined in, CMake's default
#   for it, and a multi-configuration generator then appends the same
#   configuration directory to both.
# - Otherwise a single-configuration build puts it in lib/, next to
#   libhermesvm and in the tools' ../lib. A multi-configuration build puts
#   the tools in bin/<Config> and libhermesvm in lib/<Config>, so the plugin
#   goes in bin/<Config> (as add_hermes_tool places the tools), and a copy
#   goes next to a shared libhermesvm for the hosts that load the VM from
#   it. Windows does the same in every build: the loader looks next to the
#   Hermes module, and a DLL is a runtime artifact.
function(hermes_intl_plugin_output_dir plugin vm tools_dir)
  get_target_property(vm_framework ${vm} FRAMEWORK)
  if(vm_framework)
    get_target_property(vm_dir ${vm} LIBRARY_OUTPUT_DIRECTORY)
    if(NOT vm_dir)
      get_target_property(vm_dir ${vm} BINARY_DIR)
    endif()
    set_target_properties(${plugin} PROPERTIES
      LIBRARY_OUTPUT_DIRECTORY "${vm_dir}")
    foreach(config IN LISTS CMAKE_CONFIGURATION_TYPES CMAKE_BUILD_TYPE)
      string(TOUPPER "${config}" uconfig)
      get_target_property(vm_config_dir ${vm}
        LIBRARY_OUTPUT_DIRECTORY_${uconfig})
      if(vm_config_dir)
        set_target_properties(${plugin} PROPERTIES
          LIBRARY_OUTPUT_DIRECTORY_${uconfig} "${vm_config_dir}")
      endif()
    endforeach()
  elseif(CMAKE_CONFIGURATION_TYPES OR WIN32)
    # CMake adds the per-configuration directory itself, as add_hermes_tool
    # relies on.
    if(NOT "${CMAKE_CFG_INTDIR}" STREQUAL ".")
      string(REPLACE "${CMAKE_CFG_INTDIR}" "" tools_dir "${tools_dir}")
    endif()
    set_target_properties(${plugin} PROPERTIES
      LIBRARY_OUTPUT_DIRECTORY "${tools_dir}"
      RUNTIME_OUTPUT_DIRECTORY "${tools_dir}")
    get_target_property(vm_type ${vm} TYPE)
    if(vm_type STREQUAL "SHARED_LIBRARY")
      add_custom_command(TARGET ${plugin} POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E make_directory $<TARGET_FILE_DIR:${vm}>
        COMMAND ${CMAKE_COMMAND} -E copy_if_different
          $<TARGET_FILE:${plugin}>
          $<TARGET_FILE_DIR:${vm}>/$<TARGET_FILE_NAME:${plugin}>
        COMMENT "Copying the Intl plugin next to ${vm}"
        VERBATIM)
    endif()
  else()
    set_target_properties(${plugin} PROPERTIES
      LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib)
  endif()
endfunction()
