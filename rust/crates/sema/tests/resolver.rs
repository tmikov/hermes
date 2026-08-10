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
use sema::dump::sem_dump;
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
/// node): an AST nested deeper than `kASTMaxRecursionDepth` reports
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
/// Runs on a thread with an enlarged stack: several hundred levels of
/// *unoptimized* `visit_children`/`visit_node` frames (the generated dispatch
/// matches on every node kind, so a debug-build frame is large) exceed the
/// 2 MiB the test harness gives a test thread. That is a debug-build
/// property of this traversal, not of the limit — which is exactly what the
/// limit is there to keep bounded.
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
    /// Comfortably past `AST_MAX_RECURSION_DEPTH` — the port of
    /// `ESTree::kASTMaxRecursionDepth`
    /// (`include/hermes/AST/RecursiveVisitor.h:686-692`), which is
    /// profile-selected: 512 in debug (the `HERMES_LIMIT_STACK_DEPTH` branch
    /// the ASan oracle build takes), 1024 in release. 1100 is past both, so
    /// this test pins the limit's behavior in either profile.
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
/// rather than resolve to something wrong. Calls themselves are modeled as
/// of S2 T6 (`visit(CallExpressionNode *)`, SemanticResolver.cpp:1117), but
/// the `$SHBuiltin` CommonJS-module protocol inside them is S4 — the panic
/// is the guarantee it is not silently mis-resolved.
#[test]
#[should_panic(expected = "$SHBuiltin.moduleFactory needs visitModuleFactory")]
fn shbuiltin_module_factory_is_not_modeled() {
    resolve("$SHBuiltin.moduleFactory(1, function (g, r) {});");
}

/// The same for the other two module property names, so all three panics are
/// pinned rather than only the first.
#[test]
#[should_panic(expected = "$SHBuiltin.export needs visitModuleExport")]
fn shbuiltin_export_is_not_modeled() {
    resolve("$SHBuiltin.export('x', 1);");
}

#[test]
#[should_panic(expected = "$SHBuiltin.import needs visitModuleImport")]
fn shbuiltin_import_is_not_modeled() {
    resolve("$SHBuiltin.import(1, 'x');");
}

// ---- S2 T6: the eval specials -------------------------------------------

/// `registerLocalEval` (SemanticResolver.cpp:2835-2843) reached end-to-end
/// through a real direct `eval()` call: the scope the call is in AND every
/// ancestor up to the global scope get `local_eval`, while a sibling scope
/// does not. `LexicalScope::local_eval` never reaches `-dump-sema`, so the
/// differential is blind to this — only a unit test can catch a regression.
#[test]
fn a_direct_eval_marks_its_whole_scope_chain() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    // `eval` is unbound here (no ambient decls), which is the `isEval = true`
    // branch of cpp:1129-1131.
    let root = parse(
        &gc,
        &mut sm,
        "function f() { { eval('1'); } }\nfunction g() { { 1; } }\n",
    );
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .expect("resolution failed");

    // Three FunctionInfos, in creation order: the global function, `f`, `g`.
    assert_eq!(sem_ctx.functions_len(), 3);
    let global = sem_ctx.get_global_function();
    let f = FunctionInfoId::from_sema_id(ast::SemaId(1));
    let g = FunctionInfoId::from_sema_id(ast::SemaId(2));
    // `f`'s scopes are its body scope and the nested block's; same for `g`.
    let marked = |func| -> Vec<bool> {
        sem_ctx
            .function(func)
            .get_scopes()
            .iter()
            .map(|s| sem_ctx.scope(*s).local_eval)
            .collect()
    };
    assert_eq!(marked(global), vec![true], "the global scope is an ancestor");
    assert_eq!(marked(f), vec![true, true], "the call's scope and its parent");
    assert_eq!(marked(g), vec![false, false], "an unrelated function");
}

/// With `eval` disabled (`Context::setEnableEval(false)`, cpp:1134-1149) the
/// warning becomes `EvalDisabled` and `registerLocalEval` does NOT run.
/// Unreachable from the differential corpus: `sema_differential.rs` has no
/// per-file flag mechanism, so it can only ever compare hermesc's default
/// (eval enabled) against ours.
#[test]
fn disabled_eval_warns_differently_and_marks_no_scope() {
    let mut ctx = Context::new();
    ctx.set_enable_eval(false);
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    sm.set_handler(Box::new(SharedHandler(Rc::clone(&log))));
    let root = parse(&gc, &mut sm, "eval('1');\n");
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .expect("resolution failed");

    assert_eq!(
        log.borrow().as_slice(),
        ["eval() is disabled at runtime".to_string()]
    );
    assert!(
        !sem_ctx.scope(sem_ctx.get_global_scope()).local_eval,
        "registerLocalEval must not run when eval is disabled"
    );
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
/// 2000-link chain is several times `kASTMaxRecursionDepth` (512 in debug,
/// 1024 in release) — a
/// recursive walk would report "Too many nested expressions" and fail —
/// yet it must resolve cleanly *and* fold end to end.
#[test]
fn a_long_binary_chain_is_folded_without_recursing() {
    /// Comfortably past `AST_MAX_RECURSION_DEPTH` in either build profile
    /// (512 debug / 1024 release), and far below `MAX_NESTED_BINARY` (30000).
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
#[should_panic(expected = "$SHBuiltin.export needs visitModuleExport")]
fn a_panic_deep_inside_nested_scopes_unwinds_cleanly() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    // `$SHBuiltin.export(...)` is the S4 module protocol, hence a deliberate
    // panic (S2 T6 made plain calls resolve, so this replaced the original
    // `g()`). It sits inside a block, inside a function body, inside the
    // program — three live binding scopes.
    let root = parse(
        &gc,
        &mut sm,
        "function f() {\n  {\n    $SHBuiltin.export('x', 1);\n  }\n}\n",
    );
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let _ = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[]);
}

// ---- S2 T1: loops, labels, break/continue, switch ---------------------
//
// `label_index` is NOT printed by `-dump-sema`, so `sema_differential.rs` is
// completely BLIND to a wrong (or missing) label index — a break-target
// miscompile would pass the byte comparison. These tests are the only pin on
// it, so they read the `Cell`s straight off the tree `resolve_ast` RETURNED.

/// The `label_index` decoration of any of the label-bearing node kinds —
/// the test-side counterpart of the resolver's `label_index_of`
/// (`getLabelDecorationBase`, SemanticResolver.cpp:680-693).
fn label_index(node: &Node) -> u32 {
    match node {
        Node::WhileStatement(n) => n.label_index.get(),
        Node::DoWhileStatement(n) => n.label_index.get(),
        Node::ForInStatement(n) => n.label_index.get(),
        Node::ForOfStatement(n) => n.label_index.get(),
        Node::ForStatement(n) => n.label_index.get(),
        Node::SwitchStatement(n) => n.label_index.get(),
        Node::BreakStatement(n) => n.label_index.get(),
        Node::ContinueStatement(n) => n.label_index.get(),
        Node::LabeledStatement(n) => n.label_index.get(),
        _ => panic!("no label decoration on {}", node.node_type_str()),
    }
}

/// Parse + resolve `src`, returning the `SemContext` and the RETURNED root.
/// The closure shape is forced by the `GCLock` borrow (see `resolve` above).
fn with_resolved<R>(
    src: &str,
    f: impl for<'gc> FnOnce(&SemContext, &'gc Node<'gc>) -> R,
) -> R {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let root = parse(&gc, &mut sm, src);
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .unwrap_or_else(|| panic!("resolution failed for: {src}"));
    f(&sem_ctx, resolved)
}

/// Every one of the five `LoopStatementNode` kinds allocates exactly one
/// label, in visit order, from the enclosing function's counter
/// (`allocateLabel`, cpp:555/601/617/627).
#[test]
fn every_loop_kind_allocates_one_label_in_visit_order() {
    let src = "while (a) ;\ndo ; while (a);\nfor (;;) ;\n\
               for (x in a) ;\nfor (x of a) ;\n";
    with_resolved(src, |sem_ctx, resolved| {
        let Node::Program(p) = resolved else {
            unreachable!("not a Program")
        };
        let kinds: Vec<(u32, &str)> = p
            .body
            .iter()
            .map(|n| (label_index(n), n.node_type_str()))
            .collect();
        assert_eq!(
            kinds,
            vec![
                (0, "WhileStatement"),
                (1, "DoWhileStatement"),
                (2, "ForStatement"),
                (3, "ForInStatement"),
                (4, "ForOfStatement"),
            ]
        );
        assert_eq!(
            sem_ctx.function(sem_ctx.get_global_function()).num_labels,
            5
        );
    });
}

/// A `LabeledStatement` gets its own label index, but a `break`/`continue`
/// naming it resolves to the label's *target statement* — the enclosing
/// loop, not the `LabeledStatement` (cpp:642-652 + 700-702/729-731).
#[test]
fn labeled_break_and_continue_target_the_labeled_loop() {
    let src = "l1: while (a) {\n  l2: for (;;) {\n    break l1;\n\
               \x20   continue l2;\n  }\n}\n";
    with_resolved(src, |sem_ctx, resolved| {
        let outer_labeled = first_statement(resolved);
        assert_eq!(label_index(outer_labeled), 0, "l1: itself");
        let Node::LabeledStatement(l1) = outer_labeled else {
            unreachable!("not a LabeledStatement")
        };
        assert_eq!(label_index(l1.body), 1, "the while");
        let Node::WhileStatement(w) = l1.body else {
            unreachable!("not a WhileStatement")
        };
        let Node::BlockStatement(outer_block) = w.body else {
            unreachable!("not a BlockStatement")
        };
        let inner_labeled =
            outer_block.body.iter().next().expect("empty while body");
        assert_eq!(label_index(inner_labeled), 2, "l2: itself");
        let Node::LabeledStatement(l2) = inner_labeled else {
            unreachable!("not a LabeledStatement")
        };
        assert_eq!(label_index(l2.body), 3, "the for");
        let Node::ForStatement(f) = l2.body else {
            unreachable!("not a ForStatement")
        };
        let Node::BlockStatement(inner_block) = f.body else {
            unreachable!("not a BlockStatement")
        };
        let mut it = inner_block.body.iter();
        let brk = it.next().expect("no break");
        let cont = it.next().expect("no continue");
        assert_eq!(
            label_index(brk),
            1,
            "`break l1` targets the WHILE (label 1), not the label (0)"
        );
        assert_eq!(label_index(cont), 3, "`continue l2` targets the for");
        assert_eq!(
            sem_ctx.function(sem_ctx.get_global_function()).num_labels,
            4
        );
    });
}

