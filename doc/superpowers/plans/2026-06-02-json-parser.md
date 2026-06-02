# JSONParser → Rust Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port Hermes' `JSONParser` (value model + `JSONFactory` + recursive-descent parser), `JSONEmitter`, and `JSONSharedValue` to Rust as the first consumer of the completed Rust `JSLexer`, validated byte-for-byte against C++ and benchmarkable.

**Architecture:** Values live in a `bumpalo` arena owned by a `JSONFactory` that uniques strings/numbers and shares hidden classes; the parser drives `JSLexer` and reports through the lexer's `SourceErrorManager`. A faithful `JSONEmitter` port (in the `support` crate) backs both `emitInto` and a differential round-trip oracle. A C++ `json-parse-dump` tool and a Rust `json_parse_dump` bin emit identical canonical JSON for byte-for-byte corpus diffing, and share a `--bench=N` timing mode.

**Tech Stack:** Rust (workspace `rust/`, toolchain 1.96.0), `bumpalo` (new dep), the existing `support`/`parser`/`atom_table` crates; C++ tool via `add_hermes_tool`.

**Spec:** `doc/superpowers/specs/2026-06-02-json-parser-design.md`. Read it first.

**Conventions (from the lexer port — keep them):** faithful to the C++ (copy comments / keep them close, cite C++ line ranges), copyright header on every new file, commit per task on branch `rust` (never PR/merge), commit trailer `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`. Build/test with `--manifest-path rust/Cargo.toml`; **zero warnings** is the gate. Never `cd`.

**C++ sources of truth:**
- `include/hermes/Parser/JSONParser.h`, `lib/Parser/JSONParser.cpp`
- `include/hermes/Support/JSONEmitter.h`, `lib/Support/JSONEmitter.cpp`
- `lib/Support/Conversions.cpp:211-370` (`numberToString`) — the ECMAScript Number::toString
- `unittests/Parser/JSONParserTest.cpp` (5 cases), `unittests/Support/JSONEmitterTest.cpp` (11 cases)
- `tools/js-lexer-dump/` + `tools/CMakeLists.txt` (the oracle-tool pattern to mirror)

**Key already-built APIs (verified):**
- `JSLexer::new(buf_id: SourceId, sm: &'a mut SourceErrorManager, strtab: &'a AtomTable, ctx: GrammarContext)` and `new_with_convert_surrogates(.., convert_surrogates: bool)`. JSON uses `GrammarContext::AllowDiv`. `advance(&mut self, ctx) -> &Token`, `token(&self) -> &Token`, `get_source_mgr(&self) -> &SourceErrorManager` (needs a new `_mut`).
- `Token`: `kind() -> TokenKind`, `get_string_literal() -> AtomBytes`, `get_numeric_literal() -> f64`, `source_range() -> SMRange` (`SMRange { start: SMLoc, end: SMLoc }`).
- `TokenKind` variants: `string_literal`, `numeric_literal`, `minus`, `l_brace`, `r_brace`, `l_square`, `r_square`, `comma`, `colon`, `rw_true`, `rw_false`, `rw_null`.
- `AtomTable::new()`, `atom_bytes(impl Into<Vec<u8>>+AsRef<[u8]>) -> AtomBytes`, `bytes(AtomBytes) -> &[u8]`. `AtomBytes: Copy+Eq+Hash`.
- `SourceErrorManager`: `add_buffer(&str,&str)->SourceId`, `error_at(loc: SMLoc, range: Option<SMRange>, msg: impl Into<String>, sub: Subsystem)`, `error_count()->u32`. `Subsystem::Parser` (in `support::diag`).

---

## File structure

| File | Responsibility |
|------|----------------|
| `rust/crates/support/src/json_emitter.rs` (create) | `JSONEmitter` + `number_to_string` (port of `JSONEmitter.{h,cpp}` + `numberToString`) |
| `rust/crates/support/src/lib.rs` (modify) | add `pub mod json_emitter;` |
| `rust/crates/parser/Cargo.toml` (modify) | add `bumpalo` dep + `[[bin]] json-parse-dump` |
| `rust/crates/parser/src/lib.rs` (modify) | add `pub mod json;` |
| `rust/crates/parser/src/json/mod.rs` (create) | `JSONValue`, `JSONHiddenClass`, accessors, `emit_into`, `JSONSharedValue`; re-exports |
| `rust/crates/parser/src/json/factory.rs` (create) | `JSONFactory` (uniquing, hidden classes, `new_object`/`new_array`/`sort_props`) |
| `rust/crates/parser/src/json/parser.rs` (create) | `JSONParser` (recursive descent over `JSLexer`) |
| `rust/crates/parser/src/lexer/mod.rs` (modify) | add `get_source_mgr_mut(&mut self) -> &mut SourceErrorManager` |
| `rust/crates/parser/src/bin/json_parse_dump.rs` (create) | Rust differential/bench tool |
| `tools/json-parse-dump/json-parse-dump.cpp` + `CMakeLists.txt` (create) | C++ oracle/bench tool |
| `tools/CMakeLists.txt` (modify) | `add_subdirectory(json-parse-dump)` |
| `rust/crates/parser/tests/json_parser_ported.rs` (create) | the 5 ported `JSONParserTest` cases |
| `rust/crates/parser/tests/json_differential.rs` (create) | corpus byte-for-byte diff vs C++ |
| `rust/crates/parser/tests/json_corpus/*.json` (create) | differential corpus |
| `rust/crates/parser/tests/gen_big_json.rs` or `tools/gen-big-json` (create) | benchmark input generator |

---

## Phase A — JSONEmitter (support crate, zero `unsafe`)

### Task A1: `number_to_string` (ECMAScript Number::toString)

**Files:**
- Create: `rust/crates/support/src/json_emitter.rs`
- Modify: `rust/crates/support/src/lib.rs` (add `pub mod json_emitter;`)

- [ ] **Step 1: Write the failing tests.** Put at the bottom of `json_emitter.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_to_string_matches_ecmascript() {
        // Port-of-numberToString spot checks (lib/Support/Conversions.cpp:211).
        let cases: &[(f64, &str)] = &[
            (0.0, "0"),
            (-0.0, "0"),
            (1.0, "1"),
            (-1.0, "-1"),
            (456.7, "456.7"),
            (100.0, "100"),
            (0.1, "0.1"),
            (0.0001, "0.0001"),     // n=-3, fixed
            (1e-7, "1e-7"),         // n=-6, scientific
            (1e20, "100000000000000000000"), // n=21, fixed
            (1e21, "1e+21"),        // n=22, scientific
            (123.45, "123.45"),
            (5e-324, "5e-324"),     // min subnormal
            (f64::NAN, "NaN"),
            (f64::INFINITY, "Infinity"),
            (f64::NEG_INFINITY, "-Infinity"),
        ];
        for &(v, expected) in cases {
            assert_eq!(number_to_string(v), expected, "for {v:?}");
        }
    }
}
```

- [ ] **Step 2: Run to verify it fails.** Run: `cargo test --manifest-path rust/Cargo.toml -p support json_emitter`. Expected: FAIL (module/function not found).

- [ ] **Step 3: Implement.** Add the copyright header, then:

```rust
//! Port of `hermes::JSONEmitter` (include/hermes/Support/JSONEmitter.{h,cpp})
//! plus the `numberToString` helper it relies on (lib/Support/Conversions.cpp).

use std::fmt::Write;

/// Port of `hermes::numberToString` (lib/Support/Conversions.cpp:211) — the
/// ECMAScript Number::toString algorithm. Produces the shortest round-tripping
/// decimal. The C++ obtains the shortest (significand, exponent) via
/// `fastDoubleToDecimal`; Rust's `{:e}` formatting yields the same unique
/// shortest digits, so we extract digits + exponent from it and then apply the
/// identical fixed/scientific formatting rules (spec steps 6-12).
pub fn number_to_string(m: f64) -> String {
    // 1. NaN.
    if m.is_nan() {
        return "NaN".to_string();
    }
    // 2. +0 or -0 -> "0".
    if m == 0.0 {
        return "0".to_string();
    }
    // 4. +/- Infinity.
    if m == f64::INFINITY {
        return "Infinity".to_string();
    }
    if m == f64::NEG_INFINITY {
        return "-Infinity".to_string();
    }

    let mut out = String::new();
    if m < 0.0 {
        out.push('-');
    }

    // Shortest significand digits + decimal exponent from `{:e}`:
    //   456.7 -> "4.567e2", 1.0 -> "1e0", 1e21 -> "1e21".
    // Rust strips trailing zeros, matching `fastDoubleToDecimal` (significand
    // not divisible by 10).
    let sci = format!("{:e}", m.abs());
    let (mantissa, exp_str) = sci.split_once('e').expect("`{:e}` always has 'e'");
    let e: i32 = exp_str.parse().expect("valid exponent");
    let digits: Vec<u8> = mantissa.bytes().filter(|&b| b != b'.').collect();
    let k = digits.len() as i32;
    // value = digits_int * 10^(e-(k-1)); decimal-point position n = e + 1.
    let n = e + 1;

    if (-5..=21).contains(&n) {
        if n >= k {
            // k digits, then n-k zeros.
            for &d in &digits {
                out.push(d as char);
            }
            for _ in 0..(n - k) {
                out.push('0');
            }
        } else if n > 0 {
            // n digits, '.', remaining k-n digits.
            for i in 0..n {
                out.push(digits[i as usize] as char);
            }
            out.push('.');
            for i in n..k {
                out.push(digits[i as usize] as char);
            }
        } else {
            // "0.", -n zeros, then k digits.
            out.push('0');
            out.push('.');
            for _ in 0..(-n) {
                out.push('0');
            }
            for &d in &digits {
                out.push(d as char);
            }
        }
    } else {
        // Scientific notation, e.g. 1.2e+3.
        let exponent_sign = if n < 0 { '-' } else { '+' };
        let exp_val = (n - 1).unsigned_abs();
        out.push(digits[0] as char);
        if k != 1 {
            out.push('.');
            for i in 1..k {
                out.push(digits[i as usize] as char);
            }
        }
        out.push('e');
        out.push(exponent_sign);
        let _ = write!(out, "{exp_val}");
    }
    out
}
```

Add `pub mod json_emitter;` to `rust/crates/support/src/lib.rs` (alphabetical, after `manager` or wherever it sorts).

- [ ] **Step 4: Run to verify it passes.** Run: `cargo test --manifest-path rust/Cargo.toml -p support json_emitter`. Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add rust/crates/support/src/json_emitter.rs rust/crates/support/src/lib.rs
git commit -m "rust(support): port numberToString (ECMAScript Number::toString) for JSONEmitter

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task A2: `JSONEmitter` core (open/close, key, scalar values, escaping)

**Files:**
- Modify: `rust/crates/support/src/json_emitter.rs`
- Reference: `lib/Support/JSONEmitter.cpp`, `unittests/Support/JSONEmitterTest.cpp`

- [ ] **Step 1: Write the failing tests.** Port the non-pretty `JSONEmitterTest` cases. Add to the `tests` module:

