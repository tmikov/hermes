/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

/// js-lexer-dump: Drive hermes::parser::JSLexer and print a deterministic
/// token dump suitable for byte-for-byte comparison against the Rust lexer.
///
/// DUMP FORMAT (the contract — implement exactly):
/// One line per token: `<start> <end> <nl> <KIND>[ <field> ...]\n`
/// - start, end = decimal byte offsets from the beginning of the source
///   buffer: tok.getStartLoc().getPointer() - lex.getBufferStart() and
///   end likewise.
/// - nl = "nl" if lex.isNewLineBeforeCurrentToken() else "--".
/// - KIND = the TokenKinds.def variant name of tok.getKind()
///   (e.g. none, identifier, rw_function, l_brace, starstar, numeric_literal,
///   eof). Generated via a switch over the .def so names match the Rust
///   TokenKind variants.
/// - Per-kind fields (none for punctuators/eof):
///   - identifier / private_identifier / any reserved word:
///       ` ident=Q(name)`
///     (identifier->getIdentifier(), private_identifier->getPrivateIdentifier,
///      resword->getResWordIdentifier(); respect the header's kind asserts)
///   - numeric_literal: ` bits=0xHHHHHHHHHHHHHHHH`
///     = llvh::DoubleToBits(tok.getNumericLiteral()), lowercase, zero-padded
///     to 16 hex digits.
///   - bigint_literal:
///       ` value=Q(getBigIntLiteral) raw=Q(getBigIntLiteralRawValue)`
///   - string_literal:
///       ` escapes=<0|1> value=Q(getStringLiteral)`
///     (flag from getStringLiteralContainsEscapes(); no raw field)
///   - regexp_literal:
///       ` body=Q(getRegExpLiteral()->getBody()) flags=Q(...->getFlags())`
///   - no_substitution_template / template_head / template_middle /
///     template_tail:
///       ` cooked=<Q(getTemplateValue)|null> raw=Q(getTemplateRawValue)`
///     (emit unquoted token `null` when getTemplateValue() is nullptr)
///   - jsx_text:
///       ` value=Q(getJSXTextValue) raw=Q(getJSXTextRaw)`
/// - Always emit a trailing eof line.
///
/// Q(s) quoting (byte-exact, operates on raw bytes of UniqueString->str(),
/// a llvh::StringRef; never re-decodes UTF-8):
///   Wrap in double quotes; inside:
///   - printable ASCII 0x20..0x7e literal, EXCEPT:
///       double-quote -> backslash-quote, backslash -> backslash-backslash
///   - newline (0x0a) -> backslash-n
///   - tab (0x09)     -> backslash-t
///   - carriage return (0x0d) -> backslash-r
///   - every other byte (incl. non-ASCII / ill-formed-UTF-8 / WTF-8 bytes)
///     as \xHH (lowercase, two hex digits)
/// This makes lone-surrogate bytes round-trip visibly.
///
/// Grammar context: --context=regexp (default) -> AllowRegExp
///                  --context=div             -> AllowDiv
/// JSX/Flow contexts are not supported by this tool.

#include "hermes/Parser/JSLexer.h"
#include "hermes/Support/SourceErrorManager.h"

#include "llvh/Support/MemoryBuffer.h"
#include "llvh/Support/raw_ostream.h"

#include <cstring>

using namespace hermes;
using namespace hermes::parser;

/// \return the variant name string for \p kind as it appears in TokenKinds.def.
static const char *tokenVariantName(TokenKind kind) {
  switch (kind) {
#define TOK(name, str)  \
  case TokenKind::name: \
    return #name;
#include "hermes/Parser/TokenKinds.def"
  }
  return "<unknown>";
}

/// Emit \p s quoted per the Q() spec into \p os.
static void quoteBytes(llvh::raw_ostream &os, llvh::StringRef s) {
  os << '"';
  for (unsigned char c : s) {
    if (c == '"') {
      os << "\\\"";
    } else if (c == '\\') {
      os << "\\\\";
    } else if (c == '\n') {
      os << "\\n";
    } else if (c == '\t') {
      os << "\\t";
    } else if (c == '\r') {
      os << "\\r";
    } else if (c >= 0x20 && c <= 0x7e) {
      os << (char)c;
    } else {
      // Non-printable or non-ASCII: emit \xHH.
      const char *hex = "0123456789abcdef";
      os << "\\x";
      os << hex[(c >> 4) & 0xf];
      os << hex[c & 0xf];
    }
  }
  os << '"';
}

/// Emit kind-specific fields for \p tok into \p os.
static void emitFields(
    llvh::raw_ostream &os,
    JSLexer &lex,
    const Token &tok) {
  (void)lex;
  switch (tok.getKind()) {
    case TokenKind::identifier:
      os << " ident=";
      quoteBytes(os, tok.getIdentifier()->str());
      break;

    case TokenKind::private_identifier:
      os << " ident=";
      quoteBytes(os, tok.getPrivateIdentifier()->str());
      break;

    default:
      // Reserved words: emit the identifier string.
      if (tok.isResWord()) {
        os << " ident=";
        quoteBytes(os, tok.getResWordIdentifier()->str());
      }
      // Punctuators and eof: no extra fields.
      break;
  }
}

static void usage(const char *argv0) {
  llvh::errs() << "Usage: " << argv0 << " <file|->\n"
               << "  Dump tokens from the JS lexer to stdout.\n"
               << "  Use - to read from stdin.\n";
}

int main(int argc, char **argv) {
  if (argc < 2) {
    usage(argv[0]);
    return 1;
  }

  const char *filePath = argv[argc - 1];

  // Read input.
  auto fileBufOrErr =
      llvh::MemoryBuffer::getFileOrSTDIN(llvh::StringRef(filePath));
  if (!fileBufOrErr) {
    llvh::errs() << argv[0] << ": error reading '" << filePath
                 << "': " << fileBufOrErr.getError().message() << "\n";
    return 1;
  }

  // Set up the lexer.
  JSLexer::Allocator alloc;
  SourceErrorManager sm;
  JSLexer lex(std::move(fileBufOrErr.get()), sm, alloc);
  const char *base = lex.getBufferStart();

  // Default grammar context — allow regexp literals.
  JSLexer::GrammarContext grammarContext = JSLexer::AllowRegExp;

  llvh::raw_ostream &os = llvh::outs();

  // Token loop: advance to load the first token, then continue.
  for (;;) {
    const Token *tok = lex.advance(grammarContext);
    size_t start = (size_t)(tok->getStartLoc().getPointer() - base);
    size_t end = (size_t)(tok->getEndLoc().getPointer() - base);
    const char *nl = lex.isNewLineBeforeCurrentToken() ? "nl" : "--";
    os << start << ' ' << end << ' ' << nl << ' '
       << tokenVariantName(tok->getKind());
    emitFields(os, lex, *tok);
    os << '\n';
    if (tok->getKind() == TokenKind::eof) {
      break;
    }
  }

  return 0;
}
