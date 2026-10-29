# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Configured by plugin-output-dir.sh: a hermesvm in lib/, as in the Hermes
# tree, and a plugin placed by hermes_intl_plugin_output_dir. Each
# configuration's directories are written to dirs-<Config>.txt. FAKE_FRAMEWORK
# sets FRAMEWORK on hermesvm, which on a platform other than Apple changes
# nothing else; VM_DIR and VM_RELEASE_DIR set its LIBRARY_OUTPUT_DIRECTORY
# and LIBRARY_OUTPUT_DIRECTORY_RELEASE.
cmake_minimum_required(VERSION 3.21)
project(PluginOutputDir C)
set(HERMES_ROOT ${CMAKE_CURRENT_SOURCE_DIR}/../../../../..)
include(${HERMES_ROOT}/cmake/modules/HermesIntl.cmake)

set(HERMES_TOOLS_OUTPUT_DIR "${CMAKE_BINARY_DIR}/bin/${CMAKE_CFG_INTDIR}")
add_subdirectory(lib)
add_library(hermesintl SHARED lib/empty.c)
hermes_intl_plugin_output_dir(hermesintl hermesvm "${HERMES_TOOLS_OUTPUT_DIR}")
file(GENERATE OUTPUT ${CMAKE_BINARY_DIR}/dirs-$<CONFIG>.txt CONTENT
  "vm: $<TARGET_FILE_DIR:hermesvm>\nplugin: $<TARGET_FILE_DIR:hermesintl>\n")
