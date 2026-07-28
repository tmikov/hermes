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
use sema::ids::FunctionInfoId;
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

/// Resolution boundary: a construct not yet modeled must panic loudly
/// rather than resolve to something wrong. Identifier references are now
/// modeled (S1 T4, `visit(IdentifierNode *)`) and so are functions (S1 T7);
/// a call expression still needs `visit(CallExpressionNode *)`
/// (SemanticResolver.cpp:1117, S2).
#[test]
#[should_panic(expected = "sema S1: unhandled node kind CallExpression")]
fn call_expression_is_not_modeled() {
    resolve("f();");
}

/// `var` declarations are now modeled (S1 T5): `var x;` at the top level
/// declares a `GlobalProperty`, not an `UndeclaredGlobalProperty` — the
/// behavioral counterpart to the panic-boundary test this replaced.
///
/// Deliberately NOT written via the shared `resolve()` helper — see
/// `loose_identifier_reference_becomes_undeclared_global_property`'s doc
/// comment below for why: `validateAndDeclareIdentifier`'s new-decl path
/// stores a `Binding{decl, Some(ident)}` (a live `NodeRc` back to the
/// declaring `Identifier`), so `resolve()`'s "return `SemContext`, drop
/// `Context`" shape trips the same `NodeRc`-outlives-`Context` panic.
#[test]
fn var_declaration_at_global_scope_is_a_global_property() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let root = parse(&gc, &mut sm, "var x;\n");
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .expect("resolution failed for: var x;");

    let global_scope = sem_ctx.get_global_scope();
    assert_eq!(sem_ctx.scope(global_scope).decls.len(), 1);
    let decl = sem_ctx.decl(sem_ctx.scope(global_scope).decls[0]);
    assert_eq!(decl.kind, DeclKind::GlobalProperty);
}

/// A loose-mode identifier reference resolves to a fresh
/// `UndeclaredGlobalProperty` in the global scope — the behavioral half of
/// "identifiers are now modeled" that the panic-boundary test above only
/// proves negatively for a *different* construct.
///
/// Deliberately NOT written via the `resolve()` helper above: that helper
/// moves `SemContext` out to its caller while `Context` (the arena) drops
/// inside it, which is fine as long as nothing the returned `SemContext`
/// holds points back into the arena. `resolveIdentifier`'s ambient-global
/// path is the first thing in this crate that stores a `NodeRc` in a
/// `Binding` (`Binding{decl, identifier}`, mirroring the C++ exactly), so
/// returning `sem_ctx` out of a function that then drops `ctx` trips
/// `Context::drop`'s "NodeRc must not outlive Context" panic — `sem_ctx`
/// hasn't been dropped yet (it outlives the function), so the `NodeRc`
/// inside it is still alive when `ctx` goes away. Keeping `ctx` and
/// `sem_ctx` in the same scope (as `main` in `sema-dump` does) relies on
/// reverse-declaration-order drop instead, which is sound: `sem_ctx`
/// (declared after `ctx`) drops first.
#[test]
fn loose_identifier_reference_becomes_undeclared_global_property() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let root = parse(&gc, &mut sm, "x;\n");
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .expect("resolution failed for: x;");

    let global_scope = sem_ctx.get_global_scope();
    assert_eq!(sem_ctx.scope(global_scope).decls.len(), 1);
    let decl = sem_ctx.decl(sem_ctx.scope(global_scope).decls[0]);
    assert_eq!(decl.kind, DeclKind::UndeclaredGlobalProperty);
}

// ---- S1 T6: the fold-loop shapes ----------------------------------------
//
// `visit(BinaryExpressionNode *, Node **)` (SemanticResolver.cpp:405-436)
// folds a linearized `+`/`-` chain strictly left-to-right, bottom-up, and
// STOPS at the first link that fails to fold. These tests pin the three
// distinguishable outcomes of that loop against the port's rebuild-based
// mapping (see `resolver/expressions.rs`'s module doc): fully folded,
// partially folded with a rebuilt spine above the fold, and not folded at
// all because the *first* link already failed.

