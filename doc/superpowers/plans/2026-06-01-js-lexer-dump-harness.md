# JS Lexer — Token-Dump Harness (subsystem ②) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a standalone C++ tool, `js-lexer-dump`, that links the real Hermes `JSLexer` and prints a deterministic, byte-for-byte-stable token dump for a source file — the differential oracle the Rust lexer will be validated against.

**Architecture:** A new Hermes tool under `tools/js-lexer-dump/`, registered in `tools/CMakeLists.txt`, built with `add_hermes_tool` linking `hermesParser`. It reads a file (or stdin), adds it to a `SourceErrorManager`, constructs a `JSLexer`, and loops `advance()` to EOF, emitting one line per token in a fixed format. Token kind names are emitted as the **`.def` variant names** (e.g. `rw_function`, `l_brace`, `numeric_literal`) so they match the Rust `TokenKind` variants exactly. Numeric values are printed as raw f64 bits to prove exact rounding. Diagnostics go to stderr (validated separately); the token stream goes to stdout.

**Tech Stack:** C++17 (Hermes style: 80-col, 2-space indent, no exceptions/RTTI, copyright header, doc comments), CMake, links `hermesParser`/`hermesSupport`/`LLVHSupport`. Built in the existing `cmake-build-asan` tree.

**Reference spec:** `doc/superpowers/specs/2026-06-01-js-lexer-design.md` (§ "Validation strategy").
**C++ source of truth / references:**
- `include/hermes/Parser/JSLexer.h` — `JSLexer`, `Token` accessors, `GrammarContext`, `getBufferStart()`, `getCurToken()`, `advance()`, `isNewLineBeforeCurrentToken()`.
- `unittests/Parser/JSLexerTest.cpp` — exact construction/driving idiom (`JSLexer::Allocator alloc; SourceErrorManager sm; JSLexer lex(input, sm, alloc); lex.advance(ctx)->getKind()`).
- `include/hermes/Parser/TokenKinds.def` — to generate variant-name and `tokenKindStr` tables.
- `tools/hermesc/CMakeLists.txt` — `add_hermes_tool` usage pattern.
- `tools/CMakeLists.txt` — `add_subdirectory` registration list.
- `lib/Parser/CMakeLists.txt` — `hermesParser` is `add_hermes_library(hermesParser ... LINK_OBJLIBS hermesSupport hermesAST dtoa LLVHSupport)`.
- File read: `llvh::MemoryBuffer::getFileOrSTDIN(path)`, then `sm.addNewSourceBuffer(std::move(buf))`.

**Build/run commands (used throughout):**
- Configure already done (the repo has `cmake-build-asan/` with `hermesc` built). Adding a new `add_subdirectory` is picked up automatically by Ninja on the next build (it re-runs CMake configure).
- Build: `cmake --build cmake-build-asan --target js-lexer-dump`
- Run: `cmake-build-asan/bin/js-lexer-dump <file.js>`  (or pipe via stdin: `... js-lexer-dump - < file.js`)

**Do NOT** `cd` out of the project root; pass paths to all commands.

---

## The dump format (the contract — pin this exactly)

One line per token, fields space-separated, terminated by `\n`:

```
<start> <end> <nl> <KIND>[ <field> ...]
```

- `<start>`, `<end>`: decimal byte offsets into the buffer =
  `tok.getStartLoc().getPointer() - lex.getBufferStart()` and
  `tok.getEndLoc().getPointer() - lex.getBufferStart()`.
- `<nl>`: `nl` if `lex.isNewLineBeforeCurrentToken()` else `--`.
- `<KIND>`: the `.def` **variant name** of `tok.getKind()` (e.g. `none`, `identifier`,
  `rw_function`, `l_brace`, `starstar`, `numeric_literal`, `eof`).
- Per-kind `<field>`s (none for punctuators / `eof`):
  - `identifier`, `private_identifier`, any reserved word: `ident=Q(name)`.
  - `numeric_literal`: `bits=0xHHHHHHHHHHHHHHHH` — the 64-bit IEEE-754 representation of
    `tok.getNumericLiteral()` via `llvh::DoubleToBits` (lowercase hex, zero-padded to 16).
  - `bigint_literal`: `value=Q(getBigIntLiteral) raw=Q(getBigIntLiteralRawValue)`.
  - `string_literal`: `escapes=<0|1> value=Q(getStringLiteral)` (use
    `getStringLiteralContainsEscapes()` for the flag; plain string literals have no raw —
    omit it).
  - `regexp_literal`: `body=Q(getRegExpLiteral()->getBody()) flags=Q(...getFlags())`.
  - `no_substitution_template`, `template_head`, `template_middle`, `template_tail`:
    `cooked=<Q(getTemplateValue)|null> raw=Q(getTemplateRawValue)` — emit the literal
    token `null` (unquoted) when `getTemplateValue()` is null (NotEscapeSequence case).
  - `jsx_text`: `value=Q(getJSXTextValue) raw=Q(getJSXTextRaw)`.

