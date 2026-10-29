# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# The Apple smoke test (smoke.cpp): it links hermesvm.framework, as an app
# does, and is run on the simulators and on macOS (the Phase 6 Mac commands).
# Framework builds have no test suite, so it is built only when asked for:
# cmake --build <build> --target hermes-intl-apple-smoke.
set(HERMES_ENABLE_EH ON)
set(HERMES_ENABLE_RTTI ON)

add_hermes_executable(hermes-intl-apple-smoke smoke.cpp)
# JSI's headers only: its code comes from hermesvm.framework, which exports
# it (the framework build's JSI is static, so linking it would add a copy).
target_include_directories(hermes-intl-apple-smoke PRIVATE ${HERMES_JSI_DIR})
target_link_libraries(hermes-intl-apple-smoke PRIVATE hermesvm)
# The frameworks are looked up in ../lib (the build tree), in Frameworks/
# next to the executable (an iOS-style app) and in ../Frameworks (a
# macOS-style Contents/MacOS), never through CMake's absolute build-tree
# path, so that a copied layout uses its own copies.
set_target_properties(hermes-intl-apple-smoke PROPERTIES
  EXCLUDE_FROM_ALL TRUE
  RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin
  BUILD_WITH_INSTALL_RPATH TRUE
  INSTALL_RPATH
    "@executable_path/../lib;@executable_path/Frameworks;@executable_path/../Frameworks")
add_dependencies(hermes-intl-apple-smoke hermesintl)