/// Resolve `src` and return the expression of its first (and only)
/// `ExpressionStatement`, keeping the arena alive across the assertions in
/// `f`.
fn with_first_expression<R>(src: &str, f: impl FnOnce(&Node) -> R) -> R {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let root = parse(&gc, &mut sm, src);
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .unwrap_or_else(|| panic!("resolution failed for: {src}"));
    let Node::Program(p) = resolved else {
        unreachable!("resolve_ast returned a non-Program root")
    };
    let stmt = p.body.iter().next().expect("empty program body");
    let Node::ExpressionStatement(es) = stmt else {
        panic!("first statement is not an ExpressionStatement")
    };
    f(es.expression)
}

/// A fully constant `+`/`-` chain collapses to a single literal: every link
/// folds, and the last fold's product is what the visit returns (C++ writes
/// it through `ppNode`, cpp:421).
#[test]
fn constant_binary_chain_folds_to_a_single_literal() {
    with_first_expression("1 + 2 - 3;\n", |e| match e {
        Node::NumericLiteral(n) => assert_eq!(n.value.get(), 0.0),
        other => {
            panic!("expected a folded literal, got {}", other.node_type_str())
        }
    });
}

/// A chain whose *tail* is non-constant folds its constant prefix and keeps
/// the rest: `1 + 2 + x` becomes `3 + x`. This is the case the port's
/// rebuild mapping exists for — C++ mutates `list[1]->_left` in place, while
/// here link 1 must be REBUILT around the folded literal, and that rebuilt
/// node must be what the visit returns.
#[test]
fn partially_constant_binary_chain_folds_its_prefix() {
    with_first_expression("1 + 2 + x;\n", |e| {
        let be = e
            .as_binary_expression()
            .expect("the outer link must survive as a BinaryExpression");
        match be.left {
            Node::NumericLiteral(n) => assert_eq!(n.value.get(), 3.0),
            other => panic!(
                "left should be the folded 3, got {}",
                other.node_type_str()
            ),
        }
        assert!(matches!(be.right, Node::Identifier(_)));
    });
}

/// Folding is strictly left-to-right and bottom-up: once a link fails, the
/// loop stops (C++ `break`, cpp:429), so `x + 1 + 2` folds NOTHING — even
/// though `1 + 2` "looks" foldable, those two literals are never operands of
/// the same link. Nothing changed anywhere, so the tree is returned
/// pointer-identical.
#[test]
fn binary_chain_stops_folding_at_the_first_failure() {
    with_first_expression("x + 1 + 2;\n", |e| {
        let be = e.as_binary_expression().expect("nothing may fold here");
        let inner = be
            .left
            .as_binary_expression()
            .expect("the inner link must survive too");
        assert!(matches!(inner.left, Node::Identifier(_)));
        assert!(matches!(inner.right, Node::NumericLiteral(_)));
        assert!(matches!(be.right, Node::NumericLiteral(_)));
    });
}

/// A non-`+`/`-` binary goes through the non-linearized path
/// (cpp:432-435): children visited generically, then a single fold attempt.
#[test]
fn non_linearized_binary_still_folds() {
    with_first_expression("6 * 7;\n", |e| match e {
        Node::NumericLiteral(n) => assert_eq!(n.value.get(), 42.0),
        other => {
            panic!("expected a folded literal, got {}", other.node_type_str())
        }
    });
}

/// `astFoldUnaryExpression` (cpp:499) turns `-5` into a single literal.
#[test]
fn unary_minus_on_a_literal_folds() {
    with_first_expression("-5;\n", |e| match e {
        Node::NumericLiteral(n) => assert_eq!(n.value.get(), -5.0),
        other => {
            panic!("expected a folded literal, got {}", other.node_type_str())
        }
    });
}

/// A fold nested inside an unrelated parent rebuilds the whole spine up to
/// the root, which is the mechanism `resolve_ast`'s "return the new root"
/// contract exists for.
#[test]
fn a_fold_rebuilds_the_root() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let root = parse(&gc, &mut sm, "1 + 2;\n");
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .expect("resolution failed");
    assert!(
        !std::ptr::eq(root, resolved),
        "a fold must rebuild every ancestor, including the Program"
    );
}

