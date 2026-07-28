/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Tests for `sema::resolve::resolve_ast` / `sema::resolver` — the S0
//! resolver entry.
//!
//! The *behavioral* test for this component is `sema_differential.rs`, which
//! compares `sema-dump`'s output against the real `hermesc -dump-sema`
//! byte-for-byte. What is checked here is what a byte-comparison cannot
//! see: the shape of the resulting `SemContext` (so a regression names the
//! broken invariant rather than dumping a 60-line diff), and the S0
//! boundary panics — the guarantee that this resolver never *silently*
//! under-resolves a construct it doesn't model yet.
//!
//! The parse-driver setup is trimmed from
//! `rust/crates/parser/src/bin/ast_dump.rs`, like `decl_collector.rs`'s.

use ast::context::{Context, GCLock, NodeRc};
use ast::node::{ExpressionStatement, Node, NumericLiteral, Program};
use ast::node_child::{NodeList, NodeMetadata, Strictness};
use atom_table::INVALID_ATOM_BYTES;
use parser::js::JSParserImpl;
use parser::lexer::{GrammarContext, JSLexer};
use sema::keywords::Keywords;
use sema::resolve::resolve_ast;
use sema::sem_context::{DeclKind, SemContext};
use std::cell::RefCell;
use std::rc::Rc;
use support::diag::{DiagHandler, ResolvedDiagnostic};
use support::manager::SourceErrorManager;

/// Parse `src` as a `Program` and return its root node, panicking on any
/// parse error.
fn parse<'gc>(
    gc: &'gc GCLock,
    sm: &mut SourceErrorManager,
    src: &str,
) -> &'gc Node<'gc> {
    let buf_id = sm.add_buffer_bytes("input", src.as_bytes());
    let result: Option<&Node> = {
        let atoms = &gc.ctx().atom_table;
        let lexer =
            JSLexer::new(buf_id, sm, atoms, GrammarContext::AllowRegExp);
        let mut parser = JSParserImpl::new(gc, lexer);
        parser.parse()
    };
    assert_eq!(sm.error_count(), 0, "unexpected parse errors in: {src}");
    result.expect("parser returned no Program")
}

/// Parse and resolve `src` with no ambient declarations, returning the
/// resulting `SemContext` and the resolved root node's strictness.
///
/// The strictness is read off the root `resolve_ast` *returned* (which for
/// these inputs is the one that went in — see
/// `unrewritten_resolution_returns_the_same_root`), not off the one that
/// went in: the resolver rebuilds every ancestor of a rewritten node, so the
/// returned root is the only one guaranteed to carry the final annotations.
fn resolve(src: &str) -> (SemContext, Strictness) {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let root = parse(&gc, &mut sm, src);
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let root = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .unwrap_or_else(|| panic!("resolution failed for: {src}"));
    let strictness = match root {
        Node::Program(p) => p.strictness.get(),
        _ => unreachable!(),
    };
    (sem_ctx, strictness)
}

/// The text of `atom`, for assertions. Needs a live `GCLock`, hence the
/// closure-shaped `resolve_with_ambient` below.
fn atom_string(gc: &GCLock, atom: atom_table::AtomBytes) -> String {
    String::from_utf8(gc.bytes(atom).to_vec()).expect("atom is not UTF-8")
}

/// The global function and the global scope exist, the global scope belongs
/// to the global function, and `visit(ProgramNode *)` marked the function as
/// a program node with a function-body scope.
#[test]
fn empty_program_creates_global_function_and_scope() {
    let (sem_ctx, strictness) = resolve("");
    sem_ctx.assert_global_function_and_scope();

    let global_fn = sem_ctx.get_global_function();
    let global_scope = sem_ctx.get_global_scope();
    let info = sem_ctx.function(global_fn);
    assert!(info.is_program_node);
    assert!(!info.strict);
    assert_eq!(info.get_scopes(), &[global_scope]);
    // ScopeRAII's `isFunctionBodyScope` arm ran.
    assert_eq!(info.get_function_body_scope(), global_scope);
    assert_eq!(sem_ctx.scope(global_scope).parent_function, global_fn);
    assert_eq!(sem_ctx.scope(global_scope).parent_scope, None);
    assert_eq!(sem_ctx.scope(global_scope).depth, 0);
    // makeStrictness(false).
    assert_eq!(strictness, Strictness::NonStrictMode);
    // No ambient decls were passed, so the global scope is empty.
    assert!(sem_ctx.scope(global_scope).decls.is_empty());
    // The binding table scope was retained after being popped.
    assert!(!sem_ctx.get_binding_table_global_scope().is_null());
}

