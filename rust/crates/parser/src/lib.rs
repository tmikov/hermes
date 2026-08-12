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
//! # Quickstart
//!
//! ```
//! use ast::node::Node;
//! use parser::{parse, ParseFlags};
//!
//! let flags = ParseFlags::default();
//! let mut parsed = parse("1 + 2;", flags).expect("parse error");
//!
//! // The AST lives in an arena owned by `parsed`; read it under a lock.
//! let statements = parsed.with_program(|_gc, program| match program {
//!     Node::Program(p) => p.body.iter().count(),
//!     _ => unreachable!("the root of a parse is always a Program"),
//! });
//! assert_eq!(statements, 1);
//!
//! // Or dump it the way `hermesc -dump-ast` does.
//! let json = parsed.to_estree_json(false);
//! assert!(json.starts_with(r#"{"type":"Program""#));
//! ```
//!
//! The pieces a consumer touches:
//! - [`parse`] / [`parse_named`] returning [`ParsedJS`] — the convenience
//!   façade in [`facade`], which assembles an `ast::context::Context`, a
//!   `SourceErrorManager`, a [`lexer::JSLexer`] and a [`js::JSParserImpl`]
//!   into one call. It adds no behavior; anything it does not expose is
//!   reachable by driving those pieces directly.
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
//! The remaining modules are the lexer's own building blocks: [`cursor`] (the
//! scan cursor), [`number`] (numeric-literal conversion), [`utf8`] (the
//! UTF-8/UTF-16 conversions the C++ keeps in `Support`), and
//! [`html_entities`] (the JSX entity table generated from `HTMLEntities.def`).
//! Only the port's internals call them, and no public signature in this crate
//! mentions them; they are public incidentally rather than by design, and may
//! be demoted to `pub(crate)` in a future release.
//!
//! See `rust/ARCHITECTURE.md` for the design rationale and
//! doc/superpowers/specs/2026-06-06-js-parser-design.md for the port spec.

#![warn(missing_docs)]

pub mod cursor;
pub mod facade;
pub mod html_entities;
pub mod js;
pub mod json;
pub mod lexer;
pub mod number;
pub mod token;
pub mod token_kinds;
pub mod utf8;

pub use facade::{parse, parse_named, ParseError, ParseFlags, ParsedJS};
