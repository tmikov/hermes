/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The convenience front door: [`resolve`] a [`ParsedJS`], get a
//! [`ResolvedJS`].
//!
//! This module adds no analysis. It is a thin assembly of the pieces
//! `tools`' `sema-dump` bin wires up by hand — a [`SemContext`] seeded with
//! the [`Keywords`] table, the [`SourceErrorManager`] the parse already used,
//! and one of the two [`crate::resolve`] entry points — into one call, so
//! that a consumer who only wants "source in, resolved AST out" does not have
//! to know the assembly order. It is the semantic counterpart of
//! `hermes_parser`'s `parse` façade, and it starts where that one leaves off:
//! its input is that façade's [`ParsedJS`].
//!
//! Everything it uses stays public: for a shared `SemContext` across several
//! files, a hand-built arena, or any other control the façade does not
//! expose, call [`crate::resolve::resolve_ast`] /
//! [`crate::resolve::resolve_ast_for_parser`] directly the way
//! `crates/tools/src/bin/sema_dump.rs` does.
//!
//! # Why `resolve` consumes the `ParsedJS`
//!
//! The resolver is a *transforming* visitor (see [`crate::resolver`]'s module
//! doc). Rewriting a node rebuilds its ancestors, so the root that comes out
//! of resolution is a different node than the one that went in, and it is the
//! one carrying the results — reading the old root would silently read a
//! stale tree. Taking the `ParsedJS` by value and handing back a `ResolvedJS`
//! makes that unmissable: after `resolve`, the pre-resolution root is simply
//! not reachable any more.
//!
//! # Lifetime model
//!
//! Inherited from `ParsedJS`, which [`ResolvedJS`] owns: AST nodes live in
//! the `Context` arena and are only reachable while a [`GCLock`] is held, so
//! reading the tree goes through [`ResolvedJS::with_program`], which takes the
//! lock for the duration of a closure. Only one `GCLock` may exist per thread
//! at a time, so those calls must not be nested.

use hermes_ast::context::{GCLock, NodeRc};
use hermes_ast::node::Node;
use hermes_parser::js::JSParserImpl;
use hermes_parser::lexer::{GrammarContext, JSLexer};
use hermes_parser::ParsedJS;
use hermes_support::diag::{DiagKind, OutputOptions, ResolvedDiagnostic};
use hermes_support::manager::SourceErrorManager;
use hermes_support::render::render_diagnostic;

use crate::dump::sem_dump;
use crate::keywords::Keywords;
use crate::libhermes::LIBHERMES;
use crate::resolve::{resolve_ast, resolve_ast_for_parser};
use crate::sem_context::SemContext;

/// A successful resolution: the arena, the resolved AST, and the
/// [`SemContext`] holding the results, owned together.
///
/// Produced by [`resolve`], [`resolve_for_parser`] or [`resolve_for_compile`],
/// each of which consumes the [`ParsedJS`] it resolves. Read the tree *with*
/// its semantic information through [`with_program`](Self::with_program), or
/// dump both with [`to_sema_dump`](Self::to_sema_dump).
///
/// **Not `Send`**, for the same reason `ParsedJS` is not: the arena uses
/// `Cell`/`UnsafeCell` and the `GCLock` guarding it is thread-local by design.
/// (The name keeps the port's `ParsedJS` casing rather than Rust's
/// `ResolvedJs`; that is deliberate, for consistency inside the port.)
pub struct ResolvedJS {
    /// The resolution results: every `Decl`, `LexicalScope` and
    /// `FunctionInfo`, plus the side tables keyed by AST node.
    ///
    /// **Must be declared before `parsed`**: fields drop in declaration order,
    /// a `SemContext` holds [`NodeRc`]s into the arena (binding identifiers
    /// and `$SHBuiltin` declarations), and `Context::drop` panics if a
    /// `NodeRc` into it is still alive.
    sem_ctx: SemContext,

