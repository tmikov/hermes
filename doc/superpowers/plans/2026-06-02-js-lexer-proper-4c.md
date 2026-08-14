# JS Lexer Proper — Phase 4c: `convertSurrogates` (the last JSLexer feature)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Implement the `convertSurrogates` lexer option: when set, `getStringLiteral` re-encodes the WTF-8 internal string form (lone surrogates / surrogate-encoded astral chars) into **valid UTF-8** (combining surrogate pairs into supplementary-plane characters, replacing unpaired surrogates with U+FFFD), via `convertSurrogatesInString`. This is the last `JSLexer` feature; after it, the lexer port is complete.

**Architecture:** Add the UTF-8↔UTF-16 conversion utilities to `parser::utf8` (`encode_utf16`, `convert_utf8_with_surrogates_to_utf16`, `convert_to_code_point_at`, `convert_utf16_to_utf8_with_replacements`), a `convert_surrogates_in_string` lexer method, and a `get_string_literal(bytes) -> AtomBytes` lexer method that branches on `convert_surrogates`. Replace the `debug_assert!(!self.convert_surrogates)` guards in `scan_string`/`scan_template_literal`/`scan_regexp` with `get_string_literal` calls (the C++ `getStringLiteral`), and add a `with_convert_surrogates` constructor option / setter.

**Tech Stack:** Rust 2021; `unsafe` only in `cursor.rs`.
**Reference spec:** `doc/superpowers/specs/2026-06-01-js-lexer-design.md`.
**C++ source of truth:** `include/hermes/Support/UTF8.h:195-225` (`encodeUTF16`, `convertUTF8WithSurrogatesToUTF16`), `lib/Support/UTF8.cpp:77-110` (`convertToCodePointAt`) + `convertUTF16ToUTF8WithReplacements`, `lib/Parser/JSLexer.cpp:2486-2495` (`convertSurrogatesInString`), `JSLexer.h:689-694` (`getStringLiteral`), the `JSLexer` ctor `convertSurrogates` param.

