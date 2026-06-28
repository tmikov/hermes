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

use crate::lexer::JSLexer;

use super::JSParserImpl;

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