    /// The arena, the source manager, and the pinned root — which
    /// `ParsedJS::transform_program` has re-pinned to the *resolved* root.
    parsed: ParsedJS,
}

impl ResolvedJS {
    /// Run `f` with the arena locked, the resolved root node, and the
    /// [`SemContext`] in hand.
    ///
    /// This is the read path. The `SemContext` comes along because that is
    /// the point of resolution: given an [`hermes_ast::node::Identifier`] in
    /// the tree, [`SemContext::get_expression_decl`] /
    /// [`SemContext::get_declaration_decl`] give the [`crate::ids::DeclId`] it
    /// binds to, and [`SemContext::decl`] gives that declaration.
    ///
    /// References into the arena cannot escape the closure — their lifetime
    /// ends with the lock — so return owned data instead. The one thing that
    /// *can* escape is a [`NodeRc`], which is refcounted rather than borrowed;
    /// dropping this `ResolvedJS` while such a handle is still alive panics
    /// inside `Context::drop`.
    ///
    /// The bound is higher-ranked because [`Node`] is *invariant* in its
    /// lifetime: a walker ([`hermes_ast::visitor::Visitor`]) needs the node
    /// reference and the node's own lifetime to be the same `'gc`, which only
    /// a `for<'gc>` closure can promise.
    ///
    /// # Panics
    ///
    /// Panics if another [`GCLock`] is active on this thread — in particular
    /// if `with_program` is called from inside another `with_program`.
    pub fn with_program<R, F>(&mut self, f: F) -> R
    where
        F: for<'gc> FnOnce(
            &'gc GCLock<'static, '_>,
            &'gc Node<'gc>,
            &SemContext,
        ) -> R,
    {
        // Disjoint field borrows: `sem_ctx` immutably, `parsed` mutably.
        let sem_ctx = &self.sem_ctx;
        self.parsed.with_program(|gc, root| f(gc, root, sem_ctx))
    }

    /// The resolution results, for the queries that do not need the AST —
    /// walking the scope tree, or reading a `FunctionInfo` reached from a
    /// [`crate::ids::FunctionInfoId`] obtained inside
    /// [`with_program`](Self::with_program).
    pub fn sem_context(&self) -> &SemContext {
        &self.sem_ctx
    }

    /// Dump the `SemContext` and the annotated AST as text, through
    /// [`crate::dump::sem_dump`] — the `-dump-sema` format, byte-for-byte.
    /// (After [`resolve_for_compile`] this is exactly what
    /// `hermesc -dump-sema` prints for the same input; that differential is
    /// this crate's correctness gate. After the parser path it is the same
    /// format over the differently-resolved tree, which is what the C++
    /// `sema-parser-dump` tool prints.)
    ///
    /// Bytes rather than a `String` because an identifier in the source may
    /// be an unpaired surrogate, which the dumper writes out as WTF-8 — not
    /// valid UTF-8. For ordinary sources `String::from_utf8` succeeds.
    ///
    /// # Panics
    ///
    /// Takes the arena lock, so it panics if another [`GCLock`] is live on
    /// this thread — in particular when called from inside
    /// [`with_program`](Self::with_program).
    pub fn to_sema_dump(&mut self) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        self.with_program(|gc, root, sem_ctx| {
            sem_dump(&mut out, gc, sem_ctx, root);
        });
        out
    }

    /// The diagnostics recorded so far, in emission order: the parse's (which
    /// were warnings and notes, since the parse succeeded) followed by
    /// resolution's.
    ///
    /// For a `ResolvedJS` that came out of [`resolve`] or
    /// [`resolve_for_compile`] these are again warnings and notes only;
    /// [`resolve_for_parser`] can return one carrying errors — see its doc.
    /// Render one with [`hermes_support::render::render_diagnostic`].
    pub fn diagnostics(&self) -> &[ResolvedDiagnostic] {
        self.parsed.diagnostics()
    }

    /// How many of the recorded diagnostics are errors.
    ///
    /// Zero for a `ResolvedJS` from [`resolve`] or [`resolve_for_compile`].
    /// This is the check the C++ `resolveASTForParser` callers make instead
    /// of looking at a return value, so it is what
    /// [`resolve_for_parser`]'s result must be tested with.
    pub fn error_count(&self) -> u32 {
        self.parsed.source_manager().error_count()
    }

    /// The source manager owning the parsed buffer (and the `libhermes`
    /// buffer, if [`CompileOptions::std_globals`] was on), for coordinate
    /// lookups and for driving the AST dumper by hand.
    pub fn source_manager(&self) -> &SourceErrorManager {
        self.parsed.source_manager()
    }

    /// Give back the [`ParsedJS`], now holding the *resolved* AST, and drop
    /// the `SemContext`.
    ///
    /// This is how the rest of the parser façade's surface — ESTree JSON
    /// dumping in particular, which is what a resolve-then-serialize consumer
    /// like `hermes-parser-wasm` does — stays reachable after resolution
    /// without this type mirroring it method for method. The AST keeps every
    /// rewrite the resolver made; only the `Decl`/scope tables go away.
    pub fn into_parsed(self) -> ParsedJS {
        let ResolvedJS { sem_ctx, parsed } = self;
        // Explicit, and load-bearing: the `SemContext` holds `NodeRc`s into
        // the arena `parsed` owns, and `Context::drop` panics if one outlives
        // it. Destructuring drops nothing on its own, so without this the
        // order would be the caller's to get wrong.
        drop(sem_ctx);
        parsed
    }
}

