/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Tests for `sema::decl_collector::DeclCollector`, ported from
//! `lib/Sema/DeclCollector.{h,cpp}`. Unlike the hand-built-tree tests
//! elsewhere in this crate, these parse real source with the `parser`
//! crate (a dev-dependency) — the parse-driver setup below is trimmed from
//! `rust/crates/parser/src/bin/ast_dump.rs`.
//!
//! Expected collections are transcribed from the C++ `DeclCollector.cpp`
//! *behavior*, not guessed:
//! - `VariableDeclaration` dispatches on `_kind == kw_.identVar` (cpp:113):
//!   `var` -> `addToFunc`, `let`/`const` -> `addToCur` (cpp:112-119).
//! - At a function's OWN top-level scope, `scopeStack_` has exactly one
//!   entry (`newScope()` is called exactly once by `runImpl`, cpp:64-97),
//!   so `addToFunc` (`scopeStack_.front()`) and `addToCur`
//!   (`scopeStack_.back()`) push onto the very same `Vec` — `var`/`let`/
//!   `const`/function declarations directly at a function's top level are
//!   therefore indistinguishable in the resulting `ScopeDecls`, interleaved
//!   in visit order. The var/let split only becomes observable once a
//!   nested scope (block/for/switch/catch) is on the stack.
//! - `FunctionDeclaration` is always recorded via `addToCur` (cpp:143), and
//!   additionally pushed to `scopedFuncDecls_` when `scopeStack_.size() > 1`
//!   (cpp:144-146) — i.e. only when it's nested inside a block/switch/etc.,
//!   not when it's directly at the function's top level.
//! - `closeScope` only stores a scope's `ScopeDecls` if non-empty (cpp:194).

use ast::context::{Context, GCLock};
use ast::node::{Node, VariableDeclaration};
use ast::NodeId;
use parser::js::JSParserImpl;
use parser::lexer::{GrammarContext, JSLexer};
use sema::decl_collector::DeclCollector;
use sema::keywords::Keywords;
use support::manager::SourceErrorManager;

/// Parse `src` as a `Program` and return its root node, panicking on any
/// parse error. Trimmed from `parser/src/bin/ast_dump.rs`'s driver setup.
fn parse<'gc>(gc: &'gc GCLock, src: &str) -> &'gc Node<'gc> {
    let mut sm = SourceErrorManager::new();
    let buf_id = sm.add_buffer_bytes("input", src.as_bytes());
    let result: Option<&Node> = {
        let atoms = &gc.ctx().atom_table;
        let lexer =
            JSLexer::new(buf_id, &mut sm, atoms, GrammarContext::AllowRegExp);
        let mut parser = JSParserImpl::new(gc, lexer);
        parser.parse()
    };
    assert_eq!(sm.error_count(), 0, "unexpected parse errors in: {src}");
    result.expect("parser returned no Program")
}

/// A `recursion_depth_exceeded` callback that must never fire in these
/// tests (every corpus here is tiny, well under any real nesting limit).
fn never_exceeded(_n: &Node) {
    panic!("recursion depth unexpectedly exceeded");
}

/// Extract `(kind_name, identifier_name)` for one of the declaration kinds
/// `DeclCollector` records, so tests can assert "kind + extracted name"
/// (per the task brief) instead of raw node identity.
fn decl_kind_and_name(gc: &GCLock, node: &Node) -> (&'static str, String) {
    match node {
        Node::VariableDeclaration(vd) => {
            ("VariableDeclaration", first_declarator_name(gc, vd))
        }
        Node::FunctionDeclaration(f) => {
            let id = f.id.expect("FunctionDeclaration always has an id here");
            let ident = id.as_identifier().expect("id is an Identifier");
            ("FunctionDeclaration", atom_string(gc, ident.name.get()))
        }
        Node::CatchClause(_) => ("CatchClause", String::new()),
        other => panic!("unexpected decl node kind: {}", other.node_type_str()),
    }
}

fn atom_string(gc: &GCLock, atom: atom_table::AtomBytes) -> String {
    String::from_utf8_lossy(gc.bytes(atom)).into_owned()
}

fn first_declarator_name(gc: &GCLock, vd: &VariableDeclaration) -> String {
    let first = vd.declarations.iter().next().expect("has a declarator");
    let declarator =
        first.as_variable_declarator().expect("VariableDeclarator");
    let ident = declarator.id.as_identifier().expect("id is an Identifier");
    atom_string(gc, ident.name.get())
}

