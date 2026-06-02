# JS Lexer Proper — Phase 2c: regular-expression literals

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Lex regular-expression literals (`/body/flags`) under `AllowRegExp` — scanning the body (char classes, escapes, non-terminated errors) uninterpreted, then the flags — and wire the `AllowRegExp` `/` `advance` arm. Validated byte-for-byte against `js-lexer-dump --context=regexp`.

**Architecture:** Add `scan_regexp` to `parser::lexer` (a child module `lexer/regexp.rs`, matching the post-split layout). Port `JSLexer.cpp:2384–2484`. Wire the `AllowRegExp` branch of the `/` `advance` arm (replacing the 1a "treat as div" TODO). Extend `emit_fields` with `regexp_literal` → `body=Q(..) flags=Q(..)`. Parameterize the differential harness by `GrammarContext` and add a `--context=regexp` corpus.

**Tech Stack:** Rust 2021; `unsafe` only in `cursor.rs`. Reuses `consume_one_identifier_part_no_escape::<JS>`, `append_unicode_to_storage`, `decode_utf8_advance`, `match_unicode_line_terminator_offset1`, `strtab.atom_bytes`. Token: `set_regexp_literal(RegExpLiteral { body, flags })`, `get_regexp_literal()`.

**Reference spec:** `doc/superpowers/specs/2026-06-01-js-lexer-design.md`.
**C++ source of truth:** `lib/Parser/JSLexer.cpp:2384–2484` (`scanRegExp`), the `/` `advance` arm (`:542–561`, the `AllowRegExp` branch). Dump: `tools/js-lexer-dump/js-lexer-dump.cpp` (`regexp_literal` → `body=Q flags=Q`).

**Porting rule:** faithful port; copy comments. The body loop's `inClass` `[`/`]` tracking, the `\`-escape-can't-be-a-line-terminator rule, the non-terminated error + "regular expression started here" note, and the flags loop's `\u`-in-flags error must match exactly. The C++ `RegExpLiteral` is heap-allocated via the bump allocator; in Rust the `Token` stores `RegExpLiteral { body: AtomBytes, flags: AtomBytes }` by value (the type already exists from phase 1a).

**Do NOT** `cd` out of the project root.

---

## Task 0: `scan_regexp` + `/` AllowRegExp arm

**Files:** `rust/crates/parser/src/lexer/regexp.rs` (new), `rust/crates/parser/src/lexer/mod.rs` (declare `mod regexp;`, wire the `/` arm).

- [ ] **Step 1: failing tests** (helper `regexp(src) -> (Vec<u8> /*body*/, Vec<u8> /*flags*/)`, lexing
  with `GrammarContext::AllowRegExp`):

```rust
#[test]
fn regexp_basic() {
    use TokenKind::*;
    assert_eq!(kinds_ctx("/abc/g", GrammarContext::AllowRegExp), vec![regexp_literal, eof]);
    assert_eq!(regexp("/abc/gi"), (b"abc".to_vec(), b"gi".to_vec()));
    assert_eq!(regexp("/[/]/"), (b"[/]".to_vec(), b"".to_vec()));      // '/' inside a class is body
    assert_eq!(regexp("/a\\/b/"), (b"a\\/b".to_vec(), b"".to_vec()));  // escaped '/' is body
    assert_eq!(regexp("/x/y"), (b"x".to_vec(), b"y".to_vec()));
    // under AllowDiv, '/' is a division operator, NOT a regexp:
    assert_eq!(kinds_ctx("a / b", GrammarContext::AllowDiv), vec![identifier, slash, identifier, eof]);
}
```

