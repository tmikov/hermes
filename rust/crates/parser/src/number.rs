//! Numeric-literal conversion primitives for the JS lexer, ported from
//! include/hermes/Support/Conversions.h. The decimal/real path uses Rust std's
//! correctly-rounded `str::parse::<f64>()` (the same fast_float algorithm the
//! C++ lexer uses) — no FFI, no third-party crate.
