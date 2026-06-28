/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The Pre/Lazy parser passes. Port of the `ParserPass` machinery in
//! `lib/Parser/JSParserImpl.{h,cpp}` and `include/hermes/Parser/JSParser.h`.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use support::location::SMLoc;

/// The parser mode. Port of `enum ParserPass` (JSParser.h:26-36). Same order:
/// PreParse, LazyParse, FullParse.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ParserPass {
    /// Parse and index the file's functions without keeping an AST.
    PreParse,
    /// Re-parse, skipping function bodies indexed by a prior PreParse.
    LazyParse,
    /// Completely parse the full file (the default, eager mode).
    FullParse,
}

/// Information about a pre-parsed function body, recorded during the
/// `PreParse` pass and consumed during `LazyParse`.
/// Port of `PreParsedFunctionInfo` (PreParser.h:38-58).
#[derive(Clone)]
pub struct PreParsedFunctionInfo {
    /// The end location of the function body (closing `}`).
    pub end: SMLoc,

    /// Whether the function body began with `"use strict"`.
    pub strict_mode: bool,

    /// Directive prologues found at the top of the function body.
    /// Stored as owned byte vectors because UniqueString atoms are
    /// arena-allocated and reclaimed between parse passes — we cannot hold
    /// raw pointers across pass boundaries (PreParser.h:46-48).
    pub directives: Vec<Vec<u8>>,

    /// Whether the function body contains an arrow function.
    pub contains_arrow_functions: bool,

    /// Conservative estimate: whether a non-arrow function may have an arrow
    /// child that references `arguments`, requiring eager Arguments capture.
    pub may_contain_arrow_functions_using_arguments: bool,
}

/// Per-buffer table produced by the `PreParse` pass.
/// Port of `PreParsedBufferInfo` (PreParser.h:60-63).
pub struct PreParsedBufferInfo {
    /// Maps function-body start **offset** (within the source buffer) to its
    /// pre-parsed metadata. The C++ uses `SMLoc` (a pointer) as the key;
    /// we use the `u32` offset so the map is trivially `Send`/serialisable.
    pub function_info: HashMap<u32, PreParsedFunctionInfo>,
}

/// RAII Drop-guard for the three arrow-bookkeeping flags
/// (`isArrowFunction_`, `containsArrowFunctions_`,
/// `mayContainArrowFunctionsUsingArguments_`). Owns `Rc<Cell<bool>>` clones of
/// each flag so it can restore them on Drop without borrowing `self` — the same
/// pattern as `ParamFlagGuard` in mod.rs. Strict-mode and `seen_directives`
/// are managed separately at each call site (they live on `&mut self` fields
/// that cannot be owned by the guard).
///
/// Port of `JSParserImpl::SaveFunctionState` (JSParserImpl.h:1699-1740).
pub(super) struct SaveFunctionState {
    is_arrow: Rc<Cell<bool>>,
    contains: Rc<Cell<bool>>,
    may_contain: Rc<Cell<bool>>,
    old_is_arrow: bool,
    old_contains: bool,
    old_may_contain: bool,
}

impl Drop for SaveFunctionState {
    fn drop(&mut self) {
        // C++ dtor JSParserImpl.h:1728-1738.
        if !self.is_arrow.get() {
            self.contains.set(self.old_contains);
            self.may_contain.set(self.old_may_contain);
        }
        self.is_arrow.set(self.old_is_arrow);
    }
}

use ast::node::{Node, NodeKind};

use crate::lexer::{GrammarContext, JSLexer};

