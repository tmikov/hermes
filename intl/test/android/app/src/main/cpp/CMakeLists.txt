# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# The test app's JNI library: it creates a Hermes runtime through
# libhermesvm.so from the AAR (prefab), with JSI from the libjsi.so the app
# packages (HERMES_JNI_LIBS) and its headers from the checkout (HERMES_SRC).
cmake_minimum_required(VERSION 3.22)
project(hermesintlapp CXX)
set(CMAKE_CXX_STANDARD 17)

find_package(hermes-engine REQUIRED CONFIG)
find_package(fbjni REQUIRED CONFIG)

add_library(jsi SHARED IMPORTED)
set_target_properties(jsi PROPERTIES
  IMPORTED_LOCATION ${HERMES_JNI_LIBS}/${ANDROID_ABI}/libjsi.so
  INTERFACE_INCLUDE_DIRECTORIES ${HERMES_SRC}/API/jsi)

add_library(hermesintlapp SHARED IntlApp.cpp)
target_link_libraries(hermesintlapp hermes-engine::hermesvm jsi fbjni::fbjni
  log)