impl std::fmt::Debug for ResolvedJS {
    /// Summarizes rather than printing the AST or the tables, which can be
    /// huge. (Hand-written because neither `ParsedJS` nor `SemContext`
    /// derives `Debug`.)
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedJS")
            .field("functions", &self.sem_ctx.functions_len())
            .field("diagnostics", &self.diagnostics().len())
            .field("errors", &self.error_count())
            .finish_non_exhaustive()
    }
}

/// A resolution that reported at least one error.
///
/// There is no AST to carry, matching `hermes_parser::ParseError`: on the
/// compile path there genuinely is none (the resolver returns nothing once it
/// has failed), and on the parser path the tree is dropped rather than
/// returned — use [`resolve_for_parser`] if you need the partially resolved
/// tree that reported these errors.
#[derive(Debug, Clone)]
pub struct ResolveError {
    /// Every diagnostic recorded, in emission order: errors, warnings, notes.
    diagnostics: Vec<ResolvedDiagnostic>,
    /// How many of them were errors.
    error_count: u32,
}

impl ResolveError {
    /// Every diagnostic recorded during parsing and resolution, in emission
    /// order.
    pub fn diagnostics(&self) -> &[ResolvedDiagnostic] {
        &self.diagnostics
    }

    /// How many of the diagnostics are errors. Greater than zero for every
    /// `ResolveError` the façade produces.
    pub fn error_count(&self) -> u32 {
        self.error_count
    }

    /// The diagnostics rendered one string each, LLVM-style (location line,
    /// message, source line, caret), without ANSI colors.
    pub fn messages(&self) -> Vec<String> {
        let opts = OutputOptions {
            show_colors: false,
            ..OutputOptions::default()
        };
        self.diagnostics
            .iter()
            .map(|d| render_diagnostic(d, &opts))
            .collect()
    }

    /// Collect what the source manager recorded. The manager is the one the
    /// parse installed a collecting handler on, so this is every message from
    /// both phases.
    fn from_resolved(resolved: &ResolvedJS) -> ResolveError {
        ResolveError {
            diagnostics: resolved.diagnostics().to_vec(),
            error_count: resolved.error_count(),
        }
    }
}

