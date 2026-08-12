/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Tests for `sema::sem_context`, ported switch-arm-for-switch-arm from
//! `lib/Sema/SemContext.cpp`. See task-4-brief.md sections A-D.

use hermes_ast::context::{Context, GCLock};
use hermes_ast::node::{
    ClassBody, ClassDeclaration, Identifier, MethodDefinition, Node,
};
use hermes_ast::node_child::{NodeList, NodeMetadata};
use sema::ids::{DeclId, FunctionInfoId, ScopeId};
use sema::keywords::Keywords;
use sema::sem_context::{
    ConstructorKind, Constness, DeclKind, DeclSpecial, FuncIsArrow, SemContext,
};

fn r() -> hermes_support::location::SMRange {
    let l = hermes_support::location::SMLoc {
        source: hermes_support::location::SourceId::from_index(0),
        offset: 0,
    };
    hermes_support::location::SMRange { start: l, end: l }
}

fn new_sem_context(gc: &GCLock) -> SemContext {
    SemContext::new(Keywords::new(gc))
}

fn ident<'gc>(gc: &'gc GCLock, name: &str) -> &'gc Node<'gc> {
    gc.alloc(Node::Identifier(Identifier::new(
        NodeMetadata::new(r()),
        gc.atom_bytes(name),
        None,
        false,
    )))
}

// ===================== A. DeclKind predicate table =========================

/// (kind, is_tdz, is_var_like, is_var_like_or_scoped_function, is_let_like,
/// is_global, is_private_name, constness)
type DeclKindRow = (DeclKind, bool, bool, bool, bool, bool, bool, Constness);

#[test]
fn decl_kind_predicate_table_matches_cpp_semcontext_h_130_189() {
    use DeclKind::*;
    let table: &[DeclKindRow] = &[
        (Let, true, false, false, true, false, false, Constness::Never),
        (Const, true, false, false, true, false, false, Constness::Always),
        (Class, true, false, false, true, false, false, Constness::Never),
        (Catch, false, false, false, true, false, false, Constness::Never),
        (Import, false, false, false, true, false, false, Constness::Always),
        (ScopedFunction, false, false, true, true, false, false, Constness::Never),
        (ES5Catch, false, false, false, true, false, false, Constness::Never),
        (
            FunctionExprName,
            false,
            false,
            false,
            false,
            false,
            false,
            Constness::StrictModeOnly,
        ),
        (ClassExprName, false, false, false, false, false, false, Constness::Always),
        (TypedBuiltin, false, false, false, false, false, false, Constness::Never),
        (PrivateField, false, false, false, false, false, true, Constness::Never),
        (PrivateMethod, false, false, false, false, false, true, Constness::Never),
        (PrivateGetter, false, false, false, false, false, true, Constness::Never),
        (PrivateSetter, false, false, false, false, false, true, Constness::Never),
        (
            PrivateGetterSetter,
            false,
            false,
            false,
            false,
            false,
            true,
            Constness::Never,
        ),
        (Var, false, true, true, false, false, false, Constness::Never),
        (Parameter, false, true, true, false, false, false, Constness::Never),
        (GlobalProperty, false, true, true, false, true, false, Constness::Never),
        (
            UndeclaredGlobalProperty,
            false,
            true,
            true,
            false,
            true,
            false,
            Constness::Never,
        ),
    ];

    // 19 variants: Let..UndeclaredGlobalProperty (SemContext.h:58-105).
    // NOTE: the task brief said 18; the C++ header actually lists 19 — see
    // task-4-report.md "C++-vs-brief discrepancies".
    assert_eq!(table.len(), 19);

    // Exact declaration order, transcribed literally from SemContext.h:58-105
    // (Import comes before Catch there, not after — easy to transpose).
    // Verified via discriminant comparison since the predicates below are
    // ordinal-comparison-based and would silently tolerate some transpositions.
    let expected_order = [
        Let,
        Const,
        Class,
        Import,
        Catch,
        ScopedFunction,
        ES5Catch,
        FunctionExprName,
        ClassExprName,
        TypedBuiltin,
        PrivateField,
        PrivateMethod,
        PrivateGetter,
        PrivateSetter,
        PrivateGetterSetter,
        Var,
        Parameter,
        GlobalProperty,
        UndeclaredGlobalProperty,
    ];
    for w in expected_order.windows(2) {
        assert!(w[0] < w[1], "{:?} must sort before {:?}", w[0], w[1]);
    }
    assert_eq!(expected_order.len(), table.len());

    for &(kind, tdz, var_like, var_like_or_sf, let_like, global, priv_name, constness) in
        table
    {
        assert_eq!(kind.is_tdz(), tdz, "is_tdz({kind:?})");
        assert_eq!(kind.is_var_like(), var_like, "is_var_like({kind:?})");
        assert_eq!(
            kind.is_var_like_or_scoped_function(),
            var_like_or_sf,
            "is_var_like_or_scoped_function({kind:?})"
        );
        assert_eq!(kind.is_let_like(), let_like, "is_let_like({kind:?})");
        assert_eq!(kind.is_global(), global, "is_global({kind:?})");
        assert_eq!(kind.is_private_name(), priv_name, "is_private_name({kind:?})");
        assert_eq!(kind.constness(), constness, "constness({kind:?})");
    }
}