`Q(s)` = the quoting/escaping function (Task 2): wrap in double quotes; inside, emit
printable ASCII `0x20..0x7e` literally except `"`→`\"` and `\`→`\\`; `\n`→`\n`, `\t`→`\t`,
`\r`→`\r`; every other byte (including non-ASCII / ill-formed-UTF-8 WTF-8 bytes from lone
surrogates) as `\xHH` (lowercase, two digits). Operates on raw bytes of the `UniqueString`
(`->str()` → `llvh::StringRef`), never re-decoding — so WTF-8 bytes round-trip visibly.

A trailing `eof` line is always emitted. Example — input `for x=5` (default `--context=regexp`):
```
0 3 -- rw_for
4 5 nl-or--? identifier ident="x"
5 6 -- equal
6 7 -- numeric_literal bits=0x4014000000000000
7 7 -- eof
```
(The exact `nl`/`--` and offsets are whatever the real lexer reports; assertions in the
plan use inputs without leading whitespace so offsets are obvious.)

**Grammar context:** the tool drives every `advance()` with a single fixed context chosen
by `--context` (`regexp` default, or `div`). A standalone dumper has no parser feedback, so
regexp-vs-div disambiguation is out of its scope; the Rust differential drives its lexer
with the identical fixed context. (JSX/Flow contexts can be added later when those lexer
paths are ported; not in this subsystem.)

---

## File structure

```
tools/
  CMakeLists.txt                 # add: add_subdirectory(js-lexer-dump)
  js-lexer-dump/
    CMakeLists.txt               # add_hermes_tool(js-lexer-dump js-lexer-dump.cpp LINK_OBJLIBS hermesParser)
    js-lexer-dump.cpp            # the tool (format documented in a header comment)
```

---

## Task 0: Tool skeleton that builds and links

**Files:**
- Create: `tools/js-lexer-dump/CMakeLists.txt`
- Create: `tools/js-lexer-dump/js-lexer-dump.cpp`
- Modify: `tools/CMakeLists.txt` (register subdir)

- [ ] **Step 1: Create `tools/js-lexer-dump/CMakeLists.txt`**

```cmake
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

add_hermes_tool(js-lexer-dump js-lexer-dump.cpp
  LINK_OBJLIBS hermesParser hermesSupport LLVHSupport)
```

If linking fails for missing symbols, mirror `hermesParser`'s own `LINK_OBJLIBS`
(`hermesSupport hermesAST dtoa LLVHSupport`) — add what's needed and note it.

- [ ] **Step 2: Create `tools/js-lexer-dump/js-lexer-dump.cpp` (skeleton)**

Copyright header, then a `main` that reads argv, reads the file via
`llvh::MemoryBuffer::getFileOrSTDIN`, errors to stderr if it fails, and (for now) prints
nothing else. Include a top-of-file doc comment describing the dump format (copy the
"dump format" section above into the comment). Use `llvh::cl` or plain argv parsing — plain
argv is fine for one positional arg.

```cpp
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

/// \file js-lexer-dump.cpp
/// Differential oracle for the Rust JSLexer port: prints a deterministic,
/// byte-stable token dump for a JS source file. (Format documented below.)
// ... format doc comment ...

#include "hermes/Parser/JSLexer.h"
#include "hermes/Support/SourceErrorManager.h"
#include "llvh/Support/MemoryBuffer.h"
#include "llvh/Support/raw_ostream.h"

#include <cstdlib>

using namespace hermes;
using namespace hermes::parser;