```rust
    fn emit<F: FnOnce(&mut JSONEmitter)>(f: F) -> String {
        let mut s = String::new();
        {
            let mut j = JSONEmitter::new(&mut s, false);
            f(&mut j);
        }
        s
    }

    #[test]
    fn empty_array() {
        assert_eq!(emit(|j| { j.open_array(); j.close_array(); }), "[]");
    }

    #[test]
    fn empty_dict() {
        assert_eq!(emit(|j| { j.open_dict(); j.close_dict(); }), "{}");
    }

    #[test]
    fn sample() {
        // unittests/Support/JSONEmitterTest.cpp: Sample
        let s = emit(|j| {
            j.open_dict();
            j.emit_key("name"); j.emit_str("hermes");
            j.emit_key("age"); j.emit_i64(2);
            j.emit_key("hot"); j.emit_bool(true);
            j.emit_key("cold"); j.emit_bool(false);
            j.emit_key("tags");
            j.open_array();
            j.emit_str("small"); j.emit_str("light");
            j.close_array();
            j.close_dict();
        });
        assert_eq!(s, r#"{"name":"hermes","age":2,"hot":true,"cold":false,"tags":["small","light"]}"#);
    }

    #[test]
    fn smoke_with_double_and_escapes() {
        // unittests/Support/JSONEmitterTest.cpp: SmokeTest
        let s = emit(|j| {
            j.open_dict();
            j.emit_key("a"); j.emit_i64(123);
            j.emit_key("b"); j.emit_f64(456.7);
            j.emit_key("dict1");
            j.open_dict();
            j.emit_key("dict1_arr1");
            j.open_array();
            j.emit_str("val1"); j.emit_str("val2"); j.emit_str("val3");
            j.close_array();
            j.emit_key("dict1_empty"); j.open_dict(); j.close_dict();
            j.emit_key("dict1_empty2"); j.open_array(); j.close_array();
            j.emit_key("str1"); j.emit_str("\"ABC\u{8}DEF\\");
            j.close_dict();
            j.close_dict();
        });
        assert_eq!(s, r#"{"a":123,"b":456.7,"dict1":{"dict1_arr1":["val1","val2","val3"],"dict1_empty":{},"dict1_empty2":[],"str1":"\"ABC\bDEF\\"}}"#);
    }

    #[test]
    fn escapes() {
        // unittests/Support/JSONEmitterTest.cpp: Escapes
        let s = emit(|j| j.emit_str("x\"\\/\u{8}\u{c}\n\r\tx"));
        assert_eq!(s, r#""x\"\\\/\b\f\n\r\tx""#);
    }

    #[test]
    fn forward_slashes() {
        // EmitGroupsOfForwardSlashes
        let s = emit(|j| {
            j.open_dict();
            j.emit_key("url"); j.emit_str("http://www.example.com");
            j.close_dict();
        });
        assert_eq!(s, r#"{"url":"http:\/\/www.example.com"}"#);
    }

    #[test]
    fn non_ascii_and_astral() {
        // NonAsciiEscapes + EmitUTF8
        let s = emit(|j| {
            j.open_dict();
            j.emit_key("ha"); j.emit_str("\u{54C8}");
            j.emit_key("gClef"); j.emit_str("\u{1D11E}");
            j.emit_key("wave"); j.emit_str("hi\u{1F44B}");
            j.close_dict();
        });
        assert_eq!(s, r#"{"ha":"哈","gClef":"𝄞","wave":"hi👋"}"#);
    }

    #[test]
    fn non_finite_is_null() {
        // NonFinite — the emitter (not number_to_string) maps non-finite to null.
        let s = emit(|j| {
            j.open_array();
            j.emit_f64(f64::INFINITY); j.emit_f64(f64::NEG_INFINITY); j.emit_f64(f64::NAN);
            j.close_array();
        });
        assert_eq!(s, "[null,null,null]");
    }
```

- [ ] **Step 2: Run to verify it fails.** Run: `cargo test --manifest-path rust/Cargo.toml -p support json_emitter`. Expected: FAIL (`JSONEmitter` not defined).

- [ ] **Step 3: Implement.** Add above the `tests` module. This ports `JSONEmitter.cpp` lines as cited:

```rust
/// A single object (Dictionary or Array) being emitted.
/// Port of `JSONEmitter::State` (JSONEmitter.h:170).
#[derive(Clone, Copy, PartialEq, Eq)]
enum StateType {
    Dict,
    Array,
}

struct State {
    ty: StateType,
    needs_comma: bool,
    needs_key: bool,
    needs_value: bool,
    is_empty: bool,
}

impl State {
    fn new(ty: StateType) -> State {
        State {
            ty,
            needs_comma: false,
            needs_key: ty == StateType::Dict,
            needs_value: false,
            is_empty: true,
        }
    }
}

/// Port of `hermes::JSONEmitter` (include/hermes/Support/JSONEmitter.h). Emits
/// JSON to a `String`. `pretty` adds newlines + indentation. Unbalanced
/// dict/array use is caught via `debug_assert!` (C++ `assert`).
///
/// Strings must be valid UTF-8 (C++ takes `StringRef` and treats invalid UTF-8
/// as fatal). Non-ASCII code points are emitted as escaped UTF-16 code units.
pub struct JSONEmitter<'w> {
    out: &'w mut String,
    pretty: bool,
    indent: u32,
    states: Vec<State>,
}

impl<'w> JSONEmitter<'w> {
    pub fn new(out: &'w mut String, pretty: bool) -> JSONEmitter<'w> {
        JSONEmitter { out, pretty, indent: 0, states: Vec::new() }
    }

    fn in_dict(&self) -> bool {
        matches!(self.states.last(), Some(s) if s.ty == StateType::Dict)
    }
    fn in_array(&self) -> bool {
        matches!(self.states.last(), Some(s) if s.ty == StateType::Array)
    }

    /// JSONEmitter.cpp:239 — housekeeping before emitting any value (not a key).
    fn will_emit_value(&mut self) {
        if self.states.is_empty() {
            return;
        }
        let is_array;
        {
            let state = self.states.last_mut().unwrap();
            debug_assert!(!state.needs_key, "Expected a key");
            if state.needs_comma {
                self.out.push(',');
            }
            state.needs_key = state.ty == StateType::Dict;
            state.needs_comma = true;
            state.needs_value = false;
            state.is_empty = false;
            is_array = state.ty == StateType::Array;
        }
        if is_array {
            self.pretty_new_line();
        }
    }

    pub fn emit_bool(&mut self, val: bool) {
        self.will_emit_value();
        self.out.push_str(if val { "true" } else { "false" });
    }

    /// Covers the C++ integer overloads (short/int/long/...).
    pub fn emit_i64(&mut self, val: i64) {
        self.will_emit_value();
        let _ = write!(self.out, "{val}");
    }
    pub fn emit_u64(&mut self, val: u64) {
        self.will_emit_value();
        let _ = write!(self.out, "{val}");
    }

    /// JSONEmitter.cpp:67 — finite via numberToString, non-finite -> "null".
    pub fn emit_f64(&mut self, val: f64) {
        self.will_emit_value();
        if val.is_finite() {
            self.out.push_str(&number_to_string(val));
        } else {
            self.out.push_str("null");
        }
    }

    /// JSONEmitter.cpp:78 — emit a UTF-8 string value (not a dict key).
    pub fn emit_str(&mut self, val: &str) {
        self.will_emit_value();
        self.primitive_emit_string(val);
    }

    /// JSONEmitter.cpp:193 — emit a value from UTF-16 code units. Each unit is
    /// emitted independently (no surrogate combination).
    pub fn emit_u16(&mut self, val: &[u16]) {
        self.will_emit_value();
        self.out.push('"');
        for &curr in val {
            self.emit_one_escaped_unit(curr);
        }
        self.out.push('"');
    }

    pub fn emit_null_value(&mut self) {
        self.will_emit_value();
        self.out.push_str("null");
    }

    /// JSONEmitter.cpp:88 — emit a dict key.
    pub fn emit_key(&mut self, key: &str) {
        debug_assert!(self.in_dict(), "Not emitting a dictionary");
        {
            let state = self.states.last_mut().unwrap();
            debug_assert!(state.needs_key, "Not expecting a key");
            debug_assert!(!state.needs_value, "Missing a value for a key.");
            if state.needs_comma {
                self.out.push(',');
            }
            state.needs_comma = false;
            state.needs_key = false;
            state.needs_value = true;
        }
        self.pretty_new_line();
        self.primitive_emit_string(key);
        self.out.push(':');
        if self.pretty {
            self.out.push(' ');
        }
    }

    pub fn open_dict(&mut self) {
        self.will_emit_value();
        self.out.push('{');
        self.indent_more();
        self.states.push(State::new(StateType::Dict));
    }
    pub fn close_dict(&mut self) {
        debug_assert!(self.in_dict(), "Not currently emitting a dictionary");
        debug_assert!(!self.states.last().unwrap().needs_value, "Missing a value for a key.");
        self.indent_less();
        if !self.states.last().unwrap().is_empty {
            self.pretty_new_line();
        }
        self.out.push('}');
        self.states.pop();
    }
    pub fn open_array(&mut self) {
        self.will_emit_value();
        self.indent_more();
        self.out.push('[');
        self.states.push(State::new(StateType::Array));
    }
    pub fn close_array(&mut self) {
        debug_assert!(self.in_array(), "Not currently emitting an array");
        self.indent_less();
        if !self.states.last().unwrap().is_empty {
            self.pretty_new_line();
        }
        self.out.push(']');
        self.states.pop();
    }

    /// JSONEmitter.cpp:234 — terminate a JSON Lines record.
    pub fn end_jsonl(&mut self) {
        debug_assert!(self.states.is_empty(), "Previous object was not terminated.");
        self.out.push('\n');
    }

    /// JSONEmitter.cpp:141 — escape + emit a UTF-8 string (key or value).
    fn primitive_emit_string(&mut self, s: &str) {
        self.out.push('"');
        for ch in s.chars() {
            let cp = ch as u32;
            if cp > 0x7F {
                // encodeUTF16(cp) -> 1 or 2 units, each as \uXXXX.
                if cp <= 0xFFFF {
                    self.write_u_escape(cp as u16);
                } else {
                    let c = cp - 0x10000;
                    self.write_u_escape(0xD800 + (c >> 10) as u16);
                    self.write_u_escape(0xDC00 + (c & 0x3FF) as u16);
                }
                continue;
            }
            if cp == 0x22 || cp == 0x5C || cp == 0x2F {
                // escape " \ /
                self.out.push('\\');
            }
            if cp >= 0x20 {
                self.out.push(cp as u8 as char);
                continue;
            }
            match cp {
                0x08 => self.out.push_str("\\b"),
                0x0C => self.out.push_str("\\f"),
                0x0A => self.out.push_str("\\n"),
                0x0D => self.out.push_str("\\r"),
                0x09 => self.out.push_str("\\t"),
                _ => self.write_u_escape(cp as u16),
            }
        }
        self.out.push('"');
    }

    /// One UTF-16 code unit, escaped per JSONEmitter.cpp:193 (the char16 path).
    fn emit_one_escaped_unit(&mut self, curr: u16) {
        let c = curr as u32;
        if c > 0x7F {
            self.write_u_escape(curr);
            return;
        }
        if c >= 0x20 {
            if c == 0x22 || c == 0x5C || c == 0x2F {
                self.out.push('\\');
            }
            self.out.push(c as u8 as char);
            return;
        }
        match c {
            0x08 => self.out.push_str("\\b"),
            0x0C => self.out.push_str("\\f"),
            0x0A => self.out.push_str("\\n"),
            0x0D => self.out.push_str("\\r"),
            0x09 => self.out.push_str("\\t"),
            _ => self.write_u_escape(curr),
        }
    }

    fn write_u_escape(&mut self, u: u16) {
        let _ = write!(self.out, "\\u{u:04x}");
    }

    fn pretty_new_line(&mut self) {
        if !self.pretty {
            return;
        }
        self.out.push('\n');
        for _ in 0..self.indent {
            self.out.push(' ');
        }
    }
    fn indent_more(&mut self) {
        if self.pretty {
            self.indent += 2;
        }
    }
    fn indent_less(&mut self) {
        if self.pretty {
            debug_assert!(self.indent >= 2, "Unbalanced indentation.");
            self.indent -= 2;
        }
    }
}
```