/// The whole point of `linearizeLeft` (ESTree.h:1437-1451): a `+`/`-` chain
/// is walked ITERATIVELY, so its links do not consume recursion depth. A
/// 2000-link chain is nearly twice `kASTMaxRecursionDepth` (1024) — a
/// recursive walk would report "Too many nested expressions" and fail —
/// yet it must resolve cleanly *and* fold end to end.
#[test]
fn a_long_binary_chain_is_folded_without_recursing() {
    /// Comfortably past `AST_MAX_RECURSION_DEPTH` (1024), and far below
    /// `MAX_NESTED_BINARY` (30000).
    const LINKS: usize = 2000;

    let src = (0..=LINKS)
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(" + ")
        + ";\n";
    // 0 + 1 + ... + LINKS
    let expected = (LINKS * (LINKS + 1) / 2) as f64;
    with_first_expression(&src, |e| match e {
        Node::NumericLiteral(n) => assert_eq!(n.value.get(), expected),
        other => panic!(
            "a constant chain must fold whole, got {}",
            other.node_type_str()
        ),
    });
}

/// The same for `=` chains and `linearizeRight` (ESTree.h:1464-1477): 2000
/// nested assignments resolve (and validate every target) without tripping
/// the recursion limit.
#[test]
fn a_long_assignment_chain_does_not_recurse() {
    const LINKS: usize = 2000;

    let src = "var a;\n".to_string() + &"a = ".repeat(LINKS) + "1;\n";
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let root = parse(&gc, &mut sm, &src);
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    assert!(
        resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[]).is_some(),
        "a linearized `=` chain must not exhaust the recursion budget"
    );
    assert_eq!(sm.error_count(), 0);
}

// ---- S1 T7: functions ---------------------------------------------------
//
// The differential (`sema_differential.rs`) already pins the *dump* of every
// shape below. What it cannot see is node identity — `SemContextDumper`
// prints `hoistedFunction <name>`, and a stale `NodeRc` carries the same
// name as its rebuilt copy — nor the scope-list shape behind
// `getParameterScope()`/`getFunctionBodyScope()`. Those are what these
// tests pin.

/// \return the `FunctionInfoId` decorating a function-like node.
fn sem_info_of(node: &Node) -> FunctionInfoId {
    let id = match node {
        Node::FunctionDeclaration(n) => n.sem_info.get(),
        Node::FunctionExpression(n) => n.sem_info.get(),
        _ => panic!("not a function-like node"),
    };
    FunctionInfoId::from_sema_id(id.expect("visitFunctionLike sets semInfo"))
}

/// \return the first statement of a resolved `Program`.
fn first_statement<'gc>(root: &'gc Node<'gc>) -> &'gc Node<'gc> {
    let Node::Program(p) = root else {
        unreachable!("not a Program root")
    };
    p.body.iter().next().expect("empty program body")
}

/// **The `hoistedFunctions` backref fixup** (spec §3.4 (a)).
///
/// `visit(FunctionDeclarationNode *)` records the function node in
/// `curScope_->hoistedFunctions` *before* descending into it (cpp:236). A
/// fold inside the body (`1 + 2`) rebuilds the `BlockStatement` and
/// therefore the `FunctionDeclaration`, so the recorded `NodeRc` would
/// point at a node that is no longer in the tree unless the visit patches
/// it — see `resolver/functions.rs`'s module doc.
///
/// The differential cannot catch this (the dump prints only the name), so
/// the check is on node identity: the recorded node must be the one
/// `resolve_ast` returned, and that one must NOT be the node that went in
/// (otherwise the test would pass vacuously, having never exercised a
/// rebuild at all).
#[test]
fn hoisted_function_backref_follows_a_rebuilt_node() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let src = "function f() {\n  var x = 1 + 2;\n}\n";
    let root = parse(&gc, &mut sm, src);
    let original_id = first_statement(root).node_id();
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .expect("resolution failed");

    let rebuilt = first_statement(resolved);
    assert!(
        matches!(rebuilt, Node::FunctionDeclaration(_)),
        "the first statement must be the function declaration"
    );
    assert_ne!(
        rebuilt.node_id(),
        original_id,
        "non-degeneracy: the fold must have REBUILT the FunctionDeclaration, \
         otherwise this test proves nothing"
    );

    let global_scope = sem_ctx.get_global_scope();
    let hoisted = &sem_ctx.scope(global_scope).hoisted_functions;
    assert_eq!(hoisted.len(), 1, "one hoisted function declaration");
    assert_eq!(
        hoisted[0].node(&gc).node_id(),
        rebuilt.node_id(),
        "the hoistedFunctions entry is stale: it points at the \
         pre-rebuild FunctionDeclaration"
    );
}