- [ ] **Step 2:** run → FAIL.
- [ ] **Step 3: implement** `scan_regexp` (port `:2384–2484`): assert `/`, advance, scan the body
  into `tmp_storage` (the `inClass` `[`/`]` toggle; `/` outside a class ends the body; `\` escape
  pushes the `\` then checks the next char for a line terminator/EOF → non-terminated; raw line
  terminator/EOF → non-terminated error + "regular expression started here" note; UTF-8 re-encode;
  plain byte), intern the body; then scan the flags into a fresh `tmp_storage` via
  `consume_one_identifier_part_no_escape::<JS>` in a loop, with the `\` handling that errors on
  `\u` in flags (the `escaping_backslash` toggle), intern the flags; then
  `token.set_regexp_literal(RegExpLiteral::new(body, flags))`. Wire the `/` `advance` arm's
  `AllowRegExp` branch → `scan_regexp` (the comment sub-cases and the `AllowDiv` slash/slashequal
  sub-cases stay as in 1a). NOTE: `convert_surrogates` off — intern directly.
- [ ] **Step 4:** run → PASS. **Step 5:** commit `rust(parser): port scanRegExp (regexp literals) + AllowRegExp / arm`.

---

## Task 1: dump `regexp_literal` field + `--context=regexp` differential

**Files:** `rust/crates/parser/src/lexer/dump.rs`, `rust/crates/parser/tests/differential.rs`.

- [ ] **Step 1:** Extend `emit_fields`: `regexp_literal` → ` body=` + `Q(strtab.bytes(re.body))` +
  ` flags=` + `Q(strtab.bytes(re.flags))`.
- [ ] **Step 2:** Parameterize the differential by context. Refactor `rust_dump`/`cpp_dump` to take a
  `GrammarContext`/`&str` context arg (the Rust side maps `AllowRegExp`→`GrammarContext::AllowRegExp`
  and passes `--context=regexp` to the tool; keep the existing `AllowDiv`/`--context=div` path). Add a
  `differential_regexp` test with a regexp corpus run under `--context=regexp`:

```rust
    // regexp corpus (driven with --context=regexp on both sides)
    "/abc/g /x/ /[a-z]+/gi",
    "/[/]/ /a\\/b/ /\\d+/",                 // '/' in class, escaped '/', escape
    "/foo/gimsuy",                          // all flags
    "/\u{4e2d}/u",                          // unicode in body -> WTF-8
    "x = /re/g",                            // div context would differ; here regexp follows '='
```
  (Verify each against `js-lexer-dump --context=regexp -`; keep byte-for-byte matches. Also confirm
  the existing `--context=div` corpus still passes after the refactor.)

- [ ] **Step 3:** `cmake --build cmake-build-asan --target js-lexer-dump`; then
  `cargo test --manifest-path rust/Cargo.toml -p parser --test differential -- --nocapture` → both
  the div and regexp differential tests run (not skipped) and pass; compared-counts shown.
- [ ] **Step 4:** full `cargo test -p parser` → all pass; zero warnings; `unsafe` only in `cursor.rs`.
- [ ] **Step 5:** commit `rust(parser): dump regexp_literal field + regexp differential (--context=regexp)`.

---

## Self-review checklist

- [ ] `scan_regexp` matches `JSLexer.cpp:2384–2484`: the `inClass` tracking, the `\`-escape line-terminator
  rule, the non-terminated error + note, the flags loop + `\u`-in-flags error, body/flags interned.
- [ ] The `/` `advance` arm now scans a regexp under `AllowRegExp` (and still divides under `AllowDiv`).
- [ ] Rust dump `regexp_literal body=/flags=` equals `js-lexer-dump --context=regexp` byte-for-byte.
- [ ] The differential harness handles both `--context=div` and `--context=regexp`; both corpora pass.
- [ ] Deferred-and-noted: JSX/Flow (phase 3); savepoint/lookahead/rescanRBrace/directives/magic
  comments/comment storage (phase 4); `convertSurrogates` re-encoding.
- [ ] `unsafe` only in `cursor.rs`; zero warnings; all tests pass.

## Next
Phase 3: JSX (`advanceInJSXChild`, HTML entities `consumeHTMLEntityOptional`, JSX identifier mode,
JSX string `&`-entities) + Flow (`Type` grammar context, `%checks`, `@`-identifiers, the
`PUNCTUATOR_FLOW` tokens) — extending the harness `--context` options. Then phase 4. See the roadmap.
```