impl std::fmt::Display for ResolveError {
    /// A single line — count plus the first error's location and text — as
    /// error types are expected to produce. The full LLVM-style rendering
    /// (source line and caret) is [`messages`](Self::messages); the
    /// structured form is [`diagnostics`](Self::diagnostics).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let plural = if self.error_count == 1 { "" } else { "s" };
        match self.diagnostics.iter().find(|d| d.kind == DiagKind::Error) {
            Some(d) => write!(
                f,
                "{} semantic error{plural}; first at {}:{}:{}: {}",
                self.error_count, d.file_name, d.line, d.col, d.message
            ),
            None => write!(f, "{} semantic error{plural}", self.error_count),
        }
    }
}

impl std::error::Error for ResolveError {}

/// One file's worth of ambient global declarations, as source text.
///
/// The compile path's `ambientDecls` are parsed files whose top-level
/// declarations are injected into the global scope — that is how a host tells
/// the compiler which globals its runtime provides. hermesc's
/// `-include-globals` takes them as file paths; this takes the text, because
/// the façade owns the parsing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobalDefinitions {
    /// The name to use for this buffer in diagnostics.
    pub file_name: String,
    /// The declarations, as JavaScript source.
    pub source: String,
}

/// What [`resolve_for_compile`] injects into the global scope before it
/// resolves.
///
/// `Default` is hermesc's default: the standard globals on, nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOptions {
    /// Declare the JavaScript standard library's globals (`Object`, `Math`,
    /// `print`, …) — hermesc's `-fstd-globals`, on by default there too.
    ///
    /// With this off, every reference to a standard global resolves to an
    /// implicitly created `UndeclaredGlobalProperty` instead of to an ambient
    /// declaration; nothing fails, but the `SemContext` differs. The
    /// declarations are [`crate::libhermes::LIBHERMES`], compiled into this
    /// crate.
    pub std_globals: bool,

    /// Additional ambient declaration files, parsed in this order after the
    /// standard globals — hermesc's `-include-globals`.
    ///
    /// A parse error in one of these is reported like any other and makes
    /// [`resolve_for_compile`] fail.
    pub global_definitions: Vec<GlobalDefinitions>,
}

impl Default for CompileOptions {
    fn default() -> Self {
        CompileOptions {
            std_globals: true,
            global_definitions: Vec::new(),
        }
    }
}

/// Resolve a parsed program, failing if resolution reported any error.
///
/// This is [`resolve_for_parser`] — the `compile = false` entry point, so no
/// compile-only validation and no AST rewrites — plus the error check its C++
/// callers make by hand. It is the one to reach for when the goal is "names
/// resolved, or tell me what is wrong": a linter, an editor service, a
/// transform that needs binding information.
///
/// Use [`resolve_for_compile`] instead when the result feeds a compiler back
/// end, and [`resolve_for_parser`] when the partially resolved tree is wanted
/// even though resolution reported errors.
///
/// # Panics
///
/// Takes the arena lock, so it panics if a [`GCLock`] is already live on this
/// thread.
///
/// ```
/// use hermes_parser::{parse, ParseFlags};
///
/// let parsed = parse("let x = 1; x;", ParseFlags::default()).unwrap();
/// let resolved = hermes_sema::resolve(parsed).unwrap();
/// assert_eq!(resolved.error_count(), 0);
///
/// // `continue` outside a loop is a semantic error, not a syntax error.
/// let parsed = parse("continue;", ParseFlags::default()).unwrap();
/// let err = hermes_sema::resolve(parsed).expect_err("should not resolve");
/// assert_eq!(err.error_count(), 1);
/// assert!(err.to_string().contains("'continue' not"), "{err}");
/// ```
pub fn resolve(parsed: ParsedJS) -> Result<ResolvedJS, ResolveError> {
    let resolved = resolve_for_parser(parsed);
    if resolved.error_count() == 0 {
        Ok(resolved)
    } else {
        Err(ResolveError::from_resolved(&resolved))
    }
}