/// A function whose body is NOT rewritten must leave its `hoistedFunctions`
/// entry pointer-identical — the fixup must not fire spuriously (and the
/// visit must return `Unchanged`, keeping the tree shared).
#[test]
fn hoisted_function_backref_is_untouched_without_a_rebuild() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let src = "function f(a) {\n  return a;\n}\n";
    let root = parse(&gc, &mut sm, src);
    let original_id = first_statement(root).node_id();
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .expect("resolution failed");

    assert_eq!(first_statement(resolved).node_id(), original_id);
    let global_scope = sem_ctx.get_global_scope();
    let hoisted = &sem_ctx.scope(global_scope).hoisted_functions;
    assert_eq!(hoisted.len(), 1);
    assert_eq!(hoisted[0].node(&gc).node_id(), original_id);
}

/// `declareParams`' redeclaration rules (cpp:1770-1796): a loose, simple
/// parameter list may repeat a name without an error, but the SECOND
/// declaration wins — a new `Decl` is created and the existing binding is
/// re-pointed at it, so a body reference resolves to the second parameter.
#[test]
fn duplicate_loose_parameters_rebind_to_the_last_declaration() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let src = "function f(a, a) {\n  return a;\n}\n";
    let root = parse(&gc, &mut sm, src);
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .expect("resolution failed");
    assert_eq!(sm.error_count(), 0, "loose duplicate params are allowed");

    let func_decl = first_statement(resolved);
    let info = sem_info_of(func_decl);
    let body_scope = sem_ctx.function(info).get_function_body_scope();
    // Two distinct Parameter decls, plus the implicit 'arguments'.
    let decls = &sem_ctx.scope(body_scope).decls;
    let params: Vec<_> = decls
        .iter()
        .copied()
        .filter(|&d| sem_ctx.decl(d).kind == DeclKind::Parameter)
        .collect();
    assert_eq!(params.len(), 2, "each 'a' gets its own Decl");
    assert_ne!(params[0], params[1]);

    // The `return a;` reference resolves to the LAST parameter decl.
    let Node::FunctionDeclaration(fd) = func_decl else {
        unreachable!()
    };
    let Node::BlockStatement(block) = fd.body else {
        unreachable!("function body is a BlockStatement")
    };
    let Some(Node::ReturnStatement(ret)) = block.body.iter().next() else {
        unreachable!("body starts with a ReturnStatement")
    };
    let Some(Node::Identifier(ident)) = ret.argument else {
        unreachable!("`return a;` returns an Identifier")
    };
    assert_eq!(
        sem_ctx.get_expression_decl(ident),
        Some(params[1]),
        "the body reference must see the second parameter"
    );
}

/// A strict duplicate parameter IS an error (`uniqueParams`, cpp:1755-1756
/// and 1778-1783) — the same code path, opposite outcome.
#[test]
fn duplicate_strict_parameters_are_an_error() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let root = parse(
        &gc,
        &mut sm,
        "\"use strict\";\nfunction f(a, a) {\n  return a;\n}\n",
    );
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    assert!(
        resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[]).is_none(),
        "a duplicate strict parameter must fail resolution"
    );
    assert_eq!(sm.error_count(), 1);
}

/// **The dual scope layout** (cpp:1846-1881). With parameter expressions the
/// function gets THREE scopes, in creation order: the parameter scope, the
/// (always empty) temporary `arguments` scope, and the function body scope
/// — and `getParameterScope()` (scopes[0]) is then distinct from
/// `getFunctionBodyScope()`. The `arguments` Decl lands in scopes[0], i.e.
/// the parameter scope (`SemContext::funcArgumentsDecl`).
#[test]
fn parameter_expressions_split_the_parameter_and_body_scopes() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let src = "function f(a, b = a) {\n  var c;\n  return c;\n}\n";
    let root = parse(&gc, &mut sm, src);
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .expect("resolution failed");

    let info = sem_info_of(first_statement(resolved));
    assert!(sem_ctx.function(info).has_parameter_expressions);
    assert!(!sem_ctx.function(info).simple_parameter_list);
    let scopes = sem_ctx.function(info).get_scopes().to_vec();
    assert_eq!(scopes.len(), 3, "param scope, temp arguments scope, body");
    let param_scope = sem_ctx.function(info).get_parameter_scope();
    let body_scope = sem_ctx.function(info).get_function_body_scope();
    assert_eq!(param_scope, scopes[0]);
    assert_eq!(body_scope, scopes[2]);
    assert_ne!(param_scope, body_scope);

    // The temporary 'arguments' scope holds no Decls of its own: it only
    // carries a binding, which is popped before the body is visited.
    assert!(sem_ctx.scope(scopes[1]).decls.is_empty());

    let kinds = |s| {
        sem_ctx
            .scope(s)
            .decls
            .iter()
            .map(|&d| sem_ctx.decl(d).kind)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        kinds(param_scope),
        vec![DeclKind::Parameter, DeclKind::Parameter, DeclKind::Var],
        "both parameters and the implicit 'arguments' live in scopes[0]"
    );
    assert_eq!(kinds(body_scope), vec![DeclKind::Var], "just `var c`");
}

