/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

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
    // 3. Negative: prepend '-' and operate on the absolute value.
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
        out.push_str(&exp_val.to_string());
    }
    out
}

/// A single object (Dictionary or Array) being emitted.
/// Port of `JSONEmitter::State` (JSONEmitter.h:170).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum StateType {
    Dict,
    Array,
}

#[derive(Debug)]
struct State {
    /// Whether this is a dictionary or array.
    ty: StateType,
    /// Whether a comma is needed before the next value.
    needs_comma: bool,
    /// Whether we are a dictionary expecting a key next.
    needs_key: bool,
    /// Whether we expect a value (after a key in a dict).
    needs_value: bool,
    /// Whether the dict/array is still empty.
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

    /// Like `emit_key` but takes UTF-16 code units (for WTF-8 keys that are not
    /// valid UTF-8). Port-compatible with C++ `emitKey` -> `primitiveEmitString`
    /// which decodes WTF-8 and escapes per code unit.
    pub fn emit_key_u16(&mut self, key: &[u16]) {
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
        self.out.push('"');
        for &unit in key {
            self.emit_one_escaped_unit(unit);
        }
        self.out.push('"');
        self.out.push(':');
        if self.pretty {
            self.out.push(' ');
        }
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
        // Note: the state fields are set before pretty_new_line() (which reads only
        // `pretty`/`indent`, not `states`); this reordering vs the C++ is output-identical.
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(s, r#"{"ha":"\u54c8","gClef":"\ud834\udd1e","wave":"hi\ud83d\udc4b"}"#);
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

    #[test]
    fn null_value() {
        assert_eq!(emit(|j| j.emit_null_value()), "null");
    }

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
        // EmitUTF16: u"hi\xd83d\xdc4b" -> each surrogate unit escaped as \uXXXX
        let units: Vec<u16> = vec![b'h' as u16, b'i' as u16, 0xd83d, 0xdc4b];
        let mut s = String::new();
        {
            let mut j = JSONEmitter::new(&mut s, false);
            j.open_dict();
            j.emit_key("str"); j.emit_u16(&units);
            j.close_dict();
        }
        assert_eq!(s, r#"{"str":"hi\ud83d\udc4b"}"#);
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

    #[test]
    fn emit_u16_astral_and_lone_surrogate() {
        // astral U+10000 encoded as surrogate pair [0xD800,0xDC00]: each unit
        // is > 0x7F so emit_one_escaped_unit emits each as \uXXXX ->
        // key "\ud800\udc00"; lone surrogate 0xD800 value -> "\ud800".
        let mut s = String::new();
        {
            let mut j = JSONEmitter::new(&mut s, false);
            j.open_dict();
            j.emit_key_u16(&[0xD800, 0xDC00]); // key = surrogate pair for U+10000
            j.emit_u16(&[0xD800]);             // lone surrogate value
            j.close_dict();
        }
        assert_eq!(s, "{\"\\ud800\\udc00\":\"\\ud800\"}");
    }

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
            (1e-6, "0.000001"),     // n=-5, fixed boundary
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