/// Resolve a parsed program the way the C++ `resolveASTForParser`
/// (`SemResolve.cpp:299-310`) does, and hand back the result **whether or not
/// resolution reported errors**.
///
/// That is the entry point a parser-only consumer wants, and this is its
/// exact contract, ported: `compile = false`, so it will not error on
/// constructs that parse but cannot be compiled, will not perform
/// compile-specific validation, and will not transform the AST (no constant
/// folding, no arrow-body or `try` rewriting, no `$SHBuiltin` collapsing); it
/// takes no ambient declarations; and it *always* produces a tree. The C++
/// caller (`hermes-parser-wasm.cpp:104`) ignores the `bool` return value and
/// serializes whatever root it gets, checking its diagnostic handler
/// separately — [`ResolvedJS::error_count`] is that check here.
///
/// [`resolve`] is this plus the check, for callers who want the usual
/// `Result`.
///
/// # Panics
///
/// Takes the arena lock, so it panics if a [`GCLock`] is already live on this
/// thread.
///
/// ```
/// use hermes_parser::{parse, ParseFlags};
///
/// // Resolution fails, but the tree — and what could be resolved of it —
/// // still comes back.
/// let parsed = parse("continue; var x;", ParseFlags::default()).unwrap();
/// let mut resolved = hermes_sema::resolve_for_parser(parsed);
/// assert_eq!(resolved.error_count(), 1);
/// assert!(!resolved.to_sema_dump().is_empty());
/// ```
pub fn resolve_for_parser(parsed: ParsedJS) -> ResolvedJS {
    let mut parsed = parsed;
    let sem_ctx = parsed.transform_program(|gc, root, sm| {
        let mut sem_ctx = SemContext::new(Keywords::new(gc));
        let resolved = resolve_ast_for_parser(gc, &mut sem_ctx, sm, root);
        (resolved, sem_ctx)
    });
    ResolvedJS { sem_ctx, parsed }
}

