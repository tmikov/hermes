/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! A Rust port of juno's AST -> JavaScript generator (`gen_js.rs`), which
//! turns a Hermes AST back into JavaScript/Flow/TypeScript source text.
//!
//! Ported from `unsupported/juno/crates/juno/src/gen_js.rs` (frozen, 4174
//! lines): the crate skeleton (Task 1), the core machinery (Task 2) —
//! [`Opt`] and its companion option types, [`GenJS`] with its output
//! primitives, and the top-level [`generate`] entry point — the precedence
//! table and parenthesization decision (Task 3, `precedence`), every
//! printing arm (Tasks 4-13, `arms`): literals and identifiers, ES
//! expressions and statements, functions and classes, modules, JSX, the Flow
//! type and declaration grammars, the 53 ES/Flow kinds juno's generator
//! predates, and the 46 TypeScript kinds — sema-informed identifier
//! annotation (Task 14, `annotate.rs`) — the round-trip test harness and the
//! 421-file Tier 1 corpus gate (Task 15, `tests/corpus.rs`) — the adversarial
//! parenthesization matrix and wide sweep (Task 17, `tests/paren_matrix.rs`,
//! `MANIFEST.md`) — and this crate's front door, [`to_js`] (Task 16).
//!
//! # Features
//!
//! `annotate` (**off by default**) enables `Annotation::Sem`, which prints
//! each identifier's resolved binding inline from a completed `hermes_sema`
//! analysis. It is the crate's only reason to depend on `hermes-sema`, and is
//! off by default because it is a debugging aid: nothing on the ordinary
//! generation path touches sema, so leaving it on would make every consumer
//! compile `hermes-sema` for a feature most never enable. `Annotation<'s>`
//! and [`Opt`]`<'s>` keep the same arity in both states, so a signature
//! written against one compiles under the other.
//!
//! **Every one of the 271 [`hermes_ast::node::NodeKind`]s now has a named
//! arm in [`dispatch::GenJS::gen_node`]**, and Task 13 deleted the temporary
//! catch-all the earlier tasks leaned on, so exhaustiveness is a compile-time
//! property: a kind added to the AST without an arm here is a build failure,
//! not a silent runtime error. The only kinds that report
//! [`GenJsError::UnsupportedKind`] are the 8 that have no JS source syntax at
//! all — the 7 internal ones (the cover-grammar group, `SHBuiltin`,
//! `ImplicitCheckedCast`) plus `TemplateElement`, which is only ever printed
//! inline by its `TemplateLiteral`.
//!
//! # Quickstart
//!
//! ```
//! use hermes_gen_js::{to_js, Opt};
//! use hermes_parser::{parse, ParseFlags};
//!
//! let mut parsed = parse("let x=1;function f(y){return x+y}", ParseFlags::default())
//!     .expect("parse");
//! let js = to_js(&mut parsed, Opt::default()).expect("generate");
//! assert_eq!(js, "let x = 1;\nfunction f(y) {\n  return x + y;\n}\n");
//! ```
//!
//! `crates/gen_js/examples/print_js.rs` runs the same two steps over an
//! arbitrary source string and prints the result in both [`Pretty`] modes.
//!
//! # There is no C++ oracle for this component
//!
//! Every other crate in this port was validated by byte-comparing its output
//! against a C++ binary built from the same source tree. This one cannot be:
//! `lib/AST2JS/AST2JS.cpp` exists (1239 lines, ES-only, no type-annotation
//! sites), but it is not used in Hermes's compile or execution pipeline — its
//! only caller is the `hermesc -dump-js` debug flag — was not extensively
//! tested, and was **not the port source** — this crate is a port of juno's
//! `gen_js.rs`, not of `AST2JS.cpp`. Its behavior is not a specification and
//! byte-matching it would buy nothing.
//!
//! The correctness bar instead is the **round-trip property**: source is
//! parsed, printed back out by [`to_js`]/[`generate`], and reparsed, and the
//! two ASTs — modulo the two normalizations `tests/corpus.rs` documents
//! (numeric-literal `"raw"` text, which necessarily changes when a literal
//! is reprinted from its `f64` value, and source locations, which change by
//! construction) — must be identical. That is what every test in this crate
//! checks, and it is the property to preserve when changing anything here.
//!
//! # `Pretty::Yes` is indentation, not formatting
//!
//! [`Pretty::Yes`] adds indentation and a handful of readability spaces so
//! the output is not one long line. It does not reflow line lengths,
//! normalize quote or brace style beyond what [`Opt`] already controls, wrap
//! long argument lists, or otherwise behave like a source formatter — it is
//! the same printer as [`Pretty::No`], with whitespace inserted at fixed
//! points. Do not expect `rustfmt`/`prettier`-style output from it.
//!
//! # Coverage
//!
//! What follows describes what is actually run, not a completeness claim:
//! this port found 6 doc comments elsewhere in the crate asserting some
//! enumeration was exhaustive when it was not (each one had to be deleted
//! once a test disproved it — see `precedence.rs`'s
//! `flow_no_anon_region_hazard` for the worked example), so this section
//! deliberately does not add a seventh.
//!
//! - All 271 node kinds are handled, compiler-enforced (above).
//! - 41 real defects were found and fixed during the port, almost all in
//!   `arms/` and `precedence.rs`'s parenthesization logic — the exact place a
//!   JS printer is most often silently wrong. Every one was invisible to
//!   reading juno's source and caught only by running generated output back
//!   through the parser; see the `Defect N` comments throughout `tests/`, and
//!   `MANIFEST.md` for the per-defect record.
//! - `tests/corpus.rs` regenerates and reparses all 421 files under the
//!   checked-in parser/sema corpora (393 of them parse cleanly; the other 28
//!   are error fixtures or cover-grammar trees, enumerated there by name).
//!   That corpus is **not** a stand-in for adversarial parenthesization
//!   coverage: it contains only 87 parenthesized nodes, spanning 23 kinds
//!   and 40 distinct (parent kind, child kind) edges, because real-world
//!   source rarely writes redundant parens. The "must add parens" direction
//!   of `need_parens` — where every defect above lives — is exercised at
//!   only those 40 edges by this corpus. `tests/paren_matrix.rs` is what
//!   closes that gap: a generated cross-product over (parent kind × child
//!   kind × child position) with every child explicitly parenthesized,
//!   reaching 1985 distinct (parent, child) pairs. It found 8 defects on its
//!   first run. `tests/roundtrip.rs` adds
//!   several hundred hand-written and generated-cross-product cases on top
//!   (e.g. `flow_arrow_return_type_shapes_all_round_trip`'s 3645-shape
//!   probe), each documented at its own `#[test]`.
//!
//! This crate's docs make no performance claims, benchmarked or otherwise;
//! none should be added.
//!
//! # The façade: a free function, not an inherent method
//!
//! The natural spelling for "regenerate JS from a parse" would be an
//! inherent `ParsedJS::to_js`, living on `hermes-parser`'s type. That is
//! impossible here: [`to_js`] names `hermes_parser::ParsedJS` in its own
//! public signature, so `hermes-parser` is a direct, non-optional dependency
//! of this crate — and it cannot depend back on this crate to host an
//! inherent method without a cycle. So [`to_js`] ships as a free function
//! here instead, taking `&mut ParsedJS`. The call site is unaffected either
//! way — one line for a caller already holding a `ParsedJS`:
//!
//! (An earlier version of this note credited `hermes-sema` for that ordering,
//! since its `resolve` façade also takes a `ParsedJS`. That was true when the
//! decision was made, but `hermes-sema` is optional as of the `annotate`
//! feature and removing it would not free `to_js` to become a method.)
//!
//! ```
//! # use hermes_gen_js::{to_js, Opt};
//! # use hermes_parser::{parse, ParseFlags};
//! # let mut parsed = parse("1;", ParseFlags::default()).unwrap();
//! let js = to_js(&mut parsed, Opt::default())?;
//! # Ok::<(), hermes_gen_js::GenJsError>(())
//! ```