- [ ] **Step 4: Run to verify it passes.** Run: `cargo test --manifest-path rust/Cargo.toml -p support json_emitter`. Expected: PASS (all A2 tests + A1).

- [ ] **Step 5: Commit.**

```bash
git add rust/crates/support/src/json_emitter.rs
git commit -m "rust(support): port JSONEmitter core (open/close, keys, values, escaping)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task A3: pretty-print, JSONL, UTF-16 — finish JSONEmitter parity

**Files:** Modify `rust/crates/support/src/json_emitter.rs`

- [ ] **Step 1: Write failing tests** (port `PrettyPrint`, `JSONL`, `EmitUTF16`):

```rust
    #[test]
    fn jsonl() {
        let mut s = String::new();
        {
            let mut j = JSONEmitter::new(&mut s, false);
            j.open_dict(); j.close_dict(); j.end_jsonl();
            j.open_dict(); j.close_dict(); j.end_jsonl();
        }
        assert_eq!(s, "{}\n{}\n");
    }

    #[test]
    fn emit_utf16() {
        // EmitUTF16: u"hi\xd83d\xdc4b" -> "hi👋"
        let units: Vec<u16> = vec![b'h' as u16, b'i' as u16, 0xd83d, 0xdc4b];
        let mut s = String::new();
        {
            let mut j = JSONEmitter::new(&mut s, false);
            j.open_dict();
            j.emit_key("str"); j.emit_u16(&units);
            j.close_dict();
        }
        assert_eq!(s, r#"{"str":"hi👋"}"#);
    }

    #[test]
    fn pretty_print() {
        // unittests/Support/JSONEmitterTest.cpp: PrettyPrint
        let mut s = String::new();
        {
            let mut j = JSONEmitter::new(&mut s, true);
            j.open_dict();
            j.emit_key("artist"); j.emit_str("prince");
            j.emit_key("instruments");
            j.open_array();
            j.emit_str("piano");
            j.open_dict();
            j.emit_key("guitars");
            j.open_array();
            j.emit_str("cloud"); j.emit_str("love symbol"); j.emit_str("telecaster");
            j.close_array();
            j.close_dict();
            j.emit_str("drums");
            j.close_array();
            j.emit_key("songs");
            j.open_dict();
            j.emit_key("purple rain"); j.emit_i64(1984);
            j.emit_key("1999"); j.emit_i64(1982);
            j.close_dict();
            j.emit_key("color"); j.emit_str("purple");
            j.emit_key("emptyDict"); j.open_dict(); j.close_dict();
            j.emit_key("emptyArray"); j.open_array(); j.close_array();
            j.close_dict();
        }
        let expected = "{\n  \"artist\": \"prince\",\n  \"instruments\": [\n    \"piano\",\n    {\n      \"guitars\": [\n        \"cloud\",\n        \"love symbol\",\n        \"telecaster\"\n      ]\n    },\n    \"drums\"\n  ],\n  \"songs\": {\n    \"purple rain\": 1984,\n    \"1999\": 1982\n  },\n  \"color\": \"purple\",\n  \"emptyDict\": {},\n  \"emptyArray\": []\n}";
        assert_eq!(s, expected);
    }
```

- [ ] **Step 2: Run to verify.** Run: `cargo test --manifest-path rust/Cargo.toml -p support json_emitter`. Expected: PASS (A2 already implemented the methods these exercise — `emit_u16`, `end_jsonl`, pretty). If `pretty_print` fails, diff against the expected and fix `pretty_new_line`/`indent_*`/`will_emit_value` ordering against `JSONEmitter.cpp` until byte-identical.

- [ ] **Step 3: (only if Step 2 surfaced a bug)** fix the cited method; otherwise no code change.

- [ ] **Step 4: Confirm pass + zero warnings.** Run: `cargo test --manifest-path rust/Cargo.toml -p support && cargo build --manifest-path rust/Cargo.toml`. Expected: PASS, no warnings.

- [ ] **Step 5: Commit.**

```bash
git add rust/crates/support/src/json_emitter.rs
git commit -m "rust(support): JSONEmitter pretty-print + JSONL + UTF-16 (full JSONEmitterTest parity)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Phase B — Value model + JSONFactory (parser crate)

### Task B0: add `bumpalo` + `json` module skeleton

**Files:** Modify `rust/crates/parser/Cargo.toml`, `rust/crates/parser/src/lib.rs`; create `rust/crates/parser/src/json/mod.rs`, `factory.rs`, `parser.rs`.

- [ ] **Step 1: Add the dependency.** In `rust/crates/parser/Cargo.toml` `[dependencies]` add:

```toml
bumpalo = "3.16"
```

- [ ] **Step 2: Declare the module.** In `rust/crates/parser/src/lib.rs` add `pub mod json;` (keep the list sorted).

- [ ] **Step 3: Create stubs.** `json/mod.rs` with copyright header + `pub mod factory; pub mod parser;` and a `//!` doc line citing `lib/Parser/JSONParser`. Empty (header-only) `factory.rs` and `parser.rs` with copyright headers.

- [ ] **Step 4: Verify it builds.** Run: `cargo build --manifest-path rust/Cargo.toml -p parser`. Expected: builds, zero warnings (bumpalo downloaded).

- [ ] **Step 5: Commit.**

```bash
git add rust/crates/parser/Cargo.toml rust/crates/parser/src/lib.rs rust/crates/parser/src/json/
git commit -m "rust(parser): add bumpalo dep + json module skeleton

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task B1: `JSONValue` + `JSONHiddenClass` + accessors

**Files:** Modify `rust/crates/parser/src/json/mod.rs`. Reference: `JSONParser.h:36-519`.

- [ ] **Step 1: Write failing tests** (hand-build values in a `Bump`, exercise accessors):

```rust
#[cfg(test)]
mod model_tests {
    use super::*;
    use bumpalo::Bump;

    #[test]
    fn kinds_and_scalar_accessors() {
        let arena = Bump::new();
        let n: &JSONValue = arena.alloc(JSONValue::Number(1.5));
        let b: &JSONValue = arena.alloc(JSONValue::Boolean(true));
        assert_eq!(n.kind(), JSONKind::Number);
        assert_eq!(b.kind(), JSONKind::Boolean);
        assert_eq!(n.as_number(), Some(1.5));
        assert_eq!(b.as_boolean(), Some(true));
        assert_eq!(n.as_boolean(), None);
        assert_eq!(JSONValue::Null.kind(), JSONKind::Null);
        assert_eq!(kind_to_string(JSONKind::Array), "Array");
    }

    #[test]
    fn array_accessors() {
        let arena = Bump::new();
        let a = arena.alloc(JSONValue::Number(10.0));
        let b = arena.alloc(JSONValue::Number(20.0));
        let elems: &[&JSONValue] = arena.alloc_slice_copy(&[&*a, &*b]);
        let arr = arena.alloc(JSONValue::Array(elems));
        let view = arr.as_array().unwrap();
        assert_eq!(view.len(), 2);
        assert_eq!(view.at(0).as_number(), Some(10.0));
        assert_eq!(view.iter().count(), 2);
    }
}
```

- [ ] **Step 2: Run to verify it fails.** Run: `cargo test --manifest-path rust/Cargo.toml -p parser model_tests`. Expected: FAIL.

- [ ] **Step 3: Implement** in `json/mod.rs`:

```rust
use atom_table::AtomBytes;

/// Port of `JSONKind` (JSONParser.h:36).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JSONKind {
    Object,
    Array,
    String,
    Number,
    Boolean,
    Null,
}

/// Port of `JSONKindToString` (JSONParser.cpp:21).
pub fn kind_to_string(kind: JSONKind) -> &'static str {
    match kind {
        JSONKind::Object => "Object",
        JSONKind::Array => "Array",
        JSONKind::String => "String",
        JSONKind::Number => "Number",
        JSONKind::Boolean => "Boolean",
        JSONKind::Null => "Null",
    }
}

/// A descriptor with a sorted list of names; objects of the same shape share one
/// (JSONParser.h:180 `JSONHiddenClass`). `keys` are sorted by string content.
pub struct JSONHiddenClass<'a> {
    pub(crate) keys: &'a [AtomBytes],
}

impl<'a> JSONHiddenClass<'a> {
    pub fn size(&self) -> usize {
        self.keys.len()
    }
    pub fn keys(&self) -> &'a [AtomBytes] {
        self.keys
    }
    /// JSONParser.h:225 — binary-search the sorted keys for `name` (compared by
    /// bytes); return its index. `atoms` resolves AtomBytes -> bytes.
    pub fn find(&self, name: &[u8], atoms: &atom_table::AtomTable) -> Option<usize> {
        self.keys
            .binary_search_by(|k| atoms.bytes(*k).cmp(name))
            .ok()
    }
}

