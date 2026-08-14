# JS Lexer — Number Parsing (subsystem ⑤) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the pure numeric-conversion primitives the lexer's `scanNumber` relies on — `parseIntWithRadixDigits`/`parseIntWithRadix` (integer radix parsing incl. the >2^53 power-of-2 rounding path) and a pure-Rust `str_to_double` (the `fastStrToDouble` equivalent for the decimal path) — into `parser/src/number.rs`, with **no FFI and no third-party crate**.

**Architecture:** A new `number` module in the existing `parser` crate (zero `unsafe`). Integer parsing is a faithful line-by-line port of `include/hermes/Support/Conversions.h:166–330`. The decimal/real path uses Rust std's `str::parse::<f64>()`, which **is** the same correctly-rounded algorithm (Eisel–Lemire / `fast_float`) the lexer's `fastStrToDouble` uses (`lib/Support/FastStrToDouble.cpp` → `fast_float::from_chars`), so results are bit-identical on the well-formed buffers the lexer produces. The full `scanNumber` state machine (cursor walking, radix detection, separator-placement errors, legacy-octal warnings, bigint token construction) is **lexer-proper** code that will *use* these primitives — it is not part of this subsystem.

**Tech Stack:** Rust (edition 2021), std only, zero `unsafe`.

**Reference spec:** `doc/superpowers/specs/2026-06-01-js-lexer-design.md` (decision 4) and the memory `lexer-number-parsing-fast-float` (lexer uses `fast_float`, not `dtoa`; Rust std matches it bit-for-bit → pure Rust, no FFI).
**C++ source of truth:**
- `include/hermes/Support/Conversions.h:160–330` — `charLetterToLower`, `parseIntWithRadixDigits`, `parseIntWithRadix` (PORT THESE FAITHFULLY, including the power-of-2 high-precision path lines 222–328).
- `lib/Support/FastStrToDouble.cpp` — confirms the decimal path is `fast_float` with `chars_format::general | allow_leading_plus`; the Rust equivalent is `str::parse::<f64>()`.
- `lib/Parser/JSLexer.cpp:1573–1856` (`scanNumber`) — for context on how these are called (the lexer pre-builds a clean buffer: strips `_`, no leading `0x`/`0o`/`0b`; for the decimal path the buffer holds only `[0-9.eE+-]`). DO NOT port `scanNumber` here.

**Porting rule:** keep structure close to the C++ and copy comments. The `parseIntWithRadix` body is intricate (a bit-by-bit mantissa/exponent reconstruction with rounding); transcribe it faithfully from the C++ rather than reinventing it.

**Do NOT** `cd` out of the project root.

---

## Key porting gotchas

- C++ uses `int radix`. In the digit checks, `'a' + radix - 10` underflows for `radix < 10`
  if computed in unsigned. Use `i32` arithmetic for those comparisons so letters are
  correctly rejected for radix ≤ 10 (matching C++ `int`).
- `parseIntWithRadixDigits` takes a digit callback; faithfully reproduce the separator
  validation: `_` is invalid at the first or last position, and two consecutive `_` are
  invalid (checked by looking at the next byte).
- `parseIntWithRadix`: first accumulates `result` as a plain `f64` (`result = result*radix + d`).
  Then, **only when `result >= 2^53` AND radix is a power of two (2/4/8/16/32)**, it discards
  that and re-runs a bit-by-bit reconstruction with explicit round-to-nearest-even
  (the `Mode` state machine, lines 228–327). Port the `Mode` enum and both loops exactly.
- `str_to_double` must mirror `fastStrToDouble`'s "consume the whole buffer or it's invalid"
  contract: Rust `str::parse::<f64>()` is all-or-nothing (returns `Err` on any trailing
  garbage), which matches; out-of-range parses to `Ok(inf)`/`Ok(0.0)` in both, which also
  matches. The lexer hands a `&[u8]` that is pure ASCII (`[0-9.eE+-]`), so converting via
  `std::str::from_utf8` is safe.

---

## File structure

```
rust/crates/parser/
  src/
    lib.rs        # add: pub mod number;
    number.rs     # parse_int_with_radix_digits, parse_int_with_radix, str_to_double
```

---

## Task 0: Module scaffold

**Files:** Modify `rust/crates/parser/src/lib.rs`; create `rust/crates/parser/src/number.rs`.