int main(int argc, char **argv) {
  if (argc < 2) {
    llvh::errs() << "usage: js-lexer-dump [--context=regexp|div] <file.js|->\n";
    return 1;
  }
  // (argument parsing fleshed out in Task 5; for now take the last arg as the path.)
  const char *path = argv[argc - 1];
  auto fileBuf = llvh::MemoryBuffer::getFileOrSTDIN(path);
  if (!fileBuf) {
    llvh::errs() << "error: cannot read " << path << "\n";
    return 1;
  }
  return 0;
}
```

- [ ] **Step 3: Register the subdir**

In `tools/CMakeLists.txt`, add `add_subdirectory(js-lexer-dump)` near the other tool
subdirs (e.g. after `add_subdirectory(hermes-parser)`).

- [ ] **Step 4: Build**

Run: `cmake --build cmake-build-asan --target js-lexer-dump`
Expected: configures + compiles + links cleanly; produces `cmake-build-asan/bin/js-lexer-dump`.
Run: `cmake-build-asan/bin/js-lexer-dump --help` is not implemented; instead
`printf 'x' | cmake-build-asan/bin/js-lexer-dump -` should exit 0 with no output.

- [ ] **Step 5: Commit**

```bash
git add tools/js-lexer-dump/CMakeLists.txt tools/js-lexer-dump/js-lexer-dump.cpp tools/CMakeLists.txt
git commit -m "tools(js-lexer-dump): scaffold token-dump harness tool"
```

---

## Task 1: Token loop — kind, offsets, newline flag

**Files:** Modify `tools/js-lexer-dump/js-lexer-dump.cpp`.

- [ ] **Step 1: Add the variant-name table and the advance loop**

Add a `tokenVariantName(TokenKind)` using the `.def` (so names match Rust):

```cpp
static const char *tokenVariantName(TokenKind kind) {
  switch (kind) {
#define TOK(name, str)      \
  case TokenKind::name:     \
    return #name;
#include "hermes/Parser/TokenKinds.def"
  }
  return "<invalid>";
}
```

In `main`, after reading the buffer: create `JSLexer::Allocator alloc; SourceErrorManager sm;`
add the buffer with `unsigned bufId = sm.addNewSourceBuffer(std::move(fileBuf));`, construct
`JSLexer lex(bufId, sm, alloc);`, then loop:

```cpp
const char *base = lex.getBufferStart();
for (;;) {
  const Token *tok = lex.advance(grammarContext); // grammarContext fixed; Task 5 wires --context
  size_t start = tok->getStartLoc().getPointer() - base;
  size_t end = tok->getEndLoc().getPointer() - base;
  llvh::outs() << start << ' ' << end << ' '
               << (lex.isNewLineBeforeCurrentToken() ? "nl" : "--") << ' '
               << tokenVariantName(tok->getKind());
  emitFields(lex, *tok); // no-op for now; filled in Tasks 2-4
  llvh::outs() << '\n';
  if (tok->getKind() == TokenKind::eof)
    break;
}
```

Default `grammarContext` to `JSLexer::AllowRegExp` for now. Add an empty
`static void emitFields(JSLexer &, const Token &) {}` to be filled later.

- [ ] **Step 2: Build & validate on punctuators**

Run: `cmake --build cmake-build-asan --target js-lexer-dump`
Run: `printf '{ ; }\n;' | cmake-build-asan/bin/js-lexer-dump -`
Expected output (verify offsets/newline flags by hand against the input):
```
0 1 -- l_brace
2 3 -- semi
4 5 -- r_brace
6 7 nl semi
7 7 -- eof
```
Confirm the `nl` flag is set on the token after the `\n`.

- [ ] **Step 3: Commit**

```bash
git add tools/js-lexer-dump/js-lexer-dump.cpp
git commit -m "tools(js-lexer-dump): token loop with kind, offsets, newline flag"
```

---

## Task 2: Quoting helper + identifier/reserved-word values

**Files:** Modify `tools/js-lexer-dump/js-lexer-dump.cpp`.

- [ ] **Step 1: Implement `Q` (byte-exact quoting)**

```cpp
/// Append `s` to `os` as a quoted, escaped string. Operates on raw bytes so that
/// ill-formed UTF-8 (lone surrogates encoded by the lexer) round-trips visibly.
static void quoteBytes(llvh::raw_ostream &os, llvh::StringRef s) {
  os << '"';
  for (unsigned char c : s) {
    switch (c) {
      case '"': os << "\\\""; break;
      case '\\': os << "\\\\"; break;
      case '\n': os << "\\n"; break;
      case '\t': os << "\\t"; break;
      case '\r': os << "\\r"; break;
      default:
        if (c >= 0x20 && c <= 0x7e) {
          os << (char)c;
        } else {
          os << "\\x";
          const char *hex = "0123456789abcdef";
          os << hex[c >> 4] << hex[c & 0xf];
        }
    }
  }
  os << '"';
}
```

- [ ] **Step 2: Fill `emitFields` for identifiers and reserved words**

```cpp
static void emitFields(JSLexer &lex, const Token &tok) {
  switch (tok.getKind()) {
    case TokenKind::identifier:
    case TokenKind::private_identifier:
      llvh::outs() << " ident=";
      quoteBytes(llvh::outs(), tok.getIdentifier()->str()); // private uses getPrivateIdentifier()
      break;
    default:
      if (tok.isResWord()) {
        llvh::outs() << " ident=";
        quoteBytes(llvh::outs(), tok.getResWordIdentifier()->str());
      }
      break;
  }
}
```

Note: `private_identifier` must use `getPrivateIdentifier()` (the header asserts on kind);
split the `case` so each calls the matching accessor. Keep the structure faithful to the
header's accessor preconditions.

- [ ] **Step 3: Build & validate**

Run: `cmake --build cmake-build-asan --target js-lexer-dump`
Run: `printf 'function fooé bar' | cmake-build-asan/bin/js-lexer-dump -`
Expected (the `é` is UTF-8 `c3 a9`):
```
0 8 -- rw_function
9 14 -- identifier ident="foo\xc3\xa9"
15 18 nl-or-- identifier ident="bar"
18 18 -- eof
```
(Confirm the unicode bytes are emitted as `\xc3\xa9`.)

- [ ] **Step 4: Commit**

```bash
git add tools/js-lexer-dump/js-lexer-dump.cpp
git commit -m "tools(js-lexer-dump): byte-exact quoting + identifier/resword values"
```

---

## Task 3: Numeric (f64 bits) + bigint values

**Files:** Modify `tools/js-lexer-dump/js-lexer-dump.cpp`.

- [ ] **Step 1: Add numeric + bigint to `emitFields`**

Include `"llvh/Support/MathExtras.h"` for `llvh::DoubleToBits`. Add cases:

```cpp
    case TokenKind::numeric_literal: {
      uint64_t bits = llvh::DoubleToBits(tok.getNumericLiteral());
      char buf[19];
      snprintf(buf, sizeof(buf), "0x%016llx", (unsigned long long)bits);
      llvh::outs() << " bits=" << buf;
      break;
    }
    case TokenKind::bigint_literal:
      llvh::outs() << " value=";
      quoteBytes(llvh::outs(), tok.getBigIntLiteral()->str());
      llvh::outs() << " raw=";
      quoteBytes(llvh::outs(), tok.getBigIntLiteralRawValue()->str());
      break;