/// A tiny local visitor that finds the first `FunctionDeclaration` named
/// `name` anywhere under the node it's run on. Reuses
/// `ast::node::Node::visit_children`'s generated per-kind dispatch instead
/// of duplicating a match-on-every-kind walk in each test.
struct FunctionFinder<'gc, 'g, 'ast, 'ctx> {
    name: &'g str,
    gc: &'g GCLock<'ast, 'ctx>,
    result: Option<&'gc Node<'gc>>,
}

impl<'gc, 'g, 'ast, 'ctx> ast::visitor::Visitor<'gc>
    for FunctionFinder<'gc, 'g, 'ast, 'ctx>
{
    fn visit_node(&mut self, node: &'gc Node<'gc>) {
        if self.result.is_some() {
            return;
        }
        if let Node::FunctionDeclaration(f) = node {
            if let Some(id) = f.id {
                if let Some(ident) = id.as_identifier() {
                    if self.gc.bytes(ident.name.get()) == self.name.as_bytes()
                    {
                        self.result = Some(node);
                        return;
                    }
                }
            }
        }
        node.visit_children(self);
    }
}

fn find_function_named<'gc>(
    node: &'gc Node<'gc>,
    name: &str,
    gc: &GCLock,
) -> &'gc Node<'gc> {
    let mut finder = FunctionFinder {
        name,
        gc,
        result: None,
    };
    node.visit_children(&mut finder);
    finder
        .result
        .unwrap_or_else(|| panic!("no function named {name}"))
}

fn root_scope_decls_kinds_and_names(
    gc: &GCLock,
    dc: &DeclCollector,
    root_id: NodeId,
) -> Vec<(&'static str, String)> {
    let decls = dc
        .scope_decls_for_node(root_id)
        .expect("root scope must have collected declarations");
    decls
        .iter()
        .map(|rc| decl_kind_and_name(gc, rc.node(gc)))
        .collect()
}

#[test]
fn top_level_program_hoists_var_let_and_function_into_one_root_scope() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let kw = Keywords::new(&gc);

    let src =
        "var a; let b; function f(){var inner;} { let c; function g(){} }";
    let program = parse(&gc, src);

    let mut on_exceeded = never_exceeded;
    let dc = DeclCollector::run(program, &gc, &kw, 1024, &mut on_exceeded);

    // Program's own scope: a (var), b (let), f (FunctionDeclaration) all
    // land in the SAME ScopeDecls, since scopeStack_ has exactly one level
    // at the Program's top — see the module doc.
    let root_decls =
        root_scope_decls_kinds_and_names(&gc, &dc, program.node_id());
    assert_eq!(
        root_decls,
        vec![
            ("VariableDeclaration", "a".to_string()),
            ("VariableDeclaration", "b".to_string()),
            ("FunctionDeclaration", "f".to_string()),
        ]
    );

    // `f`'s own body ("var inner;") was never visited by this collector
    // (FunctionDeclaration is a no-descend node) — nothing from `inner`.
    let f_node = find_function_named(program, "f", &gc);
    assert!(dc.scope_decls_for_node(f_node.node_id()).is_none());

    // The nested block statement `{ let c; function g(){} }` gets its own
    // scope, keyed to the BlockStatement node.
    let block = find_block(program).expect("the trailing block statement");
    let block_decls: Vec<_> = dc
        .scope_decls_for_node(block.node_id())
        .expect("block scope must have collected declarations")
        .iter()
        .map(|rc| decl_kind_and_name(&gc, rc.node(&gc)))
        .collect();
    assert_eq!(
        block_decls,
        vec![
            ("VariableDeclaration", "c".to_string()),
            ("FunctionDeclaration", "g".to_string()),
        ]
    );

    // `g` is nested inside a block (scopeStack_.len() > 1 when visited), so
    // it must ALSO be recorded in scopedFuncDecls_ (Annex B 3.3). `f` is at
    // the top level (scopeStack_.len() == 1 there), so it must NOT be.
    let scoped: Vec<_> = dc
        .scoped_func_decls()
        .iter()
        .map(|rc| decl_kind_and_name(&gc, rc.node(&gc)))
        .collect();
    assert_eq!(scoped, vec![("FunctionDeclaration", "g".to_string())]);
}

