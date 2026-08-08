/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Expression parsing for the JS parser. Port of the expression-parsing
//! section of `lib/Parser/JSParserImpl.cpp`.

use ast::context::GCLock;
use ast::node::{
    ArrayExpression, ArrowFunctionExpression, ArrayPattern, AsConstExpression, AsExpression,
    AssignmentExpression, AssignmentPattern,
    AwaitExpression, BigIntLiteral, BinaryExpression, BooleanLiteral, CallExpression,
    ConditionalExpression, CoverEmptyArgs, CoverRestElement, CoverTrailingComma,
    CoverTypedIdentifier, TypeCastExpression,
    CoverInitializer, Empty, FunctionExpression, Identifier, ImportExpression,
    LogicalExpression, MemberExpression,
    MetaProperty,
    NewExpression, Node, NullLiteral, NumericLiteral, ObjectExpression, ObjectPattern,
    OptionalCallExpression, OptionalMemberExpression, PrivateName, Property, RegExpLiteral,
    RestElement, SequenceExpression, SpreadElement, StringLiteral, Super, TaggedTemplateExpression,
    TemplateElement, TemplateLiteral, ThisExpression, TSAsExpression, TSTypeAssertion,
    UnaryExpression, UpdateExpression,
    YieldExpression,
};
use ast::node_child::{NodeList, NodeMetadata};
use support::location::SMLoc;

use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

use super::flow::AllowAnonFunctionType;
use super::flow::{AllowTypedArrowFunction, CoverTypedParameters};
use super::pre_lazy::{ParserPass, PreParsedFunctionInfo};
use super::{
    IsClassHeritageArgument, IsConstructorCall, JSParserImpl, Param, PARAM_IN, PARAM_RETURN,
    PARAM_TAGGED,
};

/// Whether the identifier `of` ends an AssignmentExpression. Faithful port of
/// the C++ `enum class OfEndsAssignment { No, Yes };` (JSParserImpl.h). Passed
/// to `check_end_assignment_expression`: `Yes` in the ordinary assignment
/// chain, `No` inside `parse_yield_expression` (where `yield of;` should yield
/// a variable called `of`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum OfEndsAssignment {
    No,
    Yes,
}