// ===================== B. new_function/new_scope/new_decl_in_scope =========

#[test]
fn ids_are_dense_indices_and_global_accessors_return_id_0() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);

    let f0 = sc.new_function(
        FuncIsArrow::No,
        ConstructorKind::None,
        None,
        None,
        false,
        Default::default(),
    );
    assert_eq!(f0, FunctionInfoId::from_sema_id(hermes_ast::SemaId(0)));

    let s0 = sc.new_scope(f0, None);
    assert_eq!(s0, ScopeId::from_sema_id(hermes_ast::SemaId(0)));

    assert_eq!(sc.get_global_function(), f0);
    assert_eq!(sc.get_global_scope(), s0);
    sc.assert_global_function_and_scope();

    let f1 = sc.new_function(
        FuncIsArrow::No,
        ConstructorKind::None,
        Some(f0),
        Some(s0),
        false,
        Default::default(),
    );
    assert_eq!(f1, FunctionInfoId::from_sema_id(hermes_ast::SemaId(1)));

    let s1 = sc.new_scope(f1, None);
    assert_eq!(s1, ScopeId::from_sema_id(hermes_ast::SemaId(1)));
    // Second scope in f1.
    let s2 = sc.new_scope(f1, Some(s1));
    assert_eq!(sc.scope(s2).depth, sc.scope(s1).depth + 1);

    // function.get_scopes() records the scope with idx_in_parent_function set.
    assert_eq!(sc.function(f1).get_scopes(), &[s1, s2]);
    assert_eq!(sc.scope(s1).idx_in_parent_function, 0);
    assert_eq!(sc.scope(s2).idx_in_parent_function, 1);
    assert_eq!(sc.scope(s1).parent_function, f1);

    let name = gc.atom_bytes("x");
    let d0 = sc.new_decl_in_scope(name, DeclKind::Let, s1, DeclSpecial::NotSpecial);
    assert_eq!(d0, DeclId::from_sema_id(hermes_ast::SemaId(0)));
    let d1 = sc.new_decl_in_scope_default(name, DeclKind::Const, s1);
    assert_eq!(d1, DeclId::from_sema_id(hermes_ast::SemaId(1)));

    // scope.decls records the decl.
    assert_eq!(sc.scope(s1).decls, vec![d0, d1]);
    assert_eq!(sc.decl(d0).kind, DeclKind::Let);
    assert_eq!(sc.decl(d1).kind, DeclKind::Const);
    assert_eq!(sc.decl(d0).scope, Some(s1));
    assert_eq!(sc.decl(d0).name, name);
    assert!(!sc.decl(d0).generic);
    assert_eq!(sc.decl(d0).special, DeclSpecial::NotSpecial);
}

#[test]
fn new_global_asserts_kind_is_global_and_uses_global_scope() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let f0 = sc.new_function(
        FuncIsArrow::No,
        ConstructorKind::None,
        None,
        None,
        false,
        Default::default(),
    );
    let global_scope = sc.new_scope(f0, None);

    let d = sc.new_global(gc.atom_bytes("g"), DeclKind::GlobalProperty);
    assert_eq!(sc.decl(d).scope, Some(global_scope));
    assert_eq!(sc.decl(d).kind, DeclKind::GlobalProperty);
}

