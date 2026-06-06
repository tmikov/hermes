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
    BigIntLiteral, BinaryExpression, BooleanLiteral, Identifier, LogicalExpression, Node,
    NullLiteral, NumericLiteral, PrivateName, SequenceExpression, StringLiteral, ThisExpression,
};
use ast::node_child::{NodeList, NodeMetadata};
use support::location::SMLoc;

use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

use super::{JSParserImpl, Param, PARAM_IN};

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    // -----------------------------------------------------------------------
    // parseExpression — 6552 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a comma-separated sequence of assignment expressions, building
    /// a SequenceExpression for 2+ operands. Port of
    /// `JSParserImpl::parseExpression` (lines 6552-6609).
    ///
    /// P1.1: the `dotdotdot` / `CoverTrailingComma` branches are deferred
    /// (they are reached only inside arrow-function covers, which are P3).
    /// We just parse the plain comma-sequence of assignment expressions.
    pub(super) fn parse_expression(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        let start_loc = self.cur_start();
        let opt_expr = self.parse_assignment_expression(param)?;

        if !self.check(TokenKind::comma) {
            return Some(opt_expr);
        }

        // Build a SequenceExpression.
        let mut expr_nodes: Vec<&'gc Node<'gc>> = vec![opt_expr];

        while self.check(TokenKind::comma) {
            // Eat the ",".
            self.advance(GrammarContext::AllowRegExp);

            // CoverParenthesizedExpressionAndArrowParameterList: (Expression ,)
            // — the trailing-comma cover node is P3 (arrow functions). For P1.1
            // we stop here; the trailing comma before ')' will be handled when
            // parsePrimaryExpression's l_paren branch is filled in (P3).
            if self.check(TokenKind::r_paren) {
                // P3: CoverTrailingCommaNode deferred.
                // For now just stop; the ')' will be consumed by the caller.
                break;
            }

            // P3: dotdotdot (spread) cover form is deferred.
            let expr2 = self.parse_assignment_expression(param)?;
            expr_nodes.push(expr2);
        }

        // If only one expression was accumulated (the trailing-comma break above
        // happened immediately), return it directly without wrapping.
        if expr_nodes.len() == 1 {
            return Some(expr_nodes[0]);
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
    // parseAssignmentExpression — P1.5 / P3 placeholder
    // -----------------------------------------------------------------------

    /// Parse an assignment expression. Port of
    /// `JSParserImpl::parseAssignmentExpression` (lines 6233-6551).
    ///
    /// P1.5: assignment operators (=, +=, …) — deferred.
    /// P3: yield / arrow functions — deferred.
    /// For now this is a pass-through to parseConditionalExpression.
    pub(super) fn parse_assignment_expression(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        // P1.5: assignment ops; P3: arrow/yield
        self.parse_conditional_expression(param)
    }

    // -----------------------------------------------------------------------
    // parseConditionalExpression — P1.4 placeholder
    // -----------------------------------------------------------------------

    /// Parse a conditional expression. Port of
    /// `JSParserImpl::parseConditionalExpression` (lines 4477-…).
    ///
    /// P1.4: the `? :` branches — deferred.
    /// For now this is a pass-through to parseBinaryExpression.
    pub(super) fn parse_conditional_expression(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        // P1.4: conditional (ternary) operator
        self.parse_binary_expression(param)
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
    /// - `as_operator` (IDENT_OP, precedence 8) is only injected by
    ///   `convertIdentOpIfPossible`, which is a no-op in P1 (TS/Flow gated).
    ///   It is therefore unreachable here; we leave it unhandled.
    #[inline]
    fn get_precedence(kind: TokenKind) -> u32 {
        use crate::token_kinds::binop_precedence;
        match binop_precedence(kind) {
            Some(p) => p as u32,
            None => match kind {
                TokenKind::rw_in | TokenKind::rw_instanceof => 8,
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
    /// and the parser context has TS/Flow type-parsing enabled.
    ///
    /// P6/P7 stub: TS/Flow 'as' operator (needs Context parse-types flag +
    /// `lexer.convert_cur_token_to_ident_op`).  In P1 the body is compiled out
    /// (mirroring C++ `#if HERMES_PARSE_TS || HERMES_PARSE_FLOW`), so this is
    /// a pure no-op.
    #[inline]
    fn convert_ident_op_if_possible(&mut self) {
        // P6/P7: TS/Flow 'as' operator (needs Context parse-types flag +
        // lexer.convert_cur_token_to_ident_op).  No-op until then.
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
    /// - `as_operator` (TS/Flow): stubbed as unreachable in P1 (P6/P7).
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
                } else {
                    // P6/P7: as_operator branch (TS AsExpression / Flow
                    // AsExpression / AsConstExpression) — unreachable in P1
                    // because convert_ident_op_if_possible is a no-op.
                    debug_assert_ne!(
                        entry.op_kind,
                        TokenKind::as_operator,
                        "as_operator is unreachable in P1 (no parse-types context)"
                    );
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

            // Consume the operator token.
            // P6/P7: as_operator uses GrammarContext::Type and then parses
            // a type annotation instead of a unary expression — unreachable
            // in P1.
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
            } else {
                debug_assert_ne!(
                    entry.op_kind,
                    TokenKind::as_operator,
                    "as_operator is unreachable in P1 (no parse-types context)"
                );
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
    // parseUnaryExpression — P1.3 placeholder
    // -----------------------------------------------------------------------

    /// Parse a unary expression. Port of
    /// `JSParserImpl::parseUnaryExpression` (lines 4112-4211).
    ///
    /// P1.3: unary operators (delete, void, typeof, +, -, ~, !, ++, --,
    ///       await) — deferred.
    /// For now this is a pass-through to parsePostfixExpression.
    pub(super) fn parse_unary_expression(&mut self) -> Option<&'gc Node<'gc>> {
        // P1.3: prefix unary operators (delete/void/typeof/+/-/~/!/++/--)
        // and await (when param_await is set). Both are deferred; the field
        // reference below keeps rustc happy until P1.3 fills this in.
        let _ = self.param_await; // read in P1.3 (await expression)
        self.parse_postfix_expression()
    }

    // -----------------------------------------------------------------------
    // parsePostfixExpression — P1.3 placeholder
    // -----------------------------------------------------------------------

    /// Parse a postfix expression (++/-- suffix). Port of
    /// `JSParserImpl::parsePostfixExpression` (lines 4091-4110).
    ///
    /// P1.3: postfix ++/-- — deferred.
    /// For now this is a pass-through to parseLeftHandSideExpression.
    pub(super) fn parse_postfix_expression(&mut self) -> Option<&'gc Node<'gc>> {
        // P1.3: postfix ++ / --
        self.parse_left_hand_side_expression()
    }

    // -----------------------------------------------------------------------
    // parseLeftHandSideExpression / parseLeftHandSideExpressionTail — P1.6
    // -----------------------------------------------------------------------

    /// Parse a left-hand-side expression. Port of
    /// `JSParserImpl::parseLeftHandSideExpression` (lines 4014-4024).
    ///
    /// P1.6: member access / call / optional-chain tail — deferred.
    /// For now just delegates to parseNewExpressionOrOptionalExpression which
    /// in turn reaches parsePrimaryExpression.
    pub(super) fn parse_left_hand_side_expression(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // P1.6: member/call/optional tail (parseLeftHandSideExpressionTail)
        self.parse_new_expression_or_optional_expression()
    }

    // -----------------------------------------------------------------------
    // parseNewExpressionOrOptionalExpression — P1.6 placeholder
    // -----------------------------------------------------------------------

    /// Minimal stub reaching parsePrimaryExpression. Port of
    /// `JSParserImpl::parseNewExpressionOrOptionalExpression` (lines 3920-…).
    ///
    /// P1.6: `new`, optional chaining (`?.`), call expressions, member
    ///       select — all deferred.
    pub(super) fn parse_new_expression_or_optional_expression(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // P1.6: `new` expression, optional chaining
        // Fall through to parsePrimaryExpression for P1.1.
        self.parse_primary_expression()
    }

    // -----------------------------------------------------------------------
    // parsePrimaryExpression — 2481 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a primary expression. Port of
    /// `JSParserImpl::parsePrimaryExpression` (lines 2481-2709).
    ///
    /// Implemented for P1.1:
    ///   rw_this, identifier, rw_null, rw_true/false, numeric_literal,
    ///   bigint_literal, string_literal, l_paren (plain grouping, no arrow cover).
    ///
    /// Deferred with honest error messages:
    ///   l_square / l_brace / no_substitution_template / template_head
    ///   regexp_literal / rw_function / at / rw_class / less (JSX) / default.
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
                if self.param_yield && is_yield {
                    self.error_cur(
                        "Unexpected usage of 'yield' as an identifier reference",
                    );
                }
                // async function expression — deferred (P3). C++ lines 2502-2507.
                // (checkAsyncFunction involves a lookahead; skip for P1.1.)

                // `arguments` tracking inside arrow functions — C++ line 2508.
                // Deferred: isArrowFunction_ flag is P3.

                // Flow match expression — deferred (context_.getParseFlowMatch()).

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

            // regexp literal — deferred (P1.9)
            TokenKind::regexp_literal => {
                self.error_cur(
                    "regexp literals not yet supported (parser phase P1.9)",
                );
                None
            }

            // array literal — deferred (P1.7)
            TokenKind::l_square => {
                self.error_cur(
                    "array literals not yet supported (parser phase P1.7)",
                );
                None
            }

            // object literal — deferred (P1.8)
            TokenKind::l_brace => {
                self.error_cur(
                    "object literals not yet supported (parser phase P1.8)",
                );
                None
            }

            // parenthesized expression / arrow-function cover — C++ 2598-2665
            TokenKind::l_paren => {
                self.advance(GrammarContext::AllowRegExp);

                // Cover "()" — empty arrow params, P3. C++ lines 2602-2605.
                if self.check(TokenKind::r_paren) {
                    self.error_cur(
                        "arrow functions not yet supported (parser phase P3)",
                    );
                    return None;
                }

                // Cover "(...rest)" — arrow params, P3. C++ lines 2609-2622.
                if self.check(TokenKind::dotdotdot) {
                    self.error_cur(
                        "arrow functions not yet supported (parser phase P3)",
                    );
                    return None;
                }

                // Plain grouped expression: parse expr, eat ')'.
                let expr = self.parse_expression(PARAM_IN)?;

                // Flow type-cast annotation — deferred (context_.getParseFlow()).

                if !self.eat(
                    TokenKind::r_paren,
                    GrammarContext::AllowDiv,
                    " at end of parenthesized expression",
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

            // template literal — deferred (P1.10)
            TokenKind::no_substitution_template | TokenKind::template_head => {
                self.error_cur(
                    "template literals not yet supported (parser phase P1.10)",
                );
                None
            }

            // function expression — deferred (P3)
            TokenKind::rw_function => {
                self.error_cur(
                    "function expressions not yet supported (parser phase P3)",
                );
                None
            }

            // decorator / class expression — deferred (P3)
            TokenKind::at | TokenKind::rw_class => {
                self.error_cur(
                    "class expressions not yet supported (parser phase P3)",
                );
                None
            }

            // JSX — context-gated (getParseJSX()). For now emit the C++ error.
            // The JSX context flag is not yet wired; in P1 plain-JS corpus this
            // branch won't fire. C++ lines 2692-2703.
            TokenKind::less => {
                // C++ error: "invalid expression (possible JSX: pass -parse-jsx)"
                // We keep a simpler message since JSX context is not yet wired.
                self.error_cur(
                    "invalid expression (possible JSX: pass -parse-jsx to parse)",
                );
                None
            }

            // default
            _ => {
                self.error_cur("invalid expression");
                None
            }
        }
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