/// Unlabeled `break` uses `currentLoopOrSwitch` and unlabeled `continue`
/// uses `currentLoop` (cpp:709-713 / 746-748), so inside a switch nested in
/// a loop they target *different* statements.
#[test]
fn unlabeled_break_and_continue_use_their_own_innermost_target() {
    let src = "while (a) {\n  switch (b) {\n  case 0:\n    break;\n\
               \x20   continue;\n  }\n}\n";
    with_resolved(src, |_sem_ctx, resolved| {
        let while_stmt = first_statement(resolved);
        assert_eq!(label_index(while_stmt), 0);
        let Node::WhileStatement(w) = while_stmt else {
            unreachable!("not a WhileStatement")
        };
        let Node::BlockStatement(block) = w.body else {
            unreachable!("not a BlockStatement")
        };
        let switch_stmt = block.body.iter().next().expect("empty body");
        assert_eq!(label_index(switch_stmt), 1);
        let Node::SwitchStatement(sw) = switch_stmt else {
            unreachable!("not a SwitchStatement")
        };
        let Some(Node::SwitchCase(case)) = sw.cases.iter().next() else {
            unreachable!("no SwitchCase")
        };
        let mut it = case.consequent.iter();
        let brk = it.next().expect("no break");
        let cont = it.next().expect("no continue");
        assert_eq!(label_index(brk), 1, "`break` targets the switch");
        assert_eq!(label_index(cont), 0, "`continue` targets the while");
    });
}

/// **The known decorate-after-children exception** (resolver/mod.rs module
/// doc; cpp:520-539). `visit(SwitchStatementNode *)` visits `_discriminant`
/// FIRST and only then calls `setLabelIndex`. A folding discriminant
/// (`1 + 2`) makes that visit return `Changed`, so the switch is REBUILT —
/// and a naive port that snapshotted the builder before writing the label
/// would hand back a switch with `INVALID_LABEL`, i.e. a `break` pointing at
/// nothing. Neither the label index nor this rebuild is visible in
/// `-dump-sema`, so this test is the only thing standing between that bug
/// and a green gate.
#[test]
fn a_rebuilt_switch_keeps_its_label_index_and_scope() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let src = "switch (1 + 2) {\ncase 0:\n  break;\n}\n";
    let root = parse(&gc, &mut sm, src);
    let original_id = first_statement(root).node_id();
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .expect("resolution failed");

    let switch_stmt = first_statement(resolved);
    assert_ne!(
        switch_stmt.node_id(),
        original_id,
        "non-degeneracy: the fold must have REBUILT the SwitchStatement, \
         otherwise this test proves nothing"
    );
    let Node::SwitchStatement(sw) = switch_stmt else {
        unreachable!("not a SwitchStatement")
    };
    assert!(
        matches!(sw.discriminant, Node::NumericLiteral(_)),
        "non-degeneracy: the discriminant must have folded"
    );
    assert_eq!(
        sw.label_index.get(),
        0,
        "the REBUILT switch lost its label index"
    );
    assert!(
        sw.scope.get().is_some(),
        "the REBUILT switch lost its scope decoration"
    );
    let Some(Node::SwitchCase(case)) = sw.cases.iter().next() else {
        unreachable!("no SwitchCase")
    };
    let brk = case.consequent.iter().next().expect("no break");
    assert_eq!(
        label_index(brk),
        0,
        "the `break` must target the switch's label"
    );
}

/// A loop nobody rewrites must come back pointer-identical: writing the
/// label index and the scope through `Cell`s must not force a rebuild.
#[test]
fn an_unrewritten_loop_is_returned_as_is() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let src = "for (var i = 0; i < 10; ++i) {\n  break;\n}\n";
    let root = parse(&gc, &mut sm, src);
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .expect("resolution failed");
    assert!(
        std::ptr::eq(root, resolved),
        "an unrewritten loop must be returned as-is, not rebuilt"
    );
}

/// The label is erased from `labelMap` when its statement is left
/// (`make_scope_exit`, cpp:670-675), so the same name may be reused by a
/// *sibling* labeled statement without a duplicate-definition error.
#[test]
fn a_label_is_erased_on_leaving_its_statement() {
    // `resolve` panics if resolution failed, so reaching the assert proves
    // no "label 'l' is already defined" error was reported.
    let (sem_ctx, _) = resolve("l: ;\nl: ;\n");
    assert_eq!(sem_ctx.function(sem_ctx.get_global_function()).num_labels, 2);
}

/// The label's *target statement* walk (cpp:642-652): a label directly
/// enclosing another label that encloses a loop resolves to the LOOP, so
/// `continue l1` is legal and points at the `while` — two levels down.
#[test]
fn a_label_enclosing_a_label_enclosing_a_loop_targets_the_loop() {
    let src = "l1: l2: while (a) {\n  continue l1;\n}\n";
    with_resolved(src, |_sem_ctx, resolved| {
        let l1_node = first_statement(resolved);
        assert_eq!(label_index(l1_node), 0, "l1: itself");
        let Node::LabeledStatement(l1) = l1_node else {
            unreachable!("not a LabeledStatement")
        };
        assert_eq!(label_index(l1.body), 1, "l2: itself");
        let Node::LabeledStatement(l2) = l1.body else {
            unreachable!("not a LabeledStatement")
        };
        assert_eq!(label_index(l2.body), 2, "the while");
        let Node::WhileStatement(w) = l2.body else {
            unreachable!("not a WhileStatement")
        };
        let Node::BlockStatement(block) = w.body else {
            unreachable!("not a BlockStatement")
        };
        let cont = block.body.iter().next().expect("empty body");
        assert_eq!(
            label_index(cont),
            2,
            "`continue l1` must reach the while (label 2) through l2"
        );
    });
}

/// The same decorate-then-rebuild hazard as
/// `a_rebuilt_switch_keeps_its_label_index_and_scope`, for a `for` loop: a
/// fold in the init rebuilds the `ForStatement`, whose label index and scope
/// were written before `visit_children_mut` snapshotted the builder.
#[test]
fn a_rebuilt_for_loop_keeps_its_label_index_and_scope() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let src = "for (var i = 1 + 2; ; ) {\n  break;\n}\n";
    let root = parse(&gc, &mut sm, src);
    let original_id = first_statement(root).node_id();
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .expect("resolution failed");

    let for_node = first_statement(resolved);
    assert_ne!(
        for_node.node_id(),
        original_id,
        "non-degeneracy: the fold must have REBUILT the ForStatement"
    );
    let Node::ForStatement(f) = for_node else {
        unreachable!("not a ForStatement")
    };
    assert_eq!(f.label_index.get(), 0, "the REBUILT for lost its label");
    assert!(f.scope.get().is_some(), "the REBUILT for lost its scope");
    let Node::BlockStatement(block) = f.body else {
        unreachable!("not a BlockStatement")
    };
    let brk = block.body.iter().next().expect("no break");
    assert_eq!(label_index(brk), 0);
}

// ==== Arrow functions (S2 T2) ========================================
//
// `visit(ArrowFunctionExpressionNode *)` (SemanticResolver.cpp:249-275) is
// two things the differential can only partly see: the **expression-body →
// block+return rewrite** (:253-266), which IS printed by `-dump-sema` (the
// corpus files pin it byte-for-byte), and the
// `containsArrowFunctions`/`containsArrowFunctionsUsingArguments`
// bookkeeping (:270-274), which `SemContextDumper` never prints at all.
// These tests pin the synthesized nodes' shape and locations — neither of
// which the dump shows — and the two flags.

/// \return the `ArrowFunctionExpression` initializing the single declarator
/// of a `var` statement.
fn arrow_of_var<'gc>(stmt: &'gc Node<'gc>) -> &'gc Node<'gc> {
    let Node::VariableDeclaration(vd) = stmt else {
        panic!("not a VariableDeclaration: {}", stmt.node_type_str())
    };
    let Some(Node::VariableDeclarator(d)) = vd.declarations.iter().next() else {
        panic!("no VariableDeclarator")
    };
    d.init.expect("declarator has no initializer")
}

/// **Rewrite #1** (cpp:253-266): `compile_ && _expression` replaces the
/// arrow's expression body with `BlockStatement([ReturnStatement(body)],
/// /* implicit */ true)`, both synthesized nodes taking their location from
/// the ORIGINAL body (`copyLocationFrom`, :255 and :262), and clears
/// `_expression`.
#[test]
fn an_expression_bodied_arrow_is_rewritten_to_a_block_with_return() {
    with_resolved("var f = (x) => x;\n", |_sem_ctx, resolved| {
        let arrow_node = arrow_of_var(first_statement(resolved));
        let Node::ArrowFunctionExpression(arrow) = arrow_node else {
            panic!("not an arrow: {}", arrow_node.node_type_str())
        };
        assert!(
            !arrow.expression.get(),
            "the RETURNED arrow must carry expression = false"
        );
        let Node::BlockStatement(block) = arrow.body else {
            panic!(
                "body is not a BlockStatement: {}",
                arrow.body.node_type_str()
            )
        };
        assert!(block.implicit.get(), "the synthesized block is implicit");
        let mut stmts = block.body.iter();
        let Some(Node::ReturnStatement(ret)) = stmts.next() else {
            panic!("the block's only statement is not a ReturnStatement")
        };
        assert!(stmts.next().is_none(), "the block has exactly one statement");
        let arg = ret.argument.expect("the return has no argument");
        assert!(
            matches!(arg, Node::Identifier(_)),
            "the returned expression is the original body"
        );
        // copyLocationFrom(arrowFunc->_body) — range AND debug location.
        assert_eq!(block.metadata.range.get(), arg.range());
        assert_eq!(ret.metadata.range.get(), arg.range());
        assert_eq!(
            block.metadata.debug_loc.get(),
            arg.metadata().debug_loc.get()
        );
        assert_eq!(
            ret.metadata.debug_loc.get(),
            arg.metadata().debug_loc.get()
        );
    });
}