#[test]
fn node_is_arrow_no_for_none_and_non_arrow_and_yes_for_arrow() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    assert_eq!(SemContext::node_is_arrow(None), FuncIsArrow::No);
    let id = ident(&gc, "x");
    assert_eq!(SemContext::node_is_arrow(Some(id)), FuncIsArrow::No);

    let body = gc.alloc(Node::BlockStatement(hermes_ast::node::BlockStatement::new(
        NodeMetadata::new(r()),
        NodeList::empty(),
        false,
    )));
    let arrow = gc.alloc(Node::ArrowFunctionExpression(
        hermes_ast::node::ArrowFunctionExpression::new(
            NodeMetadata::new(r()),
            NodeList::empty(),
            body,
            None,
            None,
            None,
            false,
            false,
        ),
    ));
    assert_eq!(SemContext::node_is_arrow(Some(arrow)), FuncIsArrow::Yes);
}

// ===================== C. func_arguments_decl ===============================

#[test]
fn func_arguments_decl_non_arrow_creates_var_arguments_and_caches() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let global_fn = sc.new_function(
        FuncIsArrow::No,
        ConstructorKind::None,
        None,
        None,
        false,
        Default::default(),
    );
    sc.new_scope(global_fn, None);

    let f = sc.new_function(
        FuncIsArrow::No,
        ConstructorKind::None,
        Some(global_fn),
        None,
        false,
        Default::default(),
    );
    let fscope = sc.new_scope(f, None);

    let args_name = gc.atom_bytes("arguments");
    let d1 = sc.func_arguments_decl(f, args_name);
    assert_eq!(sc.decl(d1).kind, DeclKind::Var);
    assert_eq!(sc.decl(d1).special, DeclSpecial::Arguments);
    assert_eq!(sc.decl(d1).scope, Some(fscope));
    assert_eq!(sc.function(f).arguments_decl, Some(d1));

    // Caches: a second call returns the same decl, doesn't create another.
    let d2 = sc.func_arguments_decl(f, args_name);
    assert_eq!(d1, d2);
}

#[test]
fn func_arguments_decl_arrow_chain_resolves_to_non_arrow_ancestor() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let global_fn = sc.new_function(
        FuncIsArrow::No,
        ConstructorKind::None,
        None,
        None,
        false,
        Default::default(),
    );
    sc.new_scope(global_fn, None);

    let outer = sc.new_function(
        FuncIsArrow::No,
        ConstructorKind::None,
        Some(global_fn),
        None,
        false,
        Default::default(),
    );
    let outer_scope = sc.new_scope(outer, None);

    let arrow1 = sc.new_function(
        FuncIsArrow::Yes,
        ConstructorKind::None,
        Some(outer),
        Some(outer_scope),
        false,
        Default::default(),
    );
    sc.new_scope(arrow1, Some(outer_scope));

    let arrow2 = sc.new_function(
        FuncIsArrow::Yes,
        ConstructorKind::None,
        Some(arrow1),
        Some(outer_scope),
        false,
        Default::default(),
    );
    sc.new_scope(arrow2, Some(outer_scope));

    let args_name = gc.atom_bytes("arguments");
    // Requesting from the innermost arrow must resolve to `outer`'s
    // "arguments" (also verifies `nearest_non_arrow`-style ancestor walk).
    let d = sc.func_arguments_decl(arrow2, args_name);
    assert_eq!(sc.decl(d).scope, Some(outer_scope));
    assert_eq!(sc.function(outer).arguments_decl, Some(d));
    assert_eq!(sc.function(arrow1).arguments_decl, None);
    assert_eq!(sc.function(arrow2).arguments_decl, None);

    // nearest_non_arrow itself, directly.
    assert_eq!(sc.nearest_non_arrow(arrow2), outer);
    assert_eq!(sc.nearest_non_arrow(arrow1), outer);
    assert_eq!(sc.nearest_non_arrow(outer), outer);
}

