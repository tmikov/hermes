# JS Lexer Proper — Phase 3b: JSX

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Lex JSX — `advanceInJSXChild` (JSX text + `{`/`<` delimiters), HTML entities (`&name;`/`&#NNN;`/`&#xHEX;`) via `HTMLEntities.def`, JSX string-literal `&`-entities + newline-in-string, and the JSX identifier mode (`-`). Validated byte-for-byte against an extended `js-lexer-dump` (`--context=jsx` + a `--jsx-child` mode).

**Architecture:** Add an `html_entities` table (generated from `HTMLEntities.def`), `consume_html_entity_optional`, the JSX branches of `scan_string` (the `<true>` path), `advance_in_jsx_child`, and the `jsx_text` dump field to `parser::lexer`. Wire `scan_string_in_context` to pick `IdentifierMode::JSX`-string behavior under `AllowJSXIdentifier`. Extend the C++ harness with `--context=jsx` (→ `AllowJSXIdentifier`) and `--jsx-child` (loop `advanceInJSXChild`).

**Tech Stack:** Rust 2021; `unsafe` only in `cursor.rs`. Python for the entity-table generator (like `unicode/gen_tables.py`).
**Reference spec:** `doc/superpowers/specs/2026-06-01-js-lexer-design.md`.
**C++ source of truth:** `include/hermes/Parser/HTMLEntities.def` (254 `HTML_ENTITY(name, value)` rows), `lib/Parser/JSLexer.cpp:45–59` (`initializeHTMLEntities`/`getHTMLEntities`), `:749–809` (`advanceInJSXChild`), `:811–907` (`consumeHTMLEntityOptional`), `:2093–2109` (the JSX branches of `scanString`: raw `\n`/`\r` pushed (not error), `&`→`consumeHTMLEntityOptional`), `JSLexer.h` `scanStringInContext` (JSX selection). Dump: `tools/js-lexer-dump/js-lexer-dump.cpp` (`jsx_text` → `value=Q raw=Q`; `string_literal` unchanged).

**Porting rule:** faithful port; copy comments. The entity table is generated from the `.def` (not hand-typed). `advanceInJSXChild` is a *separate* entry point from `advance` (the parser calls it for JSX children).

**Do NOT** `cd` out of the project root.

---

## Task 0: HTML entity table

**Files:** `rust/crates/parser/gen_html_entities.py` (new), `rust/crates/parser/src/html_entities.rs` (generated), `lib.rs` (`pub mod html_entities;`).

- [ ] **Step 1:** Write `gen_html_entities.py` (model on `rust/crates/unicode/gen_tables.py`): parse
  `include/hermes/Parser/HTMLEntities.def` (`HTML_ENTITY\((\w+),\s*(0x[0-9a-fA-F]+)\)`), assert 254
  rows, emit `src/html_entities.rs` with a `pub static HTML_ENTITIES: [(&[u8], u32); 254]` **sorted by
  name** (so a binary search works), plus a `pub fn lookup(name: &[u8]) -> Option<u32>` using
  `binary_search_by`.
- [ ] **Step 2:** Run `python3 rust/crates/parser/gen_html_entities.py`; add `pub mod html_entities;`
  to `lib.rs`. Add a test: `assert_eq!(html_entities::lookup(b"amp"), Some(0x26)); assert_eq!(lookup(b"lt"), Some(0x3c)); assert_eq!(lookup(b"nope"), None);` and a sorted/length-254 invariant test.
- [ ] **Step 3:** `cargo test -p parser` → passes. **Step 4:** commit
  `rust(parser): generate HTML entity table from HTMLEntities.def`.

---

## Task 1: `consume_html_entity_optional`

**Files:** `rust/crates/parser/src/lexer/jsx.rs` (new child module; declare `mod jsx;` in `lexer/mod.rs`).

- [ ] **Step 1: failing test** (helper that lexes a JSX string or via Task 2/3; for Task 1, a targeted
  `consume_entity_for_test(&str) -> Option<u32>`):