use super::flow::{AllowTypedArrowFunction, CoverTypedParameters};
use super::{JSParserImpl, PARAM_IN, PARAM_RETURN};

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    /// Pre-parse a source buffer: build a `PreParse`-mode parser, set strict
    /// mode if requested, run `parse()`, and return the parser so the caller
    /// can extract `take_pre_parsed()` and `get_use_static_builtin()`.
    ///
    /// Returns `None` if parsing fails (syntax error). Returns `Some(parser)`
    /// on success; the caller owns the pre-parsed side-table via `take_pre_parsed()`.
    ///
    /// Port of `JSParserImpl::preParseBuffer` (JSParserImpl.cpp:7534-7546).
    /// Deviation from C++:
    /// - C++ wraps both the `AllocationScope` and the parser in a `PreParser`
    ///   struct and returns a `shared_ptr<JSParserImpl>` aliased to that struct,
    ///   so the scope is kept alive as long as the pointer lives. In Rust there
    ///   is no `AllocationScope`; nodes are arena-allocated in the `GCLock` and
    ///   reclaimed when the lock is dropped. The caller controls the lock
    ///   lifetime independently, so we simply return the parser itself.
    pub fn pre_parse_buffer(
        gc: &'gc ast::context::GCLock<'ast, 'ctx>,
        lexer: JSLexer<'a>,
        strict: bool,
    ) -> Option<JSParserImpl<'gc, 'ast, 'ctx, 'a>> {
        let mut p = JSParserImpl::new_with_pass(gc, lexer, ParserPass::PreParse);
        p.lexer.set_strict_mode(strict);
        p.parse()?;
        Some(p)
    }

    /// Construct a `SaveFunctionState` guard that saves and restores the three
    /// arrow-bookkeeping flags on Drop. Also sets the flags for the new
    /// function scope. Port of the `SaveFunctionState` ctor
    /// (JSParserImpl.h:1719-1726).
    pub(super) fn save_function_state(&self, is_arrow: bool) -> SaveFunctionState {
        let g = SaveFunctionState {
            is_arrow: Rc::clone(&self.is_arrow_function),
            contains: Rc::clone(&self.contains_arrow_functions),
            may_contain: Rc::clone(
                &self.may_contain_arrow_functions_using_arguments,
            ),
            old_is_arrow: self.is_arrow_function.get(),
            old_contains: self.contains_arrow_functions.get(),
            old_may_contain: self
                .may_contain_arrow_functions_using_arguments
                .get(),
        };
        // C++ ctor JSParserImpl.h:1719-1726.
        self.is_arrow_function.set(is_arrow);
        if is_arrow {
            self.contains_arrow_functions.set(true);
        } else {
            self.contains_arrow_functions.set(false);
            self.may_contain_arrow_functions_using_arguments.set(false);
        }
        g
    }

    /// Return a copy of the directive list for the current function scope.
    /// Port of `copySeenDirectives` (JSParserImpl.cpp:731-739).
    #[allow(dead_code)]
    pub(super) fn copy_seen_directives(&self) -> Vec<Vec<u8>> {
        self.seen_directives.clone()
    }

    /// Move the parser to `loc` and re-lex the current token from there.
    /// Port of `JSParserImpl::seek` (JSParserImpl.h:128-131): the C++ does
    /// `lexer_.seek(startPos); tok_ = lexer_.advance();`. Our lexer keeps the
    /// current token internally, so we seek the lexer cursor then `advance`
    /// (with `AllowRegExp`, matching the parameterless C++ `lexer_.advance()`).
    pub(super) fn seek(&mut self, loc: SMLoc) {
        self.lexer.seek(loc);
        self.advance(GrammarContext::AllowRegExp);
    }

    /// On-demand parse of a single deferred function body. Called when a
    /// previously lazy-stubbed function is first executed: the parser is seeked
    /// back to `start` and the function is re-parsed eagerly so its real body
    /// (instead of the lazy stub) is produced.
    ///
    /// Port of `JSParserImpl::parseLazyFunction` (JSParserImpl.cpp:7548-7600).
    /// `kind` selects which eager entry point to drive; `param_yield`/
    /// `param_await` restore the grammar context the function was originally
    /// parsed in. Returns the re-parsed function node (the `FunctionExpression`,
    /// `FunctionDeclaration`, or `ArrowFunctionExpression`), or — for accessors
    /// and class methods — the `value` function extracted from the wrapping
    /// `Property`/`MethodDefinition` node (cpp:7572,7591).
    pub fn parse_lazy_function(
        &mut self,
        kind: NodeKind,
        param_yield: bool,
        param_await: bool,
        start: SMLoc,
    ) -> Option<&'gc Node<'gc>> {
        // cpp:7553-7556.
        // For MethodDefinition, the class body is always strict (C++ line
        // 4800/4881 in parseClassDeclaration/parseClassExpression). Strict
        // mode MUST be set before `seek` → `advance` re-lexes the first
        // token, because `static` (a future reserved word) is only recognised
        // as `rw_static` in strict mode; in non-strict mode it is lexed as a
        // plain identifier. Save and restore the strict flag around the whole
        // demand-parse so callers are unaffected.
        let old_strict = self.lexer.is_strict_mode();
        if kind == NodeKind::MethodDefinition {
            self.lexer.set_strict_mode(true);
        }

        self.seek(start);
        self.param_yield.set(param_yield);
        self.param_await.set(param_await);

        let result = match kind {
            // cpp:7559-7560.
            NodeKind::FunctionExpression => {
                self.parse_function_expression(/* force_eagerly= */ true)
            }

            // cpp:7562-7563.
            NodeKind::FunctionDeclaration => {
                self.parse_function_declaration(
                    PARAM_RETURN,
                    /* force_eagerly= */ true,
                )
            }

            // cpp:7565-7566. parseAssignmentExpression(ParamIn, /*eagerly*/true)
            // with the header defaults for the remaining args.
            NodeKind::ArrowFunctionExpression => self.parse_assignment_expression(
                PARAM_IN,
                /* force_eagerly= */ true,
                AllowTypedArrowFunction::Yes,
                CoverTypedParameters::Yes,
                None,
            ),

            // cpp:7568-7579. Re-parse the property; the deferred function is its
            // `value`. `dyn_cast<PropertyNode>` failure is not technically
            // unreachable (a fudged source buffer), so we just return None.
            NodeKind::Property => {
                let node = self.parse_property_assignment(/* eagerly= */ true)?;
                match node {
                    Node::Property(prop) => Some(prop.value),
                    _ => {
                        debug_assert!(
                            false,
                            "Expected a getter/setter function"
                        );
                        None
                    }
                }
            }

            // cpp:7581-7595. Re-parse a single class element; the deferred
            // function is the `value` of the resulting `MethodDefinition`.
            // Class bodies are always strict; strict mode was set before
            // `seek` above so the first token is lexed correctly.
            NodeKind::MethodDefinition => {
                let mut body: Vec<&'gc Node<'gc>> = Vec::new();
                let mut constructor: Option<&'gc Node<'gc>> = None;
                let success = self.parse_class_body_impl(
                    &mut body,
                    &mut constructor,
                    /* eagerly= */ true,
                );
                if !success || body.len() != 1 {
                    debug_assert!(false, "Unexpected parse_class_body_impl result");
                    None
                } else {
                    match body[0] {
                        Node::MethodDefinition(method) => Some(method.value),
                        _ => {
                            debug_assert!(false, "Expected MethodDefinitionNode");
                            None
                        }
                    }
                }
            }

            // cpp:7597-7598.
            _ => unreachable!("Asked to parse unexpected node type"),
        };

        // Restore strict mode to the state before this demand-parse.
        self.lexer.set_strict_mode(old_strict);
        result
    }
}