#[test]
fn func_arguments_decl_on_global_function_creates_undeclared_global_property() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let global_fn = sc.new_function(
        FuncIsArrow::No,
        ConstructorKind::None,
        None,
        None,
        false,
        Default::default(),
    );
    let global_scope = sc.new_scope(global_fn, None);

    let args_name = gc.atom_bytes("arguments");
    let d = sc.func_arguments_decl(global_fn, args_name);
    assert_eq!(sc.decl(d).kind, DeclKind::UndeclaredGlobalProperty);
    assert_eq!(sc.decl(d).special, DeclSpecial::NotSpecial);
    assert_eq!(sc.decl(d).scope, Some(global_scope));
}

// ===================== D. Decl-state machine ================================

fn mkdecl(n: u32) -> DeclId {
    DeclId::from_sema_id(hermes_ast::SemaId(n))
}

#[test]
fn get_expression_decl_panics_on_unresolvable_identifier() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    id.unresolvable.set(true);

    let sc = new_sem_context(&gc);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        sc.get_expression_decl(id)
    }));
    assert!(result.is_err(), "expected panic reading decl of unresolvable ident");
}

#[test]
fn set_declaration_decl_from_empty_sets_have_decl_bit() {
    // default arm, state == 0 (SemContext.cpp:255-261)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_declaration_decl(node_id, id, Some(mkdecl(7)));
    assert_eq!(sc.get_declaration_decl(id), Some(mkdecl(7)));
    assert_eq!(sc.get_expression_decl(id), None);
    assert_eq!(sc.side_table_len_for_test(), 0);
}

#[test]
fn set_declaration_decl_overwrite_have_decl_only() {
    // default arm, state == BitHaveDecl (SemContext.cpp:255-261)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_declaration_decl(node_id, id, Some(mkdecl(1)));
    sc.set_declaration_decl(node_id, id, Some(mkdecl(2)));
    assert_eq!(sc.get_declaration_decl(id), Some(mkdecl(2)));
    assert_eq!(sc.side_table_len_for_test(), 0);
}

#[test]
fn set_declaration_decl_matching_existing_expr_merges_bits_no_side_table() {
    // case BitHaveExpr, decl == node->decl_ (SemContext.cpp:226-229)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_expression_decl(node_id, id, Some(mkdecl(5)));
    sc.set_declaration_decl(node_id, id, Some(mkdecl(5)));

    assert_eq!(sc.get_expression_decl(id), Some(mkdecl(5)));
    assert_eq!(sc.get_declaration_decl(id), Some(mkdecl(5)));
    assert_eq!(sc.side_table_len_for_test(), 0);
}

#[test]
fn set_declaration_decl_different_from_expr_spills_to_side_table() {
    // case BitHaveExpr, decl != node->decl_ (SemContext.cpp:229-232)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_expression_decl(node_id, id, Some(mkdecl(5)));
    sc.set_declaration_decl(node_id, id, Some(mkdecl(9)));

    assert_eq!(sc.get_expression_decl(id), Some(mkdecl(5)));
    assert_eq!(sc.get_declaration_decl(id), Some(mkdecl(9)));
    assert_eq!(sc.side_table_len_for_test(), 1);
}

#[test]
fn set_declaration_decl_update_side_decl_back_to_equal_erases_side_entry() {
    // case BitHaveExpr|BitSideDecl, new decl == node->decl_ (SemContext.cpp:241-247)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_expression_decl(node_id, id, Some(mkdecl(5)));
    sc.set_declaration_decl(node_id, id, Some(mkdecl(9))); // -> side table
    assert_eq!(sc.side_table_len_for_test(), 1);

    sc.set_declaration_decl(node_id, id, Some(mkdecl(5))); // back to == expr
    assert_eq!(sc.get_declaration_decl(id), Some(mkdecl(5)));
    assert_eq!(sc.get_expression_decl(id), Some(mkdecl(5)));
    assert_eq!(sc.side_table_len_for_test(), 0);
}

#[test]
fn set_declaration_decl_update_side_decl_to_another_different_value() {
    // case BitHaveExpr|BitSideDecl, new decl != node->decl_ (SemContext.cpp:248-250)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_expression_decl(node_id, id, Some(mkdecl(5)));
    sc.set_declaration_decl(node_id, id, Some(mkdecl(9)));
    sc.set_declaration_decl(node_id, id, Some(mkdecl(11)));

    assert_eq!(sc.get_expression_decl(id), Some(mkdecl(5)));
    assert_eq!(sc.get_declaration_decl(id), Some(mkdecl(11)));
    assert_eq!(sc.side_table_len_for_test(), 1);
}