/// The base type for all JSON values (JSONParser.h:49). `&'a JSONValue<'a>` IS
/// the C++ `JSONValue*`: nodes live in a `bumpalo` arena; the variant replaces
/// the kind tag + LLVM RTTI; arena identity gives pointer equality.
pub enum JSONValue<'a> {
    Null,
    Boolean(bool),
    Number(f64),
    String(AtomBytes),
    Array(&'a [&'a JSONValue<'a>]),
    Object(&'a JSONHiddenClass<'a>, &'a [&'a JSONValue<'a>]),
}

impl<'a> JSONValue<'a> {
    pub fn kind(&self) -> JSONKind {
        match self {
            JSONValue::Null => JSONKind::Null,
            JSONValue::Boolean(_) => JSONKind::Boolean,
            JSONValue::Number(_) => JSONKind::Number,
            JSONValue::String(_) => JSONKind::String,
            JSONValue::Array(_) => JSONKind::Array,
            JSONValue::Object(..) => JSONKind::Object,
        }
    }
    pub fn as_number(&self) -> Option<f64> {
        match self { JSONValue::Number(n) => Some(*n), _ => None }
    }
    pub fn as_boolean(&self) -> Option<bool> {
        match self { JSONValue::Boolean(b) => Some(*b), _ => None }
    }
    /// Returns the interned handle (resolve bytes via the AtomTable).
    pub fn as_string(&self) -> Option<AtomBytes> {
        match self { JSONValue::String(a) => Some(*a), _ => None }
    }
    pub fn as_array(&self) -> Option<ArrayView<'a, '_>> {
        match self { JSONValue::Array(v) => Some(ArrayView { values: v }), _ => None }
    }
    pub fn as_object(&self) -> Option<ObjectView<'a, '_>> {
        match self {
            JSONValue::Object(c, v) => Some(ObjectView { class: c, values: v }),
            _ => None,
        }
    }
}

/// Borrowed view over an array (JSONParser.h:458 `JSONArray`).
pub struct ArrayView<'a, 'v> {
    values: &'v &'a [&'a JSONValue<'a>],
}
impl<'a, 'v> ArrayView<'a, 'v> {
    pub fn len(&self) -> usize { self.values.len() }
    pub fn is_empty(&self) -> bool { self.values.is_empty() }
    pub fn at(&self, pos: usize) -> &'a JSONValue<'a> { self.values[pos] }
    pub fn iter(&self) -> impl Iterator<Item = &'a JSONValue<'a>> + '_ {
        self.values.iter().copied()
    }
}
```

(`ObjectView` is added in Task B3, where object lookups are tested; for now the `as_object` return type compiles only after B3. To keep B1 self-contained, temporarily omit `as_object`/`ObjectView` and add them in B3 — OR define a minimal `ObjectView { class, values }` here and grow it in B3. **Choose: define the minimal struct here**, with `len()` only, expand in B3.)

Minimal `ObjectView` to add in B1:

```rust
/// Borrowed view over an object (JSONParser.h:239 `JSONObject`). Grown in B3.
pub struct ObjectView<'a, 'v> {
    pub(crate) class: &'v &'a JSONHiddenClass<'a>,
    pub(crate) values: &'v &'a [&'a JSONValue<'a>],
}
impl<'a, 'v> ObjectView<'a, 'v> {
    pub fn size(&self) -> usize { self.values.len() }
    pub fn get_hidden_class(&self) -> &'a JSONHiddenClass<'a> { self.class }
}
```

- [ ] **Step 4: Run to verify it passes.** Run: `cargo test --manifest-path rust/Cargo.toml -p parser model_tests`. Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add rust/crates/parser/src/json/mod.rs
git commit -m "rust(parser): JSONValue model + hidden class + array/scalar accessors

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task B2: `JSONFactory` — singletons + number/string uniquing

**Files:** Modify `rust/crates/parser/src/json/factory.rs`, re-export from `mod.rs`. Reference: `JSONParser.cpp:74-107`, `JSONParser.h:524-581`.

- [ ] **Step 1: Write failing tests** (mirror the factory assertions in `SmokeTest2` + `NegativeNumbers`):

```rust
#[cfg(test)]
mod factory_tests {
    use super::super::*;
    use atom_table::AtomTable;
    use bumpalo::Bump;

    #[test]
    fn uniquing_and_singletons() {
        let arena = Bump::new();
        let atoms = AtomTable::new();
        let f = JSONFactory::new(&arena, &atoms);

        // Strings unique by content.
        let a = f.get_string_str("key2");
        let b = f.get_string_str("key2");
        assert!(std::ptr::eq(a, b));
        assert_eq!(a.as_string().map(|h| atoms.bytes(h).to_vec()), Some(b"key2".to_vec()));

        // Numbers unique; -0.0 distinct from 0.0 (NegativeNumbers).
        assert!(std::ptr::eq(f.get_number(1.0), f.get_number(1.0)));
        assert!(!std::ptr::eq(f.get_number(0.0), f.get_number(-0.0)));

        // Singletons.
        assert!(std::ptr::eq(f.get_null(), f.get_null()));
        assert!(std::ptr::eq(f.get_boolean(true), f.get_boolean(true)));
        assert!(!std::ptr::eq(f.get_boolean(true), f.get_boolean(false)));
    }
}
```

- [ ] **Step 2: Run to verify it fails.** Run: `cargo test --manifest-path rust/Cargo.toml -p parser factory_tests`. Expected: FAIL.

- [ ] **Step 3: Implement** `factory.rs`:

```rust
use std::cell::RefCell;
use std::collections::HashMap;

use atom_table::{AtomBytes, AtomTable};
use bumpalo::Bump;

use super::{JSONHiddenClass, JSONValue};

/// Owns all JSON nodes (in the arena), uniques strings/numbers, and shares
/// hidden classes. Port of `JSONFactory` (JSONParser.h:524). Accessors take
/// `&self`; the returned `&'a JSONValue` lives in the arena, independent of the
/// transient `RefCell` borrow, so a caller may interleave factory use with
/// parsing (as `JSONParserTest::SmokeTest2` does).
pub struct JSONFactory<'a> {
    arena: &'a Bump,
    atoms: &'a AtomTable,
    strings: RefCell<HashMap<AtomBytes, &'a JSONValue<'a>>>,
    numbers: RefCell<HashMap<u64, &'a JSONValue<'a>>>,
    classes: RefCell<HashMap<Box<[AtomBytes]>, &'a JSONHiddenClass<'a>>>,
    null_v: &'a JSONValue<'a>,
    true_v: &'a JSONValue<'a>,
    false_v: &'a JSONValue<'a>,
}

impl<'a> JSONFactory<'a> {
    pub fn new(arena: &'a Bump, atoms: &'a AtomTable) -> JSONFactory<'a> {
        JSONFactory {
            arena,
            atoms,
            strings: RefCell::new(HashMap::new()),
            numbers: RefCell::new(HashMap::new()),
            classes: RefCell::new(HashMap::new()),
            null_v: arena.alloc(JSONValue::Null),
            true_v: arena.alloc(JSONValue::Boolean(true)),
            false_v: arena.alloc(JSONValue::Boolean(false)),
        }
    }

    pub fn arena(&self) -> &'a Bump { self.arena }
    pub fn atoms(&self) -> &'a AtomTable { self.atoms }

    pub fn get_null(&self) -> &'a JSONValue<'a> { self.null_v }
    pub fn get_boolean(&self, v: bool) -> &'a JSONValue<'a> {
        if v { self.true_v } else { self.false_v }
    }

    /// JSONParser.cpp:79 — unique a string by its interned handle.
    pub fn get_string(&self, lit: AtomBytes) -> &'a JSONValue<'a> {
        if let Some(found) = self.strings.borrow().get(&lit) {
            return found;
        }
        let node: &'a JSONValue<'a> = self.arena.alloc(JSONValue::String(lit));
        self.strings.borrow_mut().insert(lit, node);
        node
    }
    /// JSONParser.cpp:92 — intern `str` then unique.
    pub fn get_string_str(&self, s: &str) -> &'a JSONValue<'a> {
        self.get_string(self.atoms.atom_bytes(s))
    }

    /// JSONParser.cpp:96 — unique a number by its bit pattern (so -0.0 != 0.0,
    /// matching `JSONNumber::Profile` using DoubleToBits).
    pub fn get_number(&self, value: f64) -> &'a JSONValue<'a> {
        let bits = value.to_bits();
        if let Some(found) = self.numbers.borrow().get(&bits) {
            return found;
        }
        let node: &'a JSONValue<'a> = self.arena.alloc(JSONValue::Number(value));
        self.numbers.borrow_mut().insert(bits, node);
        node
    }
}
```

Re-export from `mod.rs`: add `pub use factory::JSONFactory;`.

- [ ] **Step 4: Run to verify it passes.** Run: `cargo test --manifest-path rust/Cargo.toml -p parser factory_tests`. Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add rust/crates/parser/src/json/
git commit -m "rust(parser): JSONFactory singletons + string/number uniquing

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task B3: hidden classes + objects/arrays + full object accessors

**Files:** Modify `rust/crates/parser/src/json/factory.rs` and `mod.rs`. Reference: `JSONParser.cpp:109-154`, `JSONParser.h:285-456`.

- [ ] **Step 1: Write failing tests** (object accessors + `HiddenClassTest`-style sharing, built via the factory):

```rust
    #[test]
    fn objects_arrays_and_hidden_class_sharing() {
        use super::super::JSONFactory;
        let arena = Bump::new();
        let atoms = AtomTable::new();
        let f = JSONFactory::new(&arena, &atoms);

        let mk = |f: &JSONFactory<'_>, k1: f64, k2: f64| {
            // object {'key1': k1, 'key2': k2} via unsorted props
            let p1 = (f.get_string_str("key1"), f.get_number(k1));
            let p2 = (f.get_string_str("key2"), f.get_number(k2));
            f.new_object(&mut [p2, p1]).unwrap() // intentionally unsorted
        };
        let o1 = mk(&f, 1.0, 2.0);
        let o3 = mk(&f, 20.0, 10.0);

        let v1 = o1.as_object().unwrap();
        assert_eq!(v1.size(), 2);
        // shared hidden class for same-shape objects (HiddenClassTest).
        assert!(std::ptr::eq(
            v1.get_hidden_class(),
            o3.as_object().unwrap().get_hidden_class()
        ));
        // lookups
        assert_eq!(v1.count("key1", &atoms), 1);
        assert_eq!(v1.count("zzz", &atoms), 0);
        assert_eq!(v1.get("key1", &atoms).and_then(|v| v.as_number()), Some(1.0));
        // duplicate keys -> error
        let dup = (f.get_string_str("k"), f.get_number(1.0));
        assert!(f.new_object(&mut [dup, dup]).is_none());

        // arrays
        let a = f.new_array(&[f.get_number(5.0), f.get_null()]);
        let av = a.as_array().unwrap();
        assert_eq!(av.len(), 2);
        assert_eq!(av.at(0).as_number(), Some(5.0));
    }
```

- [ ] **Step 2: Run to verify it fails.** Run: `cargo test --manifest-path rust/Cargo.toml -p parser factory_tests`. Expected: FAIL.

- [ ] **Step 3a: Implement factory methods** (`factory.rs`). `Prop` is `(&'a JSONValue, &'a JSONValue)` where the key node is a `String`:

```rust
/// A single property: (key, value). The key is a `JSONValue::String`.
pub type Prop<'a> = (&'a JSONValue<'a>, &'a JSONValue<'a>);

impl<'a> JSONFactory<'a> {
    fn key_bytes(&self, key: &'a JSONValue<'a>) -> AtomBytes {
        key.as_string().expect("object key must be a JSON string")
    }