- [ ] **Step 1:** In `rust/crates/parser/src/lib.rs`, add `pub mod number;` (after
  `pub mod token_kinds;`).
- [ ] **Step 2:** Create `rust/crates/parser/src/number.rs`:

```rust
//! Numeric-literal conversion primitives for the JS lexer, ported from
//! include/hermes/Support/Conversions.h. The decimal/real path uses Rust std's
//! correctly-rounded `str::parse::<f64>()` (the same fast_float algorithm the
//! C++ lexer uses) — no FFI, no third-party crate.
```

- [ ] **Step 3:** Build: `cargo build --manifest-path rust/Cargo.toml -p parser` → clean.
- [ ] **Step 4:** Commit:

```bash
git add rust/crates/parser/src/lib.rs rust/crates/parser/src/number.rs
git commit -m "rust(parser): scaffold number module"
```

---

## Task 1: `parse_int_with_radix_digits` + `parse_int_with_radix`

**Files:** Modify `rust/crates/parser/src/number.rs`.

Faithful port of `Conversions.h:160–330`.

- [ ] **Step 1: Write the failing tests** (append to `number.rs`):

```rust
#[cfg(test)]
mod int_tests {
    use super::*;

    #[test]
    fn small_exact() {
        assert_eq!(parse_int_with_radix(b"ff", 16, true), Some(255.0));
        assert_eq!(parse_int_with_radix(b"777", 8, true), Some(511.0));
        assert_eq!(parse_int_with_radix(b"1010", 2, true), Some(10.0));
        assert_eq!(parse_int_with_radix(b"123", 10, true), Some(123.0));
        assert_eq!(parse_int_with_radix(b"z", 36, true), Some(35.0));
        // Letters are rejected for radix <= 10 (no u32 underflow on radix-10).
        assert_eq!(parse_int_with_radix(b"a", 10, true), None);
        assert_eq!(parse_int_with_radix(b"8", 8, true), None);
    }

    #[test]
    fn separators() {
        assert_eq!(parse_int_with_radix(b"1_000", 10, true), Some(1000.0));
        assert_eq!(parse_int_with_radix(b"dead_beef", 16, true), Some(0xdeadbeef as f64));
        assert_eq!(parse_int_with_radix(b"_1", 10, true), None);   // leading
        assert_eq!(parse_int_with_radix(b"1_", 10, true), None);   // trailing
        assert_eq!(parse_int_with_radix(b"1__2", 10, true), None); // double
        // When separators are disallowed, '_' is just an invalid digit.
        assert_eq!(parse_int_with_radix(b"1_0", 10, false), None);
    }

    #[test]
    fn invalid() {
        assert_eq!(parse_int_with_radix(b"xyz", 16, true), None);
        assert_eq!(parse_int_with_radix(b"12.3", 10, true), None);
    }

    // The power-of-2 high-precision path (result >= 2^53) must produce the
    // correctly-rounded f64. Rust's `u128 as f64` is round-to-nearest-even, an
    // independent correctly-rounded oracle for any value that fits in u128.
    #[test]
    fn large_power_of_two_rounding_matches_u128_oracle() {
        let cases: &[(&[u8], u32)] = &[
            (b"20000000000001", 16),               // 2^53 + 1 region
            (b"1fffffffffffff", 16),               // 2^53 - 1 (exact)
            (b"ffffffffffffffff", 16),             // u64::MAX
            (b"123456789abcdef0123", 16),          // > 2^64, still < 2^128
            (b"777777777777777777777", 8),         // large octal
            (b"1111111111111111111111111111111111111111111111111111111", 2),
        ];
        for &(s, radix) in cases {
            let txt = std::str::from_utf8(s).unwrap();
            let expected = u128::from_str_radix(txt, radix).unwrap() as f64;
            assert_eq!(
                parse_int_with_radix(s, radix, true),
                Some(expected),
                "mismatch for {txt} radix {radix}"
            );
        }
    }

    // Non-power-of-2 large values use the plain f64 accumulation (no special path);
    // for moderately large decimals that still round-trip, confirm equality with the
    // naive accumulation result.
    #[test]
    fn large_decimal() {
        assert_eq!(parse_int_with_radix(b"9007199254740993", 10, true),
                   Some(9007199254740993u128 as f64));
    }
}
```