#[test]
fn set_declaration_decl_unset_shared_value_keeps_expr() {
    // case BitHaveDecl|BitHaveExpr, decl==None (SemContext.cpp:268-270)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_both_decl(node_id, id, Some(mkdecl(3)));
    sc.set_declaration_decl(node_id, id, None);

    assert_eq!(sc.get_declaration_decl(id), None);
    assert_eq!(sc.get_expression_decl(id), Some(mkdecl(3)));
    assert_eq!(sc.side_table_len_for_test(), 0);
}

#[test]
fn set_declaration_decl_unset_decl_only_clears_value() {
    // case BitHaveDecl, decl==None (SemContext.cpp:273-276)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_declaration_decl(node_id, id, Some(mkdecl(3)));
    sc.set_declaration_decl(node_id, id, None);

    assert_eq!(sc.get_declaration_decl(id), None);
    assert_eq!(id.decl_state.get(), 0);
}

#[test]
fn set_declaration_decl_unset_side_decl_erases_side_entry() {
    // case BitSideDecl|BitHaveExpr, decl==None (SemContext.cpp:280-286)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_expression_decl(node_id, id, Some(mkdecl(5)));
    sc.set_declaration_decl(node_id, id, Some(mkdecl(9)));
    assert_eq!(sc.side_table_len_for_test(), 1);

    sc.set_declaration_decl(node_id, id, None);
    assert_eq!(sc.get_declaration_decl(id), None);
    assert_eq!(sc.get_expression_decl(id), Some(mkdecl(5)));
    assert_eq!(sc.side_table_len_for_test(), 0);
}

#[test]
fn set_declaration_decl_unset_noop_when_no_decl_present() {
    // default arm, decl==None (SemContext.cpp:288-293)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_expression_decl(node_id, id, Some(mkdecl(5)));
    sc.set_declaration_decl(node_id, id, None); // no-op: no decl bit set

    assert_eq!(sc.get_expression_decl(id), Some(mkdecl(5)));
    assert_eq!(sc.get_declaration_decl(id), None);
    assert_eq!(sc.side_table_len_for_test(), 0);
}

#[test]
fn set_expression_decl_fresh_sets_have_expr_bit() {
    // default arm, state == 0 (SemContext.cpp:373-377)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_expression_decl(node_id, id, Some(mkdecl(4)));
    assert_eq!(sc.get_expression_decl(id), Some(mkdecl(4)));
    assert_eq!(sc.get_declaration_decl(id), None);
    assert_eq!(sc.side_table_len_for_test(), 0);
}

#[test]
fn set_expression_decl_overwrite_have_expr_only() {
    // default arm, state == BitHaveExpr (SemContext.cpp:373-377)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_expression_decl(node_id, id, Some(mkdecl(4)));
    sc.set_expression_decl(node_id, id, Some(mkdecl(6)));
    assert_eq!(sc.get_expression_decl(id), Some(mkdecl(6)));
    assert_eq!(sc.side_table_len_for_test(), 0);
}

#[test]
fn set_expression_decl_matching_have_decl_only_merges_bits() {
    // case BitHaveDecl, decl == node->decl_ (SemContext.cpp:345-348)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_declaration_decl(node_id, id, Some(mkdecl(2)));
    sc.set_expression_decl(node_id, id, Some(mkdecl(2)));

    assert_eq!(sc.get_declaration_decl(id), Some(mkdecl(2)));
    assert_eq!(sc.get_expression_decl(id), Some(mkdecl(2)));
    assert_eq!(sc.side_table_len_for_test(), 0);
}

#[test]
fn set_expression_decl_different_from_have_decl_only_spills_old_decl() {
    // case BitHaveDecl, decl != node->decl_ (SemContext.cpp:348-353)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_declaration_decl(node_id, id, Some(mkdecl(2)));
    sc.set_expression_decl(node_id, id, Some(mkdecl(8)));

    assert_eq!(sc.get_declaration_decl(id), Some(mkdecl(2)), "old decl decl spilled");
    assert_eq!(sc.get_expression_decl(id), Some(mkdecl(8)));
    assert_eq!(sc.side_table_len_for_test(), 1);
}