mod annotate;
mod arms;
pub mod dispatch;
mod gen;
mod precedence;

use hermes_ast::node::{Node, NodeKind};

pub use gen::{generate, Annotation, Opt, Pretty, QuoteChar};

/// Why generation failed.
///
/// See the plan's Task 2, Step 4
/// (`doc/superpowers/plans/2026-08-15-gen-js-port.md`) and spec §4
/// (`doc/superpowers/specs/2026-08-15-gen-js-port-design.md`). A malformed
/// input tree is reported through this type, never a panic or `abort()` —
/// this is where the port deliberately departs from C++ `AST2JS.cpp:107`.
#[derive(Debug)]
pub enum GenJsError {
    /// The sink returned an error.
    Io(std::io::Error),
    /// A node kind that has no source syntax reached the generator: one of
    /// the 7 internal kinds matched explicitly in
    /// [`dispatch::GenJS::gen_node`] (the cover-grammar group, `SHBuiltin`,
    /// `ImplicitCheckedCast` — see the crate docs and spec §4), or
    /// `TemplateElement`, which only its `TemplateLiteral` may print. Since
    /// Task 13 deleted the temporary catch-all those are the *only* kinds
    /// that can produce this: every other kind has a named arm.
    ///
    /// It is also returned for a malformed *tree* whose node kinds are all
    /// fine — e.g. a `ClassProperty` whose `ts_modifiers` holds something
    /// that is not a `TSModifiers` (`arms/func.rs`'s `ts_modifiers_of`).
    ///
    /// Carries the [`NodeKind`] rather than the `&'static str` the spec's
    /// API sketch shows: `NodeKind` has no name-string accessor today (it
    /// is `@generated` from `ESTree.def`, see `crates/ast/src/node.rs:8`),
    /// and adding one is out of scope for this task. `NodeKind` already
    /// implements `Debug`, which [`Display`](std::fmt::Display) below uses,
    /// so callers lose nothing but a slightly different spelling.
    UnsupportedKind(NodeKind),
    /// An identifier's bytes contain an unpaired surrogate, which has no JS
    /// spelling (spec §5's identifier rule).
    UnrepresentableIdentifier,
    /// A `BinaryExpression`/`LogicalExpression`/`UnaryExpression`/
    /// `UpdateExpression`'s `operator` field held a spelling no grammar
    /// production ever writes there.
    ///
    /// Added for [`precedence`]'s operator classifiers
    /// (`BinaryExpressionOperator::from_label` and its three siblings).
    /// Those fields are a raw `NodeLabel` atom rather than a typed enum (our
    /// AST has no such enum at all — see `precedence.rs`'s module doc
    /// comment), so nothing at the type level stops a caller from handing
    /// `generate()` a `Node` whose `operator` is, say, `"foo"`: the bundled
    /// parser never writes anything but the fixed spelling set, but
    /// `generate()`'s `Node` parameter is not required to have come from it.
    /// A hand-built or JSON-deserialized tree is exactly the malformed input
    /// tree spec §4 requires be reported through this type rather than a
    /// panic.
    UnknownOperator {
        /// Which node kind's `operator` field was being classified:
        /// `"BinaryExpression"`, `"LogicalExpression"`, `"UnaryExpression"`,
        /// or `"UpdateExpression"`.
        kind: &'static str,
        /// The offending spelling, as decoded by `GCLock::bytes_str_lossy`
        /// (never the raw bytes — this is diagnostic text, not re-emitted
        /// output).
        spelling: String,
    },
}