/// Without parameter expressions there is exactly ONE scope, and the
/// parameter scope and the function body scope are the same object
/// (cpp:1874-1881).
#[test]
fn simple_parameters_share_one_scope_with_the_body() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let src = "function f(a) {\n  var c;\n  return c;\n}\n";
    let root = parse(&gc, &mut sm, src);
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .expect("resolution failed");

    let info = sem_info_of(first_statement(resolved));
    assert!(!sem_ctx.function(info).has_parameter_expressions);
    assert_eq!(sem_ctx.function(info).get_scopes().len(), 1);
    assert_eq!(
        sem_ctx.function(info).get_parameter_scope(),
        sem_ctx.function(info).get_function_body_scope()
    );
}

/// The `FunctionExprName` scope (cpp:1953-1961) belongs to the ENCLOSING
/// function, not to the function expression: `ScopeRAII` runs before the
/// `FunctionContext` is pushed, so `curFunctionInfo()` is still the outer
/// one. It is also the scope the `FunctionExpression` node is decorated
/// with.
#[test]
fn function_expression_name_scope_belongs_to_the_enclosing_function() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let src = "var g = function me() {\n  return me;\n};\n";
    let root = parse(&gc, &mut sm, src);
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .expect("resolution failed");

    let global_fn = sem_ctx.get_global_function();
    // The global function owns the global scope plus the name scope.
    let scopes = sem_ctx.function(global_fn).get_scopes().to_vec();
    assert_eq!(scopes.len(), 2);
    let name_scope = scopes[1];
    assert_eq!(sem_ctx.scope(name_scope).parent_function, global_fn);
    let decls = &sem_ctx.scope(name_scope).decls;
    assert_eq!(decls.len(), 1);
    assert_eq!(sem_ctx.decl(decls[0]).kind, DeclKind::FunctionExprName);

    // ... and the FunctionExpression node carries it.
    let Node::VariableDeclaration(vd) = first_statement(resolved) else {
        unreachable!()
    };
    let Some(Node::VariableDeclarator(decl)) = vd.declarations.iter().next()
    else {
        unreachable!()
    };
    let Some(Node::FunctionExpression(fe)) = decl.init else {
        unreachable!("initializer is a FunctionExpression")
    };
    assert_eq!(fe.scope.get(), Some(name_scope.sema_id()));
}

/// **The nested-scope unwind regression test** (S0 carry-item). With
/// functions and blocks there are now >= 2 binding scopes open in the
/// middle of a resolution (here: the global scope, the function body scope
/// and the block scope). `SemanticResolver`'s `Drop` must unwind them
/// back-to-front; `Vec`'s own front-to-back drop would trip
/// `pop_scope`'s "must be the current scope" `debug_assert!` *while already
/// panicking*, which is a double panic and therefore an ABORT — the test
/// process would die instead of reporting a failure.
///
/// So the assertion is the test's own shape: `should_panic` can only pass
/// if the original panic propagated normally through three open scopes.
#[test]
#[should_panic(expected = "sema S1: unhandled node kind CallExpression")]
fn a_panic_deep_inside_nested_scopes_unwinds_cleanly() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    // `g()` is an unhandled kind (S2). It sits inside a block, inside a
    // function body, inside the program — three live binding scopes.
    let root = parse(&gc, &mut sm, "function f() {\n  {\n    g();\n  }\n}\n");
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let _ = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[]);
}