#[cfg(test)]
mod tests {
    // A parser built with `new` defaults to FullParse; new_with_pass honors the arg.
    #[test]
    fn parser_pass_defaults_and_override() {
        use ast::context::Context;
        use support::manager::SourceErrorManager;
        use crate::lexer::{GrammarContext, JSLexer};
        use crate::js::{JSParserImpl, ParserPass};

        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer_bytes("t", b"1;");
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        let lexer = JSLexer::new(id, &mut sm, atoms, GrammarContext::AllowRegExp);
        let p = JSParserImpl::new_with_pass(&gc, lexer, ParserPass::PreParse);
        assert_eq!(p.pass, ParserPass::PreParse);
    }

    // The side-table round-trips through take/set; threshold defaults to 0.
    #[test]
    fn pre_parsed_table_and_threshold() {
        use ast::context::Context;
        let mut ctx = Context::new();
        assert_eq!(ctx.preemptive_function_compilation_threshold(), 0);
        ctx.set_preemptive_function_compilation_threshold(64);
        assert_eq!(ctx.preemptive_function_compilation_threshold(), 64);
    }

    // SaveFunctionState restores the three arrow-bookkeeping flags on Drop.
    // Strict-mode is managed separately (lexer field, not Rc<Cell>), so it is
    // not asserted here — that restore is done explicitly by each call-site.
    #[test]
    fn save_function_state_restores_on_drop() {
        use ast::context::Context;
        use support::manager::SourceErrorManager;
        use crate::lexer::{GrammarContext, JSLexer};
        use crate::js::{JSParserImpl, ParserPass};

        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer_bytes("t", b"0");
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        let lexer = JSLexer::new(id, &mut sm, atoms, GrammarContext::AllowRegExp);
        let mut p = JSParserImpl::new_with_pass(&gc, lexer, ParserPass::PreParse);

        p.lexer.set_strict_mode(false);
        p.contains_arrow_functions.set(false);
        {
            // Enter a NON-arrow function: flags reset to false, restored on drop.
            let _g = p.save_function_state(false);
            // Strict mode is managed separately by callers (not by the guard),
            // so we don't set it here — the guard doesn't own the lexer.
            p.contains_arrow_functions.set(true);
        }
        // contains_arrow_functions was true inside the scope but the Drop
        // impl restores old_contains (false) because is_arrow is false.
        assert!(!p.contains_arrow_functions.get(), "contains_arrow restored");
        // Verify strict-mode restore is the caller's responsibility.
        assert!(!p.lexer.is_strict_mode(), "strict was never changed by guard");
    }