/// An arrow that already has a block body is left completely alone: the
/// `compile_ && _expression` guard is false, so no node is allocated and the
/// arrow comes back pointer-identical (C++ would not have mutated it).
#[test]
fn a_block_bodied_arrow_is_not_rewritten() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let root = parse(&gc, &mut sm, "var f = (x) => { return x; };\n");
    let original_id = arrow_of_var(first_statement(root)).node_id();
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .expect("resolution failed");

    let arrow_node = arrow_of_var(first_statement(resolved));
    assert_eq!(
        arrow_node.node_id(),
        original_id,
        "a block-bodied arrow must not be rebuilt"
    );
    let Node::ArrowFunctionExpression(arrow) = arrow_node else {
        panic!("not an arrow")
    };
    assert!(!arrow.expression.get());
}

/// The decorate-before-recurse hazard for the rewrite: the rewritten arrow is
/// visited, and a fold inside the synthesized `ReturnStatement` rebuilds the
/// `BlockStatement` and therefore the arrow a SECOND time. The arrow this
/// visit returns must still carry `expression = false` and the `sem_info`
/// that `enter_function` wrote on the pre-rebuild node.
#[test]
fn a_rewritten_arrow_whose_body_folds_keeps_its_decorations() {
    with_resolved("var f = () => 1 + 2;\n", |sem_ctx, resolved| {
        let arrow_node = arrow_of_var(first_statement(resolved));
        let Node::ArrowFunctionExpression(arrow) = arrow_node else {
            panic!("not an arrow")
        };
        assert!(
            !arrow.expression.get(),
            "the twice-rebuilt arrow lost expression = false"
        );
        let sem_info = arrow
            .sem_info
            .get()
            .expect("the twice-rebuilt arrow lost its sem_info");
        assert!(sem_ctx.function(FunctionInfoId::from_sema_id(sem_info)).arrow);
        let Node::BlockStatement(block) = arrow.body else {
            panic!("body is not a BlockStatement")
        };
        let Some(Node::ReturnStatement(ret)) = block.body.iter().next() else {
            panic!("no ReturnStatement")
        };
        // Non-degeneracy: the fold must actually have happened, which is what
        // forced the second rebuild.
        assert!(
            matches!(ret.argument, Some(Node::NumericLiteral(_))),
            "1 + 2 did not fold, so the arrow was never rebuilt twice"
        );
    });
}

/// `containsArrowFunctions` + `containsArrowFunctionsUsingArguments`
/// (cpp:270-274) propagate from the arrow's OWN `usesArguments` to the
/// enclosing function. Invisible to `-dump-sema`.
#[test]
fn an_arrow_using_arguments_propagates_to_the_enclosing_function() {
    let src = "function f() { var g = () => arguments; }\n";
    with_resolved(src, |sem_ctx, resolved| {
        let f = sem_ctx.function(sem_info_of(first_statement(resolved)));
        assert!(f.contains_arrow_functions);
        assert!(
            f.contains_arrow_functions_using_arguments,
            "the arrow's usesArguments must reach f"
        );
        assert!(
            !f.uses_arguments,
            "f itself does not reference 'arguments' (the arrow does)"
        );
        // The global function contains no arrow of its own.
        let global = sem_ctx.function(sem_ctx.get_global_function());
        assert!(!global.contains_arrow_functions);
        assert!(!global.contains_arrow_functions_using_arguments);
    });
}

/// The propagation is transitive through the arrow's own
/// `containsArrowFunctionsUsingArguments` (the second disjunct, cpp:273):
/// only the INNER arrow references `arguments`.
#[test]
fn nested_arrows_propagate_arguments_use_outward() {
    let src = "function f() { var g = () => { var h = () => arguments; }; }\n";
    with_resolved(src, |sem_ctx, resolved| {
        let f = sem_ctx.function(sem_info_of(first_statement(resolved)));
        assert!(f.contains_arrow_functions);
        assert!(f.contains_arrow_functions_using_arguments);
    });
}

/// The flag reads the ARROW's `semInfo`, not the enclosing function's: `f`
/// uses `arguments` itself and contains an arrow that does not, which must
/// leave `containsArrowFunctionsUsingArguments` false.
#[test]
fn an_arrow_not_using_arguments_leaves_the_propagation_flag_clear() {
    let src = "function f() { arguments; var g = () => 1; }\n";
    with_resolved(src, |sem_ctx, resolved| {
        let f = sem_ctx.function(sem_info_of(first_statement(resolved)));
        assert!(f.uses_arguments, "f references 'arguments' directly");
        assert!(f.contains_arrow_functions);
        assert!(
            !f.contains_arrow_functions_using_arguments,
            "f's own usesArguments must not leak into the arrow flag"
        );
    });
}

// ---- S2 T3: try/catch (rewrite #2), `with` + the Unresolver -------------

/// Parse + resolve `src`, requiring resolution to FAIL with exactly
/// `errors` errors, and hand the closure the `GCLock`, the `SemContext` and
/// the root node that went IN.
///
/// `resolve_ast` returns `None` on any error (C++'s `false`), so it cannot
/// give back a root for the `with` tests below — `visit(WithStatementNode *)`
/// always reports "with statement is not supported" when `compile_` is set.
/// Using the input root is sound only because nothing in these inputs
/// rewrites anything (no fold, no arrow, no try/catch+finally), so no
/// ancestor is ever rebuilt and the input root IS the decorated tree; the
/// `identifier_states` assertions (which would see `false`/no-decl on a
/// stale tree) plus `unrewritten_resolution_returns_the_same_root` are what
/// keep that assumption honest.
///
/// A handler is installed so the expected diagnostics do not print to the
/// test runner's stderr when the resolver flushes its buffer.
fn with_failed_resolution<R>(
    src: &str,
    errors: u32,
    f: impl for<'gc> FnOnce(&GCLock, &SemContext, &'gc Node<'gc>) -> R,
) -> R {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    sm.set_handler(Box::new(SharedHandler(Rc::default())));
    let root = parse(&gc, &mut sm, src);
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    assert!(
        resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[]).is_none(),
        "expected resolution to fail for: {src}"
    );
    assert_eq!(sm.error_count(), errors, "unexpected error count for: {src}");
    f(&gc, &sem_ctx, root)
}

/// Every `Identifier` in `node`'s subtree, in visit order, as
/// `(name, unresolvable)`.
fn identifier_states<'gc>(
    gc: &GCLock,
    node: &'gc Node<'gc>,
) -> Vec<(String, bool)> {
    struct Collect<'a, 'b, 'c> {
        gc: &'a GCLock<'b, 'c>,
        out: Vec<(String, bool)>,
    }
    impl<'gc> ast::visitor::Visitor<'gc> for Collect<'_, '_, '_> {
        fn visit_node(&mut self, node: &'gc Node<'gc>) {
            if let Node::Identifier(id) = node {
                let name = atom_string(self.gc, id.name.get());
                self.out.push((name, id.unresolvable.get()));
            }
            node.visit_children(self);
        }
    }
    let mut c = Collect { gc, out: Vec::new() };
    ast::visitor::Visitor::visit_node(&mut c, node);
    c.out
}

/// The third statement of a resolved `Program`, as a `WithStatement`.
fn third_with_statement<'gc>(
    root: &'gc Node<'gc>,
) -> &'gc ast::node::WithStatement<'gc> {
    let Node::Program(p) = root else {
        unreachable!("not a Program root")
    };
    p.body
        .iter()
        .nth(2)
        .expect("no third statement")
        .as_with_statement()
        .expect("the third statement is not a WithStatement")
}

/// **The `Unresolver` pass** (`resolver/unresolver.rs`, port of
/// SemanticResolver.h:679-711 + cpp:3186-3210), reached through
/// `visit(WithStatementNode *)` (cpp:763-768).
///
/// `with` runs the pass over its BODY with `curScope_->depth + 1`, so:
/// every identifier in the body that resolved to a declaration in a scope
/// *shallower* than that loses its resolution and gets `unresolvable`; a
/// declaration made inside the `with` body (depth 1 here, i.e. not less than
/// 0 + 1) keeps it; and the `with`'s own `_object` — outside the pass's root
/// — is untouched.
///
/// The differential is blind to all of this: hermesc reports the
/// not-supported error and exits before printing any dump (`error-with.js`
/// in the corpus pins that), so this test is the only pin on the pass.
#[test]
fn with_statement_unresolves_identifiers_above_its_depth() {
    let src = "var o = {a: 1};\nvar outer = 1;\n\
               with (o) {\n  let inner;\n  outer;\n  inner;\n  o;\n}\n";
    with_failed_resolution(src, 1, |gc, sem_ctx, root| {
        let with = third_with_statement(root);

        // `_object` is outside the Unresolver's root, so `o` here keeps its
        // resolution.
        assert_eq!(
            identifier_states(gc, with.object),
            vec![("o".to_string(), false)]
        );

        // Inside the body: `let inner` (depth 1) and every reference to it
        // keep their decl; the two globals (depth 0 < 1) are unresolved.
        assert_eq!(
            identifier_states(gc, with.body),
            vec![
                ("inner".to_string(), false),
                ("outer".to_string(), true),
                ("inner".to_string(), false),
                ("o".to_string(), true),
            ]
        );

        // `setExpressionDecl(node, nullptr)` ran too, not just the flag: the
        // dumper prints ` UNR` and NO `[...]` bracket for those two, and the
        // untouched ones still print their decl.
        let mut out = Vec::new();
        sem_dump(&mut out, gc, sem_ctx, with.body);
        let dumped = String::from_utf8(out).expect("dump is not UTF-8");
        assert!(dumped.contains("Id 'outer' UNR\n"), "{dumped}");
        assert!(dumped.contains("Id 'o' UNR\n"), "{dumped}");
        assert!(dumped.contains("Id 'inner' [D:E:"), "{dumped}");
    });
}