#[test]
fn set_expression_decl_different_from_shared_have_decl_and_expr_spills() {
    // case BitHaveDecl|BitHaveExpr, decl != node->decl_ (SemContext.cpp:348-353)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_both_decl(node_id, id, Some(mkdecl(2))); // BitHaveDecl|BitHaveExpr, shared 2
    sc.set_expression_decl(node_id, id, Some(mkdecl(8)));

    assert_eq!(sc.get_declaration_decl(id), Some(mkdecl(2)));
    assert_eq!(sc.get_expression_decl(id), Some(mkdecl(8)));
    assert_eq!(sc.side_table_len_for_test(), 1);
}

#[test]
fn set_expression_decl_side_state_matching_side_value_erases_side_entry() {
    // case BitSideDecl|BitHaveExpr, decl == side value (SemContext.cpp:360-370)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_declaration_decl(node_id, id, Some(mkdecl(2)));
    sc.set_expression_decl(node_id, id, Some(mkdecl(8))); // side[node]=2
    assert_eq!(sc.side_table_len_for_test(), 1);

    sc.set_expression_decl(node_id, id, Some(mkdecl(2))); // matches side value 2
    assert_eq!(sc.get_declaration_decl(id), Some(mkdecl(2)));
    assert_eq!(sc.get_expression_decl(id), Some(mkdecl(2)));
    assert_eq!(sc.side_table_len_for_test(), 0);
}

#[test]
fn set_expression_decl_side_state_not_matching_side_value_keeps_side_entry() {
    // case BitSideDecl|BitHaveExpr, decl != side value (SemContext.cpp:360-370)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_declaration_decl(node_id, id, Some(mkdecl(2)));
    sc.set_expression_decl(node_id, id, Some(mkdecl(8))); // side[node]=2
    sc.set_expression_decl(node_id, id, Some(mkdecl(20))); // still != 2

    assert_eq!(sc.get_declaration_decl(id), Some(mkdecl(2)), "side entry untouched");
    assert_eq!(sc.get_expression_decl(id), Some(mkdecl(20)));
    assert_eq!(sc.side_table_len_for_test(), 1);
}

#[test]
fn set_expression_decl_unset_have_expr_only_clears() {
    // case BitHaveExpr, decl==None (SemContext.cpp:383-386)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_expression_decl(node_id, id, Some(mkdecl(4)));
    sc.set_expression_decl(node_id, id, None);

    assert_eq!(id.decl_state.get(), 0);
    assert_eq!(sc.get_declaration_decl(id), None);
}

#[test]
fn set_expression_decl_unset_shared_value_keeps_decl() {
    // case BitHaveExpr|BitHaveDecl, decl==None (SemContext.cpp:390-392)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_both_decl(node_id, id, Some(mkdecl(3)));
    sc.set_expression_decl(node_id, id, None);

    assert_eq!(sc.get_declaration_decl(id), Some(mkdecl(3)));
    // get_expression_decl requires !unresolvable; state no longer has
    // BitHaveExpr so it must read None.
    assert_eq!(sc.get_expression_decl(id), None);
}

#[test]
fn set_expression_decl_unset_side_state_moves_side_value_out_but_leaves_side_table_stale()
{
    // case BitHaveExpr|BitSideDecl, decl==None (SemContext.cpp:397-404).
    // NOTE: faithfully ported C++ behavior does NOT erase the side-table
    // entry here (see the doc comment on this arm in sem_context.rs) —
    // this test locks in that (surprising) fact.
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_declaration_decl(node_id, id, Some(mkdecl(2)));
    sc.set_expression_decl(node_id, id, Some(mkdecl(8))); // side[node]=2
    assert_eq!(sc.side_table_len_for_test(), 1);

    sc.set_expression_decl(node_id, id, None);
    // decl_ now holds the side value (2), state==BitHaveDecl.
    assert_eq!(sc.get_declaration_decl(id), Some(mkdecl(2)));
    assert_eq!(id.decl_state.get(), 2 /* BitHaveDecl */);
    // The side-table entry is stale but the port keeps it (verbatim C++).
    assert_eq!(sc.side_table_len_for_test(), 1);
}

