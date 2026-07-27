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
use ast::node::Node;
use ast::node_child::Strictness;
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
/// resulting `SemContext` and the root node's strictness.
fn resolve(src: &str) -> (SemContext, Strictness) {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let mut sm = SourceErrorManager::new();
    let root = parse(&gc, &mut sm, src);
    let mut sem_ctx = SemContext::new(Keywords::new(&gc));
    assert!(
        resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &[]),
        "resolution failed for: {src}"
    );
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
    assert!(resolve_ast(&gc, &mut sem_ctx, &mut sm, root, &ambient));

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
        assert!(resolver.run(&gc, root));
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
