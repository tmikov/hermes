#![warn(missing_docs)]
//! A Rust port of the Hermes JavaScript front end — lexer and parser.
//!
//! Faithful 1:1 port of the C++ `JSLexer` and `JSParserImpl`, validated
//! byte-for-byte against `hermesc -dump-ast` over a per-dialect corpus. The
//! output is the ESTree AST of the `ast` crate. Every dialect the C++ parser
//! supports is complete and covered by that differential gate: ECMAScript,
//! the Flow type grammar, TypeScript, and JSX. The three non-standard ones
//! are opt-in through the same `ast::context::Context` flags as in the C++
//! (`parse_flow` and its four extension flags, `parse_ts`, `parse_jsx`).
//!
//! The pieces a consumer touches:
//! - [`js::JSParserImpl`] — the recursive-descent parser; `new` + `parse`
//!   returns the `Program` node, or `None` after a reported error.
//! - [`lexer::JSLexer`] — the lexer, usable on its own; it reports through a
//!   `support::manager::SourceErrorManager` and interns into an `AtomTable`.
//! - [`token::Token`] and [`token_kinds::TokenKind`] — the token surface, the
//!   latter generated from `include/hermes/Parser/TokenKinds.def` order.
//! - [`js::ParserPass`] — `FullParse` (eager), plus the `PreParse`/`LazyParse`
//!   pair that indexes function bodies in one scan and defers parsing them.
//! - [`json`] — the separate `JSONParser` port (a distinct grammar sharing the
//!   same lexer), with the uniquing/hidden-class `JSONFactory`.
//!
//! The remaining modules are the lexer's building blocks and are public
//! because the lexer's own API exposes them: [`cursor`] (the scan cursor),
//! [`number`] (numeric-literal conversion), [`utf8`] (the UTF-8/UTF-16
//! conversions the C++ keeps in `Support`), and [`html_entities`] (the JSX
//! entity table generated from `HTMLEntities.def`).
//!
//! See `rust/ARCHITECTURE.md` for the design rationale and
//! doc/superpowers/specs/2026-06-06-js-parser-design.md for the port spec.

pub mod cursor;
pub mod html_entities;
pub mod js;
pub mod json;
pub mod lexer;
pub mod number;
pub mod token;
pub mod token_kinds;
pub mod utf8;