#[test]
fn set_expression_decl_unset_noop_when_no_expr_present() {
    // default arm, decl==None (SemContext.cpp:406-410)
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_declaration_decl(node_id, id, Some(mkdecl(2)));
    sc.set_expression_decl(node_id, id, None); // no-op: no expr bit set

    assert_eq!(sc.get_declaration_decl(id), Some(mkdecl(2)));
    assert_eq!(sc.side_table_len_for_test(), 0);
}

#[test]
fn set_both_decl_sets_expr_then_decl_to_same_value() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n = ident(&gc, "x");
    let id = n.as_identifier().unwrap();
    let node_id = n.node_id();

    sc.set_both_decl(node_id, id, Some(mkdecl(42)));
    assert_eq!(sc.get_declaration_decl(id), Some(mkdecl(42)));
    assert_eq!(sc.get_expression_decl(id), Some(mkdecl(42)));
    assert_eq!(sc.side_table_len_for_test(), 0);
}

// ---- Promoted decls ---------------------------------------------------

#[test]
fn promoted_decls_set_get_clear() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    let n1 = ident(&gc, "f");
    let n2 = ident(&gc, "g");

    assert_eq!(sc.get_promoted_decl(n1.node_id()), None);
    sc.set_promoted_decl(n1.node_id(), mkdecl(1));
    sc.set_promoted_decl(n2.node_id(), mkdecl(2));
    assert_eq!(sc.get_promoted_decl(n1.node_id()), Some(mkdecl(1)));
    assert_eq!(sc.get_promoted_decl(n2.node_id()), Some(mkdecl(2)));

    sc.clear_promoted_decls();
    assert_eq!(sc.get_promoted_decl(n1.node_id()), None);
    assert_eq!(sc.get_promoted_decl(n2.node_id()), None);
}

// ---- get_constructor ---------------------------------------------------

#[test]
fn get_constructor_finds_the_constructor_method() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let sc = new_sem_context(&gc);

    let ctor_key = ident(&gc, "constructor");
    let ctor_value = gc.alloc(Node::FunctionExpression(
        hermes_ast::node::FunctionExpression::new(
            NodeMetadata::new(r()),
            None,
            NodeList::empty(),
            gc.alloc(Node::BlockStatement(hermes_ast::node::BlockStatement::new(
                NodeMetadata::new(r()),
                NodeList::empty(),
                false,
            ))),
            None,
            None,
            None,
            false,
            false,
        ),
    ));
    let ctor_method = gc.alloc(Node::MethodDefinition(MethodDefinition::new(
        NodeMetadata::new(r()),
        ctor_key,
        ctor_value,
        gc.atom_bytes("constructor"),
        false,
        false,
        NodeList::empty(),
    )));

    let other_key = ident(&gc, "foo");
    let other_value = gc.alloc(Node::FunctionExpression(
        hermes_ast::node::FunctionExpression::new(
            NodeMetadata::new(r()),
            None,
            NodeList::empty(),
            gc.alloc(Node::BlockStatement(hermes_ast::node::BlockStatement::new(
                NodeMetadata::new(r()),
                NodeList::empty(),
                false,
            ))),
            None,
            None,
            None,
            false,
            false,
        ),
    ));
    let other_method = gc.alloc(Node::MethodDefinition(MethodDefinition::new(
        NodeMetadata::new(r()),
        other_key,
        other_value,
        gc.atom_bytes("method"),
        false,
        false,
        NodeList::empty(),
    )));

    let body_list = NodeList::from_iter(&gc, [other_method, ctor_method]);
    let class_body = gc.alloc(Node::ClassBody(ClassBody::new(
        NodeMetadata::new(r()),
        body_list,
    )));
    let class_decl = gc.alloc(Node::ClassDeclaration(ClassDeclaration::new(
        NodeMetadata::new(r()),
        None,
        None,
        None,
        None,
        NodeList::empty(),
        NodeList::empty(),
        class_body,
    )));

    let found = sc.get_constructor(class_decl).expect("constructor found");
    assert!(std::ptr::eq(found, ctor_method));
}