```rust
#[test]
fn html_entities() {
    assert_eq!(entity("&amp;"), Some(0x26));       // named
    assert_eq!(entity("&#65;"), Some(65));          // decimal
    assert_eq!(entity("&#x41;"), Some(0x41));       // hex
    assert_eq!(entity("&nope;"), None);             // unknown name -> None, cursor reset
    assert_eq!(entity("&amp"), None);               // no semicolon -> None
}
```

- [ ] **Step 2:** FAIL. **Step 3: implement** `consume_html_entity_optional() -> Option<u32>` (port
  `JSLexer.cpp:811–907`): the `&#x…;` hex form, the `&#…;` decimal form (both with the
  `> UNICODE_MAX_VALUE` overflow break and the non-empty-digits-before-`;` requirement), and the
  `&name;` form (up to 9 chars lookahead, `html_entities::lookup`). On any failure, reset the cursor
  to `start` and return `None`.
- [ ] **Step 4:** PASS. **Step 5:** commit `rust(parser): port consumeHTMLEntityOptional`.

---

## Task 2: JSX `scan_string` branches

**Files:** `rust/crates/parser/src/lexer/string.rs`, `lexer/mod.rs` (the `scan_string_in_context` selection).

- [ ] **Step 1: failing test** (lex a JSX-context string):

```rust
#[test]
fn jsx_string() {
    // In JSX context, '&' entities are decoded and raw newlines are allowed in strings.
    assert_eq!(jsx_str_cooked("\"a&amp;b\""), b"a&b".to_vec());   // & -> &amp; -> '&'
    assert_eq!(jsx_str_cooked("'x\ny'"), b"x\ny".to_vec());        // raw newline allowed (no error)
    assert_eq!(jsx_str_cooked("\"&#65;\""), b"A".to_vec());
}
```

