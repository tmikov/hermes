/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "HermesParserJSSerializer.h"

#include <cstdint>
#include <map>
#include <string>
#include <vector>

#include "gtest/gtest.h"
#include "hermes/AST/Context.h"
#include "hermes/Parser/JSParser.h"
#include "llvh/Support/MemoryBuffer.h"

using namespace hermes;

namespace {

/// Parse \p source and serialize it, returning the populated result.
///
/// Mirrors the production entry point (tools/hermes-parser-wasm.cpp): the
/// parser must be heap-allocated and moved into \c ParseResult::parser_
/// before \c serialize() runs, since comment and token serialization read it
/// back out of the result.
std::unique_ptr<ParseResult> parseAndSerialize(
    llvh::StringRef source,
    bool tokens) {
  auto result = std::make_unique<ParseResult>();
  auto context = std::make_shared<Context>();
  auto &sm = context->getSourceErrorManager();

  auto fileBuf = llvh::MemoryBuffer::getMemBuffer(source);
  int fileBufId = sm.addNewSourceBuffer(std::move(fileBuf));

  context->setParseJSX(true);
  context->setUseCJSModules(true);

  auto jsParser = std::make_unique<parser::JSParser>(
      *context, fileBufId, parser::FullParse);
  jsParser->setStoreComments(true);
  jsParser->setStoreTokens(tokens);

  auto parsed = jsParser->parse();
  EXPECT_TRUE(parsed.hasValue()) << "source must parse";
  if (!parsed.hasValue()) {
    return nullptr;
  }

  result->context_ = context;
  result->parser_ = std::move(jsParser);
  serialize(parsed.getValue(), &sm, *result, tokens);

  return result;
}

/// The state the position walk carries at one character boundary.
struct Triple {
  uint32_t line;
  uint32_t column;
  uint32_t offset;
};

/// The resolution of every character boundary of a source, computed the way
/// the serializer used to compute it before \c SourcePositionMap existed.
struct Reference {
  /// Resolution of each character boundary, keyed by byte offset. Offsets in
  /// the middle of a multi-byte character are absent.
  std::map<uint32_t, Triple> byByteOffset;
  /// Byte offset of each character boundary, keyed by its UTF-16 offset. The
  /// inverse of the \c offset field above, which exists because the position
  /// buffer records UTF-16 offsets and the checks below need to get back to
  /// byte offsets to compare against raw pointer arithmetic.
  std::map<uint32_t, uint32_t> byUtf16Offset;
};

/// Walk \p source one character at a time, exactly the way the serializer's
/// old single-pass scan did, and record the state at every boundary.
///
/// This is deliberately a transcription of the loop that \c SourcePositionMap
/// replaced rather than a call into the map, so that it is an independent
/// account of what the answers should be.
Reference buildReference(llvh::StringRef source) {
  Reference ref;
  uint32_t line = 1;
  uint32_t col = 0;
  uint32_t offset = 0;
  uint32_t i = 0;
  const uint32_t size = (uint32_t)source.size();
  while (true) {
    ref.byByteOffset[i] = Triple{line, col, offset};
    ref.byUtf16Offset[offset] = i;
    if (i >= size) {
      break;
    }

    const char ch = source[i];
    if (ch == '\n') {
      ++offset;
      ++line;
      col = 0;
      i += 1;
    } else if ((unsigned char)ch < 128) {
      ++offset;
      ++col;
      i += 1;
    } else if ((ch & 0xE0) == 0xC0) {
      ++offset;
      ++col;
      i += 2;
    } else if ((ch & 0xF0) == 0xE0) {
      ++offset;
      ++col;
      i += 3;
    } else {
      offset += 2;
      col += 2;
      i += 4;
    }
  }
  return ref;
}

/// One serialized token, as recovered from the program buffer.
struct TokenEntry {
  /// Loc id the token's source range was serialized under.
  uint32_t locId;
  /// Length of the token in bytes, computed by the serializer directly from
  /// the range's pointers and therefore independent of position resolution.
  uint32_t byteLength;
};

/// Recover the token region from the end of \p result 's program buffer.
///
/// Tokens are serialized last, as a count followed by four words per token
/// (type, loc id, pointer, byte length). The count is the number of stored
/// tokens less the EOF token, which the parser can be asked for directly, so
/// the region's extent is known rather than searched for; the layout is then
/// checked on the way out, since a mismatch would mean this walk is reading
/// something other than tokens.
std::vector<TokenEntry> recoverTokens(const ParseResult &result) {
  const std::vector<uint32_t> &buf = result.programBuffer_;
  const uint32_t nLocs = (uint32_t)(result.positionBuffer_.size() / 2);
  const size_t n = result.parser_->getStoredTokens().size() - 1;

  std::vector<TokenEntry> tokens;
  EXPECT_LE(1 + 4 * n, buf.size());
  if (1 + 4 * n > buf.size()) {
    return tokens;
  }

  const size_t base = buf.size() - 1 - 4 * n;
  EXPECT_EQ(n, buf[base]) << "token count word must precede the token region";
  for (size_t k = 0; k != n; ++k) {
    const size_t at = base + 1 + 4 * k;
    // Each token takes one loc id, and tokens are serialized last, so their
    // loc ids are the last ones handed out.
    EXPECT_EQ(nLocs - n + k, buf[at + 1]) << "token " << k << " loc id";
    tokens.push_back(TokenEntry{buf[at + 1], buf[at + 3]});
  }
  return tokens;
}

/// Serialize \p source and check every position it produces against
/// \c buildReference().
///
/// Three things are checked, and together they cover the whole path from a
/// source pointer to a serialized entry:
///
/// - every loc id in range has exactly one start and one end entry, so no
///   entry was dropped, duplicated or mislabelled, and the two are ordered;
/// - every entry's (line, column, offset) is what the old scan reports at
///   some character boundary, so no triple is internally inconsistent;
/// - each token's two endpoints, mapped back to byte offsets, are exactly the
///   token's byte length apart. That length is computed by the serializer
///   from the raw range pointers and never goes through position resolution,
///   so it pins down *which* boundary each endpoint resolved to - including
///   on multi-byte characters, where the byte length and the UTF-16 length
///   differ.
void expectPositionsMatchReference(llvh::StringRef source) {
  std::unique_ptr<ParseResult> result = parseAndSerialize(source, true);
  ASSERT_NE(nullptr, result);

  const Reference ref = buildReference(source);
  const std::vector<PositionResult> &positions = result->positionBuffer_;
  ASSERT_FALSE(positions.empty());
  ASSERT_EQ(0u, positions.size() % 2);

  const uint32_t nLocs = (uint32_t)(positions.size() / 2);
  std::vector<const PositionResult *> starts(nLocs, nullptr);
  std::vector<const PositionResult *> ends(nLocs, nullptr);

  for (const PositionResult &pos : positions) {
    ASSERT_LT(pos.locId, nLocs) << "loc id out of range";
    const PositionResult *&slot =
        pos.kind == 0 ? starts[pos.locId] : ends[pos.locId];
    ASSERT_EQ(nullptr, slot) << "duplicate entry for loc " << pos.locId;
    slot = &pos;

    auto byteIt = ref.byUtf16Offset.find(pos.offset);
    ASSERT_NE(ref.byUtf16Offset.end(), byteIt)
        << "offset " << pos.offset << " is not a character boundary";
    const Triple &want = ref.byByteOffset.at(byteIt->second);
    EXPECT_EQ(want.line, pos.line) << "line for loc " << pos.locId;
    EXPECT_EQ(want.column, pos.column) << "column for loc " << pos.locId;
  }

  for (uint32_t i = 0; i != nLocs; ++i) {
    ASSERT_NE(nullptr, starts[i]) << "loc " << i << " has no start";
    ASSERT_NE(nullptr, ends[i]) << "loc " << i << " has no end";
    EXPECT_LE(starts[i]->offset, ends[i]->offset) << "loc " << i << " is empty";
  }

  const std::vector<TokenEntry> tokens = recoverTokens(*result);
  EXPECT_FALSE(tokens.empty()) << "source must produce at least one token";
  for (const TokenEntry &token : tokens) {
    const uint32_t startByte =
        ref.byUtf16Offset.at(starts[token.locId]->offset);
    const uint32_t endByte = ref.byUtf16Offset.at(ends[token.locId]->offset);
    EXPECT_EQ(token.byteLength, endByte - startByte)
        << "token at loc " << token.locId
        << " does not span its serialized byte length";
  }
}

TEST(SerializerPositionTest, ResolvesPositionsInAsciiSource) {
  expectPositionsMatchReference(
      "// a comment\n"
      "var x = 1;\n"
      "function f(a, b) {\n"
      "  return a + b;\n"
      "}\n");
}

TEST(SerializerPositionTest, ResolvesPositionsWithNoTrailingNewline) {
  expectPositionsMatchReference("var x = 1;\nvar y = 2;");
}

TEST(SerializerPositionTest, ResolvesPositionsWithCRLF) {
  expectPositionsMatchReference("var x = 1;\r\nvar y = 2;\r\n");
}

TEST(SerializerPositionTest, ResolvesPositionsWithLoneCarriageReturn) {
  // A lone '\r' is a line terminator to the lexer but not to the position
  // walk, which only counts '\n'. The two disagreeing is the point.
  expectPositionsMatchReference("var x = 1;\rvar y = 2;\r");
}

TEST(SerializerPositionTest, ResolvesPositionsWithBOM) {
  expectPositionsMatchReference(
      "\xEF\xBB\xBF"
      "var x = 1;\n");
}

TEST(SerializerPositionTest, ResolvesPositionsWithTwoByteCharacters) {
  expectPositionsMatchReference(
      "var caf\xC3\xA9 = '\xC3\xA1\xC3\xA9\xC3\xAD';\n"
      "// \xC2\xA1"
      "comment!\n");
}

TEST(SerializerPositionTest, ResolvesPositionsWithThreeByteCharacters) {
  expectPositionsMatchReference(
      "var \xE4\xB8\x96\xE7\x95\x8C = '\xE4\xB8\x96\xE7\x95\x8C';\n"
      "\xE4\xB8\x96\xE7\x95\x8C;\n");
}

TEST(SerializerPositionTest, ResolvesPositionsWithAstralCharacters) {
  // U+1F600 and U+1F680 are four UTF-8 bytes and a surrogate pair in UTF-16,
  // so every position after them has a byte offset and a UTF-16 offset that
  // differ, and by different amounts.
  expectPositionsMatchReference(
      "var s = '\xF0\x9F\x98\x80';\n"
      "var t = \"a\xF0\x9F\x9A\x80\x62\";\n"
      "s + t;\n");
}

TEST(SerializerPositionTest, ResolvesPositionsWithAstralIdentifier) {
  // U+1D4D0 MATHEMATICAL BOLD SCRIPT CAPITAL A is a valid identifier start
  // and needs a surrogate pair.
  expectPositionsMatchReference(
      "var \xF0\x9D\x93\x90 = 1;\n"
      "\xF0\x9D\x93\x90 + 1;\n");
}

TEST(SerializerPositionTest, ResolvesPositionsInTemplateLiterals) {
  expectPositionsMatchReference(
      "var t = `a\xF0\x9F\x98\x80${ x }b\xE4\xB8\x96`;\n"
      "var u = `line1\nline2 \xC3\xA9`;\n");
}

TEST(SerializerPositionTest, ResolvesPositionsInJSX) {
  expectPositionsMatchReference(
      "var e = <div a=\"\xF0\x9F\x98\x80\">\n"
      "  text \xE4\xB8\x96\xE7\x95\x8C \xF0\x9F\x9A\x80\n"
      "  <span />\n"
      "</div>;\n");
}

TEST(SerializerPositionTest, ResolvesPositionsInDenselyNonAsciiSource) {
  std::string source;
  for (unsigned i = 0; i != 200; ++i) {
    source += "var \xE4\xB8\x96";
    source += std::to_string(i);
    source += " = '\xE4\xB8\x96\xE7\x95\x8C\xF0\x9F\x98\x80\xC3\xA9';\n";
  }
  expectPositionsMatchReference(source);
}

TEST(SerializerPositionTest, ResolvesPositionsInALongSingleLine) {
  // One long line whose only multi-byte character is near the front, which is
  // the shape a per-line rescan would resolve in time quadratic in the line
  // length.
  std::string source = "var s = '\xF0\x9F\x98\x80';";
  for (unsigned i = 0; i != 2000; ++i) {
    source += "var v" + std::to_string(i) + " = " + std::to_string(i) + ";";
  }
  source += "\n";
  expectPositionsMatchReference(source);
}

TEST(SerializerPositionTest, ResolvesPositionsAcrossManyLines) {
  std::string source;
  for (unsigned i = 0; i != 500; ++i) {
    source += "var v" + std::to_string(i) + " = " + std::to_string(i) + ";\n";
  }
  expectPositionsMatchReference(source);
}

TEST(SerializerPositionTest, ProgramSpansItsStatements) {
  // Loc 0 is the Program node. Its range runs from the first statement to the
  // end of the last one, so a leading comment and a trailing newline are both
  // outside it.
  const llvh::StringRef source =
      "// leading\nvar x = '\xF0\x9F\x98\x80';\nvar y = 2;\n";
  std::unique_ptr<ParseResult> result = parseAndSerialize(source, false);
  ASSERT_NE(nullptr, result);

  const PositionResult *start = nullptr;
  const PositionResult *end = nullptr;
  for (const PositionResult &pos : result->positionBuffer_) {
    if (pos.locId != 0) {
      continue;
    }
    (pos.kind == 0 ? start : end) = &pos;
  }
  ASSERT_NE(nullptr, start);
  ASSERT_NE(nullptr, end);

  // "// leading\n" is 11 bytes, all ASCII, so `var x` starts line 2 column 0
  // at UTF-16 offset 11.
  EXPECT_EQ(2u, start->line);
  EXPECT_EQ(0u, start->column);
  EXPECT_EQ(11u, start->offset);

  // The last statement ends after the ';' of "var y = 2;", which is line 3
  // column 10. The emoji on line 2 is four bytes but two UTF-16 code units,
  // so the offset is two less than the byte offset of that point.
  EXPECT_EQ(3u, end->line);
  EXPECT_EQ(10u, end->column);
  EXPECT_EQ((uint32_t)source.size() - 1 - 2, end->offset);
}

TEST(SerializerPositionTest, PositionsAreEmittedInLocOrder) {
  // The entries are written as the AST walk reaches them, which is loc order.
  // Nothing may depend on that - the consumer indexes them by loc id - but it
  // does mean the region's layout is deterministic, where sorting by address
  // left ties in whatever order the sort happened to produce.
  std::unique_ptr<ParseResult> result =
      parseAndSerialize("var x = 1; function f() { return x; }\n", false);
  ASSERT_NE(nullptr, result);

  const std::vector<PositionResult> &positions = result->positionBuffer_;
  ASSERT_EQ(0u, positions.size() % 2);
  for (size_t i = 0; i != positions.size(); ++i) {
    EXPECT_EQ((uint32_t)(i / 2), positions[i].locId);
    EXPECT_EQ((uint32_t)(i % 2), positions[i].kind);
  }
}

} // namespace
