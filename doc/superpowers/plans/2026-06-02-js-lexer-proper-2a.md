# JS Lexer Proper — Phase 2a: string literals + private identifiers

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Lex **string literals** (`'…'`/`"…"` with `\b\f\n\r\t\v`, octal/`\x`/`\u`/`\u{}` escapes, escaped line continuations, the `containsEscapes` flag) and **private identifiers** (`#name`), validated byte-for-byte against `js-lexer-dump`.

**Architecture:** Add `scan_string` (the non-JSX path), `consume_octal`, and `scan_private_identifier` to `parser::lexer`; wire the `'`/`"` and `#` (private-id) `advance` arms (the `#!` hashbang branch is already done in 1a). Extend `emit_fields` with `string_literal` → `escapes=<0|1> value=Q(cooked)`. The `convertSurrogates` re-encoding path is NOT in scope (off by default; needs UTF-16 conversion utilities — tracked as a follow-up).

**Tech Stack:** Rust 2021; `unsafe` only in `cursor.rs`. Reuses `consume_hex`/`consume_unicode_escape`/`append_unicode_to_storage`/`consume_identifier_start`/`scan_identifier_fast_path`/`scan_identifier_parts` (all from 1b-i), `utf8::decode_utf8`, `atom_table`.

**Reference spec:** `doc/superpowers/specs/2026-06-01-js-lexer-design.md`.
**C++ source of truth:** `lib/Parser/JSLexer.cpp:1311–1327` (`consumeOctal`), `:1951–1975` (`scanPrivateIdentifier`), `:1977–2126` (`scanString` — port the **non-JSX** branches; the `JSX`-gated `&`-entity and newline-in-JSX-string branches are phase 3), `:650–735`/`:700–709` (the `'`/`"` and `#` `advance` arms). Dump: `tools/js-lexer-dump/js-lexer-dump.cpp` (`string_literal` → `escapes=<0|1> value=Q(getStringLiteral)`; `private_identifier` already emits `ident=`).

**Porting rule:** faithful port; copy comments. The string escape `switch` and the loop's quote/backslash/newline/EOF/UTF-8 arms must match the C++ exactly (incl. the `\0`-not-octal special case, the strict-mode octal error in `consumeOctal`, and the "non-terminated string" + "string started here" note).

**Do NOT** `cd` out of the project root.

---

## Task 0: `consume_octal` + `scan_string` (non-JSX)

**Files:** `rust/crates/parser/src/lexer.rs`.

- [ ] **Step 1: failing tests** (helper `str_cooked(src) -> (Vec<u8> /*cooked bytes*/, bool /*escapes*/)` that lexes one `string_literal`):

```rust
#[test]
fn strings_basic() {
    use TokenKind::*;
    assert_eq!(kinds("'a' \"b\""), vec![string_literal, string_literal, eof]);
    assert_eq!(str_cooked("'hello'"), (b"hello".to_vec(), false));
    assert_eq!(str_cooked("\"a\\tb\""), (b"a\tb".to_vec(), true));     // \t -> tab, escapes=true
    assert_eq!(str_cooked("'\\n\\r\\\\'"), (vec![10, 13, b'\\'], true));
    assert_eq!(str_cooked("'\\x41'"), (b"A".to_vec(), true));         // \x41 -> 'A'
    assert_eq!(str_cooked("'\\u00e9'"), (b"\xc3\xa9".to_vec(), true)); // é (WTF-8)
    assert_eq!(str_cooked("'a\\\nb'"), (b"ab".to_vec(), true));       // escaped newline continuation
    assert_eq!(str_cooked("'caf\u{00e9}'"), (b"caf\xc3\xa9".to_vec(), false)); // raw unicode, no escape
}
```

- [ ] **Step 2:** run → FAIL.
- [ ] **Step 3: implement** `consume_octal(max_len)` (port `:1311–1327`, incl. the strict-mode
  "octals not allowed in strict mode" error) and `scan_string` (port `:1977–2126`, the non-JSX
  path): the quote loop, the escape `switch` (`'"\\`, `b f n r t v`, the `\0`-EOF / `\0`-not-octal
  / octal `0-3`(maxlen 3)/`4-7`(maxlen 2), `\x`→`consume_hex(2)`, `\u`→`consume_unicode_escape`,
  escaped `\n`/`\r`(+CRLF)/U+2028-9 line continuations, the UTF-8/`default` arms), the raw `\n`/`\r`
  → "non-terminated string" error, the EOF `\0` → error, and the non-escape UTF-8/byte arms; then
  `token.set_string_literal(strtab.atom_bytes(tmp_storage), escapes)`. Wire the `'`/`"` `advance`
  arm → `scan_string` (replace the 1b stub). NOTE: `convert_surrogates` is off by default — assert
  it is false here (the re-encoding path is deferred); intern `tmp_storage` directly.