/// The pass early-returns on an identifier that is already `unresolvable`
/// (cpp:3193-3195), which is what keeps `SemContext::get_expression_decl`'s
/// "not on an unresolvable identifier" assertion from firing when a second,
/// nested `with` walks the same subtree.
#[test]
fn nested_with_statements_do_not_re_unresolve() {
    let src = "var o = {};\nvar outer = 1;\n\
               with (o) { with (o) { outer; } }\n";
    with_failed_resolution(src, 2, |gc, _sem_ctx, root| {
        let with = third_with_statement(root);
        // The inner `with`'s pass (depth 1 + 1 = 2) runs first and unresolves
        // `o` and `outer`; the outer one (depth 0 + 1 = 1) then walks the same
        // identifiers, hits the early return, and leaves them alone.
        assert_eq!(
            identifier_states(gc, with.body),
            vec![("o".to_string(), true), ("outer".to_string(), true)]
        );
    });
}

/// **Rewrite #2** (`visit(TryStatementNode *)`, cpp:771-811): a `try` with
/// both a handler and a finalizer becomes
/// `try { try <block> catch <handler> } finally <finalizer>`.
///
/// `-dump-sema` shows the resulting *shape* (`try-catch-finally.js` in the
/// corpus compares it byte-for-byte) but prints no source locations at all,
/// so the two `copyLocationFrom`/`setEndLoc` calls (cpp:797-798, 804) can
/// only be checked here. The synthesized nested `try` spans from the
/// original `try` keyword to the END of the handler; the wrapper block
/// copies that same range and is NOT implicit (cpp:803's `false`).
#[test]
fn try_with_catch_and_finally_is_rewritten_into_nested_trys() {
    let src = "try { 1; } catch (e) { 2; } finally { 3; }\n";
    with_resolved(src, |_sem_ctx, resolved| {
        let outer = first_statement(resolved)
            .as_try_statement()
            .expect("not a TryStatement");
        assert!(
            outer.handler.is_none(),
            "the outer statement must have given its handler away"
        );
        assert!(outer.finalizer.is_some());

        // tryStatement->_block = BlockStatementNode({nestedTry}, false)
        let wrapper = outer
            .block
            .as_block_statement()
            .expect("the new block is not a BlockStatement");
        assert!(!wrapper.implicit.get(), "cpp:803 passes `false`");
        let mut wrapper_body = wrapper.body.iter();
        let nested = wrapper_body
            .next()
            .expect("the wrapper block is empty")
            .as_try_statement()
            .expect("the wrapper's only statement is not a TryStatement");
        assert!(
            wrapper_body.next().is_none(),
            "the wrapper block holds exactly one statement"
        );
        assert!(nested.handler.is_some());
        assert!(
            nested.finalizer.is_none(),
            "the nested try must have no finalizer, or it would rewrite again"
        );

        // nestedTry->copyLocationFrom(tryStatement) +
        // nestedTry->setEndLoc(nestedTry->_handler->getEndLoc()).
        let whole = first_statement(resolved).range();
        let handler_range = nested.handler.unwrap().range();
        let nested_range = nested.metadata.range.get();
        assert_eq!(nested_range.start, whole.start);
        assert_eq!(nested_range.end, handler_range.end);
        assert_ne!(
            nested_range.end, whole.end,
            "a `finally` follows the handler, so the ranges must differ"
        );
        // tryStatement->_block->copyLocationFrom(nestedTry).
        assert_eq!(wrapper.metadata.range.get(), nested_range);
    });
}

/// A `try` missing either half is left alone: `visit(TryStatementNode *)`
/// returns `Unchanged`, so the whole tree comes back pointer-identical.
#[test]
fn try_without_both_handler_and_finalizer_is_not_rewritten() {
    let sources =
        ["try { 1; } catch (e) { 2; }\n", "try { 1; } finally { 2; }\n"];
    for src in sources {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let root = parse(&gc, &mut sm, src);
        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let resolved = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
            .unwrap_or_else(|| panic!("resolution failed for: {src}"));
        assert!(
            std::ptr::eq(resolved, root),
            "{src} must not rebuild anything"
        );
    }
}

/// The rewrite is a replacement, so it must be reported even when the
/// children walk below it changes nothing further — otherwise the parent
/// would keep the pre-rewrite subtree. Same shape as rewrite #1's
/// `Unchanged if rewritten` arm.
#[test]
fn the_try_rewrite_is_reported_even_when_no_child_changes() {
    let src = "try { } catch (e) { } finally { }\n";
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let root = parse(&gc, &mut sm, src);
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .expect("resolution failed");
    assert!(
        !std::ptr::eq(resolved, root),
        "the rewrite must have rebuilt the Program"
    );
    let outer = first_statement(resolved)
        .as_try_statement()
        .expect("not a TryStatement");
    assert!(outer.handler.is_none());
}

/// `visit(CatchClauseNode *)`'s `ScopeRAII` decoration must survive a
/// rebuild forced from inside the clause: a fold in the catch BODY rebuilds
/// the body block, hence the `CatchClause`, and `scope` is a `Cell` the
/// builder snapshots (see `resolver/mod.rs`'s "decorate before recursing").
#[test]
fn a_rebuilt_catch_clause_keeps_its_scope() {
    let src = "try { } catch (e) { 1 + 2; }\n";
    with_resolved(src, |sem_ctx, resolved| {
        let outer = first_statement(resolved)
            .as_try_statement()
            .expect("not a TryStatement");
        let catch = outer
            .handler
            .expect("no handler")
            .as_catch_clause()
            .expect("handler is not a CatchClause");
        let scope = catch.scope.get().expect("the rebuilt clause lost `scope`");
        // The catch parameter is declared in that very scope.
        let scope_id = sema::ids::ScopeId::from_sema_id(scope);
        let decls = &sem_ctx.scope(scope_id).decls;
        assert_eq!(decls.len(), 1, "the catch param must be the only decl");
        assert_eq!(sem_ctx.decl(decls[0]).kind, DeclKind::ES5Catch);
        // Non-degeneracy: the fold must have happened, i.e. the clause
        // really was rebuilt.
        let body = catch
            .body
            .as_block_statement()
            .expect("catch body is not a block");
        let Some(Node::ExpressionStatement(es)) = body.body.iter().next() else {
            panic!("no ExpressionStatement in the catch body")
        };
        assert!(
            matches!(es.expression, Node::NumericLiteral(_)),
            "1 + 2 did not fold, so the CatchClause was never rebuilt"
        );
    });
}

// ---- S2 T4: classes ----------------------------------------------------
//
// The three synthetic `FunctionInfo`s a class can carry (the implicit
// constructor and the instance/static elements initializers) live in `Cell`s
// on the CLASS node, and every one of them is written *after* the class body
// has been visited or from deep inside it — so a rebuild of the class node
// (a fold in a field initializer, a rewritten arrow in a method) is exactly
// the shape that could lose them. The differential sees the ids indirectly
// (the synthetic functions appear in the `-dump-sema` function tree), but
// not which class node they are attached to; these tests pin that.

/// The three `ClassLikeDecoration` `Cell`s of a class node, as
/// `(implicitCtor, instanceElementsInit, staticElementsInit)`.
fn class_function_infos(
    node: &Node,
) -> (Option<ast::SemaId>, Option<ast::SemaId>, Option<ast::SemaId>) {
    match node {
        Node::ClassDeclaration(n) => (
            n.implicit_ctor_function_info.get(),
            n.instance_elements_init_function_info.get(),
            n.static_elements_init_function_info.get(),
        ),
        Node::ClassExpression(n) => (
            n.implicit_ctor_function_info.get(),
            n.instance_elements_init_function_info.get(),
            n.static_elements_init_function_info.get(),
        ),
        _ => panic!("no class decoration on {}", node.node_type_str()),
    }
}

/// A class with no explicit constructor gets a synthetic implicit-constructor
/// `FunctionInfo` (`createImplicitConstructorFunctionInfo`, cpp:3088-3114),
/// whose `ConstructorKind` is `Base` for a plain class and `Derived` for one
/// with a superclass, and which owns exactly one (body) scope.
#[test]
fn an_implicit_constructor_function_info_is_created_for_a_class_without_one() {
    with_resolved("class A {}\n", |sem_ctx, resolved| {
        let class = first_statement(resolved);
        let (ctor, inst, stat) = class_function_infos(class);
        assert!(inst.is_none() && stat.is_none());
        let ctor = FunctionInfoId::from_sema_id(
            ctor.expect("no implicit constructor FunctionInfo"),
        );
        let info = sem_ctx.function(ctor);
        assert_eq!(
            info.constructor_kind,
            sema::sem_context::ConstructorKind::Base
        );
        assert!(info.strict, "an implicit constructor is always strict");
        assert_eq!(info.get_scopes().len(), 1);
        assert_eq!(info.get_function_body_scope(), info.get_scopes()[0]);
    });

    // The `Derived` half of cpp:3097-3099 — `isDerivedClass()` is the only
    // input, so a derived class with NO explicit constructor is the only
    // shape that reaches it. Dump-invisible, like the `Base` case above.
    with_resolved("class A {}\nclass B extends A {}\n", |sem_ctx, resolved| {
        let Node::Program(p) = resolved else {
            unreachable!("not a Program")
        };
        let b = p.body.iter().nth(1).expect("no class B");
        let ctor = FunctionInfoId::from_sema_id(
            class_function_infos(b)
                .0
                .expect("B has no implicit constructor FunctionInfo"),
        );
        assert_eq!(
            sem_ctx.function(ctor).constructor_kind,
            sema::sem_context::ConstructorKind::Derived
        );
    });
}