// For AssignState.op field type (interned operator label).
use atom_table;
use atom_table::INVALID_ATOM_BYTES;

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    // -----------------------------------------------------------------------
    // parseExpression — 6552 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a comma-separated sequence of assignment expressions, building
    /// a SequenceExpression for 2+ operands. Port of
    /// `JSParserImpl::parseExpression` (lines 6552-6609).
    ///
    /// The comma loop handles the cover-grammar tails used by the arrow-function
    /// parameter cover: a trailing `,)` produces a `CoverTrailingComma` node, and
    /// `,...x` produces a `CoverRestElement`. These cover nodes survive into the
    /// resulting `SequenceExpression`; only `reparse_arrow_parameters` later
    /// converts them into real parameters. (When the sequence is *not* followed
    /// by `=>`, the cover nodes simply remain in the AST — matching hermesc.)
    pub(super) fn parse_expression(
        &mut self,
        param: Param,
        cover_typed_parameters: CoverTypedParameters,
    ) -> Option<&'gc Node<'gc>> {
        let start_loc = self.cur_start();
        // C++ 6556-6561: first operand threads `coverTypedParameters`.
        let opt_expr = self.parse_assignment_expression(
            param,
            false,
            AllowTypedArrowFunction::Yes,
            cover_typed_parameters,
            None,
        )?;

        if !self.check(TokenKind::comma) {
            return Some(opt_expr);
        }

        // Build a SequenceExpression.
        let mut expr_nodes: Vec<&'gc Node<'gc>> = vec![opt_expr];

        while self.check(TokenKind::comma) {
            // Eat the ",".
            let comma_rng = self.advance(GrammarContext::AllowRegExp);

            // CoverParenthesizedExpressionAndArrowParameterList: (Expression ,)
            // C++ lines 6575-6583.
            if self.check(TokenKind::r_paren) {
                let cur_start = self.cur_start();
                let node = Node::CoverTrailingComma(CoverTrailingComma::new(
                    NodeMetadata::new(self.dummy_range()),
                ));
                let cover = self.set_location(comma_rng.start, cur_start, node);
                expr_nodes.push(cover);
                break;
            }

            // C++ lines 6585-6600.
            let expr2 = if self.check(TokenKind::dotdotdot) {
                let rest = self.parse_binding_rest_element(param)?;
                let rest_range = rest.range();
                let node = Node::CoverRestElement(CoverRestElement::new(
                    NodeMetadata::new(self.dummy_range()),
                    rest,
                ));
                self.set_location(rest_range.start, rest_range.end, node)
            } else {
                // C++ 6596: parseAssignmentExpression(param) — defaults.
                self.parse_assignment_expression(
                    param,
                    false,
                    AllowTypedArrowFunction::Yes,
                    CoverTypedParameters::Yes,
                    None,
                )?
            };
            expr_nodes.push(expr2);
        }

        let end_loc = self.lexer.prev_token_end();
        let list = NodeList::from_iter(self.gc, expr_nodes);
        let node = Node::SequenceExpression(SequenceExpression::new(
            NodeMetadata::new(self.dummy_range()),
            list,
        ));
        Some(self.set_location(start_loc, end_loc, node))
    }

    // -----------------------------------------------------------------------
    // parseAssignmentExpression — P1.5
    // -----------------------------------------------------------------------

    /// True if the current token is any assignment operator. Port of
    /// `JSParserImpl::checkAssign` (lib/Parser/JSParserImpl.cpp 273-291).
    ///
    /// The 16 compound-assignment operators + plain `=`. In C++ this is a
    /// variadic `checkN(…)` call; in Rust we use `matches!`, which is the
    /// idiomatic zero-overhead equivalent.
    #[inline]
    fn check_assign(&self) -> bool {
        matches!(
            self.cur_kind(),
            TokenKind::equal
                | TokenKind::starequal
                | TokenKind::slashequal
                | TokenKind::percentequal
                | TokenKind::plusequal
                | TokenKind::minusequal
                | TokenKind::lesslessequal
                | TokenKind::greatergreaterequal
                | TokenKind::greatergreatergreaterequal
                | TokenKind::starstarequal
                | TokenKind::pipepipeequal
                | TokenKind::ampampequal
                | TokenKind::questionquestionequal
                | TokenKind::ampequal
                | TokenKind::caretequal
                | TokenKind::pipeequal
        )
    }

    /// True if the current token can legally follow an AssignmentExpression.
    /// Port of `JSParserImpl::checkEndAssignmentExpression` (lines 293-306).
    ///
    /// The "of" check mirrors C++ `checkUnescaped(ofIdent_)`: only fire when
    /// the current token is a plain identifier that spells "of" byte-for-byte
    /// (no `\u` escapes), and only when `of_ends_assignment == Yes`. In P1 we
    /// don't track the "no-escape" flag here, but the identifier parser interns
    /// unescaped identifiers normally, so we just compare the interned bytes to
    /// `b"of"`.
    #[inline]
    fn check_end_assignment_expression(
        &self,
        of_ends_assignment: OfEndsAssignment,
    ) -> bool {
        if matches!(
            self.cur_kind(),
            TokenKind::rw_in
                | TokenKind::r_paren
                | TokenKind::r_brace
                | TokenKind::r_square
                | TokenKind::comma
                | TokenKind::semi
                | TokenKind::colon
                | TokenKind::eof
        ) {
            return true;
        }
        // (ofEndsAssignment == OfEndsAssignment::Yes && checkUnescaped(ofIdent_)):
        // identifier spelled "of".
        if of_ends_assignment == OfEndsAssignment::Yes
            && self.cur_kind() == TokenKind::identifier
        {
            let bytes = self
                .lexer
                .get_string_table()
                .bytes(self.lexer.token().get_identifier());
            if bytes == b"of" {
                return true;
            }
        }
        self.lexer.is_new_line_before_current_token()
    }

    /// Parse a `yield` expression. Port of
    /// `JSParserImpl::parseYieldExpression` (lines 4652-4686).
    ///
    /// Only reachable when `param_yield` is set (inside a generator body).
    pub(super) fn parse_yield_expression(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 4654-4657: must start with the `yield` keyword/identifier.
        debug_assert!(
            self.param_yield.get()
                && self.check2(TokenKind::rw_yield, TokenKind::identifier)
                && self
                    .lexer
                    .get_string_table()
                    .bytes(self.lexer.token().get_res_word_or_identifier())
                    == b"yield",
            "yield expression must start with 'yield'"
        );
        // C++ 4658: SMRange yieldLoc = advance();
        let yield_loc = self.advance(GrammarContext::AllowRegExp);

        // C++ 4660-4670:
        //   if (check(semi) || checkEndAssignmentExpression(OfEndsAssignment::No))
        if self.check(TokenKind::semi)
            || self.check_end_assignment_expression(OfEndsAssignment::No)
        {
            // 'of' doesn't end the assignment expression in a yield.
            //    yield of;
            //          ^
            // is a valid position here and should simply yield a variable
            // called 'of'.
            return Some(self.set_location(
                yield_loc.start,
                yield_loc.end,
                Node::YieldExpression(YieldExpression::new(
                    NodeMetadata::new(self.dummy_range()),
                    None,
                    false,
                )),
            ));
        }

        // C++ 4672: bool delegate = checkAndEat(TokenKind::star);
        let delegate =
            self.check_and_eat(TokenKind::star, GrammarContext::AllowRegExp);

        // C++ 4674-4680: parse the argument. The simplified Rust signature only
        // takes `param`, so the C++ eagerly/AllowTypedArrowFunction/
        // CoverTypedParameters args are not threaded.
        let arg = self.parse_assignment_expression(param.get(PARAM_IN), false, AllowTypedArrowFunction::Yes, CoverTypedParameters::Yes, None)?;

        // C++ 4682-4685: setLocation(yieldLoc, getPrevTokenEndLoc(), node).
        let end = self.lexer.prev_token_end();
        Some(self.set_location(
            yield_loc.start,
            end,
            Node::YieldExpression(YieldExpression::new(
                NodeMetadata::new(self.dummy_range()),
                Some(arg),
                delegate,
            )),
        ))
    }

    /// Parse an assignment expression. Port of
    /// `JSParserImpl::parseAssignmentExpression` (lines 6233-6551).
    ///
    /// ## Structure
    ///
    /// The C++ uses a `State` stack + `parseHelper` closure to build
    /// right-associative chains (`a = b = c` → `a = (b = c)`) without
    /// deep recursion.  In Rust, the closure's mutable-state-by-reference
    /// pattern conflicts with `&mut self`, so we inline the logic directly:
    /// each loop iteration runs the "parseHelper" body, pushes a completed
    /// `AssignState` entry, then returns or recurses.  The fold pass runs
    /// afterwards, mirroring C++ lines 6528-6547.
    ///
    /// ## parseHelper return — `Option<Option<&'gc Node<'gc>>>`
    ///
    /// The C++ closure returns `Optional<Node*>`:
    /// - `None`      = parse error; propagate failure.
    /// - `Some(nullptr)` = assignment operator consumed; state.op is set;
    ///   the driver must recurse for the RHS.
    /// - `Some(node_ptr)` = terminal result (not an assignment op).
    ///
    /// We encode this as `Option<Option<&'gc Node>>`:
    /// - `None`            = error.
    /// - `Some(None)`      = operator consumed, continue the chain.
    /// - `Some(Some(n))`   = terminal node.
    ///
    /// ## Sub-productions
    /// - `yield` (P3.2) and `=>` arrow functions (P3.3) are parsed inline.
    /// - Destructuring-assignment reparse (ArrayExpression/ObjectExpression LHS)
    ///   is handled by `reparse_assignment_pattern` (P1.8b).
    /// - Flow typed arrows (`<T>(…) => …`), the return-type/predicate
    ///   backtrack, and typed async arrows are handled inline (P6.1; gated on
    ///   `parse_flow`). The TS return-type backtrack is a parallel block
    ///   gated on `parse_ts` (P7.5b).
    ///
    /// ## MAX_NESTED_ASSIGNMENTS
    /// `ESTree::MAX_NESTED_ASSIGNMENTS = 30000` (include/hermes/AST/ESTree.h:1407).
    pub(super) fn parse_assignment_expression(
        &mut self,
        param: Param,
        force_eagerly: bool,
        allow_typed_arrow_function: AllowTypedArrowFunction,
        cover_typed_parameters: CoverTypedParameters,
        type_params: Option<&'gc Node<'gc>>,
    ) -> Option<&'gc Node<'gc>> {
        use crate::token_kinds::token_kind_str;

        /// Maximum right-assoc assignment chain depth.
        /// Mirrors `ESTree::MAX_NESTED_ASSIGNMENTS` (ESTree.h:1407).
        const MAX_NESTED_ASSIGNMENTS: usize = 30000;

        /// One frame of the right-associative assignment chain.
        /// Mirrors C++ `State` inside `parseAssignmentExpression`.
        struct AssignState<'gc> {
            /// Start of the LHS expression (C++ `leftStartLoc`).
            left_start_loc: SMLoc,
            /// The already-parsed LHS (C++ `optLeftExpr`).
            opt_left_expr: &'gc Node<'gc>,
            /// The interned operator token string (C++ `op`).
            op: atom_table::AtomBytes,
            /// Start of the operator token (C++ `debugLoc`).
            debug_loc: SMLoc,
        }

        // Stack of in-progress assignment levels.  C++ uses SmallVector<State,2>.
        let mut stack: Vec<AssignState<'gc>> = Vec::new();

        // -------------------------------------------------------------------
        // "parseHelper" body — inlined (C++ lines 6249-6493).
        //
        // Runs one level: parses the conditional LHS, checks for an assignment
        // operator, and if found pushes a frame and signals "continue".
        //
        // Return value: `Option<Option<&'gc Node>>` (see doc above).
        //
        // We encode the return with a helper enum to avoid macro-label issues
        // or separate-function borrow fights.
        // -------------------------------------------------------------------
        enum LevelResult<'gc> {
            /// Parse error — propagate.
            Error,
            /// Terminal node (not an assignment op, or yield/arrow handled).
            Terminal(&'gc Node<'gc>),
            /// Assignment operator consumed; frame pushed onto stack.
            Continue,
        }

        // Execute one "parseHelper" pass with the given `param`.
        // Pushes a frame to `stack` if an operator was found and consumed,
        // or returns Error/Terminal otherwise.
        let run_level = |this: &mut Self,
                              stack: &mut Vec<AssignState<'gc>>,
                              cur_param: Param,
                              allow_typed_arrow_function: AllowTypedArrowFunction,
                              cover_typed_parameters: CoverTypedParameters,
                              mut type_params: Option<&'gc Node<'gc>>|
         -> LevelResult<'gc> {
            // ----------------------------------------------------------------
            // yield check (C++ 6257-6268).
            //   if (paramYield_ && check(rw_yield, identifier) &&
            //       tok_->getResWordOrIdentifier() == yieldIdent_) {
            //     auto ret = parseYieldExpression(param);
            //     if (!ret) return None;
            //     return *ret;
            //   }
            // A successful yield expression is the completed level result; we
            // return it as `Terminal` (the same channel the closure uses to hand
            // back a finished expression).
            // ----------------------------------------------------------------
            if this.param_yield.get()
                && (this.check(TokenKind::rw_yield)
                    || (this.check(TokenKind::identifier)
                        && this
                            .lexer
                            .get_string_table()
                            .bytes(this.lexer.token().get_identifier())
                            == b"yield"))
            {
                return match this.parse_yield_expression(cur_param) {
                    Some(node) => LevelResult::Terminal(node),
                    None => LevelResult::Error,
                };
            }

            // Async arrow detection (C++ 6270-6286).
            //   async x => …   — `async` followed by an identifier with no line
            //   terminator forces async-arrow parsing. `start_loc` records the
            //   `async` keyword (or the LHS start) for the final arrow location.
            let start_loc = this.cur_start();
            let mut force_async = false;
            if this.check_unescaped_name(b"async") {
                // C++: lexer_.lookahead1(TokenKind::identifier).
                let opt_next =
                    this.lexer.lookahead1::<true>(Some(TokenKind::identifier));
                if opt_next == Some(TokenKind::identifier) {
                    force_async = true;
                }
                // Flow typed async arrow (C++ 6277-6285). When `async` is
                // followed by `<` or `(`, speculatively try a typed async arrow
                // function. Tri-state: `Some(node)` commits; `None` falls back
                // to the normal async handling below.
                if this.parse_flow()
                    && (opt_next == Some(TokenKind::less)
                        || opt_next == Some(TokenKind::l_paren))
                {
                    if let Some(async_arrow) =
                        this.try_parse_typed_async_arrow_function(cur_param)
                    {
                        return LevelResult::Terminal(async_arrow);
                    }
                }
            }

            // Flow type-param head `<T>(…) => …` (C++ 6288-6339).
            if this.parse_flow()
                && allow_typed_arrow_function == AllowTypedArrowFunction::Yes
                && type_params.is_none()
                && this.check(TokenKind::less)
            {
                let sp = this.lexer.save_point();
                // C++ CollectMessagesRAII collect{&sm_, true}: defer messages,
                // commit on success / discard on rollback.
                let prev = this.lexer.get_source_mgr_mut().begin_collecting();
                // Do as the flow parser does due to JSX ambiguities. First try
                // parsing as an assignment expression disallowing typed arrow
                // functions; if that works, return it directly (C++ 6300-6309).
                let opt_assign = this.parse_assignment_expression(
                    cur_param,
                    false,
                    AllowTypedArrowFunction::No,
                    CoverTypedParameters::No,
                    None,
                );
                if let Some(assign) = opt_assign {
                    // That worked, commit the collected messages.
                    this.lexer.get_source_mgr_mut().end_collecting(prev, false);
                    return LevelResult::Terminal(assign);
                } else {
                    // Consume the type parameters and try again (C++ 6311-6336).
                    this.lexer.get_source_mgr_mut().end_collecting(prev, true);
                    sp.restore(&mut this.lexer);
                    // The Rust `SavePoint::restore` consumes `self`; C++ reuses
                    // one SavePoint and calls `.restore()` again on the bail
                    // paths below. We are back at `<` after the restore above, so
                    // re-snapshot here for the possible second restore.
                    let sp2 = this.lexer.save_point();
                    let opt_type_params = this.parse_type_params_flow();
                    // Type parameters must be followed by a '(' to be meaningful.
                    if let Some(tp) = opt_type_params {
                        if this.check(TokenKind::l_paren) {
                            type_params = Some(tp);
                            let opt_assign = this.parse_assignment_expression(
                                cur_param,
                                false,
                                AllowTypedArrowFunction::Yes,
                                CoverTypedParameters::No,
                                type_params,
                            );
                            if let Some(assign) = opt_assign {
                                // We've got the arrow function now.
                                return LevelResult::Terminal(assign);
                            } else {
                                // That's everything we can try.
                                let tp_range = tp.range();
                                this.error_at(
                                    tp_range,
                                    "type parameters must be used in an \
                                     arrow function expression",
                                );
                                return LevelResult::Error;
                            }
                        } else {
                            // Invalid type params, and also invalid JSX. Bail.
                            sp2.restore(&mut this.lexer);
                        }
                    } else {
                        // Invalid type params, and also invalid JSX. Bail.
                        sp2.restore(&mut this.lexer);
                    }
                }
            }

            // C++ lines 6341-6345: leftStartLoc / hasNewLine / optLeftExpr.
            let left_start_loc = this.cur_start();
            let has_new_line = this.lexer.is_new_line_before_current_token();
            let left_expr = match this
                .parse_conditional_expression(cur_param, cover_typed_parameters)
            {
                Some(e) => e,
                None => return LevelResult::Error,
            };

            // Flow return-type / predicate backtracking (C++ 6349-6402).
            let mut return_type: Option<&'gc Node<'gc>> = None;
            let mut predicate: Option<&'gc Node<'gc>> = None;
            if this.parse_flow()
                && allow_typed_arrow_function == AllowTypedArrowFunction::Yes
                && (left_expr.metadata().parens.get() != 0
                    || matches!(left_expr, Node::CoverEmptyArgs(_)))
                && this.check(TokenKind::colon)
            {
                let sp = this.lexer.save_point();
                // Defer our decision on whether to show or suppress messages.
                // On failure we may need to lex JSX children instead of function
                // type parameters, so messages are buffered (C++ 6369).
                let prev = this.lexer.get_source_mgr_mut().begin_collecting();
                let annot_start =
                    this.advance(GrammarContext::Type).start;
                let starts_with_predicate = this.check_name(b"%checks");
                let opt_type = if starts_with_predicate {
                    None
                } else {
                    this.parse_return_type_annotation_flow(
                        Some(annot_start),
                        AllowAnonFunctionType::No,
                    )
                };
                if let Some(t) = opt_type {
                    return_type = Some(t);
                }
                if opt_type.is_some() || starts_with_predicate {
                    if this.check(TokenKind::equalgreater) {
                        // Done parsing the return type and predicate.
                        // Successful parse, show buffered messages.
                        this.lexer
                            .get_source_mgr_mut()
                            .end_collecting(prev, false);
                    } else if this.check_name(b"%checks") {
                        let opt_pred = this.parse_predicate_flow();
                        if opt_pred.is_some()
                            && this.check(TokenKind::equalgreater)
                        {
                            predicate = opt_pred;
                            this.lexer
                                .get_source_mgr_mut()
                                .end_collecting(prev, false);
                        } else {
                            this.lexer
                                .get_source_mgr_mut()
                                .end_collecting(prev, true);
                            return_type = None;
                            predicate = None;
                            sp.restore(&mut this.lexer);
                        }
                    } else {
                        this.lexer
                            .get_source_mgr_mut()
                            .end_collecting(prev, true);
                        return_type = None;
                        sp.restore(&mut this.lexer);
                    }
                } else {
                    this.lexer.get_source_mgr_mut().end_collecting(prev, true);
                    sp.restore(&mut this.lexer);
                }
            }

            // TS return-type backtracking (C++ 6405-6444): a separate
            // `#if HERMES_PARSE_TS` sibling block. Simpler than Flow — no
            // predicates — but the same `: RetType =>` cover-typed-arrow shape.
            if this.parse_ts()
                && allow_typed_arrow_function == AllowTypedArrowFunction::Yes
                && (left_expr.metadata().parens.get() != 0
                    || matches!(left_expr, Node::CoverEmptyArgs(_)))
                && this.check(TokenKind::colon)
            {
                let sp = this.lexer.save_point();
                // Defer the show/suppress decision: on failure we may need to
                // lex JSX children instead of function type params (C++ 6417).
                let prev = this.lexer.get_source_mgr_mut().begin_collecting();
                let annot_start = this.advance(GrammarContext::Type).start;
                let opt_type =
                    this.parse_type_annotation_ts(Some(annot_start));
                if let Some(t) = opt_type {
                    return_type = Some(t);
                }
                if opt_type.is_some() {
                    if this.check(TokenKind::equalgreater) {
                        // Done parsing the return type. Show buffered messages.
                        this.lexer
                            .get_source_mgr_mut()
                            .end_collecting(prev, false);
                    } else {
                        this.lexer
                            .get_source_mgr_mut()
                            .end_collecting(prev, true);
                        return_type = None;
                        sp.restore(&mut this.lexer);
                    }
                } else {
                    this.lexer.get_source_mgr_mut().end_collecting(prev, true);
                    sp.restore(&mut this.lexer);
                }
            }

            // ----------------------------------------------------------------
            // Arrow check (C++ 6453-6466).
            //   ArrowFunction : ArrowParameters [no line terminator] =>
            //   ConciseBody.
            // A successful arrow is the completed level result; return it via
            // the `Terminal` channel (same as yield).
            // ----------------------------------------------------------------
            if this.check(TokenKind::equalgreater)
                && !this.lexer.is_new_line_before_current_token()
            {
                // C++ 6463: typeParams ? typeParams->getStartLoc() : startLoc.
                let arrow_start = match type_params {
                    Some(tp) => tp.range().start,
                    None => start_loc,
                };
                // Forward the caller's eager flag. In the eager port this is
                // inert except when `parse_lazy_function` reparses an arrow
                // body (cpp:7565-7566).
                return match this.parse_arrow_function_expression(
                    cur_param,
                    force_eagerly,
                    left_expr,
                    has_new_line,
                    type_params,
                    return_type,
                    predicate,
                    arrow_start,
                    allow_typed_arrow_function,
                    force_async,
                ) {
                    Some(node) => LevelResult::Terminal(node),
                    None => LevelResult::Error,
                };
            }

            // Flow typeParams error (C++ 6468-6477): generic type parameters were
            // parsed but no `=>` arrow follows.
            if type_params.is_some() {
                let range = this.cur_range();
                this.error_at(
                    range,
                    "'=>' expected in generic arrow function",
                );
                return LevelResult::Error;
            }

            // C++ line 6479: if (!checkAssign()) return *state.optLeftExpr;
            if !this.check_assign() {
                return LevelResult::Terminal(left_expr);
            }

            // ----------------------------------------------------------------
            // Destructuring reparse (C++ 6483-6489).
            // When the LHS is an ArrayExpression or ObjectExpression and the
            // operator is `=`, reparse the LHS as a destructuring pattern.
            // ----------------------------------------------------------------
            if this.check(TokenKind::equal)
                && matches!(
                    left_expr,
                    Node::ArrayExpression(_) | Node::ObjectExpression(_)
                )
            {
                match this.reparse_assignment_pattern(left_expr, false) {
                    Some(pattern) => {
                        // Replace left_expr with the reparsed pattern in this
                        // level's frame. We push the frame now (operator and RHS
                        // will follow in the stack-fold phase).
                        let op_kind = this.cur_kind();
                        let op = this
                            .gc
                            .ctx()
                            .atom_table
                            .atom_bytes(token_kind_str(op_kind).as_bytes());
                        let debug_loc = this.advance(GrammarContext::AllowRegExp).start;
                        stack.push(AssignState {
                            left_start_loc,
                            opt_left_expr: pattern,
                            op,
                            debug_loc,
                        });
                        return LevelResult::Continue;
                    }
                    None => return LevelResult::Error,
                }
            }

            // C++ line 6491-6493:
            //   state.op = getTokenIdent(tok_->getKind());
            //   state.debugLoc = advance().Start;
            //   return nullptr;  (→ Some(None) in our encoding)
            let op_kind = this.cur_kind();
            let op = this
                .gc
                .ctx()
                .atom_table
                .atom_bytes(token_kind_str(op_kind).as_bytes());
            let debug_loc = this.advance(GrammarContext::AllowRegExp).start;

            stack.push(AssignState {
                left_start_loc,
                opt_left_expr: left_expr,
                op,
                debug_loc,
            });
            LevelResult::Continue
        };

        // -------------------------------------------------------------------
        // Driver — C++ lines 6496-6524.
        //
        // Push a State, call parseHelper; if error → None; if terminal → break;
        // else push new State and loop.
        // -------------------------------------------------------------------
        let opt_res: &'gc Node<'gc> = loop {
            // First level uses the incoming Flow params; subsequent RHS levels
            // use AllowTypedArrowFunction::Yes / CoverTypedParameters::No / null
            // (C++ 6499-6523).
            let (lvl_allow, lvl_cover, lvl_type_params) = if stack.is_empty() {
                (
                    allow_typed_arrow_function,
                    cover_typed_parameters,
                    type_params,
                )
            } else {
                (
                    AllowTypedArrowFunction::Yes,
                    CoverTypedParameters::No,
                    None,
                )
            };
            match run_level(
                self,
                &mut stack,
                param,
                lvl_allow,
                lvl_cover,
                lvl_type_params,
            ) {
                LevelResult::Error => return None,
                LevelResult::Terminal(n) => break n,
                LevelResult::Continue => {
                    // C++ line 6513: stack.size() > MAX_NESTED_ASSIGNMENTS
                    // guard, whose body (cpp:6514) is a bare
                    // `recursionDepthExceeded()` call — so the diagnostic is
                    // that function's: `error(tok_->getStartLoc(), ...)`
                    // (cpp:348-352), the point overload
                    // (JSParserImpl.h:472-474), rendering a bare caret.
                    if stack.len() > MAX_NESTED_ASSIGNMENTS {
                        let loc = self.cur_start();
                        self.error_at_loc(
                            loc,
                            "Too many nested expressions/statements/declarations",
                        );
                        return None;
                    }
                    // Loop to parse the RHS of the assignment operator.
                }
            }
        };

        // -------------------------------------------------------------------
        // Fold phase — C++ lines 6528-6547.
        //
        // Drain the stack right-associatively, building AssignmentExpression
        // nodes.  `opt_res` is the innermost (rightmost) expression; we fold
        // it into each level's left side, from bottom of stack outward.
        // -------------------------------------------------------------------
        let mut opt_res = opt_res;
        while let Some(top) = stack.pop() {
            // C++ line 6529: checkEndAssignmentExpression() guard.
            if !self.check_end_assignment_expression(OfEndsAssignment::Yes) {
                let range = self.cur_range();
                self.error_at(
                    range,
                    "unexpected token after assignment expression",
                );
                return None;
            }
            let end = self.lexer.prev_token_end();
            // C++ line 6540-6545: new AssignmentExpressionNode(op, left, right).
            // AssignmentExpression::new(metadata, operator, left, right).
            let node = Node::AssignmentExpression(AssignmentExpression::new(
                NodeMetadata::new(self.dummy_range()),
                top.op,
                top.opt_left_expr,
                opt_res,
            ));
            // C++ setLocation(leftStartLoc, getPrevTokenEndLoc(), debugLoc, node).
            opt_res = self.set_location_d(top.left_start_loc, end, top.debug_loc, node);
        }

        Some(opt_res)
    }

    // -----------------------------------------------------------------------
    // reparseArrowParameters — 5681 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Convert an already-parsed cover expression into an arrow-function
    /// parameter list, appending each parameter node to `param_list`. Port of
    /// `JSParserImpl::reparseArrowParameters` (lines 5681-5816). Returns false on
    /// a hard error.
    ///
    /// ## Immutable-AST adaptation
    /// The C++ does `std::move(seqNode->_expressions)` / `std::move(callNode->
    /// _arguments)` to steal the children out of the cover node. Our AST children
    /// are immutable (`&'gc Node`), so we instead ITERATE the existing `NodeList`
    /// (read the children) into a fresh `Vec` and process that — no mutation.
    /// Fresh `RestElement`/`AssignmentPattern` nodes are built exactly as C++.
    pub(super) fn reparse_arrow_parameters(
        &mut self,
        node: &'gc Node<'gc>,
        has_new_line: bool,
        param_list: &mut Vec<&'gc Node<'gc>>,
        is_async: &mut bool,
    ) -> bool {
        // Empty argument list "()". C++ 5686-5688.
        if node.metadata().parens.get() == 0
            && matches!(node, Node::CoverEmptyArgs(_))
        {
            return true;
        }

        // A single identifier without parens. C++ 5690-5698.
        if node.metadata().parens.get() == 0 {
            if let Node::Identifier(ident) = node {
                param_list.push(node);
                let range = node.range();
                let name_bytes = self
                    .gc
                    .ctx()
                    .atom_table
                    .bytes(ident.name.get())
                    .to_owned();
                return self.validate_binding_identifier(
                    range,
                    &name_bytes,
                    TokenKind::identifier,
                );
            }
        }

        // The list of cover sub-expressions to reparse (C++ `nodeList`).
        let node_list: Vec<&'gc Node<'gc>>;

        // C++ 5702-5732.
        if let Node::CallExpression(call_node) = node {
            // Async function parameters look like call expressions. For example:
            // async(x,y)
            // It must have no surrounding parens and the name must be 'async'.
            // It must also not already be `async`, because the CallExpression
            // determines whether it is `async`.
            // It must not have a newline between 'async' and the parameters.
            // Set `isAsync = true` to indicate that this was async.
            // C++ 5702-5719.
            let callee_is_async = if let Node::Identifier(callee) =
                call_node.callee
            {
                let callee_range = call_node.callee.range();
                let callee_bytes = self
                    .gc
                    .ctx()
                    .atom_table
                    .bytes(callee.name.get())
                    .to_owned();
                // callee->_name == asyncIdent_ &&
                // isUnescaped(callee->_name, callee->getSourceRange())
                let unescaped = (callee_range.end.offset
                    - callee_range.start.offset)
                    as usize
                    == callee_bytes.len();
                callee_bytes == b"async" && unescaped
            } else {
                false
            };
            if !*is_async
                && node.metadata().parens.get() == 0
                && callee_is_async
                && !has_new_line
            {
                node_list = call_node.arguments.iter().collect();
                *is_async = true;
            } else {
                let range = node.range();
                self.error_at(range, "invalid arrow function parameter list");
                return false;
            }
        } else {
            // C++ 5720-5732.
            if node.metadata().parens.get() != 1 {
                let range = node.range();
                self.error_at(range, "invalid arrow function parameter list");
                return false;
            }

            if let Node::SequenceExpression(seq_node) = node {
                node_list = seq_node.expressions.iter().collect();
            } else {
                node.metadata().parens.set(0);
                node_list = vec![node];
            }
        }

        // C++ 5734: paramAwait_ = paramAwait_ || isAsync (RAII).
        let _save_param_await =
            self.save_param_await(self.param_await.get() || *is_async);

        let list_len = node_list.len();
        // C++ 5746-5813.
        for (idx, expr0) in node_list.into_iter().enumerate() {
            let is_last = idx == list_len - 1;
            let mut expr = expr0;

            // checkParens (C++ 5738-5744, 5750-5751).
            if expr.metadata().parens.get() != 0 {
                let range = expr.range();
                self.error_at(
                    range,
                    "parentheses are not allowed around parameters",
                );
                continue;
            }

            // CoverRestElement. C++ 5753-5759.
            if let Node::CoverRestElement(cre) = expr {
                if !is_last {
                    let range = expr.range();
                    self.error_at(range, "rest parameter must be last");
                } else {
                    param_list.push(cre.rest);
                }
                continue;
            }

            // SpreadElement (async arrow heads parse rest as SpreadElement).
            // C++ 5761-5770.
            if let Node::SpreadElement(spread) = expr {
                if !is_last {
                    let range = expr.range();
                    self.error_at(range, "rest parameter must be last");
                } else {
                    // C++ 5767-5768 builds a fresh RestElement with NO
                    // setLocation, so its source range stays invalid and the
                    // dumper omits loc/range. Use `invalid_range()` to match.
                    let node = Node::RestElement(RestElement::new(
                        NodeMetadata::new(self.invalid_range()),
                        spread.argument,
                    ));
                    let rest = self.gc.alloc(node);
                    param_list.push(rest);
                }
                continue;
            }

            // CoverTrailingComma — just skip. C++ 5772-5778.
            if matches!(expr, Node::CoverTrailingComma(_)) {
                debug_assert!(
                    is_last,
                    "CoverTrailingComma should have been only parsed last"
                );
                continue;
            }

            // If we encounter an initializer, unpack it. C++ 5780-5792.
            let mut init: Option<&'gc Node<'gc>> = None;
            let mut asn_range: Option<support::location::SMRange> = None;
            if let Node::AssignmentExpression(asn) = expr {
                let eq_op = self.gc.ctx().atom_table.atom_bytes(b"=");
                if asn.operator.get() == eq_op {
                    asn_range = Some(expr.range());
                    expr = asn.left;
                    init = Some(asn.right);

                    if expr.metadata().parens.get() != 0 {
                        let range = expr.range();
                        self.error_at(
                            range,
                            "parentheses are not allowed around parameters",
                        );
                        continue;
                    }
                }
            }

            // reparseAssignmentPattern(expr, true). C++ 5794-5797.
            let opt_param = match self.reparse_assignment_pattern(expr, true) {
                Some(p) => p,
                None => continue,
            };
            expr = opt_param;

            // C++ 5799-5802.
            if let Some(init) = init {
                let r = asn_range.unwrap();
                let node = Node::AssignmentPattern(AssignmentPattern::new(
                    NodeMetadata::new(self.dummy_range()),
                    expr,
                    init,
                ));
                expr = self.set_location(r.start, r.end, node);
            }

            // C++ 5804-5810.
            if let Node::Identifier(ident) = expr {
                let range = expr.range();
                let name_bytes = self
                    .gc
                    .ctx()
                    .atom_table
                    .bytes(ident.name.get())
                    .to_owned();
                self.validate_binding_identifier(
                    range,
                    &name_bytes,
                    TokenKind::identifier,
                );
            }

            param_list.push(expr);
        }

        true
    }

    // -----------------------------------------------------------------------
    // parseArrowFunctionExpression — 5818 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse the body of an arrow function given the already-parsed cover
    /// parameters in `left_expr`. Port of
    /// `JSParserImpl::parseArrowFunctionExpression` (lines 5818-5911).
    ///
    /// The `pass_ == PreParse` block (cpp:5896-5908) is ported: when in
    /// `PreParse` mode, `parse_function_body` records the body info in the
    /// side-table (cpp:803-810). `force_eagerly` threads through to
    /// `parse_function_body` for the `LazyParse` skip logic.
    ///
    /// The Flow `type_params`/`return_type`/`predicate` arguments attach to the
    /// resulting `ArrowFunctionExpression`; `allow_typed_arrow` is threaded into
    /// the concise (expression) body parse (C++ 5872-5877).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn parse_arrow_function_expression(
        &mut self,
        param: Param,
        force_eagerly: bool,
        left_expr: &'gc Node<'gc>,
        has_new_line: bool,
        type_params: Option<&'gc Node<'gc>>,
        return_type: Option<&'gc Node<'gc>>,
        predicate: Option<&'gc Node<'gc>>,
        start_loc: SMLoc,
        allow_typed_arrow: AllowTypedArrowFunction,
        force_async: bool,
    ) -> Option<&'gc Node<'gc>> {
        // The C++ `SaveFunctionState` (5849) restores `strictMode` on scope
        // exit. A `"use strict"` directive in the (block) body must not leak
        // strictness to the enclosing code, so save/restore the lexer flag
        // around the body. Result computed first so restore runs on every path.
        let old_strict = self.lexer.is_strict_mode();
        // SaveFunctionState guard — mirrors C++ SaveFunctionState (cpp:5849).
        // is_arrow=true: sets containsArrowFunctions_ on the enclosing scope.
        let _g = self.save_function_state(true);
        let old_seen_len = self.seen_directives.len();
        let result = self.parse_arrow_function_expression_inner(
            param,
            force_eagerly,
            left_expr,
            has_new_line,
            type_params,
            return_type,
            predicate,
            start_loc,
            allow_typed_arrow,
            force_async,
        );
        self.seen_directives.truncate(old_seen_len);
        self.lexer.set_strict_mode(old_strict);
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_arrow_function_expression_inner(
        &mut self,
        param: Param,
        force_eagerly: bool,
        left_expr: &'gc Node<'gc>,
        has_new_line: bool,
        type_params: Option<&'gc Node<'gc>>,
        return_type: Option<&'gc Node<'gc>>,
        predicate: Option<&'gc Node<'gc>>,
        start_loc: SMLoc,
        allow_typed_arrow: AllowTypedArrowFunction,
        force_async: bool,
    ) -> Option<&'gc Node<'gc>> {
        // ArrowFunction : ArrowParameters [no line terminator] => ConciseBody.
        debug_assert!(
            self.check(TokenKind::equalgreater)
                && !self.lexer.is_new_line_before_current_token(),
            "ArrowFunctionExpression expects [no new line] '=>'"
        );

        // C++ 5834: argsParamAwait = forceAsync (RAII).
        let _save_args_param_await = self.save_param_await(force_async);

        // C++ 5836-5842.
        if !self.eat(
            TokenKind::equalgreater,
            GrammarContext::AllowRegExp,
            " in arrow function expression",
        ) {
            return None;
        }

        let mut is_async = force_async;
        let mut param_list: Vec<&'gc Node<'gc>> = Vec::new();
        // C++ 5846-5847.
        if !self.reparse_arrow_parameters(
            left_expr,
            has_new_line,
            &mut param_list,
            &mut is_async,
        ) {
            return None;
        }

        // `SaveFunctionState` (cpp:5849) is constructed in the outer wrapper
        // `parse_arrow_function_expression` (is_arrow=true), which covers both
        // the parameter reparse above and the body below.

        // C++ 5854-5855: paramYield_ = false; paramAwait_ = isAsync (RAII).
        let _save_body_param_yield = self.save_param_yield(false);
        let _save_body_param_await = self.save_param_await(is_async);

        let body;
        let expression;
        if self.check(TokenKind::l_brace) {
            // C++ 5856-5867.
            body = self.parse_function_body(
                Param::default(),
                force_eagerly,
                // oldParamYield.get() == false; argsParamAwait.get() == force_async.
                false,
                force_async,
                GrammarContext::AllowDiv,
                /* parse_directives= */ true,
            )?;
            expression = false;
        } else {
            // It's possible to recurse onto parseAssignmentExpression directly
            // and get stuck without a depth check if we don't have one here.
            // C++ 5868-5882.
            let _guard = self.check_recursion()?;
            // C++ 5872-5877: concise body threads `allowTypedArrowFunction`.
            body = self.parse_assignment_expression(
                param.get(PARAM_IN),
                // C++ 5874 passes forceEagerly=true: a concise (expression)
                // arrow body is never a lazy stub.
                true,
                allow_typed_arrow,
                CoverTypedParameters::No,
                None,
            )?;
            expression = true;
        }

        // C++ 5884-5894.
        let end = self.lexer.prev_token_end();
        let params = NodeList::from_iter(self.gc, param_list);
        let node =
            Node::ArrowFunctionExpression(ArrowFunctionExpression::new(
                NodeMetadata::new(self.dummy_range()),
                params,
                body,
                type_params,
                return_type,
                predicate,
                expression,
                is_async,
            ));
        let arrow = self.set_location(start_loc, end, node);

        // cpp:5896-5908 — record the arrow function in the PreParse side-table.
        // The C++ uses try_emplace + assert(inserted) because an arrow can only
        // appear once at a given source offset. We mirror the assert with a
        // debug_assert on the vacant-entry path. The C++ uses an AllocationScope
        // to discard the AST; Rust lets the GC arena reclaim nodes after the
        // PreParse GCLock is dropped.
        //
        // Collect side-table values before entering the HashMap entry API to
        // avoid overlapping (&self, &mut self) borrows.
        if self.pass == ParserPass::PreParse {
            use std::collections::hash_map::Entry;
            let key = start_loc.offset;
            let info = PreParsedFunctionInfo {
                end: body.range().end,
                strict_mode: self.lexer.is_strict_mode(),
                directives: self.copy_seen_directives(),
                contains_arrow_functions: self.contains_arrow_functions.get(),
                may_contain_arrow_functions_using_arguments: self
                    .may_contain_arrow_functions_using_arguments
                    .get(),
            };
            match self.pre_parsed.function_info.entry(key) {
                Entry::Vacant(e) => {
                    e.insert(info);
                }
                Entry::Occupied(_) => {
                    debug_assert!(
                        false,
                        "duplicate arrow start offset in PreParse table"
                    );
                }
            }
        }

        Some(arrow)
    }

    // -----------------------------------------------------------------------
    // validate_binding_identifier — P1.8b
    // -----------------------------------------------------------------------

    /// Validate a binding identifier: emit errors for `yield`/`await`/`let`
    /// in strict or param context. Port of
    /// `JSParserImpl::validateBindingIdentifier` (lines 1008-1044).
    ///
    /// Emits errors but does NOT stop progress. Returns true if `kind` is a
    /// legal binding identifier token kind (`identifier` or `rw_yield`).
    ///
    /// The borrow pattern: capture the comparison results into booleans BEFORE
    /// the `&mut self` error calls to avoid overlapping borrows.
    pub(super) fn validate_binding_identifier(
        &mut self,
        range: support::location::SMRange,
        id_bytes: &[u8],
        kind: TokenKind,
    ) -> bool {
        // Capture comparison results before any &mut self error call.
        let is_yield = id_bytes == b"yield";
        let is_await = id_bytes == b"await";
        let is_let = id_bytes == b"let";

        if is_yield && (self.lexer.is_strict_mode() || self.param_yield.get()) {
            self.error_at(range, "Unexpected usage of 'yield' as an identifier");
        }
        if is_await && self.param_await.get() {
            self.error_at(range, "Unexpected usage of 'await' as an identifier");
        }
        if is_let && self.lexer.is_strict_mode() {
            self.error_at(
                range,
                "Invalid use of strict mode reserved word as binding identifier",
            );
        }
        kind == TokenKind::identifier || kind == TokenKind::rw_yield
    }

    // -----------------------------------------------------------------------
    // reparse_assignment_pattern — P1.8b
    // -----------------------------------------------------------------------

    /// Reparse an expression node as a destructuring assignment pattern. Port
    /// of `JSParserImpl::reparseAssignmentPattern` (lines 5913-5988).
    ///
    /// ## Immutable-children adaptation
    /// The C++ mutates ArrayExpression/ObjectExpression in place. In Rust our
    /// AST nodes have immutable children (`&'gc Node<'gc>`), so we BUILD FRESH
    /// pattern nodes by reading the expression's data, not by mutating it.
    ///
    /// - `ArrayExpression` → `reparse_array_assignment_pattern`
    /// - `ObjectExpression` → `reparse_object_assignment_pattern`
    /// - `Identifier` → validate and return as-is
    /// - already a `PatternNode` → return as-is
    /// - Flow covers (`CoverTypedIdentifier`, `TypeCastExpression`) → rebuild
    ///   the target pattern/identifier with the carried type annotation (P6.1)
    /// - `in_decl=true` and no match → "identifier or pattern expected" error
    /// - Otherwise → return as-is (P1 callers always pass `in_decl=false`)
    pub(super) fn reparse_assignment_pattern(
        &mut self,
        node: &'gc Node<'gc>,
        in_decl: bool,
    ) -> Option<&'gc Node<'gc>> {
        // Only enter the reparse branches when the node has no parentheses.
        if node.metadata().parens.get() == 0 {
            if let Node::ArrayExpression(aen) = node {
                return self.reparse_array_assignment_pattern(aen);
            }
            if let Node::ObjectExpression(oen) = node {
                return self.reparse_object_assignment_pattern(oen);
            }
            if let Node::Identifier(ident) = node {
                // Validation emits errors but does not prevent progress.
                let range = node.range();
                let name_bytes = self
                    .gc
                    .ctx()
                    .atom_table
                    .bytes(ident.name.get())
                    .to_owned();
                self.validate_binding_identifier(range, &name_bytes, TokenKind::identifier);
                return Some(node);
            }
            if node.is_pattern() {
                // PatternNodes have already been validated.
                return Some(node);
            }
            // Flow: CoverTypedIdentifier (C++ 5941-5960). The reparsed target
            // receives the cover's `right` as its type annotation. Because the
            // Rust AST pattern/identifier type-annotation fields are immutable
            // after construction, rebuild the target node with the annotation.
            if let Node::CoverTypedIdentifier(cover) = node {
                let sub = self.reparse_assignment_pattern(cover.left, in_decl)?;
                let ty = cover.right;
                let cover_range = node.range();
                if let Some(rebuilt) = self.rebuild_pattern_with_type(
                    sub,
                    ty,
                    Some(cover.optional.get()),
                ) {
                    return Some(self.set_location(
                        cover_range.start,
                        cover_range.end,
                        rebuilt,
                    ));
                }
                // Not a pattern/identifier target: fall through (matches the
                // C++ which has no else and returns the error below if inDecl).
            }
            // Flow: TypeCastExpression (C++ 5961-5978).
            if let Node::TypeCastExpression(typecast) = node {
                let sub =
                    self.reparse_assignment_pattern(typecast.expression, in_decl)?;
                let ty = Some(typecast.type_annotation);
                if let Some(rebuilt) =
                    self.rebuild_pattern_with_type(sub, ty, None)
                {
                    let sub_start = sub.range().start;
                    let ty_end = typecast.type_annotation.range().end;
                    return Some(self.set_location(sub_start, ty_end, rebuilt));
                }
            }
        }

        if in_decl {
            let range = node.range();
            self.error_at(range, "identifier or pattern expected");
            return None;
        }

        // Not in decl, and no parens-free match: return unchanged (valid for
        // assignment targets like member expressions, call expressions, etc.).
        Some(node)
    }

    /// Rebuild an `ArrayPattern`/`ObjectPattern`/`Identifier` carrying a new type
    /// annotation (and, for identifiers, an optional `optional` flag). Mirrors
    /// the C++ in-place mutation of `_typeAnnotation`/`_optional` in
    /// `reparseAssignmentPattern` (5947-5977); the Rust AST type-annotation
    /// fields are immutable after construction, so a fresh node is built.
    /// Returns `None` if `sub` is not one of the three reparsable kinds. The
    /// caller assigns the location via `set_location`.
    fn rebuild_pattern_with_type(
        &self,
        sub: &'gc Node<'gc>,
        ty: Option<&'gc Node<'gc>>,
        optional: Option<bool>,
    ) -> Option<Node<'gc>> {
        match sub {
            Node::ArrayPattern(apn) => {
                Some(Node::ArrayPattern(ArrayPattern::new(
                    NodeMetadata::new(self.dummy_range()),
                    apn.elements,
                    ty,
                )))
            }
            Node::ObjectPattern(opn) => {
                Some(Node::ObjectPattern(ObjectPattern::new(
                    NodeMetadata::new(self.dummy_range()),
                    opn.properties,
                    ty,
                )))
            }
            Node::Identifier(id) => {
                Some(Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    id.name.get(),
                    ty,
                    // C++ sets `_optional` only for the cover case (5957); the
                    // typecast case leaves it untouched.
                    optional.unwrap_or_else(|| id.optional.get()),
                )))
            }
            _ => None,
        }
    }

    /// Reparse an ArrayExpression as an ArrayPattern. Port of
    /// `JSParserImpl::reparseArrayAsignmentPattern` (lines 5990-6052).
    ///
    /// Builds a fresh `ArrayPattern` with freshly-reparsed elements.
    fn reparse_array_assignment_pattern(
        &mut self,
        aen: &'gc ArrayExpression<'gc>,
    ) -> Option<&'gc Node<'gc>> {
        // Intern the "=" operator label once.
        let equal_op = self.gc.ctx().atom_table.atom_bytes(b"=");

        let mut elements: Vec<&'gc Node<'gc>> = Vec::new();
        let elem_iter: Vec<&'gc Node<'gc>> = aen.elements.iter().collect();
        let elem_count = elem_iter.len();

        for (idx, elem) in elem_iter.iter().enumerate() {
            let elem = *elem;

            // Elision (Empty node) — pass through.
            if matches!(elem, Node::Empty(_)) {
                elements.push(elem);
                continue;
            }

            // SpreadElement → RestElement.
            if let Node::SpreadElement(spread) = elem {
                // Rest must be the last element and there must be no trailing comma.
                let is_last = idx == elem_count - 1;
                if !is_last || aen.trailing_comma.get() {
                    let range = elem.range();
                    self.error_at(range, "rest element must be last");
                    continue;
                }
                let arg = self.reparse_assignment_pattern(spread.argument, false)?;
                let rest_end = elem.range().end;
                let rest_start = elem.range().start;
                let rest = Node::RestElement(RestElement::new(
                    NodeMetadata::new(self.dummy_range()),
                    arg,
                ));
                let rest_ref = self.set_location(rest_start, rest_end, rest);
                elements.push(rest_ref);
                continue;
            }

            // Check for AssignmentExpression with `=` and no parens
            // (unpacks into `left = init`).
            let (mut sub_elem, init) =
                if let Node::AssignmentExpression(asn) = elem {
                    if elem.metadata().parens.get() == 0
                        && asn.operator.get() == equal_op
                    {
                        (asn.left, Some(asn.right))
                    } else {
                        (elem, None)
                    }
                } else {
                    (elem, None)
                };

            // Reparse sub_elem recursively.
            match self.reparse_assignment_pattern(sub_elem, false) {
                Some(reparsed) => sub_elem = reparsed,
                None => continue,
            }

            // Wrap in AssignmentPattern if there was an initializer.
            if let Some(init_expr) = init {
                // For the location: C++ `setLocation(asn, asn, new AssignmentPatternNode)`.
                // `asn` is the original AssignmentExpression elem — use its range.
                let asn_range = elem.range();
                let ap = Node::AssignmentPattern(AssignmentPattern::new(
                    NodeMetadata::new(self.dummy_range()),
                    sub_elem,
                    init_expr,
                ));
                sub_elem = self.set_location(asn_range.start, asn_range.end, ap);
            }

            elements.push(sub_elem);
        }

        // Build fresh ArrayPattern at the AEN's location.
        let aen_range = aen.metadata.range.get();
        let ap = Node::ArrayPattern(ArrayPattern::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, elements),
            None,
        ));
        Some(self.set_location(aen_range.start, aen_range.end, ap))
    }

    /// Reparse an ObjectExpression as an ObjectPattern. Port of
    /// `JSParserImpl::reparseObjectAssignmentPattern` (lines 6054-6151).
    ///
    /// Builds a fresh `ObjectPattern` with freshly-reparsed properties.
    fn reparse_object_assignment_pattern(
        &mut self,
        oen: &'gc ObjectExpression<'gc>,
    ) -> Option<&'gc Node<'gc>> {
        // Intern the "=" and "init" atoms once.
        let equal_op = self.gc.ctx().atom_table.atom_bytes(b"=");
        let init_kind = self.gc.ctx().atom_table.atom_bytes(b"init");

        let mut properties: Vec<&'gc Node<'gc>> = Vec::new();
        let prop_iter: Vec<&'gc Node<'gc>> = oen.properties.iter().collect();
        let prop_count = prop_iter.len();

        for (idx, node) in prop_iter.iter().enumerate() {
            let node = *node;

            // SpreadElement → RestElement.
            if let Node::SpreadElement(spread) = node {
                // Rest must be the last property.
                let is_last = idx == prop_count - 1;
                if !is_last {
                    let range = node.range();
                    self.error_at(range, "rest property must be last");
                    continue;
                }
                // NOTE: per spec, the rest argument is NOT recursively reparsed
                // (see #if 0 block in C++). We just wrap the argument directly.
                // For non-decl (`in_decl=false`, the only P1 caller), just wrap.
                let rest_arg = spread.argument;
                let rest_range = node.range();
                let rest = Node::RestElement(RestElement::new(
                    NodeMetadata::new(self.dummy_range()),
                    rest_arg,
                ));
                let rest_ref = self.set_location(rest_range.start, rest_range.end, rest);
                properties.push(rest_ref);
                continue;
            }

            // Must be a Property node.
            let prop = match node {
                Node::Property(p) => p,
                _ => {
                    let range = node.range();
                    self.error_at(range, "invalid destructuring target");
                    continue;
                }
            };

            // Kind must be "init".
            if prop.kind.get() != init_kind {
                // Combine the start of the property with the start of the key
                // (mirrors C++ `SourceErrorManager::combineIntoRange`).
                let err_range = support::location::SMRange {
                    start: node.range().start,
                    end: prop.key.range().start,
                };
                self.error_at(err_range, "invalid destructuring target");
                continue;
            }

            let orig_value = prop.value;
            let end_loc = orig_value.range().end;

            // Unpack AssignmentExpression(`=`) or CoverInitializer.
            let (mut value, init) =
                if let Node::AssignmentExpression(asn) = orig_value {
                    if asn.operator.get() == equal_op {
                        (asn.left, Some(asn.right))
                    } else {
                        (orig_value, None)
                    }
                } else if let Node::CoverInitializer(ci) = orig_value {
                    // CoverInitializedName: `{a = 1}`.
                    // Clone the key (which must be an Identifier) as the value.
                    let key_ident = match prop.key {
                        Node::Identifier(id) => id,
                        _ => {
                            debug_assert!(
                                false,
                                "CoverInitializedName must start with an identifier"
                            );
                            continue;
                        }
                    };
                    // Build a fresh Identifier from the key.
                    let cloned_ident = Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        key_ident.name.get(),
                        None,
                        false,
                    ));
                    let key_range = prop.key.range();
                    let cloned_ref =
                        self.set_location(key_range.start, key_range.end, cloned_ident);
                    (cloned_ref as &'gc Node<'gc>, Some(ci.init as &'gc Node<'gc>))
                } else {
                    (orig_value, None)
                };

            // Recursively reparse the value.
            match self.reparse_assignment_pattern(value, false) {
                Some(reparsed) => value = reparsed,
                None => continue,
            }

            // Wrap in AssignmentPattern if there was an initializer.
            if let Some(init_expr) = init {
                // C++ `setLocation(value, endLoc, new AssignmentPatternNode)`.
                let val_start = value.range().start;
                let ap = Node::AssignmentPattern(AssignmentPattern::new(
                    NodeMetadata::new(self.dummy_range()),
                    value,
                    init_expr,
                ));
                value = self.set_location(val_start, end_loc, ap);
            }

            // Build fresh Property preserving key/kind/computed/method/shorthand.
            let new_prop = Node::Property(Property::new(
                NodeMetadata::new(self.dummy_range()),
                prop.key,
                value,
                prop.kind.get(),
                prop.computed.get(),
                prop.method.get(),
                prop.shorthand.get(),
            ));
            let prop_range = node.range();
            let new_prop_ref =
                self.set_location(prop_range.start, prop_range.end, new_prop);
            properties.push(new_prop_ref);
        }

        // Build fresh ObjectPattern at the OEN's location.
        let oen_range = oen.metadata.range.get();
        let op = Node::ObjectPattern(ObjectPattern::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, properties),
            None,
        ));
        Some(self.set_location(oen_range.start, oen_range.end, op))
    }

    // -----------------------------------------------------------------------
    // parseConditionalExpression — P1.4
    // -----------------------------------------------------------------------

    /// Parse a conditional (ternary `?:`) expression. Port of
    /// `JSParserImpl::parseConditionalExpression` (lines 4477-4615).
    ///
    /// `cover_typed_parameters` controls whether a `CoverTypedIdentifier` may be
    /// produced for what might turn out to be typed arrow parameters (C++ default
    /// `CoverTypedParameters::Yes`, JSParserImpl.h:1016).
    pub(super) fn parse_conditional_expression(
        &mut self,
        param: Param,
        cover_typed_parameters: CoverTypedParameters,
    ) -> Option<&'gc Node<'gc>> {
        let start_loc = self.cur_start();

        let test = self.parse_binary_expression(param)?;

        if !self.check(TokenKind::question) {
            // No '?', so this isn't a conditional expression. If
            // CoverTypedParameters::Yes, account for this being formal
            // parameters (C++ 4486-4504).
            if self.parse_types()
                && cover_typed_parameters == CoverTypedParameters::Yes
            {
                // tri-state: outer None = error → ?; Some(Some(n)) = node;
                // Some(None) = not a cover, continue.
                let opt_cover =
                    self.try_parse_cover_typed_identifier_node(test, false)?;
                if let Some(cover) = opt_cover {
                    return Some(cover);
                }
            }
            return Some(test);
        }

        let question_range = self.cur_range();

        let mut consequent: Option<&'gc Node<'gc>> = None;

        // Flow/TS typed-parameter cover + typed-arrow consequent backtracking
        // (C++ 4510-4571).
        if self.parse_types() {
            // Save here to save the '?' (we can only save on punctuators).
            let sp = self.lexer.save_point();
            self.advance(GrammarContext::AllowRegExp);

            // If CoverTypedParameters::Yes, the '?' may be part of an optional
            // parameter, not a conditional (C++ 4522-4528).
            if cover_typed_parameters == CoverTypedParameters::Yes {
                let opt_cover =
                    self.try_parse_cover_typed_identifier_node(test, true)?;
                if let Some(cover) = opt_cover {
                    return Some(cover);
                }
            }

            // A '?' without ':' that is not a conditional: typed arrow params
            // without a type annotation, e.g. `(foo?) => 1` (C++ 4536-4542).
            if cover_typed_parameters == CoverTypedParameters::Yes
                && (self.check(TokenKind::comma)
                    || self.check(TokenKind::r_paren)
                    || self.check(TokenKind::equal))
            {
                let node =
                    Node::CoverTypedIdentifier(CoverTypedIdentifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        test,
                        None,
                        true,
                    ));
                return Some(self.set_location(
                    start_loc,
                    question_range.end,
                    node,
                ));
            }

            // Real backtracking stage. Parse with AllowTypedArrowFunction::Yes,
            // then require a ':' afterwards; otherwise restore and retry below
            // with AllowTypedArrowFunction::No (C++ 4544-4570).
            // SaveAndSuppressMessages: pure-suppress parser messages.
            let saved_suppressed =
                self.lexer.get_source_mgr().suppressed_messages();
            self.lexer.get_source_mgr_mut().set_suppressed_messages(Some(
                support::diag::Subsystem::Parser,
            ));
            let _guard = match self.check_recursion() {
                Some(g) => g,
                None => {
                    self.lexer
                        .get_source_mgr_mut()
                        .set_suppressed_messages(saved_suppressed);
                    return None;
                }
            };
            let opt_consequent = self.parse_assignment_expression(
                PARAM_IN,
                false,
                AllowTypedArrowFunction::Yes,
                CoverTypedParameters::No,
                None,
            );
            self.lexer
                .get_source_mgr_mut()
                .set_suppressed_messages(saved_suppressed);
            if let Some(c) = opt_consequent {
                if self.check(TokenKind::colon) {
                    consequent = Some(c);
                } else {
                    sp.restore(&mut self.lexer);
                }
            } else {
                sp.restore(&mut self.lexer);
            }
        }

        // CHECK_RECURSION: mirrors C++ line 4576 (before the !consequent block).
        let _guard = self.check_recursion()?;

        // Only try with AllowTypedArrowFunction::No if we haven't already set up
        // the consequent above (C++ 4580-4591).
        let consequent = if let Some(c) = consequent {
            c
        } else {
            // Consume the '?' (first time, or after savePoint.restore()).
            self.advance(GrammarContext::AllowRegExp);
            self.parse_assignment_expression(
                PARAM_IN,
                false,
                AllowTypedArrowFunction::No,
                CoverTypedParameters::No,
                None,
            )?
        };

        // Eat ':' — required after '... ? ...'.
        if !self.eat(
            TokenKind::colon,
            GrammarContext::AllowRegExp,
            "in conditional expression after '... ? ...'",
        ) {
            let _ = question_range; // referenced only for the C++ error note
            return None;
        }

        // Parse the alternate (false branch). C++ 4601-4605:
        // AllowTypedArrowFunction::Yes, CoverTypedParameters::No.
        let alternate = self.parse_assignment_expression(
            param,
            false,
            AllowTypedArrowFunction::Yes,
            CoverTypedParameters::No,
            None,
        )?;

        let end_loc = self.lexer.prev_token_end();
        let node = Node::ConditionalExpression(ConditionalExpression::new(
            NodeMetadata::new(self.dummy_range()),
            test,
            alternate,
            consequent,
        ));
        Some(self.set_location(start_loc, end_loc, node))
    }

    // -----------------------------------------------------------------------
    // tryParseCoverTypedIdentifierNode — 4618 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// In Flow/TS arrow-function parameters, optional parameters look like
    /// `Identifier ? : TypeAnnotation`. Because the colon and type annotation are
    /// optional, consume the colon here and return a `CoverTypedIdentifier` if it
    /// is possible we are parsing typed arrow parameters. Port of
    /// `JSParserImpl::tryParseCoverTypedIdentifierNode` (lines 4618-4649).
    ///
    /// Tri-state result (mirrors the C++ `Optional<Node *>`):
    ///   - `None`            = error already reported, propagate with `?`.
    ///   - `Some(None)`      = not a cover node, continue as usual.
    ///   - `Some(Some(n))`   = the `CoverTypedIdentifier` node.
    fn try_parse_cover_typed_identifier_node(
        &mut self,
        test: &'gc Node<'gc>,
        optional: bool,
    ) -> Option<Option<&'gc Node<'gc>>> {
        debug_assert!(self.parse_types(), "must be parsing types");
        // Faithful to C++ 4628-4646: the outer `if` has a trailing fall-through
        // comment after the inner `if`, so they are not actually collapsible.
        #[allow(clippy::collapsible_if)]
        if self.check(TokenKind::colon)
            && test.metadata().parens.get() == 0
        {
            if matches!(
                test,
                Node::Identifier(_)
                    | Node::ObjectExpression(_)
                    | Node::ArrayExpression(_)
            ) {
                // Deliberately wrap the type annotation later when reparsing.
                // C++ 4633-4634: parseTypeAnnotation(annotStart) — wraps.
                let annot_start = self.advance(GrammarContext::Type).start;
                let ty = self.parse_type_annotation(
                    Some(annot_start),
                    AllowAnonFunctionType::Yes,
                )?;

                let end = self.lexer.prev_token_end();
                let node =
                    Node::CoverTypedIdentifier(CoverTypedIdentifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        test,
                        Some(ty),
                        optional,
                    ));
                let test_start = test.range().start;
                return Some(Some(self.set_location(test_start, end, node)));
            }
            // The colon indicates something other than the typeAnnotation for
            // the parameter. Continue as usual.
        }
        Some(None)
    }

    // -----------------------------------------------------------------------
    // tryParseTypedAsyncArrowFunction — 6154 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Speculatively parse a typed async arrow function
    /// (`async <T>(x: T): T => …` / `async (x: number) => x`). Port of
    /// `JSParserImpl::tryParseTypedAsyncArrowFunction` (lines 6154-6230).
    ///
    /// Entered when `async` is followed by `<` or `(`. Returns `None` if this is
    /// not a typed async arrow (caller falls back to normal async handling).
    fn try_parse_typed_async_arrow_function(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.parse_flow());
        debug_assert!(self.check_unescaped_name(b"async"));
        let sp = self.lexer.save_point();
        let start = self.advance(GrammarContext::AllowRegExp).start;

        let mut type_params: Option<&'gc Node<'gc>> = None;
        let mut return_type: Option<&'gc Node<'gc>> = None;
        let mut predicate: Option<&'gc Node<'gc>> = None;

        // C++ SaveAndSuppressMessages: pure-suppress parser messages while the
        // speculative parse runs.
        let saved_suppressed = self.lexer.get_source_mgr().suppressed_messages();
        self.lexer.get_source_mgr_mut().set_suppressed_messages(Some(
            support::diag::Subsystem::Parser,
        ));

        // Labeled block so every early-bail path restores suppression below; the
        // block evaluates to `Some((leftExpr, hasNewLine))` on success.
        let result: Option<(&'gc Node<'gc>, bool)> = 'try_async: {
            if self.check(TokenKind::less) {
                match self.parse_type_params_flow() {
                    Some(tp) => type_params = Some(tp),
                    None => break 'try_async None,
                }
            }

            if !self.check(TokenKind::l_paren) {
                break 'try_async None;
            }

            let has_new_line = self.lexer.is_new_line_before_current_token();
            let left_expr = match self
                .parse_conditional_expression(param, CoverTypedParameters::Yes)
            {
                Some(e) => e,
                None => break 'try_async None,
            };

            if self.check(TokenKind::colon) {
                let annot_start = self.advance(GrammarContext::Type).start;
                if !self.check_name(b"%checks") {
                    match self.parse_return_type_annotation_flow(
                        Some(annot_start),
                        AllowAnonFunctionType::No,
                    ) {
                        Some(t) => return_type = Some(t),
                        None => break 'try_async None,
                    }
                }
                if self.check_name(b"%checks") {
                    match self.parse_predicate_flow() {
                        Some(p) => predicate = Some(p),
                        None => break 'try_async None,
                    }
                }
            }

            if !self.check(TokenKind::equalgreater) {
                break 'try_async None;
            }

            Some((left_expr, has_new_line))
        };

        self.lexer
            .get_source_mgr_mut()
            .set_suppressed_messages(saved_suppressed);

        let (left_expr, has_new_line) = match result {
            Some(v) => v,
            None => {
                sp.restore(&mut self.lexer);
                return None;
            }
        };

        self.parse_arrow_function_expression(
            param,
            /* eagerly */ false,
            left_expr,
            has_new_line,
            type_params,
            return_type,
            predicate,
            start,
            AllowTypedArrowFunction::Yes,
            /* force_async */ true,
        )
    }

    // -----------------------------------------------------------------------
    // parseBinaryExpression — P1.2
    // -----------------------------------------------------------------------

    /// Return the binary-operator precedence of `kind`, or 0 if `kind` is not
    /// a binary operator.  Mirrors C++ anonymous `getPrecedence(TokenKind)`:
    /// - The BINOP table entries are gated to the `_first_binary…_last_binary`
    ///   range by `binop_precedence`.
    /// - `rw_in` and `rw_instanceof` are reserved words (outside that range)
    ///   but the C++ RESWORD macro gives them precedence 8; handle them
    ///   explicitly here.
    /// - `as_operator` (IDENT_OP, precedence 8) is injected by
    ///   `convertIdentOpIfPossible` when Flow/TS type-parsing is on;
    ///   `binop_precedence` only covers the BINOP range, so it is handled
    ///   explicitly in the `None` arm below.
    #[inline]
    fn get_precedence(kind: TokenKind) -> u32 {
        use crate::token_kinds::binop_precedence;
        match binop_precedence(kind) {
            Some(p) => p as u32,
            None => match kind {
                TokenKind::rw_in | TokenKind::rw_instanceof => 8,
                // IDENT_OP(as_operator, "as", 8) (TokenKinds.def:163). The C++
                // `getPrecedence` flat table assigns IDENT_OP entries their
                // precedence; `binop_precedence` only covers the BINOP range,
                // so the `as` operator is handled here.
                TokenKind::as_operator => 8,
                _ => 0,
            },
        }
    }

    /// Return `true` if `kind` is a left-associative binary operator.
    /// Only `**` is right-associative.  Port of C++ anonymous `isLeftAssoc`.
    #[inline]
    fn is_left_assoc(kind: TokenKind) -> bool {
        kind != TokenKind::starstar
    }

    /// Return the precedence of the current token unless it equals `except`,
    /// in which case return 0.  Port of C++ anonymous `getPrecedenceExcept`.
    #[inline]
    fn get_precedence_except(kind: TokenKind, except: TokenKind) -> u32 {
        if kind != except {
            Self::get_precedence(kind)
        } else {
            0
        }
    }

    /// Convert the current identifier token to `as_operator` if it spells "as"
    /// and the parser context has TS/Flow type-parsing enabled. Port of
    /// `JSParserImpl::convertIdentOpIfPossible` (JSParserImpl.cpp:4252-4260).
    #[inline]
    fn convert_ident_op_if_possible(&mut self) {
        // C++ 4254-4257: gated on `getParseTypes()` and the current token being
        // an `identifier` whose (escape-sensitive) value is `as`.
        if self.cur_kind() == TokenKind::identifier && self.parse_types() {
            let bytes = self
                .lexer
                .get_string_table()
                .bytes(self.lexer.token().get_identifier());
            if bytes == b"as" {
                self.lexer
                    .convert_cur_token_to_ident_op(TokenKind::as_operator);
            }
        }
    }

    /// Build the `as_operator` result node in `newBinNode`. Port of the Flow
    /// arm of `JSParserImpl::parseBinaryExpression::newBinNode`
    /// (JSParserImpl.cpp:4319-4351). `right` is the parsed type annotation.
    ///
    /// Special-cases `x as const` (a `GenericTypeAnnotation` with no type-params,
    /// no parens, whose `id` is an `Identifier` named `const`, not optional and
    /// with no type-annotation) → `AsConstExpression`; otherwise `AsExpression`.
    fn make_as_node(
        &mut self,
        left: &'gc Node<'gc>,
        right: &'gc Node<'gc>,
        start: SMLoc,
        end: SMLoc,
    ) -> &'gc Node<'gc> {
        // C++ 4321-4327: under TS, `x as T` is a `TSAsExpression`. This branch
        // is checked BEFORE the Flow `as`/`as const` handling, and TS has no
        // `as const` special case — `x as const` is a plain `TSAsExpression`
        // whose `typeAnnotation` is a `TSTypeReference` to `const`.
        if self.parse_ts() {
            let node = Node::TSAsExpression(TSAsExpression::new(
                NodeMetadata::new(self.dummy_range()),
                left,
                right,
            ));
            return self.set_location(start, end, node);
        }
        // C++ 4330: otherwise must be parsing Flow types.
        debug_assert!(self.parse_flow(), "must be parsing types");
        // C++ 4331-4345: `x as const` special case.
        if let Node::GenericTypeAnnotation(gen) = right {
            if gen.type_parameters.is_none() && right.metadata().parens.get() == 0 {
                if let Node::Identifier(ident) = gen.id {
                    let const_atom =
                        self.gc.ctx().atom_table.atom_bytes(b"const");
                    if ident.name.get() == const_atom
                        && !ident.optional.get()
                        && ident.type_annotation.is_none()
                    {
                        let node = Node::AsConstExpression(AsConstExpression::new(
                            NodeMetadata::new(self.dummy_range()),
                            left,
                        ));
                        return self.set_location(start, end, node);
                    }
                }
            }
        }
        // C++ 4346-4349: the general `x as T` case.
        let node = Node::AsExpression(AsExpression::new(
            NodeMetadata::new(self.dummy_range()),
            left,
            right,
        ));
        self.set_location(start, end, node)
    }

    /// Parse a binary expression using a stack-based precedence-climbing
    /// algorithm.  Port of `JSParserImpl::parseBinaryExpression`
    /// (lib/Parser/JSParserImpl.cpp lines 4262-4475).
    ///
    /// Handles:
    /// - All BINOP operators (`+`, `-`, `*`, `/`, `%`, `**`, `<<`, `>>`,
    ///   `>>>`, `<`, `>`, `<=`, `>=`, `==`, `!=`, `===`, `!==`, `&`, `^`,
    ///   `|`, `&&`, `||`, `??`).
    /// - `instanceof` and `in` (when `PARAM_IN` is set).
    /// - Private-name LHS for `#x in y`.
    /// - `&&`/`||`/`??` → `LogicalExpression`; others → `BinaryExpression`.
    /// - Nullish/boolean mixing error ("Mixing '??' with '&&' or '||' …").
    /// - `as_operator`: under TS, `x as T` → `TSAsExpression` (no `as const`
    ///   special case); under Flow, `x as T` → `AsExpression`, `x as const` →
    ///   `AsConstExpression`.
    pub(super) fn parse_binary_expression(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        use support::location::SMRange;

        // Stack entry: left-hand expression, operator, start location of LHS.
        // The C++ uses a SmallVector; we use a Vec (plain heap; fine for P1).
        struct StackEntry<'gc> {
            expr: &'gc Node<'gc>,
            op_kind: TokenKind,
            expr_start: SMLoc,
        }

        let mut stack: Vec<StackEntry<'gc>> = Vec::with_capacity(16);

        // Nullish/boolean mixing-error tracking.
        // True after we have seen a '??' operator.
        let mut has_nullish = false;
        // True after we have seen a '&&' or '||' operator.
        let mut has_boolean = false;

        // ---------------------------------------------------------------
        // new_bin_node — allocate BinaryExpression or LogicalExpression
        // ---------------------------------------------------------------
        // We can't capture `self` in a closure and also call `&mut self`
        // methods, so this is an out-of-band helper that borrows the
        // pieces it needs directly (gc + lexer for interning; the error
        // manager for the mixing error).  We use a macro-like inline
        // closure whose captures are the individual fields we need.
        //
        // The has_nullish/has_boolean flags are passed by &mut ref so the
        // closure can mutate them, mirroring the C++ lambda captures.

        // Helper: intern the operator spelling into a NodeLabel.
        // C++ `getTokenIdent(opKind)` returns the pre-interned UniqueString.
        // In Rust: `token_kind_str(opKind)` → &str → intern via atom_table.
        let make_op_label = |gc: &'gc GCLock<'_, '_>, kind: TokenKind| {
            let s = crate::token_kinds::token_kind_str(kind);
            gc.ctx().atom_table.atom_bytes(s.as_bytes())
        };

        // Whether the current token is `in` (reserved word, so excluded from
        // the main BINOP table).  When `PARAM_IN` is NOT set, `in` must NOT
        // be treated as a binary operator — that is the "ForIn initialiser"
        // restriction from the spec.
        let except_kind = if !param.has(PARAM_IN) {
            TokenKind::rw_in
        } else {
            TokenKind::none
        };

        // -----------------------------------------------------------------
        // Parse first operand (private identifier or unary expression).
        // -----------------------------------------------------------------
        let mut top_expr_start = self.cur_start();
        let mut top_expr: &'gc Node<'gc> = if self.check(TokenKind::private_identifier) {
            // consumePrivateIdentifier closure (C++ lines 4361-4383).
            let tok_start = self.lexer.token().start_loc();
            let tok_end = self.lexer.token().end_loc();
            let priv_ident_name = self.lexer.token().get_private_identifier();
            // Build PrivateName(Identifier(...)).
            let ident_node = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                priv_ident_name,
                None,
                false,
            ));
            let ident_ref = self.set_location(tok_start, tok_end, ident_node);
            let priv_node = Node::PrivateName(PrivateName::new(
                NodeMetadata::new(self.dummy_range()),
                ident_ref,
            ));
            let priv_ref = self.set_location(tok_start, tok_end, priv_node);
            self.advance(GrammarContext::AllowDiv);

            // Validate: a PrivateName is only legal as the LHS of `in`, and
            // only if `in`'s precedence is not beaten by the current stack top.
            let prev_prec = stack
                .last()
                .map(|e| Self::get_precedence(e.op_kind))
                .unwrap_or(0);
            let in_prec = Self::get_precedence(TokenKind::rw_in);
            if !self.check(TokenKind::rw_in) || prev_prec >= in_prec {
                let priv_range = priv_ref.range();
                self.error_at(
                    priv_range,
                    "Private name can only be used as left-hand side of `in` expression",
                );
            }
            priv_ref
        } else {
            self.parse_unary_expression()?
        };
        let mut top_expr_end = self.lexer.prev_token_end();
        self.convert_ident_op_if_possible();

        // -----------------------------------------------------------------
        // Main precedence-climbing loop.
        // -----------------------------------------------------------------
        loop {
            let cur_kind = self.cur_kind();
            let precedence = Self::get_precedence_except(cur_kind, except_kind);
            if precedence == 0 {
                break;
            }

            // Pop stack entries whose operator has >= precedence than the
            // current one (left-associative) or strictly > (right-associative
            // allows equal-precedence to stay on the stack so we can build
            // the right-hand side fully before folding).
            while let Some(top) = stack.last() {
                let top_prec = Self::get_precedence(top.op_kind);
                if precedence > top_prec {
                    break;
                }
                if precedence == top_prec && !Self::is_left_assoc(top.op_kind) {
                    // Right-associative: don't pop on equal precedence.
                    break;
                }
                // Pop and fold: top.expr <op> top_expr.
                let entry = stack.pop().unwrap();
                let op_label = make_op_label(self.gc, entry.op_kind);
                let new_start = entry.expr_start;
                let new_end = top_expr_end;

                // Decide LogicalExpression vs BinaryExpression and handle
                // the nullish/boolean mixing-error (C++ newBinNode lambda).
                top_expr = if entry.op_kind == TokenKind::ampamp
                    || entry.op_kind == TokenKind::pipepipe
                    || entry.op_kind == TokenKind::questionquestion
                {
                    // Mixing-error check.
                    if (has_nullish && entry.op_kind != TokenKind::questionquestion)
                        || (has_boolean && entry.op_kind == TokenKind::questionquestion)
                    {
                        let err_range = SMRange {
                            start: entry.expr.range().start,
                            end: top_expr.range().end,
                        };
                        self.error_at(
                            err_range,
                            "Mixing '??' with '&&' or '||' requires parentheses",
                        );
                    }
                    if entry.op_kind == TokenKind::questionquestion {
                        has_nullish = true;
                    } else {
                        has_boolean = true;
                    }
                    let node = Node::LogicalExpression(LogicalExpression::new(
                        NodeMetadata::new(self.dummy_range()),
                        entry.expr,
                        top_expr,
                        op_label,
                    ));
                    self.set_location(new_start, new_end, node)
                } else if entry.op_kind == TokenKind::as_operator {
                    // Flow `as`/`as const` (C++ newBinNode 4319-4351). `top_expr`
                    // here is the parsed type annotation (RHS).
                    self.make_as_node(entry.expr, top_expr, new_start, new_end)
                } else {
                    let node = Node::BinaryExpression(BinaryExpression::new(
                        NodeMetadata::new(self.dummy_range()),
                        entry.expr,
                        top_expr,
                        op_label,
                    ));
                    self.set_location(new_start, new_end, node)
                };
                top_expr_start = top_expr.range().start;
            }

            // Push current top_expr and the incoming operator.
            stack.push(StackEntry {
                expr: top_expr,
                op_kind: cur_kind,
                expr_start: top_expr_start,
            });

            // Consume the operator token and parse the RHS.
            // C++ 4432-4453: the `as_operator` consumes with GrammarContext::Type
            // and the RHS is a *type annotation* (not a unary expression); every
            // other operator consumes with the default (AllowRegExp) and the RHS
            // is a private-identifier or unary expression.
            if cur_kind == TokenKind::as_operator {
                self.advance(GrammarContext::Type);
                top_expr_start = self.cur_start();
                // C++ parseTypeAnnotation() — defaults AllowAnonFunctionType::Yes
                // (JSParserImpl.h:1209). `parse_type_annotation` dispatches to
                // the Flow or TS version per the enabled dialect.
                top_expr = self.parse_type_annotation(None, AllowAnonFunctionType::Yes)?;
            } else {
                self.advance(GrammarContext::AllowRegExp);
                top_expr_start = self.cur_start();

                // Parse the right-hand operand (private identifier or unary).
                top_expr = if self.check(TokenKind::private_identifier) {
                    let tok_start = self.lexer.token().start_loc();
                    let tok_end = self.lexer.token().end_loc();
                    let priv_ident_name = self.lexer.token().get_private_identifier();
                    let ident_node = Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        priv_ident_name,
                        None,
                        false,
                    ));
                    let ident_ref = self.set_location(tok_start, tok_end, ident_node);
                    let priv_node = Node::PrivateName(PrivateName::new(
                        NodeMetadata::new(self.dummy_range()),
                        ident_ref,
                    ));
                    let priv_ref = self.set_location(tok_start, tok_end, priv_node);
                    self.advance(GrammarContext::AllowDiv);

                    // Validate: PrivateName as RHS is only legal for `in`, and
                    // the current operator on the stack-top must be exactly `in`
                    // with no higher-precedence operator above it.
                    let prev_prec = stack
                        .last()
                        .map(|e| Self::get_precedence(e.op_kind))
                        .unwrap_or(0);
                    let in_prec = Self::get_precedence(TokenKind::rw_in);
                    if !self.check(TokenKind::rw_in) || prev_prec >= in_prec {
                        let priv_range = priv_ref.range();
                        self.error_at(
                            priv_range,
                            "Private name can only be used as left-hand side of `in` expression",
                        );
                    }
                    priv_ref
                } else {
                    self.parse_unary_expression()?
                };
            }
            top_expr_end = self.lexer.prev_token_end();
            self.convert_ident_op_if_possible();
        }

        // -----------------------------------------------------------------
        // Drain the remaining stack.
        // -----------------------------------------------------------------
        while let Some(entry) = stack.pop() {
            let op_label = make_op_label(self.gc, entry.op_kind);
            let new_start = entry.expr_start;
            let new_end = top_expr_end;

            top_expr = if entry.op_kind == TokenKind::ampamp
                || entry.op_kind == TokenKind::pipepipe
                || entry.op_kind == TokenKind::questionquestion
            {
                if (has_nullish && entry.op_kind != TokenKind::questionquestion)
                    || (has_boolean && entry.op_kind == TokenKind::questionquestion)
                {
                    let err_range = SMRange {
                        start: entry.expr.range().start,
                        end: top_expr.range().end,
                    };
                    self.error_at(
                        err_range,
                        "Mixing '??' with '&&' or '||' requires parentheses",
                    );
                }
                if entry.op_kind == TokenKind::questionquestion {
                    has_nullish = true;
                } else {
                    has_boolean = true;
                }
                let node = Node::LogicalExpression(LogicalExpression::new(
                    NodeMetadata::new(self.dummy_range()),
                    entry.expr,
                    top_expr,
                    op_label,
                ));
                self.set_location(new_start, new_end, node)
            } else if entry.op_kind == TokenKind::as_operator {
                // Flow `as`/`as const` (C++ newBinNode 4319-4351).
                self.make_as_node(entry.expr, top_expr, new_start, new_end)
            } else {
                let node = Node::BinaryExpression(BinaryExpression::new(
                    NodeMetadata::new(self.dummy_range()),
                    entry.expr,
                    top_expr,
                    op_label,
                ));
                self.set_location(new_start, new_end, node)
            };
            // top_expr_end stays the same (right side doesn't change).
        }

        Some(top_expr)
    }

    // -----------------------------------------------------------------------
    // parseUnaryExpression — P1.3
    // -----------------------------------------------------------------------

    /// Parse a unary expression. Port of
    /// `JSParserImpl::parseUnaryExpression` (lines 4112-4211).
    ///
    /// Handles:
    /// - Prefix unary: `delete`, `void`, `typeof`, `+`, `-`, `~`, `!`
    ///   → `UnaryExpression(operator, argument, prefix=true)`
    /// - Prefix update: `++`, `--` → `UpdateExpression(operator, argument, prefix=true)`
    /// - `await` (when `param_await` is set) → `AwaitExpression(argument)`
    /// - TS type assertion `<Type>expr` (when `parse_ts` && !`parse_jsx`).
    /// - Default: fall through to `parse_postfix_expression()`.
    pub(super) fn parse_unary_expression(&mut self) -> Option<&'gc Node<'gc>> {
        use crate::token_kinds::token_kind_str;

        let start_loc = self.cur_start();

        match self.cur_kind() {
            // Prefix UnaryExpression: delete / void / typeof / + / - / ~ / !
            TokenKind::rw_delete
            | TokenKind::rw_void
            | TokenKind::rw_typeof
            | TokenKind::plus
            | TokenKind::minus
            | TokenKind::tilde
            | TokenKind::exclaim => {
                let op_kind = self.cur_kind();
                // Intern operator name before advancing (mirrors C++ `op = getTokenIdent(tok_)`)
                let op_label = self.gc.ctx().atom_table.atom_bytes(
                    token_kind_str(op_kind).as_bytes(),
                );
                self.advance(GrammarContext::AllowRegExp);
                let _guard = self.check_recursion()?;
                let expr = self.parse_unary_expression()?;

                // ExponentiationExpression only allows UpdateExpression on the
                // left. A bare unary operator before `**` must be parenthesized.
                if self.check(TokenKind::starstar) {
                    use support::location::SMRange;
                    self.error_at(
                        SMRange {
                            start: start_loc,
                            end: self.lexer.token().end_loc(),
                        },
                        "Unary operator before ** must use parens to disambiguate",
                    );
                }

                let end_loc = self.lexer.prev_token_end();
                let node = Node::UnaryExpression(UnaryExpression::new(
                    NodeMetadata::new(self.dummy_range()),
                    op_label,
                    expr,
                    true,
                ));
                Some(self.set_location(start_loc, end_loc, node))
            }

            // Prefix UpdateExpression: ++ / --
            TokenKind::plusplus | TokenKind::minusminus => {
                let op_kind = self.cur_kind();
                let op_label = self.gc.ctx().atom_table.atom_bytes(
                    token_kind_str(op_kind).as_bytes(),
                );
                self.advance(GrammarContext::AllowRegExp);
                let _guard = self.check_recursion()?;
                let expr = self.parse_unary_expression()?;

                let end_loc = self.lexer.prev_token_end();
                let node = Node::UpdateExpression(UpdateExpression::new(
                    NodeMetadata::new(self.dummy_range()),
                    op_label,
                    expr,
                    true,
                ));
                Some(self.set_location(start_loc, end_loc, node))
            }

            // TS type assertion `< Type > UnaryExpression` (C++ 4162-4189).
            TokenKind::less => {
                // TSTypeAssertions are only parsed when JSX is disabled, so
                // there's no backtracking necessary here (C++ 4164-4166).
                if self.parse_ts() && !self.parse_jsx() {
                    // < Type > UnaryExpression
                    // ^
                    self.advance(GrammarContext::Type);
                    let opt_type = self.parse_type_annotation_ts(None)?;
                    // C++ 4170-4172: the closing `>` is eaten in AllowRegExp —
                    // the ONE place a TS `>` is not consumed in Type context.
                    if !self.eat(
                        TokenKind::greater,
                        GrammarContext::AllowRegExp,
                        "in type assertion",
                    ) {
                        self.lexer.get_source_mgr_mut().note_at(
                            start_loc,
                            None,
                            "start of assertion",
                            support::diag::Subsystem::Parser,
                        );
                        return None;
                    }
                    let _guard = self.check_recursion()?;
                    let opt_expr = self.parse_unary_expression()?;
                    let end = self.lexer.prev_token_end();
                    let node = Node::TSTypeAssertion(TSTypeAssertion::new(
                        NodeMetadata::new(self.dummy_range()),
                        opt_type,
                        opt_expr,
                    ));
                    Some(self.set_location(start_loc, end, node))
                } else {
                    // Not a TS assertion: fall through to postfix.
                    self.parse_postfix_expression()
                }
            }

            // await expression (only when inside an async function)
            TokenKind::identifier => {
                // Capture whether the current identifier spells "await" BEFORE
                // the &mut self advance call (avoids borrow conflict — same
                // pattern as the yield check in parsePrimaryExpression).
                let is_await = self
                    .lexer
                    .get_string_table()
                    .bytes(self.lexer.token().get_identifier())
                    == b"await";
                if is_await && self.param_await.get() {
                    self.advance(GrammarContext::AllowRegExp);
                    let _guard = self.check_recursion()?;
                    let expr = self.parse_unary_expression()?;
                    let end_loc = self.lexer.prev_token_end();
                    let node = Node::AwaitExpression(AwaitExpression::new(
                        NodeMetadata::new(self.dummy_range()),
                        expr,
                    ));
                    Some(self.set_location(start_loc, end_loc, node))
                } else {
                    // All other identifiers: fall through to postfix.
                    self.parse_postfix_expression()
                }
            }

            // Default: fall through to parsePostfixExpression.
            _ => self.parse_postfix_expression(),
        }
    }

    // -----------------------------------------------------------------------
    // parsePostfixExpression — P1.3
    // -----------------------------------------------------------------------

    /// Parse a postfix expression (++/-- suffix). Port of
    /// `JSParserImpl::parsePostfixExpression` (lines 4091-4110).
    ///
    /// Parses the LHS via `parse_left_hand_side_expression`, then if the
    /// current token is `++`/`--` AND there is no newline before it, wraps
    /// the result in a `UpdateExpression(operator, argument, prefix=false)`.
    ///
    /// The end-of-node range is the end of the `++`/`--` token (BEFORE
    /// `advance`), and the debug loc is the *start* of the `++`/`--` token —
    /// faithfully porting the C++ 4-arg `setLocation(startLoc, tok_, tok_, n)`.
    pub(super) fn parse_postfix_expression(&mut self) -> Option<&'gc Node<'gc>> {
        use crate::token_kinds::token_kind_str;

        let start_loc = self.cur_start();
        let expr = self.parse_left_hand_side_expression(IsClassHeritageArgument::No)?;

        if self.check2(TokenKind::plusplus, TokenKind::minusminus)
            && !self.lexer.is_new_line_before_current_token()
        {
            let op_kind = self.cur_kind();
            let op_label = self.gc.ctx().atom_table.atom_bytes(
                token_kind_str(op_kind).as_bytes(),
            );
            // Capture the operator token's locations BEFORE advancing.
            // C++ 4-arg setLocation: start=startLoc, end=tok_->getEndLoc(),
            // debugLoc=tok_->getStartLoc().
            let op_start = self.lexer.token().start_loc();
            let op_end = self.lexer.token().end_loc();
            self.advance(GrammarContext::AllowDiv);

            let node = Node::UpdateExpression(UpdateExpression::new(
                NodeMetadata::new(self.dummy_range()),
                op_label,
                expr,
                false,
            ));
            Some(self.set_location_d(start_loc, op_end, op_start, node))
        } else {
            Some(expr)
        }
    }

    // -----------------------------------------------------------------------
    // parseLeftHandSideExpression / parseLeftHandSideExpressionTail — P1.6
    // -----------------------------------------------------------------------

    /// Parse a left-hand-side expression. Port of
    /// `JSParserImpl::parseLeftHandSideExpression` (lines 4014-4024).
    ///
    /// Parses a NewExpression or OptionalExpression, then checks for a call
    /// tail (optional chain `?.`, `(args)` or template-literal).
    /// The `is_class_heritage_argument` flag is threaded for P3+ class-extends
    /// parsing; P1 callers always pass `IsClassHeritageArgument::No`.
    pub(super) fn parse_left_hand_side_expression(
        &mut self,
        is_class_heritage_argument: IsClassHeritageArgument,
    ) -> Option<&'gc Node<'gc>> {
        let start_loc = self.cur_start();
        let expr =
            self.parse_new_expression_or_optional_expression(IsConstructorCall::No)?;
        self.parse_left_hand_side_expression_tail(start_loc, expr, is_class_heritage_argument)
    }

    /// Finish a left-hand-side expression after the base has been parsed.
    /// Port of `JSParserImpl::parseLeftHandSideExpressionTail` (4026-4089).
    ///
    /// Handles the optional-chain `?.` prefix on a call expression, determines
    /// `seenOptionalChain`, and dispatches to `parseCallExpression` when the
    /// next token is `(` or a template literal.
    ///
    /// Flow type-argument speculation on the call tail is handled (P6.0), as is
    /// the Flow record-expression branch + its alternative type-args
    /// commit-condition (P6.4). The TS arm is OR'd into the same gate (P7.5b).
    pub(super) fn parse_left_hand_side_expression_tail(
        &mut self,
        start_loc: support::location::SMLoc,
        mut expr: &'gc Node<'gc>,
        is_class_heritage_argument: IsClassHeritageArgument,
    ) -> Option<&'gc Node<'gc>> {
        // Consume `?.` if present (4030-4034).
        let optional =
            self.check_and_eat(TokenKind::questiondot, GrammarContext::AllowRegExp);
        // seenOptionalChain: true if we consumed `?.`, OR if the base expression
        // is already an OptionalMember/OptionalCall at paren depth 0.
        let seen_optional_chain = optional
            || (expr.metadata().parens.get() == 0
                && matches!(
                    expr,
                    Node::OptionalMemberExpression(_) | Node::OptionalCallExpression(_)
                ));

        // Flow/TS type-arguments block (C++ 4036-4062). If the `<` immediately
        // follows a `?.` it cannot be a binary expression and is unambiguously
        // Flow type syntax — hence the `optional` case uses the non-ambiguous
        // `getParseFlow()` gate; otherwise `getParseFlowAmbiguous()`. The C++
        // gate ORs `getParseTS()` on top: `((optional ? getParseFlow() :
        // getParseFlowAmbiguous()) || getParseTS())`.
        let mut type_args: Option<&'gc Node<'gc>> = None;
        let flow_gate = if optional {
            self.parse_flow()
        } else {
            self.parse_flow_ambiguous()
        };
        if (flow_gate || self.parse_ts()) && self.check(TokenKind::less) {
            let (opt_type_args, sp) = self.speculative_type_args();
            // Commit when a `(` follows (call expression with type-args), OR
            // — P6.4 — when the Flow record-expression alternative holds
            // (C++ 4049-4053): `parse_flow() && parse_flow_records()
            //   && is_class_heritage_argument != Yes
            //   && check_record_expression_flow(expr)`.
            if opt_type_args.is_some()
                && (self.check(TokenKind::l_paren)
                    || (self.parse_flow()
                        && self.parse_flow_records()
                        && is_class_heritage_argument
                            != IsClassHeritageArgument::Yes
                        && self.check_record_expression_flow(expr)))
            {
                type_args = opt_type_args;
            } else {
                sp.restore(&mut self.lexer);
            }
        }

        // Is this a CallExpression? (4065-4074)
        // C++ checks checkN(l_paren, no_substitution_template, template_head).
        if self.check_n3(
            TokenKind::l_paren,
            TokenKind::no_substitution_template,
            TokenKind::template_head,
        ) {
            expr = self.parse_call_expression(
                start_loc,
                expr,
                type_args,
                seen_optional_chain,
                optional,
            )?;
        }
        // P6.4: Flow record expression (C++ 4075-4086).
        else if self.parse_flow()
            && self.parse_flow_records()
            && is_class_heritage_argument != IsClassHeritageArgument::Yes
            && self.check_record_expression_flow(expr)
        {
            expr =
                self.parse_record_expression_flow(start_loc, expr, type_args)?;
        }

        Some(expr)
    }

    // -----------------------------------------------------------------------
    // parseNewExpressionOrOptionalExpression — P1.6
    // -----------------------------------------------------------------------

    /// Parse a `new`-expression or a plain optional expression. Port of
    /// `JSParserImpl::parseNewExpressionOrOptionalExpression` (3920-4012).
    ///
    /// If the current token is NOT `new`, delegates to
    /// `parse_optional_expression_except_new`. Otherwise:
    /// - `new.target` → `MetaProperty(meta=Identifier"new", prop=Identifier"target")`
    ///   followed by the optional-expression tail.
    /// - `new <callee> [(<args>)]` → `NewExpression`; if arguments follow, also
    ///   handles trailing member selects.
    ///
    /// Flow `typeArgs` speculation on `new` is handled (P6.0): `new C<T>` keeps
    /// type-args with no `(` required. The TS arm is OR'd in (P7.5b).
    pub(super) fn parse_new_expression_or_optional_expression(
        &mut self,
        is_constructor_call: IsConstructorCall,
    ) -> Option<&'gc Node<'gc>> {
        if !self.check(TokenKind::rw_new) {
            return self
                .parse_optional_expression_except_new(is_constructor_call);
        }

        // Consume `new`; C++ `advance()` returns the OLD range (the `new` range).
        let new_range = self.advance(GrammarContext::AllowRegExp);
        let new_start = new_range.start;

        // new . target (MetaProperty)?
        if self.check_and_eat(TokenKind::period, GrammarContext::AllowDiv) {
            // "new . target" — 3927-3948.
            // After eating `.`, current token should be `target` identifier.
            // We use `get_res_word_or_identifier` because `target` is a plain
            // identifier (not a keyword), but defensively check.
            let target_bytes = if self.cur_kind() == TokenKind::identifier {
                Some(
                    self.lexer
                        .get_string_table()
                        .bytes(self.lexer.token().get_identifier()),
                )
            } else {
                None
            };
            if target_bytes.as_deref() != Some(b"target") {
                self.error_cur("'target' expected in member expression");
                self.lexer.get_source_mgr_mut().note_at(
                    new_start,
                    None,
                    "start of member expression",
                    support::diag::Subsystem::Parser,
                );
                return None;
            }
            // Build MetaProperty(Identifier"new", Identifier"target").
            let new_ident = self
                .gc
                .ctx()
                .atom_table
                .atom_bytes(b"new");
            let target_ident = self
                .gc
                .ctx()
                .atom_table
                .atom_bytes(b"target");
            let meta = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                new_ident,
                None,
                false,
            ));
            let meta_ref = self.set_location(new_start, new_range.end, meta);
            let prop = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                target_ident,
                None,
                false,
            ));
            let prop_tok_start = self.lexer.token().start_loc();
            let prop_tok_end = self.lexer.token().end_loc();
            let prop_ref = self.set_location(prop_tok_start, prop_tok_end, prop);
            // Advance past "target".
            self.advance(GrammarContext::AllowDiv);
            let meta_prop = Node::MetaProperty(MetaProperty::new(
                NodeMetadata::new(self.dummy_range()),
                meta_ref,
                prop_ref,
            ));
            let meta_prop_ref = self.set_location(new_start, prop_tok_end, meta_prop);
            // Then continue with the optional-expression tail.
            return self.parse_optional_expression_except_new_tail(
                is_constructor_call,
                new_start,
                meta_prop_ref,
            );
        }

        // CHECK_RECURSION (line 3950).
        let _guard = self.check_recursion()?;

        // Recurse with IsConstructorCall::Yes to parse the callee.
        let expr = self
            .parse_new_expression_or_optional_expression(IsConstructorCall::Yes)?;

        // Flow/TS typeArgs block (C++ 3957-3975): attempt type-args at a `<`,
        // rolling back if it was a comparison. Unlike call expressions, no `(`
        // is required to commit — `new C<T>` is a valid NewExpression. The C++
        // gate is `(getParseFlowAmbiguous() || getParseTS())`.
        let mut type_args: Option<&'gc Node<'gc>> = None;
        if (self.parse_flow_ambiguous() || self.parse_ts())
            && self.check(TokenKind::less)
        {
            let (opt_type_args, sp) = self.speculative_type_args();
            if opt_type_args.is_some() {
                type_args = opt_type_args;
            } else {
                sp.restore(&mut self.lexer);
            }
        }

        // If there's no `(`, this is `new Foo` (no args) — a NewExpression.
        if !self.check(TokenKind::l_paren) {
            let end = self.lexer.prev_token_end();
            let node = Node::NewExpression(NewExpression::new(
                NodeMetadata::new(self.dummy_range()),
                expr,
                type_args,
                NodeList::empty(),
            ));
            return Some(self.set_location(new_start, end, node));
        }

        // There IS a `(` — parse arguments.
        let debug_loc = self.lexer.token().start_loc();
        let (arg_list, end_loc) = self.parse_arguments()?;
        let node = Node::NewExpression(NewExpression::new(
            NodeMetadata::new(self.dummy_range()),
            expr,
            type_args,
            NodeList::from_iter(self.gc, arg_list),
        ));
        let mut expr = self.set_location_d(new_start, end_loc, debug_loc, node);

        // Handle trailing member selects after `new Foo(args)`:
        // e.g. `new A().b` — the `.b` member-select comes here.
        let mut object_loc = new_start;
        while self.check_n3(
            TokenKind::l_square,
            TokenKind::period,
            TokenKind::questiondot,
        ) {
            let next_object_loc = self.lexer.token().start_loc();
            expr = self.parse_member_select(new_start, object_loc, expr, false)?;
            object_loc = next_object_loc;
        }

        Some(expr)
    }

    // -----------------------------------------------------------------------
    // parseOptionalExpressionExceptNew — P1.6
    // -----------------------------------------------------------------------

    /// Parse a primary/super/import expression and then continue with the
    /// optional-expression tail. Port of
    /// `JSParserImpl::parseOptionalExpressionExceptNew` (3424-3519).
    ///
    /// The `rw_import` arm handles the `import.meta` MetaProperty and the
    /// `import(...)` ImportExpression (dynamic import) forms (P4).
    fn parse_optional_expression_except_new(
        &mut self,
        is_constructor_call: IsConstructorCall,
    ) -> Option<&'gc Node<'gc>> {
        let start_loc = self.cur_start();

        let expr: &'gc Node<'gc> = if self.check(TokenKind::rw_super) {
            // SuperProperty can be used the same way as PrimaryExpression, but
            // must not have a TemplateLiteral immediately after the `super`
            // keyword.
            // C++ JSParserImpl.cpp 3429-3441 (rw_super branch).
            let super_range = self.cur_range();
            // C++ setLocation(tok_, tok_, new SuperNode()).
            let node = self.set_location(
                super_range.start,
                super_range.end,
                Node::Super(Super::new(NodeMetadata::new(self.dummy_range()))),
            );
            self.advance(GrammarContext::AllowRegExp);
            if !self.check_n3(
                TokenKind::l_paren,
                TokenKind::l_square,
                TokenKind::period,
            ) {
                // C++ 3436-3440: errorExpected({l_paren, l_square, period},
                // "after 'super' keyword", "location of 'super'", startLoc).
                // `startLoc` is real (the note text is still dropped per
                // house style).
                self.error_expected_msg(
                    "'(', '[' or '.' expected after 'super' keyword",
                    None,
                    Some(start_loc),
                );
                return None;
            }
            node
        } else if self.check(TokenKind::rw_import) {
            // C++ JSParserImpl.cpp 3442-3509 (rw_import branch).
            // Consume `import`; C++ `advance()` returns the OLD range (the
            // `import` range). Grammar context AllowRegExp matches the
            // surrounding code.
            let import_range = self.advance(GrammarContext::AllowRegExp);
            if self.check_and_eat(TokenKind::period, GrammarContext::AllowRegExp)
            {
                // ImportMeta: import . meta
                //                      ^
                // C++ 3444-3465.
                // C++ 3447 uses `check(metaIdent_)` — the `check(UniqueString*)`
                // overload (JSParserImpl.h:523), which compares the interned
                // identifier and is escape-INsensitive (so `import.meta`
                // is still a valid MetaProperty, matching the `new.target`
                // sibling). Use `check_name`, NOT `check_unescaped_name`.
                if !self.check_name(b"meta") {
                    // C++ error(tok_->getSourceRange(), "'meta' expected in
                    // member expression") plus a note pointing at the start of
                    // the member expression (the `import` keyword). Mirror the
                    // sibling `new.target` error path.
                    self.error_cur("'meta' expected in member expression");
                    self.lexer.get_source_mgr_mut().note_at(
                        import_range.start,
                        None,
                        "start of member expression",
                        support::diag::Subsystem::Parser,
                    );
                    return None;
                }
                // Build MetaProperty(Identifier"import", Identifier"meta").
                // Note: the `meta` node's NAME is the atom `import` (C++ uses
                // `importIdent_` at 3456), located over `import_range`.
                let import_ident =
                    self.gc.ctx().atom_table.atom_bytes(b"import");
                let meta_ident = self.gc.ctx().atom_table.atom_bytes(b"meta");
                let meta = Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    import_ident,
                    None,
                    false,
                ));
                let meta_ref =
                    self.set_location(import_range.start, import_range.end, meta);
                let prop = Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    meta_ident,
                    None,
                    false,
                ));
                let prop_tok_start = self.lexer.token().start_loc();
                let prop_tok_end = self.lexer.token().end_loc();
                let prop_ref =
                    self.set_location(prop_tok_start, prop_tok_end, prop);
                // Advance past "meta".
                self.advance(GrammarContext::AllowRegExp);
                let meta_prop = Node::MetaProperty(MetaProperty::new(
                    NodeMetadata::new(self.dummy_range()),
                    meta_ref,
                    prop_ref,
                ));
                // C++ setLocation(meta, getPrevTokenEndLoc(), ...) — 3462-3465.
                let end = self.lexer.prev_token_end();
                self.set_location(import_range.start, end, meta_prop)
            } else {
                // ImportCall: import ( AssignmentExpression ... ) — C++
                // 3466-3509.
                // Guard against parseAssignmentExpression without
                // parsePrimaryExpression.
                let _guard = self.check_recursion()?;

                // ImportCall must be a call with an AssignmentExpression as the
                // argument.
                if !self.eat(
                    TokenKind::l_paren,
                    GrammarContext::AllowRegExp,
                    "in import call",
                ) {
                    return None;
                }

                let source = self.parse_assignment_expression(PARAM_IN, false, AllowTypedArrowFunction::Yes, CoverTypedParameters::Yes, None)?;

                self.check_and_eat(
                    TokenKind::comma,
                    GrammarContext::AllowRegExp,
                );

                let options = if !self.check(TokenKind::r_paren) {
                    // C++ parseAssignmentExpression() — default param is
                    // ParamIn (JSParserImpl.h 1132-1133).
                    let o = self.parse_assignment_expression(PARAM_IN, false, AllowTypedArrowFunction::Yes, CoverTypedParameters::Yes, None)?;
                    self.check_and_eat(
                        TokenKind::comma,
                        GrammarContext::AllowRegExp,
                    );
                    Some(o)
                } else {
                    None
                };

                // Capture the `)` END before eating it (C++ 3496).
                let end_loc = self.lexer.token().end_loc();
                if !self.eat(
                    TokenKind::r_paren,
                    GrammarContext::AllowRegExp,
                    "in import call",
                ) {
                    return None;
                }

                let node = Node::ImportExpression(ImportExpression::new(
                    NodeMetadata::new(self.dummy_range()),
                    source,
                    options,
                ));
                self.set_location(start_loc, end_loc, node)
            }
        } else {
            self.parse_primary_expression()?
        };

        self.parse_optional_expression_except_new_tail(is_constructor_call, start_loc, expr)
    }

    /// Continue an optional-expression after the base expression by consuming
    /// member-select suffixes (`[…]`, `.id`, `?.id`, `?.(args)`). Port of
    /// `JSParserImpl::parseOptionalExpressionExceptNew_tail` (3521-3592).
    ///
    /// ### Recursion-depth accounting
    ///
    /// The C++ uses `SaveAndRestore<unsigned> savedRecursionDepth{recursionDepth_,
    /// recursionDepth_}` (saves a copy, restores on return) and then
    /// `++recursionDepth_; recursionDepthCheck()` on each loop iteration. The
    /// intent is:
    ///   - Each *call to the tail* starts from the current depth.
    ///   - Each *iteration* of the loop increments the global counter by 1, so
    ///     a very long chain (`a.b.c.d…`) can still hit the limit.
    ///   - At the end of the tail (however it exits) the counter is restored to
    ///     the value it had when the tail was entered (the `SaveAndRestore`).
    ///
    /// In Rust we replicate this with an explicit save/restore:
    ///   1. Save `self.recursion_depth.get()` before the loop.
    ///   2. Each iteration increments by 1 and calls `recursion_depth_check`.
    ///   3. After the loop (or on early return from it) we restore the saved
    ///      value.
    ///
    /// This matches the C++ semantics without a per-iteration RAII guard
    /// (which would under-count, only tracking one level).
    ///
    /// A template literal immediately following the expression forms a tagged
    /// template (P1.9).
    pub(in crate::js) fn parse_optional_expression_except_new_tail(
        &mut self,
        is_constructor_call: IsConstructorCall,
        start_loc: support::location::SMLoc,
        mut expr: &'gc Node<'gc>,
    ) -> Option<&'gc Node<'gc>> {
        let mut object_loc = start_loc;
        let mut seen_optional_chain = false;

        // Save the recursion depth before the loop; restore on exit (mirrors
        // C++ `SaveAndRestore<unsigned> savedRecursionDepth`).
        let saved_depth = self.recursion_depth.get();

        loop {
            // checkN(l_square, period, questiondot) || checkTemplateLiteral()
            let is_member = self.check_n3(
                TokenKind::l_square,
                TokenKind::period,
                TokenKind::questiondot,
            );
            let is_template = self.check2(
                TokenKind::no_substitution_template,
                TokenKind::template_head,
            );
            if !is_member && !is_template {
                break;
            }

            // ++recursionDepth_; if (LLVM_UNLIKELY(recursionDepthCheck())) return None;
            let new_depth = self.recursion_depth.get() + 1;
            self.recursion_depth.set(new_depth);
            // `>=`, not `>`: `recursionDepthCheck()` (JSParserImpl.h:699-704)
            // errors unless the POST-increment depth is still
            // `< MAX_RECURSION_DEPTH`. Same boundary as `check_recursion`.
            if new_depth >= super::MAX_RECURSION_DEPTH {
                // Point location, not a range — `recursionDepthCheck()` routes
                // to `recursionDepthExceeded` (cpp:348-352), which uses the
                // `error(SMLoc, Twine)` overload (JSParserImpl.h:472-474).
                let loc = self.cur_start();
                self.error_at_loc(
                    loc,
                    "Too many nested expressions/statements/declarations",
                );
                // Restore before returning.
                self.recursion_depth.set(saved_depth);
                return None;
            }

            let next_object_loc = self.lexer.token().start_loc();

            if is_member {
                if self.check(TokenKind::questiondot) {
                    seen_optional_chain = true;
                    if is_constructor_call == IsConstructorCall::Yes {
                        // Report but continue — C++ does the same.
                        let range = self.cur_range();
                        self.error_at(
                            range,
                            "Constructor calls may not contain an optional chain",
                        );
                    }
                }
                // MemberExpression [ Expression ]
                // MemberExpression . IdentifierName
                // MemberExpression OptionalChain
                let new_expr = self.parse_member_select(
                    start_loc,
                    object_loc,
                    expr,
                    seen_optional_chain,
                );
                object_loc = next_object_loc;
                // Restore depth before potential early return.
                self.recursion_depth.set(saved_depth);
                expr = new_expr?;
                // Re-save depth for the next iteration (the C++ restore only
                // happens at the top of SaveAndRestore scope, i.e. on return).
                // We mimic this by keeping saved_depth constant and restoring
                // after each parse_member_select call, but since the loop
                // continues, we must re-establish the invariant. The C++ counter
                // STAYS incremented across iterations (SaveAndRestore only
                // restores on function return, not per-iteration). So we must
                // NOT reset saved_depth here — we let the depth accumulate.
                // Re-set to the new (incremented) value.
                self.recursion_depth.set(new_depth);
            } else {
                // Tagged template literal branch — P1.9.
                // C++ 3559-3587: `super` as tag is an error (P3: unreachable here);
                // optional chain + template is a static-semantics error.
                debug_assert!(is_template);
                if seen_optional_chain {
                    let range = self.cur_range();
                    self.error_at(
                        range,
                        "invalid use of tagged template literal in optional chain",
                    );
                    // Note the location of the optional chain.
                    self.lexer.get_source_mgr_mut().note_at(
                        expr.range().start,
                        None,
                        "location of optional chain",
                        support::diag::Subsystem::Parser,
                    );
                    self.recursion_depth.set(saved_depth);
                    // Deviation: C++ (3566-3577) emits this diagnostic and
                    // CONTINUES to build the TaggedTemplateExpression (error
                    // recovery). We abort instead. Unobservable in -dump-ast
                    // (errored input produces no AST either way); revisit with
                    // the broader error-recovery fidelity work (see the
                    // error-limit/force_eof TODO in mod.rs).
                    return None;
                }
                let quasi = self.parse_template_literal(PARAM_TAGGED);
                self.recursion_depth.set(saved_depth);
                let quasi = quasi?;
                let quasi_end = quasi.range().end;
                let tagged = Node::TaggedTemplateExpression(TaggedTemplateExpression::new(
                    NodeMetadata::new(self.dummy_range()),
                    expr,
                    quasi,
                ));
                // C++ `setLocation(startLoc, optTemplate.getValue(), ...)` —
                // 3-arg (debug = start).
                expr = self.set_location(start_loc, quasi_end, tagged);
                object_loc = next_object_loc;
                // Re-save the depth for the next iteration.
                self.recursion_depth.set(new_depth);
            }
        }

        // Restore the recursion depth to the value before this tail call.
        self.recursion_depth.set(saved_depth);
        Some(expr)
    }

    // -----------------------------------------------------------------------
    // parse_arguments — P1.6
    // -----------------------------------------------------------------------

    /// Parse a function call's argument list: `( arg, ...arg, arg )`. Port of
    /// `JSParserImpl::parseArguments` (3594-3647).
    ///
    /// Returns the argument node-list and the end location (the `)` end).
    /// Each `...expr` becomes a `SpreadElement`; plain expressions are passed
    /// through.
    ///
    /// Faithfully ports the trailing-comma + spread-before-arrow error check
    /// (3628-3632): if there is a trailing comma after a spread and `=>` follows,
    /// error "Rest parameter must be last formal parameter". In P1 arrow
    /// functions are deferred, so this error is never triggered in practice,
    /// but the check is present for correctness.
    pub(super) fn parse_arguments(
        &mut self,
    ) -> Option<(Vec<&'gc Node<'gc>>, support::location::SMLoc)> {
        // Consume `(`.
        let l_paren_range = self.advance(GrammarContext::AllowRegExp);
        let l_paren_start = l_paren_range.start;

        let mut arg_list: Vec<&'gc Node<'gc>> = Vec::new();

        if !self.check(TokenKind::r_paren) {
            let mut last_was_spread;
            loop {
                let arg_start = self.lexer.token().start_loc();
                let is_spread =
                    self.check_and_eat(TokenKind::dotdotdot, GrammarContext::AllowRegExp);

                let arg = self.parse_assignment_expression(PARAM_IN, false, AllowTypedArrowFunction::Yes, CoverTypedParameters::Yes, None)?;

                if is_spread {
                    let spread_end = self.lexer.prev_token_end();
                    let node = Node::SpreadElement(SpreadElement::new(
                        NodeMetadata::new(self.dummy_range()),
                        arg,
                    ));
                    let node_ref = self.set_location(arg_start, spread_end, node);
                    arg_list.push(node_ref);
                } else {
                    arg_list.push(arg);
                }
                last_was_spread = is_spread;

                if !self.check_and_eat(TokenKind::comma, GrammarContext::AllowRegExp) {
                    break;
                }

                // Check for ",)" — trailing comma before ")".
                if self.check(TokenKind::r_paren) {
                    let end_loc = self.lexer.token().end_loc();
                    self.advance(GrammarContext::AllowDiv);
                    // If we saw a spread and `=>` follows, that's an async-arrow
                    // rest-parameter error (C++ 3628-3632). Port faithfully even
                    // though `=>` errors elsewhere in P1.
                    if last_was_spread && self.check(TokenKind::equalgreater) {
                        let err_loc = arg_list.last().unwrap().range().end;
                        self.lexer.get_source_mgr_mut().error_at(
                            err_loc,
                            None,
                            "Rest parameter must be last formal parameter",
                            support::diag::Subsystem::Parser,
                        );
                    }
                    return Some((arg_list, end_loc));
                }
            }
        }

        // Consume the closing `)`.
        let end_loc = self.lexer.token().end_loc();
        if !self.eat(
            TokenKind::r_paren,
            GrammarContext::AllowDiv,
            "at end of function call",
        ) {
            // Emit a note pointing to the opening `(`.
            self.lexer.get_source_mgr_mut().note_at(
                l_paren_start,
                None,
                "location of '('",
                support::diag::Subsystem::Parser,
            );
            return None;
        }

        Some((arg_list, end_loc))
    }

    // -----------------------------------------------------------------------
    // parse_array_literal — P1.7
    // -----------------------------------------------------------------------

    /// Parse an array literal: `[ elem, , ...spread, ]`. Port of
    /// `JSParserImpl::parseArrayLiteral` (2711-2763).
    ///
    /// Elements:
    /// - Elision (bare `,`) → `EmptyNode` located at the comma token.
    /// - `...expr` → `SpreadElement` via `parse_spread_element`.
    /// - Otherwise → `parse_assignment_expression`.
    ///
    /// Trailing `,` before `]` sets `trailingComma = true`.
    fn parse_array_literal(&mut self) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check(TokenKind::l_square));

        // Consume `[`; record its start for the final setLocation.
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        let mut elem_list: Vec<&'gc Node<'gc>> = Vec::new();
        let mut trailing_comma = false;

        if !self.check(TokenKind::r_square) {
            loop {
                if self.check(TokenKind::comma) {
                    // Elision: bare `,` → Empty node located at the comma.
                    let comma_range = self.cur_range();
                    let empty_node = Node::Empty(Empty::new(NodeMetadata::new(self.dummy_range())));
                    let empty_ref =
                        self.set_location(comma_range.start, comma_range.end, empty_node);
                    elem_list.push(empty_ref);
                } else if self.check(TokenKind::dotdotdot) {
                    // Spread: `...assignmentExpr`.
                    let spread_ref = self.parse_spread_element()?;
                    elem_list.push(spread_ref);
                } else {
                    // Regular assignment expression. (C++ parseArrayLiteral has
                    // no CHECK_RECURSION here — the recursion guards live in the
                    // expression chain it calls into.)
                    let expr = self.parse_assignment_expression(PARAM_IN, false, AllowTypedArrowFunction::Yes, CoverTypedParameters::Yes, None)?;
                    elem_list.push(expr);
                }

                if !self.check_and_eat(TokenKind::comma, GrammarContext::AllowRegExp) {
                    break;
                }
                if self.check(TokenKind::r_square) {
                    // Trailing `,` before `]`.
                    trailing_comma = true;
                    break;
                }
            }
        }

        let end_loc = self.lexer.token().end_loc();
        if !self.eat(
            TokenKind::r_square,
            GrammarContext::AllowDiv,
            "at end of array literal '[...'",
        ) {
            self.lexer.get_source_mgr_mut().note_at(
                start_loc,
                None,
                "location of '['",
                support::diag::Subsystem::Parser,
            );
            return None;
        }

        let node = Node::ArrayExpression(ArrayExpression::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, elem_list),
            trailing_comma,
        ));
        Some(self.set_location(start_loc, end_loc, node))
    }

    // -----------------------------------------------------------------------
    // parse_spread_element — P1.7
    // -----------------------------------------------------------------------

    /// Parse a spread element: `... assignmentExpr`. Port of
    /// `JSParserImpl::parseSpreadElement` (2815-2827).
    ///
    /// Located from the `...` start to `prev_token_end()` (the end of the
    /// argument expression).
    fn parse_spread_element(&mut self) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check(TokenKind::dotdotdot));

        // Consume `...`; record its start.
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        // (C++ parseSpreadElement has no CHECK_RECURSION; the guard lives in the
        // expression chain it calls into.)
        let arg = self.parse_assignment_expression(PARAM_IN, false, AllowTypedArrowFunction::Yes, CoverTypedParameters::Yes, None)?;

        let end_loc = self.lexer.prev_token_end();
        let node = Node::SpreadElement(SpreadElement::new(NodeMetadata::new(self.dummy_range()), arg));
        Some(self.set_location(start_loc, end_loc, node))
    }

    // -----------------------------------------------------------------------
    // check_unescaped_name — P1.8 helper
    // -----------------------------------------------------------------------

    /// True if the current token is an `identifier` whose interned bytes equal
    /// `name` AND the token has no `\u` escapes. Port of `checkUnescaped`
    /// (JSParserImpl.h:538-543) + `isUnescaped` (529-534).
    ///
    /// The escape check mirrors C++ `isUnescaped`: a unicode escape like `get`
    /// encodes `get` but its source form is 11 bytes wide, not 3. An unescaped
    /// identifier has source width == interned byte count.
    #[inline]
    pub(super) fn check_unescaped_name(&self, name: &[u8]) -> bool {
        if self.cur_kind() != TokenKind::identifier {
            return false;
        }
        let bytes = self
            .lexer
            .get_string_table()
            .bytes(self.lexer.token().get_identifier());
        if bytes != name {
            return false;
        }
        // isUnescaped: token source range length == identifier byte length.
        let tok_range = self.lexer.token().source_range();
        let tok_len = (tok_range.end.offset - tok_range.start.offset) as usize;
        tok_len == name.len()
    }

    /// True if the current token is an identifier whose interned name equals
    /// `name`, regardless of whether it was written with escapes. Port of the
    /// C++ `check(<UniqueString *>)` overload (which compares `tok_` against an
    /// interned identifier such as `getIdent_`/`setIdent_`), used by
    /// `parseClassElement` to detect `get`/`set` accessor specifiers (escaped
    /// `get` is still a getter in the C++ parser).
    pub(super) fn check_name(&self, name: &[u8]) -> bool {
        if self.cur_kind() != TokenKind::identifier {
            return false;
        }
        let bytes = self
            .lexer
            .get_string_table()
            .bytes(self.lexer.token().get_identifier());
        bytes == name
    }

    // -----------------------------------------------------------------------
    // parse_object_literal — P1.8
    // -----------------------------------------------------------------------

    /// Parse an object literal: `{ prop, ... }`. Port of
    /// `JSParserImpl::parseObjectLiteral` (2792-2813).
    ///
    /// Delegates property parsing to `parse_object_properties`, then wraps
    /// the result in an `ObjectExpression`.
    fn parse_object_literal(&mut self) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check(TokenKind::l_brace));

        // Consume `{`; record its start for the final setLocation.
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        let mut elem_list: Vec<&'gc Node<'gc>> = Vec::new();
        if !self.parse_object_properties(&mut elem_list) {
            return None;
        }

        let end_loc = self.lexer.token().end_loc();
        if !self.eat(
            TokenKind::r_brace,
            GrammarContext::AllowDiv,
            " at end of object literal '{'",
        ) {
            self.lexer.get_source_mgr_mut().note_at(
                start_loc,
                None,
                "location of '{'",
                support::diag::Subsystem::Parser,
            );
            return None;
        }

        let node = Node::ObjectExpression(ObjectExpression::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, elem_list),
        ));
        Some(self.set_location(start_loc, end_loc, node))
    }

    // -----------------------------------------------------------------------
    // parse_object_properties — P1.8
    // -----------------------------------------------------------------------

    /// Parse the comma-separated list of object properties. Port of
    /// `JSParserImpl::parseObjectProperties` (2765-2790).
    ///
    /// Stops on `}` (not consumed). Returns false on parse error.
    pub(super) fn parse_object_properties(
        &mut self,
        elem_list: &mut Vec<&'gc Node<'gc>>,
    ) -> bool {
        if self.check(TokenKind::r_brace) {
            return true;
        }

        loop {
            if self.check(TokenKind::dotdotdot) {
                // Spread element.
                let spread = match self.parse_spread_element() {
                    Some(n) => n,
                    None => return false,
                };
                elem_list.push(spread);
            } else {
                let prop = match self.parse_property_assignment(false) {
                    Some(n) => n,
                    None => return false,
                };
                elem_list.push(prop);
            }

            // Consume comma, then stop on `}` (trailing comma allowed).
            if !self.check_and_eat(TokenKind::comma, GrammarContext::AllowRegExp) {
                break;
            }
            if self.check(TokenKind::r_brace) {
                break;
            }
        }
        true
    }

    // -----------------------------------------------------------------------
    // parse_property_name — P1.8
    // -----------------------------------------------------------------------

    /// Parse a property name for an object literal or class member. Port of
    /// `JSParserImpl::parsePropertyName` (3268-3340).
    ///
    /// Handles:
    /// - String literal key → `StringLiteralNode`
    /// - Numeric literal key → `NumericLiteralNode`
    /// - BigInt literal key → `BigIntLiteralNode`
    /// - `identifier` key (plain ident, not a reserved word) → `IdentifierNode`
    /// - `[expr]` computed key → the expression itself (caller tracks
    ///   `computed = true`)
    /// - Reserved word used as key → `IdentifierNode` (e.g. `{if: 1}`)
    pub(super) fn parse_property_name(&mut self) -> Option<&'gc Node<'gc>> {
        let tok_start = self.lexer.token().start_loc();
        let tok_end = self.lexer.token().end_loc();

        match self.cur_kind() {
            TokenKind::string_literal => {
                let value = self.lexer.token().get_string_literal();
                let node = Node::StringLiteral(StringLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    value,
                ));
                let res = self.set_location(tok_start, tok_end, node);
                self.advance(GrammarContext::AllowRegExp);
                Some(res)
            }

            TokenKind::numeric_literal => {
                let value = self.lexer.token().get_numeric_literal();
                let node = Node::NumericLiteral(NumericLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    value,
                ));
                let res = self.set_location(tok_start, tok_end, node);
                self.advance(GrammarContext::AllowRegExp);
                Some(res)
            }

            TokenKind::bigint_literal => {
                let bigint = self.lexer.token().get_bigint_literal();
                let node = Node::BigIntLiteral(BigIntLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    bigint,
                ));
                let res = self.set_location(tok_start, tok_end, node);
                self.advance(GrammarContext::AllowRegExp);
                Some(res)
            }

            TokenKind::identifier => {
                let name = self.lexer.token().get_identifier();
                let node = Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    name,
                    None,
                    false,
                ));
                let res = self.set_location(tok_start, tok_end, node);
                self.advance(GrammarContext::AllowRegExp);
                Some(res)
            }

            TokenKind::l_square => {
                // Computed key: `[expr]`.
                let start_loc = self.advance(GrammarContext::AllowRegExp).start;
                let opt_expr = self.parse_assignment_expression(PARAM_IN, false, AllowTypedArrowFunction::Yes, CoverTypedParameters::Yes, None)?;
                if !self.need(TokenKind::r_square, " at end of computed property key") {
                    self.lexer.get_source_mgr_mut().note_at(
                        start_loc,
                        None,
                        "start of property key",
                        support::diag::Subsystem::Parser,
                    );
                    return None;
                }
                self.advance(GrammarContext::AllowRegExp);
                Some(opt_expr)
            }

            _ => {
                // Reserved word used as a property name (e.g. `{if: 1}`).
                if self.lexer.token().is_res_word() {
                    let name = self.lexer.token().get_res_word_identifier();
                    let node = Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        name,
                        None,
                        false,
                    ));
                    let res = self.set_location(tok_start, tok_end, node);
                    self.advance(GrammarContext::AllowRegExp);
                    Some(res)
                } else {
                    self.error_cur(
                        "invalid property name - must be a string, number or identifier",
                    );
                    None
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // parse_property_assignment — P1.8
    // -----------------------------------------------------------------------

    /// Parse a single object property. Port of
    /// `JSParserImpl::parsePropertyAssignment` (2829-3266).
    ///
    /// ## Data paths (P1.8)
    /// - `get`/`set` as data property (`{get: 1}`, `{set: 1}`, `{get}`, `{set}`)
    /// - `async` as data property (`{async: 1}`, `{async}`)
    /// - Plain `identifier` — shorthand (`{a}`) or keyed (`{a: 1}`)
    /// - Computed key (`{[expr]: val}`)
    /// - String/numeric/bigint keys (`{"k": 1}`, `{0: 1}`)
    /// - Shorthand (`{a}`)
    /// - `CoverInitializedName` (`{a = 1}`)
    ///
    /// ## Method paths (P3.4)
    /// - Getter/setter bodies (`get foo() {}`, `set foo(v) {}`)
    /// - Async methods (`async foo() {}`, `async *gen() {}`, `async [k]() {}`)
    /// - Generator methods (`*foo() {}`, `*[k]() {}`)
    /// - Plain method definitions (`foo() {}`, `[k]() {}`, `'s'() {}`, `0() {}`)
    ///
    /// ## SaveFunctionState note
    /// `SaveFunctionState saveFunctionState{this}` (C++ 2833) saves and restores
    /// parser flags clobbered when entering a method/getter/setter body. The
    /// `param_yield`/`param_await` flags are saved/restored locally via
    /// [`Self::save_param_yield`]/[`Self::save_param_await`] ParamFlagGuards at
    /// each method leaf. The OTHER observable flag SaveFunctionState restores is
    /// the lexer `strictMode` — a `"use strict"` directive inside a method body
    /// must not leak strictness to the enclosing object-literal expression — so
    /// this wrapper saves/restores it around the whole property parse (the
    /// result is computed first so the restore runs on every error `?` path).
    pub(super) fn parse_property_assignment(
        &mut self,
        eagerly: bool,
    ) -> Option<&'gc Node<'gc>> {
        let old_strict = self.lexer.is_strict_mode();
        // SaveFunctionState for object method/getter/setter scope — mirrors
        // the SaveFunctionState constructed for each method in C++.
        // is_arrow=false: method is a regular function scope.
        let _g = self.save_function_state(false);
        let old_seen_len = self.seen_directives.len();
        let result = self.parse_property_assignment_inner(eagerly);
        self.seen_directives.truncate(old_seen_len);
        self.lexer.set_strict_mode(old_strict);
        result
    }

    fn parse_property_assignment_inner(
        &mut self,
        eagerly: bool,
    ) -> Option<&'gc Node<'gc>> {
        let start_loc = self.cur_start();

        let mut computed = false;
        // `generator`/`async`/`method` start false; the method leaves set them
        // before falling into the shared value logic (C++ 2835-2838).
        let mut generator = false;
        let mut async_ = false;
        let mut method = false;
        let key: &'gc Node<'gc>;

        if self.check_unescaped_name(b"get") {
            // Could be a getter or a property named "get".
            let ident = self.lexer.token().get_identifier();
            let ident_rng = self.lexer.token().source_range();
            self.advance(GrammarContext::AllowRegExp);

            if self.check2(TokenKind::colon, TokenKind::l_paren) {
                // `{get: value}` or `{get(…) {…}}` — data property "get".
                // (Method case deferred below.)
                key = self.set_location(
                    ident_rng.start,
                    ident_rng.end,
                    Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        ident,
                        None,
                        false,
                    )),
                );
                // Fall through to value logic.
            } else if self.parse_types() && self.check(TokenKind::less) {
                // `{get<T>(…) {…}}` — a method named "get" with type params.
                // C++ 2852-2860.
                method = true;
                key = self.set_location(
                    ident_rng.start,
                    ident_rng.end,
                    Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        ident,
                        None,
                        false,
                    )),
                );
                // Fall through to value logic (which sees the `<`).
            } else if self.check2(TokenKind::comma, TokenKind::r_brace) {
                // Shorthand `{get}`.
                key = self.set_location(
                    ident_rng.start,
                    ident_rng.end,
                    Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        ident,
                        None,
                        false,
                    )),
                );
                let value = self.set_location(
                    ident_rng.start,
                    ident_rng.end,
                    Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        ident,
                        None,
                        false,
                    )),
                );
                let init_kind = self.gc.ctx().atom_table.atom_bytes(b"init");
                let prop = Node::Property(Property::new(
                    NodeMetadata::new(self.dummy_range()),
                    key,
                    value,
                    init_kind,
                    false,
                    false,
                    true,
                ));
                return Some(self.set_location(start_loc, value.range().end, prop));
            } else {
                // A getter method (C++ 2877-2943): `get propName() { … }`.
                computed = self.check(TokenKind::l_square);
                let opt_key = self.parse_property_name()?;

                let paren_loc = self.lexer.token().start_loc();
                if !self.eat(
                    TokenKind::l_paren,
                    GrammarContext::AllowRegExp,
                    " in getter declaration",
                ) {
                    self.lexer.get_source_mgr_mut().note_at(
                        start_loc,
                        None,
                        "start of getter declaration",
                        support::diag::Subsystem::Parser,
                    );
                    return None;
                }
                if !self.eat(
                    TokenKind::r_paren,
                    GrammarContext::AllowRegExp,
                    " in empty getter parameter list",
                ) {
                    self.lexer.get_source_mgr_mut().note_at(
                        start_loc,
                        None,
                        "start of getter declaration",
                        support::diag::Subsystem::Parser,
                    );
                    return None;
                }

                // `: ReturnType`. C++ 2900-2909.
                let mut return_type: Option<&'gc Node<'gc>> = None;
                if self.parse_types() && self.check(TokenKind::colon) {
                    let annot_start = self.advance(GrammarContext::Type).start;
                    return_type = Some(self.parse_return_type_annotation(
                        Some(annot_start),
                        AllowAnonFunctionType::Yes,
                    )?);
                }

                // C++ 2911-2912: a getter body is neither yield- nor
                // await-contextual.
                let _guard_yield = self.save_param_yield(false);
                let _guard_await = self.save_param_await(false);
                if !self.need(TokenKind::l_brace, " in getter declaration") {
                    self.lexer.get_source_mgr_mut().note_at(
                        start_loc,
                        None,
                        "start of getter declaration",
                        support::diag::Subsystem::Parser,
                    );
                    return None;
                }
                let block = self.parse_function_body(
                    PARAM_RETURN,
                    eagerly,
                    false,
                    false,
                    GrammarContext::AllowRegExp,
                    true,
                )?;
                let body_end = block.range().end;

                let func = FunctionExpression::new(
                    NodeMetadata::new(self.dummy_range()),
                    None,
                    NodeList::empty(),
                    block,
                    None,
                    return_type,
                    None,
                    false,
                    false,
                );
                func.is_method_definition.set(true);
                let func_expr = self.set_location(
                    paren_loc,
                    body_end,
                    Node::FunctionExpression(func),
                );

                let get_kind = self.gc.ctx().atom_table.atom_bytes(b"get");
                let prop = Node::Property(Property::new(
                    NodeMetadata::new(self.dummy_range()),
                    opt_key,
                    func_expr,
                    get_kind,
                    computed,
                    false,
                    false,
                ));
                return Some(self.set_location(start_loc, body_end, prop));
            }
        } else if self.check_unescaped_name(b"set") {
            // Could be a setter or a property named "set".
            let ident = self.lexer.token().get_identifier();
            let ident_rng = self.lexer.token().source_range();
            self.advance(GrammarContext::AllowRegExp);

            if self.check2(TokenKind::colon, TokenKind::l_paren) {
                // `{set: value}` or `{set(…) {…}}` — data property "set".
                key = self.set_location(
                    ident_rng.start,
                    ident_rng.end,
                    Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        ident,
                        None,
                        false,
                    )),
                );
                // Fall through to value logic.
            } else if self.parse_types() && self.check(TokenKind::less) {
                // `{set<T>(…) {…}}` — a method named "set" with type params.
                // C++ 2957-2965.
                method = true;
                key = self.set_location(
                    ident_rng.start,
                    ident_rng.end,
                    Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        ident,
                        None,
                        false,
                    )),
                );
                // Fall through to value logic (which sees the `<`).
            } else if self.check2(TokenKind::comma, TokenKind::r_brace) {
                // Shorthand `{set}`.
                key = self.set_location(
                    ident_rng.start,
                    ident_rng.end,
                    Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        ident,
                        None,
                        false,
                    )),
                );
                let value = self.set_location(
                    ident_rng.start,
                    ident_rng.end,
                    Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        ident,
                        None,
                        false,
                    )),
                );
                let init_kind = self.gc.ctx().atom_table.atom_bytes(b"init");
                let prop = Node::Property(Property::new(
                    NodeMetadata::new(self.dummy_range()),
                    key,
                    value,
                    init_kind,
                    false,
                    false,
                    true,
                ));
                return Some(self.set_location(start_loc, value.range().end, prop));
            } else {
                // A setter method (C++ 2982-3055): `set propName(v) { … }`.
                computed = self.check(TokenKind::l_square);
                let opt_key = self.parse_property_name()?;

                // C++ 2989-2990: a setter body is neither yield- nor
                // await-contextual.
                let _guard_yield = self.save_param_yield(false);
                let _guard_await = self.save_param_await(false);

                let paren_loc = self.lexer.token().start_loc();
                self.eat(
                    TokenKind::l_paren,
                    GrammarContext::AllowRegExp,
                    " in setter declaration",
                );

                // PropertySetParameterList -> FormalParameter -> BindingElement.
                let param = self.parse_binding_element(Param::default())?;

                if !self.eat(
                    TokenKind::r_paren,
                    GrammarContext::AllowRegExp,
                    " at end of setter parameter list",
                ) {
                    self.lexer.get_source_mgr_mut().note_at(
                        start_loc,
                        None,
                        "start of setter declaration",
                        support::diag::Subsystem::Parser,
                    );
                    return None;
                }

                // `: ReturnType`. C++ 3014-3023.
                let mut return_type: Option<&'gc Node<'gc>> = None;
                if self.parse_types() && self.check(TokenKind::colon) {
                    let annot_start = self.advance(GrammarContext::Type).start;
                    return_type = Some(self.parse_return_type_annotation(
                        Some(annot_start),
                        AllowAnonFunctionType::Yes,
                    )?);
                }

                if !self.need(TokenKind::l_brace, " in setter declaration") {
                    self.lexer.get_source_mgr_mut().note_at(
                        start_loc,
                        None,
                        "start of setter declaration",
                        support::diag::Subsystem::Parser,
                    );
                    return None;
                }
                let block = self.parse_function_body(
                    PARAM_RETURN,
                    eagerly,
                    false,
                    false,
                    GrammarContext::AllowRegExp,
                    true,
                )?;
                let body_end = block.range().end;

                let params = NodeList::from_iter(self.gc, [param]);
                let func = FunctionExpression::new(
                    NodeMetadata::new(self.dummy_range()),
                    None,
                    params,
                    block,
                    None,
                    return_type,
                    None,
                    false,
                    false,
                );
                func.is_method_definition.set(true);
                let func_expr = self.set_location(
                    paren_loc,
                    body_end,
                    Node::FunctionExpression(func),
                );

                let set_kind = self.gc.ctx().atom_table.atom_bytes(b"set");
                let prop = Node::Property(Property::new(
                    NodeMetadata::new(self.dummy_range()),
                    opt_key,
                    func_expr,
                    set_kind,
                    computed,
                    false,
                    false,
                ));
                return Some(self.set_location(start_loc, body_end, prop));
            }
        } else if self.check_unescaped_name(b"async") {
            // Could be an async method or a property named "async".
            let ident = self.lexer.token().get_identifier();
            let ident_rng = self.lexer.token().source_range();
            self.advance(GrammarContext::AllowRegExp);

            if self.check2(TokenKind::colon, TokenKind::l_paren) {
                // `{async: value}` or `{async(…) {…}}` — data property "async".
                // (Method case `{async(…) {…}}` deferred below.)
                key = self.set_location(
                    ident_rng.start,
                    ident_rng.end,
                    Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        ident,
                        None,
                        false,
                    )),
                );
                // Fall through to value logic.
            } else if self.parse_types() && self.check(TokenKind::less) {
                // `{async<T>(…) {…}}` — a method named "async" with type
                // params. C++ 3069-3077.
                method = true;
                key = self.set_location(
                    ident_rng.start,
                    ident_rng.end,
                    Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        ident,
                        None,
                        false,
                    )),
                );
                // Fall through to value logic (which sees the `<`).
            } else if self.check2(TokenKind::comma, TokenKind::r_brace) {
                // Shorthand `{async}`.
                key = self.set_location(
                    ident_rng.start,
                    ident_rng.end,
                    Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        ident,
                        None,
                        false,
                    )),
                );
                let value = self.set_location(
                    ident_rng.start,
                    ident_rng.end,
                    Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        ident,
                        None,
                        false,
                    )),
                );
                let init_kind = self.gc.ctx().atom_table.atom_bytes(b"init");
                let prop = Node::Property(Property::new(
                    NodeMetadata::new(self.dummy_range()),
                    key,
                    value,
                    init_kind,
                    false,
                    false,
                    true,
                ));
                return Some(self.set_location(start_loc, value.range().end, prop));
            } else {
                // An async method (C++ 3094-3110): `async name() {}`,
                // `async *gen() {}`, `async [k]() {}`.
                if self.lexer.is_new_line_before_current_token() {
                    self.error_cur(
                        "newline not allowed after 'async' in a method definition",
                    );
                }
                // This is an async function: parse the key and set `async`.
                async_ = true;
                method = true;
                generator =
                    self.check_and_eat(TokenKind::star, GrammarContext::AllowRegExp);
                computed = self.check(TokenKind::l_square);
                key = self.parse_property_name()?;
                // Fall through to the shared value logic.
            }
        } else if self.check(TokenKind::identifier) {
            // Plain identifier key.
            let ident = self.lexer.token().get_identifier();
            let tok_start = self.lexer.token().start_loc();
            let tok_end = self.lexer.token().end_loc();
            key = self.set_location(
                tok_start,
                tok_end,
                Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    ident,
                    None,
                    false,
                )),
            );
            self.advance(GrammarContext::AllowRegExp);

            // Shorthand if next is `,` or `}`.
            if self.check2(TokenKind::comma, TokenKind::r_brace) {
                let value = self.set_location(
                    tok_start,
                    tok_end,
                    Node::Identifier(Identifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        ident,
                        None,
                        false,
                    )),
                );
                let init_kind = self.gc.ctx().atom_table.atom_bytes(b"init");
                let prop = Node::Property(Property::new(
                    NodeMetadata::new(self.dummy_range()),
                    key,
                    value,
                    init_kind,
                    false,
                    false,
                    true,
                ));
                return Some(self.set_location(start_loc, value.range().end, prop));
            }
            // Otherwise fall through to value logic.
        } else {
            // C++ 3131-3139: a generator method (`*name() {}`, `*[k]() {}`), or
            // a computed/string/numeric/bigint-keyed property or method.
            generator =
                self.check_and_eat(TokenKind::star, GrammarContext::AllowRegExp);
            computed = self.check(TokenKind::l_square);
            key = self.parse_property_name()?;
        }

        // -----------------------------------------------------------------------
        // Value logic (C++ lines 3141-3265).
        // -----------------------------------------------------------------------

        let mut shorthand = false;

        // CoverInitializedName: IdentifierReference `=` Initializer (C++ 3144-3157).
        // This fires for shorthand patterns like `{a = 1}` used in destructuring covers.
        if matches!(key, Node::Identifier(_)) && self.check(TokenKind::equal) && !computed {
            // Advance past `=`; the start of the CoverInitializer is the `=`.
            let cover_start = self.advance(GrammarContext::AllowRegExp).start;
            let init_expr = self.parse_assignment_expression(PARAM_IN, false, AllowTypedArrowFunction::Yes, CoverTypedParameters::Yes, None)?;
            shorthand = true;

            let cover_end = self.lexer.prev_token_end();
            let value = self.set_location(
                cover_start,
                cover_end,
                Node::CoverInitializer(CoverInitializer::new(
                    NodeMetadata::new(self.dummy_range()),
                    init_expr,
                )),
            );
            let init_kind = self.gc.ctx().atom_table.atom_bytes(b"init");
            let prop = Node::Property(Property::new(
                NodeMetadata::new(self.dummy_range()),
                key,
                value,
                init_kind,
                computed,
                false,
                shorthand,
            ));
            let end_loc = self.lexer.prev_token_end();
            return Some(self.set_location(start_loc, end_loc, prop));
        }

        let value: &'gc Node<'gc>;

        // Method definition (C++ 3158-3245): try this when we have '(' or '<'
        // (a type-param list; the `less` check is unconditional in C++ — a `<`
        // after a property key always routes here) to indicate a method, OR
        // when we already know this is `async` (which must indicate a method,
        // so we must avoid parsing an ordinary property from ':').
        if self.check2(TokenKind::l_paren, TokenKind::less) || async_ {
            // Parse the MethodDefinition manually here (we already consumed the
            // PropertyName above):
            //   PropertyName "(" UniqueFormalParameters ")" "{" FunctionBody "}"
            //                ^
            let _guard_yield = self.save_param_yield(generator);
            let _guard_await = self.save_param_await(async_);

            method = true;

            // Flow method type parameters. C++ 3175-3183.
            let mut type_params: Option<&'gc Node<'gc>> = None;
            if self.parse_flow() && self.check(TokenKind::less) {
                type_params = Some(self.parse_type_params_flow()?);
            }
            // TS method type parameters. C++ 3184-3191.
            if self.parse_ts() && self.check(TokenKind::less) {
                type_params = Some(self.parse_ts_type_parameters()?);
            }

            // (
            let paren_loc = self.lexer.token().start_loc();
            if !self.need(TokenKind::l_paren, " in method definition") {
                self.lexer.get_source_mgr_mut().note_at(
                    start_loc,
                    None,
                    "start of method definition",
                    support::diag::Subsystem::Parser,
                );
                return None;
            }

            let mut args: Vec<&'gc Node<'gc>> = Vec::new();
            if !self.parse_formal_parameters(Param::default(), &mut args) {
                return None;
            }

            // `: ReturnType`. C++ 3206-3215.
            let mut return_type: Option<&'gc Node<'gc>> = None;
            if self.parse_types() && self.check(TokenKind::colon) {
                let annot_start = self.advance(GrammarContext::Type).start;
                return_type = Some(self.parse_return_type_annotation(
                    Some(annot_start),
                    AllowAnonFunctionType::Yes,
                )?);
            }

            if !self.need(TokenKind::l_brace, " in method definition") {
                self.lexer.get_source_mgr_mut().note_at(
                    start_loc,
                    None,
                    "start of method definition",
                    support::diag::Subsystem::Parser,
                );
                return None;
            }
            let body = self.parse_function_body(
                PARAM_RETURN,
                eagerly,
                generator,
                async_,
                GrammarContext::AllowRegExp,
                true,
            )?;
            let body_end = body.range().end;

            let params = NodeList::from_iter(self.gc, args);
            let func = FunctionExpression::new(
                NodeMetadata::new(self.dummy_range()),
                None,
                params,
                body,
                type_params,
                return_type,
                None,
                generator,
                async_,
            );
            func.is_method_definition.set(true);
            value = self.set_location(
                paren_loc,
                body_end,
                Node::FunctionExpression(func),
            );
        } else {
            // `: value` — standard property (C++ 3246-3259).
            if !self.eat(
                TokenKind::colon,
                GrammarContext::AllowRegExp,
                " in property initialization",
            ) {
                self.lexer.get_source_mgr_mut().note_at(
                    start_loc,
                    None,
                    "start of property initialization",
                    support::diag::Subsystem::Parser,
                );
                return None;
            }
            value = self.parse_assignment_expression(PARAM_IN, false, AllowTypedArrowFunction::Yes, CoverTypedParameters::Yes, None)?;
        }

        let end_loc = self.lexer.prev_token_end();
        let init_kind = self.gc.ctx().atom_table.atom_bytes(b"init");
        let prop = Node::Property(Property::new(
            NodeMetadata::new(self.dummy_range()),
            key,
            value,
            init_kind,
            computed,
            method,
            shorthand,
        ));
        Some(self.set_location(start_loc, end_loc, prop))
    }

    // -----------------------------------------------------------------------
    // parse_member_select — P1.6
    // -----------------------------------------------------------------------

    /// Parse one member-select suffix: `[expr]`, `.id`, `?.id`, or `?.(args)`.
    /// Port of `JSParserImpl::parseMemberSelect` (3649-3793).
    ///
    /// `start_loc` is the start of the whole expression chain (not just this
    /// suffix), matching C++ `setLocation(startLoc, …)`.
    ///
    /// `object_loc` is used only in the error-message note ("start of member
    /// expression") — C++ passes it to `need(…, objectLoc)`.
    ///
    /// `seen_optional_chain` is the outer flag; `optional` is whether THIS
    /// particular suffix started with `?.`.
    ///
    /// Flow `?.<T>()` type-arguments on an optional call are handled (P6.0):
    /// a `<` immediately after `?.` is unambiguously Flow type syntax. The TS
    /// sibling block (`?.m<T>()`) is handled likewise (P7.5b).
    fn parse_member_select(
        &mut self,
        start_loc: support::location::SMLoc,
        object_loc: support::location::SMLoc,
        expr: &'gc Node<'gc>,
        seen_optional_chain: bool,
    ) -> Option<&'gc Node<'gc>> {
        let punc_loc = self.lexer.token().start_loc();
        // Consume `?.` if present.
        let optional =
            self.check_and_eat(TokenKind::questiondot, GrammarContext::AllowRegExp);

        if self.check_and_eat(TokenKind::l_square, GrammarContext::AllowRegExp) {
            // MemberExpression [ Expression ] — computed member access.
            // Parsing an Expression directly without going through
            // PrimaryExpression; can overflow, so check.
            let _guard = self.check_recursion()?;
            let prop_expr = self.parse_expression(PARAM_IN, CoverTypedParameters::Yes)?;
            let end_loc = self.lexer.token().end_loc();
            if !self.eat(
                TokenKind::r_square,
                GrammarContext::AllowDiv,
                "at end of member expression '[...'",
            ) {
                self.lexer.get_source_mgr_mut().note_at(
                    punc_loc,
                    None,
                    "location of '['",
                    support::diag::Subsystem::Parser,
                );
                return None;
            }
            if optional || seen_optional_chain {
                let node = Node::OptionalMemberExpression(OptionalMemberExpression::new(
                    NodeMetadata::new(self.dummy_range()),
                    expr,
                    prop_expr,
                    true,
                    optional,
                ));
                return Some(self.set_location_d(start_loc, end_loc, punc_loc, node));
            }
            let node = Node::MemberExpression(MemberExpression::new(
                NodeMetadata::new(self.dummy_range()),
                expr,
                prop_expr,
                true,
            ));
            return Some(self.set_location_d(start_loc, end_loc, punc_loc, node));
        }

        // `.id` or `?.id` path (also handles `?.` without `(` or `<`).
        //
        // The C++ condition is:
        //   checkAndEat(period) ||
        //   (optional && !(check(l_paren) || (getParseFlow() && check(less))))
        // i.e. a bare `?.` that is NOT followed by `(` and NOT followed by a
        // Flow `<…>` type-argument list is the `?.id` form.
        let ate_period =
            self.check_and_eat(TokenKind::period, GrammarContext::AllowDiv);
        let questiondot_typeargs =
            self.parse_flow() && self.check(TokenKind::less);
        if ate_period
            || (optional
                && !(self.check(TokenKind::l_paren) || questiondot_typeargs))
        {
            // The next token must be an identifier, a private identifier, or a
            // reserved word used as a member name (e.g. `a.if`).
            if !self.check2(TokenKind::identifier, TokenKind::private_identifier)
                && !self.lexer.token().is_res_word()
            {
                if !self.need(
                    TokenKind::identifier,
                    "after '.' or '?.' in member expression",
                ) {
                    self.lexer.get_source_mgr_mut().note_at(
                        object_loc,
                        None,
                        "start of member expression",
                        support::diag::Subsystem::Parser,
                    );
                    return None;
                }
            }

            let id: &'gc Node<'gc>;
            if self.check(TokenKind::private_identifier) {
                // Private name: `a.#x`
                id = self.parse_private_name()?;
            } else {
                // Plain identifier OR reserved word used as property name.
                let name = self.lexer.token().get_res_word_or_identifier();
                let tok_start = self.lexer.token().start_loc();
                let tok_end = self.lexer.token().end_loc();
                let node = Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    name,
                    None,
                    false,
                ));
                let node_ref = self.set_location(tok_start, tok_end, node);
                self.advance(GrammarContext::AllowDiv);
                id = node_ref;
            }

            let id_end = id.range().end;
            if optional || seen_optional_chain {
                let node = Node::OptionalMemberExpression(OptionalMemberExpression::new(
                    NodeMetadata::new(self.dummy_range()),
                    expr,
                    id,
                    false,
                    optional,
                ));
                return Some(self.set_location_d(start_loc, id_end, punc_loc, node));
            }
            let node = Node::MemberExpression(MemberExpression::new(
                NodeMetadata::new(self.dummy_range()),
                expr,
                id,
                false,
            ));
            return Some(self.set_location_d(start_loc, id_end, punc_loc, node));
        }

        // The only remaining case is `?.(args)` or `?.<T>(args)` — an optional
        // call on `?.`. C++ assert: `optional && (check(l_paren) ||
        // (getParseFlow() && check(less)))`.
        debug_assert!(
            optional
                && (self.check(TokenKind::l_paren)
                    || (self.parse_flow() && self.check(TokenKind::less)))
        );

        // Flow type-arguments on an optional call (C++ 3744-3760). NO SavePoint
        // here: a `<` immediately after `?.` is unambiguously Flow type syntax,
        // so we commit and require the `(`.
        let mut type_args: Option<&'gc Node<'gc>> = None;
        if self.parse_flow() && self.check(TokenKind::less) {
            type_args = Some(self.parse_type_args_flow(GrammarContext::Type)?);
            if !self.need(
                TokenKind::l_paren,
                "after type arguments in optional call",
            ) {
                self.lexer.get_source_mgr_mut().note_at(
                    object_loc,
                    None,
                    "start of optional call",
                    support::diag::Subsystem::Parser,
                );
                return None;
            }
        }
        // TS type-arguments on an optional call (C++ 3761-3777): a TS-only
        // sibling `#if HERMES_PARSE_TS` block, likewise unambiguous after `?.`.
        if self.parse_ts() && self.check(TokenKind::less) {
            type_args = Some(self.parse_ts_type_arguments()?);
            if !self.need(
                TokenKind::l_paren,
                "after type arguments in optional call",
            ) {
                self.lexer.get_source_mgr_mut().note_at(
                    object_loc,
                    None,
                    "start of optional call",
                    support::diag::Subsystem::Parser,
                );
                return None;
            }
        }

        let debug_loc = self.lexer.token().start_loc();
        let (arg_list, end_loc) = self.parse_arguments()?;
        let node = Node::OptionalCallExpression(OptionalCallExpression::new(
            NodeMetadata::new(self.dummy_range()),
            expr,
            type_args,
            NodeList::from_iter(self.gc, arg_list),
            true,
        ));
        Some(self.set_location_d(start_loc, end_loc, debug_loc, node))
    }

    /// Speculatively parse Flow type-arguments `<…>` at the current `<`,
    /// suppressing any parser diagnostics produced during the attempt. Common
    /// helper for the ambiguous call/new/LHS-tail sites
    /// (JSParserImpl.cpp:3810-3827, 3958-3974, 4044-4061): each of those takes a
    /// `SavePoint`, opens a `SaveAndSuppressMessages`, parses type-args, then
    /// keeps or rolls back based on a per-site commit condition.
    ///
    /// Returns `(parsed_type_args, save_point)`. The caller decides whether the
    /// commit-condition holds: if not, it must call `sp.restore(&mut self.lexer)`
    /// and drop the type-args. Diagnostic suppression is always restored here.
    fn speculative_type_args(
        &mut self,
    ) -> (Option<&'gc Node<'gc>>, crate::lexer::SavePoint) {
        debug_assert!(self.check(TokenKind::less));
        let sp = self.lexer.save_point();
        // C++ SourceErrorManager::SaveAndSuppressMessages{&sm_, Subsystem::Parser}:
        // pure-suppress parser messages during the speculative parse (lexer
        // messages still flow). Mirror the lexer-lookahead idiom (save/set/restore).
        let saved_suppressed = self.lexer.get_source_mgr().suppressed_messages();
        self.lexer
            .get_source_mgr_mut()
            .set_suppressed_messages(Some(support::diag::Subsystem::Parser));
        let type_args = self.parse_type_arguments();
        self.lexer
            .get_source_mgr_mut()
            .set_suppressed_messages(saved_suppressed);
        (type_args, sp)
    }

    // -----------------------------------------------------------------------
    // parse_call_expression — P1.6
    // -----------------------------------------------------------------------

    /// Parse a call expression chain starting after the base expression has
    /// already been parsed. Port of `JSParserImpl::parseCallExpression`
    /// (3795-3893).
    ///
    /// On entry the current token is `(` (or a template literal head, which
    /// is P1.9).  Each iteration of the loop handles one suffix:
    ///
    /// - `(args)` → `CallExpression` or `OptionalCallExpression` (if
    ///   `seen_optional_chain`).
    /// - `[expr]` / `.id` / `?.id` / `?.(args)` → `parseMemberSelect`.
    /// - Template literal → P1.9 deferral error.
    ///
    /// `type_args` carries Flow/TS type arguments from the caller. After each
    /// `(args)` call the type-args are consumed (reset to `None`) so the next
    /// call in the chain can speculatively supply its own (`f<T>()<U>()`).
    ///
    /// Flow type-argument speculation (P6.0) runs at the top of the loop; the
    /// TS arm is OR'd into the same gate (P7.5b).
    fn parse_call_expression(
        &mut self,
        start_loc: support::location::SMLoc,
        mut expr: &'gc Node<'gc>,
        mut type_args: Option<&'gc Node<'gc>>,
        mut seen_optional_chain: bool,
        mut optional: bool,
    ) -> Option<&'gc Node<'gc>> {
        let mut object_loc = start_loc;

        loop {
            // Flow/TS type-argument block (C++ 3809-3828). Each call in a chain
            // may carry type arguments; attempt to parse them at a `<`, rolling
            // back if it was just a comparison operator. The C++ gate is
            // `(getParseFlowAmbiguous() || getParseTS())`.
            if (self.parse_flow_ambiguous() || self.parse_ts())
                && type_args.is_none()
                && self.check(TokenKind::less)
            {
                let (opt_type_args, sp) = self.speculative_type_args();
                if opt_type_args.is_some() && self.check(TokenKind::l_paren) {
                    // Call expression with type arguments.
                    type_args = opt_type_args;
                } else {
                    // Not a call with type-args; roll back.
                    sp.restore(&mut self.lexer);
                }
            }

            if self.check(TokenKind::l_paren) {
                let debug_loc = self.lexer.token().start_loc();
                // parseArguments can itself recurse into parseCallExpression
                // without going through a primary or declaration → CHECK_RECURSION.
                let _guard = self.check_recursion()?;
                let (arg_list, end_loc) = self.parse_arguments()?;

                if seen_optional_chain {
                    let node = Node::OptionalCallExpression(OptionalCallExpression::new(
                        NodeMetadata::new(self.dummy_range()),
                        expr,
                        type_args,
                        NodeList::from_iter(self.gc, arg_list),
                        optional,
                    ));
                    expr = self.set_location_d(start_loc, end_loc, debug_loc, node);
                } else {
                    let node = Node::CallExpression(CallExpression::new(
                        NodeMetadata::new(self.dummy_range()),
                        expr,
                        type_args,
                        NodeList::from_iter(self.gc, arg_list),
                    ));
                    expr = self.set_location_d(start_loc, end_loc, debug_loc, node);
                }
                // Consume the type-args (they have been used).
                type_args = None;
                // After a call, `optional` must NOT propagate (only the
                // initial `?.` is `optional`; subsequent calls in the chain
                // are not individually optional unless preceded by `?.`).
                optional = false;
            } else if self.check_n3(
                TokenKind::l_square,
                TokenKind::period,
                TokenKind::questiondot,
            ) {
                if self.check(TokenKind::questiondot) {
                    seen_optional_chain = true;
                }
                let next_object_loc = self.lexer.token().start_loc();
                expr = self.parse_member_select(
                    start_loc,
                    object_loc,
                    expr,
                    seen_optional_chain,
                )?;
                object_loc = next_object_loc;
                // A `?.(args)` inside parseMemberSelect will have consumed the
                // `?.` and the args; `optional` resets to false for the next round.
                optional = false;
            } else if self.check2(
                TokenKind::no_substitution_template,
                TokenKind::template_head,
            ) {
                // Tagged template literal — P1.9.
                // C++ 3874-3886: debugLoc = template start; setLocation 4-arg.
                let debug_loc = self.lexer.token().start_loc();
                let quasi = self.parse_template_literal(PARAM_TAGGED)?;
                let quasi_end = quasi.range().end;
                let tagged = Node::TaggedTemplateExpression(TaggedTemplateExpression::new(
                    NodeMetadata::new(self.dummy_range()),
                    expr,
                    quasi,
                ));
                expr = self.set_location_d(start_loc, quasi_end, debug_loc, tagged);
            } else {
                break;
            }
        }

        Some(expr)
    }

    // -----------------------------------------------------------------------
    // parse_private_name — P1.6
    // -----------------------------------------------------------------------

    /// Parse a `#identifier` private name. Port of
    /// `JSParserImpl::parsePrivateName` (1182-1195).
    ///
    /// Precondition: current token is `private_identifier`.
    /// Returns a `PrivateName` node wrapping an `Identifier` whose name is
    /// the identifier part (without `#`).
    ///
    /// The C++ additionally errors if the private name is `#constructor`
    /// (`privateIdent == constructorIdent_`). We port that check.
    pub(super) fn parse_private_name(&mut self) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check(TokenKind::private_identifier));
        let private_ident_name = self.lexer.token().get_private_identifier();
        let tok_start = self.lexer.token().start_loc();
        let tok_end = self.lexer.token().end_loc();

        // Build the inner Identifier node with the private identifier's name
        // (the part after `#`).
        let ident_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            private_ident_name,
            None,
            false,
        ));
        let ident_ref = self.set_location(tok_start, tok_end, ident_node);

        // Error if the private name is `#constructor`.
        let constructor_bytes = b"constructor";
        let name_bytes = self.lexer.get_string_table().bytes(private_ident_name);
        if name_bytes == constructor_bytes {
            let ident_range = ident_ref.range();
            self.error_at(ident_range, "Private names cannot be '#constructor'");
        }

        // Consume the private_identifier token.  `advance()` returns the old
        // token's range; `advance().Start` == tok_start (same token).
        self.advance(GrammarContext::AllowDiv);

        // PrivateName node with the same source range as the private_identifier.
        let priv_node = Node::PrivateName(PrivateName::new(
            NodeMetadata::new(self.dummy_range()),
            ident_ref,
        ));
        Some(self.set_location(tok_start, tok_end, priv_node))
    }

    // -----------------------------------------------------------------------
    // parsePrimaryExpression — 2481 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a primary expression. Port of
    /// `JSParserImpl::parsePrimaryExpression` (lines 2481-2709).
    ///
    /// Implemented:
    ///   rw_this, identifier, rw_null, rw_true/false, numeric_literal,
    ///   bigint_literal, string_literal, l_paren (plain grouping, no arrow cover),
    ///   regexp_literal (P1.10), l_square (P1.7), l_brace (P1.8).
    ///
    /// Deferred with honest error messages:
    ///   no_substitution_template / template_head / rw_function / at /
    ///   rw_class / less (JSX) / default.
    pub(super) fn parse_primary_expression(&mut self) -> Option<&'gc Node<'gc>> {
        let _guard = self.check_recursion()?;

        match self.cur_kind() {
            // this
            TokenKind::rw_this => {
                let tok_start = self.lexer.token().start_loc();
                let tok_end = self.lexer.token().end_loc();
                let node = Node::ThisExpression(ThisExpression::new(
                    NodeMetadata::new(self.dummy_range()),
                ));
                let res = self.set_location(tok_start, tok_end, node);
                self.advance(GrammarContext::AllowDiv);
                Some(res)
            }

            // identifier
            TokenKind::identifier => {
                // yield is only allowed as an IdentifierReference when
                // ParamYield is false. C++ lines 2493-2501.
                // Capture the boolean first so the atom-table borrow ends
                // before the `&mut self` error_cur call.
                let is_yield = self
                    .lexer
                    .get_string_table()
                    .bytes(self.lexer.token().get_identifier())
                    == b"yield";
                if self.param_yield.get() && is_yield {
                    self.error_cur(
                        "Unexpected usage of 'yield' as an identifier reference",
                    );
                }
                // async function expression. C++ lines 2502-2507.
                if self.check_unescaped_name(b"async")
                    && self.check_async_function()
                {
                    return self.parse_function_expression(false);
                }

                // `arguments` tracking inside arrow functions — C++ line 2508.
                // If we are inside an arrow function and the identifier is
                // `arguments`, the enclosing non-arrow function may need to
                // capture its `arguments` object. Port of
                // JSParserImpl.cpp:2508-2511.
                if self.is_arrow_function.get() {
                    let name_bytes = self.gc.ctx().atom_table.bytes(
                        self.lexer.token().get_identifier(),
                    );
                    if name_bytes == b"arguments" {
                        self.may_contain_arrow_functions_using_arguments
                            .set(true);
                    }
                }

                // Flow match expression. C++ JSParserImpl.cpp:2513-2518.
                if self.parse_flow()
                    && self.parse_flow_match()
                    && self.check_maybe_flow_match()
                {
                    return self.parse_match_call_or_match_expression_flow();
                }

                let name = self.lexer.token().get_identifier();
                let tok_start = self.lexer.token().start_loc();
                let tok_end = self.lexer.token().end_loc();
                let node = Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    name,
                    None,  // typeAnnotation
                    false, // optional
                ));
                let res = self.set_location(tok_start, tok_end, node);
                self.advance(GrammarContext::AllowDiv);
                Some(res)
            }

            // null
            TokenKind::rw_null => {
                let tok_start = self.lexer.token().start_loc();
                let tok_end = self.lexer.token().end_loc();
                let node = Node::NullLiteral(NullLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                ));
                let res = self.set_location(tok_start, tok_end, node);
                self.advance(GrammarContext::AllowDiv);
                Some(res)
            }

            // true / false
            TokenKind::rw_true | TokenKind::rw_false => {
                let value = self.cur_kind() == TokenKind::rw_true;
                let tok_start = self.lexer.token().start_loc();
                let tok_end = self.lexer.token().end_loc();
                let node = Node::BooleanLiteral(BooleanLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    value,
                ));
                let res = self.set_location(tok_start, tok_end, node);
                self.advance(GrammarContext::AllowDiv);
                Some(res)
            }

            // numeric literal
            TokenKind::numeric_literal => {
                let value = self.lexer.token().get_numeric_literal();
                let tok_start = self.lexer.token().start_loc();
                let tok_end = self.lexer.token().end_loc();
                let node = Node::NumericLiteral(NumericLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    value,
                ));
                let res = self.set_location(tok_start, tok_end, node);
                self.advance(GrammarContext::AllowDiv);
                Some(res)
            }

            // bigint literal
            TokenKind::bigint_literal => {
                let bigint = self.lexer.token().get_bigint_literal();
                let tok_start = self.lexer.token().start_loc();
                let tok_end = self.lexer.token().end_loc();
                let node = Node::BigIntLiteral(BigIntLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    bigint,
                ));
                let res = self.set_location(tok_start, tok_end, node);
                self.advance(GrammarContext::AllowDiv);
                Some(res)
            }

            // string literal
            TokenKind::string_literal => {
                let value = self.lexer.token().get_string_literal();
                let tok_start = self.lexer.token().start_loc();
                let tok_end = self.lexer.token().end_loc();
                let node = Node::StringLiteral(StringLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    value,
                ));
                let res = self.set_location(tok_start, tok_end, node);
                self.advance(GrammarContext::AllowDiv);
                Some(res)
            }

            // regexp literal — P1.10. Port of JSParserImpl.cpp 2573-2582.
            TokenKind::regexp_literal => {
                let re = self.lexer.token().get_regexp_literal();
                let tok_start = self.lexer.token().start_loc();
                let tok_end = self.lexer.token().end_loc();
                let node = Node::RegExpLiteral(RegExpLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    re.body(),
                    re.flags(),
                ));
                let res = self.set_location(tok_start, tok_end, node);
                self.advance(GrammarContext::AllowDiv);
                Some(res)
            }

            // array literal — P1.7
            TokenKind::l_square => self.parse_array_literal(),

            // object literal — P1.8
            TokenKind::l_brace => self.parse_object_literal(),

            // parenthesized expression / arrow-function cover — C++ 2598-2665
            TokenKind::l_paren => {
                let start_loc = self.advance(GrammarContext::AllowRegExp).start;

                // Cover "()". C++ lines 2602-2606.
                if self.check(TokenKind::r_paren) {
                    let end_loc = self.advance(GrammarContext::AllowDiv).end;
                    let node = Node::CoverEmptyArgs(CoverEmptyArgs::new(
                        NodeMetadata::new(self.dummy_range()),
                    ));
                    return Some(self.set_location(start_loc, end_loc, node));
                }

                // Cover "(...rest)". C++ lines 2608-2623.
                let expr = if self.check(TokenKind::dotdotdot) {
                    let rest = self.parse_binding_rest_element(PARAM_IN)?;
                    let rest_range = rest.range();
                    let node = Node::CoverRestElement(CoverRestElement::new(
                        NodeMetadata::new(self.dummy_range()),
                        rest,
                    ));
                    self.set_location(rest_range.start, rest_range.end, node)
                } else {
                    // Plain grouped expression: parse expr.
                    // C++ passes CoverTypedParameters::Yes (Flow/TS-only).
                    self.parse_expression(PARAM_IN, CoverTypedParameters::Yes)?
                };

                // Flow type-cast annotation (C++ 2625-2653).
                let mut expr = expr;
                if self.parse_flow() {
                    // The location encompasses the `()` by using `start_loc` and
                    // the current token (`)`) as start/end. If `tok_` is not
                    // `)`, the `eat` below errors immediately.
                    let cast_end = self.cur_range().end;
                    if let Node::CoverTypedIdentifier(cover) = expr {
                        if let Some(right) = cover.right {
                            if !cover.optional.get() {
                                let node = Node::TypeCastExpression(
                                    TypeCastExpression::new(
                                        NodeMetadata::new(self.dummy_range()),
                                        cover.left,
                                        right,
                                    ),
                                );
                                expr = self
                                    .set_location(start_loc, cast_end, node);
                            }
                        }
                    } else if self.check(TokenKind::colon) {
                        let annot_start =
                            self.advance(GrammarContext::Type).start;
                        let ty = self.parse_type_annotation_flow(
                            Some(annot_start),
                            AllowAnonFunctionType::Yes,
                        )?;
                        // Re-read the end after parsing (now at `)`); C++ uses
                        // `tok_` which is the post-annotation token.
                        let cast_end2 = self.cur_range().end;
                        let node = Node::TypeCastExpression(
                            TypeCastExpression::new(
                                NodeMetadata::new(self.dummy_range()),
                                expr,
                                ty,
                            ),
                        );
                        expr = self.set_location(start_loc, cast_end2, node);
                    }
                }

                // C++ 2655-2660: eat(r_paren, AllowDiv, "at end of
                // parenthesized expression", "started here", startLoc).
                // `startLoc` is the '(' — real, so on a one-line
                // `var a = (1 + 2;` the diagnostic underlines the whole
                // `(1 + 2;` span, and on a multi-line one it gets a
                // "started here" note at the '('.
                if !self.eat_at(
                    TokenKind::r_paren,
                    GrammarContext::AllowDiv,
                    " at end of parenthesized expression",
                    Some("started here"),
                    start_loc,
                ) {
                    return None;
                }
                // Record the parentheses surrounding the expression.
                // NOTE: C++ returns the SAME inner node (just with parens
                // incremented), it does NOT wrap in a new node.
                // The outer ExpressionStatement will see startLoc = start of '('
                // and set the statement range accordingly.
                inc_parens(expr);
                Some(expr)
            }

            // template literal — P1.9
            TokenKind::no_substitution_template | TokenKind::template_head => {
                self.parse_template_literal(Param::default())
            }

            // function expression. C++ 2667-2670.
            TokenKind::rw_function => self.parse_function_expression(false),

            // decorator / class expression. C++ 2671-2674.
            TokenKind::at | TokenKind::rw_class => self.parse_class_expression(),

            // JSX — context-gated (getParseJSX()). C++ lines 2691-2703.
            TokenKind::less => {
                if self.parse_jsx() {
                    return self.parse_jsx_root();
                }
                // C++ reports at `tok_->getStartLoc()`, i.e. through the
                // `error(SMLoc, Twine)` overload (JSParserImpl.h:472-474),
                // NOT `error(Twine)` — so the diagnostic is a bare caret at
                // the token start with no underlined range
                // (JSParserImpl.cpp:2699-2702).
                let loc = self.cur_range().start;
                self.error_at_loc(
                    loc,
                    "invalid expression (possible JSX: pass -parse-jsx to parse)",
                );
                None
            }

            // default
            _ => {
                // `error(tok_->getStartLoc(), "invalid expression")`
                // (JSParserImpl.cpp:2706) — a point location, see above.
                let loc = self.cur_range().start;
                self.error_at_loc(loc, "invalid expression");
                None
            }
        }
    }

    // -----------------------------------------------------------------------
    // parse_template_literal — P1.9
    // -----------------------------------------------------------------------

    /// Parse a template literal (tagged or untagged). Port of
    /// `JSParserImpl::parseTemplateLiteral` (lines 3342-3414).
    ///
    /// Precondition: current token is `no_substitution_template` or
    /// `template_head`.
    ///
    /// `param` carries `PARAM_TAGGED` for tagged template literals. When the
    /// token contains a `NotEscapeSequence` (invalid escape) and `PARAM_TAGGED`
    /// is NOT set, an error is emitted and the parse fails. When `PARAM_TAGGED`
    /// IS set, `cooked` is `None` (→ `INVALID_ATOM_BYTES` → JSON `null`).
    ///
    /// Returns a `TemplateLiteral(quasis, expressions)` node.
    pub(super) fn parse_template_literal(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        debug_assert!(
            self.check2(TokenKind::no_substitution_template, TokenKind::template_head),
            "parse_template_literal: expected template literal start"
        );

        let start_loc = self.cur_start();

        let mut quasis: Vec<&'gc Node<'gc>> = Vec::new();
        let mut expressions: Vec<&'gc Node<'gc>> = Vec::new();

        // Push the current TemplateElement token onto `quasis` and advance.
        // `tail` indicates whether this is the last quasi.
        // Returns false on error (invalid escape in untagged template).
        let mut push_template_element = |this: &mut Self, tail: bool| -> bool {
            // Invalid escape check (only an error in untagged context).
            if this.lexer.token().get_template_literal_contains_not_escapes()
                && !param.has(PARAM_TAGGED)
            {
                let range = this.cur_range();
                this.error_at(
                    range,
                    "untagged template literal contains invalid escape sequence",
                );
                return false;
            }
            // Build cooked: None → INVALID_ATOM_BYTES (dumps as JSON null).
            let cooked = match this.lexer.token().get_template_value() {
                Some(ab) => ab,
                None => INVALID_ATOM_BYTES,
            };
            let raw = this.lexer.token().get_template_raw_value();
            let tok_start = this.lexer.token().start_loc();
            let tok_end = this.lexer.token().end_loc();
            let quasi_node = Node::TemplateElement(TemplateElement::new(
                NodeMetadata::new(this.dummy_range()),
                tail,
                cooked,
                raw,
            ));
            let quasi_ref = this.set_location(tok_start, tok_end, quasi_node);
            quasis.push(quasi_ref);
            true
        };

        // TemplateSpans: loop while not at end of template.
        // C++ loops while NOT (no_substitution_template | template_tail).
        while !self.check2(
            TokenKind::no_substitution_template,
            TokenKind::template_tail,
        ) {
            // Must be template_head or template_middle.
            if !self.check2(TokenKind::template_head, TokenKind::template_middle) {
                let range = self.cur_range();
                self.error_at(range, "expected template literal");
                return None;
            }

            // Push the non-tail TemplateElement.
            if !push_template_element(self, false) {
                return None;
            }
            // Consume the template_head/template_middle token.
            // C++ `subStart = advance().Start` (subStart is only used for the
            // error note; we capture it but don't need it for a fatal-less path).
            let sub_start = self.advance(GrammarContext::AllowRegExp).start;

            // Parse the substitution expression.
            let opt_expr = self.parse_expression(PARAM_IN, CoverTypedParameters::Yes);
            let opt_expr = match opt_expr {
                Some(e) => e,
                None => return None,
            };
            expressions.push(opt_expr);

            // The } terminating the expression must be present.
            if !self.check(TokenKind::r_brace) {
                if !self.need(TokenKind::r_brace, " at end of substitution in template literal") {
                    self.lexer.get_source_mgr_mut().note_at(
                        sub_start,
                        None,
                        "start of substitution",
                        support::diag::Subsystem::Parser,
                    );
                }
                return None;
            }

            // Rescan the `}` as template_middle or template_tail.
            self.lexer.rescan_rbrace_in_template_literal();
        }

        // Push the tail TemplateElement (no_substitution_template or template_tail).
        if !push_template_element(self, true) {
            return None;
        }

        // Consume the tail token; C++ `advance().End` gives the end loc.
        let end_loc = self.advance(GrammarContext::AllowDiv).end;

        let quasis_list = NodeList::from_iter(self.gc, quasis);
        let expr_list = NodeList::from_iter(self.gc, expressions);
        let node = Node::TemplateLiteral(TemplateLiteral::new(
            NodeMetadata::new(self.dummy_range()),
            quasis_list,
            expr_list,
        ));
        Some(self.set_location(start_loc, end_loc, node))
    }

}

// ---------------------------------------------------------------------------
// incParens helper
// ---------------------------------------------------------------------------

/// Increment the paren count on a node, capping at 2. Port of
/// `ESTree.h Node::incParens()`.
pub(super) fn inc_parens(n: &Node) {
    let md = n.metadata();
    let p = md.parens.get();
    md.parens.set((p + 1).min(2));
}