- [ ] **Step 2: Run — expect FAIL:**
  `cargo test --manifest-path rust/Cargo.toml -p parser -- int_tests` → FAIL (undefined).

- [ ] **Step 3: Implement** by reading `Conversions.h:160–330` and porting faithfully. The
  Rust signatures:

```rust
/// Lowercase an ASCII letter via the C++ `charLetterToLower` trick.
#[inline]
fn char_letter_to_lower(c: u8) -> u8 {
    c | 32
}

/// Parse `bytes` (non-empty, no leading "0x" etc.) in `radix`, invoking `digit`
/// with each digit value left to right. `allow_sep` permits '_' separators
/// between digits. Returns false on any invalid input.
/// Port of `parseIntWithRadixDigits` (Conversions.h).
pub fn parse_int_with_radix_digits(
    bytes: &[u8],
    radix: u32,
    allow_sep: bool,
    mut digit: impl FnMut(u8),
) -> bool {
    debug_assert!((2..=36).contains(&radix), "Invalid radix");
    debug_assert!(!bytes.is_empty(), "Empty string");
    let radix = radix as i32;
    for (i, &c) in bytes.iter().enumerate() {
        let c_low = char_letter_to_lower(c);
        if c >= b'0' && c <= b'9' && (c as i32) < b'0' as i32 + radix {
            digit(c - b'0');
        } else if c_low >= b'a' && (c_low as i32) < b'a' as i32 + radix - 10 {
            digit(c_low - b'a' + 0xa);
        } else if allow_sep && c == b'_' {
            // '_' must be between two existing digits.
            if i == 0 || i == bytes.len() - 1 {
                return false;
            }
            // Previous char isn't '_' (else we'd have already returned). Check next.
            if bytes[i + 1] == b'_' {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}

/// Parse `bytes` (non-empty, no leading "0x" etc.) in `radix`. Returns the f64
/// result on success, or None on invalid input.
/// Port of `parseIntWithRadix` (Conversions.h), including the >2^53 power-of-two
/// bit-by-bit rounding path.
pub fn parse_int_with_radix(bytes: &[u8], radix: u32, allow_sep: bool) -> Option<f64> {
    // ... PORT Conversions.h:210–330 faithfully: first the plain accumulation
    // via parse_int_with_radix_digits with a closure doing result = result*radix + d;
    // then, if result >= 2^53 (MAX_MANTISSA = 9007199254740992.0) and radix is a
    // power of two, redo with the Mode state machine (LeadingZero/Mantissa/
    // ExpLowBit/ExpLeadingZero/Exponent) exactly as in the C++. Copy the comments.
}
```

Notes for the port:
- `MAX_MANTISSA = 9007199254740992.0` (2^53).
- "radix is a power of two" = `radix.is_power_of_two()` (radix ∈ {2,4,8,16,32}).
- The second pass re-iterates `bytes`, skipping `_` when `allow_sep`, reading each digit's
  bits MSB-first via `bitMask = radix >> 1` then `>>= 1`. Reproduce the `Mode` enum and the
  final `match curMode` rounding (`result += lowestExponentBit && lastMantissaBit` etc.).
- Translate `digitCallback(c - '0')` value extraction in the second pass the same way the
  C++ does (`curDigit` from the same digit decoding).

- [ ] **Step 4: Run — expect PASS:**
  `cargo test --manifest-path rust/Cargo.toml -p parser -- int_tests` → all pass (the
  `u128`-oracle test proves the rounding path is correctly rounded). Zero warnings.

- [ ] **Step 5: Commit:**

```bash
git add rust/crates/parser/src/number.rs
git commit -m "rust(parser): port parseIntWithRadix (incl. power-of-2 rounding path)"
```

---

## Task 2: `str_to_double` (decimal/real path)

**Files:** Modify `rust/crates/parser/src/number.rs`.

- [ ] **Step 1: Write the failing tests** (append to `number.rs`):