**Porting rule:** faithful port; copy comments. `convertSurrogatesInString` produces *valid* UTF-8 (unlike `appendUnicodeToStorage`'s WTF-8). It is only invoked when the flag is set.

**Do NOT** `cd` out of the project root.

---

## Task 0: UTF-8↔UTF-16 conversion utilities

**Files:** `rust/crates/parser/src/utf8.rs`.

- [x] **Step 1: failing tests:**

```rust
#[test]
fn utf16_roundtrip_and_replacement() {
    // encode_utf16: BMP -> 1 u16, astral -> surrogate pair.
    let mut v = vec![]; encode_utf16(&mut v, 0x41); assert_eq!(v, [0x41]);
    let mut v = vec![]; encode_utf16(&mut v, 0x1F600); assert_eq!(v, [0xD83D, 0xDE00]);

    // convert_utf8_with_surrogates_to_utf16: WTF-8 astral (surrogate pair, 3 bytes each) -> 2 u16.
    let wtf8: &[u8] = b"\xed\xa0\xbd\xed\xb8\x80"; // U+1F600 as a surrogate pair (WTF-8)
    let u16s = convert_utf8_with_surrogates_to_utf16(wtf8);
    assert_eq!(u16s, [0xD83D, 0xDE00]);

    // convert_utf16_to_utf8_with_replacements: surrogate pair -> 4-byte UTF-8; lone surrogate -> U+FFFD.
    assert_eq!(convert_utf16_to_utf8_with_replacements(&[0xD83D, 0xDE00]), b"\xf0\x9f\x98\x80".to_vec());
    assert_eq!(convert_utf16_to_utf8_with_replacements(&[0xD800]), "\u{FFFD}".as_bytes().to_vec()); // lone high
    assert_eq!(convert_utf16_to_utf8_with_replacements(&[0xDC00]), "\u{FFFD}".as_bytes().to_vec()); // lone low
    assert_eq!(convert_utf16_to_utf8_with_replacements(&[0x41, 0x42]), b"AB".to_vec());
}
```

- [x] **Step 2:** FAIL. **Step 3: implement** (port the C++):
  - `encode_utf16(out: &mut Vec<u16>, cp: u32)` — port `UTF8.h:197-210`.
  - `convert_utf8_with_surrogates_to_utf16(bytes: &[u8]) -> Vec<u16>` — port `:216-225`: loop
    `decode_utf8::<true>` (surrogates allowed, no-op error) then `encode_utf16`.
  - `convert_to_code_point_at(u16s: &[u16], i: usize) -> (u32, usize)` — port `UTF8.cpp:77-96`:
    low surrogate→(FFFD,1); high surrogate + next low→(pair,2) else (FFFD,1); else (c,1).
  - `convert_utf16_to_utf8_with_replacements(u16s: &[u16]) -> Vec<u8>` — port the `convertUTF16ToUTF8WithReplacements`
    loop (ASCII fast path; else `convert_to_code_point_at` + `encode_utf8`). (Skip the `maxCharacters`
    param — the lexer always passes 0/unbounded.)
- [x] **Step 4:** PASS. **Step 5:** commit `rust(parser): UTF-8<->UTF-16 conversion utils (surrogate handling)`.

---

## Task 1: `convert_surrogates_in_string` + `get_string_literal` wiring

**Files:** `rust/crates/parser/src/lexer/` (a `string`/`state` module + `mod.rs`).

- [x] **Step 1: failing test:**

```rust
#[test]
fn convert_surrogates() {
    // With convert_surrogates ON, an astral char in a string literal is re-encoded
    // to VALID UTF-8 (not the WTF-8 surrogate-pair form).
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("t", "'\\u{1F600}' '\\uD800'");
    let tab = AtomTable::new();
    let mut lex = JSLexer::new_with_convert_surrogates(id, &mut sm, &tab, GrammarContext::AllowDiv, true);
    let t = lex.advance(GrammarContext::AllowDiv);
    assert_eq!(tab.bytes(t.get_string_literal()), b"\xf0\x9f\x98\x80"); // valid 4-byte UTF-8 emoji
    let t = lex.advance(GrammarContext::AllowDiv);
    assert_eq!(tab.bytes(t.get_string_literal()), "\u{FFFD}".as_bytes()); // lone surrogate -> U+FFFD

    // With it OFF (default), the WTF-8 form is preserved (the existing 2a behavior).
    let mut sm2 = SourceErrorManager::new();
    let id2 = sm2.add_buffer("t2", "'\\u{1F600}'");
    let tab2 = AtomTable::new();
    let mut lex2 = JSLexer::new(id2, &mut sm2, &tab2, GrammarContext::AllowDiv);
    let t = lex2.advance(GrammarContext::AllowDiv);
    assert_eq!(tab2.bytes(t.get_string_literal()), b"\xed\xa0\xbd\xed\xb8\x80"); // WTF-8 surrogate pair
}
```

- [x] **Step 2:** FAIL. **Step 3: implement:**
  - `convert_surrogates_in_string(&self, bytes: &[u8]) -> AtomBytes` (port `JSLexer.cpp:2486-2495`):
    `convert_utf8_with_surrogates_to_utf16(bytes)` → `convert_utf16_to_utf8_with_replacements(&u16s)` →
    `strtab.atom_bytes(&out)`.
  - `get_string_literal(&self, bytes: &[u8]) -> AtomBytes` (port `JSLexer.h:689-694`): if
    `convert_surrogates` → `convert_surrogates_in_string(bytes)` else `strtab.atom_bytes(bytes)`.
  - Replace the `debug_assert!(!self.convert_surrogates)` + direct `strtab.atom_bytes(...)` in
    `scan_string` (the cooked value), `scan_template_literal` (cooked AND raw — the C++
    `scanTemplateLiteral` uses `getStringLiteral` for both), and `scan_regexp` (body + flags — the C++
    `scanRegExp` uses `getStringLiteral`) with `self.get_string_literal(...)`. (advance_in_jsx_child's
    `set_jsx_text` also uses `getStringLiteral` in the C++ — wire it too.)
  - Add `new_with_convert_surrogates(buf_id, sm, strtab, grammar_context, convert_surrogates)` (or a
    `set_convert_surrogates`); the existing `new` keeps `convert_surrogates = false`.
- [x] **Step 4:** PASS. **Step 5:** Run the FULL suite incl. the 5 differentials (they use the default
  `convert_surrogates=false`, so they must be UNCHANGED). Commit
  `rust(parser): convertSurrogates re-encoding (getStringLiteral)`.

---

## Self-review checklist

- [x] The UTF-16 utils match the C++ (`encodeUTF16`, `convertToCodePointAt` replacement rules,
  `convertUTF16ToUTF8WithReplacements`).
- [x] `convert_surrogates_in_string` produces valid UTF-8 (astral pairs combined; lone surrogates →
  U+FFFD); `get_string_literal` branches on the flag.
- [x] All string/template/regexp/jsx-text value interning goes through `get_string_literal` (matching
  the C++), so the flag affects them all; with the flag OFF the behavior is byte-identical to before
  (the 5 differentials still pass unchanged).
- [x] `unsafe` only in `cursor.rs`; zero warnings; all tests pass.

## Done
After 4c the `JSLexer` port is complete: the full public surface (token lexing, JSX, Flow, all
literals, storage, magic comments, SavePoint, lookahead, directives, rescanRBrace, convertSurrogates)
is ported and validated. Update `doc/superpowers/RustPortRoadmap.md` to mark the lexer **COMPLETE**.
Remaining optional: the `--non-strict` js-lexer-dump flag (test convenience only).
```