/// The `hasConstructor` flag (set from `visitFunctionLike`, cpp:1656)
/// suppresses the implicit constructor, and a `constructor` in a derived
/// class gets `ConstructorKind::Derived`.
#[test]
fn an_explicit_constructor_suppresses_the_implicit_one() {
    let src = "class A {}\nclass B extends A { constructor() {} }\n";
    with_resolved(src, |sem_ctx, resolved| {
        let Node::Program(p) = resolved else {
            unreachable!("not a Program")
        };
        let mut it = p.body.iter();
        let a = it.next().expect("no class A");
        let b = it.next().expect("no class B");
        assert!(
            class_function_infos(a).0.is_some(),
            "A has no explicit constructor"
        );
        assert!(
            class_function_infos(b).0.is_none(),
            "B's explicit constructor must suppress the implicit one"
        );
        // The explicit constructor's own FunctionInfo is Derived.
        let body = b
            .as_class_declaration()
            .expect("not a ClassDeclaration")
            .body
            .as_class_body()
            .expect("not a ClassBody");
        let method = body
            .body
            .iter()
            .next()
            .expect("empty class body")
            .as_method_definition()
            .expect("not a MethodDefinition");
        let func = method
            .value
            .as_function_expression()
            .expect("method value is not a FunctionExpression");
        let id = FunctionInfoId::from_sema_id(
            func.sem_info.get().expect("no semInfo on the constructor"),
        );
        assert_eq!(
            sem_ctx.function(id).constructor_kind,
            sema::sem_context::ConstructorKind::Derived
        );
    });
}

/// A field initializer that FOLDS rebuilds the `ClassBody` and hence the
/// class node — the instance-elements-init id was written on the original
/// node from inside that walk and the implicit-constructor id after it, so
/// both must be present on the node the resolver RETURNED.
#[test]
fn a_rebuilt_class_keeps_its_synthetic_function_infos() {
    let src = "class C { x = 1 + 2; static y; }\n";
    with_resolved(src, |sem_ctx, resolved| {
        let class = first_statement(resolved);
        let (ctor, inst, stat) = class_function_infos(class);
        assert!(ctor.is_some(), "the rebuilt class lost implicitCtor");
        let inst = FunctionInfoId::from_sema_id(
            inst.expect("the rebuilt class lost instanceElementsInit"),
        );
        let stat = FunctionInfoId::from_sema_id(
            stat.expect("the rebuilt class lost staticElementsInit"),
        );
        assert_ne!(inst, stat);
        // The `ScopeRAII` decoration (written before the walk) must survive
        // the same rebuild — unlike the three ids, THIS one the differential
        // does see (`ClassDeclaration Scope %s.N`).
        assert!(
            class
                .as_class_declaration()
                .expect("not a ClassDeclaration")
                .scope
                .get()
                .is_some(),
            "the rebuilt class lost `scope`"
        );
        // The instance initializer declared `arguments` (cpp:1039); the
        // static one never ran `declareArguments` (`static y` has no value).
        assert!(sem_ctx.function(inst).arguments_decl.is_some());
        assert!(sem_ctx.function(stat).arguments_decl.is_none());
        // Non-degeneracy: the fold must really have happened.
        let body = class
            .as_class_declaration()
            .expect("not a ClassDeclaration")
            .body
            .as_class_body()
            .expect("not a ClassBody");
        let prop = body
            .body
            .iter()
            .next()
            .expect("empty class body")
            .as_class_property()
            .expect("not a ClassProperty");
        assert!(
            matches!(prop.value, Some(Node::NumericLiteral(_))),
            "1 + 2 did not fold, so the class was never rebuilt"
        );
    });
}

/// A class declaration carries TWO decls on its one `Identifier`: the
/// hoisted `Class` declaration decl and the inner `ClassExprName`
/// expression decl the class body sees (cpp:923-935) — the side-table case.
#[test]
fn a_class_declaration_name_carries_both_a_class_and_a_class_expr_name_decl() {
    with_resolved("class C {}\n", |sem_ctx, resolved| {
        let class = first_statement(resolved)
            .as_class_declaration()
            .expect("not a ClassDeclaration");
        let id = class
            .id
            .expect("no class id")
            .as_identifier()
            .expect("class id is not an Identifier");
        let decl =
            sem_ctx.get_declaration_decl(id).expect("no declaration decl");
        let expr = sem_ctx.get_expression_decl(id).expect("no expression decl");
        assert_ne!(decl, expr);
        assert_eq!(sem_ctx.decl(decl).kind, DeclKind::Class);
        assert_eq!(sem_ctx.decl(expr).kind, DeclKind::ClassExprName);
        // The ClassExprName lives in the class node's own scope.
        let scope = sema::ids::ScopeId::from_sema_id(
            class.scope.get().expect("no scope on the class"),
        );
        assert_eq!(sem_ctx.decl(expr).scope, Some(scope));
    });
}

/// Classes force strict mode on the ENCLOSING function only for the
/// duration of the class (cpp:919's `SaveAndRestore`): the methods are
/// strict, the global function goes back to loose.
#[test]
fn a_class_forces_strict_mode_only_inside_itself() {
    with_resolved("class C { m() {} }\n", |sem_ctx, resolved| {
        assert!(
            !sem_ctx.function(sem_ctx.get_global_function()).strict,
            "the global function must be loose again after the class"
        );
        let Node::Program(p) = resolved else {
            unreachable!("not a Program")
        };
        assert_eq!(p.strictness.get(), Strictness::NonStrictMode);
        let class = first_statement(resolved)
            .as_class_declaration()
            .expect("not a ClassDeclaration");
        let method = class
            .body
            .as_class_body()
            .expect("not a ClassBody")
            .body
            .iter()
            .next()
            .expect("empty class body")
            .as_method_definition()
            .expect("not a MethodDefinition");
        let func = method
            .value
            .as_function_expression()
            .expect("method value is not a FunctionExpression");
        let id = FunctionInfoId::from_sema_id(
            func.sem_info.get().expect("no semInfo on the method"),
        );
        assert!(sem_ctx.function(id).strict, "a method must be strict");
        assert_eq!(func.strictness.get(), Strictness::StrictMode);
    });
}

/// The `ClassContext` stack: a class nested inside a method of another class
/// gets its OWN context, so the inner class's explicit constructor must not
/// suppress the OUTER class's implicit one (C++'s `curClassContext_` linked
/// list; here `SemanticResolver::class_stack`).
#[test]
fn a_nested_class_gets_its_own_class_context() {
    let src = "class Outer {\n  m() {\n    class Inner extends Outer {\n\
               \x20     constructor() {}\n    }\n    return Inner;\n  }\n}\n";
    with_resolved(src, |sem_ctx, resolved| {
        let outer = first_statement(resolved);
        assert!(
            class_function_infos(outer).0.is_some(),
            "Outer has no explicit constructor of its own"
        );
        // Walk to the inner class declaration.
        let method = outer
            .as_class_declaration()
            .expect("not a ClassDeclaration")
            .body
            .as_class_body()
            .expect("not a ClassBody")
            .body
            .iter()
            .next()
            .expect("empty class body")
            .as_method_definition()
            .expect("not a MethodDefinition");
        let block = method
            .value
            .as_function_expression()
            .expect("method value is not a FunctionExpression")
            .body
            .as_block_statement()
            .expect("method body is not a block");
        let inner = block.body.iter().next().expect("empty method body");
        assert!(
            class_function_infos(inner).0.is_none(),
            "Inner's explicit constructor must suppress ITS implicit one"
        );
        // And Inner's constructor is Derived (it extends Outer), which is
        // what proves `cur_class_is_derived` read Inner's context, not
        // Outer's.
        let ctor = inner
            .as_class_declaration()
            .expect("Inner is not a ClassDeclaration")
            .body
            .as_class_body()
            .expect("not a ClassBody")
            .body
            .iter()
            .next()
            .expect("empty inner class body")
            .as_method_definition()
            .expect("not a MethodDefinition");
        let id = FunctionInfoId::from_sema_id(
            ctor.value
                .as_function_expression()
                .expect("not a FunctionExpression")
                .sem_info
                .get()
                .expect("no semInfo"),
        );
        assert_eq!(
            sem_ctx.function(id).constructor_kind,
            sema::sem_context::ConstructorKind::Derived
        );
    });
}

// ---- S2 T5: private names + static blocks ------------------------------

/// The private-name mangling (`Context::getPrivateNameIdentifier`,
/// AST/Context.h:389-393) keeps a private `Decl` out of the ordinary
/// variable namespace: `#x`'s decl name is NOT the `x` its `Identifier` node
/// carries, so the `var x` in the method below is a completely separate decl.
/// (The exact `#`-prefixed spelling is pinned by `sem_context.rs`'s
/// `private_name_identifier_prefixes_a_hash`, which has a `GCLock` to read
/// atom text with, and end-to-end by the `Decl %d.N '#x' PrivateField` line
/// the differential compares.)
#[test]
fn a_private_field_decl_is_not_the_same_as_a_same_named_variable() {
    let src = "class C { #x; m() { var x; return this.#x; } }\n";
    with_resolved(src, |sem_ctx, resolved| {
        let class = first_statement(resolved);
        let class_decl =
            class.as_class_declaration().expect("not a ClassDeclaration");
        let scope = sema::ids::ScopeId::from_sema_id(
            class_decl.scope.get().expect("no scope on the class"),
        );
        let body = class_decl.body.as_class_body().expect("not a ClassBody");
        let mut elms = body.body.iter();
        // The `ClassPrivateProperty`'s key is a bare `Identifier` (no
        // `PrivateName` wrapper), bound by `declarePrivateName`'s
        // `setBothDecl`.
        let key = elms
            .next()
            .expect("empty class body")
            .as_class_private_property()
            .expect("not a ClassPrivateProperty")
            .key
            .as_identifier()
            .expect("a ClassPrivateProperty key is an Identifier");
        let private_decl =
            sem_ctx.get_expression_decl(key).expect("unresolved private name");
        assert_eq!(sem_ctx.get_declaration_decl(key), Some(private_decl));
        assert_eq!(sem_ctx.decl(private_decl).kind, DeclKind::PrivateField);
        assert_eq!(
            sem_ctx.decl(private_decl).special,
            sema::sem_context::DeclSpecial::NotSpecial,
            "a FIELD never gets PrivateStatic (cpp:2182 passes isStatic=false)"
        );
        // It lives in the class's own scope, right after the ClassExprName.
        assert_eq!(sem_ctx.decl(private_decl).scope, Some(scope));
        assert_eq!(sem_ctx.scope(scope).decls.len(), 2);
        assert_eq!(sem_ctx.scope(scope).decls[1], private_decl);
        // The decl's NAME is the mangled one, so it can never be the atom the
        // `Identifier` node carries.
        assert_ne!(sem_ctx.decl(private_decl).name, key.name.get());
    });
}

