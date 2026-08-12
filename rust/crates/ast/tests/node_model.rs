/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Structural tests over the generated node set: range predicates, deep match,
//! visit_children counts, and GC survival across child + decoration lists.

use hermes_ast::context::{Context, GCLock, NodeRc};
use hermes_ast::node::*;
use hermes_ast::node_child::{NodeList, NodeMetadata};

fn r() -> hermes_support::location::SMRange {
    let l = hermes_support::location::SMLoc {
        source: hermes_support::location::SourceId::from_index(0),
        offset: 0,
    };
    hermes_support::location::SMRange { start: l, end: l }
}

fn num<'gc>(gc: &'gc GCLock, v: f64) -> &'gc Node<'gc> {
    gc.alloc(Node::NumericLiteral(NumericLiteral::new(
        NodeMetadata::new(r()),
        v,
    )))
}

/// Counts children visited by the generated `visit_children`.
struct Counter(usize);
impl<'gc> hermes_ast::visitor::Visitor<'gc> for Counter {
    fn visit_node(&mut self, node: &'gc Node<'gc>) {
        self.0 += 1;
        node.visit_children(self);
    }
}

#[test]
fn range_predicates_match_kinds() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);

    let prog = gc.alloc(Node::Program(Program::new(
        NodeMetadata::new(r()),
        NodeList::empty(),
    )));
    assert!(prog.is_function_like(), "Program is in the FunctionLike range");
    assert_eq!(prog.kind(), NodeKind::Program);

    let bin = gc.alloc(Node::BinaryExpression(BinaryExpression::new(
        NodeMetadata::new(r()),
        num(&gc, 1.0),
        num(&gc, 2.0),
        gc.atom_bytes("+".as_bytes()),
    )));
    assert!(!bin.is_function_like(), "BinaryExpression is not function-like");
    assert!(bin.as_identifier().is_none());
    assert!(num(&gc, 0.0).as_identifier().is_none());

    // ForStatement spans Statement→LoopStatement ranges and carries label_index + scope.
    let body = gc.alloc(Node::BlockStatement(BlockStatement::new(
        NodeMetadata::new(r()),
        NodeList::empty(),
        false, // implicit
    )));
    let fs = gc.alloc(Node::ForStatement(ForStatement::new(
        NodeMetadata::new(r()),
        None,
        None,
        None,
        body,
    )));
    assert!(fs.is_statement() && fs.is_loop_statement());
    if let Node::ForStatement(n) = fs {
        assert_eq!(n.label_index.get(), hermes_ast::node_child::INVALID_LABEL);
    }
}

#[test]
fn visit_children_counts() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    // BinaryExpression(1, 2): 1 (bin) + 2 leaves = 3 visited.
    let bin = gc.alloc(Node::BinaryExpression(BinaryExpression::new(
        NodeMetadata::new(r()),
        num(&gc, 1.0),
        num(&gc, 2.0),
        gc.atom_bytes("*".as_bytes()),
    )));
    let mut c = Counter(0);
    use hermes_ast::visitor::Visitor;
    c.visit_node(bin);
    assert_eq!(c.0, 3);
}

#[test]
fn gc_traces_decorations_on_function_declaration() {
    let mut ctx = Context::new();
    let keep: NodeRc;
    {
        let gc = GCLock::new(&mut ctx);
        let dec = num(&gc, 7.0); // reachable ONLY via FunctionDeclaration.decorations
        let list = NodeList::from_iter(&gc, [dec]);
        // Build a minimal FunctionDeclaration; set its `decorations` decoration list.
        let body = gc.alloc(Node::BlockStatement(BlockStatement::new(
            NodeMetadata::new(r()),
            NodeList::empty(),
            false, // implicit
        )));
        let fd = gc.alloc(Node::FunctionDeclaration(FunctionDeclaration::new(
            NodeMetadata::new(r()),
            None,              // id (NodePtr, optional)
            NodeList::empty(), // params
            body,              // body
            None,              // type_parameters
            None,              // return_type
            None,              // predicate
            false,             // generator
            false,             // r#async
        )));
        if let Node::FunctionDeclaration(n) = fd {
            n.decorations.set(list);
        }
        keep = NodeRc::from_node(&gc, fd);
    }
    ctx.gc();
    // Nothing is unreachable → a correct marker frees NOTHING. A nonzero free count
    // means `decorations` (attached via the FunctionLike base range) was not traced.
    assert_eq!(
        ctx.num_free_nodes(),
        0,
        "marker must trace `decorations` on FunctionLike nodes, not only Program"
    );
    {
        let gc2 = GCLock::new(&mut ctx);
        let fd = keep.node(&gc2);
        if let Node::FunctionDeclaration(n) = fd {
            let d = n
                .decorations
                .get()
                .iter()
                .next()
                .expect("decoration survived gc");
            assert!(matches!(d, Node::NumericLiteral(x) if x.value.get() == 7.0));
        } else {
            panic!()
        }
        drop(keep);
    }
}

#[test]
fn node_metadata_debug_loc_defaults_to_start() {
    use hermes_ast::node_child::NodeMetadata;
    use hermes_support::location::{SMLoc, SMRange, SourceId};

    let start = SMLoc { source: SourceId::from_index(0), offset: 10 };
    let end = SMLoc { source: SourceId::from_index(0), offset: 20 };
    let md = NodeMetadata::new(SMRange { start, end });
    assert_eq!(md.debug_loc.get(), start, "debug_loc must default to range start");

    let dbg = SMLoc { source: SourceId::from_index(0), offset: 15 };
    let md2 = NodeMetadata::new_with_debug(SMRange { start, end }, dbg);
    assert_eq!(md2.debug_loc.get(), dbg, "new_with_debug must set explicit debug_loc");

    let dup = md2.duplicate_pub_for_test();
    assert_eq!(dup.debug_loc.get(), dbg, "duplicate must carry debug_loc");
}