```

- [ ] **Step 2: Build & validate**

Run: `cmake --build cmake-build-asan --target js-lexer-dump`
Run: `printf '5 0.1 0xff 10n' | cmake-build-asan/bin/js-lexer-dump -`
Expected (verify the bit patterns: `5.0`=`0x4014000000000000`, `0.1`=`0x3fb999999999999a`,
`255.0`=`0x406fe00000000000`):
```
0 1 -- numeric_literal bits=0x4014000000000000
2 5 -- numeric_literal bits=0x3fb999999999999a
6 10 -- numeric_literal bits=0x406fe00000000000
11 14 -- bigint_literal value="10" raw="10n"
14 14 -- eof
```

- [ ] **Step 3: Commit**

```bash
git add tools/js-lexer-dump/js-lexer-dump.cpp
git commit -m "tools(js-lexer-dump): numeric f64 bits + bigint values"
```

---

## Task 4: String, template, regexp, JSX-text values

**Files:** Modify `tools/js-lexer-dump/js-lexer-dump.cpp`.

- [ ] **Step 1: Add the remaining value-bearing kinds to `emitFields`**

```cpp
    case TokenKind::string_literal:
      llvh::outs() << " escapes=" << (tok.getStringLiteralContainsEscapes() ? 1 : 0)
                   << " value=";
      quoteBytes(llvh::outs(), tok.getStringLiteral()->str());
      break;
    case TokenKind::regexp_literal:
      llvh::outs() << " body=";
      quoteBytes(llvh::outs(), tok.getRegExpLiteral()->getBody()->str());
      llvh::outs() << " flags=";
      quoteBytes(llvh::outs(), tok.getRegExpLiteral()->getFlags()->str());
      break;
    case TokenKind::no_substitution_template:
    case TokenKind::template_head:
    case TokenKind::template_middle:
    case TokenKind::template_tail: {
      llvh::outs() << " cooked=";
      if (tok.getTemplateValue())
        quoteBytes(llvh::outs(), tok.getTemplateValue()->str());
      else
        llvh::outs() << "null";
      llvh::outs() << " raw=";
      quoteBytes(llvh::outs(), tok.getTemplateRawValue()->str());
      break;
    }
    case TokenKind::jsx_text:
      llvh::outs() << " value=";
      quoteBytes(llvh::outs(), tok.getJSXTextValue()->str());
      llvh::outs() << " raw=";
      quoteBytes(llvh::outs(), tok.getJSXTextRaw()->str());
      break;