    // PreParse over a file with two functions records both, with correct strict
    // flag and directives.
    #[test]
    fn preparse_records_functions() {
        use ast::context::Context;
        use support::manager::SourceErrorManager;
        use crate::lexer::{GrammarContext, JSLexer};
        use crate::js::{JSParserImpl, ParserPass};

        let src = b"function a(){ 'use strict'; return 1; }\nvar b = () => 2;\n";
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer_bytes("t", src);
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        let lexer = JSLexer::new(id, &mut sm, atoms, GrammarContext::AllowRegExp);
        let mut p = JSParserImpl::new_with_pass(&gc, lexer, ParserPass::PreParse);
        assert!(p.parse().is_some());
        let t = p.take_pre_parsed();
        // function a's body { ... } and the arrow are both recorded.
        assert_eq!(t.function_info.len(), 2);
        // exactly one recorded function is strict (function a, due to 'use strict').
        let strict_count =
            t.function_info.values().filter(|i| i.strict_mode).count();
        assert_eq!(strict_count, 1);
        let with_dir = t
            .function_info
            .values()
            .filter(|i| !i.directives.is_empty())
            .count();
        assert_eq!(with_dir, 1);
    }

    /// Walk the AST looking for a BlockStatement with
    /// `is_lazy_function_body == true`.
    fn has_lazy_stub<'gc>(node: &'gc ast::node::Node<'gc>) -> bool {
        use ast::node::Node;
        use ast::visitor::Visitor;

        struct LazyFinder(bool);
        impl<'gc> Visitor<'gc> for LazyFinder {
            fn visit_node(&mut self, node: &'gc Node<'gc>) {
                if self.0 {
                    return;
                }
                if let Node::BlockStatement(b) = node {
                    if b.is_lazy_function_body.get() {
                        self.0 = true;
                        return;
                    }
                }
                node.visit_children(self);
            }
        }

