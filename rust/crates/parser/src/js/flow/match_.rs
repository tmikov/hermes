/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Flow `match` expressions and statements (the Flow pattern-matching
//! proposal). Port of the `match` productions of
//! `lib/Parser/JSParserImpl-flow.cpp` (lines 860-1616). Gated on the dedicated
//! `parse_flow_match` context flag.
//!
//! The high-risk piece is the call-vs-match disambiguation: `match` is NOT a
//! reserved word, so `match(x)` could be a plain call or the head of a match
//! construct. The statement form rolls back with a `SavePoint`; the expression
//! form reinterprets an already-parsed argument list (no `SavePoint`).

use hermes_ast::node::{
    BigIntLiteral, BooleanLiteral, CallExpression, Identifier,
    MatchArrayPattern, MatchAsPattern, MatchBindingPattern, MatchExpression,
    MatchExpressionCase, MatchIdentifierPattern, MatchInstanceObjectPattern,
    MatchInstancePattern, MatchLiteralPattern, MatchMemberPattern,
    MatchObjectPattern, MatchObjectPatternProperty, MatchOrPattern,
    MatchRestPattern, MatchStatement, MatchStatementCase, MatchUnaryPattern,
    MatchWildcardPattern, Node, NullLiteral, NumericLiteral, SequenceExpression,
    StringLiteral,
};
use hermes_ast::node_child::{NodeList, NodeMetadata};
use hermes_support::location::{SMLoc, SMRange};

use crate::js::{
    IsClassHeritageArgument, IsConstructorCall, JSParserImpl, Param, PARAM_IN,
    PARAM_RETURN,
};
use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

use super::{AllowTypedArrowFunction, CoverTypedParameters};

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    // -----------------------------------------------------------------------
    // checkMaybeFlowMatch / checkMaybeFlowMatchSlowPath
    //   JSParserImpl.h:1327-1331 + JSParserImpl-flow.cpp:860-864
    // -----------------------------------------------------------------------

    /// Whether we are *maybe* at the start of a Flow match expression or
    /// statement: `match` [no LineTerminator here] `(`. Port of
    /// `JSParserImpl::checkMaybeFlowMatch` (JSParserImpl.h:1327-1331) +
    /// `checkMaybeFlowMatchSlowPath` (flow.cpp:860-864).
    pub(in crate::js) fn check_maybe_flow_match(&mut self) -> bool {
        // JSParserImpl.h:1328-1330.
        if !self.check_name(b"match") {
            return false;
        }
        // checkMaybeFlowMatchSlowPath — flow.cpp:861-863. C++ `lookahead1(None)`
        // uses the header default `RequireNoNewLine = true` (JSLexer.h:658): a
        // newline between `match` and `(` means this is NOT a match construct
        // (e.g. `match\n(x)` is `match` then `(x)`, not `match(x)`).
        let opt_next = self.lexer.lookahead1::<true>(None);
        opt_next == Some(TokenKind::l_paren)
    }

    // -----------------------------------------------------------------------
    // reparseArgumentsAsMatchArgumentFlow — flow.cpp:866-887
    // -----------------------------------------------------------------------

    /// Validate and process an argument list into the single argument of a
    /// match statement or expression. Port of
    /// `JSParserImpl::reparseArgumentsAsMatchArgumentFlow` (flow.cpp:866-887).
    ///
    /// An empty list errors; any spread element errors; a single argument is
    /// returned directly; otherwise the list becomes a `SequenceExpression`.
    fn reparse_arguments_as_match_argument_flow(
        &mut self,
        range: SMRange,
        mut arg_list: Vec<&'gc Node<'gc>>,
    ) -> &'gc Node<'gc> {
        // flow.cpp:869-871.
        if arg_list.is_empty() {
            self.error_at(range, "'match' argument must not be empty");
        }
        // flow.cpp:872-878.
        for arg in &arg_list {
            if matches!(arg, Node::SpreadElement(_)) {
                self.error_at(
                    arg.range(),
                    "'match' argument cannot contain spread elements",
                );
            }
        }
        // flow.cpp:879-883.
        if arg_list.len() == 1 {
            return arg_list.pop().unwrap();
        }
        // flow.cpp:884-887.
        let node = Node::SequenceExpression(SequenceExpression::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, arg_list),
        ));
        self.set_location(range.start, range.end, node)
    }

    // -----------------------------------------------------------------------
    // tryParseMatchStatementFlow — flow.cpp:890-994
    // -----------------------------------------------------------------------

    /// Try to parse a `match` statement, with the cursor at `match`. Port of
    /// `JSParserImpl::tryParseMatchStatementFlow` (flow.cpp:890-994).
    ///
    /// Tri-state, mirroring the C++ `Optional<Optional<Node*>>` idiom:
    /// - `None` — a hard error was reported; propagate with `?`.
    /// - `Some(None)` — not a match statement; fall through to an
    ///   expression-statement.
    /// - `Some(Some(node))` — a `MatchStatement`.
    pub(in crate::js) fn try_parse_match_statement_flow(
        &mut self,
        param: Param,
    ) -> Option<Option<&'gc Node<'gc>>> {
        let start_loc: SMLoc;
        let args_start_loc: SMLoc;
        let args_end_loc: SMLoc;
        let arg_list: Vec<&'gc Node<'gc>>;
        {
            // flow.cpp:896-904: this save point is required because Flow
            // supports both match statements and match expressions, and `match`
            // is not reserved. We don't suppress errors as the only place that
            // could error here is `parseArguments`, which would error
            // identically if this was parsed as an expression.
            let sp = self.lexer.save_point();

            // flow.cpp:905-906: checked already by `checkMaybeFlowMatch`.
            debug_assert!(self.check_name(b"match"));
            start_loc = self.advance(GrammarContext::AllowRegExp).start;
            // flow.cpp:908-909: checked already by `checkMaybeFlowMatch`.
            debug_assert!(!self.lexer.is_new_line_before_current_token());

            // flow.cpp:911-913.
            args_start_loc = self.cur_start();
            let (al, ael) = self.parse_arguments()?;
            arg_list = al;
            args_end_loc = ael;

            // flow.cpp:915-919: not a match statement → roll back.
            if self.lexer.is_new_line_before_current_token()
                || !self.check(TokenKind::l_brace)
            {
                sp.restore(&mut self.lexer);
                return Some(None);
            }
        }
        // flow.cpp:921: we are unambiguously parsing a match statement now.

        // flow.cpp:923-924.
        let arg = self.reparse_arguments_as_match_argument_flow(
            SMRange { start: args_start_loc, end: args_end_loc },
            arg_list,
        );

        // flow.cpp:926-927.
        debug_assert!(self.check(TokenKind::l_brace));
        let lbrace_loc = self.advance(GrammarContext::AllowRegExp).start;

        // flow.cpp:929-979.
        let mut cases: Vec<&'gc Node<'gc>> = Vec::new();
        while !self.check(TokenKind::r_brace) {
            let case_start_loc = self.cur_start();

            let pattern = self.parse_match_pattern_flow()?;

            // flow.cpp:938-944.
            let mut guard: Option<&'gc Node<'gc>> = None;
            if self.check(TokenKind::rw_if) {
                guard = Some(self.parse_match_case_guard_flow()?);
            }

            // flow.cpp:946-952.
            if !self.eat_at(
                TokenKind::equalgreater,
                GrammarContext::AllowRegExp,
                " after match pattern",
                Some("location of pattern"),
                case_start_loc,
            ) {
                return None;
            }

            // flow.cpp:954-966: a match *statement* case body must be a
            // block; only a match *expression* case body may be an arbitrary
            // expression. `parse_block` asserts that the current token is
            // '{', so without this check a non-block body such as
            // `match (x) { _ => 1 };` panics instead of reporting an error.
            // The C++ had the same defect and was fixed alongside this (see
            // `test/Parser/flow/match/statement-non-block-body-error.js`).
            if !self.need_at(
                TokenKind::l_brace,
                " in 'match' statement case body",
                Some("location of pattern"),
                case_start_loc,
            ) {
                return None;
            }

            let body =
                self.parse_block(param.get(PARAM_RETURN), GrammarContext::AllowRegExp, false)?;

            // flow.cpp:970-974.
            let node = Node::MatchStatementCase(MatchStatementCase::new(
                NodeMetadata::new(self.dummy_range()),
                pattern,
                body,
                guard,
            ));
            let body_end = body.range().end;
            cases.push(self.set_location(case_start_loc, body_end, node));

            // flow.cpp:976-978: the comma is optional between statement cases.
            if self.check(TokenKind::comma) {
                self.advance(GrammarContext::AllowRegExp);
            }
        }

        // flow.cpp:981-988.
        let end_loc = self.cur_range().end;
        if !self.eat_at(
            TokenKind::r_brace,
            GrammarContext::AllowRegExp,
            " at end of 'match' statement",
            Some("location of '{'"),
            lbrace_loc,
        ) {
            return None;
        }

        // flow.cpp:990-993.
        let node = Node::MatchStatement(MatchStatement::new(
            NodeMetadata::new(self.dummy_range()),
            arg,
            NodeList::from_iter(self.gc, cases),
        ));
        Some(Some(self.set_location(start_loc, end_loc, node)))
    }

    // -----------------------------------------------------------------------
    // parseMatchCallOrMatchExpressionFlow — flow.cpp:996-1031
    // -----------------------------------------------------------------------

    /// Parse either a plain call to a function named `match`, or a `match`
    /// expression, with the cursor at `match`. Port of
    /// `JSParserImpl::parseMatchCallOrMatchExpressionFlow` (flow.cpp:996-1031).
    ///
    /// Unlike the statement form, this NEVER uses a `SavePoint`: it parses the
    /// argument list once and then either reinterprets it as a match argument
    /// (followed by `{`) or keeps it as a call's arguments.
    pub(in crate::js) fn parse_match_call_or_match_expression_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // flow.cpp:997-1002.
        let start_loc = self.cur_start();
        let match_ident_name = self.lexer.token().get_identifier();
        let ident_start = self.lexer.token().start_loc();
        let ident_end = self.lexer.token().end_loc();
        let ident_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            match_ident_name,
            None,  // typeAnnotation
            false, // optional
        ));
        let match_ident_node = self.set_location(ident_start, ident_end, ident_node);
        // flow.cpp:1003.
        self.advance(GrammarContext::AllowRegExp);

        // flow.cpp:1005-1010.
        let paren_loc = self.cur_start();
        let (arg_list, end_loc) = self.parse_arguments()?;

        // flow.cpp:1012-1016: a `{` (with no preceding newline) means this is a
        // match expression.
        if !self.lexer.is_new_line_before_current_token()
            && self.check(TokenKind::l_brace)
        {
            let arg = self.reparse_arguments_as_match_argument_flow(
                SMRange { start: paren_loc, end: end_loc },
                arg_list,
            );
            return self.parse_match_expression_flow(start_loc, arg);
        }

        // flow.cpp:1018-1030: otherwise it's a real call; continue the LHS tail.
        let call_node = Node::CallExpression(CallExpression::new(
            NodeMetadata::new(self.dummy_range()),
            match_ident_node,
            None, // typeArguments
            NodeList::from_iter(self.gc, arg_list),
        ));
        // setLocation(startLoc, endLoc, parenLoc, ...): the debug location is
        // the `(`.
        let call_node = self.set_location_d(start_loc, end_loc, paren_loc, call_node);
        let expr = self.parse_optional_expression_except_new_tail(
            IsConstructorCall::No,
            start_loc,
            call_node,
        )?;
        self.parse_left_hand_side_expression_tail(
            start_loc,
            expr,
            IsClassHeritageArgument::No,
        )
    }

    // -----------------------------------------------------------------------
    // parseMatchExpressionFlow — flow.cpp:1033-1091
    // -----------------------------------------------------------------------

    /// Parse a `match` expression body, with the cursor at `{`. Port of
    /// `JSParserImpl::parseMatchExpressionFlow` (flow.cpp:1033-1091).
    fn parse_match_expression_flow(
        &mut self,
        start_loc: SMLoc,
        argument: &'gc Node<'gc>,
    ) -> Option<&'gc Node<'gc>> {
        // flow.cpp:1036-1037.
        debug_assert!(self.check(TokenKind::l_brace));
        let lbrace_loc = self.advance(GrammarContext::AllowRegExp).start;

        // flow.cpp:1039-1076.
        let mut cases: Vec<&'gc Node<'gc>> = Vec::new();
        while !self.check(TokenKind::r_brace) {
            let case_start_loc = self.cur_start();

            let pattern = self.parse_match_pattern_flow()?;

            // flow.cpp:1048-1054.
            let mut guard: Option<&'gc Node<'gc>> = None;
            if self.check(TokenKind::rw_if) {
                guard = Some(self.parse_match_case_guard_flow()?);
            }

            // flow.cpp:1056-1062.
            if !self.eat_at(
                TokenKind::equalgreater,
                GrammarContext::AllowRegExp,
                " after match pattern",
                Some("location of pattern"),
                case_start_loc,
            ) {
                return None;
            }

            // flow.cpp:1064-1066: expression-case body is an assignment
            // expression (with the header defaults Yes/Yes/None).
            let body = self.parse_assignment_expression(
                PARAM_IN,
                false,
                AllowTypedArrowFunction::Yes,
                CoverTypedParameters::Yes,
                None,
            )?;

            // flow.cpp:1068-1072.
            let node = Node::MatchExpressionCase(MatchExpressionCase::new(
                NodeMetadata::new(self.dummy_range()),
                pattern,
                body,
                guard,
            ));
            let body_end = body.range().end;
            cases.push(self.set_location(case_start_loc, body_end, node));

            // flow.cpp:1074-1075: the comma is required between expression cases
            // (trailing comma allowed); its absence ends the loop.
            if !self.check_and_eat(TokenKind::comma, GrammarContext::AllowRegExp) {
                break;
            }
        }

        // flow.cpp:1078-1085.
        let end_loc = self.cur_range().end;
        if !self.eat_at(
            TokenKind::r_brace,
            GrammarContext::AllowRegExp,
            " at end of 'match' expression",
            Some("location of '{'"),
            lbrace_loc,
        ) {
            return None;
        }

        // flow.cpp:1087-1090.
        let node = Node::MatchExpression(MatchExpression::new(
            NodeMetadata::new(self.dummy_range()),
            argument,
            NodeList::from_iter(self.gc, cases),
        ));
        Some(self.set_location(start_loc, end_loc, node))
    }

    // -----------------------------------------------------------------------
    // parseMatchCaseGuardFlow — flow.cpp:1093-1115
    // -----------------------------------------------------------------------

    /// Parse a `match` case guard `if ( <expr> )`, with the cursor at `if`.
    /// Port of `JSParserImpl::parseMatchCaseGuardFlow` (flow.cpp:1093-1115).
    /// The guard keyword is the reserved `if`, NOT a contextual `when`.
    fn parse_match_case_guard_flow(&mut self) -> Option<&'gc Node<'gc>> {
        // flow.cpp:1094-1095.
        debug_assert!(self.check(TokenKind::rw_if));
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;
        // flow.cpp:1096: captured before the `(` is eaten, so it names the
        // guard's opening paren.
        let cond_loc = self.cur_start();
        // flow.cpp:1097-1103.
        if !self.eat_at(
            TokenKind::l_paren,
            GrammarContext::AllowRegExp,
            " after 'if' guard",
            Some("location of 'if' guard"),
            start_loc,
        ) {
            return None;
        }
        // flow.cpp:1104-1106.
        let guard = self.parse_expression(PARAM_IN, CoverTypedParameters::No)?;
        // flow.cpp:1107-1113.
        if !self.eat_at(
            TokenKind::r_paren,
            GrammarContext::AllowRegExp,
            " at end of 'if' guard",
            Some("'if' guard starts here"),
            cond_loc,
        ) {
            return None;
        }
        Some(guard)
    }

    // -----------------------------------------------------------------------
    // parseMatchPatternFlow — flow.cpp:1117-1160
    // -----------------------------------------------------------------------

    /// Parse a full match pattern (with leading `|`, or-patterns, and trailing
    /// `as` binding). Port of `JSParserImpl::parseMatchPatternFlow`
    /// (flow.cpp:1117-1160).
    fn parse_match_pattern_flow(&mut self) -> Option<&'gc Node<'gc>> {
        // flow.cpp:1118-1119: optional leading `|`.
        let start_loc = self.cur_start();
        self.check_and_eat(TokenKind::pipe, GrammarContext::AllowRegExp);

        // flow.cpp:1120-1123.
        let first_pattern = self.parse_match_subpattern_flow()?;
        let mut pattern = first_pattern;

        // flow.cpp:1124-1137: or-pattern.
        if self.check(TokenKind::pipe) {
            let mut patterns: Vec<&'gc Node<'gc>> = vec![first_pattern];
            while self.check_and_eat(TokenKind::pipe, GrammarContext::AllowRegExp) {
                let p = self.parse_match_subpattern_flow()?;
                patterns.push(p);
            }
            let node = Node::MatchOrPattern(MatchOrPattern::new(
                NodeMetadata::new(self.dummy_range()),
                NodeList::from_iter(self.gc, patterns),
            ));
            pattern = self.set_location(start_loc, self.lexer.prev_token_end(), node);
        }

        // flow.cpp:1138-1158: trailing `as` binding. C++ `checkAndEat(asIdent_)`
        // — `as` is a contextual identifier (not a token), so check by name and
        // advance manually (the default `checkAndEat` grammar context is
        // AllowRegExp).
        if self.check_name(b"as") {
            self.advance(GrammarContext::AllowRegExp);
            let target: &'gc Node<'gc>;
            // flow.cpp:1140-1144: `as const`/`as var`/`as let` binding target.
            // checkN(rw_const, rw_var, letIdent_) is MIXED — there is no
            // rw_let.
            if self.check(TokenKind::rw_const)
                || self.check(TokenKind::rw_var)
                || self.check_name(b"let")
            {
                target = self.parse_match_binding_pattern_flow()?;
            } else if self.check(TokenKind::identifier)
                || self.lexer.token().is_res_word()
            {
                // flow.cpp:1145-1149.
                target = self.parse_match_binding_identifier_flow()?;
            } else {
                // flow.cpp:1150-1153.
                self.error_cur("expected identifier or binding pattern");
                return None;
            }
            let node = Node::MatchAsPattern(MatchAsPattern::new(
                NodeMetadata::new(self.dummy_range()),
                pattern,
                target,
            ));
            pattern = self.set_location(start_loc, self.lexer.prev_token_end(), node);
        }
        Some(pattern)
    }

    // -----------------------------------------------------------------------
    // parseMatchSubpatternFlow — flow.cpp:1162-1401
    // -----------------------------------------------------------------------

    /// Parse a single match subpattern (the big switch on token kind). Port of
    /// `JSParserImpl::parseMatchSubpatternFlow` (flow.cpp:1162-1401).
    fn parse_match_subpattern_flow(&mut self) -> Option<&'gc Node<'gc>> {
        match self.cur_kind() {
            // flow.cpp:1164-1171: null literal.
            TokenKind::rw_null => {
                let lit = self.make_match_null_literal();
                let pat = self.wrap_match_literal_pattern(lit);
                self.advance(GrammarContext::AllowDiv);
                Some(pat)
            }

            // flow.cpp:1173-1184: boolean literal.
            TokenKind::rw_true | TokenKind::rw_false => {
                let value = self.cur_kind() == TokenKind::rw_true;
                let tok_start = self.lexer.token().start_loc();
                let tok_end = self.lexer.token().end_loc();
                let lit_node = Node::BooleanLiteral(BooleanLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    value,
                ));
                let lit = self.set_location(tok_start, tok_end, lit_node);
                let pat = self.wrap_match_literal_pattern(lit);
                self.advance(GrammarContext::AllowDiv);
                Some(pat)
            }

            // flow.cpp:1186-1195: numeric literal.
            TokenKind::numeric_literal => {
                let lit = self.make_match_numeric_literal();
                let pat = self.wrap_match_literal_pattern(lit);
                self.advance(GrammarContext::AllowDiv);
                Some(pat)
            }

            // flow.cpp:1197-1206: bigint literal.
            TokenKind::bigint_literal => {
                let lit = self.make_match_bigint_literal();
                let pat = self.wrap_match_literal_pattern(lit);
                self.advance(GrammarContext::AllowDiv);
                Some(pat)
            }

            // flow.cpp:1208-1217: string literal.
            TokenKind::string_literal => {
                let lit = self.make_match_string_literal();
                let pat = self.wrap_match_literal_pattern(lit);
                self.advance(GrammarContext::AllowDiv);
                Some(pat)
            }

            // flow.cpp:1219-1329: identifier.
            TokenKind::identifier => self.parse_match_identifier_subpattern_flow(),

            // flow.cpp:1331-1364: unary `+`/`-` pattern.
            TokenKind::plus | TokenKind::minus => {
                let op_bytes = match self.cur_kind() {
                    TokenKind::plus => b"+".as_slice(),
                    _ => b"-".as_slice(),
                };
                let op = self.gc.ctx().atom_table.atom_bytes(op_bytes);
                let start_loc = self.advance(GrammarContext::AllowRegExp).start;

                let argument = match self.cur_kind() {
                    // flow.cpp:1338-1345.
                    TokenKind::numeric_literal => {
                        let lit = self.make_match_numeric_literal();
                        self.advance(GrammarContext::AllowDiv);
                        lit
                    }
                    // flow.cpp:1347-1354.
                    TokenKind::bigint_literal => {
                        let lit = self.make_match_bigint_literal();
                        self.advance(GrammarContext::AllowDiv);
                        lit
                    }
                    // flow.cpp:1356-1358. Point location, NOT the current
                    // token's range: C++ calls `error(tok_->getStartLoc(),
                    // ...)` — the `error(SMLoc, Twine)` overload.
                    _ => {
                        self.error_at_loc(
                            self.cur_start(),
                            "invalid match unary pattern argument",
                        );
                        return None;
                    }
                };
                let node = Node::MatchUnaryPattern(MatchUnaryPattern::new(
                    NodeMetadata::new(self.dummy_range()),
                    argument,
                    op,
                ));
                Some(self.set_location(start_loc, self.lexer.prev_token_end(), node))
            }

            // flow.cpp:1366-1372: `const`/`var` binding.
            TokenKind::rw_const | TokenKind::rw_var => {
                self.parse_match_binding_pattern_flow()
            }

            // flow.cpp:1374-1389: group pattern (no wrapper node).
            TokenKind::l_paren => {
                let start_loc = self.advance(GrammarContext::AllowRegExp).start;
                let pattern = self.parse_match_pattern_flow()?;
                // flow.cpp:1380-1386.
                if !self.eat_at(
                    TokenKind::r_paren,
                    GrammarContext::AllowDiv,
                    " at end of a match pattern group",
                    Some("location of '('"),
                    start_loc,
                ) {
                    return None;
                }
                Some(pattern)
            }

            // flow.cpp:1391-1392.
            TokenKind::l_brace => self.parse_match_object_pattern_flow(),

            // flow.cpp:1394-1395.
            TokenKind::l_square => self.parse_match_array_pattern_flow(),

            // flow.cpp:1397-1399. Point location, NOT the current token's
            // range: C++ calls `error(tok_->getStartLoc(), ...)` — the
            // `error(SMLoc, Twine)` overload.
            _ => {
                self.error_at_loc(self.cur_start(), "invalid match pattern");
                None
            }
        }
    }

    /// The `identifier` arm of `parseMatchSubpatternFlow` (flow.cpp:1219-1329):
    /// wildcard `_`, `let` binding, or an identifier pattern optionally followed
    /// by a `.`/`[lit]` member chain and an instance `{ … }`.
    fn parse_match_identifier_subpattern_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // flow.cpp:1220-1225: wildcard `_`.
        if self.check_name(b"_") {
            let tok_start = self.lexer.token().start_loc();
            let tok_end = self.lexer.token().end_loc();
            let node = Node::MatchWildcardPattern(MatchWildcardPattern::new(
                NodeMetadata::new(self.dummy_range()),
            ));
            let pat = self.set_location(tok_start, tok_end, node);
            self.advance(GrammarContext::AllowDiv);
            return Some(pat);
        }
        // flow.cpp:1226-1231: `let` binding.
        if self.check_name(b"let") {
            return self.parse_match_binding_pattern_flow();
        }

        // flow.cpp:1232-1240: identifier pattern.
        let start_loc = self.cur_start();
        let ident = self.make_match_current_identifier();
        let pat_node = Node::MatchIdentifierPattern(MatchIdentifierPattern::new(
            NodeMetadata::new(self.dummy_range()),
            ident,
        ));
        let tok_start = self.lexer.token().start_loc();
        let tok_end = self.lexer.token().end_loc();
        let mut pat = self.set_location(tok_start, tok_end, pat_node);
        self.advance(GrammarContext::AllowDiv);

        // flow.cpp:1242-1314: `.`/`[lit]` member chain.
        while self.check2(TokenKind::period, TokenKind::l_square) {
            if self.check_and_eat(TokenKind::period, GrammarContext::AllowRegExp) {
                // flow.cpp:1244-1248: need(identifier, "in match member
                // pattern", nullptr, {}) — a genuine no-hint call site (no
                // `what`/`whatLoc` at all), so the plain 2-arg `need` form is
                // correct here.
                if !self.need(TokenKind::identifier, " in match member pattern") {
                    return None;
                }
                let property = self.make_match_current_identifier();
                self.advance(GrammarContext::AllowDiv);
                let node = Node::MatchMemberPattern(MatchMemberPattern::new(
                    NodeMetadata::new(self.dummy_range()),
                    pat,
                    property,
                ));
                pat = self.set_location(start_loc, self.lexer.prev_token_end(), node);
            } else {
                // flow.cpp:1260-1313: computed member `[lit]`.
                let computed_start_loc =
                    self.advance(GrammarContext::AllowRegExp).start; // Eat `[`
                let property = match self.cur_kind() {
                    TokenKind::numeric_literal => {
                        let lit = self.make_match_numeric_literal();
                        self.advance(GrammarContext::AllowDiv);
                        lit
                    }
                    TokenKind::bigint_literal => {
                        let lit = self.make_match_bigint_literal();
                        self.advance(GrammarContext::AllowDiv);
                        lit
                    }
                    TokenKind::string_literal => {
                        let lit = self.make_match_string_literal();
                        self.advance(GrammarContext::AllowDiv);
                        lit
                    }
                    _ => {
                        // flow.cpp:1291-1300: whatLoc is `computedStartLoc`
                        // (the `[` that opened the computed property).
                        self.error_expected3(
                            TokenKind::numeric_literal,
                            TokenKind::bigint_literal,
                            TokenKind::string_literal,
                            " in match member pattern computed property",
                            Some("start of computed property"),
                            computed_start_loc,
                        );
                        return None;
                    }
                };
                // flow.cpp:1302-1308.
                if !self.eat_at(
                    TokenKind::r_square,
                    GrammarContext::AllowDiv,
                    " at end of computed member property",
                    Some("location of '['"),
                    computed_start_loc,
                ) {
                    return None;
                }
                let node = Node::MatchMemberPattern(MatchMemberPattern::new(
                    NodeMetadata::new(self.dummy_range()),
                    pat,
                    property,
                ));
                pat = self.set_location(start_loc, self.lexer.prev_token_end(), node);
            }
        }

        // flow.cpp:1316-1326: instance pattern with object fields.
        if self.check(TokenKind::l_brace) {
            let props = self.parse_match_instance_object_pattern_flow()?;
            let node = Node::MatchInstancePattern(MatchInstancePattern::new(
                NodeMetadata::new(self.dummy_range()),
                pat,
                props,
            ));
            return Some(self.set_location(start_loc, self.lexer.prev_token_end(), node));
        }

        // flow.cpp:1328.
        Some(pat)
    }

    // -----------------------------------------------------------------------
    // parseMatchBindingIdentifierFlow — flow.cpp:1403-1412
    // -----------------------------------------------------------------------

    /// Parse a binding identifier inside a match pattern. Port of
    /// `JSParserImpl::parseMatchBindingIdentifierFlow` (flow.cpp:1403-1412).
    fn parse_match_binding_identifier_flow(&mut self) -> Option<&'gc Node<'gc>> {
        // flow.cpp:1405-1408.
        let id = self.lexer.token().get_res_word_or_identifier();
        let kind = self.cur_kind();
        let range = self.cur_range();
        // Bind the interned bytes to an owned buffer so the immutable borrow of
        // the atom table ends before the `&mut self` validate call.
        let id_bytes = self.gc.ctx().atom_table.bytes(id).to_owned();
        if !self.validate_binding_identifier(range, &id_bytes, kind) {
            return None;
        }
        // flow.cpp:1409-1411. NB: C++ builds the IdentifierNode with
        // setLocation(tok_, tok_, ...) AFTER advancing, so the location is the
        // token that FOLLOWS the identifier (a faithful quirk we preserve).
        self.advance(GrammarContext::AllowRegExp);
        let tok_start = self.lexer.token().start_loc();
        let tok_end = self.lexer.token().end_loc();
        let node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            None,
            false,
        ));
        Some(self.set_location(tok_start, tok_end, node))
    }

    // -----------------------------------------------------------------------
    // parseMatchBindingPatternFlow — flow.cpp:1414-1435
    // -----------------------------------------------------------------------

    /// Parse a `const`/`var`/`let` binding pattern in a match pattern. Port of
    /// `JSParserImpl::parseMatchBindingPatternFlow` (flow.cpp:1414-1435).
    fn parse_match_binding_pattern_flow(&mut self) -> Option<&'gc Node<'gc>> {
        // flow.cpp:1416: checkN(rw_const, rw_var, letIdent_) is MIXED.
        debug_assert!(
            self.check(TokenKind::rw_const)
                || self.check(TokenKind::rw_var)
                || self.check_name(b"let")
        );
        // flow.cpp:1417-1418.
        let kind = self.lexer.token().get_res_word_or_identifier();
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;
        // flow.cpp:1419-1426: errorExpected(identifier, "in match binding
        // pattern", "start of binding pattern", startLoc), then bail out.
        // `startLoc` is real.
        //
        // HISTORY: C++ used to report the error and keep going, so
        // `parseMatchBindingIdentifierFlow` read the identifier off the
        // current (non-identifier, non-reserved-word) token and tripped
        // `Token::getResWordOrIdentifier`'s assert (JSLexer.h:160) — the port
        // mirrored that as the `debug_assert!` in
        // `Token::get_res_word_or_identifier` (token.rs:133), which panicked
        // on the same input (defect 11 in CppDefectsFound.md). Upstream fixed
        // it in `550aafe33` ("Fix crash after reporting a bad match binding
        // pattern") by returning `None` here; this is the mirror of that fix.
        if !self.check(TokenKind::identifier) && !self.lexer.token().is_res_word() {
            self.error_expected_msg(
                "'identifier' expected in match binding pattern",
                Some("start of binding pattern"),
                Some(start_loc),
            );
            return None;
        }
        // flow.cpp:1427-1429.
        let ident = self.parse_match_binding_identifier_flow()?;
        // flow.cpp:1430-1434.
        let node = Node::MatchBindingPattern(MatchBindingPattern::new(
            NodeMetadata::new(self.dummy_range()),
            ident,
            kind,
        ));
        Some(self.set_location(start_loc, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseMatchRestPatternFlow — flow.cpp:1437-1451
    // -----------------------------------------------------------------------

    /// Parse a match rest pattern `... [const|var|let id]`. Port of
    /// `JSParserImpl::parseMatchRestPatternFlow` (flow.cpp:1437-1451). The
    /// binding is OPTIONAL; bare `...rest` (no binding keyword) is a parse error
    /// downstream — faithful to hermesc.
    fn parse_match_rest_pattern_flow(&mut self) -> Option<&'gc Node<'gc>> {
        // flow.cpp:1438-1439.
        debug_assert!(self.check(TokenKind::dotdotdot));
        let rest_start_loc = self.advance(GrammarContext::AllowRegExp).start;
        // flow.cpp:1440-1446.
        let mut arg: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::rw_const)
            || self.check(TokenKind::rw_var)
            || self.check_name(b"let")
        {
            arg = Some(self.parse_match_binding_pattern_flow()?);
        }
        // flow.cpp:1447-1450.
        let node = Node::MatchRestPattern(MatchRestPattern::new(
            NodeMetadata::new(self.dummy_range()),
            arg,
        ));
        Some(self.set_location(rest_start_loc, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseMatchObjectPatternPropertiesFlow — flow.cpp:1453-1560
    // -----------------------------------------------------------------------

    /// Parse the body (properties + optional rest + closing `}`) shared by
    /// object and instance-object match patterns. Port of
    /// `JSParserImpl::parseMatchObjectPatternPropertiesFlow`
    /// (flow.cpp:1453-1560). Returns the properties and the optional rest on
    /// success.
    fn parse_match_object_pattern_properties_flow(
        &mut self,
        start_loc: SMLoc,
    ) -> Option<(Vec<&'gc Node<'gc>>, Option<&'gc Node<'gc>>)> {
        let mut properties: Vec<&'gc Node<'gc>> = Vec::new();
        let mut rest: Option<&'gc Node<'gc>> = None;

        // flow.cpp:1457-1550.
        while !self.check(TokenKind::r_brace) {
            // flow.cpp:1458-1465: rest.
            if self.check(TokenKind::dotdotdot) {
                rest = Some(self.parse_match_rest_pattern_flow()?);
                break;
            }

            let prop_start_loc = self.cur_start();
            // flow.cpp:1469-1479: shorthand `const x` ≡ `x: const x`.
            let prop = if self.check(TokenKind::rw_const)
                || self.check(TokenKind::rw_var)
                || self.check_name(b"let")
            {
                let binding_pattern = self.parse_match_binding_pattern_flow()?;
                // flow.cpp:1475-1479: key is bindingPattern->_id.
                let key = match binding_pattern {
                    Node::MatchBindingPattern(bp) => bp.id,
                    _ => unreachable!("parse_match_binding_pattern_flow returns MatchBindingPattern"),
                };
                let node = Node::MatchObjectPatternProperty(
                    MatchObjectPatternProperty::new(
                        NodeMetadata::new(self.dummy_range()),
                        key,
                        binding_pattern,
                        true,
                    ),
                );
                self.set_location(prop_start_loc, self.lexer.prev_token_end(), node)
            } else {
                // flow.cpp:1480-1545: normal property `key: pattern`.
                let key = match self.cur_kind() {
                    // flow.cpp:1485-1492.
                    TokenKind::identifier => {
                        let k = self.make_match_current_identifier();
                        self.advance(GrammarContext::AllowDiv);
                        k
                    }
                    // flow.cpp:1494-1502.
                    TokenKind::string_literal => {
                        let k = self.make_match_string_literal();
                        self.advance(GrammarContext::AllowDiv);
                        k
                    }
                    // flow.cpp:1503-1510.
                    TokenKind::numeric_literal => {
                        let k = self.make_match_numeric_literal();
                        self.advance(GrammarContext::AllowDiv);
                        k
                    }
                    // flow.cpp:1512-1520.
                    TokenKind::bigint_literal => {
                        let k = self.make_match_bigint_literal();
                        self.advance(GrammarContext::AllowDiv);
                        k
                    }
                    // flow.cpp:1521-1531.
                    _ => {
                        // flow.cpp:1521-1531: whatLoc is `propStartLoc`.
                        self.error_expected4(
                            TokenKind::identifier,
                            TokenKind::string_literal,
                            TokenKind::numeric_literal,
                            TokenKind::bigint_literal,
                            " in match object pattern property key",
                            Some("start of match object pattern property key"),
                            prop_start_loc,
                        );
                        return None;
                    }
                };
                // flow.cpp:1533-1539.
                if !self.eat_at(
                    TokenKind::colon,
                    GrammarContext::AllowRegExp,
                    " in match object pattern property",
                    Some("start of match object pattern property"),
                    prop_start_loc,
                ) {
                    return None;
                }
                // flow.cpp:1540-1547. The `?` is upstream `ca6de21ce`
                // ("Check the parsed value of a match object property"):
                // C++ used to call `.getValue()` on this `Optional` without
                // checking it, so a property value that failed to parse
                // dereferenced null right after the error was reported
                // (`match (x) { {a: *}: 2 }`, and `{a: const [y]}` through
                // the binding-pattern path). The fix adds `if (!optPattern)
                // return false;`, which is what `?` has always done here —
                // this port never had the defect, and the two upstream
                // regression tests are imported as
                // `sema_corpus/flow-match-pattern-object-{value,binding}-
                // error.js` to keep it that way.
                let pattern = self.parse_match_pattern_flow()?;
                let node = Node::MatchObjectPatternProperty(
                    MatchObjectPatternProperty::new(
                        NodeMetadata::new(self.dummy_range()),
                        key,
                        pattern,
                        false,
                    ),
                );
                self.set_location(prop_start_loc, self.lexer.prev_token_end(), node)
            };
            properties.push(prop);
            // flow.cpp:1548-1549.
            if !self.check_and_eat(TokenKind::comma, GrammarContext::AllowRegExp) {
                break;
            }
        }
        // flow.cpp:1551-1557.
        if !self.eat_at(
            TokenKind::r_brace,
            GrammarContext::AllowDiv,
            " at end of object match pattern",
            Some("location of '{'"),
            start_loc,
        ) {
            return None;
        }
        Some((properties, rest))
    }

    // -----------------------------------------------------------------------
    // parseMatchObjectPatternFlow — flow.cpp:1562-1575
    // -----------------------------------------------------------------------

    /// Parse a match object pattern `{ … }`, with the cursor at `{`. Port of
    /// `JSParserImpl::parseMatchObjectPatternFlow` (flow.cpp:1562-1575).
    fn parse_match_object_pattern_flow(&mut self) -> Option<&'gc Node<'gc>> {
        // flow.cpp:1563-1564.
        debug_assert!(self.check(TokenKind::l_brace));
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;
        // flow.cpp:1568-1569.
        let (properties, rest) =
            self.parse_match_object_pattern_properties_flow(start_loc)?;
        // flow.cpp:1571-1575.
        let node = Node::MatchObjectPattern(MatchObjectPattern::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, properties),
            rest,
        ));
        Some(self.set_location(start_loc, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseMatchInstanceObjectPatternFlow — flow.cpp:1577-1590
    // -----------------------------------------------------------------------

    /// Parse the `{ … }` fields of an instance match pattern. Port of
    /// `JSParserImpl::parseMatchInstanceObjectPatternFlow` (flow.cpp:1577-1590).
    fn parse_match_instance_object_pattern_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // flow.cpp:1579-1580.
        debug_assert!(self.check(TokenKind::l_brace));
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;
        // flow.cpp:1584-1585.
        let (properties, rest) =
            self.parse_match_object_pattern_properties_flow(start_loc)?;
        // flow.cpp:1587-1591.
        let node = Node::MatchInstanceObjectPattern(
            MatchInstanceObjectPattern::new(
                NodeMetadata::new(self.dummy_range()),
                NodeList::from_iter(self.gc, properties),
                rest,
            ),
        );
        Some(self.set_location(start_loc, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseMatchArrayPatternFlow — flow.cpp:1592-1629
    // -----------------------------------------------------------------------

    /// Parse a match array pattern `[ … ]`, with the cursor at `[`. Port of
    /// `JSParserImpl::parseMatchArrayPatternFlow` (flow.cpp:1592-1629).
    fn parse_match_array_pattern_flow(&mut self) -> Option<&'gc Node<'gc>> {
        // flow.cpp:1595-1596.
        debug_assert!(self.check(TokenKind::l_square));
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;
        let mut elements: Vec<&'gc Node<'gc>> = Vec::new();
        let mut rest: Option<&'gc Node<'gc>> = None;

        // flow.cpp:1600-1616.
        while !self.check(TokenKind::r_square) {
            // flow.cpp:1601-1608: rest.
            if self.check(TokenKind::dotdotdot) {
                rest = Some(self.parse_match_rest_pattern_flow()?);
                break;
            }
            // flow.cpp:1610-1613.
            let pattern = self.parse_match_pattern_flow()?;
            elements.push(pattern);
            // flow.cpp:1614-1615.
            if !self.check_and_eat(TokenKind::comma, GrammarContext::AllowRegExp) {
                break;
            }
        }
        // flow.cpp:1617-1623.
        if !self.eat_at(
            TokenKind::r_square,
            GrammarContext::AllowDiv,
            " at end of array match pattern",
            Some("location of '['"),
            start_loc,
        ) {
            return None;
        }
        // flow.cpp:1625-1628.
        let node = Node::MatchArrayPattern(MatchArrayPattern::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, elements),
            rest,
        ));
        Some(self.set_location(start_loc, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // Small literal/identifier construction helpers (factored out of the
    // repeated `setLocation(tok_, tok_, new LiteralNode(...))` idioms above).
    // -----------------------------------------------------------------------

    /// Build a `NullLiteral` located at the current token (no advance).
    fn make_match_null_literal(&mut self) -> &'gc Node<'gc> {
        let tok_start = self.lexer.token().start_loc();
        let tok_end = self.lexer.token().end_loc();
        let node = Node::NullLiteral(NullLiteral::new(
            NodeMetadata::new(self.dummy_range()),
        ));
        self.set_location(tok_start, tok_end, node)
    }

    /// Build a `NumericLiteral` located at the current token (no advance).
    fn make_match_numeric_literal(&mut self) -> &'gc Node<'gc> {
        let value = self.lexer.token().get_numeric_literal();
        let tok_start = self.lexer.token().start_loc();
        let tok_end = self.lexer.token().end_loc();
        let node = Node::NumericLiteral(NumericLiteral::new(
            NodeMetadata::new(self.dummy_range()),
            value,
        ));
        self.set_location(tok_start, tok_end, node)
    }

    /// Build a `BigIntLiteral` located at the current token (no advance).
    fn make_match_bigint_literal(&mut self) -> &'gc Node<'gc> {
        let bigint = self.lexer.token().get_bigint_literal();
        let tok_start = self.lexer.token().start_loc();
        let tok_end = self.lexer.token().end_loc();
        let node = Node::BigIntLiteral(BigIntLiteral::new(
            NodeMetadata::new(self.dummy_range()),
            bigint,
        ));
        self.set_location(tok_start, tok_end, node)
    }

    /// Build a `StringLiteral` located at the current token (no advance).
    fn make_match_string_literal(&mut self) -> &'gc Node<'gc> {
        let value = self.lexer.token().get_string_literal();
        let tok_start = self.lexer.token().start_loc();
        let tok_end = self.lexer.token().end_loc();
        let node = Node::StringLiteral(StringLiteral::new(
            NodeMetadata::new(self.dummy_range()),
            value,
        ));
        self.set_location(tok_start, tok_end, node)
    }

    /// Build an `Identifier` from the current token's identifier (no advance).
    fn make_match_current_identifier(&mut self) -> &'gc Node<'gc> {
        let name = self.lexer.token().get_identifier();
        let tok_start = self.lexer.token().start_loc();
        let tok_end = self.lexer.token().end_loc();
        let node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            name,
            None,
            false,
        ));
        self.set_location(tok_start, tok_end, node)
    }

    /// Wrap a literal node in a `MatchLiteralPattern` located at the current
    /// token (the literal and pattern share the single-token span). Mirrors the
    /// `setLocation(tok_, tok_, new MatchLiteralPatternNode(lit))` idiom.
    fn wrap_match_literal_pattern(
        &mut self,
        literal: &'gc Node<'gc>,
    ) -> &'gc Node<'gc> {
        let tok_start = self.lexer.token().start_loc();
        let tok_end = self.lexer.token().end_loc();
        let node = Node::MatchLiteralPattern(MatchLiteralPattern::new(
            NodeMetadata::new(self.dummy_range()),
            literal,
        ));
        self.set_location(tok_start, tok_end, node)
    }
}