#[test]
fn get_constructor_returns_none_when_absent() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let sc = new_sem_context(&gc);

    let other_key = ident(&gc, "foo");
    let other_value = gc.alloc(Node::FunctionExpression(
        hermes_ast::node::FunctionExpression::new(
            NodeMetadata::new(r()),
            None,
            NodeList::empty(),
            gc.alloc(Node::BlockStatement(hermes_ast::node::BlockStatement::new(
                NodeMetadata::new(r()),
                NodeList::empty(),
                false,
            ))),
            None,
            None,
            None,
            false,
            false,
        ),
    ));
    let other_method = gc.alloc(Node::MethodDefinition(MethodDefinition::new(
        NodeMetadata::new(r()),
        other_key,
        other_value,
        gc.atom_bytes("method"),
        false,
        false,
        NodeList::empty(),
    )));
    let class_body = gc.alloc(Node::ClassBody(ClassBody::new(
        NodeMetadata::new(r()),
        NodeList::from_iter(&gc, [other_method]),
    )));
    let class_expr = gc.alloc(Node::ClassExpression(
        hermes_ast::node::ClassExpression::new(
            NodeMetadata::new(r()),
            None,
            None,
            None,
            None,
            NodeList::empty(),
            NodeList::empty(),
            class_body,
        ),
    ));

    assert!(sc.get_constructor(class_expr).is_none());
}

// ---- misc storage accessors ---------------------------------------------

#[test]
fn builtin_declarations_and_binding_table_accessors() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = new_sem_context(&gc);
    assert!(sc.builtin_declarations().is_empty());

    let fd = gc.alloc(Node::FunctionDeclaration(
        hermes_ast::node::FunctionDeclaration::new(
            NodeMetadata::new(r()),
            None,
            NodeList::empty(),
            gc.alloc(Node::BlockStatement(hermes_ast::node::BlockStatement::new(
                NodeMetadata::new(r()),
                NodeList::empty(),
                false,
            ))),
            None,
            None,
            None,
            false,
            false,
        ),
    ));
    sc.add_builtin_declaration(hermes_ast::context::NodeRc::from_node(&gc, fd));
    assert_eq!(sc.builtin_declarations().len(), 1);

    // Binding table: exists and is initially empty (no active scope yet).
    assert_eq!(sc.binding_table().count(&gc.atom_bytes("x")), 0);

    assert!(sc.get_binding_table_global_scope().is_null());
    let ptr = {
        let scope = hermes_support::persistent_scoped_map::Scope::new(sc.binding_table());
        scope.ptr()
        // `scope` (and its borrow of `sc`) is dropped here; `ptr` keeps the
        // (now-popped) scope alive via its own `Rc`.
    };
    sc.set_binding_table_global_scope(ptr);
    assert!(!sc.get_binding_table_global_scope().is_null());
}

/// `private_name_identifier` — the port of
/// `Context::getPrivateNameIdentifier` (AST/Context.h:389-393), whose whole
/// body is `getIdentifier(llvh::Twine("#") + str->str())`. It must prepend
/// exactly one `#`, intern into the SAME table ordinary identifiers use (so
/// the result compares equal to a hand-interned `"#x"`), and be idempotent in
/// the sense that equal inputs give equal atoms.
#[test]
fn private_name_identifier_prefixes_a_hash() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);

    let x = gc.atom_bytes("x");
    let mangled = sema::sem_context::private_name_identifier(&gc, x);
    assert_eq!(gc.bytes(mangled), b"#x");
    // Same atom table as ordinary identifiers, so this is atom equality.
    assert_eq!(mangled, gc.atom_bytes("#x"));
    // And distinct from the unmangled name, which is the whole point.
    assert_ne!(mangled, x);

    // Stable across calls.
    assert_eq!(mangled, sema::sem_context::private_name_identifier(&gc, x));

    // Only ONE `#` is added: mangling an already-mangled name double-prefixes
    // rather than being a no-op (nothing does that, but it pins that the
    // function is a plain prefix, not a normalizer).
    let twice = sema::sem_context::private_name_identifier(&gc, mangled);
    assert_eq!(gc.bytes(twice), b"##x");

    // The empty name is not special-cased.
    let empty = gc.atom_bytes("");
    assert_eq!(
        gc.bytes(sema::sem_context::private_name_identifier(&gc, empty)),
        b"#"
    );
}