- [ ] **Step 2:** FAIL. **Step 3: implement** the JSX branches in `scan_string` (the `<true>`/`JSX`
  path, `JSLexer.cpp:2093–2109`): when JSX, a raw `\n`/`\r` is pushed to storage (NOT a non-terminated
  error); a `&` calls `consume_html_entity_optional` (append the code point if Some, else push `&`).
  Make `scan_string` parameterized by a `jsx: bool` (or a `StringContext` enum), and have
  `scan_string_in_context` pass `jsx = (grammar_context == AllowJSXIdentifier)` (port
  `scanStringInContext`). The non-JSX path (2a) is unchanged. Also note: under JSX, the `\` escape
  branch is NOT taken (`!JSX && *curCharPtr_ == '\\'` in the C++ — in JSX a backslash is a literal char).
- [ ] **Step 4:** PASS. **Step 5:** commit `rust(parser): JSX string branches (& entities, raw newlines)`.

---

## Task 3: `advance_in_jsx_child` + `jsx_text` dump

**Files:** `rust/crates/parser/src/lexer/jsx.rs`, `lexer/dump.rs`.

- [ ] **Step 1: failing test:**

```rust
#[test]
fn jsx_child() {
    use TokenKind::*;
    // helper `advance_jsx(src) -> Vec<TokenKind>` loops advance_in_jsx_child to eof.
    assert_eq!(advance_jsx("hello{x}<a"), vec![jsx_text, l_brace, jsx_text, less, identifier, eof]);
    // adjust to the actual stream: "hello" jsx_text, "{" l_brace, then jsx child continues...
    // jsx text value decodes entities; raw keeps them.
    assert_eq!(jsx_text_value("a&amp;b{"), (b"a&b".to_vec(), b"a&amp;b".to_vec())); // (value, raw)
}
```
(Adjust to the actual token stream `advance_in_jsx_child` produces — it emits `l_brace`/`less` as
their own tokens and accumulates everything else as one `jsx_text` until `{`/`<`/EOF.)

- [ ] **Step 2:** FAIL. **Step 3: implement** `advance_in_jsx_child(&mut self) -> &Token` (port
  `JSLexer.cpp:749–809`): `{`→`l_brace`, `<`→`less`, EOF→`eof`, else accumulate `jsx_text` (TV in
  `tmp_storage`, TRV in `raw_storage`): UTF-8 re-encode into both; `&`→`consume_html_entity_optional`
  (append cp to TV, the consumed span to TRV); stop at `{`/`<`/EOF and `set_jsx_text(value, raw)`.
  `finish_token`. Extend `emit_fields`: `jsx_text` → ` value=Q(..) raw=Q(..)`.
- [ ] **Step 4:** PASS. **Step 5:** commit `rust(parser): port advanceInJSXChild + jsx_text dump`.

---

## Task 4: harness `--context=jsx` / `--jsx-child` + JSX differential

**Files:** `tools/js-lexer-dump/js-lexer-dump.cpp`, `rust/crates/parser/tests/differential.rs`.

- [ ] **Step 1:** Harness: add `--context=jsx` → `JSLexer::AllowJSXIdentifier`, and a `--jsx-child`
  flag that loops `advanceInJSXChild()` instead of `advance(ctx)` (until `eof`). Update usage/doc.
  Build + smoke test (`printf 'a&amp;b{x}' | js-lexer-dump --jsx-child -` → `jsx_text`, `l_brace`,
  `jsx_text`?, ...).
- [ ] **Step 2:** Differential: extend `run_differential`/`context_flag` for `AllowJSXIdentifier`
  → `--context=jsx`, and add a `differential_jsx_child` test that drives the Rust
  `advance_in_jsx_child` loop and the harness `--jsx-child` over a JSX-text corpus:

```rust
    // jsx-child corpus (driven with --jsx-child on both sides)
    "hello world{",
    "a&amp;b&lt;c{",                         // named entities
    "x&#65;&#x42;y<",                        // decimal + hex entities
    "text\u{4e2d}more{",                     // unicode jsx text -> WTF-8
    "line1\nline2<",                         // newlines in jsx text
```
  Also add a `--context=jsx` corpus (regular `advance` with JSX identifier mode) for `<div-foo>`-style
  identifiers with `-` and the JSX `>` token. VERIFY each against the oracle; keep byte-for-byte matches.
- [ ] **Step 3:** `cmake --build cmake-build-asan --target js-lexer-dump`; then
  `cargo test --manifest-path rust/Cargo.toml -p parser --test differential -- --nocapture` → all
  differentials (div/regexp/type/jsx/jsx-child) run and pass; counts shown.
- [ ] **Step 4:** full `cargo test -p parser` → all pass; zero warnings; `unsafe` only in `cursor.rs`.
- [ ] **Step 5:** commit `rust(parser): --context=jsx + --jsx-child differential`.

---

## Self-review checklist

- [ ] HTML entity table generated from `HTMLEntities.def` (254 rows, sorted, binary-search lookup).
- [ ] `consume_html_entity_optional` matches `JSLexer.cpp:811–907` (named/decimal/hex, overflow break,
  cursor reset on failure).
- [ ] JSX `scan_string` branches match (`&`-entities, raw newlines allowed, no `\`-escape under JSX).
- [ ] `advance_in_jsx_child` matches `:749–809` (TV/TRV, entity decoding, `{`/`<`/EOF stops).
- [ ] Harness `--context=jsx`/`--jsx-child`; the JSX + jsx-child differentials pass byte-for-byte.
- [ ] Deferred-and-noted: savepoint/lookahead/directives/rescanRBrace/magic comments (phase 4),
  `convertSurrogates` re-encoding.
- [ ] `unsafe` only in `cursor.rs`; zero warnings; all tests pass.

## Next
Phase 4: the stateful/parser-facing APIs — `SavePoint`, `lookahead1`/`lookahead2`,
`isCurrentTokenADirective`, `rescanRBraceInTemplateLiteral` (enables `template_middle`/`template_tail`),
`isLetFollowedByDeclStart`, `isUsing/AwaitUsingFollowedByIdentifier`, comment storage + magic comments
(`//# sourceURL=`/`sourceMappingURL=`), `convertSurrogates`, `seek`/`forceEOF`/token storage. See the roadmap.
```