impl std::fmt::Display for GenJsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GenJsError::Io(e) => write!(f, "I/O error while generating JS: {e}"),
            GenJsError::UnsupportedKind(kind) => {
                write!(f, "node kind {kind:?} has no JS source syntax")
            }
            GenJsError::UnrepresentableIdentifier => {
                write!(
                    f,
                    "identifier contains an unpaired surrogate with no JS spelling"
                )
            }
            GenJsError::UnknownOperator { kind, spelling } => {
                write!(
                    f,
                    "{kind}'s operator field has an unrecognized spelling {spelling:?}"
                )
            }
        }
    }
}

impl std::error::Error for GenJsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GenJsError::Io(e) => Some(e),
            GenJsError::UnsupportedKind(_)
            | GenJsError::UnrepresentableIdentifier
            | GenJsError::UnknownOperator { .. } => None,
        }
    }
}

pub use gen::GenJS;

/// Regenerate JS source from a parsed program: locks the arena, prints the
/// `Program` node with [`generate`], and hands back the result as a `String`.
///
/// This is the crate's front door — see the crate docs' "Quickstart" and
/// "The façade: a free function, not an inherent method" for what it does
/// and why it is shaped this way rather than as an inherent
/// `ParsedJS::to_js`. The call site for a user already holding a `ParsedJS`
/// is one line:
///
/// ```
/// use hermes_gen_js::{to_js, Opt};
/// use hermes_parser::{parse, ParseFlags};
///
/// let mut parsed = parse("let x = 1;", ParseFlags::default()).unwrap();
/// let js = to_js(&mut parsed, Opt::default()).unwrap();
/// assert_eq!(js, "let x = 1;\n");
/// ```
///
/// # Panics
///
/// Takes the arena lock (via [`hermes_parser::ParsedJS::with_program`]), so
/// it panics if a `GCLock` is already live on this thread.
pub fn to_js(parsed: &mut hermes_parser::ParsedJS, opt: Opt<'_>) -> Result<String, GenJsError> {
    let mut out = Vec::new();
    parsed.with_program(|ctx, root| generate(&mut out, ctx, root, opt))?;
    // Spec §5 (`doc/superpowers/specs/2026-08-15-gen-js-port-design.md`):
    // string literals are escaped to ASCII and identifiers are re-encoded
    // through `try_bytes_str`, so generator output is always valid UTF-8.
    Ok(String::from_utf8(out).expect("generate() only ever writes valid UTF-8"))
}

impl GenJS<'_, '_> {
    /// Builds the error for a node kind that has no source syntax.
    ///
    /// Shared by the 7 internal-kind arms and, temporarily, by the
    /// catch-all in [`dispatch::GenJS::gen_node`] (deleted in Task 13).
    fn unsupported_kind(&mut self, node: &Node) -> Result<(), GenJsError> {
        Err(GenJsError::UnsupportedKind(node.kind()))
    }
}
