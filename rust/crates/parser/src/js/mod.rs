/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The JS parser (`JSParserImpl`). Port of `lib/Parser/JSParserImpl*`.
//! Recursive-descent LL(1) over `JSLexer`, building the `ast` ESTree.

// Items added here are used from P1+ parsing phases; suppress warnings so the
// P0 scaffold stays warning-free even though only `new` and the first-token
// test are exercised now.
// TODO(parser-P1): once expression/statement parsing wires these up, drop this
// module-level allow (or narrow it to whatever genuinely remains unused) so new
// dead code is caught again.
#![allow(dead_code)] // used from P1+

use ast::context::GCLock;
use ast::node::Node;
use support::location::{SMLoc, SMRange};

use crate::lexer::{GrammarContext, JSLexer};
use crate::token_kinds::TokenKind;

/// A bitmask of grammar parameters threaded between parse functions.
/// Port of `JSParserImpl::Param`.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Param(u32);

/// `[In]` — "in" is recognized as a binary operator in RelationalExpression.
pub const PARAM_IN: Param = Param(1 << 0);
/// `[Return]`
pub const PARAM_RETURN: Param = Param(1 << 1);
/// `[Default]`
pub const PARAM_DEFAULT: Param = Param(1 << 2);
/// `[Tagged]`
pub const PARAM_TAGGED: Param = Param(1 << 3);

impl Param {
    /// Union (C++ `operator+`).
    pub fn plus(self, b: Param) -> Param {
        Param(self.0 | b.0)
    }
    /// Difference (C++ `operator-`).
    pub fn minus(self, b: Param) -> Param {
        Param(self.0 & !b.0)
    }
    /// True if any flag in `p` is set (C++ `has`).
    pub fn has(self, p: Param) -> bool {
        (self.0 & p.0) != 0
    }
    /// True if ALL flags in `p` are set (C++ `hasAll`).
    pub fn has_all(self, p: Param) -> bool {
        (self.0 & p.0) == p.0
    }
    /// `p` if any of its bits are set here, else empty (C++ `get`).
    /// (The C++ variadic `get(p, tail...)` is just `a.get(x).plus(a.get(y))`
    /// in Rust — single-arg `get` + `plus` cover it.)
    pub fn get(self, p: Param) -> Param {
        Param(self.0 & p.0)
    }
}

/// Maximum recursion depth, mirroring the non-MSVC default in JSParserImpl.h.
const MAX_RECURSION_DEPTH: u32 = 1024;

