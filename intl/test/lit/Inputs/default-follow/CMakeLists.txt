# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Configured by default-follow.js: hermes_intl_build_option with the default
# TEST_DEFAULT, printing the resulting HERMES_BUILD_INTL and the recorded
# default.

cmake_minimum_required(VERSION 3.21)
project(DefaultFollow NONE)
include(${CMAKE_CURRENT_LIST_DIR}/../../../../../cmake/modules/HermesIntl.cmake)
hermes_intl_build_option(${TEST_DEFAULT} "the test says so")
message("HERMES_BUILD_INTL=$CACHE{HERMES_BUILD_INTL} "
  "recorded=[$CACHE{HERMES_INTL_DEFAULTED_TO}]")