/// A "use strict" directive flips the global function's strictness and the
/// Program node's `strictness` decoration.
#[test]
fn use_strict_directive_sets_strictness() {
    let (sem_ctx, strictness) = resolve("\"use strict\";\n");
    assert!(sem_ctx.function(sem_ctx.get_global_function()).strict);
    assert_eq!(strictness, Strictness::StrictMode);
}

/// A leading string literal that is NOT the first statement is not a
/// directive, so it must not flip strictness.
#[test]
fn non_prologue_string_is_not_a_directive() {
    let (sem_ctx, strictness) = resolve("1;\n\"use strict\";\n");
    assert!(!sem_ctx.function(sem_ctx.get_global_function()).strict);
    assert_eq!(strictness, Strictness::NonStrictMode);
}

/// `processAmbientDecls` declares both `var`s and `function`s from every
/// ambient file, `var`s first, deduplicating repeats — the behavior the
/// 63-decl libhermes dump depends on.
#[test]
fn ambient_decls_become_undeclared_global_properties() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let ambient = vec![NodeRc::from_node(
        &gc,
        parse(
            &gc,
            &mut sm,
            "var a; var b; function b() {} function c() {} var a;",
        ),
    )];
    let root = parse(&gc, &mut sm, "");
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    assert!(resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &ambient).is_some());

    let global_scope = sem_ctx.get_global_scope();
    let names: Vec<String> = sem_ctx
        .scope(global_scope)
        .decls
        .iter()
        .map(|d| {
            let decl = sem_ctx.decl(*d);
            assert_eq!(decl.kind, DeclKind::UndeclaredGlobalProperty);
            assert_eq!(decl.scope, Some(global_scope));
            atom_string(&gc, decl.name)
        })
        .collect();
    // `a` and `b` (vars, in source order) then `c` — the only function whose
    // name isn't already bound. The duplicate `var a` and the `function b`
    // are both skipped by the `bindingTable_.count(name)` guard, which is
    // exactly why libhermes' 64 declarations dump as 63 decls.
    assert_eq!(names, vec!["a", "b", "c"]);
}

/// `bufferMessages_`: diagnostics produced during resolution are buffered
/// and only reach the handler when the resolver is dropped. If the
/// `enable_buffering`/`disable_buffering` pair were unbalanced, the message
/// would never be delivered (still buffered) — so a delivered message proves
/// the flush ran.
#[test]
fn resolver_diagnostics_are_buffered_then_flushed() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    // A handler whose log can be read without borrowing `sm` (the resolver
    // holds `&mut sm` for its whole lifetime).
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    sm.set_handler(Box::new(SharedHandler(Rc::clone(&log))));
    let root = parse(&gc, &mut sm, "\"inline\";\n\"noinline\";\n");
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));

    {
        let binding_table = sem_ctx.binding_table_rc();
        let mut resolver = sema::resolver::SemanticResolver::new(
            &binding_table,
            &mut sem_ctx,
            &mut sm,
            &[],
            true,
        );
        assert!(resolver.run(&gc, root).is_some());
        // The warning has been produced and counted, but is still buffered:
        // nothing has reached the handler.
        assert_eq!(
            log.borrow().len(),
            0,
            "message escaped the buffer before the resolver was dropped"
        );
    }
    // Dropping the resolver disables buffering, which flushes.
    assert_eq!(
        log.borrow().as_slice(),
        ["Should not declare both 'inline' and 'noinline'.".to_string()],
        "buffered message was never flushed"
    );
    assert_eq!(sm.warning_count(), 1);
}

/// The resolver is a *transforming* visitor: `resolve_ast` hands back the
/// (possibly new) root, because rewriting any node rebuilds every ancestor
/// up to and including the `Program`. Nothing on the S0 path rewrites
/// anything, so the returned root must be the very node that went in —
/// pointer-identical, not an equal copy. This is the invariant every later
/// stage's "unchanged subtrees are shared" behavior rests on.
#[test]
fn unrewritten_resolution_returns_the_same_root() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    // Every shape the S0 corpus can produce: a directive prologue, an
    // expression statement, an empty statement, the literal kinds.
    let root = parse(
        &gc,
        &mut sm,
        "\"use strict\";\n1;\n;\n\"s\";\ntrue;\nnull;\n",
    );
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .expect("resolution failed");
    assert!(
        std::ptr::eq(root, resolved),
        "an unrewritten tree must be returned as-is, not rebuilt"
    );
}

