/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The JS parser (`JSParserImpl`). Port of `lib/Parser/JSParserImpl*`.
//! Recursive-descent LL(1) over `JSLexer`, building the `ast` ESTree.

use std::cell::Cell;
use std::rc::Rc;

use ast::context::GCLock;
use ast::node::Node;
use support::location::{SMLoc, SMRange};

use crate::lexer::{GrammarContext, JSLexer};
use crate::token_kinds::TokenKind;

mod expressions;
mod statements;

/// Whether import/export declarations are allowed in this statement list.
/// Port of `JSParserImpl::AllowImportExport`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)] // `No` variant used in P2+ (block-statement parsing)
pub(super) enum AllowImportExport {
    Yes,
    No,
}

/// Whether we are recursing into `new` expression parsing.
/// Port of C++ `JSParserImpl::IsConstructorCall`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum IsConstructorCall {
    No,
    Yes,
}

/// Whether this LHS is being parsed as the `extends` clause of a class.
/// Port of C++ `JSParserImpl::IsClassHeritageArgument`.
/// P1 callers always pass `No`; P3+ (class parsing) will pass `Yes`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)]
pub(super) enum IsClassHeritageArgument {
    No,
    Yes,
}

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

/// RAII guard for the recursion depth counter. Decrements on Drop so that
/// every `check_recursion` call site is balanced even on early return.
///
/// Design note: the C++ uses a macro that increments on entry and decrements
/// on scope exit via RAII. A guard holding `&mut self.recursion_depth` can't
/// coexist with the `&mut self` the parse methods need, so instead the counter
/// lives in an `Rc<Cell<u32>>` and the guard owns its own `Rc` clone (no borrow
/// into `self`, zero `unsafe`). The `Rc::clone` per recursive entry is a
/// pointer copy + non-atomic refcount bump — negligible vs. lexing/allocation.
pub(super) struct RecursionGuard(Rc<Cell<u32>>);