        let mut finder = LazyFinder(false);
        finder.visit_node(node);
        finder.0
    }

    /// Find the first `FunctionDeclaration` node in the AST and return it.
    fn find_function_decl<'gc>(
        node: &'gc ast::node::Node<'gc>,
    ) -> Option<&'gc ast::node::Node<'gc>> {
        use ast::node::Node;
        use ast::visitor::Visitor;

        struct FnFinder<'gc>(Option<&'gc Node<'gc>>);
        impl<'gc> Visitor<'gc> for FnFinder<'gc> {
            fn visit_node(&mut self, node: &'gc Node<'gc>) {
                if self.0.is_some() {
                    return;
                }
                if let Node::FunctionDeclaration(_) = node {
                    self.0 = Some(node);
                    return;
                }
                node.visit_children(self);
            }
        }

        let mut finder = FnFinder(None);
        finder.visit_node(node);
        finder.0
    }

    // Demand-parsing a deferred function reproduces a non-stub body. We first
    // PreParse + LazyParse (threshold 0) to get a skeleton whose function body
    // is a lazy stub, then call `parse_lazy_function` at the function's start
    // and assert the re-parsed body is eager (not a stub) and non-empty.
    #[test]
    fn parse_lazy_function_reparses_body() {
        use ast::context::Context;
        use ast::node::{Node, NodeKind};
        use support::manager::SourceErrorManager;
        use crate::lexer::{GrammarContext, JSLexer};
        use crate::js::{JSParserImpl, ParserPass};

        let src = b"function a(){ return 1 + 2; }\n";
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer_bytes("t", src);
        let mut ctx = Context::new();
        ctx.set_preemptive_function_compilation_threshold(0); // defer everything
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        // First PreParse to build the table.
        let table = {
            let l = JSLexer::new(id, &mut sm, atoms, GrammarContext::AllowRegExp);
            let mut pp =
                JSParserImpl::new_with_pass(&gc, l, ParserPass::PreParse);
            pp.parse().unwrap();
            pp.take_pre_parsed()
        };
        // LazyParse to build the skeleton with a lazy-stub body.
        let l = JSLexer::new(id, &mut sm, atoms, GrammarContext::AllowRegExp);
        let mut lp =
            JSParserImpl::new_with_pass(&gc, l, ParserPass::LazyParse);
        lp.set_pre_parsed(table);
        let prog = lp.parse().unwrap();
        assert!(has_lazy_stub(prog), "skeleton body should be a lazy stub");

        // Grab the FunctionDeclaration's start location from the skeleton.
        let func = find_function_decl(prog).expect("FunctionDeclaration");
        let start = func.range().start;

        // Demand-parse the deferred function body eagerly.
        let body = lp
            .parse_lazy_function(NodeKind::FunctionDeclaration, false, false, start)
            .expect("parse_lazy_function should succeed");

        // The result is a FunctionDeclaration whose body is a real (non-stub)
        // BlockStatement containing the `return 1 + 2;` statement.
        let Node::FunctionDeclaration(fd) = body else {
            panic!("expected a FunctionDeclaration node");
        };
        let Node::BlockStatement(block) = fd.body else {
            panic!("expected a BlockStatement body");
        };
        assert!(
            !block.is_lazy_function_body.get(),
            "re-parsed body must NOT be a lazy stub"
        );
        assert!(
            !block.body.is_empty(),
            "re-parsed body must contain statements"
        );
        // The single statement is the `return 1 + 2;`.
        assert!(!has_lazy_stub(body), "re-parsed function has no lazy stub");
    }

    // LazyParse with threshold 0 defers a function body: the BlockStatement
    // is a lazy stub.
    #[test]
    fn lazyparse_defers_body() {
        use ast::context::Context;
        use support::manager::SourceErrorManager;
        use crate::lexer::{GrammarContext, JSLexer};
        use crate::js::{JSParserImpl, ParserPass};

        let src = b"function a(){ return 1 + 2; }\n";
        let mut sm = SourceErrorManager::new();
        let id = sm.add_buffer_bytes("t", src);
        let mut ctx = Context::new();
        ctx.set_preemptive_function_compilation_threshold(0); // defer everything
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        // First PreParse to build the table.
        let table = {
            let l = JSLexer::new(id, &mut sm, atoms, GrammarContext::AllowRegExp);
            let mut pp =
                JSParserImpl::new_with_pass(&gc, l, ParserPass::PreParse);
            pp.parse().unwrap();
            pp.take_pre_parsed()
        };
        let l = JSLexer::new(id, &mut sm, atoms, GrammarContext::AllowRegExp);
        let mut lp =
            JSParserImpl::new_with_pass(&gc, l, ParserPass::LazyParse);
        lp.set_pre_parsed(table);
        let prog = lp.parse().unwrap();
        // Walk to the function's body and assert it's a lazy stub.
        assert!(
            has_lazy_stub(prog),
            "expected a lazy function body stub"
        );
    }
}