/// Finds the sole top-level `BlockStatement` in a `Program`'s body (a plain
/// linear scan — there's exactly one in the fixture above).
fn find_block<'gc>(program: &'gc Node<'gc>) -> Option<&'gc Node<'gc>> {
    let prog = program.as_program()?;
    prog.body
        .iter()
        .find(|n| matches!(n, Node::BlockStatement(_)))
}

#[test]
fn var_inside_nested_block_hoists_to_function_scope_not_block_scope() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let kw = Keywords::new(&gc);

    let src = "\
function f() {
  var a;
  let b;
  function g(){}
  {
    let c;
    var d;
    function h(){}
  }
}";
    let program = parse(&gc, src);
    let f_node = find_function_named(program, "f", &gc);

    let mut on_exceeded = never_exceeded;
    let dc = DeclCollector::run(f_node, &gc, &kw, 1024, &mut on_exceeded);

    // f's own top-level scope: a (var, direct), b (let, direct), g
    // (FunctionDeclaration, direct), then d (var, HOISTED from the nested
    // block via addToFunc) — appended in visit order.
    let f_decls =
        root_scope_decls_kinds_and_names(&gc, &dc, f_node.node_id());
    assert_eq!(
        f_decls,
        vec![
            ("VariableDeclaration", "a".to_string()),
            ("VariableDeclaration", "b".to_string()),
            ("FunctionDeclaration", "g".to_string()),
            ("VariableDeclaration", "d".to_string()),
        ]
    );

    // The nested block's OWN scope only has c (let) and h (function decl);
    // `d` (var) does NOT appear here — it was hoisted to f's scope instead.
    let block = find_block_in_function(f_node).expect("the nested block");
    let block_decls: Vec<_> = dc
        .scope_decls_for_node(block.node_id())
        .expect("block scope must have collected declarations")
        .iter()
        .map(|rc| decl_kind_and_name(&gc, rc.node(&gc)))
        .collect();
    assert_eq!(
        block_decls,
        vec![
            ("VariableDeclaration", "c".to_string()),
            ("FunctionDeclaration", "h".to_string()),
        ]
    );

    // Only `h` is nested inside a block (scopeStack_.len() > 1); `g` sits
    // directly at f's top level (scopeStack_.len() == 1), so only `h`
    // shows up in scopedFuncDecls_.
    let scoped: Vec<_> = dc
        .scoped_func_decls()
        .iter()
        .map(|rc| decl_kind_and_name(&gc, rc.node(&gc)))
        .collect();
    assert_eq!(scoped, vec![("FunctionDeclaration", "h".to_string())]);
}

fn find_block_in_function<'gc>(func: &'gc Node<'gc>) -> Option<&'gc Node<'gc>> {
    let f = func.as_function_declaration()?;
    let body = f.body.as_block_statement()?;
    body.body
        .iter()
        .find(|n| matches!(n, Node::BlockStatement(_)))
}

#[test]
fn switch_cases_share_one_scope_and_nested_function_is_scoped() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let kw = Keywords::new(&gc);

    let src = "\
function f() {
  switch (x) {
    case 1:
      let y;
      break;
    default:
      function k(){}
  }
}";
    let program = parse(&gc, src);
    let f_node = find_function_named(program, "f", &gc);

    let mut on_exceeded = never_exceeded;
    let dc = DeclCollector::run(f_node, &gc, &kw, 1024, &mut on_exceeded);

    // f's own top-level scope collected nothing directly (the switch is the
    // only statement, and it creates its own scope) -- so no entry at all
    // for f (closeScope only stores non-empty scopes).
    assert!(dc.scope_decls_for_node(f_node.node_id()).is_none());

    // Every `case`/`default` in one switch shares a SINGLE scope, keyed to
    // the SwitchStatement itself (SwitchCase has no scope-creating
    // override, so both cases' declarations land in the same ScopeDecls).
    let switch = find_switch(f_node).expect("the switch statement");
    let switch_decls: Vec<_> = dc
        .scope_decls_for_node(switch.node_id())
        .expect("switch scope must have collected declarations")
        .iter()
        .map(|rc| decl_kind_and_name(&gc, rc.node(&gc)))
        .collect();
    assert_eq!(
        switch_decls,
        vec![
            ("VariableDeclaration", "y".to_string()),
            ("FunctionDeclaration", "k".to_string()),
        ]
    );

    // `k` is nested inside the switch's scope (scopeStack_.len() > 1), so
    // it must also appear in scopedFuncDecls_.
    let scoped: Vec<_> = dc
        .scoped_func_decls()
        .iter()
        .map(|rc| decl_kind_and_name(&gc, rc.node(&gc)))
        .collect();
    assert_eq!(scoped, vec![("FunctionDeclaration", "k".to_string())]);
}

