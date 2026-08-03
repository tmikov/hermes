# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

set(HermesParserNativeTestSources
  StringTableTest.cpp
  KindHashTest.cpp
  ContainerWriterTest.cpp
  )

add_hermes_unittest(HermesParserNativeTests
  ${HermesParserNativeTestSources}
  )

target_include_directories(HermesParserNativeTests PRIVATE
  ${CMAKE_SOURCE_DIR}/tools/hermes-parser-native
  )

target_link_libraries(HermesParserNativeTests
  LLVHSupport
  )
