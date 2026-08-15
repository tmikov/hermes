/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#![forbid(unsafe_code)]

//! Hermes semantic analysis (Rust port).
//!
//! Parsing gives you a tree; this crate tells you what the names in it mean.
//! It builds the lexical scope tree, creates a `Decl` for every binding,
//! resolves every identifier to the declaration it names, runs the validation
//! the C++ `SemanticResolver` is responsible for, and — on the compile path —
//! performs the AST rewrites sema is allowed to make. The result is a
//! [`sem_context::SemContext`].
//!
//! # Quickstart
//!
//! ```
//! use hermes_parser::ast::node::Node;
//! use hermes_parser::{parse, ParseFlags};
//! use hermes_sema::sem_context::DeclKind;
//!
//! let parsed = parse("var x = 1; x;", ParseFlags::default()).expect("parse");
//! let mut resolved = hermes_sema::resolve(parsed).expect("resolve");
//!
//! // The reference `x` binds to the declaration `var x`.
//! let (name, decl) = resolved.with_program(|gc, program, sem| {
//!     let body = match program {
//!         Node::Program(p) => p.body,
//!         _ => unreachable!("the root of a parse is always a Program"),
//!     };
//!     let expr = match body.iter().last().unwrap() {
//!         Node::ExpressionStatement(e) => e.expression,
//!         _ => unreachable!(),
//!     };
//!     match expr {
//!         // `name` is an interned atom, so read it through the generated
//!         // `name_str` accessor, which borrows the atom table under `gc`.
//!         Node::Identifier(id) => {
//!             (id.name_str(gc).to_string(), sem.get_expression_decl(id))
//!         }
//!         _ => unreachable!(),
//!     }
//! });
//! assert_eq!(name, "x");
//! let decl = decl.expect("`x` must resolve");
//!
//! // A top-level `var` in a script declares a property of the global object.
//! let kind = resolved.sem_context().decl(decl).kind;
//! assert_eq!(kind, DeclKind::GlobalProperty);
//! ```
//!
//! `crates/sema/examples/print_bindings.rs` is that query applied to every
//! identifier in a file — the canonical use of the two crates — and it also
//! shows how a [`hermes_parser::ast::visitor::Visitor`] can hold the
//! `&GCLock` it needs for `name_str` in a field. (Give the lock its own
//! lifetime parameters there; `GCLock<'ast, 'ctx>` is invariant in `'ast`, so
//! reusing the visitor's `'gc` for it does not compile.)
//!
//! Text in the AST is interned rather than owned: `name_str` and the
//! `try_<field>_str` / `<field>_str_lossy` pair for string *values* are
//! documented in [`hermes_parser`]'s quickstart, and
//! [`hermes_parser::ast::context::GCLock::bytes`] remains the exact-bytes
//! accessor.
//!
//! ## The compile path, and the `-dump-sema` text
//!
//! [`resolve()`] above is the *parser* path: no ambient declarations, no AST
//! rewrites — what a tooling embedder wants. [`resolve_for_compile`] is the
//! other entry point, the one `hermesc` itself uses: it declares the standard
//! globals and performs sema's rewrites. [`ResolvedJS::to_sema_dump`] then
//! renders the result in `hermesc -dump-sema`'s exact format.
//!
//! ```
//! use hermes_parser::{parse, ParseFlags};
//! use hermes_sema::{resolve_for_compile, CompileOptions};
//!
//! let parsed = parse("function f() { return 1; }", ParseFlags::default())
//!     .expect("parse");
//! let mut resolved =
//!     resolve_for_compile(parsed, &CompileOptions::default()).expect("resolve");
//!
//! // Bytes, not a `String`: an identifier can be an unpaired surrogate, which
//! // the dumper writes as WTF-8.
//! let dump = resolved.to_sema_dump();
//! let text = String::from_utf8_lossy(&dump);
//! assert!(text.starts_with("SemContext\n"));
//! // `Math` and friends are declared because this is the compile path.
//! assert!(text.contains("'Math' UndeclaredGlobalProperty"));
//! ```
//!
//! `crates/sema/examples/resolve_and_dump.rs` is this plus argument handling
//! and a `--summary` mode that walks the tree with the visitor instead of
//! dumping it.
//!
//! The pieces a consumer touches:
//! - [`resolve()`] / [`resolve_for_parser`] / [`resolve_for_compile`] returning
//!   [`ResolvedJS`] — the convenience façade over `hermes_parser`'s
//!   [`hermes_parser::ParsedJS`]. It adds no analysis; anything it does not
//!   expose is reachable by calling [`resolve::resolve_ast`] /
//!   [`resolve::resolve_ast_for_parser`] directly, the way
//!   `crates/tools/src/bin/sema_dump.rs` does.
//! - [`sem_context::SemContext`] — the results: `Decl`, `LexicalScope`,
//!   `FunctionInfo`, and the side tables keyed by AST node.
//! - [`ResolvedJS::to_sema_dump`] — the `hermesc -dump-sema` text, which is
//!   what this crate's differential gate compares byte-for-byte. (The
//!   printers behind it live in [`dump`] and [`dump_context`].)
//!
//! The façade function [`resolve()`] and the module [`mod@resolve`] share a
//! name, as `parse` would if the parser had a `parse` module: they are in
//! different namespaces, so `hermes_sema::resolve(parsed)` calls the function
//! and `hermes_sema::resolve::resolve_ast` names the entry point inside the
//! module. Both spellings are used in the examples above.
//!
//! # Stability
//!
//! This crate is pre-1.0 and the port it wraps is not finished (see the scope
//! note below), so its ten public modules are not all equally settled. The
//! **stable** surface — what 0.1.x means to keep source-compatible — is:
//!
//! - the façade: [`resolve()`], [`resolve_for_parser`], [`resolve_for_compile`],
//!   [`ResolvedJS`], [`ResolveError`], [`CompileOptions`],
//!   [`GlobalDefinitions`];
//! - the two low-level entry points in [`mod@resolve`]:
//!   [`resolve::resolve_ast`] and [`resolve::resolve_ast_for_parser`];
//! - the result model: [`sem_context`] and [`ids`].
//!
//! The other seven modules — [`resolver`], [`decl_collector`], [`ast_eval`],
//! [`dump`], [`dump_context`], [`libhermes`], [`keywords`] — are **advanced /
//! port-internal**. They are `pub` because the port's own tools (`sema-dump`)
//! and integration tests drive them directly, not because their shape is
//! settled. They may change, or be demoted to `pub(crate)`, in a 0.x bump.
//! Each says so in its own module doc.
//!
//! # Scope of the port
//!
//! The eager, untyped (non-FlowChecker) path of `lib/Sema` is ported and
//! gated byte-for-byte against `hermesc -dump-sema`. Still unported, and loud
//! rather than silent where they are reached:
//!
//! - the `$SHBuiltin` module protocol (`visitModuleFactory` / `visitModuleExport`
//!   / `visitModuleImport` and `resolveCommonJSAST`) — the three branches in
//!   `resolver/calls.rs` panic with a pointer at the C++ lines;
//! - the lazy-compilation and `eval` entry points (`resolveASTLazy`,
//!   `resolveASTInScope`), which need `SemContext`'s parent/child tree and
//!   shared binding table — see [`mod@resolve`]'s module doc;
//! - `visitProgram`'s `SaveAndRestore` of `globalScope_`
//!   (`SemanticResolver.cpp:216-217`): the assignment is ported, the restore
//!   is not. It only becomes observable once `Program` can recur, which is
//!   the same lazy/`eval` work as the previous bullet — see the comment at
//!   the site in `resolver/mod.rs`;
//! - the FlowChecker itself, which is a separate C++ component and not part
//!   of this crate.
//!
//! AST types (`Node`, `Visitor`, `GCLock`) come from `hermes_parser::ast`,
//! which is the same `hermes-ast` crate this one is built on, so depending on
//! `hermes-parser` and `hermes-sema` is enough.
//!
//! Source of truth in the C++ tree:
//! - `include/hermes/Sema/SemContext.h` (`Decl`, `LexicalScope`,
//!   `FunctionInfo` — see `hermes_sema::ids`)
//! - `include/hermes/AST/Context.h` (`Keywords`, line 168) and
//!   `include/hermes/AST/Keywords.def` (see `hermes_sema::keywords`)
//! - `lib/Sema/SemanticResolver.cpp` / `include/hermes/Sema/SemResolve.h`
//!   (the validator/resolver, plus the two `resolve` entry points the façade
//!   wraps)