```

- [ ] **Step 2: Build & validate**

Run: `cmake --build cmake-build-asan --target js-lexer-dump`
Run: `printf '"a\\tb" `\``t${x}`\`` /ab/gi' | cmake-build-asan/bin/js-lexer-dump -`
(craft an input exercising a string with an escape, a template head + tail around `${x}`,
and a regexp). Verify:
- the string shows `escapes=1 value="a\tb"`,
- `template_head` shows `cooked="t" raw="t"` and `template_tail` after `}`,
- `regexp_literal` shows `body="ab" flags="gi"`.
Confirm the exact lines by reading the output; adjust the validation input as needed and
record the actual expected output in the commit message body if helpful.

- [ ] **Step 3: Commit**

```bash
git add tools/js-lexer-dump/js-lexer-dump.cpp
git commit -m "tools(js-lexer-dump): string/template/regexp/jsx-text values"
```

---

## Task 5: CLI (`--context`) + format doc + end-to-end check

**Files:** Modify `tools/js-lexer-dump/js-lexer-dump.cpp`.

- [ ] **Step 1: Parse `--context=regexp|div` and the path**

Replace the placeholder argv handling: scan args; `--context=regexp` (default) →
`JSLexer::AllowRegExp`, `--context=div` → `JSLexer::AllowDiv`; the remaining positional is
the path (`-` = stdin). Unknown flags → usage error to stderr, exit 1. Pass the chosen
`grammarContext` into the loop.

- [ ] **Step 2: Ensure the format doc comment at the top of the file is complete**

It must document every field and the `Q` escaping, so the Rust differential can be written
against it without reading the code.

- [ ] **Step 3: Build & end-to-end validate**

Run: `cmake --build cmake-build-asan --target js-lexer-dump`
Run on a mixed snippet file and confirm a coherent dump, and that `--context=div` changes
`/` handling:
```bash
printf 'let re = /x/g\n' | cmake-build-asan/bin/js-lexer-dump --context=regexp -
printf 'a /b/ c'        | cmake-build-asan/bin/js-lexer-dump --context=div -
```
Expected: with `regexp`, `/x/g` lexes as one `regexp_literal`; with `div` on `a /b/ c`,
the `/` lexes as `slash` tokens (division), not a regexp.

- [ ] **Step 4: Commit**

```bash
git add tools/js-lexer-dump/js-lexer-dump.cpp
git commit -m "tools(js-lexer-dump): --context flag, format docs, end-to-end check"
```

---

## Self-review checklist (after Task 5)

- [ ] Builds cleanly under `cmake-build-asan` (ASan) with no new warnings; tool at
  `cmake-build-asan/bin/js-lexer-dump`.
- [ ] Variant names come from `TokenKinds.def` (match Rust `TokenKind` variants exactly).
- [ ] `Q` emits ill-formed-UTF-8 bytes as `\xHH` (lone-surrogate round-trip), verified.
- [ ] Numeric printed as 16-digit f64 bits; bigint value+raw; string escapes-flag+value;
  template cooked(`null` when absent)+raw; regexp body+flags; jsx_text value+raw.
- [ ] Per-kind accessors respect the header's kind preconditions (e.g.
  `getPrivateIdentifier` for `private_identifier`).
- [ ] `--context` switches regexp/div; default regexp; stdin via `-`.
- [ ] Format documented in the file header comment.

## Next subsystem

After this lands: subsystem ③ the string interner (copy juno `atom_table` + WTF-8 byte
path), then ④ Unicode `CharacterProperties`, ⑤ number parsing, then the lexer proper — at
which point this harness becomes the live differential. See
`doc/superpowers/RustPortRoadmap.md`.
