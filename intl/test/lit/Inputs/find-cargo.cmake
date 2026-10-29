# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Run by find-cargo.sh: hermes_intl_find_cargo in the environment this runs
# in, printing the cargo it finds, the host triple that cargo names, and the
# places it searched, with ROOT (the test's directory) shown as <t>.

cmake_minimum_required(VERSION 3.21)
set(PROJECT_SOURCE_DIR ${CMAKE_CURRENT_LIST_DIR}/../../../..)
include(${PROJECT_SOURCE_DIR}/cmake/modules/HermesIntl.cmake)

# Search PATH, CARGO_HOME and HOME only, not the prefixes CMake adds.
set(CMAKE_FIND_USE_CMAKE_SYSTEM_PATH FALSE)
set(CMAKE_FIND_USE_CMAKE_PATH FALSE)
set(CMAKE_FIND_USE_CMAKE_ENVIRONMENT_PATH FALSE)

hermes_intl_find_cargo()
set(out "cargo: ${HERMES_INTL_CARGO}, host: [${HERMES_INTL_CARGO_HOST}]")
string(APPEND out "\nsearched: ${HERMES_INTL_CARGO_SEARCHED}")
string(REPLACE "${ROOT}" "<t>" out "${out}")
execute_process(COMMAND ${CMAKE_COMMAND} -E echo "${out}")
