/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Golden tests for `hermes_sema::dump::sem_dump` / `ASTPrinter`, ported from
//! `lib/Sema/SemResolve.cpp:20-161,258-297`. Hand-builds `SemContext` +
//! ESTree trees (no parser) and asserts the exact multi-line text the C++
//! `semDump` would produce for the equivalent structure. Every test goes
//! through `sem_dump` — the only public entry point (`ASTPrinter` itself
//! is a private implementation detail of `hermes_sema::dump`) — so expected
//! strings always include the leading `SemContext` block Task 5's dumper
//! produces.

use hermes_ast::context::{Context, GCLock};
use hermes_ast::node::{
    BinaryExpression, Identifier, Node, NumericLiteral, Program,
    TypeAnnotation,
};
use hermes_ast::node_child::{NodeList, NodeMetadata};
use hermes_sema::dump::sem_dump;
use hermes_sema::ids::{FunctionInfoId, ScopeId};
use hermes_sema::keywords::Keywords;
use hermes_sema::sem_context::{
    ConstructorKind, DeclKind, DeclSpecial, FuncIsArrow, SemContext,
};

fn r() -> hermes_support::location::SMRange {
    let l = hermes_support::location::SMLoc {
        source: hermes_support::location::SourceId::from_index(0),
        offset: 0,
    };
    hermes_support::location::SMRange { start: l, end: l }
}

/// A `SemContext` with a single (loose) global function + its one scope —
/// the minimum `print_sem_context` needs to not index out of bounds
/// (`root_func` defaults to function id 0 when the dumped root isn't
/// itself function-like).
fn new_global_sem_context(
    gc: &GCLock,
) -> (SemContext, FunctionInfoId, ScopeId) {
    let mut sc = SemContext::new(Keywords::new(gc));
    let f = sc.new_function(
        FuncIsArrow::No,
        ConstructorKind::None,
        None,
        None,
        /* strict */ false,
        Default::default(),
    );
    let s = sc.new_scope(f, None);
    (sc, f, s)
}

fn num<'gc>(gc: &'gc GCLock, v: f64) -> &'gc Node<'gc> {
    gc.alloc(Node::NumericLiteral(NumericLiteral::new(
        NodeMetadata::new(r()),
        v,
    )))
}

fn bin<'gc>(
    gc: &'gc GCLock,
    left: &'gc Node<'gc>,
    right: &'gc Node<'gc>,
    op: &str,
) -> &'gc Node<'gc> {
    gc.alloc(Node::BinaryExpression(BinaryExpression::new(
        NodeMetadata::new(r()),
        left,
        right,
        gc.atom_bytes(op),
    )))
}

fn ident<'gc>(gc: &'gc GCLock, name: &str) -> &'gc Node<'gc> {
    gc.alloc(Node::Identifier(Identifier::new(
        NodeMetadata::new(r()),
        gc.atom_bytes(name),
        /* type_annotation */ None,
        /* optional */ false,
    )))
}

/// The one mandated test covering the `BinaryExpression` `+`/`-`
/// linearization (SemResolve.cpp:70-95): `(1 + 2) - 3`.
///
/// Locks in the quirk documented in `hermes_sema::dump`'s module doc: the C++
/// prints `list[0]->_operator` (here, `+`, the *innermost* operator) on
/// **every** `BinOp` line, not each step's actual operator — so the second
/// line reads `BinOp +`, not `BinOp -`, even though the outer node's own
/// operator is `-`.
#[test]
fn linearized_binary_1_plus_2_minus_3() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let (sc, _f, _s) = new_global_sem_context(&gc);

    let one = num(&gc, 1.0);
    let two = num(&gc, 2.0);
    let three = num(&gc, 3.0);
    let inner = bin(&gc, one, two, "+"); // 1 + 2
    let outer = bin(&gc, inner, three, "-"); // (1 + 2) - 3

    let mut out = Vec::new();
    sem_dump(&mut out, &gc, &sc, outer);

    let expected = "\
SemContext
Func loose mayReachImplicitReturn
    Scope %s.1

