# JS Lexer Proper — Phase 1b-ii: numbers (`scanNumber`)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Lex numeric literals — decimal, hex/octal/binary (`0x`/`0o`/`0b`), legacy octal, fractions, exponents, numeric separators, and BigInt (`n`) — by porting `scanNumber` and wiring the already-built `parser::number` primitives, with `bits=`/bigint dump fields and a numeric differential corpus.

**Architecture:** Add `scan_number` to `parser::lexer`, a faithful port of `JSLexer.cpp:1573–1856`. The decimal/real path calls `parser::number::str_to_double`; the integer-radix paths call `parser::number::parse_int_with_radix`; BigInt digit validation uses a `parse_int_with_radix_digits` call. Wire the `advance` digit arms (`0-9` and `.NNN`) that 1a/1b-i stubbed. Extend `dump_token` with `bits=` (numeric) and `value=`/`raw=` (bigint), reusing `quote_bytes`.

**Tech Stack:** Rust 2021; `unsafe` stays only in `cursor.rs`. Uses `parser::number`, `parser::token_kinds`, `atom_table`, `unicode`.

**Reference spec:** `doc/superpowers/specs/2026-06-01-js-lexer-design.md`; memory `lexer-number-parsing-fast-float`.
**C++ source of truth:** `lib/Parser/JSLexer.cpp:1573–1856` (`scanNumber`). It uses `consumeIdentifierStart`/`consumeIdentifierParts` (done in 1b-i) for the trailing-identifier/`n` check, `parseIntWithRadixDigits`/`parseIntWithRadix` and `fastStrToDouble` (→ `parser::number`), `tmpStorage_`, `sm_.warning`, `errorRange`, and `Token::setNumericLiteral`/`setBigIntLiteral`. Dump format: `tools/js-lexer-dump/js-lexer-dump.cpp` (`numeric_literal` → `bits=0x%016llx` of `DoubleToBits`; `bigint_literal` → `value=Q(...) raw=Q(...)`).

**Porting rule:** faithful port; copy comments. `scan_number` is an intricate state machine (radix detect → integer digits → fraction → exponent → trailing-identifier/BigInt → value computation incl. the ≤9-digit decimal fast path, the legacy-octal `8`/`9` redetection + warning, the separator-placement validation, and the strict-mode octal errors). Transcribe it branch-for-branch.

**Do NOT** `cd` out of the project root.

---

## Task 0: `scan_number` core + decimal/hex/oct/bin/fraction/exponent

**Files:** `rust/crates/parser/src/lexer.rs`.

- [ ] **Step 1: failing test** (through `advance`; helper `numbers(src) -> Vec<(TokenKind, u64 /*bits*/)>` or compare via the differential later — for the unit test, expose the lexed numeric value):

```rust
#[test]
fn numbers_basic() {
    use TokenKind::*;
    // helper `num_bits(src)` lexes one numeric_literal and returns its f64 bits.
    assert_eq!(num_bits("5"), 5.0f64.to_bits());
    assert_eq!(num_bits("0.1"), 0.1f64.to_bits());
    assert_eq!(num_bits("0xff"), 255.0f64.to_bits());
    assert_eq!(num_bits("0o17"), 15.0f64.to_bits());
    assert_eq!(num_bits("0b1010"), 10.0f64.to_bits());
    assert_eq!(num_bits("1e10"), 1e10f64.to_bits());
    assert_eq!(num_bits("1_000"), 1000.0f64.to_bits());
    assert_eq!(num_bits(".5"), 0.5f64.to_bits());
    assert_eq!(num_bits("3.14e2"), 314.0f64.to_bits());
    // kind check
    assert_eq!(kinds("5 0xff 1.5"), vec![numeric_literal, numeric_literal, numeric_literal, eof]);
}
```

- [ ] **Step 2:** run → FAIL.
- [ ] **Step 3: implement `scan_number(grammar_context)`** by porting `JSLexer.cpp:1573–1856`
  faithfully (cursor-based; build the cleaned buffer for the decimal path and call
  `number::str_to_double`; integer radix → `number::parse_int_with_radix`; the ≤9-digit decimal
  fast path inline; legacy-octal `updateLegacyOctalRadix` warning; separator-placement checks;
  strict-mode octal errors; BigInt detection using `consume_identifier_start`/`parts` then
  `number::parse_int_with_radix_digits` to validate digits, building the normalized value (drop
  `_`, drop trailing `n`) and raw string, interning both via `strtab.atom_bytes`, and
  `token.set_bigint_literal(value, raw)`). Wire the `advance` arms: the `0-9` digit arm and the
  `.` arm's `[1] in 0..9` sub-case → `scan_number` (replace the 1b stubs). Keep the `.`/`...`/
  `period` punctuator sub-cases as in 1a.