/// The recursion-depth protocol (`RecursiveVisitor.h`'s
/// `incRecursionDepth`/`decRecursionDepth`, bracketing every dispatched
/// node): an AST nested deeper than `kASTMaxRecursionDepth` (1024) reports
/// `recursionDepthExceeded`'s error exactly once and fails resolution,
/// instead of overflowing the stack.
///
/// The tree is hand-built and deliberately not valid JS (an
/// `ExpressionStatement` whose expression is another `ExpressionStatement`):
/// the depth protocol is kind-agnostic — in C++ the *dispatcher* brackets
/// every node whatever its kind — and `ExpressionStatement` is the only kind
/// with a child that the S0 resolver models, so it is the only shape that
/// can be nested this deep without tripping an unrelated S0 boundary panic.
/// A parsed source string can't stand in either: the parser has its own
/// (lower) nesting limit and would reject it first.
///
/// Runs on a thread with an enlarged stack: 1024 levels of *unoptimized*
/// `visit_children`/`visit_node` frames (the generated dispatch matches on
/// every node kind, so a debug-build frame is large) exceed the 2 MiB the
/// test harness gives a test thread. That is a debug-build property of this
/// traversal, not of the limit — which is exactly what the limit is there to
/// keep bounded.
#[test]
fn too_deeply_nested_ast_reports_the_recursion_limit() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(too_deeply_nested_ast_reports_the_recursion_limit_impl)
        .expect("failed to spawn the deep-recursion test thread")
        .join()
        .expect("the deep-recursion test thread panicked");
}

fn too_deeply_nested_ast_reports_the_recursion_limit_impl() {
    /// Comfortably past `ESTree::kASTMaxRecursionDepth` == 1024
    /// (`include/hermes/AST/RecursiveVisitor.h:686-692`).
    const DEPTH: usize = 1100;

    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    sm.set_handler(Box::new(SharedHandler(Rc::clone(&log))));
    // Parse a trivial program purely to obtain a source range that belongs
    // to a buffer the manager knows about, so the reported error can be
    // resolved to a location like any real one.
    let range = parse(&gc, &mut sm, "1;\n").range();

    let mut inner: &Node = gc.alloc(Node::NumericLiteral(NumericLiteral::new(
        NodeMetadata::new(range),
        1.0,
    )));
    for _ in 0..DEPTH {
        inner = gc.alloc(Node::ExpressionStatement(ExpressionStatement::new(
            NodeMetadata::new(range),
            inner,
            // Not a directive: the parser leaves `_directive` null for a
            // statement that isn't one, and `scanDirectives` stops there.
            INVALID_ATOM_BYTES,
        )));
    }
    let deep_root = gc.alloc(Node::Program(Program::new(
        NodeMetadata::new(range),
        NodeList::from_iter(&gc, [inner]),
    )));

    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved = resolve_ast(&gc, &mut sem_ctx, &mut sm, deep_root, &[]);
    assert!(resolved.is_none(), "over-deep resolution must fail");
    assert_eq!(
        log.borrow().as_slice(),
        ["Too many nested expressions/statements/declarations".to_string()],
        "the depth error must be reported exactly once"
    );
    assert_eq!(sm.error_count(), 1);
}

/// A `DiagHandler` whose log lives outside the `SourceErrorManager`, so it
/// can be inspected while the manager is mutably borrowed.
struct SharedHandler(Rc<RefCell<Vec<String>>>);

impl DiagHandler for SharedHandler {
    fn handle(&mut self, diag: &ResolvedDiagnostic) {
        self.0.borrow_mut().push(diag.message.clone());
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Resolution boundary: a construct S0 does not model must panic loudly
/// rather than resolve to something wrong. An identifier reference needs
/// `visit(IdentifierNode *)` (S1).
#[test]
#[should_panic(expected = "sema S0: unhandled node kind Identifier")]
fn identifier_reference_is_not_modeled() {
    resolve("x;");
}

/// Declaration boundary: a `var` reaches `processCollectedDeclarations`
/// with a non-empty `ScopeDecls`, which is S1 scope.
#[test]
#[should_panic(expected = "sema S0: declarations are S1 scope")]
fn declarations_are_not_modeled() {
    resolve("var x;");
}