/// Resolve a parsed program the way the C++ `resolveAST`
/// (`SemResolve.cpp:163-195`) does: the **compile** path.
///
/// Differences from [`resolve_for_parser`], which are exactly the C++ entry
/// points' differences (`compile = true`):
///
/// * It rejects what the compiler cannot handle, and runs the
///   compile-specific validation.
/// * It **transforms the AST**: constant folding of `+`/`-` chains and unary
///   operators, an expression-bodied arrow rewritten to a block with a
///   `return`, `try`/`catch`/`finally` split into nested `try`s,
///   `$SHBuiltin.x` collapsed to an `SHBuiltin` node, an anonymous
///   `export default function` turned into a function expression, and
///   block-scoped function promotion.
/// * It takes ambient declarations — see [`CompileOptions`].
/// * It can fail: the C++ returns `false` and this returns `Err`, in which
///   case there is no tree at all (unlike the parser path, which always has
///   one).
///
/// # Panics
///
/// Takes the arena lock, so it panics if a [`GCLock`] is already live on this
/// thread. Also panics if [`CompileOptions::std_globals`] is set and the
/// compiled-in `libhermes` declarations fail to parse, which would be a bug
/// in this crate rather than in the caller's input.
///
/// ```
/// use hermes_parser::{parse, ParseFlags};
/// use hermes_sema::{resolve_for_compile, CompileOptions};
///
/// let parsed = parse("Math.max(1, 2);", ParseFlags::default()).unwrap();
/// let mut resolved =
///     resolve_for_compile(parsed, &CompileOptions::default()).unwrap();
/// // `Math` came from the standard globals, so it is an ambient declaration
/// // rather than an implicitly created one.
/// let dump = String::from_utf8(resolved.to_sema_dump()).unwrap();
/// assert!(dump.contains("'Math' UndeclaredGlobalProperty"), "{dump}");
/// ```
pub fn resolve_for_compile(
    parsed: ParsedJS,
    options: &CompileOptions,
) -> Result<ResolvedJS, ResolveError> {
    let mut parsed = parsed;
    let sem_ctx = parsed.transform_program(|gc, root, sm| {
        // The ambient files are parsed into the same arena and the same
        // source manager as the input, exactly like `loadGlobalDefinition`
        // (CompilerDriver.cpp:773-785); their `Program` nodes are what
        // `resolveAST` takes as `ambientDecls`. (The C++ driver parses them
        // *before* the input file because that is when it reads its command
        // line; the resolver visits them in list order either way, so the
        // resolution results do not depend on it.)
        let mut ambient_decls: Vec<NodeRc> = Vec::new();
        let mut ambient_ok = true;
        if options.std_globals {
            match parse_ambient(gc, sm, "<libhermes>", LIBHERMES) {
                // `libhermes` is a compiled-in constant, so a failure here is
                // a bug in this crate rather than in the caller's input.
                None => panic!("libhermes must parse: it is a constant"),
                Some(program) => ambient_decls.push(program),
            }
        }
        for gd in &options.global_definitions {
            match parse_ambient(gc, sm, &gd.file_name, &gd.source) {
                None => {
                    // The parser reported the error into `sm`; stop before
                    // resolving against a half-loaded global scope, as the
                    // driver does (`LoadGlobalsFailed`).
                    ambient_ok = false;
                    break;
                }
                Some(program) => ambient_decls.push(program),
            }
        }

        if !ambient_ok {
            return (root, None);
        }
        let mut sem_ctx = SemContext::new(Keywords::new(gc));
        match resolve_ast(gc, &mut sem_ctx, sm, root, &ambient_decls) {
            Some(resolved) => (resolved, Some(sem_ctx)),
            // Resolution failed: keep the pre-resolution root (the arena must
            // keep exactly one pinned root either way) and report below. The
            // `SemContext` is dropped here, releasing its `NodeRc`s while the
            // arena is still alive, which is what `Context::drop` requires.
            None => (root, None),
        }
    });

    match sem_ctx {
        Some(sem_ctx) => Ok(ResolvedJS { sem_ctx, parsed }),
        None => {
            // No `ResolvedJS` to build the error from; read the same two
            // things off the `ParsedJS` directly, then drop it.
            Err(ResolveError {
                diagnostics: parsed.diagnostics().to_vec(),
                error_count: parsed.source_manager().error_count(),
            })
        }
    }
}

