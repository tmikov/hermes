/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "HermesParserJSSerializer.h"

#include <algorithm>
#include <cstdint>
#include <cstring>

#include "gtest/gtest.h"
#include "hermes/AST/Context.h"
#include "hermes/Parser/JSParser.h"
#include "llvh/Support/MemoryBuffer.h"

using namespace hermes;

namespace {

/// Parse \p source and serialize it, returning the populated result.
///
/// Mirrors the production entry point
/// (tools/hermes-parser/hermes-parser-wasm.cpp): the parser must be
/// heap-allocated and moved into \c ParseResult::parser_ before \c
/// serialize() runs, since comment/token serialization reads it back out of
/// the result.
std::unique_ptr<ParseResult> parseAndSerialize(const char *source) {
  auto result = std::make_unique<ParseResult>();
  auto context = std::make_shared<Context>();
  auto &sm = context->getSourceErrorManager();

  auto fileBuf = llvh::MemoryBuffer::getMemBuffer(llvh::StringRef{source});
  int fileBufId = sm.addNewSourceBuffer(std::move(fileBuf));

  auto jsParser = std::make_unique<parser::JSParser>(
      *context, fileBufId, parser::FullParse);
  auto parsed = jsParser->parse();
  EXPECT_TRUE(parsed.hasValue());

  result->context_ = context;
  result->parser_ = std::move(jsParser);
  serialize(
      llvh::cast<ESTree::ProgramNode>(parsed.getValue()), &sm, *result, false);

  return result;
}

TEST(SerializerTest, InternsRepeatedIdentifiersOnce) {
  auto result = parseAndSerialize("var foo; foo; foo; foo;");

  uint32_t fooCount = 0;
  for (uint32_t i = 0; i < result->stringTable_.count(); ++i) {
    uint32_t start = result->stringTable_.offsets()[i];
    uint32_t end = result->stringTable_.offsets()[i + 1];
    if (result->stringTable_.data().substr(start, end - start) == "foo") {
      ++fooCount;
    }
  }

  EXPECT_EQ(1u, fooCount) << "identifier must be interned exactly once";
}

TEST(SerializerTest, PadsNumbersToEvenIndex) {
  auto result = parseAndSerialize("1.5;");

  // Locate the IEEE-754 halves of 1.5 and assert the pair starts on an even
  // index, which is what lets a Float64Array view over the region address it.
  double value = 1.5;
  uint64_t bits;
  memcpy(&bits, &value, sizeof(bits));
  const uint32_t lo = (uint32_t)bits;
  const uint32_t hi = (uint32_t)(bits >> 32);

  const auto &buf = result->programBuffer_;
  bool found = false;
  for (size_t i = 0; i + 1 < buf.size(); ++i) {
    if (buf[i] == lo && buf[i + 1] == hi) {
      EXPECT_EQ(0u, i % 2) << "double must start on an even index";
      found = true;
    }
  }
  EXPECT_TRUE(found) << "1.5 must appear in the program buffer";
}

TEST(SerializerTest, StringIdsAreBiasedByOne) {
  auto result = parseAndSerialize("var foo;");

  // Find the table id assigned to "foo".
  uint32_t fooId = UINT32_MAX;
  for (uint32_t i = 0; i < result->stringTable_.count(); ++i) {
    uint32_t start = result->stringTable_.offsets()[i];
    uint32_t end = result->stringTable_.offsets()[i + 1];
    if (result->stringTable_.data().substr(start, end - start) == "foo") {
      fooId = i;
    }
  }
  ASSERT_NE(UINT32_MAX, fooId) << "\"foo\" must be interned";

  // The program buffer must reference it as id + 1, since 0 means null.
  const auto &buf = result->programBuffer_;
  EXPECT_NE(std::find(buf.begin(), buf.end(), fooId + 1), buf.end())
      << "program buffer must reference the string as id + 1";
}

} // namespace