BinaryExpression
    NumericLiteral
    BinOp +
    NumericLiteral
    BinOp +
    NumericLiteral

";
    assert_eq!(String::from_utf8(out).unwrap(), expected);
}

/// A non-`+`/`-` operator must NOT be linearized: no `BinOp` lines, and
/// both operands are reached through the normal `visit_children` recursion
/// (SemResolve.cpp:93-94 is only reached for `+`/`-`).
#[test]
fn non_linearized_binary_operator_recurses_normally() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let (sc, _f, _s) = new_global_sem_context(&gc);

    let one = num(&gc, 1.0);
    let two = num(&gc, 2.0);
    let root = bin(&gc, one, two, "*");

    let mut out = Vec::new();
    sem_dump(&mut out, &gc, &sc, root);

    let expected = "\
SemContext
Func loose mayReachImplicitReturn
    Scope %s.1

BinaryExpression
    NumericLiteral
    NumericLiteral

";
    assert_eq!(String::from_utf8(out).unwrap(), expected);
}

/// The one mandated `sem_dump` smoke test: an empty `Program` with its
/// `scope`/`sem_info` Cells set (as the resolver would leave them),
/// matching the tail of the empty-file golden: `Program Scope %s.1\n\n`.
#[test]
fn sem_dump_empty_program_tail() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let (sc, f, s) = new_global_sem_context(&gc);

    let prog = gc.alloc(Node::Program(Program::new(
        NodeMetadata::new(r()),
        NodeList::empty(),
    )));
    let prog_ref = prog.as_program().unwrap();
    prog_ref.scope.set(Some(s.sema_id()));
    prog_ref.sem_info.set(Some(f.sema_id()));

    let mut out = Vec::new();
    sem_dump(&mut out, &gc, &sc, prog);

    let expected = "\
SemContext
Func loose mayReachImplicitReturn
    Scope %s.1

Program Scope %s.1

";
    // The exact match above already includes the mandated tail
    // (`Program Scope %s.1\n\n`); spelled out here for readability.
    assert!(expected.ends_with("Program Scope %s.1\n\n"));
    assert_eq!(String::from_utf8(out).unwrap(), expected);
}

/// `Id 'x' [D:E:%d.N 'x']`: `declD == exprD` (SemResolve.cpp:109-111).
#[test]
fn identifier_decl_equals_expr_prints_d_e_colon() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let (mut sc, _f, s) = new_global_sem_context(&gc);

    let d = sc.new_decl_in_scope(
        gc.atom_bytes("x"),
        DeclKind::Let,
        s,
        DeclSpecial::NotSpecial,
    );

    let node = ident(&gc, "x");
    let id = node.as_identifier().unwrap();
    let node_id = node.node_id();
    sc.set_declaration_decl(node_id, id, Some(d));
    sc.set_expression_decl(node_id, id, Some(d));

    let mut out = Vec::new();
    sem_dump(&mut out, &gc, &sc, node);

    let expected = "\
SemContext
Func loose mayReachImplicitReturn
    Scope %s.1
        Decl %d.1 'x' Let

Id 'x' [D:E:%d.1 'x']

";
    assert_eq!(String::from_utf8(out).unwrap(), expected);
}

