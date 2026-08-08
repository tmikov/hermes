# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# tools/hermes-parser is only built by Emscripten, so these tests are the only
# thing that compiles the serializer with a host compiler.
set(HermesParserJSTestSources
  SourcePositionMapTest.cpp
  SerializerPositionTest.cpp
  ../../tools/hermes-parser/HermesParserJSSerializer.cpp
  )

add_hermes_unittest(HermesParserJSTests
  ${HermesParserJSTestSources}
  )

target_include_directories(HermesParserJSTests PRIVATE
  ${CMAKE_SOURCE_DIR}/tools/hermes-parser
  )

target_link_libraries(HermesParserJSTests
  hermesAST
  hermesParser
  hermesSema
  LLVHSupport
  )