- [ ] **Step 4:** run → PASS. **Step 5:** commit `rust(parser): port scanString (string literals, non-JSX)`.

---

## Task 1: `scan_private_identifier` + `#` arm

**Files:** `rust/crates/parser/src/lexer.rs`.

- [ ] **Step 1: failing test:**

```rust
#[test]
fn private_identifiers() {
    use TokenKind::*;
    assert_eq!(kinds("#foo #bar"), vec![private_identifier, private_identifier, eof]);
    assert_eq!(ident_bytes_of("#x"), b"x"); // the interned name excludes '#'
    // '#' followed by no identifier -> "empty private identifier" error (still a token? returns false -> continue)
}
```

- [ ] **Step 2:** FAIL. **Step 3: implement** `scan_private_identifier` (port `:1951–1975`): skip
  `#`, then if `is_ascii_identifier_start(peek)` → `scan_identifier_fast_path::<JS>`, else if
  `consume_identifier_start()` → `scan_identifier_parts::<JS>`, else error "empty private
  identifier" + return false. On success `token.set_private_identifier(token.res_word_or_identifier())`.
  Wire the `#` `advance` arm (port `:563–574`): the `#!`-at-buffer-start hashbang is ALREADY
  handled (1a) — confirm and don't duplicate; the else branch sets token start and calls
  `scan_private_identifier`; if it returns false, `continue` (no token emitted).
- [ ] **Step 4:** PASS. **Step 5:** commit `rust(parser): port scanPrivateIdentifier (#name)`.

---

## Task 2: dump `string_literal` field + differential

**Files:** `rust/crates/parser/src/lexer.rs`, `rust/crates/parser/tests/differential.rs`.

- [ ] **Step 1:** Extend `emit_fields`: `string_literal` → ` escapes=` + (1 or 0 from
  `token.string_literal_contains_escapes()`) + ` value=` + `quote_bytes(strtab.bytes(token.string_literal_atom))`.
  (`private_identifier` already emits `ident=`.)
- [ ] **Step 2:** Extend the differential corpus (still `--context=div`) with strings + private ids:

```rust
    "'a' \"b\" 'hello world'",
    "'a\\tb' \"x\\ny\" '\\r\\\\\\''",       // escapes -> escapes=1
    "'\\x41\\x7e' '\\u00e9\\u4e2d'",          // hex + unicode escapes
    "'caf\u{00e9}' \"\u{4e2d}\u{6587}\"",     // raw unicode in strings -> WTF-8 \xHH
    "'a\\\nb' 'line\\\r\ncont'",              // escaped line continuations
    "#foo #_bar x.#priv",                     // private identifiers (and member-like)
    "'a' #b 5 ;",                             // mixed
```
(Verify each against the oracle; drop/adjust any that produce a stderr error with mismatching
stdout — keep the differential byte-for-byte on stdout.)

- [ ] **Step 3:** `cmake --build cmake-build-asan --target js-lexer-dump`; then
  `cargo test --manifest-path rust/Cargo.toml -p parser --test differential -- --nocapture` →
  runs (not skipped), passes; compared-count up.
- [ ] **Step 4:** full `cargo test -p parser` → all pass; zero warnings; `unsafe` only in `cursor.rs`.
- [ ] **Step 5:** commit `rust(parser): dump string_literal field + string/private-id differential`.

---

## Self-review checklist

- [ ] `scan_string` matches `JSLexer.cpp:1977–2126` (non-JSX): the escape switch, octal handling
  (`\0`-not-octal special case, `consume_octal(3)`/`(2)`), `\x`/`\u`/`\u{}`, line continuations,
  the "non-terminated string" + "string started here" paths, the UTF-8 re-encode arms.
- [ ] `consume_octal` strict-mode error matches; `containsEscapes` is set iff a `\` was seen.
- [ ] `scan_private_identifier` matches `:1951–1975`; the `#` arm doesn't duplicate the 1a hashbang.
- [ ] Rust dump `string_literal escapes=/value=` equals `js-lexer-dump` byte-for-byte (incl. WTF-8
  `\xHH` and control bytes).
- [ ] Deferred-and-noted: `convertSurrogates` re-encoding (needs UTF-16 conversion utils); JSX
  string `&`-entities + JSX-newline-in-string (phase 3); templates (2b); regexp (2c).
- [ ] `unsafe` only in `cursor.rs`; zero warnings; all tests pass.

## Next
Phase 2b: template literals (`scanTemplateLiteral` — `no_substitution_template`/`template_head`;
`template_middle`/`template_tail` need `rescanRBrace`, phase 4). Then 2c regexp + the
`AllowRegExp` `/` arm + `--context=regexp` differential. See the roadmap.
```