- [ ] **Step 4:** run → PASS. **Step 5:** commit `rust(parser): port scanNumber (decimal/radix/fraction/exponent/bigint)`.

---

## Task 1: dump `bits=` / bigint fields + numeric differential

**Files:** `rust/crates/parser/src/lexer.rs`, `rust/crates/parser/tests/differential.rs`.

- [ ] **Step 1:** Extend `emit_fields`: `numeric_literal` → ` bits=0x` + the 16-digit lowercase
  hex of `token.numeric().to_bits()` (matching the harness `snprintf("0x%016llx", DoubleToBits)`);
  `bigint_literal` → ` value=` + `quote_bytes(strtab.bytes(value_atom))` + ` raw=` +
  `quote_bytes(strtab.bytes(raw_atom))`.
- [ ] **Step 2:** Extend the differential corpus with **valid** numeric forms (still
  `--context=div`; keep error/NaN-producing literals OUT — errors go to stderr and a NaN bit
  pattern could differ; the differential compares stdout token streams):

```rust
    "0 1 42 1000000000 9007199254740993",
    "0.1 .5 3.14159 1e10 2E-3 6.022e23 1_000_000",
    "0xff 0xDEAD_BEEF 0o17 0b1010 0XAB 0O7 0B11",
    "10n 0xffn 255n 0n",
    "1 .5 .25",                  // '.' number vs period
    "a.5",                        // ident '.' number? -> a . numeric? (use punct-safe: '0 .5')
```
(Adjust any entry that would lex an identifier the harness can't compare to a 1b-i identifier —
1b-i identifiers ARE lexed now, so mixed ident+number corpus is fine; but avoid strings/regexp.
Replace the `a.5` example with `0 .5` etc.)

- [ ] **Step 3:** `cmake --build cmake-build-asan --target js-lexer-dump`; then
  `cargo test --manifest-path rust/Cargo.toml -p parser --test differential -- --nocapture` →
  runs (not skipped), passes byte-for-byte, compared-count increased.
- [ ] **Step 4:** full `cargo test -p parser` → all pass; zero warnings; `unsafe` only in `cursor.rs`.
- [ ] **Step 5:** commit `rust(parser): dump numeric bits= / bigint fields + numeric differential`.

---

## Self-review checklist

- [ ] `scan_number` matches `JSLexer.cpp:1573–1856` branch-for-branch: radix detection, the
  ≤9-digit decimal fast path, fraction/exponent, legacy-octal `8`/`9` redetection + warning,
  separator placement, strict-mode octal errors, BigInt path (normalized value + raw, interned).
- [ ] Decimal/real values are bit-identical to the harness (`str_to_double` == fast_float);
  hex/oct/bin via `parse_int_with_radix`; the rounding path covered by `parser::number`'s tests.
- [ ] `bits=` is 16-digit lowercase hex of `f64::to_bits`; bigint `value=`/`raw=` match.
- [ ] Numeric differential corpus runs (not skipped) and passes byte-for-byte; the
  compared-entry count went up.
- [ ] Deferred-and-noted: strings/templates/regexp/private-id (phase 2), JSX/Flow (phase 3),
  savepoint/lookahead (phase 4). Error/NaN-producing numeric literals kept out of the
  differential corpus (stderr-only diagnostics).
- [ ] `unsafe` only in `cursor.rs`; zero warnings; all tests pass.

## Next
Phase 2: string/template/regexp/bigint... → string literals (`scanString`, octal/hex/unicode
escapes, `convertSurrogates`), template literals (`scanTemplateLiteral` + `rescanRBrace`),
regexp (`scanRegExp` + the `AllowRegExp` `/` arm), private identifiers (`scanPrivateIdentifier`,
the `#` arm) — extending the harness to emit those (it already does) and adding
`--context=regexp` differential cases. See the roadmap.
```