/// A legal getter+setter pair collapses onto ONE decl, whose kind was
/// UPGRADED in place from `PrivateGetter` to `PrivateGetterSetter`
/// (cpp:2253-2255) — and BOTH accessors' identifier nodes are bound to it
/// (the `setBothDecl` at cpp:2257). `isStatic` on both halves becomes
/// `Decl::Special::PrivateStatic`.
#[test]
fn a_private_getter_setter_pair_shares_one_upgraded_decl() {
    let src = "class C { static get #x() {} static set #x(v) {} }\n";
    with_resolved(src, |sem_ctx, resolved| {
        let class = first_statement(resolved);
        let body = class
            .as_class_declaration()
            .expect("not a ClassDeclaration")
            .body
            .as_class_body()
            .expect("not a ClassBody");
        let decls: Vec<sema::ids::DeclId> = body
            .body
            .iter()
            .map(|elm| {
                let key = elm
                    .as_method_definition()
                    .expect("not a MethodDefinition")
                    .key
                    .as_private_name()
                    .expect("key is not a PrivateName")
                    .id
                    .as_identifier()
                    .expect("a PrivateName's id is an Identifier");
                let decl =
                    sem_ctx.get_expression_decl(key).expect("unresolved");
                assert_eq!(
                    sem_ctx.get_declaration_decl(key),
                    Some(decl),
                    "setBothDecl must set both to the same decl"
                );
                decl
            })
            .collect();
        assert_eq!(decls.len(), 2);
        assert_eq!(decls[0], decls[1], "the pair must share one decl");
        assert_eq!(
            sem_ctx.decl(decls[0]).kind,
            DeclKind::PrivateGetterSetter,
            "the second accessor must upgrade the first's decl in place"
        );
        assert_eq!(
            sem_ctx.decl(decls[0]).special,
            sema::sem_context::DeclSpecial::PrivateStatic
        );
    });
}

/// A `var` inside a static block hoists to the STATIC BLOCK's own body scope,
/// not to the function the class lives in (cpp:1058-1064): the block's
/// `FunctionInfo` is synthetic, flagged `isStaticBlock`, and owns that scope.
#[test]
fn a_static_block_hoists_its_vars_into_its_own_function_scope() {
    let src = "function f() { class C { static { var x; } } }\n";
    with_resolved(src, |sem_ctx, resolved| {
        let func = first_statement(resolved);
        let outer = FunctionInfoId::from_sema_id(
            func.as_function_declaration()
                .expect("not a FunctionDeclaration")
                .sem_info
                .get()
                .expect("no semInfo"),
        );
        let block = static_block_of_class_in_function_body(func);
        let info = FunctionInfoId::from_sema_id(
            block.function_info.get().expect("no functionInfo on the block"),
        );
        assert!(sem_ctx.function(info).is_static_block);
        assert_ne!(info, outer);
        let scope = sema::ids::ScopeId::from_sema_id(
            block.scope.get().expect("no scope on the block"),
        );
        assert_eq!(sem_ctx.function(info).get_function_body_scope(), scope);
        // The `var x` decl is in the block's scope, so it is NOT in any of
        // the enclosing function's.
        let x_decl = *sem_ctx
            .scope(scope)
            .decls
            .first()
            .expect("nothing hoisted into the static block");
        assert_eq!(sem_ctx.scope(scope).decls.len(), 1);
        assert_eq!(sem_ctx.decl(x_decl).kind, DeclKind::Var);
        for s in sem_ctx.function(outer).get_scopes() {
            assert!(
                !sem_ctx.scope(*s).decls.contains(&x_decl),
                "a static block's `var` must not hoist to the function"
            );
        }
    });
}

/// A fold inside a static block rebuilds the `StaticBlock` node, so BOTH of
/// the `Cell`s `visit(StaticBlockNode *)` writes before recursing (`scope`
/// from `ScopeRAII` and `function_info` from
/// `createStaticBlockFunctionInfo`) must be present on the node the resolver
/// RETURNED. `function_info` is what IRGen looks the block's body up by, and
/// the differential cannot see it go missing (the dump reaches the
/// `StaticBlock` `FunctionInfo` through the function tree either way).
#[test]
fn a_rebuilt_static_block_keeps_its_scope_and_function_info() {
    let src = "function f() { class C { static { var x = 1 + 2; } } }\n";
    with_resolved(src, |sem_ctx, resolved| {
        let func = first_statement(resolved);
        let block = static_block_of_class_in_function_body(func);
        assert!(
            block.scope.get().is_some(),
            "the rebuilt static block lost `scope`"
        );
        let info = FunctionInfoId::from_sema_id(
            block
                .function_info
                .get()
                .expect("the rebuilt static block lost `function_info`"),
        );
        assert!(sem_ctx.function(info).is_static_block);
        // Non-degeneracy: the fold must really have happened, i.e. the block
        // really was rebuilt.
        let decl = block
            .body
            .iter()
            .next()
            .expect("empty static block")
            .as_variable_declaration()
            .expect("not a VariableDeclaration")
            .declarations
            .iter()
            .next()
            .expect("no declarators")
            .as_variable_declarator()
            .expect("not a VariableDeclarator");
        assert!(
            matches!(decl.init, Some(Node::NumericLiteral(_))),
            "1 + 2 did not fold, so the static block was never rebuilt"
        );
    });
}

/// Walk `function f() { class C { static { ... } } }` down to the
/// `StaticBlock`.
fn static_block_of_class_in_function_body<'gc>(
    func: &'gc Node<'gc>,
) -> &'gc ast::node::StaticBlock<'gc> {
    let class = func
        .as_function_declaration()
        .expect("not a FunctionDeclaration")
        .body
        .as_block_statement()
        .expect("function body is not a block")
        .body
        .iter()
        .next()
        .expect("empty function body");
    class
        .as_class_declaration()
        .expect("not a ClassDeclaration")
        .body
        .as_class_body()
        .expect("not a ClassBody")
        .body
        .iter()
        .next()
        .expect("empty class body")
        .as_static_block()
        .expect("not a StaticBlock")
}

// ---- S3 T1: ScopedFunctionPromoter ------------------------------------

/// A block-nested function declaration at the top level of a loose-mode
/// PROGRAM is promoted (`getPromotedScopedFuncDecls` +
/// `processPromotedFuncDecls`, SemanticResolver.cpp:224-227, 2129-2141).
///
/// The resulting decl kinds ARE dump-visible (`promotion-basic.js` pins
/// them); what is not is the `SemContext::promotedFunctionDecls_` side entry
/// the second, block-scoped declaration lands in — the promoted identifier
/// prints only its declaration/expression decl.
#[test]
fn a_block_nested_function_is_promoted_to_global_scope() {
    with_resolved("{\n  function f() {}\n}\n", |sem_ctx, resolved| {
        let block = first_statement(resolved)
            .as_block_statement()
            .expect("not a BlockStatement");
        let id_node = block
            .body
            .iter()
            .next()
            .expect("empty block")
            .as_function_declaration()
            .expect("not a FunctionDeclaration")
            .id
            .expect("no function id");
        let id = id_node.as_identifier().expect("id is not an Identifier");

        // `processPromotedFuncDecls` used GlobalProperty because the
        // promoting function context is the global scope (cpp:2131-2133),
        // and that decl — not the block's — is what the name resolves to.
        let declared =
            sem_ctx.get_declaration_decl(id).expect("no declaration decl");
        assert_eq!(sem_ctx.decl(declared).kind, DeclKind::GlobalProperty);
        assert_eq!(
            sem_ctx.decl(declared).scope,
            Some(sem_ctx.get_global_scope())
        );
        assert_eq!(sem_ctx.get_expression_decl(id), Some(declared));

        // Visiting the block then created the SECOND declaration, the
        // block-scoped `ScopedFunction` one, and recorded it in the
        // `promotedFunctionDecls_` side table keyed by the identifier node
        // (`validateAndDeclareIdentifier`, cpp:2609-2625).
        let promoted = sem_ctx
            .get_promoted_decl(id_node.node_id())
            .expect("no promoted decl recorded");
        assert_ne!(promoted, declared);
        assert_eq!(sem_ctx.decl(promoted).kind, DeclKind::ScopedFunction);
        let block_scope = sema::ids::ScopeId::from_sema_id(
            block.scope.get().expect("no scope on the block"),
        );
        assert_eq!(sem_ctx.decl(promoted).scope, Some(block_scope));
    });
}

/// The same, one function down: `visitFunctionBodyAfterParamsVisited`'s call
/// site (cpp:1904-1910) promotes into the function body scope with
/// `Decl::Kind::Var`, not `GlobalProperty`.
#[test]
fn a_block_nested_function_inside_a_function_is_promoted_as_var() {
    let src = "function outer() {\n  {\n    function g() {}\n  }\n}\n";
    with_resolved(src, |sem_ctx, resolved| {
        let outer = first_statement(resolved)
            .as_function_declaration()
            .expect("not a FunctionDeclaration");
        let block = outer
            .body
            .as_block_statement()
            .expect("function body is not a block")
            .body
            .iter()
            .next()
            .expect("empty function body")
            .as_block_statement()
            .expect("not a BlockStatement");
        let id_node = block
            .body
            .iter()
            .next()
            .expect("empty block")
            .as_function_declaration()
            .expect("not a FunctionDeclaration")
            .id
            .expect("no function id");
        let id = id_node.as_identifier().expect("id is not an Identifier");

        let declared =
            sem_ctx.get_declaration_decl(id).expect("no declaration decl");
        assert_eq!(sem_ctx.decl(declared).kind, DeclKind::Var);
        let outer_info = FunctionInfoId::from_sema_id(
            outer.sem_info.get().expect("no sem_info on `outer`"),
        );
        assert_eq!(
            sem_ctx.decl(declared).scope,
            Some(sem_ctx.function(outer_info).get_function_body_scope())
        );

        let promoted = sem_ctx
            .get_promoted_decl(id_node.node_id())
            .expect("no promoted decl recorded");
        assert_eq!(sem_ctx.decl(promoted).kind, DeclKind::ScopedFunction);
    });
}