#![warn(missing_docs)]

pub mod ast_eval;
// Private for the same reason its C++ counterpart is declared in the internal
// `lib/Sema/SemanticResolver.h` rather than in `SemResolve.h` — see the
// module's own doc.
mod check_implicit_return;
pub mod decl_collector;
pub mod dump;
pub mod dump_context;
pub mod ids;
pub mod keywords;
pub mod libhermes;
mod linearize;
pub mod resolve;
pub mod resolver;
pub mod sem_context;

/// The façade module is private: its items are re-exported here so each has
/// exactly one path in the docs, matching `hermes_parser::facade`.
mod facade;

pub use facade::{
    resolve, resolve_for_compile, resolve_for_parser, CompileOptions,
    GlobalDefinitions, ResolveError, ResolvedJS,
};

/// One recorded diagnostic, re-exported because it appears in the façade's
/// signatures ([`ResolveError::diagnostics`], [`ResolvedJS::diagnostics`]).
/// Render one with `hermes_support::render::render_diagnostic`. It is the
/// same type `hermes_parser::ResolvedDiagnostic` names.
pub use hermes_support::diag::ResolvedDiagnostic;

/// The source manager owning the parsed buffers, re-exported because
/// [`ResolvedJS::source_manager`] returns one.
pub use hermes_support::manager::SourceErrorManager;