```rust
#[cfg(test)]
mod double_tests {
    use super::*;

    fn bits(v: f64) -> u64 {
        v.to_bits()
    }

    #[test]
    fn known_bit_patterns() {
        // These mirror the js-lexer-dump oracle's `bits=` output.
        assert_eq!(str_to_double(b"5").map(bits), Some(0x4014000000000000));
        assert_eq!(str_to_double(b"0.1").map(bits), Some(0x3fb999999999999a));
        assert_eq!(str_to_double(b"255").map(bits), Some(0x406fe00000000000));
        // (5, 0.1, 255 bit patterns were confirmed against the real C++ js-lexer-dump.)
        // These two cross-check delegation to the std parser:
        assert_eq!(str_to_double(b"1e10").map(bits), Some(1e10f64.to_bits()));
        assert_eq!(str_to_double(b"3.14159").map(bits), Some(3.14159f64.to_bits()));
    }

    #[test]
    fn must_consume_all() {
        assert_eq!(str_to_double(b"12x"), None);
        assert_eq!(str_to_double(b""), None);
        assert_eq!(str_to_double(b"1.2.3"), None);
    }

    #[test]
    fn leading_plus_and_exponent() {
        assert_eq!(str_to_double(b"+5").map(bits), Some(5.0f64.to_bits()));
        assert_eq!(str_to_double(b"5e+3").map(bits), Some(5000.0f64.to_bits()));
        assert_eq!(str_to_double(b"5E-3").map(bits), Some(0.005f64.to_bits()));
    }

    #[test]
    fn out_of_range() {
        // Overflow -> +inf; underflow -> 0.0 (matches fast_float ignoring out-of-range).
        assert_eq!(str_to_double(b"1e400"), Some(f64::INFINITY));
        assert_eq!(str_to_double(b"1e-400"), Some(0.0));
    }
}
```

- [ ] **Step 2: Run — expect FAIL:**
  `cargo test --manifest-path rust/Cargo.toml -p parser -- double_tests` → FAIL.

- [ ] **Step 3: Implement:**

```rust
/// Parse a cleaned decimal/real numeric buffer (only `[0-9.eE+-]`, separators
/// already stripped) to an f64. Returns the value if the WHOLE buffer parses,
/// or None on invalid input — mirroring `fastStrToDouble`'s "consume all or
/// fail" contract. Out-of-range inputs parse to +/-inf or 0.0 (as fast_float and
/// Rust std both do). Rust std's parser is the same correctly-rounded algorithm
/// as the lexer's `fast_float`, so results are bit-identical.
pub fn str_to_double(bytes: &[u8]) -> Option<f64> {
    // The buffer is pure ASCII; from_utf8 cannot fail, but handle defensively.
    let s = std::str::from_utf8(bytes).ok()?;
    s.parse::<f64>().ok()
}
```

- [ ] **Step 4: Run — expect PASS:**
  `cargo test --manifest-path rust/Cargo.toml -p parser` (all parser tests, incl. the
  earlier token-table ones) → pass. Zero warnings.

  If any `known_bit_patterns` case fails, that is a real fast_float-vs-Rust divergence to
  investigate (do NOT just change the expected bits) — capture it and report, since this is
  the fidelity-critical claim. (The listed cases are standard correctly-rounded values and
  should match.)

- [ ] **Step 5: Commit:**

```bash
git add rust/crates/parser/src/number.rs
git commit -m "rust(parser): pure-Rust str_to_double for the decimal path"
```

---

## Self-review checklist (after Task 2)

- [ ] `parse_int_with_radix_digits`/`parse_int_with_radix` are faithful ports of
  `Conversions.h:160–330`, comments copied; the power-of-2 `Mode` reconstruction is exact.
- [ ] The `u128`-oracle test passes for power-of-2 radixes above 2^53 (correct rounding).
- [ ] Radix ≤ 10 rejects letters (no unsigned underflow on `radix - 10`).
- [ ] Separator validation matches C++ (no leading/trailing/double `_`); `allow_sep=false`
  treats `_` as invalid.
- [ ] `str_to_double` is pure Rust (`str::parse::<f64>()`), consume-all semantics, no FFI /
  no crate; known bit patterns match the js-lexer-dump oracle's `bits=` output.
- [ ] Zero `unsafe`; zero warnings; all `parser` crate tests pass.

## Next

After this lands, all five lexer support-layer prerequisites are done (token tables, dump
harness, interner, Unicode, number parsing). The **lexer proper** follows — `cursor.rs`
(encapsulated `*const u8`), `token.rs`, then `advance`/identifiers/literals/templates/
regexp/JSX+Flow/savepoint+lookahead — validated live against the js-lexer-dump oracle. See
`doc/superpowers/RustPortRoadmap.md`.
```
