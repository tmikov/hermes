# JS Lexer Proper — Phase 2b: template literals

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Lex template literals starting with `` ` `` — `no_substitution_template` (`` `abc` ``) and `template_head` (`` `a${ ``) — with cooked (TV) + raw (TRV) values, the `NotEscapeSequence` → null-cooked rule, and CR→LF normalization. Validated byte-for-byte against `js-lexer-dump`. (`template_middle`/`template_tail` start at `}` via `rescanRBraceInTemplateLiteral`, which is parser-driven — phase 4; the harness can't emit them from a plain `advance` loop either.)

**Architecture:** Add `scan_template_literal` to `parser::lexer` and a `raw_storage: Vec<u8>` field (the TRV buffer, alongside `tmp_storage` for the TV). Port `JSLexer.cpp:2128–2381`. Wire the `` ` `` `advance` arm. Extend `emit_fields` with the four template kinds → `cooked=<Q(..)|null> raw=Q(..)`. Reuses `consume_hex`, `consume_unicode_escape_optional`, `append_unicode_to_storage` (both the `tmp_storage` and a passed-target form), `utf8::decode_utf8`, `match_unicode_line_terminator_offset1`.

**Tech Stack:** Rust 2021; `unsafe` only in `cursor.rs`.
**Reference spec:** `doc/superpowers/specs/2026-06-01-js-lexer-design.md`.
**C++ source of truth:** `lib/Parser/JSLexer.cpp:2128–2381` (`scanTemplateLiteral`), the `` ` `` `advance` arm (`:706–709`). Dump: `tools/js-lexer-dump/js-lexer-dump.cpp` (the four template kinds → `cooked=<Q|null> raw=Q`, with literal `null` when cooked is absent).

**Porting rule:** faithful port; copy comments. The TRV/`trv` (CR→LF), the dual-buffer (TV in `tmp_storage`, TRV in `raw_storage`), the `NotEscapeSequence` cases (`foundNotEscapeSequence` for octal-like `\1-9`, bad `\x`, bad `\u`) and the resulting **null cooked**, the `\u` optional escape (`consume_unicode_escape_optional` with raw append of the consumed span), and the supplementary-plane `raw_storage.pop_back()` + re-append must match exactly.

**Do NOT** `cd` out of the project root.

---

## Task 0: `scan_template_literal`

**Files:** `rust/crates/parser/src/lexer.rs`.

- [ ] **Step 1:** Add `raw_storage: Vec<u8>` to the `JSLexer` struct (init `Vec::new()` in `new()`).

- [ ] **Step 2: failing tests** (helper `template(src) -> (TokenKind, Option<Vec<u8>> /*cooked*/, Vec<u8> /*raw*/)`):

```rust
#[test]
fn templates_basic() {
    use TokenKind::*;
    // `abc` -> no_substitution_template, cooked="abc" raw="abc"
    assert_eq!(template("`abc`"), (no_substitution_template, Some(b"abc".to_vec()), b"abc".to_vec()));
    // `a${ -> template_head
    assert_eq!(template("`a${").0, template_head);
    // escapes: cooked has the cooked value, raw has the literal backslash sequence
    assert_eq!(template("`a\\nb`"), (no_substitution_template, Some(vec![b'a',10,b'b']), b"a\\nb".to_vec()));
    // NotEscapeSequence (\9) -> cooked is None, raw keeps it
    assert_eq!(template("`\\9`"), (no_substitution_template, None, b"\\9".to_vec()));
    // CR -> LF normalization in cooked AND raw
    assert_eq!(template("`a\rb`"), (no_substitution_template, Some(vec![b'a',10,b'b']), vec![b'a',10,b'b']));
    // kind sequence: `a${ b } -> template_head, identifier, r_brace (rescan is parser-driven)
    assert_eq!(kinds("`a${b}`"), vec![template_head, identifier, r_brace, /* trailing ` is its own scan */ ..]); // adjust to actual
}
```
(Adjust the last case to the actual token stream a plain `advance` loop produces: `` `a${ `` →
template_head, `b` → identifier, `}` → r_brace, then `` ` `` → starts a NEW template scan that hits
EOF/non-terminated — so use a cleaner case like `` `a${b}` `` only up to r_brace, or test
`` `head${ `` and `` `whole` `` separately. Keep the corpus to head + no-substitution forms.)

- [ ] **Step 3:** run → FAIL.
- [ ] **Step 4: implement** `scan_template_literal` by porting `JSLexer.cpp:2128–2381` faithfully:
  the `is_head`/`is_tail` flags (start `` ` `` vs `}`), `tmp_storage`(TV)+`raw_storage`(TRV) cleared,
  the `trv` CR→LF closure, the main loop (`` ` `` → tail+break; `${` → not-tail+break; `\` escape
  switch with raw-append-of-`trv(c)` first; the EOF non-terminated error + note; raw `\r`(+CRLF)
  CR→LF in both buffers; the UTF-8 re-encode into both buffers with the `raw_storage.pop_back()`
  fix for supplementary planes; plain byte into both), then cooked = `None` if
  `found_not_escape_sequence` else intern(`tmp_storage`), raw = intern(`raw_storage`), and
  `token.set_template_literal(kind, cooked, raw)` selecting the kind from is_head×is_tail
  (`` ` ``+tail→no_substitution_template; `` ` ``+not-tail→template_head; `}`+tail→template_tail;
  `}`+not-tail→template_middle). Wire the `` ` `` `advance` arm → `scan_template_literal` (replace
  the stub). The `}`-start path exists in the function but is only reached via
  `rescanRBraceInTemplateLiteral` (phase 4) — do not wire a `}` arm here.
  NOTE: `convert_surrogates` off by default — intern directly (`debug_assert!(!self.convert_surrogates)`).
