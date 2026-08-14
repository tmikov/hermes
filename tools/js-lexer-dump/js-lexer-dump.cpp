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
///     (identifier->getIdentifier(),
///      private_identifier->getPrivateIdentifier(),
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
///   - IDENT_OP tokens (currently only `as_operator`): no fields; these are
///     only produced when the parser calls convertCurTokenToIdentOp() —
///     a plain advance() loop will never emit them.
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
///                  --context=type            -> Type (Flow type grammar)
///                  --context=jsx             -> AllowJSXIdentifier
/// JSX child mode: --jsx-child drives lex.advanceInJSXChild() instead of
///                  lex.advance(ctx); it accumulates JSX text and emits
///                  l_brace/less for {/< (used for the jsx_text differential).
/// Strict-mode note: the lexer is constructed with strictMode=true (default),
/// so strict-mode reserved words are lexed as rw_* tokens. Pass --non-strict
/// to call setStrictMode(false): future reserved words (implements, interface,
/// package, private, protected, public, static, yield) then lex as plain
/// identifiers, legacy octal numbers / leading-zero decimals / legacy octal
/// string escapes are accepted without an error, and no strict-mode diagnostic
/// is emitted. The Rust lexer under differential test must be configured
/// identically.

#include "hermes/Parser/JSLexer.h"
#include "hermes/Support/SourceErrorManager.h"

#include "llvh/Support/MathExtras.h"
#include "llvh/Support/MemoryBuffer.h"
#include "llvh/Support/raw_ostream.h"

