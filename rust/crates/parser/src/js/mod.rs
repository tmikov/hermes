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

mod classes;
mod expressions;
mod flow;
mod functions;
mod modules;
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

/// RAII guard for a `paramYield_`/`paramAwait_` flag, restoring the saved old
/// value on Drop. Mirrors C++ `llvh::SaveAndRestore<bool>` on those fields,
/// which flip the flag for a scope (name-binding, or args+body) and restore it
/// on EVERY exit path — including error early-returns. A manual save-local +
/// restore-at-end would leak the new value on `?` early-returns, so the guard
/// must own the restore.
///
/// Same `Rc<Cell<bool>>` design as `RecursionGuard`: the flag lives in an
/// `Rc<Cell<bool>>` on the parser, and the guard owns its own `Rc` clone plus
/// the saved old value, so the caller can freely use `&mut self` while the
/// guard is alive.
pub(super) struct ParamFlagGuard {
    cell: Rc<Cell<bool>>,
    old: bool,
}

impl Drop for ParamFlagGuard {
    fn drop(&mut self) {
        self.cell.set(self.old);
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
    /// Set when the parser is inside a generator function (`yield`). In an
    /// `Rc<Cell<bool>>` so `ParamFlagGuard` can own a handle without borrowing
    /// `self`, restoring the saved value on every exit path (mirrors the C++
    /// `llvh::SaveAndRestore<bool>` on `paramYield_`).
    pub(super) param_yield: Rc<Cell<bool>>,
    /// Set when the parser is inside an async function (`await`).
    /// Read in P1.3+ (await expression parsing in parseUnaryExpression).
    /// In an `Rc<Cell<bool>>` — see `param_yield`.
    pub(super) param_await: Rc<Cell<bool>>,
    /// Set on the `use static builtin` directive.
    pub(super) use_static_builtin: bool,
    /// Whether an anonymous function type (`T => U` without parentheses) is
    /// allowed in the current type-annotation context. Port of the C++
    /// `allowAnonFunctionType_` field (JSParserImpl.h:255).
    /// In an `Rc<Cell<bool>>` — see `param_yield`.
    pub(super) allow_anon_function_type: Rc<Cell<bool>>,
    /// Whether a conditional type (`T extends U ? V : W`) not wrapped in
    /// parentheses is allowed. Port of the C++ `allowConditionalType_` field
    /// (JSParserImpl.h:259). In an `Rc<Cell<bool>>` — see `param_yield`.
    pub(super) allow_conditional_type: Rc<Cell<bool>>,
}

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    /// Construct the parser and lex the first token (C++ ctor does
    /// `tok_ = lexer_.advance()`).
    pub fn new(gc: &'gc GCLock<'ast, 'ctx>, mut lexer: JSLexer<'a>) -> Self {
        // Initialize the lexer's strict mode from the context, mirroring the
        // C++ JSParserImpl constructor which passes `context.isStrictMode()` to
        // the JSLexer constructor. The JSLexer's own default is strict=true, but
        // a default parse (script, no "use strict") must start in sloppy mode so
        // that e.g. `let;` lexes/parses as a loose-mode identifier expression.
        lexer.set_strict_mode(gc.ctx().strict_mode());
        lexer.advance(GrammarContext::AllowRegExp);
        JSParserImpl {
            gc,
            lexer,
            recursion_depth: Rc::new(Cell::new(0)),
            param_yield: Rc::new(Cell::new(false)),
            param_await: Rc::new(Cell::new(false)),
            use_static_builtin: false,
            allow_anon_function_type: Rc::new(Cell::new(false)),
            allow_conditional_type: Rc::new(Cell::new(false)),
        }
    }

    /// True if the parser detected `use static builtin`.
    pub fn get_use_static_builtin(&self) -> bool {
        self.use_static_builtin
    }

    /// True if Flow type parsing is enabled. Shorthand for the C++
    /// `context_.getParseFlow()` calls throughout the parser.
    pub(super) fn parse_flow(&self) -> bool {
        self.gc.ctx().parse_flow()
    }

    /// True if the Flow ambiguous-expression grammar is enabled. Shorthand for
    /// the C++ `context_.getParseFlowAmbiguous()`.
    pub(super) fn parse_flow_ambiguous(&self) -> bool {
        self.gc.ctx().parse_flow_ambiguous()
    }

    /// True if Flow `component`/`hook` syntax is enabled. Shorthand for the C++
    /// `context_.getParseFlowComponentSyntax()`.
    // P6.3: first consumer is the component/hook grammar.
    #[allow(dead_code)]
    pub(super) fn parse_flow_component_syntax(&self) -> bool {
        self.gc.ctx().parse_flow_component_syntax()
    }

    /// True if Flow `record` declarations/expressions are enabled. Shorthand for
    /// the C++ `context_.getParseFlowRecords()`.
    // P6.4: first consumer is the record grammar.
    #[allow(dead_code)]
    pub(super) fn parse_flow_records(&self) -> bool {
        self.gc.ctx().parse_flow_records()
    }

    /// True if Flow `match` expressions/statements are enabled. Shorthand for
    /// the C++ `context_.getParseFlowMatch()`.
    // P6.5: first consumer is the match grammar.
    #[allow(dead_code)]
    pub(super) fn parse_flow_match(&self) -> bool {
        self.gc.ctx().parse_flow_match()
    }

    /// True if TypeScript parsing is enabled. Shorthand for the C++
    /// `context_.getParseTS()`. Used by `parse_types()`; always false until
    /// TypeScript parsing lands (P7).
    pub(super) fn parse_ts(&self) -> bool {
        false // P7: TypeScript parsing.
    }

    /// True if any type-annotation dialect is enabled. Port of the C++
    /// `context_.getParseTypes()` (Context.h:504-506).
    pub(super) fn parse_types(&self) -> bool {
        self.parse_flow() || self.parse_ts()
    }

