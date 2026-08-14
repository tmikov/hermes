# JS Lexer Proper — Phase 3a: Flow `Type` grammar context

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Implement the Flow `Type` grammar-context behavior in `advance` — the `{|`/`|}` (`l_bracepipe`/`piper_brace`) tokens, `%checks`, the `@`-prefixed Flow identifiers, and the Type-context tweaks (`<`→`less`, `>`→`greater`, `??` not formed, `|}`) — and validate via a new `--context=type` differential.

**Architecture:** Add the `GrammarContext::Type`-gated branches to the `advance` punctuator arms in `parser::lexer/mod.rs` (1a omitted them since the differential was div-only). Map `Type`→`IdentifierMode::Flow` in `scan_identifier_*_in_context`. Extend the C++ `js-lexer-dump` harness with `--context=type` (→ `JSLexer::Type`), and add a `--context=type` differential corpus.

**Tech Stack:** Rust 2021; `unsafe` only in `cursor.rs`. C++ harness change (`tools/js-lexer-dump/`).
**Reference spec:** `doc/superpowers/specs/2026-06-01-js-lexer-design.md`.
**C++ source of truth:** `lib/Parser/JSLexer.cpp` `advance` Type-gated arms — `{` (`:340–348`, `{|`→`l_bracepipe`), `|` (`:398–422`, `|}`→`piper_brace`), `?` (`:434–436`, `??` only when `!= Type`), `%` (`:476–489`, `%checks`→identifier), `<` (`:579–582`, Type→`less`), `>` (`:603–610`, Type/JSX→`greater`), `@` (`:672–681`, Type→Flow identifier). Identifier Flow mode: `consumeOneIdentifierPartNoEscape` (`Mode==Flow && ch=='@'`), `scanIdentifierFastPath` (`:1897–1900`). The `as` IDENT_OP is parser-driven (`convertCurTokenToIdentOp`) — no advance change.

**Porting rule:** faithful port; copy comments. These arms are gated `if HERMES_PARSE_FLOW && grammar_context == Type` — in Rust they're plain `if grammar_context == GrammarContext::Type` branches (Flow is always compiled in).

**Do NOT** `cd` out of the project root.

---

## Task 0: harness `--context=type`

**Files:** `tools/js-lexer-dump/js-lexer-dump.cpp`.

- [ ] **Step 1:** Extend the `--context=` parsing to accept `type` → `JSLexer::Type` (alongside the
  existing `regexp`/`div`). Update the usage string + the format-doc comment.
- [ ] **Step 2:** Build: `cmake --build cmake-build-asan --target js-lexer-dump`. Smoke:
  `printf '{| a |}' | cmake-build-asan/bin/js-lexer-dump --context=type -` → emits `l_bracepipe`,
  `identifier`, `piper_brace`, `eof`.
- [ ] **Step 3:** commit `tools(js-lexer-dump): add --context=type for Flow type lexing`.

---

## Task 1: Type-context `advance` arms

**Files:** `rust/crates/parser/src/lexer/mod.rs` (+ `identifier.rs` if the Flow mode wiring lives there).

- [ ] **Step 1: failing tests** (helper `kinds_ctx(src, GrammarContext::Type)`):

```rust
#[test]
fn flow_type_context() {
    use TokenKind::*;
    assert_eq!(kinds_ctx("{|", GrammarContext::Type), vec![l_bracepipe, eof]);
    assert_eq!(kinds_ctx("|}", GrammarContext::Type), vec![piper_brace, eof]);
    assert_eq!(kinds_ctx("{ }", GrammarContext::Type), vec![l_brace, r_brace, eof]); // plain still works
    assert_eq!(kinds_ctx("<", GrammarContext::Type), vec![less, eof]);               // not lessless etc.
    assert_eq!(kinds_ctx(">>", GrammarContext::Type), vec![greater, greater, eof]);  // >> as two >
    assert_eq!(kinds_ctx("??", GrammarContext::Type), vec![question, question, eof]); // not questionquestion
    assert_eq!(kinds_ctx("%checks", GrammarContext::Type), vec![identifier, eof]);
    assert_eq!(kinds_ctx("@foo", GrammarContext::Type), vec![identifier, eof]);       // @-prefixed Flow ident
    // outside Type, these behave normally:
    assert_eq!(kinds_ctx("{|", GrammarContext::AllowDiv), vec![l_brace, pipe, eof]);
    assert_eq!(kinds_ctx("@foo", GrammarContext::AllowDiv), vec![at, identifier, eof]);
}
```