/// A visible let-like declaration with the same name blocks promotion
/// (ScopedFunctionPromoter.cpp:232-244): the function keeps ONLY its
/// block-scoped `ScopedFunction` decl and nothing is recorded in the
/// promoted-decl side table.
#[test]
fn a_visible_let_blocks_promotion() {
    with_resolved("let f;\n{\n  function f() {}\n}\n", |sem_ctx, resolved| {
        let Node::Program(p) = resolved else {
            unreachable!("not a Program")
        };
        let block = p
            .body
            .iter()
            .nth(1)
            .expect("program has no second statement")
            .as_block_statement()
            .expect("not a BlockStatement");
        let id_node = block
            .body
            .iter()
            .next()
            .expect("empty block")
            .as_function_declaration()
            .expect("not a FunctionDeclaration")
            .id
            .expect("no function id");
        let id = id_node.as_identifier().expect("id is not an Identifier");

        // The one and only decl for this name is the block-scoped one.
        let declared =
            sem_ctx.get_declaration_decl(id).expect("no declaration decl");
        assert_eq!(sem_ctx.decl(declared).kind, DeclKind::ScopedFunction);
        let block_scope = sema::ids::ScopeId::from_sema_id(
            block.scope.get().expect("no scope on the block"),
        );
        assert_eq!(sem_ctx.decl(declared).scope, Some(block_scope));
        assert_eq!(sem_ctx.get_expression_decl(id), Some(declared));
        // Nothing was promoted, so nothing was recorded.
        assert_eq!(sem_ctx.get_promoted_decl(id_node.node_id()), None);
        // Non-degeneracy: the `let` that blocked it really is in the global
        // scope, so the promoter had something to find.
        let let_decl = *sem_ctx
            .scope(sem_ctx.get_global_scope())
            .decls
            .first()
            .expect("global scope has no decls");
        assert_eq!(sem_ctx.decl(let_decl).kind, DeclKind::Let);
    });
}

// ---- S4a T3: the module visits (`resolver/modules.rs`) -----------------
//
// The differential pins the DUMP of every module shape in the corpus, but
// two things below it cannot see: `FunctionInfo::imports` (dump-blind
// everywhere — `SemContextDumper.cpp` never mentions it, and neither does
// this port's `dump_context.rs`), and rewrite #4's rebuilt
// `FunctionExpression`, which under `compile = true` always sits behind an
// `'export' statement requires module mode` error, and `hermesc` never dumps
// after a `resolveAST` failure (CompilerDriver.cpp:960-974). These tests are
// the only pin for both.

/// Resolve `root` through the ERROR-TOLERANT resolver entry
/// (`SemanticResolver::run_always`) at an explicit `compile` setting.
///
/// [`resolve_ast`] cannot serve these tests: every module declaration is an
/// error under `compile = true`, so it would hand back `None` — and the
/// rewritten tree with it. `resolve::resolve_ast_for_parser` hands the tree
/// back unconditionally but only at `compile = false`, where rewrite #4
/// deliberately does not fire. This is exactly that function's body with
/// `compile` lifted to a parameter.
fn resolve_always<'gc>(
    gc: &'gc GCLock,
    sem_ctx: &mut SemContext,
    sm: &mut SourceErrorManager,
    root: &'gc Node<'gc>,
    compile: bool,
) -> &'gc Node<'gc> {
    let binding_table = sem_ctx.binding_table_rc();
    let mut resolver = sema::resolver::SemanticResolver::new(
        &binding_table,
        sem_ctx,
        sm,
        /* ambient_decls */ &[],
        compile,
    );
    resolver.run_always(gc, root)
}

/// \return the `FunctionInfoId` of the global function, i.e. the one
/// `visit(ProgramNode *)` created.
fn global_function(sem_ctx: &SemContext) -> FunctionInfoId {
    sem_ctx.scope(sem_ctx.get_global_scope()).parent_function
}

/// **`FunctionInfo::imports` content** (cpp:887).
///
/// Every `ImportDeclaration` is pushed onto `curFunctionInfo()->imports`, in
/// source order, and each recorded `NodeRc` must name the very node that is
/// in the tree the resolver RETURNED — not a stale copy. The list is
/// dump-blind, so this is its only pin.
///
/// The `var x = 1 + 2;` tail is deliberate: the fold rebuilds the
/// `BinaryExpression` and therefore the whole `Program` spine, so the
/// assertions below run against a REBUILT root (asserted, so the test cannot
/// pass vacuously) — which is what proves the recorded `NodeRc`s survive an
/// ancestor rebuild.
#[test]
fn import_declarations_are_recorded_on_the_function_info() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let src = "import d, {a as b} from 'm';\nimport * as ns from 'n';\n\
               var x = 1 + 2;\n";
    let root = parse(&gc, &mut sm, src);
    let original_root_id = root.node_id();
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved =
        resolve_always(&gc, &mut sem_ctx, &mut sm, root, /* compile */ true);

    // One error per import, and nothing else.
    assert_eq!(sm.error_count(), 2, "one 'import' error per declaration");
    // Non-degeneracy: the trailing fold must really have rebuilt the spine,
    // otherwise "survives an ancestor rebuild" proves nothing.
    assert_ne!(
        resolved.node_id(),
        original_root_id,
        "the `1 + 2` fold must have rebuilt the Program"
    );

    let Node::Program(p) = resolved else {
        unreachable!("not a Program")
    };
    let in_tree: Vec<&Node> = p
        .body
        .iter()
        .filter(|n| matches!(n, Node::ImportDeclaration(_)))
        .collect();
    assert_eq!(in_tree.len(), 2, "two ImportDeclarations in the tree");

    let imports = &sem_ctx.function(global_function(&sem_ctx)).imports;
    assert_eq!(imports.len(), 2, "one entry per ImportDeclaration");
    // CONTENT, not just length: the same nodes, in the same order.
    assert_eq!(
        imports[0].node(&gc).node_id(),
        in_tree[0].node_id(),
        "imports[0] is not the first ImportDeclaration in the tree"
    );
    assert_eq!(
        imports[1].node(&gc).node_id(),
        in_tree[1].node_id(),
        "imports[1] is not the second ImportDeclaration in the tree"
    );

    // And the specifiers really did declare their locals as `Import` — the
    // `extractIdentsFromDecl` arm (cpp:2334-2347) this visit makes reachable.
    let kinds: Vec<DeclKind> = sem_ctx
        .scope(sem_ctx.get_global_scope())
        .decls
        .iter()
        .map(|&d| sem_ctx.decl(d).kind)
        .collect();
    assert_eq!(
        kinds.iter().filter(|&&k| k == DeclKind::Import).count(),
        3,
        "`d`, `b` and `ns` are Import decls"
    );
}

/// The `FunctionInfo::imports` backref fixup (spec §3.4 (a)) must NOT fire
/// spuriously: an `ImportDeclaration` whose children walk changes nothing
/// stays pointer-identical, and the recorded entry with it.
///
/// This is the shape the fixup's `Changed` branch is defensive against but
/// can never see today: an `ImportDeclaration`'s only children are
/// specifiers (whose own children are `Identifier` leaves), a `StringLiteral`
/// source and `ImportAttribute`s (literal key/value pairs) — nothing the
/// resolver rewrites or folds. The branch is kept anyway because the
/// obligation is structural, not shape-specific; if a later phase ever gives
/// an import a foldable child, this test is what will start exercising the
/// other side of it.
#[test]
fn import_backref_is_untouched_without_a_rebuild() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let root = parse(&gc, &mut sm, "import {a} from 'm';\n");
    let original_id = first_statement(root).node_id();
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved =
        resolve_always(&gc, &mut sem_ctx, &mut sm, root, /* compile */ true);

    assert_eq!(first_statement(resolved).node_id(), original_id);
    let imports = &sem_ctx.function(global_function(&sem_ctx)).imports;
    assert_eq!(imports.len(), 1);
    assert_eq!(imports[0].node(&gc).node_id(), original_id);
}

/// **Rewrite #4** (cpp:1525-1544): an ANONYMOUS `export default function`
/// becomes a `FunctionExpression` "for cleaner IRGen", carrying the
/// declaration's `_id`/`_params`/`_body`/`_typeParameters`/`_returnType`/
/// `_predicate`/`_generator`, its strictness and its location.
///
/// Also pins cpp:1538 forwarding `funcDecl->_async`: the rewritten node of
/// `export default async function () {}` stays async. This used to be a
/// literal `/* async */ false`, which silently dropped the flag; fixed
/// upstream in `6b59daf0d`.
#[test]
fn export_default_anonymous_function_is_rewritten_to_an_expression() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let src = "export default async function (a) { return a; }\n";
    let root = parse(&gc, &mut sm, src);
    let decl_before = root
        .as_program()
        .expect("not a Program")
        .body
        .iter()
        .next()
        .expect("empty program")
        .as_export_default_declaration()
        .expect("not an ExportDefaultDeclaration")
        .declaration;
    let decl_range_before = decl_before.range();
    assert!(
        decl_before
            .as_function_declaration()
            .expect("parsed as a FunctionDeclaration")
            .r#async
            .get(),
        "non-degeneracy: the source really is `async`"
    );

    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved =
        resolve_always(&gc, &mut sem_ctx, &mut sm, root, /* compile */ true);
    assert_eq!(sm.error_count(), 1, "the 'export' module-mode error");

    let export = first_statement(resolved)
        .as_export_default_declaration()
        .expect("the rewrite must keep an ExportDefaultDeclaration");
    let func = export
        .declaration
        .as_function_expression()
        .expect("rewrite #4 must replace the FunctionDeclaration");
    assert!(func.id.is_none(), "the anonymous `_id` is carried over");
    assert_eq!(func.params.iter().count(), 1, "`_params` carried over");
    assert!(
        matches!(func.body, Node::BlockStatement(_)),
        "`_body` carried over"
    );
    assert!(func.type_parameters.is_none());
    assert!(func.return_type.is_none());
    assert!(func.predicate.is_none());
    assert!(!func.generator.get(), "`_generator` carried over");
    // cpp:1538 forwards `funcDecl->_async` (`6b59daf0d`).
    assert!(
        func.r#async.get(),
        "rewrite #4 must carry `_async` over (cpp:1538) — an anonymous \
         `export default async function` stays async"
    );
    // copyLocationFrom(funcDecl) (cpp:1540).
    let range = func.metadata.range.get();
    assert_eq!(range.start, decl_range_before.start);
    assert_eq!(range.end, decl_range_before.end);
    // The function was still resolved as a function: `visitFunctionLike`
    // ran on the REBUILT node, so it carries a `sem_info` and a strictness.
    assert!(func.sem_info.get().is_some(), "the rewritten node was visited");
    assert_ne!(func.strictness.get(), Strictness::NotSet);
}