- [ ] **Step 5:** run → PASS. **Step 6:** commit `rust(parser): port scanTemplateLiteral (template head / no-substitution)`.

---

## Task 1: dump template fields + differential

**Files:** `rust/crates/parser/src/lexer.rs`, `rust/crates/parser/tests/differential.rs`.

- [ ] **Step 1:** Extend `emit_fields` for `no_substitution_template`/`template_head`/
  `template_middle`/`template_tail`: ` cooked=` + (the literal `null` if cooked is None, else
  `Q(strtab.bytes(cooked_atom))`) + ` raw=` + `Q(strtab.bytes(raw_atom))`. (Match the harness; the
  `Token` stores cooked as `Option<AtomBytes>` — `getTemplateValue()==null` ⇒ `null`.)
- [ ] **Step 2:** Extend the differential corpus (still `--context=div`) with template forms that a
  plain `advance` loop lexes cleanly — i.e. `no_substitution_template` and `template_head`:

```rust
    "`hello` `a b c`",                      // no_substitution_template
    "`a${",                                  // template_head (then EOF)
    "`x${ `y${ `done`",                      // multiple heads + a no-substitution at the end
    "`tab\\tnl\\n` `raw\\u00e9`",            // escapes: cooked vs raw differ
    "`not\\9esc`",                           // NotEscapeSequence -> cooked=null
    "`cr\rlf`",                              // CR -> LF in cooked+raw
    "`uni\u{4e2d}` `astral\u{1f600}`",       // raw unicode -> WTF-8 in cooked+raw
```
(For `` `a${b}` `` style, the `}` would start a new template scan in a plain loop, which is the
parser's job to avoid — so keep corpus entries that end in `` ` `` (no-substitution) or `${`
(head). Verify each against the oracle; drop any whose stdout diverges.)

- [ ] **Step 3:** `cmake --build cmake-build-asan --target js-lexer-dump`; then
  `cargo test --manifest-path rust/Cargo.toml -p parser --test differential -- --nocapture` →
  runs, passes; compared-count up.
- [ ] **Step 4:** full `cargo test -p parser` → all pass; zero warnings; `unsafe` only in `cursor.rs`.
- [ ] **Step 5:** commit `rust(parser): dump template cooked=/raw= fields + template differential`.

---

## Self-review checklist

- [ ] `scan_template_literal` matches `JSLexer.cpp:2128–2381`: dual TV/TRV buffers, `trv` CR→LF,
  the escape switch, the NotEscapeSequence cases → null cooked, the `\u` optional escape with raw
  span append, the supplementary-plane `raw_storage.pop_back()` re-append, the non-terminated error.
- [ ] The four template kinds are selected correctly from is_head×is_tail.
- [ ] Rust dump `cooked=<Q|null> raw=Q` equals `js-lexer-dump` byte-for-byte (incl. `null` cooked
  and WTF-8 in both buffers).
- [ ] Deferred-and-noted: `template_middle`/`template_tail` (need `rescanRBrace`, phase 4); JSX/Flow
  (phase 3); regexp (2c); `convertSurrogates` re-encoding.
- [ ] `unsafe` only in `cursor.rs`; zero warnings; all tests pass.

## Next
Phase 2c: regexp — port `scanRegExp` (`JSLexer.cpp:2384+`) and wire the `AllowRegExp` `/` arm,
enabling `--context=regexp` differential cases (regexp literals + the `/`-as-regex-vs-div
decision). See the roadmap.
```