- [ ] **Step 2:** run → FAIL.
- [ ] **Step 3: implement** the Type-gated branches in `advance` (port the C++ arms cited above):
  - `{`: if `Type` and `peek_at(1)=='|'` → `l_bracepipe` (advance 2); else `l_brace`.
  - `|`: if `Type` and `peek_at(1)=='}'` → `piper_brace` (advance 2); else the existing `||`/`|=`/`|` logic.
  - `?`: the `??`/`??=` formation only when `!= Type` (in Type, `?` is its own token; `?.` still applies).
  - `%`: if `Type` and the next 7 bytes are `%checks` → `identifier` with the interned `%checks`
    (intern `b"%checks"`, advance 7); else the existing `%=`/`%` logic.
  - `<`: if `Type` → `less` (advance 1); else the existing `<=`/`<<`/`<<=`/`<` logic.
  - `>`: if `Type` (or `AllowJSXIdentifier`, but JSX is 3b) → `greater` (advance 1); else the
    existing `>=`/`>>`/`>>>`/`>>=`/`>>>=`/`>` logic.
  - `@`: if `Type` → `scan_identifier_fast_path_in_context` (Flow mode, the `@` is part of the
    identifier); else `at`.
  - Wire `scan_identifier_fast_path_in_context`/`scan_identifier_parts_in_context` to choose
    `IdentifierMode::Flow` when `grammar_context == Type` (mirror `JSLexer.h`'s
    `scanIdentifierFastPathInContext`). Confirm `consume_one_identifier_part_no_escape` already
    handles `Mode::Flow && ch=='@'` (from 1b-i) and that `scan_identifier_fast_path`'s ASCII loop
    accepts `@` in Flow mode (`:1900`).
- [ ] **Step 4:** run → PASS. **Step 5:** commit `rust(parser): Flow Type-context advance arms ({|, |}, %checks, @, <, >, ?)`.

---

## Task 2: `--context=type` differential

**Files:** `rust/crates/parser/tests/differential.rs`.

- [ ] **Step 1:** Add a `differential_type` test driven with `GrammarContext::Type` / `--context=type`:

```rust
    "{| a: number |} | string",
    "<T> >> << ?? %checks",
    "@flow @decorator a b",
    "Array<string> Map<K, V>",                 // generics: < > as individual tokens
    "x | y & z",                                // unions/intersections
    "{ a: 1 } [1, 2]",                          // plain punctuators still work in Type
```
  (Verify each against `js-lexer-dump --context=type -`; keep byte-for-byte matches.)
- [ ] **Step 2:** `cargo test --manifest-path rust/Cargo.toml -p parser --test differential -- --nocapture`
  → div/regexp/type differentials all run (not skipped) and pass; type compared-count shown.
- [ ] **Step 3:** full `cargo test -p parser` → all pass; zero warnings; `unsafe` only in `cursor.rs`.
- [ ] **Step 4:** commit `rust(parser): --context=type Flow differential`.

---

## Self-review checklist

- [ ] The Type-context arms match the C++ Flow-gated branches; outside Type the tokens are unchanged.
- [ ] `@`-prefixed Flow identifiers lex correctly (Flow mode); `%checks` interns as identifier.
- [ ] Harness `--context=type` maps to `JSLexer::Type`; the Type differential passes byte-for-byte.
- [ ] Deferred-and-noted: JSX (3b); savepoint/lookahead/directives/rescanRBrace/magic comments (phase 4).
- [ ] `unsafe` only in `cursor.rs`; zero warnings; all tests pass.

## Next
Phase 3b: JSX — `advanceInJSXChild`, `consumeHTMLEntityOptional` + `HTMLEntities.def`, JSX identifier
mode (`-`), JSX string `&`-entities + newline-in-string, the JSX `>` arm; extend the harness with
`--context=jsx` and a JSX-child mode. Then phase 4. See the roadmap.
```