/// The other side of the `_async` forwarding above: a NON-async anonymous
/// `export default function () {}` must still come out non-async, i.e. the
/// fix in `6b59daf0d` forwards the flag rather than hard-coding the other
/// literal.
#[test]
fn export_default_anonymous_non_async_function_stays_non_async() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let src = "export default function (a) { return a; }\n";
    let root = parse(&gc, &mut sm, src);
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved =
        resolve_always(&gc, &mut sem_ctx, &mut sm, root, /* compile */ true);

    let func = first_statement(resolved)
        .as_export_default_declaration()
        .expect("the rewrite must keep an ExportDefaultDeclaration")
        .declaration
        .as_function_expression()
        .expect("rewrite #4 must replace the FunctionDeclaration");
    assert!(!func.r#async.get(), "`_async` was false and stays false");
}

/// Rewrite #4 fires ONLY for an anonymous function declaration: a NAMED
/// `export default function f() {}` keeps its `FunctionDeclaration`
/// (cpp:1526 `!funcDecl->_id`).
#[test]
fn export_default_named_function_is_not_rewritten() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let root = parse(&gc, &mut sm, "export default function f() {}\n");
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved =
        resolve_always(&gc, &mut sem_ctx, &mut sm, root, /* compile */ true);
    assert!(
        matches!(
            first_statement(resolved)
                .as_export_default_declaration()
                .expect("not an ExportDefaultDeclaration")
                .declaration,
            Node::FunctionDeclaration(_)
        ),
        "a NAMED default export keeps its FunctionDeclaration"
    );
}

/// The other half of cpp:1525's `dyn_cast<FunctionDeclarationNode>`: a
/// non-function default export is untouched by rewrite #4 (it is still
/// visited, and here still folded, but never turned into a
/// `FunctionExpression`).
#[test]
fn export_default_non_function_is_not_rewritten() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let root = parse(&gc, &mut sm, "export default 1 + 2;\n");
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved =
        resolve_always(&gc, &mut sem_ctx, &mut sm, root, /* compile */ true);
    assert!(
        matches!(
            first_statement(resolved)
                .as_export_default_declaration()
                .expect("not an ExportDefaultDeclaration")
                .declaration,
            // Folded by ASTEval, but still not a FunctionExpression.
            Node::NumericLiteral(_)
        ),
        "a non-function default export is untouched by rewrite #4"
    );
}

/// Under `compile = false` (`resolveASTForParser`) rewrite #4 does NOT fire
/// (cpp:1526 is `compile_ &&`) and NO `'export'` error is reported
/// (cpp:1520). Half of the bug-for-bug asymmetry; the import half is the
/// next test.
#[test]
fn compile_false_skips_the_export_error_and_the_rewrite() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let root = parse(&gc, &mut sm, "export default function () {}\n");
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved =
        resolve_always(&gc, &mut sem_ctx, &mut sm, root, /* compile */ false);
    assert_eq!(sm.error_count(), 0, "no 'export' error at compile = false");
    assert!(
        matches!(
            first_statement(resolved)
                .as_export_default_declaration()
                .expect("not an ExportDefaultDeclaration")
                .declaration,
            Node::FunctionDeclaration(_)
        ),
        "rewrite #4 is compile_-gated and must not have fired"
    );
}

/// The other half of the asymmetry: an `import` in the same position STILL
/// errors at `compile = false`, because cpp:876-879 is not `compile_`-gated
/// the way cpp:1511/1520/1550 are.
#[test]
fn compile_false_still_errors_on_imports() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let root = parse(&gc, &mut sm, "import {a} from 'm';\n");
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    resolve_always(&gc, &mut sem_ctx, &mut sm, root, /* compile */ false);
    assert_eq!(
        sm.error_count(),
        1,
        "the import error is NOT compile_-gated (cpp:876-879)"
    );
}

/// The import-assertions error is gated on `compile_ && !_attributes.empty()`
/// (cpp:881-885): an attribute-less import reports only the module-mode
/// error, one with attributes reports both — at the same location, in that
/// order.
#[test]
fn import_attributes_add_a_second_error_only_when_present() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    sm.set_handler(Box::new(SharedHandler(Rc::clone(&log))));
    let src = "import 'a.js';\nimport 'b.js' with {type: 'json'};\n";
    let root = parse(&gc, &mut sm, src);
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    resolve_always(&gc, &mut sem_ctx, &mut sm, root, /* compile */ true);

    assert_eq!(
        *log.borrow(),
        vec![
            "'import' statement requires module mode".to_string(),
            "'import' statement requires module mode".to_string(),
            "import assertions are not supported".to_string(),
        ]
    );
}

// ---- C++ defect-fix mirrors (upstream `07efab88d`, `b351e1184`) --------
//
// Both fixes have a corpus file (`shbuiltin-private-name.js`,
// `class-field-class-expr.js`), but the differential only proves the two
// sides AGREE. These two pin what each fix actually changed, by name.

/// Upstream `07efab88d` ("Fix crash on `$SHBuiltin.#privateName()`"): the
/// property of a non-computed member expression can be a `PrivateName`, which
/// the `cast<IdentifierNode>` at cpp:1166-1167 used to assert on (and which
/// this port reproduced as an explicit panic). A private property is never a
/// builtin access, so `$SHBuiltin` is left alone and reported exactly once as
/// an ordinary invalid use when the identifier itself is visited.
///
/// Mirrors `test/Sema/shbuiltin-private-name.js`; a `#[should_panic]` test
/// used to be impossible to write here only because the panic was the bug.
#[test]
fn shbuiltin_private_name_is_rejected_not_asserted() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    sm.set_handler(Box::new(SharedHandler(Rc::clone(&log))));
    let src = "class C {\n  #x;\n  m() {\n    $SHBuiltin.#x();\n  }\n}\n";
    let root = parse(&gc, &mut sm, src);
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    // `resolve_ast` (not `resolve_always`): the resolution FAILS, which is
    // the point — before the fix it panicked instead.
    assert!(
        resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[]).is_none(),
        "resolution must fail, not panic"
    );
    assert_eq!(
        *log.borrow(),
        vec!["invalid use of $SHBuiltin".to_string()],
        "exactly one ordinary invalid-use error"
    );
}

/// The three module property names still take their branches: the
/// `dyn_cast` gate `07efab88d` added must not have disabled the rewrite for
/// an ordinary identifier property. (`shbuiltin_module_factory_is_not_modeled`
/// above covers `moduleFactory`; this is the non-module rewrite itself,
/// which the corpus's `shbuiltin-calls.js` also pins through the dump.)
#[test]
fn shbuiltin_identifier_property_still_rewrites() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let root = parse(&gc, &mut sm, "$SHBuiltin.foo(1);\n");
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    let resolved = resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[])
        .expect("resolution must succeed");
    assert_eq!(sm.error_count(), 0, "no invalid-use error: it was rewritten");
    let callee = first_statement(resolved)
        .as_expression_statement()
        .expect("not an ExpressionStatement")
        .expression
        .as_call_expression()
        .expect("not a CallExpression")
        .callee;
    assert!(
        matches!(
            callee
                .as_member_expression()
                .expect("not a MemberExpression")
                .object,
            Node::SHBuiltin(_)
        ),
        "rewrite #3 must still fire for an identifier property"
    );
}

/// Upstream `b351e1184` ("Fix scope parenting of class expressions in field
/// initializers"): a scope created by a field initializer's VALUE belongs to
/// the synthesized elements-initializer function, so it must be parented in
/// that function's body scope — not in the enclosing class's scope, which
/// belongs to the outer function.
///
/// This is exactly the invariant the dumper's per-function scope walk relies
/// on (`dump_context.rs`'s `processed == scopes.len()` assert, port of
/// `SemContext.cpp:478`), and it is asserted directly here so a regression
/// names the broken link rather than tripping an assert three layers away.
#[test]
fn field_initializer_scopes_are_parented_in_the_initializer_function() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let src = "class C {\n  x = class {};\n  static y = class {};\n}\n";
    let root = parse(&gc, &mut sm, src);
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    assert!(resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[]).is_some());

    // Apart from the global function (whose second scope is the class
    // declaration's), the two synthesized initializer functions are the only
    // ones with more than one scope: their body scope plus the class
    // expression's.
    let global = global_function(&sem_ctx);
    let multi: Vec<FunctionInfoId> = (0..sem_ctx.functions_len())
        .map(|i| FunctionInfoId::from_sema_id(ast::SemaId(i as u32)))
        .filter(|&f| f != global && sem_ctx.function(f).get_scopes().len() > 1)
        .collect();
    assert_eq!(
        multi.len(),
        2,
        "the instance and static elements-init functions"
    );
    for f in multi {
        let info = sem_ctx.function(f);
        let body = info.get_function_body_scope();
        let scopes = info.get_scopes().to_vec();
        assert_eq!(scopes.len(), 2, "body scope + the class expression's");
        assert_eq!(scopes[0], body, "scopes[0] is the body scope");
        assert_eq!(
            sem_ctx.scope(scopes[1]).parent_scope,
            Some(body),
            "the class expression's scope must hang off the initializer \
             function's body scope, not the enclosing class scope"
        );
        // ... and the body scope itself still hangs off the class scope.
        assert_ne!(sem_ctx.scope(body).parent_scope, None);
    }
}