/// Parse one ambient-declaration file into `gc`'s arena, returning its
/// `Program` pinned, or `None` if the parse reported an error.
///
/// Mirrors `loadGlobalDefinition` (CompilerDriver.cpp:773-785): its own
/// buffer in the shared source manager, the same parser as the input, and the
/// resulting `Program` becomes one entry of the `DeclarationFileListTy`.
fn parse_ambient<'gc>(
    gc: &'gc GCLock<'static, '_>,
    sm: &mut SourceErrorManager,
    file_name: &str,
    source: &str,
) -> Option<NodeRc> {
    let buf_id = sm.add_buffer(file_name, source);
    // Scoped so the parser — and its `&mut sm` borrow — is gone before the
    // caller uses `sm` again. The returned node lives in the arena, so it
    // outlives the parser.
    let program: Option<&'gc Node<'gc>> = {
        let lexer = JSLexer::new(
            buf_id,
            sm,
            &gc.ctx().atom_table,
            GrammarContext::AllowRegExp,
        );
        JSParserImpl::new(gc, lexer).parse()
    };
    program.map(|p| NodeRc::from_node(gc, p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::DeclId;
    use crate::sem_context::DeclKind;
    use hermes_ast::node::NodeKind;
    use hermes_parser::{parse, ParseFlags};

    /// Resolve `source` on the parser path and return the dump as text.
    fn dump(source: &str) -> String {
        let parsed = parse(source, ParseFlags::default()).unwrap();
        let mut resolved = resolve(parsed).unwrap();
        String::from_utf8(resolved.to_sema_dump()).unwrap()
    }

    /// The `Decl` the last statement's identifier expression binds to.
    fn last_expression_decl(resolved: &mut ResolvedJS) -> Option<DeclId> {
        resolved.with_program(|_gc, program, sem| {
            let body = match program {
                Node::Program(p) => p.body,
                _ => panic!("root is not a Program"),
            };
            let stmt = body.iter().last().expect("empty program");
            let expr = match stmt {
                Node::ExpressionStatement(e) => e.expression,
                _ => panic!("last statement is not an ExpressionStatement"),
            };
            match expr {
                Node::Identifier(id) => sem.get_expression_decl(id),
                _ => panic!("not an identifier expression"),
            }
        })
    }

    #[test]
    fn resolves_a_reference_to_its_declaration() {
        let parsed = parse("var x = 1; x;", ParseFlags::default()).unwrap();
        let mut resolved = resolve(parsed).unwrap();
        let decl = last_expression_decl(&mut resolved).expect("unresolved");
        // `var` at the top level of a script is a property of the global
        // object, not a plain `Var`.
        assert_eq!(
            resolved.sem_context().decl(decl).kind,
            DeclKind::GlobalProperty
        );
        assert!(resolved.diagnostics().is_empty());
    }

    #[test]
    fn lexical_declarations_are_scoped() {
        let src = "let x = 1; { let x = 2; } x;";
        let parsed = parse(src, ParseFlags::default()).unwrap();
        let mut resolved = resolve(parsed).unwrap();
        let decl = last_expression_decl(&mut resolved).expect("unresolved");
        assert_eq!(resolved.sem_context().decl(decl).kind, DeclKind::Let);
        // The inner `let x` is a second, distinct declaration.
        let dump = String::from_utf8(resolved.to_sema_dump()).unwrap();
        assert_eq!(dump.matches("'x' Let").count(), 2, "{dump}");
    }

    #[test]
    fn semantic_errors_are_reported_without_printing() {
        let parsed = parse("continue;", ParseFlags::default()).unwrap();
        let err = resolve(parsed).unwrap_err();
        assert_eq!(err.error_count(), 1);
        assert_eq!(err.diagnostics().len() as u32, err.error_count());
        assert_eq!(err.diagnostics()[0].kind, DiagKind::Error);
        // The re-export at the crate root names the same type.
        let _: &[crate::ResolvedDiagnostic] = err.diagnostics();
        // `Display` is a one-line summary; the full rendering is `messages`.
        let shown = err.to_string();
        assert!(!shown.contains('\n'), "{shown}");
        assert!(
            shown.starts_with("1 semantic error; first at input:1:"),
            "{shown}"
        );
        assert_eq!(err.messages().len(), 1);
        assert!(err.messages()[0].contains('\n'), "{:?}", err.messages()[0]);
    }

    #[test]
    fn parser_path_keeps_the_tree_on_error() {
        let parsed = parse("continue; var x;", ParseFlags::default()).unwrap();
        let mut resolved = resolve_for_parser(parsed);
        assert_eq!(resolved.error_count(), 1);
        // Still a whole program, and `var x` still resolved.
        assert_eq!(
            resolved.with_program(|_gc, root, _sem| root.kind()),
            NodeKind::Program
        );
        let dump = String::from_utf8(resolved.to_sema_dump()).unwrap();
        assert!(dump.contains("'x' GlobalProperty"), "{dump}");
    }

    #[test]
    fn compile_path_transforms_the_ast_and_the_parser_path_does_not() {
        // Constant folding is a `compile = true` rewrite.
        let parsed = parse("var y = 1 + 2;", ParseFlags::default()).unwrap();
        let mut compiled = resolve_for_compile(
            parsed,
            &CompileOptions {
                std_globals: false,
                ..Default::default()
            },
        )
        .unwrap();
        let folded = String::from_utf8(compiled.to_sema_dump()).unwrap();
        assert!(!folded.contains("BinaryExpression"), "{folded}");

        let unfolded = dump("var y = 1 + 2;");
        assert!(unfolded.contains("BinaryExpression"), "{unfolded}");
    }

    #[test]
    fn compile_path_fails_where_the_parser_path_does_not() {
        // `with` is legal sloppy-mode JavaScript that the compiler refuses;
        // the error is `compile`-gated (SemanticResolver.cpp), so the parser
        // path accepts it and only marks the scope unresolvable.
        let src = "with (o) { x; }";
        let parsed = parse(src, ParseFlags::default()).unwrap();
        assert_eq!(resolve_for_parser(parsed).error_count(), 0);

        let parsed = parse(src, ParseFlags::default()).unwrap();
        let err = resolve_for_compile(parsed, &CompileOptions::default())
            .expect_err("compile path must reject it");
        assert!(err.error_count() > 0);
    }

    #[test]
    fn std_globals_are_ambient_declarations() {
        let parsed = parse("print(1);", ParseFlags::default()).unwrap();
        let mut with = resolve_for_compile(parsed, &CompileOptions::default())
            .unwrap()
            .to_sema_dump();
        let parsed = parse("print(1);", ParseFlags::default()).unwrap();
        let without = resolve_for_compile(
            parsed,
            &CompileOptions {
                std_globals: false,
                ..Default::default()
            },
        )
        .unwrap()
        .to_sema_dump();
        // Both resolve `print`; only the first has the other ~60 ambient
        // globals declared alongside it.
        assert!(with.len() > without.len());
        with.truncate(0);
        assert!(String::from_utf8(without)
            .unwrap()
            .contains("'print' UndeclaredGlobalProperty"));
    }

    #[test]
    fn user_global_definitions_are_declared() {
        let parsed = parse("myGlobal;", ParseFlags::default()).unwrap();
        let opts = CompileOptions {
            std_globals: false,
            global_definitions: vec![GlobalDefinitions {
                file_name: "<host>".to_string(),
                source: "var myGlobal;".to_string(),
            }],
        };
        let mut resolved = resolve_for_compile(parsed, &opts).unwrap();
        let decl = last_expression_decl(&mut resolved).expect("unresolved");
        // An ambient `var` becomes an ambient global property, which is what
        // distinguishes it from the implicitly created kind.
        assert_eq!(
            resolved.sem_context().decl(decl).kind,
            DeclKind::UndeclaredGlobalProperty
        );
    }

    #[test]
    fn a_broken_global_definition_file_is_an_error() {
        let parsed = parse("x;", ParseFlags::default()).unwrap();
        let opts = CompileOptions {
            std_globals: false,
            global_definitions: vec![GlobalDefinitions {
                file_name: "<host>".to_string(),
                source: "var 1x;".to_string(),
            }],
        };
        let err = resolve_for_compile(parsed, &opts).unwrap_err();
        assert!(err.error_count() > 0);
        assert!(err.to_string().contains("<host>"), "{err}");
    }

    #[test]
    fn into_parsed_gives_back_the_resolved_tree() {
        let parsed = parse("var y = 1 + 2;", ParseFlags::default()).unwrap();
        let resolved = resolve_for_compile(
            parsed,
            &CompileOptions {
                std_globals: false,
                ..Default::default()
            },
        )
        .unwrap();
        // The constant folding survives; the ESTree dumper is reachable.
        let json = resolved.into_parsed().to_estree_json(false);
        assert!(json.contains(r#""value":3"#), "{json}");
    }

    #[test]
    fn debug_summarizes() {
        let parsed = parse("function f() {}", ParseFlags::default()).unwrap();
        let resolved = resolve(parsed).unwrap();
        let shown = format!("{resolved:?}");
        assert!(shown.starts_with("ResolvedJS { functions: 2"), "{shown}");
    }

    #[test]
    fn dump_is_the_hermesc_dump_sema_text() {
        let text = dump("var x;");
        assert!(text.starts_with("SemContext\n"), "{text}");
        assert!(text.ends_with('\n'), "{text}");
    }
}
