/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Expression parsing for the JS parser. Port of the expression-parsing
//! section of `lib/Parser/JSParserImpl.cpp`.

use ast::node::{
    BigIntLiteral, BooleanLiteral, Identifier, Node, NullLiteral, NumericLiteral,
    SequenceExpression, StringLiteral, ThisExpression,
};
use ast::node_child::{NodeList, NodeMetadata};

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
    // parseBinaryExpression — P1.2 placeholder
    // -----------------------------------------------------------------------

    /// Parse a binary expression (operator precedence climbing). Port of
    /// `JSParserImpl::parseBinaryExpression` (lines 4262-…).
    ///
    /// P1.2: precedence-table binary operators — deferred.
    /// For now this is a pass-through to parseUnaryExpression.
    pub(super) fn parse_binary_expression(
        &mut self,
        _param: Param,
    ) -> Option<&'gc Node<'gc>> {
        // P1.2: binary operators (precedence table)
        self.parse_unary_expression()
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
                let ident_bytes = self
                    .lexer
                    .get_string_table()
                    .bytes(self.lexer.token().get_identifier())
                    .to_vec();
                if self.param_yield && ident_bytes == b"yield" {
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
                // incParens caps at 2 (mirrors ESTree.h Node::incParens).
                // NOTE: C++ returns the SAME inner node (just with parens
                // incremented), it does NOT wrap in a new node.
                // The outer ExpressionStatement will see startLoc = start of '('
                // and set the statement range accordingly.
                let md = expr.metadata();
                let p = md.parens.get();
                md.parens.set((p + 1).min(2));
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
// incParens helper (free function so statements.rs can call it too)
// ---------------------------------------------------------------------------

/// Increment the paren count on a node, capping at 2. Port of
/// `ESTree.h Node::incParens()`.
#[allow(dead_code)]
pub(super) fn inc_parens(n: &Node) {
    let md = n.metadata();
    let p = md.parens.get();
    md.parens.set((p + 1).min(2));
}
