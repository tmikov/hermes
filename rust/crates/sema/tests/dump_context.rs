/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Golden test for `hermes_sema::dump_context::SemContextDumper`, ported from
//! `lib/Sema/SemContext.cpp:415-573`. Hand-builds a `SemContext` (no
//! parser) and asserts the exact multi-line text the C++ `printSemContext`
//! would produce for the equivalent structure.

use hermes_ast::context::{Context, GCLock, NodeRc};
use hermes_ast::node::{BlockStatement, FunctionDeclaration, Identifier, Node};
use hermes_ast::node_child::{NodeList, NodeMetadata};
use hermes_sema::dump_context::SemContextDumper;
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

#[test]
fn prints_global_function_scope_decls_and_nested_strict_function() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let mut sc = SemContext::new(Keywords::new(&gc));

    // Global function (loose) + global scope + 2 decls.
    let global_fn = sc.new_function(
        FuncIsArrow::No,
        ConstructorKind::None,
        None,
        None,
        /* strict */ false,
        Default::default(),
    );
    let global_scope = sc.new_scope(global_fn, None);
    sc.new_decl_in_scope(
        gc.atom_bytes("x"),
        DeclKind::Let,
        global_scope,
        DeclSpecial::NotSpecial,
    );
    sc.new_decl_in_scope(
        gc.atom_bytes("f"),
        DeclKind::GlobalProperty,
        global_scope,
        DeclSpecial::NotSpecial,
    );

    // Nested function (strict), child of the global function/scope, with
    // one hoisted-function entry pointing at a hand-built
    // `FunctionDeclaration` whose id is the `Identifier` "g".
    let nested_fn = sc.new_function(
        FuncIsArrow::No,
        ConstructorKind::None,
        Some(global_fn),
        Some(global_scope),
        /* strict */ true,
        Default::default(),
    );
    let nested_scope = sc.new_scope(nested_fn, None);

    let g_id = gc.alloc(Node::Identifier(Identifier::new(
        NodeMetadata::new(r()),
        gc.atom_bytes("g"),
        None,
        false,
    )));
    let g_decl_node = gc.alloc(Node::new_function_declaration(&gc, 
        FunctionDeclaration::new(
            NodeMetadata::new(r()),
            Some(g_id),
            NodeList::empty(),
            gc.alloc(Node::BlockStatement(BlockStatement::new(
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
    sc.scope_mut(nested_scope)
        .hoisted_functions
        .push(NodeRc::from_node(&gc, g_decl_node));

    let mut dumper = SemContextDumper::new();
    let mut out = Vec::new();
    dumper.print_sem_context(&mut out, &gc, &sc, None);

    let expected = "\
SemContext
Func loose mayReachImplicitReturn
    Scope %s.1
        Decl %d.1 'x' Let
        Decl %d.2 'f' GlobalProperty
    Func strict mayReachImplicitReturn
        Scope %s.2
            hoistedFunction g
";
    // ASCII-only golden: comparing as `String` is fine here (and reads
    // better than a byte-slice literal); the byte-buffer output sink's
    // WTF-8 pass-through behavior is exercised directly by a dedicated
    // unit test in `hermes_sema::dump_context`'s own test module.
    assert_eq!(String::from_utf8(out).unwrap(), expected);
}