    /// JSONParser.cpp:120 — sort props by key content; return the first
    /// duplicate key (by interned identity) if any, else None.
    pub fn sort_props(&self, props: &mut [Prop<'a>]) -> Option<AtomBytes> {
        props.sort_by(|a, b| {
            self.atoms.bytes(self.key_bytes(a.0)).cmp(self.atoms.bytes(self.key_bytes(b.0)))
        });
        let mut last: Option<AtomBytes> = None;
        for p in props.iter() {
            let kb = self.key_bytes(p.0);
            if last == Some(kb) {
                return Some(kb);
            }
            last = Some(kb);
        }
        None
    }

    /// JSONParser.cpp:109 — look up or create the shared hidden class for `keys`
    /// (already content-sorted).
    pub fn get_hidden_class(&self, keys: &[AtomBytes]) -> &'a JSONHiddenClass<'a> {
        if let Some(found) = self.classes.borrow().get(keys) {
            return found;
        }
        let arena_keys: &'a [AtomBytes] = self.arena.alloc_slice_copy(keys);
        let cls: &'a JSONHiddenClass<'a> =
            self.arena.alloc(JSONHiddenClass { keys: arena_keys });
        self.classes.borrow_mut().insert(keys.into(), cls);
        cls
    }

    /// JSONParser.cpp:138 — create an object from props. Sorts + dedups; returns
    /// None on a duplicate key.
    pub fn new_object(&self, props: &mut [Prop<'a>]) -> Option<&'a JSONValue<'a>> {
        if self.sort_props(props).is_some() {
            return None;
        }
        let keys: Vec<AtomBytes> = props.iter().map(|p| self.key_bytes(p.0)).collect();
        let cls = self.get_hidden_class(&keys);
        let values: Vec<&'a JSONValue<'a>> = props.iter().map(|p| p.1).collect();
        let values: &'a [&'a JSONValue<'a>] = self.arena.alloc_slice_copy(&values);
        Some(self.arena.alloc(JSONValue::Object(cls, values)))
    }

    /// JSONParser.h:617 — create an array from values.
    pub fn new_array(&self, values: &[&'a JSONValue<'a>]) -> &'a JSONValue<'a> {
        let values: &'a [&'a JSONValue<'a>] = self.arena.alloc_slice_copy(values);
        self.arena.alloc(JSONValue::Array(values))
    }
}
```

- [ ] **Step 3b: Grow `ObjectView`** in `mod.rs` (needs the `AtomTable` to resolve names; values parallel the class keys):

```rust
impl<'a, 'v> ObjectView<'a, 'v> {
    /// JSONParser.h:286 — value for `name`, or None.
    pub fn get(&self, name: &str, atoms: &atom_table::AtomTable) -> Option<&'a JSONValue<'a>> {
        self.class.find(name.as_bytes(), atoms).map(|i| self.values[i])
    }
    /// JSONParser.h:295 — value for `name`; panics if absent (C++ asserts).
    pub fn at(&self, name: &str, atoms: &atom_table::AtomTable) -> &'a JSONValue<'a> {
        self.get(name, atoms).expect("name not found")
    }
    /// JSONParser.h:323 — 1 if present else 0.
    pub fn count(&self, name: &str, atoms: &atom_table::AtomTable) -> usize {
        if self.class.find(name.as_bytes(), atoms).is_some() { 1 } else { 0 }
    }
    /// Value by position (0..size).
    pub fn value_at(&self, index: usize) -> &'a JSONValue<'a> { self.values[index] }
    /// Key (interned handle) by position.
    pub fn key_at(&self, index: usize) -> AtomBytes { self.class.keys[index] }
    /// JSONParser.h:330 — (key, value) pairs, in the hidden class's sorted order.
    pub fn iter(&self) -> impl Iterator<Item = (AtomBytes, &'a JSONValue<'a>)> + '_ {
        self.class.keys.iter().copied().zip(self.values.iter().copied())
    }
}
```

Re-export `Prop` from `mod.rs`: `pub use factory::{JSONFactory, Prop};`.

> **Iteration-order note (faithful):** C++ `JSONObject` iterates in **hidden-class (sorted) order**, not insertion order — `JSONObject::iterator` indexes `hiddenClass_->begin()[index_]`. `JSONParserTest::SmokeTest2` asserts key1<key2<key3<key4 which is *both* insertion and sorted order for that input, so it passes either way; we follow the C++ and iterate in sorted order.

- [ ] **Step 4: Run to verify it passes.** Run: `cargo test --manifest-path rust/Cargo.toml -p parser factory_tests model_tests`. Expected: PASS. Then `cargo build --manifest-path rust/Cargo.toml` — zero warnings.

- [ ] **Step 5: Commit.**

```bash
git add rust/crates/parser/src/json/
git commit -m "rust(parser): hidden classes, new_object/new_array, full object accessors

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task B4: `emit_into` on `JSONValue`

**Files:** Modify `rust/crates/parser/src/json/mod.rs`. Reference: `JSONParser.cpp:39-72`.

- [ ] **Step 1: Write failing test** (build the `EmitTest` object via the factory, emit, compare):

```rust
    #[test]
    fn emit_into_round_trip() {
        use super::JSONFactory;
        use atom_table::AtomTable;
        use bumpalo::Bump;
        use support::json_emitter::JSONEmitter;

        let arena = Bump::new();
        let atoms = AtomTable::new();
        let f = JSONFactory::new(&arena, &atoms);

        // {'key1':1,'key2':'value2','key3':{'nested1':true},'key4':[false,null,'value2']}
        let nested = {
            let p = (f.get_string_str("nested1"), f.get_boolean(true));
            f.new_object(&mut [p]).unwrap()
        };
        let arr = f.new_array(&[f.get_boolean(false), f.get_null(), f.get_string_str("value2")]);
        let obj = f.new_object(&mut [
            (f.get_string_str("key1"), f.get_number(1.0)),
            (f.get_string_str("key2"), f.get_string_str("value2")),
            (f.get_string_str("key3"), nested),
            (f.get_string_str("key4"), arr),
        ]).unwrap();

        let mut s = String::new();
        {
            let mut e = JSONEmitter::new(&mut s, false);
            obj.emit_into(&mut e, &atoms);
        }
        // sorted-key order: key1,key2,key3,key4
        assert_eq!(s, r#"{"key1":1,"key2":"value2","key3":{"nested1":true},"key4":[false,null,"value2"]}"#);
    }
```

- [ ] **Step 2: Run to verify it fails.** Run: `cargo test --manifest-path rust/Cargo.toml -p parser emit_into`. Expected: FAIL.

- [ ] **Step 3: Implement** in `mod.rs`:

```rust
use support::json_emitter::JSONEmitter;

impl<'a> JSONValue<'a> {
    /// Port of `JSONValue::emitInto` (JSONParser.cpp:39). `atoms` resolves
    /// interned string handles to bytes. Strings must be valid UTF-8.
    pub fn emit_into(&self, emitter: &mut JSONEmitter, atoms: &atom_table::AtomTable) {
        match self {
            JSONValue::Object(class, values) => {
                emitter.open_dict();
                for (k, v) in class.keys.iter().copied().zip(values.iter().copied()) {
                    let key = std::str::from_utf8(atoms.bytes(k)).expect("valid UTF-8 key");
                    emitter.emit_key(key);
                    v.emit_into(emitter, atoms);
                }
                emitter.close_dict();
            }
            JSONValue::Array(values) => {
                emitter.open_array();
                for v in values.iter().copied() {
                    v.emit_into(emitter, atoms);
                }
                emitter.close_array();
            }
            JSONValue::String(a) => {
                let s = std::str::from_utf8(atoms.bytes(*a)).expect("valid UTF-8 string");
                emitter.emit_str(s);
            }
            JSONValue::Number(n) => emitter.emit_f64(*n),
            JSONValue::Boolean(b) => emitter.emit_bool(*b),
            JSONValue::Null => emitter.emit_null_value(),
        }
    }
}
```

- [ ] **Step 4: Run to verify it passes.** Run: `cargo test --manifest-path rust/Cargo.toml -p parser emit_into`. Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add rust/crates/parser/src/json/mod.rs
git commit -m "rust(parser): JSONValue::emit_into (port of emitInto)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Phase C — JSONParser (recursive descent over JSLexer)

### Task C0: add `get_source_mgr_mut` to JSLexer

**Files:** Modify `rust/crates/parser/src/lexer/mod.rs` (next to `get_source_mgr` at line 346).

- [ ] **Step 1: Write failing test** (in the lexer's test module or a small inline test):

```rust
    #[test]
    fn source_mgr_mut_reports_errors() {
        let mut sm = support::manager::SourceErrorManager::new();
        let atoms = atom_table::AtomTable::new();
        let id = sm.add_buffer("t", "x");
        let mut lex = JSLexer::new(id, &mut sm, &atoms, GrammarContext::AllowDiv);
        let loc = lex.token().start_loc();
        lex.get_source_mgr_mut().error_at(loc, None, "boom", support::diag::Subsystem::Parser);
        assert_eq!(lex.get_source_mgr().error_count(), 1);
    }
```

(Adjust the `SourceErrorManager::new()` / module paths to the actual ones used elsewhere in the lexer tests.)

- [ ] **Step 2: Run to verify it fails.** Run: `cargo test --manifest-path rust/Cargo.toml -p parser source_mgr_mut`. Expected: FAIL (no such method).

- [ ] **Step 3: Implement** (mirror `get_source_mgr`):

```rust
    /// Mutable access to the diagnostics manager, for the parser to report
    /// errors (C++ `getSourceMgr()` returns a non-const ref).
    pub fn get_source_mgr_mut(&mut self) -> &mut SourceErrorManager {
        self.sm
    }
```

- [ ] **Step 4: Run to verify it passes.** Run: `cargo test --manifest-path rust/Cargo.toml -p parser source_mgr_mut`. Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add rust/crates/parser/src/lexer/mod.rs
git commit -m "rust(parser): JSLexer::get_source_mgr_mut for parser-side diagnostics

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task C1: JSONParser construction + scalar `parse_value` + error routing

**Files:** Modify `rust/crates/parser/src/json/parser.rs`, re-export from `mod.rs`. Reference: `JSONParser.cpp:177-247`, `JSONParser.h:630-674`.

- [ ] **Step 1: Write failing tests** (scalars + the `-` error). NB: the lexer accepts single- and double-quoted strings in strict JS mode, matching the JSONParser tests:

```rust
#[cfg(test)]
mod parser_tests {
    use super::super::*;
    use atom_table::AtomTable;
    use bumpalo::Bump;
    use support::manager::SourceErrorManager;

    fn parse_ok<'a>(arena: &'a Bump, atoms: &'a AtomTable, src: &str) -> Option<&'a JSONValue<'a>> {
        // Mirrors `JSONParser parser(factory, src, sm); parser.parse()`.
        let f = arena.alloc(JSONFactory::new(arena, atoms));
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("json", src);
        let mut p = JSONParser::new(f, id, &mut sm, atoms, false);
        let r = p.parse();
        r
    }

    #[test]
    fn scalars() {
        let arena = Bump::new();
        let atoms = AtomTable::new();
        assert_eq!(parse_ok(&arena, &atoms, "true").and_then(|v| v.as_boolean()), Some(true));
        assert_eq!(parse_ok(&arena, &atoms, "false").and_then(|v| v.as_boolean()), Some(false));
        assert_eq!(parse_ok(&arena, &atoms, "null").map(|v| v.kind()), Some(JSONKind::Null));
        assert_eq!(parse_ok(&arena, &atoms, "42").and_then(|v| v.as_number()), Some(42.0));
        assert_eq!(parse_ok(&arena, &atoms, "-1.5").and_then(|v| v.as_number()), Some(-1.5));
        let s = parse_ok(&arena, &atoms, "'hi'").unwrap().as_string().unwrap();
        assert_eq!(atoms.bytes(s), b"hi");
    }

    #[test]
    fn lone_minus_errors() {
        // NegativeNumbers: "-" -> failure, error count 1.
        let arena = Bump::new();
        let atoms = AtomTable::new();
        let f = arena.alloc(JSONFactory::new(&arena, &atoms));
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer("json", "-");
        let mut p = JSONParser::new(f, id, &mut sm, &atoms, false);
        assert!(p.parse().is_none());
        assert_eq!(p.error_count(), 1);
    }
}
```

> **Lifetime note:** the factory is allocated *in the arena* (`arena.alloc(JSONFactory::new(..))`) so it shares lifetime `'a` with the values and can be referenced as `&'a JSONFactory<'a>` by the parser while the caller still reads values afterward. (The ported unittests in Task C3 follow the same shape.)

- [ ] **Step 2: Run to verify it fails.** Run: `cargo test --manifest-path rust/Cargo.toml -p parser parser_tests`. Expected: FAIL.

- [ ] **Step 3: Implement** `parser.rs` (parse + scalar `parse_value`; array/object stubs return an error for now, filled in C2/C3):

```rust
use atom_table::AtomTable;
use support::location::SMRange;
use support::manager::SourceErrorManager;
use support::diag::Subsystem;

use crate::lexer::{GrammarContext, JSLexer};
use crate::token::Token;
use crate::token_kinds::TokenKind;

use super::{JSONFactory, JSONValue, Prop};

/// JSON grammar uses `/` as division (never regexp).
const CTX: GrammarContext = GrammarContext::AllowDiv;

/// Port of `JSONParser` (JSONParser.h:630). Drives `JSLexer`; errors go through
/// the lexer's single `&mut SourceErrorManager`.
pub struct JSONParser<'a> {
    factory: &'a JSONFactory<'a>,
    lexer: JSLexer<'a>,
}

impl<'a> JSONParser<'a> {
    pub fn new(
        factory: &'a JSONFactory<'a>,
        buf_id: support::location::SourceId,
        sm: &'a mut SourceErrorManager,
        atoms: &'a AtomTable,
        convert_surrogates: bool,
    ) -> JSONParser<'a> {
        let lexer = JSLexer::new_with_convert_surrogates(buf_id, sm, atoms, CTX, convert_surrogates);
        JSONParser { factory, lexer }
    }

    pub fn error_count(&self) -> u32 {
        self.lexer.get_source_mgr().error_count()
    }

    /// JSONParser.h:666 — report at the current token's range.
    fn error(&mut self, msg: impl Into<String>) {
        let range: SMRange = self.lexer.token().source_range();
        self.lexer
            .get_source_mgr_mut()
            .error_at(range.start, Some(range), msg.into(), Subsystem::Parser);
    }

    fn cur(&self) -> &Token { self.lexer.token() }
    fn advance(&mut self) -> &Token { self.lexer.advance(CTX) }

    /// JSONParser.cpp:192 — parse the whole input.
    pub fn parse(&mut self) -> Option<&'a JSONValue<'a>> {
        self.advance();
        let res = self.parse_value()?;
        if self.lexer.get_source_mgr().error_count() != 0 {
            return None;
        }
        Some(res)
    }

    /// JSONParser.cpp:202 — parse a single value.
    fn parse_value(&mut self) -> Option<&'a JSONValue<'a>> {
        let mut needs_negation = false;
        match self.cur().kind() {
            TokenKind::string_literal => {
                let res = self.factory.get_string(self.cur().get_string_literal());
                self.advance();
                Some(res)
            }
            TokenKind::minus => {
                needs_negation = true;
                self.advance();
                if self.cur().kind() != TokenKind::numeric_literal {
                    self.error("No numeric literal following minus (-) token in value");
                    return None;
                }
                self.parse_number(needs_negation)
            }
            TokenKind::numeric_literal => self.parse_number(needs_negation),
            TokenKind::l_brace => {
                self.advance();
                self.parse_object()
            }
            TokenKind::l_square => {
                self.advance();
                self.parse_array()
            }
            TokenKind::rw_true => { self.advance(); Some(self.factory.get_boolean(true)) }
            TokenKind::rw_false => { self.advance(); Some(self.factory.get_boolean(false)) }
            TokenKind::rw_null => { self.advance(); Some(self.factory.get_null()) }
            _ => {
                self.error("JSON object or array expected");
                None
            }
        }
    }

    fn parse_number(&mut self, needs_negation: bool) -> Option<&'a JSONValue<'a>> {
        let v = self.cur().get_numeric_literal();
        let res = self.factory.get_number(if needs_negation { -v } else { v });
        self.advance();
        Some(res)
    }

    // Filled in C2 / C3.
    fn parse_array(&mut self) -> Option<&'a JSONValue<'a>> {
        self.error("expected ']'");
        None
    }
    fn parse_object(&mut self) -> Option<&'a JSONValue<'a>> {
        self.error("expected '}'");
        None
    }
}
```

Re-export from `mod.rs`: `pub use parser::JSONParser;`. Confirm the actual path of `SourceId` (`support::location::SourceId`) against the lexer's imports and fix if different.

- [ ] **Step 4: Run to verify it passes.** Run: `cargo test --manifest-path rust/Cargo.toml -p parser parser_tests::scalars parser_tests::lone_minus_errors`. Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add rust/crates/parser/src/json/
git commit -m "rust(parser): JSONParser construction + scalar parse_value + error routing

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task C2: `parse_array`

**Files:** Modify `rust/crates/parser/src/json/parser.rs`. Reference: `JSONParser.cpp:249-276`.

- [ ] **Step 1: Write failing test:**

```rust
    #[test]
    fn arrays() {
        let arena = Bump::new();
        let atoms = AtomTable::new();
        let a = parse_ok(&arena, &atoms, "[-1.0, -1, -0]").unwrap();
        let v = a.as_array().unwrap();
        assert_eq!(v.len(), 3);
        assert_eq!(v.at(0).as_number(), Some(-1.0));
        assert_eq!(v.at(2).as_number(), Some(-0.0));
        assert!(parse_ok(&arena, &atoms, "[]").unwrap().as_array().unwrap().is_empty());
        assert_eq!(parse_ok(&arena, &atoms, "[1,2,3,]").map(|_| ()), Some(())); // trailing comma allowed (mirror C++)
        assert!(parse_ok(&arena, &atoms, "[1,2").is_none()); // unterminated
    }
```

> Verify the trailing-comma behavior against `JSONParser.cpp:259-265`: after a comma, if the next token is `]` the loop breaks (so `[1,]` is accepted). Keep the test matching the C++.

- [ ] **Step 2: Run to verify it fails.** Run: `cargo test --manifest-path rust/Cargo.toml -p parser parser_tests::arrays`. Expected: FAIL.

- [ ] **Step 3: Implement** (replace the C2 stub) — faithful to `JSONParser.cpp:249-276`:

```rust
    /// JSONParser.cpp:249 — parse `[ ... ]` (the `[` already consumed).
    fn parse_array(&mut self) -> Option<&'a JSONValue<'a>> {
        let mut storage: Vec<&'a JSONValue<'a>> = Vec::new();
        if self.cur().kind() != TokenKind::r_square {
            loop {
                let val = self.parse_value()?;
                storage.push(val);
                if self.cur().kind() == TokenKind::comma {
                    self.advance();
                    if self.cur().kind() == TokenKind::r_square {
                        break;
                    }
                } else {
                    break;
                }
            }
            if self.cur().kind() != TokenKind::r_square {
                self.error("expected ']'");
                return None;
            }
        }
        self.advance(); // consume ']'
        Some(self.factory.new_array(&storage))
    }
```

- [ ] **Step 4: Run to verify it passes.** Run: `cargo test --manifest-path rust/Cargo.toml -p parser parser_tests::arrays`. Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add rust/crates/parser/src/json/parser.rs
git commit -m "rust(parser): JSONParser::parse_array

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task C3: `parse_object` + duplicate keys; port the 5 JSONParserTest cases

**Files:** Modify `rust/crates/parser/src/json/parser.rs`; create `rust/crates/parser/tests/json_parser_ported.rs`. Reference: `JSONParser.cpp:278-323`, `unittests/Parser/JSONParserTest.cpp`.

- [ ] **Step 1a: Implement `parse_object`** (faithful to `JSONParser.cpp:278-323`):

```rust
    /// JSONParser.cpp:278 — parse `{ ... }` (the `{` already consumed).
    fn parse_object(&mut self) -> Option<&'a JSONValue<'a>> {
        let mut pairs: Vec<Prop<'a>> = Vec::new();
        if self.cur().kind() != TokenKind::r_brace {
            loop {
                if self.cur().kind() != TokenKind::string_literal {
                    self.error("expected a string");
                    return None;
                }
                let key = self.factory.get_string(self.cur().get_string_literal());
                if self.advance().kind() != TokenKind::colon {
                    self.error("expected ':'");
                    return None;
                }
                self.advance();
                let val = self.parse_value()?;
                pairs.push((key, val));
                if self.cur().kind() == TokenKind::comma {
                    self.advance();
                    if self.cur().kind() == TokenKind::r_brace {
                        break;
                    }
                } else {
                    break;
                }
            }
            if self.cur().kind() != TokenKind::r_brace {
                self.error("expected '}'");
                return None;
            }
        }
        self.advance(); // consume '}'

        if let Some(dup) = self.factory.sort_props(&mut pairs) {
            let name = String::from_utf8_lossy(self.factory.atoms().bytes(dup)).into_owned();
            self.error(format!("key '{name}' is already present"));
            return None;
        }
        // Already sorted + dup-checked: build directly.
        self.factory.new_object_sorted(&pairs)
    }
```

Add a `new_object_sorted` to `factory.rs` (the `propsAreSorted=true` path of `JSONParser.cpp:138`):

```rust
    /// JSONParser.cpp:138 with propsAreSorted=true — props already sorted and
    /// dup-checked.
    pub fn new_object_sorted(&self, props: &[Prop<'a>]) -> Option<&'a JSONValue<'a>> {
        let keys: Vec<AtomBytes> = props.iter().map(|p| self.key_bytes(p.0)).collect();
        let cls = self.get_hidden_class(&keys);
        let values: Vec<&'a JSONValue<'a>> = props.iter().map(|p| p.1).collect();
        let values: &'a [&'a JSONValue<'a>] = self.arena.alloc_slice_copy(&values);
        Some(self.arena.alloc(JSONValue::Object(cls, values)))
    }
```

- [ ] **Step 1b: Write the ported unittests** in `tests/json_parser_ported.rs` (copyright header first). Port all 5 `JSONParserTest` cases faithfully. Skeleton + the first two; fill `NegativeNumbers`, `HiddenClassTest`, `EmitTest` analogously to their C++:

```rust
use atom_table::AtomTable;
use bumpalo::Bump;
use parser::json::{JSONFactory, JSONKind, JSONParser, JSONValue};
use support::manager::SourceErrorManager;

/// Mirror of the C++ setup: factory in the arena, parse a source string.
fn parse<'a>(arena: &'a Bump, atoms: &'a AtomTable, src: &str)
    -> (Option<&'a JSONValue<'a>>, u32)
{
    let f = arena.alloc(JSONFactory::new(arena, atoms));
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("json", src);
    let mut p = JSONParser::new(f, id, &mut sm, atoms, false);
    let r = p.parse();
    (r, p.error_count())
}

#[test]
fn smoke_test_1() {
    // JSONParserTest::SmokeTest1
    let src = "{\n  '6': null,\n  '1': null,\n  '2': null,\n  '3': null,\n  '4': null,\n  '5': null\n}";
    let arena = Bump::new();
    let atoms = AtomTable::new();
    let (parsed, _) = parse(&arena, &atoms, src);
    assert!(parsed.is_some());
}

#[test]
fn smoke_test_2() {
    // JSONParserTest::SmokeTest2 — parse + accessors + uniquing.
    let src = "{ 'key1' : 1, 'key2' : 'value2', 'key3' : {'nested1': true}, \"key4\" : [false, null, 'value2']}";
    let arena = Bump::new();
    let atoms = AtomTable::new();
    let f = arena.alloc(JSONFactory::new(&arena, &atoms));
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("json", src);
    let mut p = JSONParser::new(f, id, &mut sm, &atoms, false);
    let t1 = p.parse().unwrap();
    let o1 = t1.as_object().unwrap();
    assert_eq!(o1.size(), 4);
    assert_eq!(o1.count("key0", &atoms), 0);
    assert_eq!(o1.count("key1", &atoms), 1);
    assert!(std::ptr::eq(o1.get("key1", &atoms).unwrap(), f.get_number(1.0)));
    let value2 = o1.get("key2", &atoms).unwrap();
    assert!(std::ptr::eq(value2, f.get_string_str("value2"))); // uniquing
    // nested object
    let o2 = o1.get("key3", &atoms).unwrap().as_object().unwrap();
    assert!(std::ptr::eq(o2.get("nested1", &atoms).unwrap(), f.get_boolean(true)));
    // array, incl. shared 'value2' node
    let a1 = o1.get("key4", &atoms).unwrap().as_array().unwrap();
    assert_eq!(a1.len(), 3);
    assert!(std::ptr::eq(a1.at(0), f.get_boolean(false)));
    assert!(std::ptr::eq(a1.at(1), f.get_null()));
    assert!(std::ptr::eq(a1.at(2), value2));
}

#[test]
fn negative_numbers() {
    // JSONParserTest::NegativeNumbers
    let arena = Bump::new();
    let atoms = AtomTable::new();
    let (t1, _) = parse(&arena, &atoms, "[-1.0, -1, -0]");
    let a1 = t1.unwrap().as_array().unwrap();
    let expected = [-1.0f64, -1.0, -0.0];
    assert_eq!(a1.len(), expected.len());
    for (i, &e) in expected.iter().enumerate() {
        let actual = a1.at(i).as_number().unwrap();
        // distinguish -0.0 from 0.0 via bit pattern, as the C++ ASSERT_EQ does.
        assert_eq!(actual.to_bits(), e.to_bits(), "elem {i}");
    }
    // lone "-" -> failure, error count 1 (fresh manager).
    let (t2, errs) = parse(&arena, &atoms, "-");
    assert!(t2.is_none());
    assert_eq!(errs, 1);
}

#[test]
fn hidden_class_test() {
    // JSONParserTest::HiddenClassTest — same-shape objects share one class.
    let src = "[ {'key1': 1, 'key2': {'key2': 5, 'key1': 6}}, {'key2': 10, 'key1': 20}]";
    let arena = Bump::new();
    let atoms = AtomTable::new();
    let (t1, _) = parse(&arena, &atoms, src);
    let array = t1.unwrap().as_array().unwrap();
    assert_eq!(array.len(), 2);
    let o1 = array.at(0).as_object().unwrap();
    let o2 = o1.get("key2", &atoms).unwrap().as_object().unwrap();
    assert!(std::ptr::eq(o1.get_hidden_class(), o2.get_hidden_class()));
    let o3 = array.at(1).as_object().unwrap();
    assert!(std::ptr::eq(o1.get_hidden_class(), o3.get_hidden_class()));
}

#[test]
fn emit_test() {
    // JSONParserTest::EmitTest — parse then emit, compare bytes.
    use support::json_emitter::JSONEmitter;
    let src = "{ 'key1' : 1, 'key2' : 'value2', 'key3' : {'nested1': true}, \"key4\" : [false, null, 'value2']}";
    let arena = Bump::new();
    let atoms = AtomTable::new();
    let (t1, _) = parse(&arena, &atoms, src);
    let mut s = String::new();
    {
        let mut e = JSONEmitter::new(&mut s, false);
        t1.unwrap().emit_into(&mut e, &atoms);
    }
    assert_eq!(
        s,
        r#"{"key1":1,"key2":"value2","key3":{"nested1":true},"key4":[false,null,"value2"]}"#
    );
}
```

- [ ] **Step 2: Run to verify.** Run: `cargo test --manifest-path rust/Cargo.toml -p parser --test json_parser_ported`. Expected: FAIL first (object parsing), then PASS after Step 1a compiles.

- [ ] **Step 3:** (covered by 1a) — ensure `parse_object` + `new_object_sorted` compile and the ported tests pass.

- [ ] **Step 4: Full check.** Run: `cargo test --manifest-path rust/Cargo.toml -p parser && cargo build --manifest-path rust/Cargo.toml`. Expected: all pass, zero warnings.

- [ ] **Step 5: Commit.**

```bash
git add rust/crates/parser/src/json/ rust/crates/parser/tests/json_parser_ported.rs
git commit -m "rust(parser): JSONParser::parse_object + duplicate-key check; port 5 JSONParserTest cases

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Phase D — Differential oracle + corpus

### Task D1: C++ `json-parse-dump` tool (+ `--bench`)

**Files:** Create `tools/json-parse-dump/json-parse-dump.cpp`, `tools/json-parse-dump/CMakeLists.txt`; modify `tools/CMakeLists.txt`. Reference: `tools/js-lexer-dump/`.

**Output contract (both tools must match byte-for-byte):**
- Reads source from a file arg, or `-` for stdin.
- On success (parse returns a value and `getErrorCount()==0`): emit canonical JSON via `JSONEmitter` (non-pretty) to stdout, no trailing newline.
- On failure: print exactly `ERROR <errorCount>` + `\n` to stdout, nothing else.
- `--bench=N`: read once, parse `N` times (fresh `Allocator`+`JSONFactory` each iteration), do not emit; print `parsed <N>x, <ms> ms, <MB/s> MB/s` to stdout.
- `--convert-surrogates` toggles the flag (default off, matching the library + unittests).

- [ ] **Step 1: Write the tool.** Copyright header, then (sketch — model on `js-lexer-dump.cpp`):

```cpp
// Read input (file arg or stdin) into a MemoryBuffer.
// Parse mode:
//   SourceErrorManager sm;
//   JSLexer::Allocator alloc; JSONFactory factory(alloc);
//   JSONParser parser(factory, std::move(buf), sm, convertSurrogates);
//   auto v = parser.parse();
//   if (v && sm.getErrorCount()==0) { std::string s; raw_string_ostream OS(s);
//       JSONEmitter e(OS); (*v)->emitInto(e); OS.flush(); llvh::outs() << s; }
//   else llvh::outs() << "ERROR " << sm.getErrorCount() << "\n";
// Bench mode: loop N times timing only parser.parse() with a fresh
//   Allocator+JSONFactory per iteration; print timing.
```

Write the complete `.cpp` (argument parsing for `[--bench=N] [--convert-surrogates] <file|->`, the read helper, both modes). Use `std::chrono::steady_clock` for timing and compute MB/s from input size × N.

- [ ] **Step 2: CMake registration.** `tools/json-parse-dump/CMakeLists.txt`:

```cmake
add_hermes_tool(json-parse-dump json-parse-dump.cpp
  LINK_OBJLIBS hermesParser hermesSupport LLVHSupport)
```

Add to `tools/CMakeLists.txt`: `add_subdirectory(json-parse-dump)` (next to the `js-lexer-dump` entry).

- [ ] **Step 3: Build it.** Run: `cmake --build cmake-build-asan --target json-parse-dump`. Expected: builds. (If `cmake-build-asan/` is missing, configure per CLAUDE.md first.)

- [ ] **Step 4: Smoke-check the contract.**

```bash
printf '{ "a":1, "b":[true,null,-0] }' | cmake-build-asan/bin/json-parse-dump -
# expect: {"a":1,"b":[true,null,0]}
printf '[1,2' | cmake-build-asan/bin/json-parse-dump -
# expect: ERROR 1
```

- [ ] **Step 5: Commit.**

```bash
git add tools/json-parse-dump/ tools/CMakeLists.txt
git commit -m "tools: json-parse-dump differential/bench oracle (C++)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task D2: Rust `json_parse_dump` bin (+ `--bench`)

**Files:** Create `rust/crates/parser/src/bin/json_parse_dump.rs`; the `[[bin]]` was added in B0 (if not, add it now).

- [ ] **Step 1: Add the bin target** to `rust/crates/parser/Cargo.toml` (if not already):

```toml
[[bin]]
name = "json-parse-dump"
path = "src/bin/json_parse_dump.rs"
```

- [ ] **Step 2: Implement** the same contract as D1, in Rust:

```rust
// Copyright header.
// Args: [--bench=N] [--convert-surrogates] <file|->
// Read input; for parse mode:
//   let arena = Bump::new(); let atoms = AtomTable::new();
//   let f = JSONFactory::new(&arena, &atoms);
//   let mut sm = SourceErrorManager::new();
//   let id = sm.add_buffer("json", &src);
//   let mut p = JSONParser::new(&f, id, &mut sm, &atoms, convert_surrogates);
//   match p.parse() { Some(v) if p.error_count()==0 => {
//       let mut s = String::new(); { let mut e = JSONEmitter::new(&mut s, false);
//       v.emit_into(&mut e, &atoms); } print!("{s}"); }
//     _ => println!("ERROR {}", p.error_count()) }
// Bench mode: loop N times, fresh Bump+AtomTable+factory+manager each iter,
//   time only parse(); print "parsed {N}x, {ms} ms, {mbps} MB/s".
```

Write the full file. Use `std::time::Instant` for timing.

- [ ] **Step 3: Build + smoke-check parity with C++.**

```bash
cargo build --manifest-path rust/Cargo.toml -p parser --bin json-parse-dump
printf '{ "a":1, "b":[true,null,-0] }' | cargo run -q --manifest-path rust/Cargo.toml -p parser --bin json-parse-dump -- -
# expect identical to the C++ tool: {"a":1,"b":[true,null,0]}
```

- [ ] **Step 4: Confirm.** Compare both tools' output on the smoke inputs; must match byte-for-byte.

- [ ] **Step 5: Commit.**

```bash
git add rust/crates/parser/Cargo.toml rust/crates/parser/src/bin/json_parse_dump.rs
git commit -m "rust(parser): json-parse-dump Rust bin (differential/bench, mirrors C++)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

### Task D3: corpus + `json_differential.rs`

**Files:** Create `rust/crates/parser/tests/json_corpus/*.json`, `rust/crates/parser/tests/json_differential.rs`. Reference: `rust/crates/parser/tests/differential.rs`.

- [ ] **Step 1: Build the corpus.** Create `tests/json_corpus/` with valid + error inputs, e.g.:
  - `scalars.json`: `[true,false,null,0,-0,1,-1.5,42,1e21,1e-7,0.0001,123.45]`
  - `nested.json`: `{ "a":1, "b":{"c":[1,2,{"d":true}]}, "e":[] }`
  - `strings.json`: `["plain","esc:\"\\\/\b\f\n\r\t","astral:hi👋","unicode:哈"]`
  - `shapes.json`: `[{"k1":1,"k2":2},{"k2":3,"k1":4},{"k1":5,"k2":6}]` (hidden-class sharing)
  - `empty.json`: `[{},[],""]`
  - errors: `err_lone_minus.json` = `-`, `err_unterminated.json` = `[1,2`, `err_no_colon.json` = `{"a" 1}`, `err_garbage.json` = `@`
  Keep all strings valid UTF-8 (no lone surrogates).

- [ ] **Step 2: Write the differential test** mirroring `differential.rs`'s binary-resolution + `REQUIRE_DIFFERENTIAL` pattern:

```rust
use std::path::{Path, PathBuf};
use std::process::Command;

fn cpp_bin() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../cmake-build-asan/bin/json-parse-dump")
}
fn run(bin: &Path, src: &[u8]) -> String {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(bin).arg("-")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(src).unwrap();
    String::from_utf8(child.wait_with_output().unwrap().stdout).unwrap()
}

#[test]
fn json_corpus_differential() {
    let cpp = cpp_bin();
    if !cpp.exists() {
        if std::env::var_os("REQUIRE_DIFFERENTIAL").is_some() {
            panic!("REQUIRE_DIFFERENTIAL=1 but json-parse-dump not built at {cpp:?}");
        }
        eprintln!("skip: json-parse-dump not built at {cpp:?}");
        return;
    }
    // Build the Rust bin once and locate it (CARGO_BIN_EXE_ is set for bins in
    // this crate when running its own tests):
    let rust = env!("CARGO_BIN_EXE_json-parse-dump");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/json_corpus");
    let mut count = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") { continue; }
        let src = std::fs::read(&path).unwrap();
        let cpp_out = run(&cpp, &src);
        let rust_out = run(Path::new(rust), &src);
        assert_eq!(cpp_out, rust_out, "mismatch on {:?}", path.file_name().unwrap());
        count += 1;
    }
    eprintln!("json differential: {count} corpus files matched");
    assert!(count > 0);
}
```

(If `CARGO_BIN_EXE_json-parse-dump` isn't available from an integration test, fall back to locating the bin under `target/<profile>/` the way `differential.rs` does — check that file and mirror it.)

- [ ] **Step 3: Run it (forced).** Run: `cmake --build cmake-build-asan --target json-parse-dump && REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test json_differential -- --nocapture`. Expected: PASS, "N corpus files matched". Fix any mismatch by reading the diff (most likely number formatting or escaping) against the C++.

- [ ] **Step 4: Confirm whole-suite + zero warnings.** Run: `cargo test --manifest-path rust/Cargo.toml && cargo build --manifest-path rust/Cargo.toml`.

- [ ] **Step 5: Commit.**

```bash
git add rust/crates/parser/tests/json_corpus/ rust/crates/parser/tests/json_differential.rs
git commit -m "rust(parser): JSON differential corpus + byte-for-byte oracle test

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Phase E — JSONSharedValue (the one encapsulated `unsafe`)

### Task E1: owned-arena mode + `JSONSharedValue`

**Files:** Modify `rust/crates/parser/src/json/mod.rs` (+ possibly `factory.rs`). Reference: `JSONParser.h:676-698`.

- [ ] **Step 1: Write failing test:**

```rust
    #[test]
    fn shared_value_outlives_parse() {
        use std::rc::Rc;
        use bumpalo::Bump;
        // Build a value in an Rc<Bump>, wrap it, drop everything else, still read.
        let shared: JSONSharedValue = {
            let arena = Rc::new(Bump::new());
            let v: &JSONValue = arena.alloc(JSONValue::Number(3.5));
            // SAFETY contract exercised by the constructor.
            JSONSharedValue::new(v, arena.clone())
        };
        assert_eq!(shared.get().as_number(), Some(3.5));
    }
```

- [ ] **Step 2: Run to verify it fails.** Run: `cargo test --manifest-path rust/Cargo.toml -p parser shared_value`. Expected: FAIL.

- [ ] **Step 3: Implement** in `mod.rs`. This is the **only** hand-written `unsafe` in the component; document the invariant:

```rust
use std::rc::Rc;
use bumpalo::Bump;

/// A holder pairing a JSON value with the arena that backs it (port of
/// `JSONSharedValue`, JSONParser.h:676 — C++ uses `const JSONValue*` +
/// `shared_ptr<const Allocator>`). Self-referential, so the value pointer is
/// lifetime-erased and dereferenced through one encapsulated `unsafe`.
pub struct JSONSharedValue {
    /// Points into `*allocator`. Lifetime-erased to `'static`; never used at
    /// that lifetime — `get` re-ties it to `&self`.
    value: *const JSONValue<'static>,
    /// Keeps the arena (and therefore `*value`) alive.
    #[allow(dead_code)]
    allocator: Rc<Bump>,
}

impl JSONSharedValue {
    /// `value` MUST be allocated in `allocator`. The `Rc` keeps the arena alive
    /// for as long as this holder, so the pointer stays valid.
    pub fn new(value: &JSONValue<'_>, allocator: Rc<Bump>) -> JSONSharedValue {
        let value = value as *const JSONValue<'_> as *const JSONValue<'static>;
        JSONSharedValue { value, allocator }
    }

    /// The held value, re-tied to `&self`'s lifetime.
    pub fn get(&self) -> &JSONValue<'_> {
        // SAFETY: `self.allocator` (an `Rc<Bump>`) keeps the arena alive for at
        // least `&self`, and `self.value` was allocated in it (constructor
        // contract), so the pointer is valid and the returned reference cannot
        // outlive the arena.
        unsafe { &*(self.value as *const JSONValue<'_>) }
    }
}
```

> **Note on the parser crate's unsafe policy:** the crate already permits scoped `unsafe` (the lexer cursor). This is the second such site; keep it confined to `JSONSharedValue` with the SAFETY comment, consistent with the lexer's `cursor.rs`.

- [ ] **Step 4: Run to verify it passes + ASAN-clean.** Run: `cargo test --manifest-path rust/Cargo.toml -p parser shared_value`. Expected: PASS. (If a Miri pass is available locally, run it on this test; otherwise the invariant comment + corpus coverage suffice.)

- [ ] **Step 5: Commit.**

```bash
git add rust/crates/parser/src/json/mod.rs
git commit -m "rust(parser): JSONSharedValue (Rc<Bump> + one encapsulated unsafe deref)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Phase F — Benchmark

### Task F1: big-JSON generator + run the comparison

**Files:** Create a generator (`tools/gen-big-json/` C++ or a small Rust bin `rust/crates/parser/src/bin/gen_big_json.rs`). Keep `big.json` **uncommitted** (add to `.gitignore` if needed).

- [ ] **Step 1: Write a deterministic generator** that emits a multi-MB JSON array of records, e.g. `[{"id":<i>,"name":"item-<i>","price":<float>,"tags":["a","b","c"],"active":<bool>,"nested":{"x":<i>,"y":<i>}}, ...]` for `i in 0..N` (pick `N` so the file is ~5-20 MB). A Rust bin is simplest:

```rust
// rust/crates/parser/src/bin/gen_big_json.rs — Copyright header.
// Args: <count> -> prints a JSON array of <count> records to stdout.
// Deterministic (index-derived values; no RNG needed).
```

Add a `[[bin]] name="gen-big-json"` target.

- [ ] **Step 2: Generate the file.**

```bash
cargo run -q --manifest-path rust/Cargo.toml -p parser --release --bin gen-big-json -- 100000 > /tmp/big.json
ls -la /tmp/big.json   # confirm multi-MB
```

- [ ] **Step 3: Build both tools in release-ish config and run the bench.**

```bash
# C++ (ASan tree is fine for correctness, but for a fair speed number prefer a Release build of the tool):
cmake --build cmake-build-asan --target json-parse-dump
cmake-build-asan/bin/json-parse-dump --bench=50 /tmp/big.json

cargo build --manifest-path rust/Cargo.toml -p parser --release --bin json-parse-dump
./rust/target/release/json-parse-dump --bench=50 /tmp/big.json
```

> **Fairness note:** the default C++ build here is ASan+`-O1` (per CLAUDE.md) while Rust `--release` is `-O3`-equivalent. For an apples-to-apples speed comparison, also build a Release (non-ASan) C++ tool (`cmake -B cmake-build-release -DCMAKE_BUILD_TYPE=Release` then `--target json-parse-dump`) and benchmark that. Record **both** numbers and note the build configs.

- [ ] **Step 4: Record results** in a short note appended to the roadmap (Task G1 below) — N, file size, ms, MB/s for C++ (Release) vs Rust (release), plus the build configs. Do not over-interpret; it's a first datapoint.

- [ ] **Step 5: Commit** the generator (not the data).

```bash
echo '/big.json' >> rust/crates/parser/.gitignore  # if generating into the crate dir
git add rust/crates/parser/src/bin/gen_big_json.rs rust/crates/parser/Cargo.toml
git commit -m "rust(parser): deterministic big-JSON generator for benchmarking

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Phase G — Wrap-up

### Task G1: roadmap update + capstone review

- [ ] **Step 1: Update `doc/superpowers/RustPortRoadmap.md`** — add a "JSONParser ✅ COMPLETE" row/section (entire public surface ported: value model + factory + parser + emitter + shared value; differential + 5 ported unittests + 11 emitter tests; the benchmark datapoint with build configs). Note any deviations (the deliberate fat-enum layout; `getAllocator`→`arena()`).

- [ ] **Step 2: Capstone review** of the WHOLE component (per the established workflow — it caught real lexer bugs the per-phase reviews missed). Independently: build clean, run the full suite + forced differential, and read the Rust against the C++ for: the `parse_value`/`parse_array`/`parse_object` control flow (esp. trailing-comma + error branches), uniquing/hidden-class sharing semantics, emitter escaping + `number_to_string` edge cases, and the `JSONSharedValue` `unsafe` invariant. File and fix anything found before declaring complete.

- [ ] **Step 3: Final verification.**

```bash
cargo test --manifest-path rust/Cargo.toml            # whole workspace, zero failures
cargo build --manifest-path rust/Cargo.toml           # zero warnings
cargo clippy --manifest-path rust/Cargo.toml -p parser # only pre-existing faithful-C-idiom lints
cmake --build cmake-build-asan --target json-parse-dump
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test json_differential -- --nocapture
```

- [ ] **Step 4: Commit** the roadmap update + any capstone fixes.

```bash
git add -A
git commit -m "doc(rust): mark JSONParser port complete; capstone fixes

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Self-review checklist (done while writing)

- **Spec coverage:** value model §1 → B1/B3; factory/uniquing/hidden-classes §2 → B2/B3; parser §3 → C1/C2/C3; JSONEmitter §4 → A1/A2/A3; JSONSharedValue §5 → E1; validation §6 → C3 (ported tests) + D1/D2/D3 (differential); benchmark §7 → D1/D2 (`--bench`) + F1; "what lives where" → reflected in B2/B3 (arena vs AtomTable vs factory maps). ✓
- **Deviations recorded:** fat-enum layout (spec §faithfulness); `getAllocator`→`arena()`; object iteration in sorted (not insertion) order — faithful to C++, noted in B3.
- **Type consistency:** `JSONValue<'a>`, `JSONFactory<'a>`, `ObjectView`/`ArrayView`, `Prop<'a>`, `emit_into(&mut JSONEmitter, &AtomTable)`, `new_object(&mut [Prop])`/`new_object_sorted(&[Prop])`/`new_array(&[..])`, `get_string`/`get_string_str`/`get_number`/`get_boolean`/`get_null`, `JSONParser::new(factory, buf_id, &mut sm, atoms, convert_surrogates)` used consistently across tasks. ✓
- **Open verification flags for the implementer** (cheap, do at the cited step, not placeholders): exact module path of `SourceId` (C1); `CARGO_BIN_EXE_*` availability vs the `differential.rs` target-dir fallback (D3); `SourceErrorManager::new()` constructor name as used in existing lexer tests (C0/C1).