    /// Parse a type annotation in whichever type dialect is enabled. Port of
    /// the `parseTypeAnnotation` dispatcher (JSParserImpl.h:1209-1222), which
    /// calls the Flow version under `getParseFlow()` and otherwise falls
    /// through to TS. The TS branch (`parseTypeAnnotationTS`) is P7; only the
    /// Flow dispatch exists, so `parse_flow()` must be set.
    pub(in crate::js) fn parse_type_annotation(
        &mut self,
        wrapped_start: Option<SMLoc>,
        allow_anon_function_type: flow::AllowAnonFunctionType,
    ) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.parse_flow() || self.parse_ts());
        // P7: TS dispatch (parseTypeAnnotationTS, JSParserImpl.h:1218-1219).
        self.parse_type_annotation_flow(wrapped_start, allow_anon_function_type)
    }

    /// Parse a function return type annotation (a type, or a Flow type
    /// predicate such as `x is T`) in whichever type dialect is enabled. Port
    /// of the `parseReturnTypeAnnotation` dispatcher
    /// (JSParserImpl.h:1224-1237). The TS branch is P7; only the Flow dispatch
    /// exists, so `parse_flow()` must be set.
    pub(in crate::js) fn parse_return_type_annotation(
        &mut self,
        wrapped_start: Option<SMLoc>,
        allow_anon_function_type: flow::AllowAnonFunctionType,
    ) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.parse_flow() || self.parse_ts());
        // P7: TS dispatch (parseTypeAnnotationTS, JSParserImpl.h:1233-1234).
        self.parse_return_type_annotation_flow(
            wrapped_start,
            allow_anon_function_type,
        )
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
    pub(super) fn error_at(&mut self, range: SMRange, msg: &str) {
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
    /// Report an error at a point location, with no highlighted range. Port of
    /// the C++ `error(SMLoc, Twine)` overload (used e.g. by the Flow object
    /// type "Explicit inexact syntax" and 'implies' predicate errors).
    pub(super) fn error_at_loc(&mut self, loc: SMLoc, msg: &str) {
        self.lexer.get_source_mgr_mut().error_at(
            loc,
            None,
            msg,
            support::diag::Subsystem::Parser,
        );
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
    /// Report a "'k1' or 'k2' expected{where_}" error at the current token.
    /// Port of the two-token `errorExpected(k1, k2, where, what, whatLoc)`
    /// convenience wrapper (JSParserImpl.h:455) which forwards to
    /// `errorExpected(ArrayRef<TokenKind>(toks, 2), ...)`. The list-rendering
    /// logic in C++ `errorExpected` (175-195) joins two tokens with " or " and
    /// appends " expected". The `what`/`whatLoc` note args are dropped per house
    /// style (see other `errorExpected` call sites in this port).
    pub(super) fn error_expected2(
        &mut self,
        k1: TokenKind,
        k2: TokenKind,
        where_: &str,
    ) {
        let msg = format!(
            "'{}' or '{}' expected{}",
            crate::token_kinds::token_kind_str(k1),
            crate::token_kinds::token_kind_str(k2),
            where_
        );
        self.error_cur(&msg);
    }

    /// Report a "'k1', 'k2' or 'k3' expected{where_}" error at the current
    /// token. Port of the three-token `errorExpected` initializer-list call
    /// (e.g. the export-type dispatch at JSParserImpl-flow.cpp:2569-2574); the
    /// C++ list rendering joins all but the last token with ", " and the last
    /// with " or ". The `what`/`whatLoc` note args are dropped per house style.
    pub(super) fn error_expected3(
        &mut self,
        k1: TokenKind,
        k2: TokenKind,
        k3: TokenKind,
        where_: &str,
    ) {
        let msg = format!(
            "'{}', '{}' or '{}' expected{}",
            crate::token_kinds::token_kind_str(k1),
            crate::token_kinds::token_kind_str(k2),
            crate::token_kinds::token_kind_str(k3),
            where_
        );
        self.error_cur(&msg);
    }

    /// Report a "'k1', 'k2', 'k3' or 'k4' expected{where_}" error at the
    /// current token. Port of the four-token `errorExpected` initializer-list
    /// call (e.g. the Flow object-type property separator at
    /// JSParserImpl-flow.cpp:4138-4145); the C++ list rendering joins all but
    /// the last token with ", " and the last with " or ". The `what`/`whatLoc`
    /// note args are dropped per house style.
    pub(super) fn error_expected4(
        &mut self,
        k1: TokenKind,
        k2: TokenKind,
        k3: TokenKind,
        k4: TokenKind,
        where_: &str,
    ) {
        let msg = format!(
            "'{}', '{}', '{}' or '{}' expected{}",
            crate::token_kinds::token_kind_str(k1),
            crate::token_kinds::token_kind_str(k2),
            crate::token_kinds::token_kind_str(k3),
            crate::token_kinds::token_kind_str(k4),
            where_
        );
        self.error_cur(&msg);
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

    /// The raw source bytes of the absolute source range `[start, end)`.
    /// Shared by the call sites that reproduce the C++
    /// `StringRef(start.getPointer(), end - start)` raw-slice idiom (directive
    /// raws, literal-type raws).
    pub(super) fn source_bytes(&self, start: SMLoc, end: SMLoc) -> &[u8] {
        let buf_start = self.lexer.get_buffer_start();
        let buf = self.lexer.buffer_bytes();
        &buf[(start.offset - buf_start) as usize
            ..(end.offset - buf_start) as usize]
    }

    /// Intern the raw source text of the absolute source range `[start, end)`.
    /// The Rust equivalent of the C++
    /// `lexer_.getStringLiteral(StringRef(start, end - start))` idiom used for
    /// the raw spelling of literal type annotations.
    pub(super) fn source_bytes_atom(
        &self,
        start: SMLoc,
        end: SMLoc,
    ) -> atom_table::AtomBytes {
        self.lexer.get_string_literal(self.source_bytes(start, end))
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

    /// Set `param_yield` to `new_val`, returning a guard that restores the old
    /// value on Drop. Port of `llvh::SaveAndRestore<bool>(paramYield_, new)`.
    pub(super) fn save_param_yield(&self, new_val: bool) -> ParamFlagGuard {
        let old = self.param_yield.get();
        self.param_yield.set(new_val);
        ParamFlagGuard {
            cell: Rc::clone(&self.param_yield),
            old,
        }
    }

    /// Set `param_await` to `new_val`, returning a guard that restores the old
    /// value on Drop. Port of `llvh::SaveAndRestore<bool>(paramAwait_, new)`.
    pub(super) fn save_param_await(&self, new_val: bool) -> ParamFlagGuard {
        let old = self.param_await.get();
        self.param_await.set(new_val);
        ParamFlagGuard {
            cell: Rc::clone(&self.param_await),
            old,
        }
    }

    /// Set `allow_anon_function_type` to `new_val`, returning a guard that
    /// restores the old value on Drop. Port of the
    /// `llvh::SaveAndRestore<bool>(allowAnonFunctionType_, new)` in
    /// `parseTypeAnnotationFlow` (JSParserImpl-flow.cpp:3080-3082).
    pub(super) fn save_allow_anon_function_type(
        &self,
        new_val: bool,
    ) -> ParamFlagGuard {
        let old = self.allow_anon_function_type.get();
        self.allow_anon_function_type.set(new_val);
        ParamFlagGuard {
            cell: Rc::clone(&self.allow_anon_function_type),
            old,
        }
    }

    /// Set `allow_conditional_type` to `new_val`, returning a guard that
    /// restores the old value on Drop. Port of the
    /// `llvh::SaveAndRestore<bool>(allowConditionalType_, ...)` uses in the
    /// Flow type grammar (e.g. JSParserImpl-flow.cpp:3098).
    pub(super) fn save_allow_conditional_type(
        &self,
        new_val: bool,
    ) -> ParamFlagGuard {
        let old = self.allow_conditional_type.get();
        self.allow_conditional_type.set(new_val);
        ParamFlagGuard {
            cell: Rc::clone(&self.allow_conditional_type),
            old,
        }
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

    /// Return an *invalid* `SMRange` (mirrors a C++ default-constructed
    /// `SMRange()`, whose `isValid()` is false). Used for nodes that C++ builds
    /// without ever calling `setLocation` — e.g. the fresh `RestElement` created
    /// from a `SpreadElement` in the async-arrow reparse path. The dumper's
    /// `range_is_valid` treats `start.offset > end.offset` as invalid, so loc and
    /// range are omitted, exactly as in the C++ dump.
    ///
    /// Contrast [`Self::dummy_range`], a *valid* zero-width placeholder that is
    /// expected to be overwritten by a subsequent `set_location` call.
    pub(super) fn invalid_range(&self) -> SMRange {
        let loc = self.cur_start();
        SMRange {
            start: SMLoc {
                source: loc.source,
                offset: 1,
            },
            end: SMLoc {
                source: loc.source,
                offset: 0,
            },
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
            [TokenKind::eof],
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

    /// `if(x);` parses cleanly as of P2.4 (was a deferred-error test in P1.1).
    #[test]
    fn if_statement_parses() {
        use ast::context::Context;
        use ast::node::Node;
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
        let program = parser.parse().expect("if statement parses in P2.4");
        assert_eq!(parser.error_count_pub(), 0, "zero errors");
        let Node::Program(p) = program else {
            panic!("expected Program")
        };
        let stmt = p.body.iter().next().expect("one statement");
        assert!(
            matches!(stmt, Node::IfStatement(_)),
            "expected IfStatement, got {:?}",
            stmt.kind()
        );
    }

    /// P3.1: a function expression now parses as a FunctionExpression.
    #[test]
    fn function_expression_parses() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;

        let expr = parse_expr_from(&gc, &mut sm, atoms, b"(function(){});");
        assert!(
            matches!(expr, Node::FunctionExpression(_)),
            "expected FunctionExpression, got {:?}",
            expr.kind()
        );
    }

    /// Helper: parse `src` and assert it fails with at least one error
    /// (used for the still-deferred declaration forms).
    fn assert_parse_errors(src: &[u8], why: &str) {
        use ast::context::Context;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", src);
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
        assert!(parser.parse().is_none(), "{why}");
        assert!(parser.error_count_pub() >= 1, "{why}: expected an error");
    }

    /// Shared body of [`assert_parse_has_errors`] /
    /// [`assert_flow_parse_has_errors`]: parse `src` (with Flow parsing
    /// enabled iff `parse_flow`) and assert at least one error was reported —
    /// the parse may still recover and return a `Program`.
    fn assert_parse_has_errors_impl(src: &[u8], why: &str, parse_flow: bool) {
        use ast::context::Context;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", src);
        let mut ctx = Context::new();
        ctx.set_parse_flow(parse_flow);
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        let lexer = crate::lexer::JSLexer::new(
            buf_id,
            &mut sm,
            atoms,
            crate::lexer::GrammarContext::AllowRegExp,
        );
        let mut parser = JSParserImpl::new(&gc, lexer);
        let _ = parser.parse();
        assert!(parser.error_count_pub() >= 1, "{why}: expected an error");
    }

    /// Like [`assert_parse_errors`], but only requires that at least one error
    /// was reported — the parse may still recover and return a `Program`. Used
    /// for diagnostics that C++ reports but continues past (e.g. a duplicate
    /// named import, or an `import` nested in a block).
    fn assert_parse_has_errors(src: &[u8], why: &str) {
        assert_parse_has_errors_impl(src, why, false);
    }

    /// P2 capstone: top-level declaration forms that route into
    /// `parseDeclaration`/`parseStatementListItem` must emit an HONEST deferral
    /// error (not a silent misparse). Functions/classes are P3; import/export
    /// are P4.
    // P3.1: function declarations/expressions, params, body.

    /// Helper: parse `src`, expect zero errors, return the first top-level
    /// statement. Shorthand for [`flow_parse_stmt_at`] with index 0.
    fn parse_one_stmt<'gc>(
        gc: &'gc ast::context::GCLock<'_, '_>,
        sm: &mut support::manager::SourceErrorManager,
        src: &[u8],
    ) -> &'gc ast::node::Node<'gc> {
        flow_parse_stmt_at(gc, sm, src, 0)
    }

    #[test]
    fn function_declaration_parses() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"function f(){}");
        assert!(
            matches!(stmt, Node::FunctionDeclaration(_)),
            "expected FunctionDeclaration, got {:?}",
            stmt.kind()
        );
    }

    #[test]
    fn generator_declaration_has_generator_flag() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"function* h(){}");
        if let Node::FunctionDeclaration(fd) = stmt {
            assert!(fd.generator.get(), "generator flag is true");
            assert!(!fd.r#async.get(), "async flag is false");
        } else {
            panic!("expected FunctionDeclaration");
        }
    }

    #[test]
    fn async_declaration_has_async_flag() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"async function k(){}");
        if let Node::FunctionDeclaration(fd) = stmt {
            assert!(fd.r#async.get(), "async flag is true");
            assert!(!fd.generator.get(), "generator flag is false");
        } else {
            panic!("expected FunctionDeclaration");
        }
    }

    #[test]
    fn function_params_identifier_and_rest() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"function f(a, ...r){}");
        if let Node::FunctionDeclaration(fd) = stmt {
            let params: Vec<_> = fd.params.iter().collect();
            assert_eq!(params.len(), 2);
            assert!(
                matches!(params[0], Node::Identifier(_)),
                "first param is Identifier"
            );
            assert!(
                matches!(params[1], Node::RestElement(_)),
                "second param is RestElement"
            );
        } else {
            panic!("expected FunctionDeclaration");
        }
    }

    #[test]
    fn function_params_object_and_array_patterns() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"function g({x},[y]){}");
        if let Node::FunctionDeclaration(fd) = stmt {
            let params: Vec<_> = fd.params.iter().collect();
            assert_eq!(params.len(), 2);
            assert!(
                matches!(params[0], Node::ObjectPattern(_)),
                "first param is ObjectPattern"
            );
            assert!(
                matches!(params[1], Node::ArrayPattern(_)),
                "second param is ArrayPattern"
            );
        } else {
            panic!("expected FunctionDeclaration");
        }
    }

    /// `await` was implemented in P1.3 but only reachable inside an async
    /// function body now that function bodies parse (P3.1).
    #[test]
    fn await_in_async_body_parses() {
        let mut sm = support::manager::SourceErrorManager::new();
        assert!(
            parse_snippet(&mut sm, b"async function f(){ await x; }"),
            "await in async body must parse cleanly"
        );
    }

    /// P3.2: a generator body containing `yield` now parses cleanly.
    #[test]
    fn yield_in_generator_parses() {
        let mut sm = support::manager::SourceErrorManager::new();
        assert!(
            parse_snippet(&mut sm, b"function* g(){ yield 1; }"),
            "yield in generator body must parse cleanly"
        );
    }

    // ----- P3.6: classes + decorators -----

    /// `class A extends B {}` -> ClassDeclaration whose superClass is the
    /// identifier `B`.
    #[test]
    fn class_declaration_with_heritage() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"class A extends B {}");
        let Node::ClassDeclaration(cd) = stmt else {
            panic!("expected ClassDeclaration, got {:?}", stmt.kind());
        };
        let sup = cd.super_class.expect("superClass present");
        match sup {
            Node::Identifier(id) => {
                let bytes = gc.ctx().atom_table.bytes(id.name.get());
                assert_eq!(bytes, b"B");
            }
            other => panic!("expected Identifier superClass, got {:?}", other.kind()),
        }
    }

    /// Helper: parse `class A { <member> }` and return the single class-body
    /// element.
    fn parse_one_class_member<'gc>(
        gc: &'gc ast::context::GCLock<'_, '_>,
        sm: &mut support::manager::SourceErrorManager,
        member_src: &str,
    ) -> &'gc ast::node::Node<'gc> {
        use ast::node::Node;
        let src = format!("class A {{ {member_src} }}");
        let stmt = parse_one_stmt(gc, sm, src.as_bytes());
        let Node::ClassDeclaration(cd) = stmt else {
            panic!("expected ClassDeclaration, got {:?}", stmt.kind());
        };
        let Node::ClassBody(cb) = cd.body else {
            panic!("expected ClassBody");
        };
        cb.body.iter().next().expect("one class member")
    }

    /// `m(){}` -> MethodDefinition with kind "method".
    #[test]
    fn class_method_kind_method() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let member = parse_one_class_member(&gc, &mut sm, "m(){}");
        let Node::MethodDefinition(md) = member else {
            panic!("expected MethodDefinition, got {:?}", member.kind());
        };
        let kind = gc.ctx().atom_table.bytes(md.kind.get());
        assert_eq!(kind, b"method");
        assert!(!md.r#static.get(), "not static");
    }

    /// `constructor(){}` -> MethodDefinition with kind "constructor".
    #[test]
    fn class_method_kind_constructor() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let member = parse_one_class_member(&gc, &mut sm, "constructor(){}");
        let Node::MethodDefinition(md) = member else {
            panic!("expected MethodDefinition, got {:?}", member.kind());
        };
        let kind = gc.ctx().atom_table.bytes(md.kind.get());
        assert_eq!(kind, b"constructor");
    }

    /// `get x(){}` -> MethodDefinition with kind "get".
    #[test]
    fn class_method_kind_get() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let member = parse_one_class_member(&gc, &mut sm, "get x(){}");
        let Node::MethodDefinition(md) = member else {
            panic!("expected MethodDefinition, got {:?}", member.kind());
        };
        let kind = gc.ctx().atom_table.bytes(md.kind.get());
        assert_eq!(kind, b"get");
    }

    /// `static s(){}` -> static MethodDefinition.
    #[test]
    fn class_method_static() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let member = parse_one_class_member(&gc, &mut sm, "static s(){}");
        let Node::MethodDefinition(md) = member else {
            panic!("expected MethodDefinition, got {:?}", member.kind());
        };
        assert!(md.r#static.get(), "static flag set");
    }

    /// `#p(){}` -> MethodDefinition whose key is a PrivateName.
    #[test]
    fn class_private_method_key_is_private_name() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let member = parse_one_class_member(&gc, &mut sm, "#p(){}");
        let Node::MethodDefinition(md) = member else {
            panic!("expected MethodDefinition, got {:?}", member.kind());
        };
        assert!(
            matches!(md.key, Node::PrivateName(_)),
            "method key is PrivateName, got {:?}",
            md.key.kind()
        );
    }

    /// `x = 1;` -> ClassProperty with a value.
    #[test]
    fn class_field_with_value() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let member = parse_one_class_member(&gc, &mut sm, "x = 1;");
        let Node::ClassProperty(cp) = member else {
            panic!("expected ClassProperty, got {:?}", member.kind());
        };
        assert!(cp.value.is_some(), "field has a value");
    }

    /// `#f;` -> ClassPrivateProperty.
    #[test]
    fn class_private_field() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let member = parse_one_class_member(&gc, &mut sm, "#f;");
        assert!(
            matches!(member, Node::ClassPrivateProperty(_)),
            "expected ClassPrivateProperty, got {:?}",
            member.kind()
        );
    }

    /// `static { }` -> StaticBlock.
    #[test]
    fn class_static_block() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let member = parse_one_class_member(&gc, &mut sm, "static { }");
        assert!(
            matches!(member, Node::StaticBlock(_)),
            "expected StaticBlock, got {:?}",
            member.kind()
        );
    }

    /// `const C = class {};` -> ClassExpression.
    #[test]
    fn class_expression_parses() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        let expr = parse_expr_from(&gc, &mut sm, atoms, b"(class {});");
        assert!(
            matches!(expr, Node::ClassExpression(_)),
            "expected ClassExpression, got {:?}",
            expr.kind()
        );
    }

    /// A decorated class declaration: `@dec class A {}` -> ClassDeclaration with
    /// a single Decorator.
    #[test]
    fn class_declaration_with_decorator() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"@dec class A {}");
        let Node::ClassDeclaration(cd) = stmt else {
            panic!("expected ClassDeclaration, got {:?}", stmt.kind());
        };
        let decorators: Vec<_> = cd.decorators.iter().collect();
        assert_eq!(decorators.len(), 1, "one decorator");
        assert!(
            matches!(decorators[0], Node::Decorator(_)),
            "expected Decorator node"
        );
    }

    /// The class body is always strict mode, but that strictness must NOT leak
    /// into the enclosing (sloppy) code. After a class declaration, a `with`
    /// statement — which is illegal in strict mode — must still parse cleanly.
    #[test]
    fn class_strict_mode_does_not_leak() {
        use ast::context::Context;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"class A {}\nwith(x) y;");
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
            "with-statement after class must parse (sloppy mode restored)"
        );
        assert_eq!(
            parser.error_count_pub(),
            0,
            "no errors: class strict mode must not leak to enclosing sloppy code"
        );
    }

    // P4.2: import declarations are now implemented; see the `import_*` tests
    // further below. The `import x from 'm';` form parses cleanly.

    // P4.1: `import(...)` and `import.meta` expression forms.

    #[test]
    fn import_call_no_options() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;

        let expr = parse_expr_from(&gc, &mut sm, atoms, b"import('m');");
        if let Node::ImportExpression(ie) = expr {
            assert!(
                matches!(ie.source, Node::StringLiteral(_)),
                "source should be a StringLiteral, got {:?}",
                ie.source.kind()
            );
            assert!(ie.options.is_none(), "options should be None");
        } else {
            panic!("expected ImportExpression, got {:?}", expr.kind());
        }
    }

    #[test]
    fn import_call_with_options() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;

        let expr = parse_expr_from(&gc, &mut sm, atoms, b"import('m', {});");
        if let Node::ImportExpression(ie) = expr {
            assert!(
                matches!(ie.options, Some(Node::ObjectExpression(_))),
                "options should be Some(ObjectExpression), got {:?}",
                ie.options.map(|o| o.kind())
            );
        } else {
            panic!("expected ImportExpression, got {:?}", expr.kind());
        }
    }

    #[test]
    fn import_meta_property() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;

        let expr = parse_expr_from(&gc, &mut sm, atoms, b"import.meta;");
        if let Node::MetaProperty(mp) = expr {
            if let Node::Identifier(meta) = mp.meta {
                assert_eq!(
                    gc.ctx().atom_table.bytes(meta.name.get()),
                    b"import",
                    "meta identifier name should be `import`"
                );
            } else {
                panic!("meta should be an Identifier");
            }
            if let Node::Identifier(prop) = mp.property {
                assert_eq!(
                    gc.ctx().atom_table.bytes(prop.name.get()),
                    b"meta",
                    "property identifier name should be `meta`"
                );
            } else {
                panic!("property should be an Identifier");
            }
        } else {
            panic!("expected MetaProperty, got {:?}", expr.kind());
        }
    }

    #[test]
    fn import_meta_bad_form_errors() {
        assert_parse_errors(b"import.foo;", "'meta' expected after import.");
    }

    /// C++ uses `check(metaIdent_)` (escape-insensitive) for the `meta`
    /// keyword, so an escaped `meta` is still a valid `import.meta`
    /// MetaProperty — it must NOT trip the `'meta' expected` error path.
    #[test]
    fn import_meta_escaped_meta_parses() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;

        let expr =
            parse_expr_from(&gc, &mut sm, atoms, b"import.m\\u0065ta;");
        if let Node::MetaProperty(mp) = expr {
            if let Node::Identifier(prop) = mp.property {
                assert_eq!(
                    gc.ctx().atom_table.bytes(prop.name.get()),
                    b"meta",
                    "escaped `m\\u0065ta` should intern to `meta`"
                );
            } else {
                panic!("property should be an Identifier");
            }
        } else {
            panic!("expected MetaProperty, got {:?}", expr.kind());
        }
    }

    // P4.2: import declarations.

    /// Helper: the interned bytes of an `Identifier` node.
    fn ident_bytes<'gc>(
        gc: &'gc ast::context::GCLock<'_, '_>,
        node: &ast::node::Node<'gc>,
    ) -> Vec<u8> {
        if let ast::node::Node::Identifier(id) = node {
            gc.ctx().atom_table.bytes(id.name.get()).to_vec()
        } else {
            panic!("expected Identifier, got {:?}", node.kind());
        }
    }

    #[test]
    fn import_default_specifier_parses() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"import x from 'm';");
        if let Node::ImportDeclaration(decl) = stmt {
            assert_eq!(decl.specifiers.iter().count(), 1);
            let spec = decl.specifiers.iter().next().unwrap();
            if let Node::ImportDefaultSpecifier(ds) = spec {
                assert_eq!(ident_bytes(&gc, ds.local), b"x");
            } else {
                panic!("expected ImportDefaultSpecifier, got {:?}", spec.kind());
            }
            if let Node::StringLiteral(sl) = decl.source {
                assert_eq!(gc.ctx().atom_table.bytes(sl.value.get()), b"m");
            } else {
                panic!("source should be a StringLiteral");
            }
            assert_eq!(
                gc.ctx().atom_table.bytes(decl.import_kind.get()),
                b"value"
            );
        } else {
            panic!("expected ImportDeclaration, got {:?}", stmt.kind());
        }
    }

    #[test]
    fn import_named_specifier_parses() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"import {b as c} from 'm';");
        if let Node::ImportDeclaration(decl) = stmt {
            assert_eq!(decl.specifiers.iter().count(), 1);
            let spec = decl.specifiers.iter().next().unwrap();
            if let Node::ImportSpecifier(is) = spec {
                assert_eq!(ident_bytes(&gc, is.imported), b"b");
                assert_eq!(ident_bytes(&gc, is.local), b"c");
            } else {
                panic!("expected ImportSpecifier, got {:?}", spec.kind());
            }
        } else {
            panic!("expected ImportDeclaration, got {:?}", stmt.kind());
        }
    }

    #[test]
    fn import_namespace_specifier_parses() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"import * as ns from 'm';");
        if let Node::ImportDeclaration(decl) = stmt {
            assert_eq!(decl.specifiers.iter().count(), 1);
            let spec = decl.specifiers.iter().next().unwrap();
            if let Node::ImportNamespaceSpecifier(ns) = spec {
                assert_eq!(ident_bytes(&gc, ns.local), b"ns");
            } else {
                panic!(
                    "expected ImportNamespaceSpecifier, got {:?}",
                    spec.kind()
                );
            }
        } else {
            panic!("expected ImportDeclaration, got {:?}", stmt.kind());
        }
    }

    #[test]
    fn import_default_plus_named_parses() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt =
            parse_one_stmt(&gc, &mut sm, b"import d, {a, b} from 'm';");
        if let Node::ImportDeclaration(decl) = stmt {
            let specs: Vec<_> = decl.specifiers.iter().collect();
            assert_eq!(specs.len(), 3);
            assert!(matches!(specs[0], Node::ImportDefaultSpecifier(_)));
            assert!(matches!(specs[1], Node::ImportSpecifier(_)));
            assert!(matches!(specs[2], Node::ImportSpecifier(_)));
        } else {
            panic!("expected ImportDeclaration, got {:?}", stmt.kind());
        }
    }

    #[test]
    fn import_default_plus_namespace_parses() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt =
            parse_one_stmt(&gc, &mut sm, b"import d, * as ns from 'm';");
        if let Node::ImportDeclaration(decl) = stmt {
            let specs: Vec<_> = decl.specifiers.iter().collect();
            assert_eq!(specs.len(), 2);
            assert!(matches!(specs[0], Node::ImportDefaultSpecifier(_)));
            assert!(matches!(specs[1], Node::ImportNamespaceSpecifier(_)));
        } else {
            panic!("expected ImportDeclaration, got {:?}", stmt.kind());
        }
    }

    #[test]
    fn import_bare_parses() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"import 'm';");
        if let Node::ImportDeclaration(decl) = stmt {
            assert_eq!(decl.specifiers.iter().count(), 0);
            assert_eq!(decl.attributes.iter().count(), 0);
            if let Node::StringLiteral(sl) = decl.source {
                assert_eq!(gc.ctx().atom_table.bytes(sl.value.get()), b"m");
            } else {
                panic!("source should be a StringLiteral");
            }
        } else {
            panic!("expected ImportDeclaration, got {:?}", stmt.kind());
        }
    }

    #[test]
    fn import_attribute_parses() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(
            &gc,
            &mut sm,
            b"import x from 'm' with { type: 'json' };",
        );
        if let Node::ImportDeclaration(decl) = stmt {
            assert_eq!(decl.attributes.iter().count(), 1);
            let attr = decl.attributes.iter().next().unwrap();
            if let Node::ImportAttribute(ia) = attr {
                assert_eq!(ident_bytes(&gc, ia.key), b"type");
                if let Node::StringLiteral(sl) = ia.value {
                    assert_eq!(
                        gc.ctx().atom_table.bytes(sl.value.get()),
                        b"json"
                    );
                } else {
                    panic!("attribute value should be a StringLiteral");
                }
            } else {
                panic!("expected ImportAttribute, got {:?}", attr.kind());
            }
        } else {
            panic!("expected ImportDeclaration, got {:?}", stmt.kind());
        }
    }

    #[test]
    fn import_duplicate_named_errors() {
        assert_parse_has_errors(
            b"import {a, a} from 'm';",
            "duplicate named import is a Duplicate entry error",
        );
    }

    #[test]
    fn import_in_block_errors() {
        // A `{ import ... }` block body reaches `parse_statement_list_item`
        // with `AllowImportExport::No`, triggering the top-level error.
        assert_parse_has_errors(
            b"{ import x from 'm'; }",
            "import inside a block must be at top level of module",
        );
    }

    // P4.3: export declarations.

    #[test]
    fn export_named_specifier_parses() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        // The `var a;` declaration and the `export` share one program.
        let buf_id = sm.add_buffer_bytes("input", b"var a;\nexport {a as b};");
        let atoms = &gc.ctx().atom_table;
        let lexer = crate::lexer::JSLexer::new(
            buf_id,
            &mut sm,
            atoms,
            crate::lexer::GrammarContext::AllowRegExp,
        );
        let mut parser = JSParserImpl::new(&gc, lexer);
        let program = parser.parse().expect("parse succeeded");
        assert_eq!(parser.error_count_pub(), 0, "zero errors");
        let Node::Program(p) = program else {
            panic!("expected Program")
        };
        let stmt = p.body.iter().nth(1).expect("has second statement");
        if let Node::ExportNamedDeclaration(decl) = stmt {
            assert!(decl.declaration.is_none(), "declaration None");
            assert!(decl.source.is_none(), "source None");
            assert_eq!(
                gc.ctx().atom_table.bytes(decl.export_kind.get()),
                b"value"
            );
            assert_eq!(decl.specifiers.iter().count(), 1);
            let spec = decl.specifiers.iter().next().unwrap();
            if let Node::ExportSpecifier(es) = spec {
                assert_eq!(ident_bytes(&gc, es.exported), b"b");
                assert_eq!(ident_bytes(&gc, es.local), b"a");
            } else {
                panic!("expected ExportSpecifier, got {:?}", spec.kind());
            }
        } else {
            panic!("expected ExportNamedDeclaration, got {:?}", stmt.kind());
        }
    }

    #[test]
    fn export_named_from_parses() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"export {a} from 'm';");
        if let Node::ExportNamedDeclaration(decl) = stmt {
            if let Some(Node::StringLiteral(sl)) = decl.source {
                assert_eq!(gc.ctx().atom_table.bytes(sl.value.get()), b"m");
            } else {
                panic!("source should be a StringLiteral");
            }
        } else {
            panic!("expected ExportNamedDeclaration, got {:?}", stmt.kind());
        }
    }

    #[test]
    fn export_all_parses() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"export * from 'm';");
        if let Node::ExportAllDeclaration(decl) = stmt {
            if let Node::StringLiteral(sl) = decl.source {
                assert_eq!(gc.ctx().atom_table.bytes(sl.value.get()), b"m");
            } else {
                panic!("source should be a StringLiteral");
            }
        } else {
            panic!("expected ExportAllDeclaration, got {:?}", stmt.kind());
        }
    }

    #[test]
    fn export_namespace_parses() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"export * as ns from 'm';");
        if let Node::ExportNamedDeclaration(decl) = stmt {
            assert_eq!(decl.specifiers.iter().count(), 1);
            let spec = decl.specifiers.iter().next().unwrap();
            if let Node::ExportNamespaceSpecifier(ns) = spec {
                assert_eq!(ident_bytes(&gc, ns.exported), b"ns");
            } else {
                panic!("expected ExportNamespaceSpecifier, got {:?}", spec.kind());
            }
        } else {
            panic!("expected ExportNamedDeclaration, got {:?}", stmt.kind());
        }
    }

    #[test]
    fn export_default_expr_parses() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"export default 1;");
        if let Node::ExportDefaultDeclaration(decl) = stmt {
            assert!(
                matches!(decl.declaration, Node::NumericLiteral(_)),
                "declaration should be a NumericLiteral, got {:?}",
                decl.declaration.kind()
            );
        } else {
            panic!("expected ExportDefaultDeclaration, got {:?}", stmt.kind());
        }
    }

    #[test]
    fn export_default_function_parses() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"export default function(){}");
        if let Node::ExportDefaultDeclaration(decl) = stmt {
            if let Node::FunctionDeclaration(fd) = decl.declaration {
                assert!(fd.id.is_none(), "default function has no id");
            } else {
                panic!(
                    "declaration should be a FunctionDeclaration, got {:?}",
                    decl.declaration.kind()
                );
            }
        } else {
            panic!("expected ExportDefaultDeclaration, got {:?}", stmt.kind());
        }
    }

    #[test]
    fn export_var_declaration_parses() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"export var x = 1;");
        if let Node::ExportNamedDeclaration(decl) = stmt {
            assert!(
                matches!(decl.declaration, Some(Node::VariableDeclaration(_))),
                "declaration should be a VariableDeclaration, got {:?}",
                decl.declaration.map(|d| d.kind())
            );
        } else {
            panic!("expected ExportNamedDeclaration, got {:?}", stmt.kind());
        }
    }

    #[test]
    fn export_function_declaration_parses() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"export function f(){}");
        if let Node::ExportNamedDeclaration(decl) = stmt {
            assert!(
                matches!(decl.declaration, Some(Node::FunctionDeclaration(_))),
                "declaration should be a FunctionDeclaration, got {:?}",
                decl.declaration.map(|d| d.kind())
            );
        } else {
            panic!("expected ExportNamedDeclaration, got {:?}", stmt.kind());
        }
    }

    #[test]
    fn export_in_block_errors() {
        // A `{ export ... }` block body reaches `parse_statement_list_item`
        // with `AllowImportExport::No`. Unlike import, export does NOT push the
        // declaration; it just reports the "must be at top level" error.
        assert_parse_has_errors(
            b"{ export var x = 1; }",
            "export inside a block must be at top level of module",
        );
    }

    // P5 capstone: Flow `export type` and export-kind detection
    // (C++ JSParserImpl.cpp:7133-7137, 7361-7368; flow.cpp:2498-2575).

    /// Helper: parse `src` with Flow enabled, expect one top-level
    /// `ExportNamedDeclaration`, and assert its `exportKind` atom is `kind`.
    fn assert_flow_export_kind(src: &[u8], kind: &[u8]) {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();
        let stmt = flow_parse_stmt_at(&gc, &mut sm, src, 0);
        let Node::ExportNamedDeclaration(decl) = stmt else {
            panic!("expected ExportNamedDeclaration, got {:?}", stmt.kind())
        };
        assert_eq!(
            gc.ctx().atom_table.bytes(decl.export_kind.get()),
            kind,
            "exportKind for {:?}",
            String::from_utf8_lossy(src)
        );
    }

    /// `export type A = ...;` routes through
    /// `parse_export_type_declaration_flow` (C++ 7133-7137 →
    /// flow.cpp:2557-2566) and gets exportKind `type`.
    #[test]
    fn flow_export_type_alias_kind_is_type() {
        assert_flow_export_kind(b"export type A = number;", b"type");
    }

    /// `export opaque type` goes through the `export <Declaration>` path; the
    /// kind detection (C++ 7361-7368) makes it `type`.
    #[test]
    fn flow_export_opaque_type_kind_is_type() {
        assert_flow_export_kind(b"export opaque type B = string;", b"type");
    }

    /// `export interface` goes through the `export <Declaration>` path; the
    /// kind detection (C++ 7361-7368) makes it `type`.
    #[test]
    fn flow_export_interface_kind_is_type() {
        assert_flow_export_kind(b"export interface I { x: number }", b"type");
    }

    /// Value declarations keep exportKind `value` even with Flow enabled.
    #[test]
    fn flow_export_value_kinds_stay_value() {
        assert_flow_export_kind(b"export var x = 1;", b"value");
        assert_flow_export_kind(b"export function f(){}", b"value");
    }

    /// Without `parse_flow`, `export type A = 1;` does not hit the Flow
    /// route: `type` is not a declaration start, so it errors exactly like
    /// hermesc without `-parse-flow` ("expected declaration in export").
    #[test]
    fn export_type_without_flow_errors() {
        assert_parse_has_errors(
            b"export type A = 1;",
            "export type without Flow is not a declaration",
        );
    }

    /// The `export type {…}` / `export type *` specifier/re-export forms of
    /// parseExportTypeDeclarationFlow (flow.cpp:2503-2556) are P6; they must
    /// report an honest deferral error instead of silently mis-parsing.
    #[test]
    fn flow_export_type_clause_and_star_are_honest_p6_errors() {
        assert_flow_parse_has_errors(
            b"export type {x};",
            "export type {…} is P6 and must error honestly",
        );
        assert_flow_parse_has_errors(
            b"export type * from 'm';",
            "export type * is P6 and must error honestly",
        );
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
    fn arrow_expr_parses_after_p33() {
        // Arrow functions landed in P3.3; `a => b` now parses cleanly.
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
        assert!(parser.parse().is_some(), "arrow should parse in P3.3");
        assert_eq!(parser.error_count_pub(), 0, "no errors");
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

    // P3.4: object method / getter / setter tests.

    /// Helper: parse `(OBJECT);`, expect success, return the single Property of
    /// the contained ObjectExpression.
    fn parse_single_property<'gc>(
        gc: &'gc ast::context::GCLock<'_, '_>,
        sm: &mut support::manager::SourceErrorManager,
        src: &[u8],
    ) -> &'gc ast::node::Property<'gc> {
        use ast::node::Node;
        let expr = parse_expr_ok(gc, sm, src);
        let Node::ObjectExpression(obj) = expr else {
            panic!("expected ObjectExpression, got {:?}", expr.kind());
        };
        let props: Vec<_> = obj.properties.iter().collect();
        assert_eq!(props.len(), 1, "expected exactly one property");
        match props[0] {
            Node::Property(p) => p,
            other => panic!("expected Property, got {:?}", other.kind()),
        }
    }

    #[test]
    fn object_getter_parses() {
        // `{get x() { return 1; }}` → Property kind "get", value FunctionExpression.
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let p = parse_single_property(&gc, &mut sm, b"({get x() { return 1; }});");
        assert_eq!(gc.ctx().atom_table.bytes(p.kind.get()), b"get");
        assert!(!p.method.get(), "getter is not a method");
        assert!(!p.computed.get());
        let Node::FunctionExpression(f) = p.value else {
            panic!("getter value must be FunctionExpression");
        };
        assert_eq!(f.params.iter().count(), 0, "getter has no params");
        assert!(!f.generator.get());
        assert!(!f.r#async.get());
    }

    #[test]
    fn object_setter_parses() {
        // `{set x(v) {}}` → Property kind "set", value FunctionExpression w/ 1 param.
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let p = parse_single_property(&gc, &mut sm, b"({set x(v) {}});");
        assert_eq!(gc.ctx().atom_table.bytes(p.kind.get()), b"set");
        assert!(!p.method.get(), "setter is not a method");
        let Node::FunctionExpression(f) = p.value else {
            panic!("setter value must be FunctionExpression");
        };
        assert_eq!(f.params.iter().count(), 1, "setter has one param");
    }

    #[test]
    fn object_method_parses() {
        // `{m() {}}` → Property kind "init", method=true, value FunctionExpression.
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let p = parse_single_property(&gc, &mut sm, b"({m() {}});");
        assert_eq!(gc.ctx().atom_table.bytes(p.kind.get()), b"init");
        assert!(p.method.get(), "plain method has method=true");
        assert!(!p.shorthand.get());
        let Node::FunctionExpression(f) = p.value else {
            panic!("method value must be FunctionExpression");
        };
        assert!(!f.generator.get());
        assert!(!f.r#async.get());
    }

    #[test]
    fn object_generator_method_parses() {
        // `{*g() {}}` → method=true, value FunctionExpression.generator==true.
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let p = parse_single_property(&gc, &mut sm, b"({*g() {}});");
        assert!(p.method.get());
        let Node::FunctionExpression(f) = p.value else {
            panic!("generator method value must be FunctionExpression");
        };
        assert!(f.generator.get(), "generator==true");
        assert!(!f.r#async.get());
    }

    #[test]
    fn object_async_method_parses() {
        // `{async a() {}}` → method=true, value FunctionExpression.async==true.
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let p = parse_single_property(&gc, &mut sm, b"({async a() {}});");
        assert!(p.method.get());
        let Node::FunctionExpression(f) = p.value else {
            panic!("async method value must be FunctionExpression");
        };
        assert!(f.r#async.get(), "async==true");
        assert!(!f.generator.get());
    }

    #[test]
    fn object_async_generator_method_parses() {
        // `{async *ag() {}}` → both async and generator true.
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let p = parse_single_property(&gc, &mut sm, b"({async *ag() {}});");
        assert!(p.method.get());
        let Node::FunctionExpression(f) = p.value else {
            panic!("async generator method value must be FunctionExpression");
        };
        assert!(f.r#async.get(), "async==true");
        assert!(f.generator.get(), "generator==true");
    }

    #[test]
    fn object_computed_method_parses() {
        // `{[k]() {}}` → computed=true, method=true.
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let p = parse_single_property(&gc, &mut sm, b"({[k]() {}});");
        assert!(p.computed.get(), "computed key");
        assert!(p.method.get());
        assert!(matches!(p.value, Node::FunctionExpression(_)));
    }

    #[test]
    fn object_string_and_numeric_methods_parse() {
        // `{'s'() {}, 0() {}}` → both methods parse with method=true.
        let mut sm = support::manager::SourceErrorManager::new();
        assert!(
            parse_snippet(&mut sm, b"({'s'() {}, 0() {}});"),
            "string- and numeric-keyed methods"
        );
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

    // P2.1: simple statements + labelled statements.

    /// Top-level `return x;` is an illegal location for `return` (not in a
    /// function), so it reports the "'return' not in a function" error, but
    /// the parser still keeps parsing and produces a valid Program.
    #[test]
    fn return_outside_function_reports_error_but_parses() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"return x;\n");
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
        let program = parser.parse().expect("return still parses");
        assert!(
            parser.error_count_pub() >= 1,
            "top-level return reports an error"
        );
        if let Node::Program(p) = program {
            let stmt = p.body.iter().next().expect("has statement");
            assert!(
                matches!(stmt, Node::ReturnStatement(_)),
                "still produces a ReturnStatement"
            );
        } else {
            panic!("expected Program");
        }
    }

    /// `throw` with the argument on the next line is a syntax error
    /// ("'throw' argument must be on the same line") and the parse fails.
    #[test]
    fn throw_newline_before_argument_fails() {
        use ast::context::Context;
        let mut sm = support::manager::SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"throw\nx;\n");
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
            "throw with newline before argument fails"
        );
        assert!(parser.error_count_pub() >= 1);
    }

    /// `foo: x;` parses to a LabeledStatement whose label is `foo` and whose
    /// body is the expression statement `x;`.
    #[test]
    fn labelled_statement_parses() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"foo: x;\n");
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
        let program = parser.parse().expect("labelled statement parses");
        assert_eq!(parser.error_count_pub(), 0, "zero errors");
        if let Node::Program(p) = program {
            let stmt = p.body.iter().next().expect("has statement");
            if let Node::LabeledStatement(ls) = stmt {
                if let Node::Identifier(id) = ls.label {
                    assert_eq!(gc.ctx().atom_table.bytes(id.name.get()), b"foo");
                } else {
                    panic!("label must be an Identifier");
                }
                assert!(
                    matches!(ls.body, Node::ExpressionStatement(_)),
                    "body is an ExpressionStatement"
                );
            } else {
                panic!("expected LabeledStatement");
            }
        } else {
            panic!("expected Program");
        }
    }

    // -----------------------------------------------------------------------
    // P2.2 binding-pattern leaves (driven via the test-only wrapper, since they
    // are not reachable from a statement until P2.3).
    // -----------------------------------------------------------------------

    #[test]
    fn binding_array_pattern_basic() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"[a, , ...b]");
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
        let pat = parser
            .parse_binding_pattern_for_test()
            .expect("array binding pattern parses");
        assert_eq!(parser.error_count_pub(), 0, "no errors");

        let ap = match pat {
            Node::ArrayPattern(ap) => ap,
            other => panic!("expected ArrayPattern, got {:?}", other.kind()),
        };
        let elems: Vec<&Node> = ap.elements.iter().collect();
        assert_eq!(elems.len(), 3, "three elements");
        assert!(
            matches!(elems[0], Node::Identifier(_)),
            "elem0 = Identifier(a)"
        );
        assert!(matches!(elems[1], Node::Empty(_)), "elem1 = Empty hole");
        match elems[2] {
            Node::RestElement(r) => {
                assert!(
                    matches!(r.argument, Node::Identifier(_)),
                    "rest arg = Identifier(b)"
                );
            }
            other => panic!("elem2 should be RestElement, got {:?}", other.kind()),
        }
    }

    #[test]
    fn binding_array_pattern_default_initializer() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"[a = 1]");
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
        let pat = parser
            .parse_binding_pattern_for_test()
            .expect("array binding pattern with default parses");
        assert_eq!(parser.error_count_pub(), 0, "no errors");

        let ap = match pat {
            Node::ArrayPattern(ap) => ap,
            other => panic!("expected ArrayPattern, got {:?}", other.kind()),
        };
        let elems: Vec<&Node> = ap.elements.iter().collect();
        assert_eq!(elems.len(), 1, "one element");
        match elems[0] {
            Node::AssignmentPattern(asn) => {
                assert!(
                    matches!(asn.left, Node::Identifier(_)),
                    "left = Identifier(a)"
                );
                assert!(
                    matches!(asn.right, Node::NumericLiteral(_)),
                    "right = NumericLiteral(1)"
                );
            }
            other => {
                panic!("elem0 should be AssignmentPattern, got {:?}", other.kind())
            }
        }
    }

    #[test]
    fn binding_object_pattern_basic() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"{a, b: c, d = 1, ...r}");
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
        let pat = parser
            .parse_binding_pattern_for_test()
            .expect("object binding pattern parses");
        assert_eq!(parser.error_count_pub(), 0, "no errors");

        let op = match pat {
            Node::ObjectPattern(op) => op,
            other => panic!("expected ObjectPattern, got {:?}", other.kind()),
        };
        let props: Vec<&Node> = op.properties.iter().collect();
        assert_eq!(props.len(), 4, "four properties");

        // {a} — shorthand Property whose value is a fresh Identifier.
        match props[0] {
            Node::Property(p) => {
                assert!(p.shorthand.get(), "a is shorthand");
                assert!(!p.computed.get(), "a not computed");
                assert!(matches!(p.key, Node::Identifier(_)), "key = a");
                assert!(matches!(p.value, Node::Identifier(_)), "value = a");
            }
            other => panic!("prop0 should be Property, got {:?}", other.kind()),
        }

        // {b: c} — keyed Property, value Identifier(c), not shorthand.
        match props[1] {
            Node::Property(p) => {
                assert!(!p.shorthand.get(), "b:c not shorthand");
                assert!(matches!(p.key, Node::Identifier(_)), "key = b");
                assert!(matches!(p.value, Node::Identifier(_)), "value = c");
            }
            other => panic!("prop1 should be Property, got {:?}", other.kind()),
        }

        // {d = 1} — Property whose value is an AssignmentPattern.
        match props[2] {
            Node::Property(p) => {
                assert!(p.shorthand.get(), "d = 1 is shorthand");
                match p.value {
                    Node::AssignmentPattern(asn) => {
                        assert!(
                            matches!(asn.left, Node::Identifier(_)),
                            "left = d"
                        );
                        assert!(
                            matches!(asn.right, Node::NumericLiteral(_)),
                            "right = 1"
                        );
                    }
                    other => panic!(
                        "prop2 value should be AssignmentPattern, got {:?}",
                        other.kind()
                    ),
                }
            }
            other => panic!("prop2 should be Property, got {:?}", other.kind()),
        }

        // {...r} — RestElement whose argument is an Identifier.
        match props[3] {
            Node::RestElement(r) => {
                assert!(
                    matches!(r.argument, Node::Identifier(_)),
                    "rest arg = r"
                );
            }
            other => panic!("prop3 should be RestElement, got {:?}", other.kind()),
        }
    }

    // -----------------------------------------------------------------------
    // P2.3: variable declarations (var/let/const/using).
    // -----------------------------------------------------------------------

    /// Parse `src`, returning the parser so the caller can inspect the program
    /// and (via a `CollectingHandler`) the emitted diagnostics. The handler is
    /// installed before parsing so error message text is captured.
    fn parse_with_collector<'gc>(
        gc: &'gc ast::context::GCLock<'_, '_>,
        sm: &mut support::manager::SourceErrorManager,
        atoms: &atom_table::AtomTable,
        src: &[u8],
    ) -> Option<&'gc ast::node::Node<'gc>> {
        sm.set_handler(Box::new(support::diag::CollectingHandler::new()));
        let buf_id = sm.add_buffer_bytes("input", src);
        let lexer = crate::lexer::JSLexer::new(
            buf_id,
            sm,
            atoms,
            crate::lexer::GrammarContext::AllowRegExp,
        );
        let mut parser = JSParserImpl::new(gc, lexer);
        parser.parse()
    }

    /// `var [a] = b;` → a VariableDeclaration with kind "var" whose single
    /// declarator's `id` is an ArrayPattern.
    #[test]
    fn var_array_pattern_declaration() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"var [a] = b;");
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
        let program = parser.parse().expect("var [a] = b; parses");
        assert_eq!(parser.error_count_pub(), 0);

        let Node::Program(p) = program else {
            panic!("expected Program")
        };
        let stmt = p.body.iter().next().expect("one statement");
        let Node::VariableDeclaration(vd) = stmt else {
            panic!("expected VariableDeclaration, got {:?}", stmt.kind())
        };
        assert_eq!(
            gc.ctx().atom_table.bytes(vd.kind.get()),
            b"var",
            "kind should be 'var'"
        );
        let decl = vd.declarations.iter().next().expect("one declarator");
        let Node::VariableDeclarator(d) = decl else {
            panic!("expected VariableDeclarator")
        };
        assert!(
            matches!(d.id, Node::ArrayPattern(_)),
            "declarator id should be ArrayPattern, got {:?}",
            d.id.kind()
        );
        assert!(d.init.is_some(), "declarator should have an initializer");
    }

    /// `const x;` → reports "missing initializer in const declaration".
    #[test]
    fn const_without_initializer_errors() {
        use ast::context::Context;
        use support::diag::{CollectingHandler, DiagKind};
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        let _program = parse_with_collector(&gc, &mut sm, atoms, b"const x;");

        let h = sm.handler_as::<CollectingHandler>().unwrap();
        let errs: Vec<_> = h
            .messages()
            .iter()
            .filter(|m| m.kind == DiagKind::Error)
            .collect();
        assert!(
            errs.iter()
                .any(|m| m.message == "missing initializer in const declaration"),
            "expected 'missing initializer in const declaration', got {:?}",
            errs.iter().map(|m| &m.message).collect::<Vec<_>>()
        );
    }

    /// `var [a];` → reports "destucturing declaration must be initialized"
    /// (the C++ typo "destucturing" is preserved).
    #[test]
    fn destructuring_without_initializer_errors() {
        use ast::context::Context;
        use support::diag::{CollectingHandler, DiagKind};
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        let _program = parse_with_collector(&gc, &mut sm, atoms, b"var [a];");

        let h = sm.handler_as::<CollectingHandler>().unwrap();
        let errs: Vec<_> = h
            .messages()
            .iter()
            .filter(|m| m.kind == DiagKind::Error)
            .collect();
        assert!(
            errs.iter()
                .any(|m| m.message == "destucturing declaration must be initialized"),
            "expected 'destucturing declaration must be initialized', got {:?}",
            errs.iter().map(|m| &m.message).collect::<Vec<_>>()
        );
    }

    /// `let x = 1;` → VariableDeclaration with kind "let".
    #[test]
    fn let_declaration_kind() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"let x = 1;");
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
        let program = parser.parse().expect("let x = 1; parses");
        assert_eq!(parser.error_count_pub(), 0);

        let Node::Program(p) = program else {
            panic!("expected Program")
        };
        let stmt = p.body.iter().next().expect("one statement");
        let Node::VariableDeclaration(vd) = stmt else {
            panic!("expected VariableDeclaration, got {:?}", stmt.kind())
        };
        assert_eq!(
            gc.ctx().atom_table.bytes(vd.kind.get()),
            b"let",
            "kind should be 'let'"
        );
    }

    /// Sloppy-mode `let;` is a loose identifier expression, not a declaration:
    /// it must parse as an ExpressionStatement. (Regression for the P1
    /// always-flag-`let` approximation now replaced by the real
    /// `is_let_followed_by_decl_start` lookahead.)
    #[test]
    fn loose_let_is_expression_statement() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"let;\nlet x;");
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
        let program = parser.parse().expect("let;\\nlet x; parses");
        assert_eq!(parser.error_count_pub(), 0);

        let Node::Program(p) = program else {
            panic!("expected Program")
        };
        let mut it = p.body.iter();
        // First: `let;` → ExpressionStatement (loose-mode identifier `let`).
        let first = it.next().expect("first statement");
        assert!(
            matches!(first, Node::ExpressionStatement(_)),
            "`let;` should be an ExpressionStatement, got {:?}",
            first.kind()
        );
        // Second: `let x;` → VariableDeclaration.
        let second = it.next().expect("second statement");
        assert!(
            matches!(second, Node::VariableDeclaration(_)),
            "`let x;` should be a VariableDeclaration, got {:?}",
            second.kind()
        );
    }

    // -----------------------------------------------------------------------
    // P2.4: block / if / while / do-while / switch / try statements.
    // -----------------------------------------------------------------------

    /// Helper: parse `src`, expect success with zero errors, return the first
    /// statement of the Program body.
    fn parse_first_stmt<'gc>(
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
            return p.body.iter().next().expect("has statement");
        }
        panic!("expected Program");
    }

    /// `{{x;}}` → a BlockStatement whose single child is a BlockStatement.
    #[test]
    fn nested_block_statement() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_first_stmt(&gc, &mut sm, b"{{x;}}");
        let Node::BlockStatement(outer) = stmt else {
            panic!("expected BlockStatement, got {:?}", stmt.kind())
        };
        let inner = outer.body.iter().next().expect("one inner statement");
        assert!(
            matches!(inner, Node::BlockStatement(_)),
            "inner statement should be BlockStatement, got {:?}",
            inner.kind()
        );
    }

    /// `if(a)b;else c;` → IfStatement with a non-None alternate.
    #[test]
    fn if_with_else() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_first_stmt(&gc, &mut sm, b"if(a)b;else c;");
        let Node::IfStatement(iff) = stmt else {
            panic!("expected IfStatement, got {:?}", stmt.kind())
        };
        assert!(iff.alternate.is_some(), "alternate should be present");
    }

    /// `if(a)if(b)c;else d;` → the else binds to the INNER if (dangling-else).
    #[test]
    fn dangling_else_binds_to_inner_if() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_first_stmt(&gc, &mut sm, b"if(a)if(b)c;else d;");
        let Node::IfStatement(outer) = stmt else {
            panic!("expected IfStatement, got {:?}", stmt.kind())
        };
        // Outer if has no else; its consequent is the inner if which DOES.
        assert!(
            outer.alternate.is_none(),
            "outer if should have no alternate"
        );
        let Node::IfStatement(inner) = outer.consequent else {
            panic!(
                "outer consequent should be IfStatement, got {:?}",
                outer.consequent.kind()
            )
        };
        assert!(
            inner.alternate.is_some(),
            "else should bind to the inner if"
        );
    }

    /// `while(x)y;` → WhileStatement whose `test` is the Identifier `x` and
    /// whose `body` is the ExpressionStatement `y;` (asserts body/test are NOT
    /// swapped — the C++ ctor takes body first).
    #[test]
    fn while_body_and_test_not_swapped() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_first_stmt(&gc, &mut sm, b"while(x)y;");
        let Node::WhileStatement(w) = stmt else {
            panic!("expected WhileStatement, got {:?}", stmt.kind())
        };
        // test must be the Identifier `x`.
        let Node::Identifier(id) = w.test else {
            panic!("test should be Identifier(x), got {:?}", w.test.kind())
        };
        assert_eq!(gc.ctx().atom_table.bytes(id.name.get()), b"x");
        // body must be the ExpressionStatement `y;`.
        assert!(
            matches!(w.body, Node::ExpressionStatement(_)),
            "body should be ExpressionStatement, got {:?}",
            w.body.kind()
        );
    }

    /// `switch(x){default:;default:;}` → reports "more than one 'default'
    /// clause in 'switch'".
    #[test]
    fn switch_duplicate_default_errors() {
        use ast::context::Context;
        use support::diag::{CollectingHandler, DiagKind};
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        let _program = parse_with_collector(
            &gc,
            &mut sm,
            atoms,
            b"switch(x){default:;default:;}",
        );
        let h = sm.handler_as::<CollectingHandler>().unwrap();
        let errs: Vec<_> = h
            .messages()
            .iter()
            .filter(|m| m.kind == DiagKind::Error)
            .collect();
        assert!(
            errs.iter().any(|m| m.message
                == "more than one 'default' clause in 'switch'"),
            "expected duplicate-default error, got {:?}",
            errs.iter().map(|m| &m.message).collect::<Vec<_>>()
        );
    }

    /// `try{}` (no catch/finally) → reports the catch/finally expected error.
    #[test]
    fn try_without_handler_errors() {
        use ast::context::Context;
        use support::diag::{CollectingHandler, DiagKind};
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        let _program = parse_with_collector(&gc, &mut sm, atoms, b"try{}");
        let h = sm.handler_as::<CollectingHandler>().unwrap();
        let errs: Vec<_> = h
            .messages()
            .iter()
            .filter(|m| m.kind == DiagKind::Error)
            .collect();
        assert!(
            errs.iter().any(|m| m
                .message
                .contains("'catch' or 'finally' expected")),
            "expected catch/finally error, got {:?}",
            errs.iter().map(|m| &m.message).collect::<Vec<_>>()
        );
    }

    /// `for(a in b)c;` → ForInStatement: left is Identifier(a), right is
    /// Identifier(b).
    #[test]
    fn for_in_basic() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_first_stmt(&gc, &mut sm, b"for(a in b)c;");
        let Node::ForInStatement(f) = stmt else {
            panic!("expected ForInStatement, got {:?}", stmt.kind())
        };
        let Node::Identifier(left) = f.left else {
            panic!("left should be Identifier(a), got {:?}", f.left.kind())
        };
        assert_eq!(gc.ctx().atom_table.bytes(left.name.get()), b"a");
        let Node::Identifier(right) = f.right else {
            panic!("right should be Identifier(b), got {:?}", f.right.kind())
        };
        assert_eq!(gc.ctx().atom_table.bytes(right.name.get()), b"b");
    }

    /// `for([a] of b)c;` → ForOfStatement whose `left` is an ArrayPattern
    /// (the `[a]` cover expression was reparsed into a pattern).
    #[test]
    fn for_of_array_pattern_left() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_first_stmt(&gc, &mut sm, b"for([a] of b)c;");
        let Node::ForOfStatement(f) = stmt else {
            panic!("expected ForOfStatement, got {:?}", stmt.kind())
        };
        assert!(
            matches!(f.left, Node::ArrayPattern(_)),
            "left should be ArrayPattern, got {:?}",
            f.left.kind()
        );
        assert!(!f.r#await.get(), "await should be false");
    }

    /// `for(var a, b in c);` → reports "Only one binding must be declared in a
    /// for-in/for-of loop".
    #[test]
    fn for_in_multiple_bindings_errors() {
        use ast::context::Context;
        use support::diag::{CollectingHandler, DiagKind};
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        let _program =
            parse_with_collector(&gc, &mut sm, atoms, b"for(var a, b in c);");
        let h = sm.handler_as::<CollectingHandler>().unwrap();
        let errs: Vec<_> = h
            .messages()
            .iter()
            .filter(|m| m.kind == DiagKind::Error)
            .collect();
        assert!(
            errs.iter().any(|m| m.message
                == "Only one binding must be declared in a for-in/for-of loop"),
            "expected single-binding error, got {:?}",
            errs.iter().map(|m| &m.message).collect::<Vec<_>>()
        );
    }

    /// `for(;;);` → ForStatement with init/test/update all None.
    #[test]
    fn for_empty_head() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_first_stmt(&gc, &mut sm, b"for(;;);");
        let Node::ForStatement(f) = stmt else {
            panic!("expected ForStatement, got {:?}", stmt.kind())
        };
        assert!(f.init.is_none(), "init should be None");
        assert!(f.test.is_none(), "test should be None");
        assert!(f.update.is_none(), "update should be None");
        assert!(
            matches!(f.body, Node::EmptyStatement(_)),
            "body should be EmptyStatement, got {:?}",
            f.body.kind()
        );
    }

    /// `for(var i=0;i<2;i++);` → ForStatement whose `init` is a
    /// VariableDeclaration.
    #[test]
    fn for_c_style_var_init() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_first_stmt(&gc, &mut sm, b"for(var i=0;i<2;i++);");
        let Node::ForStatement(f) = stmt else {
            panic!("expected ForStatement, got {:?}", stmt.kind())
        };
        let init = f.init.expect("init should be Some");
        assert!(
            matches!(init, Node::VariableDeclaration(_)),
            "init should be VariableDeclaration, got {:?}",
            init.kind()
        );
        assert!(f.test.is_some(), "test should be Some");
        assert!(f.update.is_some(), "update should be Some");
    }

    // ------------------------------------------------------------------
    // P3.2 — yield expressions
    // ------------------------------------------------------------------

    /// Parse `src` (a `function* g(){ <body> }`) and return the
    /// `YieldExpression` reached as the first statement's expression.
    fn first_yield<'gc>(
        gc: &'gc ast::context::GCLock<'_, '_>,
        sm: &mut support::manager::SourceErrorManager,
        src: &[u8],
    ) -> &'gc ast::node::YieldExpression<'gc> {
        use ast::node::Node;
        let decl = parse_first_stmt(gc, sm, src);
        let Node::FunctionDeclaration(f) = decl else {
            panic!("expected FunctionDeclaration, got {:?}", decl.kind())
        };
        let Node::BlockStatement(block) = f.body else {
            panic!("expected BlockStatement body, got {:?}", f.body.kind())
        };
        let first = block.body.iter().next().expect("body has a statement");
        let Node::ExpressionStatement(es) = first else {
            panic!("expected ExpressionStatement, got {:?}", first.kind())
        };
        let Node::YieldExpression(y) = es.expression else {
            panic!(
                "expected YieldExpression, got {:?}",
                es.expression.kind()
            )
        };
        y
    }

    /// `function* g(){ yield* a; }` → delegate=true, argument Some.
    #[test]
    fn yield_delegate() {
        use ast::context::Context;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let y = first_yield(&gc, &mut sm, b"function* g(){ yield* a; }");
        assert!(y.delegate.get(), "yield* should set delegate=true");
        assert!(y.argument.is_some(), "yield* a has an argument");
    }

    /// `function* g(){ yield; }` → argument None, delegate=false.
    #[test]
    fn yield_no_argument() {
        use ast::context::Context;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let y = first_yield(&gc, &mut sm, b"function* g(){ yield; }");
        assert!(y.argument.is_none(), "bare yield has no argument");
        assert!(!y.delegate.get(), "bare yield is not delegating");
    }

    /// `function* g(){ yield 1; }` → argument Some, delegate=false.
    #[test]
    fn yield_with_argument() {
        use ast::context::Context;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let y = first_yield(&gc, &mut sm, b"function* g(){ yield 1; }");
        assert!(y.argument.is_some(), "yield 1 has an argument");
        assert!(!y.delegate.get(), "yield 1 is not delegating");
    }

    // ------------------------------------------------------------------
    // P3.3 — arrow functions + cover-paren reparse
    // ------------------------------------------------------------------

    /// Parse `src` and return the `ArrowFunctionExpression` reached as the
    /// first statement's expression.
    fn first_arrow<'gc>(
        gc: &'gc ast::context::GCLock<'_, '_>,
        sm: &mut support::manager::SourceErrorManager,
        src: &[u8],
    ) -> &'gc ast::node::ArrowFunctionExpression<'gc> {
        use ast::node::Node;
        let stmt = parse_first_stmt(gc, sm, src);
        let Node::ExpressionStatement(es) = stmt else {
            panic!("expected ExpressionStatement, got {:?}", stmt.kind())
        };
        let Node::ArrowFunctionExpression(a) = es.expression else {
            panic!(
                "expected ArrowFunctionExpression, got {:?}",
                es.expression.kind()
            )
        };
        a
    }

    /// `a => a;` → expression=true, async=false, params=[Identifier].
    #[test]
    fn arrow_single_ident() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let a = first_arrow(&gc, &mut sm, b"a => a;");
        assert!(a.expression.get(), "concise body is an expression");
        assert!(!a.r#async.get(), "not async");
        let params: Vec<_> = a.params.iter().collect();
        assert_eq!(params.len(), 1, "one param");
        assert!(
            matches!(params[0], Node::Identifier(_)),
            "param is Identifier, got {:?}",
            params[0].kind()
        );
    }

    /// `() => 0;` → params empty.
    #[test]
    fn arrow_empty_params() {
        use ast::context::Context;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let a = first_arrow(&gc, &mut sm, b"() => 0;");
        assert_eq!(a.params.iter().count(), 0, "no params");
        assert!(a.expression.get(), "concise body");
    }

    /// `(a, ...b) => b;` → params=[Identifier, RestElement].
    #[test]
    fn arrow_rest_param() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let a = first_arrow(&gc, &mut sm, b"(a, ...b) => b;");
        let params: Vec<_> = a.params.iter().collect();
        assert_eq!(params.len(), 2, "two params");
        assert!(
            matches!(params[0], Node::Identifier(_)),
            "first param Identifier, got {:?}",
            params[0].kind()
        );
        assert!(
            matches!(params[1], Node::RestElement(_)),
            "second param RestElement, got {:?}",
            params[1].kind()
        );
    }

    /// `(a = 1) => a;` → params=[AssignmentPattern].
    #[test]
    fn arrow_default_param() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let a = first_arrow(&gc, &mut sm, b"(a = 1) => a;");
        let params: Vec<_> = a.params.iter().collect();
        assert_eq!(params.len(), 1, "one param");
        assert!(
            matches!(params[0], Node::AssignmentPattern(_)),
            "param AssignmentPattern, got {:?}",
            params[0].kind()
        );
    }

    /// `({x}) => x;` → params=[ObjectPattern].
    #[test]
    fn arrow_object_pattern_param() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let a = first_arrow(&gc, &mut sm, b"({x}) => x;");
        let params: Vec<_> = a.params.iter().collect();
        assert_eq!(params.len(), 1, "one param");
        assert!(
            matches!(params[0], Node::ObjectPattern(_)),
            "param ObjectPattern, got {:?}",
            params[0].kind()
        );
    }

    /// `a => { return a; };` → expression=false (block body).
    #[test]
    fn arrow_block_body() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let a = first_arrow(&gc, &mut sm, b"a => { return a; };");
        assert!(!a.expression.get(), "block body is not an expression");
        assert!(
            matches!(a.body, Node::BlockStatement(_)),
            "block body, got {:?}",
            a.body.kind()
        );
    }

    /// `async a => a;` → async=true.
    #[test]
    fn arrow_async_single_ident() {
        use ast::context::Context;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let a = first_arrow(&gc, &mut sm, b"async a => a;");
        assert!(a.r#async.get(), "async arrow");
        assert_eq!(a.params.iter().count(), 1, "one param");
    }

    /// `async (a) => a;` → async=true, params=[Identifier] (parsed via the
    /// async-CallExpression cover head).
    #[test]
    fn arrow_async_paren() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let a = first_arrow(&gc, &mut sm, b"async (a) => a;");
        assert!(a.r#async.get(), "async arrow");
        let params: Vec<_> = a.params.iter().collect();
        assert_eq!(params.len(), 1, "one param");
        assert!(
            matches!(params[0], Node::Identifier(_)),
            "param Identifier, got {:?}",
            params[0].kind()
        );
    }

    /// Non-arrow `(a)` → a parenthesized Identifier (parens recorded).
    #[test]
    fn paren_ident_not_arrow() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_first_stmt(&gc, &mut sm, b"(a);");
        let Node::ExpressionStatement(es) = stmt else {
            panic!("expected ExpressionStatement, got {:?}", stmt.kind())
        };
        assert!(
            matches!(es.expression, Node::Identifier(_)),
            "expression is Identifier, got {:?}",
            es.expression.kind()
        );
        assert_eq!(
            es.expression.metadata().parens.get(),
            1,
            "one paren recorded"
        );
    }

    /// Non-arrow `(a, b)` → SequenceExpression.
    #[test]
    fn paren_sequence_not_arrow() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_first_stmt(&gc, &mut sm, b"(a, b);");
        let Node::ExpressionStatement(es) = stmt else {
            panic!("expected ExpressionStatement, got {:?}", stmt.kind())
        };
        assert!(
            matches!(es.expression, Node::SequenceExpression(_)),
            "expression is SequenceExpression, got {:?}",
            es.expression.kind()
        );
    }

    /// Non-arrow `(a,)` → SequenceExpression whose last element is a
    /// `CoverTrailingComma` (matches hermesc — the cover node survives into the
    /// AST when not followed by `=>`).
    #[test]
    fn paren_trailing_comma_not_arrow() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_first_stmt(&gc, &mut sm, b"(a,);");
        let Node::ExpressionStatement(es) = stmt else {
            panic!("expected ExpressionStatement, got {:?}", stmt.kind())
        };
        let Node::SequenceExpression(seq) = es.expression else {
            panic!(
                "expected SequenceExpression, got {:?}",
                es.expression.kind()
            )
        };
        let elems: Vec<_> = seq.expressions.iter().collect();
        assert_eq!(elems.len(), 2, "[a, CoverTrailingComma]");
        assert!(
            matches!(elems[1], Node::CoverTrailingComma(_)),
            "last element CoverTrailingComma, got {:?}",
            elems[1].kind()
        );
    }

    /// Drill into `({ m() { return <expr>; } });` and return the return
    /// statement's argument, for the `super` tests below.
    #[cfg(test)]
    fn return_arg_in_object_method<'gc>(
        gc: &'gc ast::context::GCLock<'_, '_>,
        sm: &mut support::manager::SourceErrorManager,
        src: &[u8],
    ) -> &'gc ast::node::Node<'gc> {
        use ast::node::Node;
        let stmt = parse_first_stmt(gc, sm, src);
        let Node::ExpressionStatement(es) = stmt else {
            panic!("expected ExpressionStatement, got {:?}", stmt.kind())
        };
        let Node::ObjectExpression(obj) = es.expression else {
            panic!("expected ObjectExpression, got {:?}", es.expression.kind())
        };
        let prop = obj.properties.iter().next().expect("has property");
        let Node::Property(prop) = prop else {
            panic!("expected Property, got {:?}", prop.kind())
        };
        let Node::FunctionExpression(func) = prop.value else {
            panic!("expected FunctionExpression, got {:?}", prop.value.kind())
        };
        let Node::BlockStatement(block) = func.body else {
            panic!("expected BlockStatement, got {:?}", func.body.kind())
        };
        let ret = block.body.iter().next().expect("has return statement");
        let Node::ReturnStatement(ret) = ret else {
            panic!("expected ReturnStatement, got {:?}", ret.kind())
        };
        ret.argument.expect("return has argument")
    }

    /// `super.x` (in an object method) → a non-computed MemberExpression whose
    /// object is a `Super` node.
    #[test]
    fn super_member_dot() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let arg = return_arg_in_object_method(
            &gc,
            &mut sm,
            b"({ m() { return super.x; } });",
        );
        let Node::MemberExpression(member) = arg else {
            panic!("expected MemberExpression, got {:?}", arg.kind())
        };
        assert!(
            matches!(member.object, Node::Super(_)),
            "object is Super, got {:?}",
            member.object.kind()
        );
        assert!(!member.computed.get(), "super.x is not computed");
    }

    /// `super['y']` (in an object method) → a computed MemberExpression whose
    /// object is a `Super` node.
    #[test]
    fn super_member_computed() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let arg = return_arg_in_object_method(
            &gc,
            &mut sm,
            b"({ m() { return super['y']; } });",
        );
        let Node::MemberExpression(member) = arg else {
            panic!("expected MemberExpression, got {:?}", arg.kind())
        };
        assert!(
            matches!(member.object, Node::Super(_)),
            "object is Super, got {:?}",
            member.object.kind()
        );
        assert!(member.computed.get(), "super['y'] is computed");
    }

    // P5.0: Flow type alias parsing (js/flow/).

    /// Helper: parse `src` with Flow parsing enabled and assert at least one
    /// error was reported (the honest-deferral checks for unported Flow
    /// productions).
    fn assert_flow_parse_has_errors(src: &[u8], why: &str) {
        assert_parse_has_errors_impl(src, why, true);
    }

    /// `type X = number;` → TypeAlias{id "X", no type params, right
    /// NumberTypeAnnotation}.
    #[test]
    fn flow_type_alias_number() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"type X = number;");
        let Node::TypeAlias(alias) = stmt else {
            panic!("expected TypeAlias, got {:?}", stmt.kind())
        };
        assert_eq!(ident_bytes(&gc, alias.id), b"X");
        assert!(alias.type_parameters.is_none(), "no type parameters");
        assert!(
            matches!(alias.right, Node::NumberTypeAnnotation(_)),
            "right is NumberTypeAnnotation, got {:?}",
            alias.right.kind()
        );
    }

    /// `type X = 'hi';` → TypeAlias whose right is a
    /// StringLiteralTypeAnnotation with value "hi".
    #[test]
    fn flow_type_alias_string_literal() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"type X = 'hi';");
        let Node::TypeAlias(alias) = stmt else {
            panic!("expected TypeAlias, got {:?}", stmt.kind())
        };
        let Node::StringLiteralTypeAnnotation(lit) = alias.right else {
            panic!(
                "expected StringLiteralTypeAnnotation, got {:?}",
                alias.right.kind()
            )
        };
        assert_eq!(gc.ctx().atom_table.bytes(lit.value.get()), b"hi");
        assert_eq!(gc.ctx().atom_table.bytes(lit.raw.get()), b"'hi'");
    }

    /// Without `parse_flow`, `type X = number;` is plain JS: `type` is an
    /// ordinary identifier expression and the following `X` is a syntax
    /// error, exactly like hermesc without `-parse-flow` ("';' expected").
    #[test]
    fn flow_disabled_type_alias_is_plain_js() {
        assert_parse_has_errors(
            b"type X = number;",
            "'type X' must not parse as a declaration without parse_flow",
        );
    }

    /// Without `parse_flow`, `type` stays usable as a plain identifier.
    #[test]
    fn flow_disabled_type_is_plain_identifier() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"var type = 1;");
        assert!(
            matches!(stmt, Node::VariableDeclaration(_)),
            "expected VariableDeclaration, got {:?}",
            stmt.kind()
        );
    }

    /// Honest deferral errors for the unported Flow productions.
    #[test]
    fn flow_deferred_productions_error() {
        // P6.2: Flow enum declarations.
        assert_flow_parse_has_errors(b"enum E {}", "Flow enum is P6.2");
    }

    // ----------------------------------------------------------------------
    // P6.0: Flow ambiguous-expression grammar — `as`/`as const` + type-args.
    // ----------------------------------------------------------------------

    /// Helper: lock a Flow-ambiguous context (both `parse_flow` and
    /// `parse_flow_ambiguous`), parse `src`, and return the first statement's
    /// expression (it must be an `ExpressionStatement`).
    fn flow_ambiguous_expr<'gc>(
        gc: &'gc ast::context::GCLock<'_, '_>,
        sm: &mut support::manager::SourceErrorManager,
        src: &[u8],
    ) -> &'gc ast::node::Node<'gc> {
        let stmt = parse_one_stmt(gc, sm, src);
        let ast::node::Node::ExpressionStatement(es) = stmt else {
            panic!("expected ExpressionStatement, got {:?}", stmt.kind())
        };
        es.expression
    }

    /// `x as number` → `AsExpression{ Identifier "x", NumberTypeAnnotation }`.
    #[test]
    fn flow_as_expression() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        ctx.set_parse_flow_ambiguous(true);
        let gc = ctx.lock();
        let expr = flow_ambiguous_expr(&gc, &mut sm, b"x as number;");
        let Node::AsExpression(as_expr) = expr else {
            panic!("expected AsExpression, got {:?}", expr.kind())
        };
        assert_eq!(ident_bytes(&gc, as_expr.expression), b"x");
        assert!(
            matches!(as_expr.type_annotation, Node::NumberTypeAnnotation(_)),
            "type is NumberTypeAnnotation, got {:?}",
            as_expr.type_annotation.kind()
        );
    }

    /// `y as const` → `AsConstExpression{ Identifier "y" }` (the `const`
    /// special-case, NOT an `AsExpression` over a `GenericTypeAnnotation`).
    #[test]
    fn flow_as_const_expression() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        ctx.set_parse_flow_ambiguous(true);
        let gc = ctx.lock();
        let expr = flow_ambiguous_expr(&gc, &mut sm, b"y as const;");
        let Node::AsConstExpression(as_const) = expr else {
            panic!("expected AsConstExpression, got {:?}", expr.kind())
        };
        assert_eq!(ident_bytes(&gc, as_const.expression), b"y");
    }

    /// `f<T>()` is a `CallExpression` whose `type_arguments` is populated, and
    /// `a < b` rolls the speculation back into a `BinaryExpression`.
    #[test]
    fn flow_call_type_args_vs_comparison() {
        use ast::context::Context;
        use ast::node::Node;
        // f<T>() — type-args kept.
        {
            let mut sm = support::manager::SourceErrorManager::new();
            let mut ctx = Context::new();
            ctx.set_parse_flow(true);
            ctx.set_parse_flow_ambiguous(true);
            let gc = ctx.lock();
            let expr = flow_ambiguous_expr(&gc, &mut sm, b"f<T>();");
            let Node::CallExpression(call) = expr else {
                panic!("expected CallExpression, got {:?}", expr.kind())
            };
            assert!(
                call.type_arguments.is_some(),
                "f<T>() must keep type arguments"
            );
        }
        // a < b — speculation rolled back to a comparison.
        {
            let mut sm = support::manager::SourceErrorManager::new();
            let mut ctx = Context::new();
            ctx.set_parse_flow(true);
            ctx.set_parse_flow_ambiguous(true);
            let gc = ctx.lock();
            let expr = flow_ambiguous_expr(&gc, &mut sm, b"a < b;");
            assert!(
                matches!(expr, Node::BinaryExpression(_)),
                "a < b must be a BinaryExpression, got {:?}",
                expr.kind()
            );
        }
    }

    /// `new C<T>` (no args) is a `NewExpression` with type-args; `new C<T>(x)`
    /// keeps both type-args and arguments.
    #[test]
    fn flow_new_type_args() {
        use ast::context::Context;
        use ast::node::Node;
        // new C<T> — type-args, NO parens required.
        {
            let mut sm = support::manager::SourceErrorManager::new();
            let mut ctx = Context::new();
            ctx.set_parse_flow(true);
            ctx.set_parse_flow_ambiguous(true);
            let gc = ctx.lock();
            let expr = flow_ambiguous_expr(&gc, &mut sm, b"new C<T>;");
            let Node::NewExpression(new_expr) = expr else {
                panic!("expected NewExpression, got {:?}", expr.kind())
            };
            assert!(
                new_expr.type_arguments.is_some(),
                "new C<T> must keep type arguments"
            );
            assert_eq!(new_expr.arguments.iter().count(), 0, "no args");
        }
        // new C<T>(x) — type-args AND one argument.
        {
            let mut sm = support::manager::SourceErrorManager::new();
            let mut ctx = Context::new();
            ctx.set_parse_flow(true);
            ctx.set_parse_flow_ambiguous(true);
            let gc = ctx.lock();
            let expr = flow_ambiguous_expr(&gc, &mut sm, b"new C<T>(x);");
            let Node::NewExpression(new_expr) = expr else {
                panic!("expected NewExpression, got {:?}", expr.kind())
            };
            assert!(new_expr.type_arguments.is_some(), "type args kept");
            assert_eq!(new_expr.arguments.iter().count(), 1, "one arg");
        }
    }

    /// `obj?.foo<T>(x)` is an `OptionalCallExpression` with type-args (the
    /// `?.<T>()` form is unambiguous Flow — no SavePoint, no rollback).
    #[test]
    fn flow_optional_call_type_args() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        ctx.set_parse_flow_ambiguous(true);
        let gc = ctx.lock();
        let expr = flow_ambiguous_expr(&gc, &mut sm, b"obj?.foo<T>(x);");
        // obj?.foo is an OptionalMemberExpression; the trailing call is an
        // OptionalCallExpression carrying the type arguments.
        let Node::OptionalCallExpression(call) = expr else {
            panic!("expected OptionalCallExpression, got {:?}", expr.kind())
        };
        assert!(
            call.type_arguments.is_some(),
            "obj?.foo<T>(x) must keep type arguments"
        );
        assert_eq!(call.arguments.iter().count(), 1, "one argument");
    }

    /// Without the ambiguous flag, `f<T>()` is NOT a type-args call: `f < T`
    /// is a comparison, exactly like plain JS. (Guards against Flow leakage.)
    #[test]
    fn flow_ambiguous_off_keeps_comparison() {
        use ast::context::Context;
        use ast::node::Node;
        // parse_flow ON but parse_flow_ambiguous OFF: still a comparison chain.
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        // deliberately NOT setting parse_flow_ambiguous.
        let gc = ctx.lock();
        let expr = flow_ambiguous_expr(&gc, &mut sm, b"f < T > (g);");
        // `f < T > (g)` parses as `(f < T) > (g)` — a comparison, not a call.
        assert!(
            matches!(expr, Node::BinaryExpression(_)),
            "without ambiguous flag, f<T>(g) is a comparison, got {:?}",
            expr.kind()
        );
    }

    // P5.1: the full Flow type-annotation hierarchy (js/flow/).

    /// Helper: parse `src` with the caller's (Flow-enabled) context and
    /// return the right-hand side of the single top-level `TypeAlias`.
    fn flow_alias_right<'gc>(
        gc: &'gc ast::context::GCLock<'_, '_>,
        sm: &mut support::manager::SourceErrorManager,
        src: &[u8],
    ) -> &'gc ast::node::Node<'gc> {
        let stmt = parse_one_stmt(gc, sm, src);
        let ast::node::Node::TypeAlias(alias) = stmt else {
            panic!("expected TypeAlias, got {:?}", stmt.kind())
        };
        alias.right
    }

    /// Helper: assert `node` is a `GenericTypeAnnotation` over a plain
    /// `Identifier` named `name` (the shape every bare `X` parses to).
    fn assert_generic_named<'gc>(
        gc: &'gc ast::context::GCLock<'_, '_>,
        node: &ast::node::Node<'gc>,
        name: &[u8],
    ) {
        use ast::node::Node;
        let Node::GenericTypeAnnotation(g) = node else {
            panic!("expected GenericTypeAnnotation, got {:?}", node.kind())
        };
        assert!(g.type_parameters.is_none(), "no type args");
        assert_eq!(ident_bytes(gc, g.id), name);
    }

    /// Unions and intersections: member counts, leading-separator
    /// equivalence, and `&` binding tighter than `|`.
    #[test]
    fn flow_union_intersection_types() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        // `X | Y | Z` → a single Union with three members.
        let ty = flow_alias_right(&gc, &mut sm, b"type A = X | Y | Z;");
        let Node::UnionTypeAnnotation(u) = ty else {
            panic!("expected UnionTypeAnnotation, got {:?}", ty.kind())
        };
        let members: Vec<_> = u.types.iter().collect();
        assert_eq!(members.len(), 3);
        assert_generic_named(&gc, members[0], b"X");
        assert_generic_named(&gc, members[2], b"Z");

        // A leading `|` is allowed and does not add a member.
        let ty = flow_alias_right(&gc, &mut sm, b"type A = | X | Y;");
        let Node::UnionTypeAnnotation(u) = ty else {
            panic!("expected UnionTypeAnnotation, got {:?}", ty.kind())
        };
        assert_eq!(u.types.iter().count(), 2);

        // ...but a sole leading `|` yields the bare element, not a union.
        let ty = flow_alias_right(&gc, &mut sm, b"type A = | X;");
        assert_generic_named(&gc, ty, b"X");

        // `X & Y | Z` → Union[Intersection[X, Y], Z].
        let ty = flow_alias_right(&gc, &mut sm, b"type A = X & Y | Z;");
        let Node::UnionTypeAnnotation(u) = ty else {
            panic!("expected UnionTypeAnnotation, got {:?}", ty.kind())
        };
        let members: Vec<_> = u.types.iter().collect();
        assert_eq!(members.len(), 2);
        let Node::IntersectionTypeAnnotation(i) = members[0] else {
            panic!(
                "expected IntersectionTypeAnnotation, got {:?}",
                members[0].kind()
            )
        };
        assert_eq!(i.types.iter().count(), 2);
        assert_generic_named(&gc, members[1], b"Z");
    }

    /// `??X` parses as two nested NullableTypeAnnotations.
    #[test]
    fn flow_nullable_nesting() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        let ty = flow_alias_right(&gc, &mut sm, b"type A = ??X;");
        let Node::NullableTypeAnnotation(outer) = ty else {
            panic!("expected NullableTypeAnnotation, got {:?}", ty.kind())
        };
        let Node::NullableTypeAnnotation(inner) = outer.type_annotation else {
            panic!(
                "expected nested NullableTypeAnnotation, got {:?}",
                outer.type_annotation.kind()
            )
        };
        assert_generic_named(&gc, inner.type_annotation, b"X");
    }

    /// Postfix types: `X[]`/`X[][]` arrays, `X[K]` indexed access, `X?.[K]`
    /// optional indexed access, and the stickiness of `?.` in `X?.[A][B]`.
    #[test]
    fn flow_postfix_types() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        // `X[][]` → Array(Array(X)).
        let ty = flow_alias_right(&gc, &mut sm, b"type A = X[][];");
        let Node::ArrayTypeAnnotation(outer) = ty else {
            panic!("expected ArrayTypeAnnotation, got {:?}", ty.kind())
        };
        let Node::ArrayTypeAnnotation(inner) = outer.element_type else {
            panic!(
                "expected nested ArrayTypeAnnotation, got {:?}",
                outer.element_type.kind()
            )
        };
        assert_generic_named(&gc, inner.element_type, b"X");

        // `X[K]` → IndexedAccessType.
        let ty = flow_alias_right(&gc, &mut sm, b"type A = X[K];");
        let Node::IndexedAccessType(idx) = ty else {
            panic!("expected IndexedAccessType, got {:?}", ty.kind())
        };
        assert_generic_named(&gc, idx.object_type, b"X");
        assert_generic_named(&gc, idx.index_type, b"K");

        // `X?.[K]` → OptionalIndexedAccessType with optional=true.
        let ty = flow_alias_right(&gc, &mut sm, b"type A = X?.[K];");
        let Node::OptionalIndexedAccessType(opt) = ty else {
            panic!("expected OptionalIndexedAccessType, got {:?}", ty.kind())
        };
        assert!(opt.optional.get(), "?.[ access is optional");

        // `X?.[A][B]`: once a `?.[` is seen, the enclosing plain `[B]` access
        // is also an OptionalIndexedAccessType, but with optional=false.
        let ty = flow_alias_right(&gc, &mut sm, b"type A = X?.[A][B];");
        let Node::OptionalIndexedAccessType(outer) = ty else {
            panic!("expected OptionalIndexedAccessType, got {:?}", ty.kind())
        };
        assert!(!outer.optional.get(), "[B] itself is not optional");
        let Node::OptionalIndexedAccessType(inner) = outer.object_type else {
            panic!(
                "expected inner OptionalIndexedAccessType, got {:?}",
                outer.object_type.kind()
            )
        };
        assert!(inner.optional.get(), "?.[A] is optional");
    }

    /// Generic types: qualified names are left-associated
    /// QualifiedTypeIdentifiers; `Foo<>` is an empty (but present)
    /// TypeParameterInstantiation.
    #[test]
    fn flow_generic_types() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        // `Foo.Bar.Baz` → Qualified(Qualified(Foo, Bar), Baz).
        let ty = flow_alias_right(&gc, &mut sm, b"type A = Foo.Bar.Baz;");
        let Node::GenericTypeAnnotation(g) = ty else {
            panic!("expected GenericTypeAnnotation, got {:?}", ty.kind())
        };
        assert!(g.type_parameters.is_none());
        let Node::QualifiedTypeIdentifier(outer) = g.id else {
            panic!("expected QualifiedTypeIdentifier, got {:?}", g.id.kind())
        };
        assert_eq!(ident_bytes(&gc, outer.id), b"Baz");
        let Node::QualifiedTypeIdentifier(inner) = outer.qualification else {
            panic!(
                "expected inner QualifiedTypeIdentifier, got {:?}",
                outer.qualification.kind()
            )
        };
        assert_eq!(ident_bytes(&gc, inner.qualification), b"Foo");
        assert_eq!(ident_bytes(&gc, inner.id), b"Bar");

        // `Foo<>` → empty TypeParameterInstantiation.
        let ty = flow_alias_right(&gc, &mut sm, b"type A = Foo<>;");
        let Node::GenericTypeAnnotation(g) = ty else {
            panic!("expected GenericTypeAnnotation, got {:?}", ty.kind())
        };
        let args = g.type_parameters.expect("has type args");
        let Node::TypeParameterInstantiation(inst) = args else {
            panic!("expected TypeParameterInstantiation, got {:?}", args.kind())
        };
        assert_eq!(inst.params.iter().count(), 0, "`Foo<>` has no args");

        // `Foo<X, Y>` → two args.
        let ty = flow_alias_right(&gc, &mut sm, b"type A = Foo<X, Y>;");
        let Node::GenericTypeAnnotation(g) = ty else {
            panic!("expected GenericTypeAnnotation, got {:?}", ty.kind())
        };
        let Node::TypeParameterInstantiation(inst) =
            g.type_parameters.expect("has type args")
        else {
            panic!("expected TypeParameterInstantiation")
        };
        assert_eq!(inst.params.iter().count(), 2);
    }

    /// Nested generic type args: the closing `>` of inner type args must be
    /// consumed with GrammarContext::Type (the C++ default for
    /// `parseTypeArgsFlow`, JSParserImpl.h:1506) so the lexer splits the
    /// following `>>` into two `>` tokens instead of one shift token.
    #[test]
    fn flow_nested_generic_type_args() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        // `Foo<Bar<Baz<U>>>` → three nested GenericTypeAnnotation levels,
        // each with exactly one type argument.
        let ty = flow_alias_right(&gc, &mut sm, b"type A = Foo<Bar<Baz<U>>>;");
        let mut node = ty;
        for name in [&b"Foo"[..], b"Bar", b"Baz"] {
            let Node::GenericTypeAnnotation(g) = node else {
                panic!("expected GenericTypeAnnotation, got {:?}", node.kind())
            };
            assert_eq!(ident_bytes(&gc, g.id), name);
            let Node::TypeParameterInstantiation(inst) =
                g.type_parameters.expect("has type args")
            else {
                panic!("expected TypeParameterInstantiation")
            };
            assert_eq!(inst.params.iter().count(), 1, "one arg at each level");
            node = inst.params.iter().next().unwrap();
        }
        // Innermost argument: a bare `U` with no type args.
        assert_generic_named(&gc, node, b"U");
    }

    /// Typeof types: qualified chains, wrapping parens (recorded on the
    /// argument's parens counter — invisible in the AST dump), type args.
    #[test]
    fn flow_typeof_types() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        // `typeof x.y` → argument is a QualifiedTypeofIdentifier.
        let ty = flow_alias_right(&gc, &mut sm, b"type A = typeof x.y;");
        let Node::TypeofTypeAnnotation(t) = ty else {
            panic!("expected TypeofTypeAnnotation, got {:?}", ty.kind())
        };
        assert!(t.type_arguments.is_none());
        let Node::QualifiedTypeofIdentifier(q) = t.argument else {
            panic!(
                "expected QualifiedTypeofIdentifier, got {:?}",
                t.argument.kind()
            )
        };
        assert_eq!(ident_bytes(&gc, q.qualification), b"x");
        assert_eq!(ident_bytes(&gc, q.id), b"y");

        // `typeof (x)` → the paren is recorded on the Identifier argument.
        let ty = flow_alias_right(&gc, &mut sm, b"type A = typeof (x);");
        let Node::TypeofTypeAnnotation(t) = ty else {
            panic!("expected TypeofTypeAnnotation, got {:?}", ty.kind())
        };
        assert!(matches!(t.argument, Node::Identifier(_)));
        assert_eq!(t.argument.metadata().parens.get(), 1, "one paren recorded");

        // `typeof x<Y>` → type arguments attached to the TypeofTypeAnnotation.
        let ty = flow_alias_right(&gc, &mut sm, b"type A = typeof x<Y>;");
        let Node::TypeofTypeAnnotation(t) = ty else {
            panic!("expected TypeofTypeAnnotation, got {:?}", ty.kind())
        };
        let Node::TypeParameterInstantiation(inst) =
            t.type_arguments.expect("has type args")
        else {
            panic!("expected TypeParameterInstantiation")
        };
        assert_eq!(inst.params.iter().count(), 1);
    }

    /// Tuple types: plain, labeled (with optional), spread (bare and
    /// labeled), variance prefixes, inexact `...`, and empty.
    #[test]
    fn flow_tuple_types() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        // `[X, Y]` → two unlabeled (bare type) elements, not inexact.
        let ty = flow_alias_right(&gc, &mut sm, b"type A = [X, Y];");
        let Node::TupleTypeAnnotation(t) = ty else {
            panic!("expected TupleTypeAnnotation, got {:?}", ty.kind())
        };
        assert!(!t.inexact.get());
        let elems: Vec<_> = t.element_types.iter().collect();
        assert_eq!(elems.len(), 2);
        assert_generic_named(&gc, elems[0], b"X");

        // `[a: X, b?: Y]` → labeled elements; the second is optional.
        let ty = flow_alias_right(&gc, &mut sm, b"type A = [a: X, b?: Y];");
        let Node::TupleTypeAnnotation(t) = ty else {
            panic!("expected TupleTypeAnnotation, got {:?}", ty.kind())
        };
        let elems: Vec<_> = t.element_types.iter().collect();
        assert_eq!(elems.len(), 2);
        let Node::TupleTypeLabeledElement(first) = elems[0] else {
            panic!(
                "expected TupleTypeLabeledElement, got {:?}",
                elems[0].kind()
            )
        };
        assert_eq!(ident_bytes(&gc, first.label), b"a");
        assert!(!first.optional.get());
        assert!(first.variance.is_none());
        let Node::TupleTypeLabeledElement(second) = elems[1] else {
            panic!(
                "expected TupleTypeLabeledElement, got {:?}",
                elems[1].kind()
            )
        };
        assert!(second.optional.get(), "b? is optional");

        // `[X, ...Y]` → bare spread (no label); `[...rest: Y]` → labeled.
        let ty = flow_alias_right(&gc, &mut sm, b"type A = [X, ...Y];");
        let Node::TupleTypeAnnotation(t) = ty else {
            panic!("expected TupleTypeAnnotation, got {:?}", ty.kind())
        };
        let elems: Vec<_> = t.element_types.iter().collect();
        let Node::TupleTypeSpreadElement(spread) = elems[1] else {
            panic!(
                "expected TupleTypeSpreadElement, got {:?}",
                elems[1].kind()
            )
        };
        assert!(spread.label.is_none());
        assert_generic_named(&gc, spread.type_annotation, b"Y");

        let ty = flow_alias_right(&gc, &mut sm, b"type A = [...rest: Y];");
        let Node::TupleTypeAnnotation(t) = ty else {
            panic!("expected TupleTypeAnnotation, got {:?}", ty.kind())
        };
        let elems: Vec<_> = t.element_types.iter().collect();
        let Node::TupleTypeSpreadElement(spread) = elems[0] else {
            panic!(
                "expected TupleTypeSpreadElement, got {:?}",
                elems[0].kind()
            )
        };
        assert_eq!(ident_bytes(&gc, spread.label.expect("labeled")), b"rest");

        // `[+a: X, -b: Y]` → Variance kinds "plus" / "minus".
        let ty = flow_alias_right(&gc, &mut sm, b"type A = [+a: X, -b: Y];");
        let Node::TupleTypeAnnotation(t) = ty else {
            panic!("expected TupleTypeAnnotation, got {:?}", ty.kind())
        };
        let elems: Vec<_> = t.element_types.iter().collect();
        let Node::TupleTypeLabeledElement(first) = elems[0] else {
            panic!("expected TupleTypeLabeledElement")
        };
        let Node::Variance(v) = first.variance.expect("has variance") else {
            panic!("expected Variance")
        };
        assert_eq!(gc.ctx().atom_table.bytes(v.kind.get()), b"plus");
        let Node::TupleTypeLabeledElement(second) = elems[1] else {
            panic!("expected TupleTypeLabeledElement")
        };
        let Node::Variance(v) = second.variance.expect("has variance") else {
            panic!("expected Variance")
        };
        assert_eq!(gc.ctx().atom_table.bytes(v.kind.get()), b"minus");

        // `[X, ...]` → inexact, with one element.
        let ty = flow_alias_right(&gc, &mut sm, b"type A = [X, ...];");
        let Node::TupleTypeAnnotation(t) = ty else {
            panic!("expected TupleTypeAnnotation, got {:?}", ty.kind())
        };
        assert!(t.inexact.get(), "trailing ... makes the tuple inexact");
        assert_eq!(t.element_types.iter().count(), 1);

        // `[]` → empty tuple.
        let ty = flow_alias_right(&gc, &mut sm, b"type A = [];");
        let Node::TupleTypeAnnotation(t) = ty else {
            panic!("expected TupleTypeAnnotation, got {:?}", ty.kind())
        };
        assert_eq!(t.element_types.iter().count(), 0);
        assert!(!t.inexact.get());
    }

    /// The two tuple-specific diagnostics keep the exact C++ texts, and a
    /// non-identifier label trips the reparse helper's "identifier expected".
    #[test]
    fn flow_tuple_errors() {
        use ast::context::Context;
        use support::diag::{CollectingHandler, DiagKind};
        use support::manager::SourceErrorManager;

        // Comma after the inexact `...`.
        {
            let mut sm = SourceErrorManager::new();
            let mut ctx = Context::new();
            ctx.set_parse_flow(true);
            let gc = ctx.lock();
            let atoms = &gc.ctx().atom_table;
            let _ = parse_with_collector(&gc, &mut sm, atoms, b"type A = [X, ..., Y];");
            let h = sm.handler_as::<CollectingHandler>().unwrap();
            assert!(
                h.messages().iter().any(|m| m.kind == DiagKind::Error
                    && m.message
                        == "trailing commas after inexact tuple types are not allowed"),
                "got {:?}",
                h.messages().iter().map(|m| &m.message).collect::<Vec<_>>()
            );
        }

        // Variance on an unlabeled element.
        {
            let mut sm = SourceErrorManager::new();
            let mut ctx = Context::new();
            ctx.set_parse_flow(true);
            let gc = ctx.lock();
            let atoms = &gc.ctx().atom_table;
            let _ = parse_with_collector(&gc, &mut sm, atoms, b"type A = [+X];");
            let h = sm.handler_as::<CollectingHandler>().unwrap();
            assert!(
                h.messages().iter().any(|m| m.kind == DiagKind::Error
                    && m.message
                        == "Variance can only be used with labeled tuple elements"),
                "got {:?}",
                h.messages().iter().map(|m| &m.message).collect::<Vec<_>>()
            );
        }

        // A label that cannot reparse as an identifier.
        {
            let mut sm = SourceErrorManager::new();
            let mut ctx = Context::new();
            ctx.set_parse_flow(true);
            let gc = ctx.lock();
            let atoms = &gc.ctx().atom_table;
            let _ = parse_with_collector(&gc, &mut sm, atoms, b"type A = [1: X];");
            let h = sm.handler_as::<CollectingHandler>().unwrap();
            assert!(
                h.messages().iter().any(|m| m.kind == DiagKind::Error
                    && m.message == "identifier expected"),
                "got {:?}",
                h.messages().iter().map(|m| &m.message).collect::<Vec<_>>()
            );
        }
    }

    /// `keyof X` → KeyofTypeAnnotation over the generic argument.
    #[test]
    fn flow_keyof_type() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        let ty = flow_alias_right(&gc, &mut sm, b"type A = keyof X;");
        let Node::KeyofTypeAnnotation(k) = ty else {
            panic!("expected KeyofTypeAnnotation, got {:?}", ty.kind())
        };
        assert_generic_named(&gc, k.argument, b"X");
    }

    /// `X extends Y ? A : B` → ConditionalTypeAnnotation with the four
    /// generic children in the right slots.
    #[test]
    fn flow_conditional_type() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        let ty =
            flow_alias_right(&gc, &mut sm, b"type T = X extends Y ? A : B;");
        let Node::ConditionalTypeAnnotation(c) = ty else {
            panic!("expected ConditionalTypeAnnotation, got {:?}", ty.kind())
        };
        assert_generic_named(&gc, c.check_type, b"X");
        assert_generic_named(&gc, c.extends_type, b"Y");
        assert_generic_named(&gc, c.true_type, b"A");
        assert_generic_named(&gc, c.false_type, b"B");
    }

    /// Infer types: a bound after `extends` is kept inside a conditional's
    /// extends clause (conditional types disallowed there), but backtracked
    /// away when a `?` follows in a position that allows conditional types.
    #[test]
    fn flow_infer_type_bound_and_backtrack() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        // Helper: unwrap InferTypeAnnotation → TypeParameter.
        fn infer_param<'gc>(node: &'gc Node<'gc>) -> &'gc ast::node::TypeParameter<'gc> {
            let Node::InferTypeAnnotation(i) = node else {
                panic!("expected InferTypeAnnotation, got {:?}", node.kind())
            };
            let Node::TypeParameter(p) = i.type_parameter else {
                panic!(
                    "expected TypeParameter, got {:?}",
                    i.type_parameter.kind()
                )
            };
            p
        }

        // `X extends infer U ? U : never` → infer without bound.
        let ty = flow_alias_right(
            &gc,
            &mut sm,
            b"type T = X extends infer U ? U : never;",
        );
        let Node::ConditionalTypeAnnotation(c) = ty else {
            panic!("expected ConditionalTypeAnnotation, got {:?}", ty.kind())
        };
        let p = infer_param(c.extends_type);
        assert_eq!(gc.ctx().atom_table.bytes(p.name.get()), b"U");
        assert!(p.bound.is_none());
        assert!(p.uses_extends_bound.get());

        // `X extends infer U extends V ? U : never`: inside the conditional's
        // extends clause conditional types are disallowed, so `extends V`
        // binds to the infer type (the bound is KEPT).
        let ty = flow_alias_right(
            &gc,
            &mut sm,
            b"type T = X extends infer U extends V ? U : never;",
        );
        let Node::ConditionalTypeAnnotation(c) = ty else {
            panic!("expected ConditionalTypeAnnotation, got {:?}", ty.kind())
        };
        let p = infer_param(c.extends_type);
        let bound = p.bound.expect("bound kept");
        assert_generic_named(&gc, bound, b"V");

        // `infer U extends V ? A : B` at the top of an annotation: here
        // conditional types ARE allowed, so seeing `?` after the speculative
        // bound parse backtracks — `extends V` belongs to the conditional and
        // the infer loses its bound.
        let ty = flow_alias_right(
            &gc,
            &mut sm,
            b"type T = infer U extends V ? A : B;",
        );
        let Node::ConditionalTypeAnnotation(c) = ty else {
            panic!("expected ConditionalTypeAnnotation, got {:?}", ty.kind())
        };
        let p = infer_param(c.check_type);
        assert!(p.bound.is_none(), "bound backtracked away");
        assert_generic_named(&gc, c.extends_type, b"V");

        // Without a following `?` the bound is kept even at the top level.
        let ty =
            flow_alias_right(&gc, &mut sm, b"type T = infer U extends V;");
        let p = infer_param(ty);
        assert!(p.bound.is_some(), "no `?` follows — bound kept");
    }

    /// Negative literal types: the value is negated and the raw spans the
    /// `-` through the literal.
    #[test]
    fn flow_negative_literal_types() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        let ty = flow_alias_right(&gc, &mut sm, b"type A = -3;");
        let Node::NumberLiteralTypeAnnotation(n) = ty else {
            panic!(
                "expected NumberLiteralTypeAnnotation, got {:?}",
                ty.kind()
            )
        };
        assert_eq!(n.value.get(), -3.0);
        assert_eq!(gc.ctx().atom_table.bytes(n.raw.get()), b"-3");

        let ty = flow_alias_right(&gc, &mut sm, b"type A = -2n;");
        let Node::BigIntLiteralTypeAnnotation(b) = ty else {
            panic!(
                "expected BigIntLiteralTypeAnnotation, got {:?}",
                ty.kind()
            )
        };
        assert_eq!(gc.ctx().atom_table.bytes(b.raw.get()), b"-2n");
    }

    // P5.2: function types, object types, type-parameter declarations,
    // variance, predicates, return-type annotations (js/flow/).

    /// Helper: assert `node` is a `FunctionTypeAnnotation` and return it.
    fn as_fta<'gc, 'n>(
        node: &'n ast::node::Node<'gc>,
    ) -> &'n ast::node::FunctionTypeAnnotation<'gc> {
        let ast::node::Node::FunctionTypeAnnotation(fta) = node else {
            panic!("expected FunctionTypeAnnotation, got {:?}", node.kind())
        };
        fta
    }

    /// Helper: assert `node` is a `FunctionTypeParam` and return it.
    fn as_ftp<'gc, 'n>(
        node: &'n ast::node::Node<'gc>,
    ) -> &'n ast::node::FunctionTypeParam<'gc> {
        let ast::node::Node::FunctionTypeParam(ftp) = node else {
            panic!("expected FunctionTypeParam, got {:?}", node.kind())
        };
        ftp
    }

    /// The full function-type shape: type params, `this` constraint, named/
    /// optional params, rest, and the return type.
    #[test]
    fn flow_function_type_full_shape() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        let ty = flow_alias_right(
            &gc,
            &mut sm,
            b"type A = <T>(this: X, a: B, c?: D, ...rest: E) => R;",
        );
        let fta = as_fta(ty);

        // Type parameters.
        let tp = fta.type_parameters.expect("has type params");
        let Node::TypeParameterDeclaration(tpd) = tp else {
            panic!("expected TypeParameterDeclaration, got {:?}", tp.kind())
        };
        assert_eq!(tpd.params.iter().count(), 1);

        // `this` constraint: an unnamed FunctionTypeParam.
        let this_param = as_ftp(fta.this.expect("has this constraint"));
        assert!(this_param.name.is_none(), "this constraint has no name");
        assert_generic_named(&gc, this_param.type_annotation, b"X");

        // Named + optional params.
        let params: Vec<_> = fta.params.iter().collect();
        assert_eq!(params.len(), 2);
        let a = as_ftp(params[0]);
        assert_eq!(ident_bytes(&gc, a.name.expect("a named")), b"a");
        assert!(!a.optional.get());
        let c = as_ftp(params[1]);
        assert_eq!(ident_bytes(&gc, c.name.expect("c named")), b"c");
        assert!(c.optional.get());

        // Rest param.
        let rest = as_ftp(fta.rest.expect("has rest"));
        assert_eq!(ident_bytes(&gc, rest.name.expect("rest named")), b"rest");

        // Return type.
        assert_generic_named(&gc, fta.return_type, b"R");

        // An unnamed parameter type: `(number) => string`.
        let ty = flow_alias_right(&gc, &mut sm, b"type C = (number) => string;");
        let fta = as_fta(ty);
        assert!(fta.this.is_none());
        assert!(fta.rest.is_none());
        assert!(fta.type_parameters.is_none());
        let params: Vec<_> = fta.params.iter().collect();
        assert_eq!(params.len(), 1);
        let p = as_ftp(params[0]);
        assert!(p.name.is_none(), "bare type param has no name");
        assert!(matches!(p.type_annotation, Node::NumberTypeAnnotation(_)));
    }

    /// `(T)` group vs `(x: T) => R` vs `(T) => R` disambiguation; the group
    /// returns the inner type with its paren count bumped.
    #[test]
    fn flow_group_vs_function_type() {
        use ast::context::Context;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        // A plain group: the inner type itself, parens incremented.
        let ty = flow_alias_right(&gc, &mut sm, b"type A = (X);");
        assert_generic_named(&gc, ty, b"X");
        assert_eq!(ty.metadata().parens.get(), 1, "group bumps parens");

        // A named param forces a function type.
        let ty = flow_alias_right(&gc, &mut sm, b"type B = (x: X) => R;");
        let fta = as_fta(ty);
        let params: Vec<_> = fta.params.iter().collect();
        assert_eq!(ident_bytes(&gc, as_ftp(params[0]).name.unwrap()), b"x");

        // An unnamed param resolved as a function by the trailing `=>`.
        let ty = flow_alias_right(&gc, &mut sm, b"type C = (X) => R;");
        let fta = as_fta(ty);
        assert!(as_ftp(fta.params.iter().next().unwrap()).name.is_none());

        // An empty param list is always a function.
        let ty = flow_alias_right(&gc, &mut sm, b"type D = () => R;");
        assert!(as_fta(ty).params.is_empty());
    }

    /// The anonymous function type `T => U => V` nests to the right.
    #[test]
    fn flow_anon_function_type() {
        use ast::context::Context;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        let ty = flow_alias_right(&gc, &mut sm, b"type A = T => U => V;");
        let outer = as_fta(ty);
        let param = as_ftp(outer.params.iter().next().expect("one param"));
        assert!(param.name.is_none());
        assert_generic_named(&gc, param.type_annotation, b"T");
        let inner = as_fta(outer.return_type);
        assert_generic_named(&gc, inner.return_type, b"V");
    }

    /// Object types: plain/optional/method/get/set properties with their
    /// `kind` atoms and variance.
    #[test]
    fn flow_object_type_properties() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        let ty = flow_alias_right(
            &gc,
            &mut sm,
            b"type A = { x: B, y?: C, m(): D, get g(): E, set s(v: F): void, +ro: G };",
        );
        let Node::ObjectTypeAnnotation(obj) = ty else {
            panic!("expected ObjectTypeAnnotation, got {:?}", ty.kind())
        };
        assert!(!obj.exact.get());
        assert!(!obj.inexact.get());
        assert!(obj.indexers.is_empty());
        assert!(obj.call_properties.is_empty());
        assert!(obj.internal_slots.is_empty());

        let props: Vec<_> = obj.properties.iter().collect();
        assert_eq!(props.len(), 6);
        let prop = |i: usize| -> &ast::node::ObjectTypeProperty<'_> {
            let Node::ObjectTypeProperty(p) = props[i] else {
                panic!("expected ObjectTypeProperty, got {:?}", props[i].kind())
            };
            p
        };

        // x: B
        let x = prop(0);
        assert_eq!(ident_bytes(&gc, x.key), b"x");
        assert!(!x.method.get() && !x.optional.get());
        assert!(!x.r#static.get() && !x.proto.get());
        assert!(x.variance.is_none());
        assert_eq!(gc.ctx().atom_table.bytes(x.kind.get()), b"init");

        // y?: C
        let y = prop(1);
        assert!(y.optional.get());

        // m(): D — a method; the value is a FunctionTypeAnnotation.
        let m = prop(2);
        assert!(m.method.get());
        assert_generic_named(&gc, as_fta(m.value).return_type, b"D");
        assert_eq!(gc.ctx().atom_table.bytes(m.kind.get()), b"init");

        // get g(): E
        let g = prop(3);
        assert!(!g.method.get());
        assert_eq!(ident_bytes(&gc, g.key), b"g");
        assert_eq!(gc.ctx().atom_table.bytes(g.kind.get()), b"get");

        // set s(v: F): void
        let s = prop(4);
        assert_eq!(gc.ctx().atom_table.bytes(s.kind.get()), b"set");
        assert_eq!(as_fta(s.value).params.iter().count(), 1);

        // +ro: G
        let ro = prop(5);
        let variance = ro.variance.expect("has variance");
        let Node::Variance(v) = variance else {
            panic!("expected Variance, got {:?}", variance.kind())
        };
        assert_eq!(gc.ctx().atom_table.bytes(v.kind.get()), b"plus");
    }

    /// Object types: indexers (with and without an id), mapped types with
    /// every optionality sigil, call properties, internal slots, spreads,
    /// exact `{| |}`, and explicit inexact `{ ... }`.
    #[test]
    fn flow_object_type_member_families() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        let obj_of = |sm: &mut support::manager::SourceErrorManager,
                      src: &[u8]| {
            let ty = flow_alias_right(&gc, sm, src);
            let Node::ObjectTypeAnnotation(obj) = ty else {
                panic!("expected ObjectTypeAnnotation, got {:?}", ty.kind())
            };
            obj
        };

        // Indexer with an id: `[k: string]: V`.
        let obj = obj_of(&mut sm, b"type A = { [k: string]: V };");
        let idx = obj.indexers.iter().next().expect("one indexer");
        let Node::ObjectTypeIndexer(idx) = idx else {
            panic!("expected ObjectTypeIndexer, got {:?}", idx.kind())
        };
        assert_eq!(ident_bytes(&gc, idx.id.expect("has id")), b"k");
        assert!(matches!(idx.key, Node::StringTypeAnnotation(_)));
        assert!(!idx.r#static.get());

        // Indexer without an id: `[K]: V`.
        let obj = obj_of(&mut sm, b"type B = { [K]: V };");
        let idx = obj.indexers.iter().next().expect("one indexer");
        let Node::ObjectTypeIndexer(idx) = idx else {
            panic!("expected ObjectTypeIndexer, got {:?}", idx.kind())
        };
        assert!(idx.id.is_none());
        assert_generic_named(&gc, idx.key, b"K");

        // Mapped types: every optionality sigil (a null NodeString when no
        // sigil — matching the C++ nullptr → `"optional": null` dump).
        for (src, sigil) in [
            (b"type C = { [K in T]: V };".as_slice(), None),
            (b"type D = { [K in T]?: V };", Some(b"Optional".as_slice())),
            (b"type E = { [K in T]+?: V };", Some(b"PlusOptional")),
            (b"type F = { [K in T]-?: V };", Some(b"MinusOptional")),
        ] {
            let obj = obj_of(&mut sm, src);
            let prop = obj.properties.iter().next().expect("one property");
            let Node::ObjectTypeMappedTypeProperty(mapped) = prop else {
                panic!(
                    "expected ObjectTypeMappedTypeProperty, got {:?}",
                    prop.kind()
                )
            };
            let Node::TypeParameter(key_tparam) = mapped.key_tparam else {
                panic!("expected TypeParameter")
            };
            assert_eq!(
                gc.ctx().atom_table.bytes(key_tparam.name.get()),
                b"K"
            );
            assert_generic_named(&gc, mapped.source_type, b"T");
            assert_generic_named(&gc, mapped.prop_type, b"V");
            match sigil {
                None => assert_eq!(
                    mapped.optional.get(),
                    atom_table::INVALID_ATOM_BYTES,
                    "no sigil dumps as null"
                ),
                Some(s) => assert_eq!(
                    gc.ctx().atom_table.bytes(mapped.optional.get()),
                    s
                ),
            }
        }

        // Mapped type with variance before the bracket: `+[K in T]: V`.
        let obj = obj_of(&mut sm, b"type G = { +[K in T]: V };");
        let prop = obj.properties.iter().next().expect("one property");
        let Node::ObjectTypeMappedTypeProperty(mapped) = prop else {
            panic!("expected ObjectTypeMappedTypeProperty")
        };
        assert!(mapped.variance.is_some());

        // Call property + internal slot + spread.
        let obj =
            obj_of(&mut sm, b"type H = { (x: A): R, [[slot]]: T, ...S };");
        let call = obj.call_properties.iter().next().expect("one call");
        let Node::ObjectTypeCallProperty(call) = call else {
            panic!("expected ObjectTypeCallProperty, got {:?}", call.kind())
        };
        assert_eq!(as_fta(call.value).params.iter().count(), 1);
        let slot = obj.internal_slots.iter().next().expect("one slot");
        let Node::ObjectTypeInternalSlot(slot) = slot else {
            panic!("expected ObjectTypeInternalSlot, got {:?}", slot.kind())
        };
        assert_eq!(ident_bytes(&gc, slot.id), b"slot");
        assert!(!slot.method.get() && !slot.optional.get());
        let spread = obj.properties.iter().next().expect("one spread");
        let Node::ObjectTypeSpreadProperty(spread) = spread else {
            panic!("expected ObjectTypeSpreadProperty, got {:?}", spread.kind())
        };
        assert_generic_named(&gc, spread.argument, b"S");

        // A method-typed internal slot: `[[m]](): R`.
        let obj = obj_of(&mut sm, b"type I = { [[m]](): R };");
        let slot = obj.internal_slots.iter().next().expect("one slot");
        let Node::ObjectTypeInternalSlot(slot) = slot else {
            panic!("expected ObjectTypeInternalSlot")
        };
        assert!(slot.method.get());

        // Exact `{| |}` and explicit inexact `{ ... }`.
        let obj = obj_of(&mut sm, b"type J = {| a: T |};");
        assert!(obj.exact.get());
        let obj = obj_of(&mut sm, b"type K = { a: T, ... };");
        assert!(obj.inexact.get());
        let obj = obj_of(&mut sm, b"type L = { ... };");
        assert!(obj.inexact.get());
        assert!(obj.properties.is_empty());
    }

    /// `static`/`proto` (and `readonly` before `:`) fall back to property
    /// and method names in an object type that disallows those modifiers.
    #[test]
    fn flow_object_type_modifier_name_fallbacks() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        for (src, name, method) in [
            (b"type A = { static: T };".as_slice(), b"static".as_slice(), false),
            (b"type B = { proto: T };", b"proto", false),
            (b"type C = { static(): R };", b"static", true),
            (b"type D = { readonly: T };", b"readonly", false),
        ] {
            let ty = flow_alias_right(&gc, &mut sm, src);
            let Node::ObjectTypeAnnotation(obj) = ty else {
                panic!("expected ObjectTypeAnnotation, got {:?}", ty.kind())
            };
            let prop = obj.properties.iter().next().expect("one property");
            let Node::ObjectTypeProperty(prop) = prop else {
                panic!("expected ObjectTypeProperty, got {:?}", prop.kind())
            };
            assert_eq!(ident_bytes(&gc, prop.key), name);
            assert_eq!(prop.method.get(), method);
            assert!(!prop.r#static.get(), "the keyword was the name");
            assert!(!prop.proto.get(), "the keyword was the name");
        }
    }

    /// Type-parameter declarations: const, sigil and keyword variance with
    /// the `in`/`out` name-vs-variance disambiguation, `:` vs `extends`
    /// bounds, and defaults.
    #[test]
    fn flow_type_param_declarations() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        let params_of = |sm: &mut support::manager::SourceErrorManager,
                         src: &[u8]| {
            let stmt = parse_one_stmt(&gc, sm, src);
            let Node::TypeAlias(alias) = stmt else {
                panic!("expected TypeAlias, got {:?}", stmt.kind())
            };
            let tp = alias.type_parameters.expect("has type params");
            let Node::TypeParameterDeclaration(tpd) = tp else {
                panic!("expected TypeParameterDeclaration, got {:?}", tp.kind())
            };
            tpd.params.iter().collect::<Vec<_>>()
        };
        let tparam = |node: &'_ ast::node::Node<'_>| {
            let Node::TypeParameter(p) = node else {
                panic!("expected TypeParameter, got {:?}", node.kind())
            };
            let name = gc.ctx().atom_table.bytes(p.name.get()).to_vec();
            let variance = p.variance.map(|v| {
                let Node::Variance(v) = v else {
                    panic!("expected Variance, got {:?}", v.kind())
                };
                gc.ctx().atom_table.bytes(v.kind.get()).to_vec()
            });
            (name, variance)
        };

        // Plain + trailing comma.
        let params = params_of(&mut sm, b"type A<T,> = T;");
        assert_eq!(params.len(), 1);
        assert_eq!(tparam(params[0]), (b"T".to_vec(), None));

        // `const` modifier.
        let params = params_of(&mut sm, b"type B<const T> = T;");
        let Node::TypeParameter(p) = params[0] else { unreachable!() };
        assert!(p.r#const.get());

        // Sigil variance.
        let params = params_of(&mut sm, b"type C<+T, -U> = [T, U];");
        assert_eq!(tparam(params[0]), (b"T".to_vec(), Some(b"plus".to_vec())));
        assert_eq!(tparam(params[1]), (b"U".to_vec(), Some(b"minus".to_vec())));

        // `in T` / `out T`: the keyword is variance, `T` is the name.
        let params = params_of(&mut sm, b"type D<in T, out U> = [T, U];");
        assert_eq!(tparam(params[0]), (b"T".to_vec(), Some(b"in".to_vec())));
        assert_eq!(tparam(params[1]), (b"U".to_vec(), Some(b"out".to_vec())));

        // `<in>` / `<out = X>`: the keyword is the NAME, no variance.
        let params = params_of(&mut sm, b"type E<in> = X;");
        assert_eq!(tparam(params[0]), (b"in".to_vec(), None));
        let params = params_of(&mut sm, b"type F<out = X> = out;");
        assert_eq!(tparam(params[0]), (b"out".to_vec(), None));
        let Node::TypeParameter(p) = params[0] else { unreachable!() };
        assert!(p.default.is_some(), "`= X` is the default");

        // `:` bound (wrapped in TypeAnnotation) vs `extends` bound.
        let params = params_of(&mut sm, b"type G<T: number> = T;");
        let Node::TypeParameter(p) = params[0] else { unreachable!() };
        let bound = p.bound.expect("has bound");
        let Node::TypeAnnotation(bound) = bound else {
            panic!("expected TypeAnnotation, got {:?}", bound.kind())
        };
        assert!(matches!(
            bound.type_annotation,
            Node::NumberTypeAnnotation(_)
        ));
        assert!(!p.uses_extends_bound.get());

        let params = params_of(&mut sm, b"type H<T extends U> = T;");
        let Node::TypeParameter(p) = params[0] else { unreachable!() };
        assert!(p.bound.is_some());
        assert!(p.uses_extends_bound.get());

        // Default.
        let params = params_of(&mut sm, b"type I<T = string> = T;");
        let Node::TypeParameter(p) = params[0] else { unreachable!() };
        assert!(matches!(
            p.default.expect("has default"),
            Node::StringTypeAnnotation(_)
        ));
    }

    /// Return-type predicates through function types: unprefixed `x is T`
    /// (null kind), `asserts x [is T]`, and `implies x is T`.
    #[test]
    fn flow_type_predicates() {
        use ast::context::Context;
        use ast::node::Node;
        let mut sm = support::manager::SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        let predicate_of = |sm: &mut support::manager::SourceErrorManager,
                            src: &[u8]| {
            let ty = flow_alias_right(&gc, sm, src);
            as_fta(ty).return_type
        };

        // Unprefixed `x is number`: the kind is the null NodeString
        // (C++ passes nullptr; dumps as `"kind": null`).
        let ret = predicate_of(&mut sm, b"type A = (x: mixed) => x is number;");
        let Node::TypePredicate(p) = ret else {
            panic!("expected TypePredicate, got {:?}", ret.kind())
        };
        assert_eq!(ident_bytes(&gc, p.parameter_name), b"x");
        assert!(matches!(
            p.type_annotation.expect("has type"),
            Node::NumberTypeAnnotation(_)
        ));
        assert_eq!(p.kind.get(), atom_table::INVALID_ATOM_BYTES);

        // `asserts x is T` and the type-less `asserts x`.
        let ret =
            predicate_of(&mut sm, b"type B = (x: mixed) => asserts x is T;");
        let Node::TypePredicate(p) = ret else {
            panic!("expected TypePredicate, got {:?}", ret.kind())
        };
        assert_eq!(gc.ctx().atom_table.bytes(p.kind.get()), b"asserts");
        assert!(p.type_annotation.is_some());

        let ret = predicate_of(&mut sm, b"type C = (x: mixed) => asserts x;");
        let Node::TypePredicate(p) = ret else {
            panic!("expected TypePredicate, got {:?}", ret.kind())
        };
        assert!(p.type_annotation.is_none());

        // A bare `asserts` return type is just a generic type.
        let ret = predicate_of(&mut sm, b"type D = (x: mixed) => asserts;");
        assert_generic_named(&gc, ret, b"asserts");

        // `implies x is T`.
        let ret =
            predicate_of(&mut sm, b"type E = (x: mixed) => implies x is T;");
        let Node::TypePredicate(p) = ret else {
            panic!("expected TypePredicate, got {:?}", ret.kind())
        };
        assert_eq!(gc.ctx().atom_table.bytes(p.kind.get()), b"implies");
        assert!(p.type_annotation.is_some());
    }

    /// `%checks` predicates: `parse_predicate_flow` is wired into function
    /// declarations in P5.4, so drive it directly — `%checks` only lexes as
    /// one identifier in Type grammar context.
    #[test]
    fn flow_checks_predicates() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let parse_predicate = |src: &[u8]| -> (&'static str, bool) {
            let mut sm = SourceErrorManager::new();
            let buf_id = sm.add_buffer_bytes("input", src);
            let mut ctx = Context::new();
            ctx.set_parse_flow(true);
            let gc = ctx.lock();
            let atoms = &gc.ctx().atom_table;
            let lexer = crate::lexer::JSLexer::new(
                buf_id,
                &mut sm,
                atoms,
                crate::lexer::GrammarContext::AllowRegExp,
            );
            let mut parser = JSParserImpl::new(&gc, lexer);
            // Skip the leading `x`, re-lexing in Type context so the
            // following `%checks` scans as a single identifier.
            parser.advance(crate::lexer::GrammarContext::Type);
            let pred =
                parser.parse_predicate_flow().expect("predicate parses");
            let kind = match pred {
                Node::DeclaredPredicate(d) => {
                    assert!(
                        matches!(d.value, Node::Identifier(_)),
                        "the checks expression is parsed as a JS expression"
                    );
                    "declared"
                }
                Node::InferredPredicate(_) => "inferred",
                other => panic!("unexpected predicate {:?}", other.kind()),
            };
            (kind, parser.error_count_pub() == 0)
        };

        assert_eq!(parse_predicate(b"x %checks(y)"), ("declared", true));
        assert_eq!(parse_predicate(b"x %checks"), ("inferred", true));
    }

    /// The P5.2 diagnostics keep the exact C++ texts.
    #[test]
    fn flow_p52_errors() {
        use ast::context::Context;
        use support::diag::{CollectingHandler, DiagKind};
        use support::manager::SourceErrorManager;

        let assert_error = |src: &[u8], expected: &str| {
            let mut sm = SourceErrorManager::new();
            let mut ctx = Context::new();
            ctx.set_parse_flow(true);
            let gc = ctx.lock();
            let atoms = &gc.ctx().atom_table;
            let _ = parse_with_collector(&gc, &mut sm, atoms, src);
            let h = sm.handler_as::<CollectingHandler>().unwrap();
            assert!(
                h.messages()
                    .iter()
                    .any(|m| m.kind == DiagKind::Error && m.message == expected),
                "expected {:?}, got {:?}",
                expected,
                h.messages().iter().map(|m| &m.message).collect::<Vec<_>>()
            );
        };

        assert_error(
            b"type A = {| a: T, ... |};",
            "Explicit inexact syntax cannot appear inside an explicit exact object type",
        );
        assert_error(
            b"type A = { get x(a: B): T };",
            "Getter must have 0 parameters",
        );
        assert_error(
            b"type A = { set x(): void };",
            "Setter must have 1 parameter",
        );
        assert_error(
            b"type A = { get x(this: B): T };",
            "Accessors must not have 'this' annotations",
        );
        assert_error(
            b"type A = (this?: X) => Y;",
            "'this' constraint may not be optional",
        );
        assert_error(
            b"type A = (a: X, this: Y) => Z;",
            "'this' constraint must be the first parameter",
        );
        assert_error(
            b"type A = { +(): R };",
            "call property must not specify variance",
        );
        assert_error(
            b"type A = { +get x(): T };",
            "accessor property must not specify variance",
        );
        assert_error(b"type A = { proto x: T };", "invalid 'proto' modifier");
        assert_error(b"type A = { static x: T };", "invalid 'static' modifier");
        assert_error(b"type A = { +[[s]]: T };", "Unexpected variance sigil");
        // `implies<T>` parses as a generic WITH type args, so the following
        // identifier triggers the not-a-bare-identifier guard.
        assert_error(
            b"type A = (x) => implies<T> x is U;",
            "invalid return annotation. 'implies' type guard needs to be followed by identifier",
        );
        assert_error(
            b"type A = (x) => implies x;",
            "expecting 'is' after parameter of 'implies' type guard",
        );
    }

    // P5.3: opaque type aliases, interface declarations/type annotations,
    // and class implements entries (js/flow/).

    /// Helper: parse `src` with the caller's (e.g. Flow-enabled) context,
    /// expect zero errors, return the top-level statement at `idx` (for the
    /// strict-mode tests, where the directive prologue is statement 0).
    fn flow_parse_stmt_at<'gc>(
        gc: &'gc ast::context::GCLock<'_, '_>,
        sm: &mut support::manager::SourceErrorManager,
        src: &[u8],
        idx: usize,
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
            return p.body.iter().nth(idx).expect("has enough statements");
        }
        panic!("expected Program");
    }

    /// The `opaque type` alias shapes: plain, type params, the legacy
    /// `: Supertype`, and the `super`/`extends` bounds.
    #[test]
    fn flow_opaque_type_shapes() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        // (src, has type params, lower bound, upper bound, supertype)
        let check = |src: &[u8],
                     has_tp: bool,
                     has_lower: bool,
                     has_upper: bool,
                     has_super: bool| {
            let mut sm = SourceErrorManager::new();
            let mut ctx = Context::new();
            ctx.set_parse_flow(true);
            let gc = ctx.lock();
            let stmt = parse_one_stmt(&gc, &mut sm, src);
            let Node::OpaqueType(o) = stmt else {
                panic!("expected OpaqueType, got {:?}", stmt.kind())
            };
            assert_eq!(o.type_parameters.is_some(), has_tp, "{src:?} tp");
            assert_eq!(o.lower_bound.is_some(), has_lower, "{src:?} lower");
            assert_eq!(o.upper_bound.is_some(), has_upper, "{src:?} upper");
            assert_eq!(o.supertype.is_some(), has_super, "{src:?} super");
        };

        check(b"opaque type A = number;", false, false, false, false);
        check(b"opaque type B<T> = T;", true, false, false, false);
        check(b"opaque type C: number = 1;", false, false, false, true);
        check(b"opaque type D super X = Y;", false, true, false, false);
        check(b"opaque type E extends F = G;", false, false, true, false);
        check(
            b"opaque type H super X extends F = G;",
            false,
            true,
            true,
            false,
        );

        // The node shape of the legacy-supertype form.
        let mut sm = SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();
        let stmt = parse_one_stmt(&gc, &mut sm, b"opaque type C: number = 1;");
        let Node::OpaqueType(o) = stmt else {
            panic!("expected OpaqueType, got {:?}", stmt.kind())
        };
        assert_eq!(ident_bytes(&gc, o.id), b"C");
        assert!(
            matches!(o.supertype, Some(Node::NumberTypeAnnotation(_))),
            "supertype is NumberTypeAnnotation"
        );
        assert!(
            matches!(o.impltype, Node::NumberLiteralTypeAnnotation(_)),
            "impltype is NumberLiteralTypeAnnotation, got {:?}",
            o.impltype.kind()
        );
    }

    /// Interface declarations: id, type params, the `extends` list (with the
    /// GenericTypeAnnotation → InterfaceExtends unwrapping), and the body.
    #[test]
    fn flow_interface_declaration() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        // Plain interface with one property.
        let stmt = parse_one_stmt(&gc, &mut sm, b"interface I { x: number }");
        let Node::InterfaceDeclaration(decl) = stmt else {
            panic!("expected InterfaceDeclaration, got {:?}", stmt.kind())
        };
        assert_eq!(ident_bytes(&gc, decl.id), b"I");
        assert!(decl.type_parameters.is_none(), "no type params");
        assert!(decl.extends.is_empty(), "no extends");
        let Node::ObjectTypeAnnotation(body) = decl.body else {
            panic!("expected ObjectTypeAnnotation body")
        };
        assert_eq!(body.properties.iter().count(), 1, "one property");

        // Type params + a two-entry extends list; the second entry keeps the
        // generic's type arguments.
        let stmt = parse_one_stmt(
            &gc,
            &mut sm,
            b"interface J<T> extends K, L<T> { m(): void }",
        );
        let Node::InterfaceDeclaration(decl) = stmt else {
            panic!("expected InterfaceDeclaration, got {:?}", stmt.kind())
        };
        assert_eq!(ident_bytes(&gc, decl.id), b"J");
        assert!(decl.type_parameters.is_some(), "has type params");
        let extends: Vec<_> = decl.extends.iter().collect();
        assert_eq!(extends.len(), 2, "two extends entries");
        let Node::InterfaceExtends(e0) = extends[0] else {
            panic!("expected InterfaceExtends, got {:?}", extends[0].kind())
        };
        assert_eq!(ident_bytes(&gc, e0.id), b"K");
        assert!(e0.type_parameters.is_none(), "K has no type args");
        let Node::InterfaceExtends(e1) = extends[1] else {
            panic!("expected InterfaceExtends, got {:?}", extends[1].kind())
        };
        assert_eq!(ident_bytes(&gc, e1.id), b"L");
        assert!(e1.type_parameters.is_some(), "L<T> keeps its type args");

        // Empty body.
        let stmt = parse_one_stmt(&gc, &mut sm, b"interface E {}");
        let Node::InterfaceDeclaration(decl) = stmt else {
            panic!("expected InterfaceDeclaration, got {:?}", stmt.kind())
        };
        let Node::ObjectTypeAnnotation(body) = decl.body else {
            panic!("expected ObjectTypeAnnotation body")
        };
        assert!(body.properties.is_empty(), "empty body");
    }

    /// `interface { ... }` as a TYPE annotation, in both spellings:
    /// loose mode lexes `interface` as a plain identifier (the
    /// NamedType::Interface arm); strict mode lexes it as rw_interface (the
    /// reserved-word arm). Both build InterfaceTypeAnnotation.
    #[test]
    fn flow_interface_type_annotation() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        // Loose mode: the identifier arm.
        let right =
            flow_alias_right(&gc, &mut sm, b"type A = interface { x: number };");
        let Node::InterfaceTypeAnnotation(ita) = right else {
            panic!("expected InterfaceTypeAnnotation, got {:?}", right.kind())
        };
        assert!(ita.extends.is_empty(), "no extends");
        assert!(
            matches!(ita.body, Some(Node::ObjectTypeAnnotation(_))),
            "body is ObjectTypeAnnotation"
        );

        // An interface type with an extends clause.
        let right = flow_alias_right(
            &gc,
            &mut sm,
            b"type B = interface extends I { y: T };",
        );
        let Node::InterfaceTypeAnnotation(ita) = right else {
            panic!("expected InterfaceTypeAnnotation, got {:?}", right.kind())
        };
        let extends: Vec<_> = ita.extends.iter().collect();
        assert_eq!(extends.len(), 1, "one extends entry");
        assert!(matches!(extends[0], Node::InterfaceExtends(_)));

        // Strict mode: the rw_interface arm (type position).
        let stmt = flow_parse_stmt_at(
            &gc,
            &mut sm,
            b"'use strict'; type C = interface { x: number };",
            1,
        );
        let Node::TypeAlias(alias) = stmt else {
            panic!("expected TypeAlias, got {:?}", stmt.kind())
        };
        assert!(
            matches!(alias.right, Node::InterfaceTypeAnnotation(_)),
            "rw_interface arm builds InterfaceTypeAnnotation, got {:?}",
            alias.right.kind()
        );

        // Strict mode: the rw_interface arm (declaration position).
        let stmt = flow_parse_stmt_at(
            &gc,
            &mut sm,
            b"'use strict'; interface S { x: number }",
            1,
        );
        assert!(
            matches!(stmt, Node::InterfaceDeclaration(_)),
            "rw_interface declaration parses, got {:?}",
            stmt.kind()
        );
    }

    /// `parse_class_implements_flow` (direct call — the class-heritage
    /// integration lands in P5.4): `I` and `I<T>`.
    #[test]
    fn flow_class_implements() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let parse_impl = |src: &[u8], expect_args: bool| {
            let mut sm = SourceErrorManager::new();
            let buf_id = sm.add_buffer_bytes("input", src);
            let mut ctx = Context::new();
            ctx.set_parse_flow(true);
            let gc = ctx.lock();
            let atoms = &gc.ctx().atom_table;
            let lexer = crate::lexer::JSLexer::new(
                buf_id,
                &mut sm,
                atoms,
                crate::lexer::GrammarContext::AllowRegExp,
            );
            let mut parser = JSParserImpl::new(&gc, lexer);
            let node = parser
                .parse_class_implements_flow()
                .expect("class implements parses");
            let Node::ClassImplements(ci) = node else {
                panic!("expected ClassImplements, got {:?}", node.kind())
            };
            assert_eq!(ident_bytes(&gc, ci.id), b"I");
            assert_eq!(
                ci.type_parameters.is_some(),
                expect_args,
                "{src:?} type args"
            );
            assert_eq!(parser.error_count_pub(), 0, "zero errors");
        };

        parse_impl(b"I", false);
        parse_impl(b"I<T>", true);
    }

    /// The P5.3 diagnostics keep the exact C++ texts.
    #[test]
    fn flow_p53_errors() {
        use ast::context::Context;
        use support::diag::{CollectingHandler, DiagKind};
        use support::manager::SourceErrorManager;

        let assert_error = |src: &[u8], expected: &str| {
            let mut sm = SourceErrorManager::new();
            let mut ctx = Context::new();
            ctx.set_parse_flow(true);
            let gc = ctx.lock();
            let atoms = &gc.ctx().atom_table;
            let _ = parse_with_collector(&gc, &mut sm, atoms, src);
            let h = sm.handler_as::<CollectingHandler>().unwrap();
            assert!(
                h.messages()
                    .iter()
                    .any(|m| m.kind == DiagKind::Error && m.message == expected),
                "expected {:?}, got {:?}",
                expected,
                h.messages().iter().map(|m| &m.message).collect::<Vec<_>>()
            );
        };

        // Interface bodies pass AllowSpreadProperty::No, finally making this
        // P5.2 diagnostic reachable.
        assert_error(
            b"interface I { ...T }",
            "Spreading a type is only allowed inside an object type",
        );
        // An opaque alias requires `= T` (only DeclareOpaque may omit it).
        assert_error(b"opaque type X;", "'=' expected in type alias");
        // `opaque` must be followed by `type`.
        assert_error(
            b"opaque interface I {}",
            "invalid token in opaque type declaration",
        );
    }

    // P5.4: Flow non-ambiguous integration — the type grammar hung off the
    // core productions (functions, params, bindings, classes, object-literal
    // methods).

    /// Function signature: type params, annotated params, return type.
    #[test]
    fn flow_function_signature() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();
        let stmt = parse_one_stmt(
            &gc,
            &mut sm,
            b"function f<T>(x: T): T { return x; }",
        );
        let Node::FunctionDeclaration(f) = stmt else {
            panic!("expected FunctionDeclaration, got {:?}", stmt.kind())
        };
        assert!(f.type_parameters.is_some(), "type params");
        assert!(f.return_type.is_some(), "return type");
        assert!(f.predicate.is_none(), "no predicate");
        let param = f.params.iter().next().expect("one param");
        let Node::Identifier(p) = param else {
            panic!("expected Identifier param, got {:?}", param.kind())
        };
        assert!(p.type_annotation.is_some(), "param annotation");
        assert!(!p.optional.get(), "param not optional");
    }

    /// `%checks` predicates: inferred (after a return type) and declared
    /// (directly after the colon, with no return type).
    #[test]
    fn flow_function_predicates() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        // Return type + inferred predicate.
        {
            let mut sm = SourceErrorManager::new();
            let mut ctx = Context::new();
            ctx.set_parse_flow(true);
            let gc = ctx.lock();
            let stmt = parse_one_stmt(
                &gc,
                &mut sm,
                b"function p(x: mixed): boolean %checks { return !!x; }",
            );
            let Node::FunctionDeclaration(f) = stmt else {
                panic!("expected FunctionDeclaration, got {:?}", stmt.kind())
            };
            assert!(f.return_type.is_some(), "return type");
            assert!(
                matches!(f.predicate, Some(Node::InferredPredicate(_))),
                "inferred predicate"
            );
        }

        // Declared predicate with NO return type (`): %checks(expr)`).
        {
            let mut sm = SourceErrorManager::new();
            let mut ctx = Context::new();
            ctx.set_parse_flow(true);
            let gc = ctx.lock();
            let stmt = parse_one_stmt(
                &gc,
                &mut sm,
                b"function q(x: mixed): %checks (x === 1) {}",
            );
            let Node::FunctionDeclaration(f) = stmt else {
                panic!("expected FunctionDeclaration, got {:?}", stmt.kind())
            };
            assert!(f.return_type.is_none(), "no return type");
            assert!(
                matches!(f.predicate, Some(Node::DeclaredPredicate(_))),
                "declared predicate"
            );
        }
    }

    /// A leading `this` parameter is pushed as the FIRST formal parameter,
    /// with its type annotation and the following comma consumed.
    #[test]
    fn flow_this_param() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();
        let stmt = parse_one_stmt(
            &gc,
            &mut sm,
            b"function g(this: Object, a: number): void {}",
        );
        let Node::FunctionDeclaration(f) = stmt else {
            panic!("expected FunctionDeclaration, got {:?}", stmt.kind())
        };
        let params: Vec<_> = f.params.iter().collect();
        assert_eq!(params.len(), 2, "two params");
        let Node::Identifier(this_param) = params[0] else {
            panic!("expected Identifier, got {:?}", params[0].kind())
        };
        assert_eq!(
            gc.ctx().atom_table.bytes(this_param.name.get()),
            b"this",
            "first param is 'this'"
        );
        assert!(this_param.type_annotation.is_some(), "'this' annotation");
        assert!(!this_param.optional.get());
        let Node::Identifier(a_param) = params[1] else {
            panic!("expected Identifier, got {:?}", params[1].kind())
        };
        assert_eq!(gc.ctx().atom_table.bytes(a_param.name.get()), b"a");
    }

    /// Binding annotations: the `?` optional marker and `:` type on binding
    /// identifiers, and `:` types on array/object binding patterns.
    #[test]
    fn flow_binding_annotations() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        /// The single declarator's id of the variable declaration in `src`.
        fn decl_id<'gc>(
            gc: &'gc ast::context::GCLock<'_, '_>,
            src: &[u8],
        ) -> &'gc Node<'gc> {
            let mut sm = SourceErrorManager::new();
            let stmt = parse_one_stmt(gc, &mut sm, src);
            let Node::VariableDeclaration(d) = stmt else {
                panic!("expected VariableDeclaration, got {:?}", stmt.kind())
            };
            let Node::VariableDeclarator(declarator) =
                d.declarations.iter().next().expect("one declarator")
            else {
                panic!("expected VariableDeclarator")
            };
            declarator.id
        }

        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();

        // `?` + `:` on a binding identifier.
        let id = decl_id(&gc, b"var a?: number;");
        let Node::Identifier(id) = id else {
            panic!("expected Identifier, got {:?}", id.kind())
        };
        assert!(id.optional.get(), "optional");
        assert!(id.type_annotation.is_some(), "id annotation");

        // `:` on an array binding pattern.
        let pat = decl_id(&gc, b"var [x, y]: T = c;");
        let Node::ArrayPattern(pat) = pat else {
            panic!("expected ArrayPattern, got {:?}", pat.kind())
        };
        assert!(pat.type_annotation.is_some(), "array pattern annotation");

        // `:` on an object binding pattern.
        let pat = decl_id(&gc, b"var {x}: T = c;");
        let Node::ObjectPattern(pat) = pat else {
            panic!("expected ObjectPattern, got {:?}", pat.kind())
        };
        assert!(pat.type_annotation.is_some(), "object pattern annotation");

        // Optional parameter `a?: T` in a formal parameter list.
        let mut sm = SourceErrorManager::new();
        let stmt = parse_one_stmt(&gc, &mut sm, b"function fd(a?: T) {}");
        let Node::FunctionDeclaration(f) = stmt else {
            panic!("expected FunctionDeclaration, got {:?}", stmt.kind())
        };
        let param = f.params.iter().next().expect("one param");
        let Node::Identifier(p) = param else {
            panic!("expected Identifier param, got {:?}", param.kind())
        };
        assert!(p.optional.get(), "optional param");
        assert!(p.type_annotation.is_some(), "optional param annotation");
    }

    /// Class integration: class/method type params, super-class type args,
    /// the implements clause, field annotations + variance, method/getter
    /// return types, and private-field annotations.
    #[test]
    fn flow_class_integration() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();
        let stmt = parse_one_stmt(
            &gc,
            &mut sm,
            b"class C<T> extends B<T> implements I, J<T> {\n\
              \x20 x: number;\n\
              \x20 +ro: T;\n\
              \x20 readonly r: V;\n\
              \x20 #p: T;\n\
              \x20 static: number;\n\
              \x20 m<U>(a: U): U { return a; }\n\
              \x20 get g(): T { return this.x; }\n\
              }",
        );
        let Node::ClassDeclaration(c) = stmt else {
            panic!("expected ClassDeclaration, got {:?}", stmt.kind())
        };
        assert!(c.type_parameters.is_some(), "class type params");
        assert!(c.super_class.is_some(), "super class");
        assert!(c.super_type_arguments.is_some(), "super type args");

        // implements I, J<T>
        let impls: Vec<_> = c.implements.iter().collect();
        assert_eq!(impls.len(), 2, "two implements entries");
        let Node::ClassImplements(i0) = impls[0] else {
            panic!("expected ClassImplements, got {:?}", impls[0].kind())
        };
        assert!(i0.type_parameters.is_none(), "I has no type args");
        let Node::ClassImplements(i1) = impls[1] else {
            panic!("expected ClassImplements, got {:?}", impls[1].kind())
        };
        assert!(i1.type_parameters.is_some(), "J<T> has type args");

        let Node::ClassBody(body) = c.body else {
            panic!("expected ClassBody")
        };
        let elems: Vec<_> = body.body.iter().collect();
        assert_eq!(elems.len(), 7, "seven class elements");

        // x: number;
        let Node::ClassProperty(x) = elems[0] else {
            panic!("expected ClassProperty, got {:?}", elems[0].kind())
        };
        assert!(x.type_annotation.is_some(), "x annotation");
        assert!(x.variance.is_none(), "x has no variance");

        // +ro: T;
        let Node::ClassProperty(ro) = elems[1] else {
            panic!("expected ClassProperty, got {:?}", elems[1].kind())
        };
        let Some(Node::Variance(v)) = ro.variance else {
            panic!("expected Variance on +ro")
        };
        assert_eq!(gc.ctx().atom_table.bytes(v.kind.get()), b"plus");

        // readonly r: V; (contextual-keyword variance)
        let Node::ClassProperty(r) = elems[2] else {
            panic!("expected ClassProperty, got {:?}", elems[2].kind())
        };
        let Some(Node::Variance(v)) = r.variance else {
            panic!("expected Variance on readonly r")
        };
        assert_eq!(gc.ctx().atom_table.bytes(v.kind.get()), b"readonly");

        // #p: T;
        let Node::ClassPrivateProperty(p) = elems[3] else {
            panic!("expected ClassPrivateProperty, got {:?}", elems[3].kind())
        };
        assert!(p.type_annotation.is_some(), "#p annotation");

        // static: number; — `static` is the property NAME here.
        let Node::ClassProperty(s) = elems[4] else {
            panic!("expected ClassProperty, got {:?}", elems[4].kind())
        };
        let Node::Identifier(s_key) = s.key else {
            panic!("expected Identifier key")
        };
        assert_eq!(gc.ctx().atom_table.bytes(s_key.name.get()), b"static");
        assert!(!s.r#static.get(), "'static' is the name, not a modifier");
        assert!(s.type_annotation.is_some(), "static-field annotation");

        // m<U>(a: U): U {}
        let Node::MethodDefinition(m) = elems[5] else {
            panic!("expected MethodDefinition, got {:?}", elems[5].kind())
        };
        let Node::FunctionExpression(mf) = m.value else {
            panic!("expected FunctionExpression")
        };
        assert!(mf.type_parameters.is_some(), "method type params");
        assert!(mf.return_type.is_some(), "method return type");

        // get g(): T {}
        let Node::MethodDefinition(getter) = elems[6] else {
            panic!("expected MethodDefinition, got {:?}", elems[6].kind())
        };
        assert_eq!(gc.ctx().atom_table.bytes(getter.kind.get()), b"get");
        let Node::FunctionExpression(gf) = getter.value else {
            panic!("expected FunctionExpression")
        };
        assert!(gf.return_type.is_some(), "getter return type");
    }

    /// An anonymous class expression: with Flow, `<`/`implements` after
    /// `class` means there is no class name.
    #[test]
    fn flow_class_expression_heritage() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        let expr = parse_expr_from(
            &gc,
            &mut sm,
            atoms,
            b"(class <T> implements K { y: T; });",
        );
        let Node::ClassExpression(c) = expr else {
            panic!("expected ClassExpression, got {:?}", expr.kind())
        };
        assert!(c.id.is_none(), "anonymous");
        assert!(c.type_parameters.is_some(), "type params");
        assert_eq!(c.implements.iter().count(), 1, "one implements entry");
    }

    /// Object-literal methods: type params and return types, plus
    /// `get`/`set` used as method names (detected via `<`).
    #[test]
    fn flow_object_literal_methods() {
        use ast::context::Context;
        use ast::node::Node;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let mut ctx = Context::new();
        ctx.set_parse_flow(true);
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        let expr = parse_expr_from(
            &gc,
            &mut sm,
            atoms,
            b"({ m<T>(x: T): T { return x; },\n\
              \x20  get x(): number { return 1; },\n\
              \x20  set y(v: number): void {},\n\
              \x20  get<T>(x) { return x; } });",
        );
        let Node::ObjectExpression(obj) = expr else {
            panic!("expected ObjectExpression, got {:?}", expr.kind())
        };
        let props: Vec<_> = obj.properties.iter().collect();
        assert_eq!(props.len(), 4, "four properties");

        // m<T>(x: T): T {}
        let Node::Property(m) = props[0] else {
            panic!("expected Property")
        };
        assert!(m.method.get(), "m is a method");
        let Node::FunctionExpression(mf) = m.value else {
            panic!("expected FunctionExpression")
        };
        assert!(mf.type_parameters.is_some(), "method type params");
        assert!(mf.return_type.is_some(), "method return type");

        // get x(): number {}
        let Node::Property(g) = props[1] else {
            panic!("expected Property")
        };
        assert_eq!(gc.ctx().atom_table.bytes(g.kind.get()), b"get");
        let Node::FunctionExpression(gf) = g.value else {
            panic!("expected FunctionExpression")
        };
        assert!(gf.return_type.is_some(), "getter return type");

        // set y(v: number): void {}
        let Node::Property(s) = props[2] else {
            panic!("expected Property")
        };
        assert_eq!(gc.ctx().atom_table.bytes(s.kind.get()), b"set");
        let Node::FunctionExpression(sf) = s.value else {
            panic!("expected FunctionExpression")
        };
        assert!(sf.return_type.is_some(), "setter return type");

        // get<T>(x) {} — a method NAMED "get" (the `<` routes to a method).
        let Node::Property(gm) = props[3] else {
            panic!("expected Property")
        };
        assert!(gm.method.get(), "get<T> is a method");
        let Node::Identifier(gm_key) = gm.key else {
            panic!("expected Identifier key")
        };
        assert_eq!(gc.ctx().atom_table.bytes(gm_key.name.get()), b"get");
        let Node::FunctionExpression(gmf) = gm.value else {
            panic!("expected FunctionExpression")
        };
        assert!(gmf.type_parameters.is_some(), "get<T> type params");
    }

    /// The P5.4 class-element diagnostics keep the exact C++ texts.
    #[test]
    fn flow_p54_errors() {
        use ast::context::Context;
        use support::diag::{CollectingHandler, DiagKind};
        use support::manager::SourceErrorManager;

        let assert_error = |src: &[u8], expected: &str| {
            let mut sm = SourceErrorManager::new();
            let mut ctx = Context::new();
            ctx.set_parse_flow(true);
            let gc = ctx.lock();
            let atoms = &gc.ctx().atom_table;
            let _ = parse_with_collector(&gc, &mut sm, atoms, src);
            let h = sm.handler_as::<CollectingHandler>().unwrap();
            assert!(
                h.messages()
                    .iter()
                    .any(|m| m.kind == DiagKind::Error && m.message == expected),
                "expected {:?}, got {:?}",
                expected,
                h.messages().iter().map(|m| &m.message).collect::<Vec<_>>()
            );
        };

        // C++ JSParserImpl.cpp:5619-5626.
        assert_error(
            b"class C { get x<T>() { return 1; } }",
            "accessor method may not have type parameters",
        );
        // C++ JSParserImpl.cpp:5670-5672 (variance is only valid on fields).
        assert_error(b"class C { +m() {} }", "Unexpected variance sigil");
    }

    /// No-leak spot checks: with Flow parsing DISABLED the new sites must not
    /// consume type syntax (each input still errors, exactly as hermesc does
    /// without `-parse-flow`).
    #[test]
    fn flow_p54_no_leak() {
        assert_parse_has_errors(
            b"class C extends B<T> {}",
            "super type args need Flow",
        );
        assert_parse_has_errors(
            b"function f(): T { return 1; }",
            "return type needs Flow",
        );
        assert_parse_has_errors(b"var a: T;", "binding annotation needs Flow");
        assert_parse_has_errors(
            b"var o = { m<T>() {} };",
            "object-method type params need Flow",
        );
    }
}