#include <cstdio>
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
static void emitFields(llvh::raw_ostream &os, const Token &tok) {
  switch (tok.getKind()) {
    case TokenKind::identifier:
      os << " ident=";
      quoteBytes(os, tok.getIdentifier()->str());
      break;

    case TokenKind::private_identifier:
      os << " ident=";
      quoteBytes(os, tok.getPrivateIdentifier()->str());
      break;

    case TokenKind::numeric_literal: {
      uint64_t bits = llvh::DoubleToBits(tok.getNumericLiteral());
      char buf[32];
      std::snprintf(
          buf, sizeof(buf), " bits=0x%016llx", (unsigned long long)bits);
      os << buf;
      break;
    }

    case TokenKind::bigint_literal:
      os << " value=";
      quoteBytes(os, tok.getBigIntLiteral()->str());
      os << " raw=";
      quoteBytes(os, tok.getBigIntLiteralRawValue()->str());
      break;

    case TokenKind::string_literal:
      os << " escapes=" << (tok.getStringLiteralContainsEscapes() ? 1 : 0);
      os << " value=";
      quoteBytes(os, tok.getStringLiteral()->str());
      break;

    case TokenKind::regexp_literal:
      os << " body=";
      quoteBytes(os, tok.getRegExpLiteral()->getBody()->str());
      os << " flags=";
      quoteBytes(os, tok.getRegExpLiteral()->getFlags()->str());
      break;

    // NOTE: template_middle and template_tail are produced only when the parser
    // calls JSLexer::rescanRBraceInTemplateLiteral(); a plain advance() loop
    // like this tool's will never emit them (it yields template_head…r_brace).
    case TokenKind::no_substitution_template:
    case TokenKind::template_head:
    case TokenKind::template_middle:
    case TokenKind::template_tail:
      os << " cooked=";
      if (tok.getTemplateValue() != nullptr) {
        quoteBytes(os, tok.getTemplateValue()->str());
      } else {
        os << "null";
      }
      os << " raw=";
      quoteBytes(os, tok.getTemplateRawValue()->str());
      break;

    case TokenKind::jsx_text:
      os << " value=";
      quoteBytes(os, tok.getJSXTextValue()->str());
      os << " raw=";
      quoteBytes(os, tok.getJSXTextRaw()->str());
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
  llvh::errs() << "Usage: " << argv0
               << " [--context=regexp|div|type|jsx] [--jsx-child] "
                  "[--non-strict] <file|->\n"
               << "  Dump tokens from the JS lexer to stdout.\n"
               << "  --context=regexp  Allow regexp literals after /\n"
               << "  --context=div     Allow division operator after /\n"
               << "  --context=type    Flow type grammar context\n"
               << "  --context=jsx     Allow JSX identifiers (`-`)\n"
               << "  --jsx-child       Drive advanceInJSXChild() (JSX text)\n"
               << "  --non-strict      Lex in non-strict mode "
                  "(setStrictMode(false))\n"
               << "  Use - to read from stdin.\n";
}

int main(int argc, char **argv) {
  // Parse arguments.
  JSLexer::GrammarContext grammarContext = JSLexer::AllowRegExp;
  bool jsxChild = false;
  bool strictMode = true;
  const char *filePath = nullptr;

  for (int i = 1; i < argc; ++i) {
    const char *arg = argv[i];
    if (std::strncmp(arg, "--context=", 10) == 0) {
      const char *val = arg + 10;
      if (std::strcmp(val, "regexp") == 0) {
        grammarContext = JSLexer::AllowRegExp;
      } else if (std::strcmp(val, "div") == 0) {
        grammarContext = JSLexer::AllowDiv;
      } else if (std::strcmp(val, "type") == 0) {
        grammarContext = JSLexer::Type;
      } else if (std::strcmp(val, "jsx") == 0) {
        grammarContext = JSLexer::AllowJSXIdentifier;
      } else {
        llvh::errs() << argv[0] << ": unknown context value '" << val << "'\n";
        usage(argv[0]);
        return 1;
      }
    } else if (std::strcmp(arg, "--jsx-child") == 0) {
      jsxChild = true;
    } else if (std::strcmp(arg, "--non-strict") == 0) {
      strictMode = false;
    } else if (arg[0] == '-' && arg[1] == '-') {
      llvh::errs() << argv[0] << ": unknown flag '" << arg << "'\n";
      usage(argv[0]);
      return 1;
    } else {
      if (filePath != nullptr) {
        llvh::errs() << argv[0] << ": too many positional arguments\n";
        usage(argv[0]);
        return 1;
      }
      filePath = arg;
    }
  }

  if (filePath == nullptr) {
    usage(argv[0]);
    return 1;
  }

  // Read input.
  auto fileBufOrErr =
      llvh::MemoryBuffer::getFileOrSTDIN(llvh::StringRef(filePath));
  if (!fileBufOrErr) {
    llvh::errs() << argv[0] << ": error reading '" << filePath
                 << "': " << fileBufOrErr.getError().message() << "\n";
    return 1;
  }

  // Set up the lexer.
  // NOTE: JSLexer is constructed with strictMode=true (the default). This
  // means strict-mode future reserved words (implements, interface, package,
  // private, protected, public, static, yield) are lexed as rw_* tokens, not
  // identifiers. Passing --non-strict calls setStrictMode(false) so they lex
  // as identifiers (and legacy octal / leading-zero numbers and legacy octal
  // string escapes are accepted silently). The Rust lexer under differential
  // test must be configured identically.
  JSLexer::Allocator alloc;
  SourceErrorManager sm;
  JSLexer lex(std::move(fileBufOrErr.get()), sm, alloc);
  lex.setStrictMode(strictMode);
  const char *base = lex.getBufferStart();

  llvh::raw_ostream &os = llvh::outs();

  // Token loop: advance to load the first token, then continue. In --jsx-child
  // mode, drive advanceInJSXChild() (which accumulates JSX text and emits
  // l_brace/less for {/<) instead of the regular advance().
  for (;;) {
    const Token *tok =
        jsxChild ? lex.advanceInJSXChild() : lex.advance(grammarContext);
    size_t start = (size_t)(tok->getStartLoc().getPointer() - base);
    size_t end = (size_t)(tok->getEndLoc().getPointer() - base);
    const char *nl = lex.isNewLineBeforeCurrentToken() ? "nl" : "--";
    os << start << ' ' << end << ' ' << nl << ' '
       << tokenVariantName(tok->getKind());
    emitFields(os, *tok);
    os << '\n';
    if (tok->getKind() == TokenKind::eof) {
      break;
    }
  }

  return 0;
}