impl Drop for RecursionGuard {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

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
    pub(super) lexer: JSLexer<'a>,
    /// Current parser recursion depth (stack-overflow guard). In an
    /// `Rc<Cell<u32>>` so `RecursionGuard` can own a handle without borrowing
    /// `self` (see `RecursionGuard`).
    recursion_depth: Rc<Cell<u32>>,
    /// Set when the parser is inside a generator function (`yield`).
    pub(super) param_yield: bool,
    /// Set when the parser is inside an async function (`await`).
    /// Read in P1.3+ (await expression parsing in parseUnaryExpression).
    pub(super) param_await: bool,
    /// Set on the `use static builtin` directive.
    pub(super) use_static_builtin: bool,
}

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    /// Construct the parser and lex the first token (C++ ctor does
    /// `tok_ = lexer_.advance()`).
    pub fn new(gc: &'gc GCLock<'ast, 'ctx>, mut lexer: JSLexer<'a>) -> Self {
        lexer.advance(GrammarContext::AllowRegExp);
        JSParserImpl {
            gc,
            lexer,
            recursion_depth: Rc::new(Cell::new(0)),
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
    pub(super) fn cur_kind(&self) -> TokenKind {
        self.lexer.token().kind()
    }
    #[inline]
    pub(super) fn cur_range(&self) -> SMRange {
        self.lexer.token().source_range()
    }
    #[inline]
    pub(super) fn cur_start(&self) -> SMLoc {
        self.lexer.token().start_loc()
    }

    /// True if the current token is `kind`. Port of `check(TokenKind)`.
    #[inline]
    pub(super) fn check(&self, kind: TokenKind) -> bool {
        self.cur_kind() == kind
    }
    /// True if the current token is `k1` or `k2`. Port of `check(k1, k2)`.
    #[inline]
    pub(super) fn check2(&self, k1: TokenKind, k2: TokenKind) -> bool {
        let k = self.cur_kind();
        k == k1 || k == k2
    }
    /// True if the current token is any of three kinds.
    /// Port of `checkN(k1,k2,k3)`.
    #[inline]
    pub(super) fn check_n3(
        &self,
        k1: TokenKind,
        k2: TokenKind,
        k3: TokenKind,
    ) -> bool {
        let k = self.cur_kind();
        k == k1 || k == k2 || k == k3
    }

    /// True if the current token is any of four kinds.
    /// Port of `checkN(k1,k2,k3,k4)`.
    #[inline]
    pub(super) fn check_n4(
        &self,
        k1: TokenKind,
        k2: TokenKind,
        k3: TokenKind,
        k4: TokenKind,
    ) -> bool {
        let k = self.cur_kind();
        k == k1 || k == k2 || k == k3 || k == k4
    }

    /// Consume the current token, advancing the lexer; return the consumed
    /// token's range. Port of `JSParserImpl::advance` (C++ returns the PREVIOUS
    /// token's range — we copy it out before advancing).
    pub(super) fn advance(&mut self, grammar_context: GrammarContext) -> SMRange {
        let prev = self.cur_range();
        self.lexer.advance(grammar_context);
        prev
    }

    /// Consume the current token if it is `kind`; return whether it matched.
    pub(super) fn check_and_eat(
        &mut self,
        kind: TokenKind,
        grammar_context: GrammarContext,
    ) -> bool {
        if self.check(kind) {
            self.advance(grammar_context);
            true
        } else {
            false
        }
    }

    /// Report an error at `range`. Routed through the lexer's SourceErrorManager.
    fn error_at(&mut self, range: SMRange, msg: &str) {
        self.lexer.get_source_mgr_mut().error_at(
            range.start,
            Some(range),
            msg,
            support::diag::Subsystem::Parser,
        );
    }
    /// Report an error at the current token. Port of `error(Twine)`.
    pub(super) fn error_cur(&mut self, msg: &str) {
        let range = self.cur_range();
        self.error_at(range, msg);
    }

    /// Check the current token is `kind`; if not, report an error and return
    /// false. Port of `need`.
    pub(super) fn need(&mut self, kind: TokenKind, where_: &str) -> bool {
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
    pub(super) fn eat(
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

    /// Increment the recursion depth and return a guard that decrements it on
    /// drop. Returns `None` (and reports an error) if the limit is exceeded.
    ///
    /// Port of the `CHECK_RECURSION` macro (JSParserImpl.h). The returned guard
    /// owns an `Rc<Cell<u32>>` clone and decrements on drop, so the caller can
    /// freely use `&mut self` for parse calls while the guard is alive.
    pub(super) fn check_recursion(&mut self) -> Option<RecursionGuard> {
        let depth = self.recursion_depth.get() + 1;
        if depth > MAX_RECURSION_DEPTH {
            // Don't leave it incremented.
            let range = self.cur_range();
            self.error_at(range, "Too many nested expressions/statements/declarations");
            return None;
        }
        self.recursion_depth.set(depth);
        Some(RecursionGuard(Rc::clone(&self.recursion_depth)))
    }

    /// Return a placeholder `SMRange` (zero-width at current token start).
    /// Used as the initial `NodeMetadata` before `set_location` stamps the
    /// real range, mirroring the C++ pattern of constructing a node then
    /// calling `setLocation`.
    pub(super) fn dummy_range(&self) -> SMRange {
        let loc = self.cur_start();
        SMRange {
            start: loc,
            end: loc,
        }
    }

    /// Allocate `node` with its source locations set. Port of the 3-arg
    /// `setLocation(start, end, node)`: debug loc defaults to start.
    pub(super) fn set_location(
        &self,
        start: SMLoc,
        end: SMLoc,
        node: Node<'gc>,
    ) -> &'gc Node<'gc> {
        let allocated = self.gc.alloc(node);
        let md = allocated.metadata();
        md.range.set(SMRange { start, end });
        md.debug_loc.set(start);
        allocated
    }

    /// Allocate `node` with an explicit debug loc. Port of the 4-arg
    /// `setLocation(start, end, debugLoc, node)`.
    ///
    /// Used where C++ passes a *different* `debugLoc` than `start` — currently
    /// only the postfix `UpdateExpression` case where `debugLoc` is the start of
    /// the `++`/`--` operator token while `start` is the start of the operand.
    pub(super) fn set_location_d(
        &self,
        start: SMLoc,
        end: SMLoc,
        debug: SMLoc,
        node: Node<'gc>,
    ) -> &'gc Node<'gc> {
        let allocated = self.gc.alloc(node);
        let md = allocated.metadata();
        md.range.set(SMRange { start, end });
        md.debug_loc.set(debug);
        allocated
    }

    /// Parse the whole program. Entry point for the parser.
    /// Port of `JSParserImpl::parse` / `parseProgram` (lines 355-381).
    pub fn parse(&mut self) -> Option<&'gc Node<'gc>> {
        self.parse_program()
    }

    /// Parse a `Program` node. Port of `JSParserImpl::parseProgram` (355-373).
    ///
    /// Parses directives + a statement list until EOF, then wraps in a Program.
    fn parse_program(&mut self) -> Option<&'gc Node<'gc>> {
        use ast::node::Program;
        use ast::node_child::{NodeList, NodeMetadata};

        let start = self.cur_start();

        let mut stmts: Vec<&'gc Node<'gc>> = Vec::new();
        if !self.parse_statement_list(
            Param::default(),
            TokenKind::eof,
            /* parse_directives= */ true,
            AllowImportExport::Yes,
            &mut stmts,
        ) {
            return None;
        }

        let end = if stmts.is_empty() {
            start
        } else {
            stmts.last().unwrap().metadata().range.get().end
        };

        let body = NodeList::from_iter(self.gc, stmts);
        let program = Node::Program(Program::new(
            NodeMetadata::new(SMRange { start, end }),
            body,
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
    fn parses_numeric_literal_stmt() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"42;\n");
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
        let program = parser.parse().expect("42; parses");
        assert_eq!(parser.error_count_pub(), 0);
        if let Node::Program(p) = program {
            assert_eq!(p.body.iter().count(), 1);
            let stmt = p.body.iter().next().unwrap();
            if let Node::ExpressionStatement(es) = stmt {
                if let Node::NumericLiteral(nl) = es.expression {
                    assert_eq!(nl.value.get(), 42.0);
                } else {
                    panic!("expected NumericLiteral");
                }
            } else {
                panic!("expected ExpressionStatement");
            }
        } else {
            panic!("expected Program");
        }
    }

    #[test]
    fn parses_empty_statement() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b";;;\n");
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
        let program = parser.parse().expect(";;; parses");
        assert_eq!(parser.error_count_pub(), 0);
        if let Node::Program(p) = program {
            assert_eq!(p.body.iter().count(), 3);
            for stmt in p.body {
                assert!(
                    matches!(stmt, Node::EmptyStatement(_)),
                    "expected EmptyStatement"
                );
            }
        } else {
            panic!("expected Program");
        }
    }

    #[test]
    fn deferred_if_statement_errors() {
        use ast::context::Context;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"if(x);");
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
        assert!(
            parser.parse().is_none(),
            "if statement should error in P1.1"
        );
        assert!(
            parser.error_count_pub() >= 1,
            "must report at least one error"
        );
    }

    #[test]
    fn deferred_function_expr_errors() {
        use ast::context::Context;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"(function(){});");
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
        assert!(
            parser.parse().is_none(),
            "function expr should error in P1.1"
        );
        assert!(parser.error_count_pub() >= 1);
    }

    /// Array literals are implemented in P1.7; `[1]` must now parse cleanly.
    #[test]
    fn array_literal_parses() {
        use ast::context::Context;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"[1];");
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
        assert!(
            parser.parse().is_some(),
            "array literal should parse successfully in P1.7"
        );
        assert_eq!(parser.error_count_pub(), 0);
    }

    #[test]
    fn parses_sequence_expression() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"1, 2, 3;");
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
        let program = parser.parse().expect("1, 2, 3; parses");
        assert_eq!(parser.error_count_pub(), 0);
        if let Node::Program(p) = program {
            assert_eq!(p.body.iter().count(), 1);
            let stmt = p.body.iter().next().unwrap();
            if let Node::ExpressionStatement(es) = stmt {
                assert!(
                    matches!(es.expression, Node::SequenceExpression(_)),
                    "expected SequenceExpression"
                );
            } else {
                panic!("expected ExpressionStatement");
            }
        }
    }

    #[test]
    fn use_strict_directive_sets_strict_mode() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"\"use strict\"; 1;");
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
        let program = parser.parse().expect("\"use strict\"; 1; parses");
        assert_eq!(parser.error_count_pub(), 0);
        if let Node::Program(p) = program {
            // Body should have 2 statements: the directive + numeric stmt.
            assert_eq!(p.body.iter().count(), 2);
        }
        // Strict mode is now set on the lexer.
        assert!(parser.lexer.is_strict_mode());
    }

    // P1.5: assignment expression tests.

    /// Helper: parse a snippet and extract the expression from the first
    /// ExpressionStatement.
    fn parse_expr_from<'gc>(
        gc: &'gc ast::context::GCLock<'_, '_>,
        sm: &mut support::manager::SourceErrorManager,
        atoms: &atom_table::AtomTable,
        src: &[u8],
    ) -> &'gc ast::node::Node<'gc> {
        let buf_id = sm.add_buffer_bytes("input", src);
        let lexer = crate::lexer::JSLexer::new(
            buf_id,
            sm,
            atoms,
            crate::lexer::GrammarContext::AllowRegExp,
        );
        let mut parser = JSParserImpl::new(gc, lexer);
        let program = parser.parse().expect("parse succeeded");
        assert_eq!(parser.error_count_pub(), 0, "zero errors");
        if let ast::node::Node::Program(p) = program {
            let stmt = p.body.iter().next().expect("has statement");
            if let ast::node::Node::ExpressionStatement(es) = stmt {
                return es.expression;
            }
        }
        panic!("expected ExpressionStatement");
    }

    #[test]
    fn parses_simple_assignment() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;

        let expr = parse_expr_from(&gc, &mut sm, atoms, b"a = b;");
        match expr {
            Node::AssignmentExpression(n) => {
                let op_bytes = gc.ctx().atom_table.bytes(n.operator.get());
                assert_eq!(op_bytes, b"=", "operator is =");
                assert!(
                    matches!(n.left, Node::Identifier(_)),
                    "left is Identifier"
                );
                assert!(
                    matches!(n.right, Node::Identifier(_)),
                    "right is Identifier"
                );
            }
            other => panic!("expected AssignmentExpression, got {:?}", other.kind()),
        }
    }

    #[test]
    fn parses_compound_assignment_plus() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;

        let expr = parse_expr_from(&gc, &mut sm, atoms, b"a += 1;");
        match expr {
            Node::AssignmentExpression(n) => {
                let op_bytes = gc.ctx().atom_table.bytes(n.operator.get());
                assert_eq!(op_bytes, b"+=", "operator is +=");
            }
            other => panic!("expected AssignmentExpression, got {:?}", other.kind()),
        }
    }

    #[test]
    fn parses_right_assoc_chain() {
        // a = b = c  must parse as  a = (b = c)
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;

        let expr = parse_expr_from(&gc, &mut sm, atoms, b"a = b = c;");
        match expr {
            Node::AssignmentExpression(outer) => {
                // outer.left == a
                assert!(
                    matches!(outer.left, Node::Identifier(_)),
                    "outer.left is Identifier(a)"
                );
                // outer.right == (b = c)
                match outer.right {
                    Node::AssignmentExpression(inner) => {
                        let inner_left = match inner.left {
                            Node::Identifier(id) => id,
                            other => panic!(
                                "expected Identifier(b), got {:?}",
                                other.kind()
                            ),
                        };
                        let b_bytes = gc.ctx().atom_table.bytes(inner_left.name.get());
                        assert_eq!(b_bytes, b"b", "inner.left is b");
                        assert!(
                            matches!(inner.right, Node::Identifier(_)),
                            "inner.right is Identifier(c)"
                        );
                    }
                    other => panic!(
                        "outer.right must be AssignmentExpression(b=c), got {:?}",
                        other.kind()
                    ),
                }
            }
            other => panic!("expected AssignmentExpression, got {:?}", other.kind()),
        }
    }

    #[test]
    fn assignment_not_confused_with_equality() {
        // `a == b` must NOT produce an AssignmentExpression.
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;

        let expr = parse_expr_from(&gc, &mut sm, atoms, b"a == b;");
        assert!(
            matches!(expr, Node::BinaryExpression(_)),
            "== produces BinaryExpression, not AssignmentExpression"
        );
    }

    #[test]
    fn arrow_expr_errors_in_p15() {
        // Arrow functions are P3; parsing `a => b` should error.
        use ast::context::Context;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"a => b;");
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
        assert!(parser.parse().is_none(), "arrow should error in P1.5");
        assert!(parser.error_count_pub() >= 1);
    }

    // P1.8: object literal tests.

    /// Helper: parse a snippet, return the parse result (Some = success).
    fn parse_snippet(sm: &mut support::manager::SourceErrorManager, src: &[u8]) -> bool {
        use ast::context::Context;
        let buf_id = sm.add_buffer_bytes("input", src);
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        let lexer = crate::lexer::JSLexer::new(
            buf_id,
            sm,
            atoms,
            crate::lexer::GrammarContext::AllowRegExp,
        );
        let mut parser = JSParserImpl::new(&gc, lexer);
        let result = parser.parse();
        result.is_some() && parser.error_count_pub() == 0
    }

    #[test]
    fn object_literal_empty_parses() {
        let mut sm = support::manager::SourceErrorManager::new();
        assert!(parse_snippet(&mut sm, b"({});"), "empty object literal");
    }

    #[test]
    fn object_literal_keyed_parses() {
        let mut sm = support::manager::SourceErrorManager::new();
        assert!(parse_snippet(&mut sm, b"({a: 1, b: 2});"), "keyed properties");
    }

    #[test]
    fn object_literal_shorthand_parses() {
        let mut sm = support::manager::SourceErrorManager::new();
        assert!(parse_snippet(&mut sm, b"({a, b});"), "shorthand properties");
    }

    #[test]
    fn object_literal_computed_parses() {
        let mut sm = support::manager::SourceErrorManager::new();
        assert!(parse_snippet(&mut sm, b"({[x]: 1});"), "computed key");
    }

    #[test]
    fn object_literal_spread_parses() {
        let mut sm = support::manager::SourceErrorManager::new();
        assert!(parse_snippet(&mut sm, b"({...a});"), "spread element");
    }

    #[test]
    fn object_literal_string_and_number_keys_parse() {
        let mut sm = support::manager::SourceErrorManager::new();
        assert!(
            parse_snippet(&mut sm, b"({\"s\": 1, 0: 2});"),
            "string and number keys"
        );
    }

    #[test]
    fn object_literal_get_set_as_data_property() {
        // `get` and `set` used as plain property names — must succeed.
        let mut sm = support::manager::SourceErrorManager::new();
        assert!(
            parse_snippet(&mut sm, b"({get: 1, set: 2});"),
            "get/set as data properties"
        );
        assert!(
            parse_snippet(&mut sm, b"({get, set});"),
            "get/set shorthand"
        );
    }

    #[test]
    fn object_literal_async_as_data_property() {
        // `async` used as a plain property name — must succeed.
        let mut sm = support::manager::SourceErrorManager::new();
        assert!(
            parse_snippet(&mut sm, b"({async: 1});"),
            "async as data property"
        );
        assert!(
            parse_snippet(&mut sm, b"({async});"),
            "async shorthand"
        );
    }

    #[test]
    fn object_literal_cover_initializer_parses() {
        // `({a=1})` is a CoverInitializedName — hermesc accepts it in raw AST dump.
        let mut sm = support::manager::SourceErrorManager::new();
        assert!(
            parse_snippet(&mut sm, b"({a=1});"),
            "CoverInitializedName must parse"
        );
    }

    #[test]
    fn object_getter_deferred() {
        // Real getter `{get foo() {}}` — must error (P3).
        use ast::context::Context;
        let mut sm = support::manager::SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"({get foo() {}});");
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
        assert!(
            parser.parse().is_none(),
            "getter must error in P1.8 (deferred to P3)"
        );
        assert!(parser.error_count_pub() >= 1);
    }

    #[test]
    fn object_setter_deferred() {
        // Real setter `{set foo(v) {}}` — must error (P3).
        use ast::context::Context;
        let mut sm = support::manager::SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"({set foo(v) {}});");
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
        assert!(
            parser.parse().is_none(),
            "setter must error in P1.8 (deferred to P3)"
        );
        assert!(parser.error_count_pub() >= 1);
    }

    #[test]
    fn object_method_deferred() {
        // Object method `{foo() {}}` — must error (P3).
        use ast::context::Context;
        let mut sm = support::manager::SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"({foo() {}});");
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
        assert!(
            parser.parse().is_none(),
            "method must error in P1.8 (deferred to P3)"
        );
        assert!(parser.error_count_pub() >= 1);
    }

    #[test]
    fn object_async_method_deferred() {
        // Async method `{async foo() {}}` — must error (P3).
        use ast::context::Context;
        let mut sm = support::manager::SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"({async foo() {}});");
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
        assert!(
            parser.parse().is_none(),
            "async method must error in P1.8 (deferred to P3)"
        );
        assert!(parser.error_count_pub() >= 1);
    }

    #[test]
    fn object_generator_method_deferred() {
        // Generator method `{*foo() {}}` — must error (P3).
        use ast::context::Context;
        let mut sm = support::manager::SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"({*foo() {}});");
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
        assert!(
            parser.parse().is_none(),
            "generator method must error in P1.8 (deferred to P3)"
        );
        assert!(parser.error_count_pub() >= 1);
    }

    // P1.8b: destructuring-assignment reparse tests.

    /// Helper: parse source, expect success, return first-statement expression.
    fn parse_expr_ok<'gc>(
        gc: &'gc ast::context::GCLock<'_, '_>,
        sm: &mut support::manager::SourceErrorManager,
        src: &[u8],
    ) -> &'gc ast::node::Node<'gc> {
        let buf_id = sm.add_buffer_bytes("input", src);
        let atoms = &gc.ctx().atom_table;
        let lexer = crate::lexer::JSLexer::new(
            buf_id,
            sm,
            atoms,
            crate::lexer::GrammarContext::AllowRegExp,
        );
        let mut parser = JSParserImpl::new(gc, lexer);
        let program = parser.parse().expect("parse succeeded");
        assert_eq!(parser.error_count_pub(), 0, "zero errors");
        if let ast::node::Node::Program(p) = program {
            let stmt = p.body.iter().next().expect("has statement");
            if let ast::node::Node::ExpressionStatement(es) = stmt {
                return es.expression;
            }
        }
        panic!("expected ExpressionStatement");
    }

    #[test]
    fn array_destructure_simple() {
        // `[a] = b` → AssignmentExpression(=, ArrayPattern([Identifier(a)]), ...)
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let expr = parse_expr_ok(&gc, &mut sm, b"[a] = b;");
        match expr {
            Node::AssignmentExpression(asn) => {
                let op = gc.ctx().atom_table.bytes(asn.operator.get());
                assert_eq!(op, b"=");
                assert!(
                    matches!(asn.left, Node::ArrayPattern(_)),
                    "left is ArrayPattern, got {:?}",
                    asn.left.kind()
                );
            }
            other => panic!("expected AssignmentExpression, got {:?}", other.kind()),
        }
    }

    #[test]
    fn array_destructure_with_rest() {
        // `[a, ...b] = c` → ArrayPattern contains RestElement.
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let expr = parse_expr_ok(&gc, &mut sm, b"[a, ...b] = c;");
        if let Node::AssignmentExpression(asn) = expr {
            if let Node::ArrayPattern(ap) = asn.left {
                let elems: Vec<_> = ap.elements.iter().collect();
                assert_eq!(elems.len(), 2);
                assert!(matches!(elems[0], Node::Identifier(_)));
                assert!(matches!(elems[1], Node::RestElement(_)));
            } else {
                panic!("left must be ArrayPattern");
            }
        } else {
            panic!("expected AssignmentExpression");
        }
    }

    #[test]
    fn array_destructure_with_hole() {
        // `[a, , b] = c` → ArrayPattern has Empty hole.
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let expr = parse_expr_ok(&gc, &mut sm, b"[a, , b] = c;");
        if let Node::AssignmentExpression(asn) = expr {
            if let Node::ArrayPattern(ap) = asn.left {
                let elems: Vec<_> = ap.elements.iter().collect();
                assert_eq!(elems.len(), 3);
                assert!(matches!(elems[0], Node::Identifier(_)));
                assert!(matches!(elems[1], Node::Empty(_)));
                assert!(matches!(elems[2], Node::Identifier(_)));
            } else {
                panic!("left must be ArrayPattern");
            }
        } else {
            panic!("expected AssignmentExpression");
        }
    }

    #[test]
    fn array_destructure_with_default() {
        // `[a = 1, b] = c` → first element is AssignmentPattern.
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let expr = parse_expr_ok(&gc, &mut sm, b"[a = 1, b] = c;");
        if let Node::AssignmentExpression(asn) = expr {
            if let Node::ArrayPattern(ap) = asn.left {
                let elems: Vec<_> = ap.elements.iter().collect();
                assert_eq!(elems.len(), 2);
                assert!(
                    matches!(elems[0], Node::AssignmentPattern(_)),
                    "first element is AssignmentPattern"
                );
                assert!(matches!(elems[1], Node::Identifier(_)));
            } else {
                panic!("left must be ArrayPattern");
            }
        } else {
            panic!("expected AssignmentExpression");
        }
    }

    #[test]
    fn object_destructure_shorthand() {
        // `({a} = b)` → AssignmentExpression(=, ObjectPattern([Property(...)]), ...)
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let expr = parse_expr_ok(&gc, &mut sm, b"({a} = b);");
        if let Node::AssignmentExpression(asn) = expr {
            let op = gc.ctx().atom_table.bytes(asn.operator.get());
            assert_eq!(op, b"=");
            assert!(
                matches!(asn.left, Node::ObjectPattern(_)),
                "left is ObjectPattern, got {:?}",
                asn.left.kind()
            );
        } else {
            panic!("expected AssignmentExpression");
        }
    }

    #[test]
    fn object_destructure_cover_initializer() {
        // `({a = 1} = b)` → Property value is AssignmentPattern.
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let expr = parse_expr_ok(&gc, &mut sm, b"({a = 1} = b);");
        if let Node::AssignmentExpression(asn) = expr {
            if let Node::ObjectPattern(op) = asn.left {
                let props: Vec<_> = op.properties.iter().collect();
                assert_eq!(props.len(), 1);
                if let Node::Property(p) = props[0] {
                    assert!(
                        matches!(p.value, Node::AssignmentPattern(_)),
                        "property value is AssignmentPattern"
                    );
                } else {
                    panic!("expected Property");
                }
            } else {
                panic!("left must be ObjectPattern");
            }
        } else {
            panic!("expected AssignmentExpression");
        }
    }

    #[test]
    fn object_destructure_with_rest() {
        // `({...r} = o)` → ObjectPattern([RestElement(Identifier(r))]).
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let expr = parse_expr_ok(&gc, &mut sm, b"({...r} = o);");
        if let Node::AssignmentExpression(asn) = expr {
            if let Node::ObjectPattern(op) = asn.left {
                let props: Vec<_> = op.properties.iter().collect();
                assert_eq!(props.len(), 1);
                assert!(
                    matches!(props[0], Node::RestElement(_)),
                    "property is RestElement"
                );
            } else {
                panic!("left must be ObjectPattern");
            }
        } else {
            panic!("expected AssignmentExpression");
        }
    }

    #[test]
    fn nested_array_object_destructure() {
        // `[{a}, [b]] = c` — nested pattern.
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let expr = parse_expr_ok(&gc, &mut sm, b"[{a}, [b]] = c;");
        if let Node::AssignmentExpression(asn) = expr {
            if let Node::ArrayPattern(ap) = asn.left {
                let elems: Vec<_> = ap.elements.iter().collect();
                assert_eq!(elems.len(), 2);
                assert!(matches!(elems[0], Node::ObjectPattern(_)));
                assert!(matches!(elems[1], Node::ArrayPattern(_)));
            } else {
                panic!("left must be ArrayPattern");
            }
        } else {
            panic!("expected AssignmentExpression");
        }
    }
}
