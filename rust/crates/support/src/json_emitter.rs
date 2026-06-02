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