fn find_switch<'gc>(func: &'gc Node<'gc>) -> Option<&'gc Node<'gc>> {
    let f = func.as_function_declaration()?;
    let body = f.body.as_block_statement()?;
    body.body
        .iter()
        .find(|n| matches!(n, Node::SwitchStatement(_)))
}

#[test]
fn catch_param_and_body_get_separate_scopes() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let kw = Keywords::new(&gc);

    let src = "\
function f() {
  try {
  } catch (e) {
    let z;
  }
}";
    let program = parse(&gc, src);
    let f_node = find_function_named(program, "f", &gc);

    let mut on_exceeded = never_exceeded;
    let dc = DeclCollector::run(f_node, &gc, &kw, 1024, &mut on_exceeded);

    // f's own top-level scope collected nothing directly -- the try/catch
    // machinery creates its own nested scopes -- so no entry for f.
    assert!(dc.scope_decls_for_node(f_node.node_id()).is_none());

    let catch = find_catch(f_node).expect("the catch clause");

    // Per cpp:169-179: CatchClauseNode::visit records the CatchClauseNode
    // ITSELF (not the param) via addToCur when there is a param -- a
    // faithfully-transcribed quirk, not a name-based decl. Assert on the
    // node kind directly rather than through `decl_kind_and_name` (which
    // doesn't try to extract a name for CatchClause).
    let catch_decls = dc
        .scope_decls_for_node(catch.node_id())
        .expect("catch-clause scope must have collected its own node");
    assert_eq!(catch_decls.len(), 1);
    assert!(matches!(
        catch_decls[0].node(&gc),
        Node::CatchClause(_)
    ));
    assert_eq!(catch_decls[0].node(&gc).node_id(), catch.node_id());

    // The catch BODY (a BlockStatement) gets its OWN separate scope, with
    // `z`'s declaration -- NOT merged into the catch-clause's own scope.
    let catch_body = catch
        .as_catch_clause()
        .expect("CatchClause")
        .body;
    let body_decls: Vec<_> = dc
        .scope_decls_for_node(catch_body.node_id())
        .expect("catch body scope must have collected declarations")
        .iter()
        .map(|rc| decl_kind_and_name(&gc, rc.node(&gc)))
        .collect();
    assert_eq!(body_decls, vec![("VariableDeclaration", "z".to_string())]);
}

fn find_catch<'gc>(func: &'gc Node<'gc>) -> Option<&'gc Node<'gc>> {
    let f = func.as_function_declaration()?;
    let body = f.body.as_block_statement()?;
    let try_stmt = body.body.iter().find_map(|n| match n {
        Node::TryStatement(t) => Some(t),
        _ => None,
    })?;
    try_stmt.handler
}

#[test]
fn dump_does_not_panic_and_mentions_recorded_declarations() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let kw = Keywords::new(&gc);

    let program = parse(&gc, "var a;");
    let mut on_exceeded = never_exceeded;
    let dc = DeclCollector::run(program, &gc, &kw, 1024, &mut on_exceeded);

    let mut out = Vec::new();
    dc.dump(&mut out, &gc, 4);
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("VariableDeclaration"));
}

#[test]
fn recursion_depth_exceeded_callback_fires_and_stops_that_subtree() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let kw = Keywords::new(&gc);

    // A deeply left-nested block chain, well past a tiny recursion budget.
    let mut src = String::new();
    for _ in 0..20 {
        src.push_str("{ ");
    }
    src.push_str("var deep;");
    for _ in 0..20 {
        src.push_str(" }");
    }
    let program = parse(&gc, &src);

    let mut fired = 0u32;
    let mut on_exceeded = |_n: &Node| {
        fired += 1;
    };
    // Small enough budget that the nested blocks trip it.
    let dc = DeclCollector::run(program, &gc, &kw, 5, &mut on_exceeded);

    assert_eq!(fired, 1, "callback must fire exactly once");
    // The var 20 levels deep must NOT have been collected anywhere: the
    // walk was cut off before reaching it. `scoped_func_decls` is vacuously
    // empty here (the fixture has no functions at all), so assert directly
    // that no scope recorded any declarations for the enclosing `Program`.
    assert!(dc.scope_decls_for_node(program.node_id()).is_none());
}