/// `Id 'z' [D:%d.N E:%d.M 'name']`: `declD != exprD`, both present
/// (SemResolve.cpp:112-116). `declD` prints without its name (`printName
/// = false`), `exprD` prints with it.
#[test]
fn identifier_decl_differs_from_expr_prints_d_and_e() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let (mut sc, _f, s) = new_global_sem_context(&gc);

    let expr_decl = sc.new_decl_in_scope(
        gc.atom_bytes("a"),
        DeclKind::Let,
        s,
        DeclSpecial::NotSpecial,
    );
    let decl_decl = sc.new_decl_in_scope(
        gc.atom_bytes("b"),
        DeclKind::Let,
        s,
        DeclSpecial::NotSpecial,
    );

    let node = ident(&gc, "z");
    let id = node.as_identifier().unwrap();
    let node_id = node.node_id();
    // Expression decl first (state -> HAVE_EXPR), then a *different*
    // declaration decl spills the expr value into the side table
    // (SemContext.cpp: setDeclarationDecl's `HAVE_EXPR` arm).
    sc.set_expression_decl(node_id, id, Some(expr_decl));
    sc.set_declaration_decl(node_id, id, Some(decl_decl));

    let mut out = Vec::new();
    sem_dump(&mut out, &gc, &sc, node);

    let expected = "\
SemContext
Func loose mayReachImplicitReturn
    Scope %s.1
        Decl %d.1 'a' Let
        Decl %d.2 'b' Let

Id 'z' [D:%d.2 E:%d.1 'a']

";
    assert_eq!(String::from_utf8(out).unwrap(), expected);
}

/// `Id 'q' [D:%d.N 'name']`: `declD` only, no `exprD`
/// (SemResolve.cpp:117-122, the "only remaining case").
#[test]
fn identifier_decl_only_prints_d_only() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let (mut sc, _f, s) = new_global_sem_context(&gc);

    let d = sc.new_decl_in_scope(
        gc.atom_bytes("onlyDecl"),
        DeclKind::Let,
        s,
        DeclSpecial::NotSpecial,
    );

    let node = ident(&gc, "q");
    let id = node.as_identifier().unwrap();
    let node_id = node.node_id();
    sc.set_declaration_decl(node_id, id, Some(d));

    let mut out = Vec::new();
    sem_dump(&mut out, &gc, &sc, node);

    let expected = "\
SemContext
Func loose mayReachImplicitReturn
    Scope %s.1
        Decl %d.1 'onlyDecl' Let

Id 'q' [D:%d.1 'onlyDecl']

";
    assert_eq!(String::from_utf8(out).unwrap(), expected);
}

/// An unresolvable identifier with no recorded decl at all: no `[...]`
/// bracket (neither `declD` nor `exprD` is present), just the ` UNR` suffix
/// (SemResolve.cpp:125-126). Also exercises the deviation documented in
/// `hermes_sema::dump`'s module doc: `get_expression_decl` is never called on an
/// unresolvable identifier, so this doesn't hit that function's `assert!`.
#[test]
fn identifier_unresolvable_prints_unr_suffix_only() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let (sc, _f, _s) = new_global_sem_context(&gc);

    let node = ident(&gc, "u");
    let id = node.as_identifier().unwrap();
    id.unresolvable.set(true);

    let mut out = Vec::new();
    sem_dump(&mut out, &gc, &sc, node);

    let expected = "\
SemContext
Func loose mayReachImplicitReturn
    Scope %s.1

Id 'u' UNR

";
    assert_eq!(String::from_utf8(out).unwrap(), expected);
}

/// `should_visit` skips `TypeAnnotation` nodes entirely (SemResolve.cpp:52),
/// including their subtree: an identifier with a `type_annotation` child
/// must print identically to one without — no `TypeAnnotation` or nested
/// node line ever appears.
#[test]
fn type_annotation_child_is_skipped_entirely() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let (sc, _f, _s) = new_global_sem_context(&gc);

    let inner = num(&gc, 42.0);
    let type_ann = gc.alloc(Node::TypeAnnotation(TypeAnnotation::new(
        NodeMetadata::new(r()),
        inner,
    )));
    let node = gc.alloc(Node::Identifier(Identifier::new(
        NodeMetadata::new(r()),
        gc.atom_bytes("t"),
        Some(type_ann),
        false,
    )));

    let mut out = Vec::new();
    sem_dump(&mut out, &gc, &sc, node);

    let expected = "\
SemContext
Func loose mayReachImplicitReturn
    Scope %s.1

Id 't'

";
    assert_eq!(String::from_utf8(out).unwrap(), expected);
}