/// The JS parser.
///
/// Four lifetime parameters:
/// - `'gc`: the borrow-of-lock lifetime. `gc.alloc(n)` returns
///   `&'gc Node<'gc>` because `GCLock::alloc<'s>(&'s self, ..)` makes
///   `'s = 'gc`. This is also the node child-ref lifetime (i.e. child refs
///   inside built nodes are `&'gc Node<'gc>`).
/// - `'ast`: the `Context`'s own arena lifetime (first type param of
///   `GCLock<'ast, 'ctx>`). Kept separate so the borrow `'gc` doesn't
///   accidentally constrain when the `Context` was created.
/// - `'ctx`: the `&mut Context` borrow lifetime (second type param).
///   Kept separate for the same reason.
/// - `'a`: the lexer's borrow lifetimes (`&'a mut SourceErrorManager` and
///   `&'a AtomTable`).
///
/// In practice `'ast`, `'ctx`, and `'a` all unify to the enclosing call
/// frame, so callers see no extra friction.
///
/// Port of `lib/Parser/JSParserImpl.h`/`.cpp`.
pub struct JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    /// The arena lock; all nodes are allocated through this.
    /// `gc.alloc(n: Node<'gc>) -> &'gc Node<'gc>` (borrow lifetime `'gc`).
    gc: &'gc GCLock<'ast, 'ctx>,
    /// The lexer driving the token stream. Owns `&'a mut SourceErrorManager`.
    lexer: JSLexer<'a>,
    /// Current parser recursion depth (stack-overflow guard).
    recursion_depth: u32,
    /// Set when the parser is inside a generator function (`yield`).
    param_yield: bool,
    /// Set when the parser is inside an async function (`await`).
    param_await: bool,
    /// Set on the `use static builtin` directive.
    use_static_builtin: bool,
}

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    /// Construct the parser and lex the first token (C++ ctor does
    /// `tok_ = lexer_.advance()`).
    pub fn new(gc: &'gc GCLock<'ast, 'ctx>, mut lexer: JSLexer<'a>) -> Self {
        lexer.advance(GrammarContext::AllowRegExp);
        JSParserImpl {
            gc,
            lexer,
            recursion_depth: 0,
            param_yield: false,
            param_await: false,
            use_static_builtin: false,
        }
    }

    /// True if the parser detected `use static builtin`.
    pub fn get_use_static_builtin(&self) -> bool {
        self.use_static_builtin
    }

    #[inline]
    fn cur_kind(&self) -> TokenKind {
        self.lexer.token().kind()
    }
    #[inline]
    fn cur_range(&self) -> SMRange {
        self.lexer.token().source_range()
    }
    #[inline]
    fn cur_start(&self) -> SMLoc {
        self.lexer.token().start_loc()
    }

    /// True if the current token is `kind`. Port of `check(TokenKind)`.
    #[inline]
    fn check(&self, kind: TokenKind) -> bool {
        self.cur_kind() == kind
    }
    /// True if the current token is `k1` or `k2`. Port of `check(k1, k2)`.
    #[inline]
    fn check2(&self, k1: TokenKind, k2: TokenKind) -> bool {
        let k = self.cur_kind();
        k == k1 || k == k2
    }

    /// Consume the current token, advancing the lexer; return the consumed
    /// token's range. Port of `JSParserImpl::advance` (C++ returns the PREVIOUS
    /// token's range — we copy it out before advancing).
    fn advance(&mut self, grammar_context: GrammarContext) -> SMRange {
        let prev = self.cur_range();
        self.lexer.advance(grammar_context);
        prev
    }

    /// Consume the current token if it is `kind`; return whether it matched.
    fn check_and_eat(&mut self, kind: TokenKind, grammar_context: GrammarContext) -> bool {
        if self.check(kind) {
            self.advance(grammar_context);
            true
        } else {
            false
        }
    }

    /// Report an error at `range`. Routed through the lexer's SourceErrorManager.
    /// Uses `error_at(loc, range, msg, subsystem)` to attach a range underline
    /// and mark it as a Parser-subsystem diagnostic.
    /// TODO(parser-P1): port the C++ `error(SMLoc, SMRange, msg)` error-limit
    /// behavior (return false + `lexer.force_eof()` once the max error count is
    /// reached) when statement parsing can emit error sequences.
    fn error_at(&mut self, range: SMRange, msg: &str) {
        self.lexer.get_source_mgr_mut().error_at(
            range.start,
            Some(range),
            msg,
            support::diag::Subsystem::Parser,
        );
    }
    /// Report an error at the current token. Port of `error(Twine)`.
    fn error_cur(&mut self, msg: &str) {
        let range = self.cur_range();
        self.error_at(range, msg);
    }

    /// Check the current token is `kind`; if not, report an error and return
    /// false. Port of `need` (P0 form; richer where/what plumbing arrives later).
    fn need(&mut self, kind: TokenKind, where_: &str) -> bool {
        if self.check(kind) {
            return true;
        }
        let msg = format!(
            "'{}' expected{}",
            crate::token_kinds::token_kind_str(kind),
            where_
        );
        self.error_cur(&msg);
        false
    }
    /// Check the current token is `kind`; if so consume and return true, else
    /// report an error and return false. Port of `eat`.
    fn eat(
        &mut self,
        kind: TokenKind,
        grammar_context: GrammarContext,
        where_: &str,
    ) -> bool {
        if self.need(kind, where_) {
            self.advance(grammar_context);
            true
        } else {
            false
        }
    }

    /// Return true (and report an error) if the recursion limit is exceeded.
    #[inline]
    fn recursion_depth_check(&mut self) -> bool {
        if self.recursion_depth < MAX_RECURSION_DEPTH {
            return false;
        }
        let range = self.cur_range();
        self.error_at(range, "Too many nested expressions/statements/declarations");
        true
    }

    /// Allocate `node` with its source locations set. Port of the 3-arg
    /// `setLocation(start, end, node)`: debug loc defaults to start.
    ///
    /// Note: `gc.alloc` borrows `*self.gc` for `'gc`, which is the same
    /// lifetime as the returned reference — safe because `self.gc` outlives
    /// `self` (the caller holds both).
    fn set_location(&self, start: SMLoc, end: SMLoc, node: Node<'gc>) -> &'gc Node<'gc> {
        let allocated = self.gc.alloc(node);
        let md = allocated.metadata();
        md.range.set(SMRange { start, end });
        md.debug_loc.set(start);
        allocated
    }

    /// Parse the whole program. Entry point for the parser.
    /// Port of `JSParserImpl::parse` / `parseProgram`
    /// (P0: trivia-only sources → empty Program; statement parsing is P1-P4).
    pub fn parse(&mut self) -> Option<&'gc Node<'gc>> {
        self.parse_program()
    }

    /// Parse a `Program` node. The first significant token must be EOF
    /// (statement-list parsing arrives in P1-P4).
    fn parse_program(&mut self) -> Option<&'gc Node<'gc>> {
        use ast::node::Program;
        use ast::node_child::{NodeList, NodeMetadata};

        let start = self.cur_start();
        // P0 supports only trivia-only sources: the first significant token must
        // be EOF.  (Statement-list parsing lands in P1-P4.)
        if !self.check(TokenKind::eof) {
            self.error_cur("statement parsing not yet implemented (parser phase P0)");
            return None;
        }
        // EOF: zero-width range at end of input.
        let end = self.cur_start();
        // `Program::new` requires metadata; pass start/end for consistency even
        // though `set_location` below is the authoritative stamp (it overwrites
        // range + debug_loc, matching C++ `setLocation`).
        let program = Node::Program(Program::new(
            NodeMetadata::new(SMRange { start, end }),
            NodeList::empty(),
        ));
        Some(self.set_location(start, end, program))
    }

    /// Test-only accessor for the current token kind.
    #[cfg(test)]
    pub(crate) fn cur_kind_pub(&self) -> TokenKind {
        self.cur_kind()
    }

    /// Test-only accessor for the error count.
    #[cfg(test)]
    pub(crate) fn error_count_pub(&self) -> u32 {
        self.lexer.get_source_mgr().error_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_constructs_and_sees_first_token() {
        use ast::context::Context;
        use support::manager::SourceErrorManager;

        // `sm` must be declared before `ctx` so that it outlives `gc` and
        // therefore `lexer` (which borrows `&'a mut sm`). Rust drops in
        // reverse declaration order, so sm is dropped last.
        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"  /* hi */  ");
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        let lexer = crate::lexer::JSLexer::new(
            buf_id,
            &mut sm,
            atoms,
            crate::lexer::GrammarContext::AllowRegExp,
        );
        let parser = JSParserImpl::new(&gc, lexer);
        assert_eq!(
            parser.cur_kind_pub(),
            crate::token_kinds::TokenKind::eof
        );
    }

    #[test]
    fn parses_empty_program() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"/* only trivia */\n");
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        let lexer = crate::lexer::JSLexer::new(
            buf_id,
            &mut sm,
            atoms,
            crate::lexer::GrammarContext::AllowRegExp,
        );
        let mut parser = JSParserImpl::new(&gc, lexer);
        let program = parser.parse().expect("empty program parses");
        match program {
            Node::Program(p) => assert!(p.body.is_empty(), "empty source -> empty body"),
            other => panic!("expected Program, got {:?}", other.kind()),
        }
        assert_eq!(parser.error_count_pub(), 0);
    }

    #[test]
    fn non_eof_input_errors_in_p0() {
        use ast::context::Context;
        use support::manager::SourceErrorManager;

        // A real statement is not yet parseable in P0: parse must report an
        // error and return None (locks in the EOF guard).
        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"let x = 1;\n");
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        let lexer = crate::lexer::JSLexer::new(
            buf_id,
            &mut sm,
            atoms,
            crate::lexer::GrammarContext::AllowRegExp,
        );
        let mut parser = JSParserImpl::new(&gc, lexer);
        assert!(parser.parse().is_none(), "non-EOF input must not parse in P0");
        assert!(parser.error_count_pub() >= 1, "an error must be reported");
    }
}
