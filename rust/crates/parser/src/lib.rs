//! A Rust port of the Hermes JavaScript front end — lexer and parser.
//!
//! Faithful 1:1 port of the C++ `JSLexer` and `JSParserImpl`, validated
//! byte-for-byte against `hermesc -dump-ast` over a per-dialect corpus. The
//! output is the ESTree AST of the `ast` crate. Every dialect the C++ parser
//! supports is complete and covered by that differential gate: ECMAScript,
//! the Flow type grammar, TypeScript, and JSX. The three non-standard ones
//! are opt-in through the same `hermes_ast::context::Context` flags as in the C++
//! (`parse_flow` and its four extension flags, `parse_ts`, `parse_jsx`).
//!
//! # Quickstart
//!
//! ```
//! use hermes_parser::ast::node::Node;
//! use hermes_parser::{parse, ParseFlags};
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
//! ## Names and string values: from atom to `&str`
//!
//! Text in the AST is *interned*: `id.name` is a `Cell<NodeLabel>`, an index
//! into the arena's atom table, not a `String`. Every text field has a
//! generated accessor that does the lookup against the [`ast::context::GCLock`],
//! returning a `&str` that borrows the table's bytes — no allocation:
//!
//! ```
//! use hermes_parser::ast::node::Node;
//! use hermes_parser::{parse, ParseFlags};
//!
//! let src = r#"function greet(who) { return "hi"; }"#;
//! let mut parsed = parse(src, ParseFlags::default()).expect("parse error");
//!
//! let names = parsed.with_program(|gc, program| {
//!     let Node::Program(p) = program else { unreachable!() };
//!     let Some(Node::FunctionDeclaration(f)) = p.body.iter().next()
//!     else { unreachable!() };
//!     let Some(Node::Identifier(id)) = f.id else { unreachable!() };
//!     let Some(Node::Identifier(param)) = f.params.iter().next()
//!     else { unreachable!() };
//!     // `<field>_str` for a name-like field…
//!     (id.name_str(gc).to_string(), param.name_str(gc).to_string())
//! });
//! assert_eq!(names, ("greet".to_string(), "who".to_string()));
//!
//! let mut parsed = parse(r#""hi";"#, ParseFlags::default()).expect("parse");
//! let value = parsed.with_program(|gc, program| {
//!     let Node::Program(p) = program else { unreachable!() };
//!     let Some(Node::ExpressionStatement(st)) = p.body.iter().next()
//!     else { unreachable!() };
//!     let Node::StringLiteral(s) = st.expression else { unreachable!() };
//!     // …but `try_<field>_str` for a string *value*, which can legally be an
//!     // unpaired surrogate and then has no UTF-8 form at all.
//!     s.try_value_str(gc).map(str::to_string)
//! });
//! assert_eq!(value.as_deref(), Some("hi"));
//! ```
//!
//! The split is deliberate. Name-like fields (identifiers, operators,
//! keyword-like kinds) get a plain `<field>_str` that substitutes U+FFFD in
//! the case that should not arise — the lexer rejects an identifier containing
//! an unpaired surrogate. String-literal values get `try_<field>_str`
//! returning `Option<&str>` and an explicit `<field>_str_lossy`, because a
//! lone surrogate there is a legal JS value, and silently replacing it would
//! corrupt the program a codegen tool round-trips. An astral character such as
//! `"😀"` is *not* the `None` case: it is stored as a WTF-8 surrogate pair and
//! both accessors fold it back into the character. For the exact stored bytes,
//! use [`ast::context::GCLock::bytes`]; [`ast::context::GCLock::bytes_str_lossy`]
//! and [`ast::context::GCLock::try_bytes_str`] are the same conversions for an
//! atom you already hold.
//!
//! `crates/sema/examples/print_bindings.rs` puts this together with a
//! `Visitor` walk and name resolution.
//!
//! The pieces a consumer touches:
//! - [`parse`] / [`parse_named`] returning [`ParsedJS`] — the convenience
//!   façade, which assembles an [`hermes_ast::context::Context`], a
//!   `SourceErrorManager`, a [`lexer::JSLexer`] and a [`js::JSParserImpl`]
//!   into one call. It adds no behavior; anything it does not expose is
//!   reachable by driving those pieces directly.
//! - [`ast`] — the AST crate, re-exported so that depending on this crate
//!   alone is enough to name [`hermes_ast::node::Node`], walk with
//!   [`hermes_ast::visitor::Visitor`], or drive [`hermes_ast::dump`] by hand.
//! - [`js::JSParserImpl`] — the recursive-descent parser; `new` + `parse`
//!   returns the `Program` node, or `None` after a reported error.
//! - [`lexer::JSLexer`] — the lexer, usable on its own; it reports through a
//!   `hermes_support::manager::SourceErrorManager` and interns into an `AtomTable`.
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
pub mod html_entities;
pub mod js;
pub mod json;
pub mod lexer;
pub mod number;
pub mod token;
pub mod token_kinds;
pub mod utf8;

/// The façade module is private: its items are re-exported here so each has
/// exactly one path in the docs.
mod facade;

pub use facade::{parse, parse_named, ParseError, ParseFlags, ParsedJS};

/// The AST crate, re-exported under the short name `ast`, so the public path
/// is `hermes_parser::ast`. Parsing hands back AST types, so a consumer needs
/// them; re-exporting keeps this crate the only dependency they must declare.
/// `ast::node::Node`, `ast::visitor`, `ast::context::GCLock` and `ast::dump`
/// are the pieces the façade's signatures mention. The same items are also
/// reachable as `hermes_ast::…` by depending on that crate directly.
pub use hermes_ast as ast;

/// One recorded diagnostic, re-exported because it appears in the façade's
/// signatures ([`ParseError::diagnostics`], [`ParsedJS::diagnostics`]).
/// Render one with `hermes_support::render::render_diagnostic`.
pub use hermes_support::diag::ResolvedDiagnostic;
