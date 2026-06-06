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
    AssignmentExpression, AwaitExpression, BigIntLiteral, BinaryExpression, BooleanLiteral,
    ConditionalExpression, Identifier, LogicalExpression, Node, NullLiteral, NumericLiteral,
    PrivateName, SequenceExpression, StringLiteral, ThisExpression, UnaryExpression,
    UpdateExpression,
};
use ast::node_child::{NodeList, NodeMetadata};
use support::location::SMLoc;

use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

use super::{JSParserImpl, Param, PARAM_IN};

// For AssignState.op field type (interned operator label).
use atom_table;

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
    /// Port of `JSParserImpl::checkEndAssignmentExpression` (lines 293-306)
    /// with `ofEndsAssignment == OfEndsAssignment::Yes` (the default).
    ///
    /// The "of" check mirrors C++ `checkUnescaped(ofIdent_)`: only fire when
    /// the current token is a plain identifier that spells "of" byte-for-byte
    /// (no `\u` escapes). In P1 we don't track the "no-escape" flag here, but
    /// the identifier parser interns unescaped identifiers normally, so we
    /// just compare the interned bytes to `b"of"`.
    #[inline]
    fn check_end_assignment_expression(&self) -> bool {
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
        // checkUnescaped(ofIdent_): identifier spelled "of"
        if self.cur_kind() == TokenKind::identifier {
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
    /// ## Deferrals (honest errors)
    /// - P3: `yield` — emits error + returns `None`.
    /// - P3: `=>` arrow — emits error + returns `None`.
    /// - P1.8: destructuring reparse (ArrayExpression/ObjectExpression LHS) —
    ///   unreachable in P1 (those parse forms error first in
    ///   `parse_primary_expression`), but stubbed with an honest error.
    /// - P6/P7: Flow/TS type parameters — skipped (gated by context flags that
    ///   don't exist yet).
    ///
    /// ## MAX_NESTED_ASSIGNMENTS
    /// `ESTree::MAX_NESTED_ASSIGNMENTS = 30000` (include/hermes/AST/ESTree.h:1407).
    pub(super) fn parse_assignment_expression(
        &mut self,
        param: Param,
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
                              cur_param: Param|
         -> LevelResult<'gc> {
            // ----------------------------------------------------------------
            // yield check (C++ 6257-6268) — P3 deferral.
            // paramYield_ is always false in P1; stub so it's never silent.
            // ----------------------------------------------------------------
            if this.param_yield
                && (this.check(TokenKind::rw_yield)
                    || (this.check(TokenKind::identifier)
                        && this
                            .lexer
                            .get_string_table()
                            .bytes(this.lexer.token().get_identifier())
                            == b"yield"))
            {
                this.error_cur(
                    "yield expression not yet supported (parser phase P3)",
                );
                return LevelResult::Error;
            }

            // P3: async arrow (C++ 6270-6286) — skip in P1.
            // Plain `async` parses as an Identifier downstream; no
            // special-casing needed here.

            // P6: Flow type-param block (C++ 6288-6339) — gated by
            // context_.getParseFlow() which does not exist yet. Skip.

            // C++ lines 6341-6345: leftStartLoc / hasNewLine / optLeftExpr.
            let left_start_loc = this.cur_start();
            let left_expr = match this.parse_conditional_expression(cur_param) {
                Some(e) => e,
                None => return LevelResult::Error,
            };

            // P6/P7: Flow/TS return-type / predicate blocks (C++ 6349-6446) — skip.

            // ----------------------------------------------------------------
            // Arrow check (C++ 6453-6466) — P3 deferral.
            // ----------------------------------------------------------------
            if this.check(TokenKind::equalgreater)
                && !this.lexer.is_new_line_before_current_token()
            {
                this.error_cur(
                    "arrow functions not yet supported (parser phase P3)",
                );
                return LevelResult::Error;
            }

            // P6: Flow typeParams error (C++ 6468-6477) — gated, skip.

            // C++ line 6479: if (!checkAssign()) return *state.optLeftExpr;
            if !this.check_assign() {
                return LevelResult::Terminal(left_expr);
            }

            // ----------------------------------------------------------------
            // Destructuring reparse (C++ 6483-6489) — P1.8 stub.
            // Unreachable in P1 (`[`/`{` error in parse_primary_expression
            // before we get here), but present as a forward-looking stub.
            // P1.8: implement reparse_assignment_pattern (needs array/object
            // literals).
            // ----------------------------------------------------------------
            if this.check(TokenKind::equal)
                && matches!(
                    left_expr,
                    Node::ArrayExpression(_) | Node::ObjectExpression(_)
                )
            {
                this.error_cur(
                    "destructuring assignment target not yet supported \
                     (parser phase P1.8)",
                );
                return LevelResult::Error;
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
            // First level uses the incoming `param`; subsequent RHS levels use
            // the equivalent of AllowTypedArrowFunction::Yes / CoverTypedParameters::No
            // which in plain-JS collapses to just passing param through (the only
            // behaviorally different bit, PARAM_IN, is explicitly carried through
            // C++ passes the outer `allowTypedArrowFunction`/`coverTypedParameters`/
            // `typeParams` on the first level and `Yes`/`No`/`null` on subsequent
            // levels (6499-6523). Those args are Flow/TS-only (deferred to P6/P7),
            // so in the plain-JS subset every level just threads `param`.
            match run_level(self, &mut stack, param) {
                LevelResult::Error => return None,
                LevelResult::Terminal(n) => break n,
                LevelResult::Continue => {
                    // C++ line 6513: stack.size() > MAX_NESTED_ASSIGNMENTS guard.
                    if stack.len() > MAX_NESTED_ASSIGNMENTS {
                        let range = self.cur_range();
                        self.error_at(
                            range,
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
            if !self.check_end_assignment_expression() {
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
    // reparse_assignment_pattern — P1.8 stub
    // -----------------------------------------------------------------------

    /// Stub for `JSParserImpl::reparseAssignmentPattern`.
    /// Will be implemented in P1.8 (array/object literal support).
    ///
    /// In P1.5 the call site in `parse_assignment_expression` is unreachable
    /// because `[` and `{` error in `parse_primary_expression` before the LHS
    /// can ever be an `ArrayExpression` or `ObjectExpression`.
    ///
    /// P1.8: implement reparseAssignmentPattern (needs array/object literals).
    #[allow(dead_code)]
    fn reparse_assignment_pattern(
        &mut self,
        _left: &'gc Node<'gc>,
        _is_binding: bool,
    ) -> Option<&'gc Node<'gc>> {
        // P1.8: implement reparseAssignmentPattern (needs array/object literals).
        self.error_cur(
            "destructuring assignment target not yet supported (parser phase P1.8)",
        );
        None
    }

    // -----------------------------------------------------------------------
    // parseConditionalExpression — P1.4
    // -----------------------------------------------------------------------

    /// Parse a conditional (ternary `?:`) expression. Port of
    /// `JSParserImpl::parseConditionalExpression` (lines 4477-4615).
    ///
    /// Plain-JS path only. Type-gated branches are stubbed out:
    ///   - P6/P7: cover typed identifier (4492-4501) — skipped.
    ///   - P6/P7: typed arrow backtracking (4510-4572) — skipped.
    pub(super) fn parse_conditional_expression(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        let start_loc = self.cur_start();

        let test = self.parse_binary_expression(param)?;

        // P6/P7: cover typed identifier (CoverTypedParameters / tryParseCoverTypedIdentifierNode)
        // Only reached when context_.getParseTypes() is true — skip in P1.

        if !self.check(TokenKind::question) {
            return Some(test);
        }

        let question_range = self.cur_range();

        // P6/P7: typed arrow backtracking block (4510-4572):
        // savePoint, AllowTypedArrowFunction::Yes, consequent-with-colon check.
        // Only active when context_.getParseTypes() — skip in P1.
        // `consequent` stays None; we fall through to the plain-JS path below.

        // CHECK_RECURSION: mirrors C++ line 4576 (before the !consequent block).
        let _guard = self.check_recursion()?;

        // Consume the '?'.
        self.advance(GrammarContext::AllowRegExp);

        // Parse the consequent (true branch).
        // C++ passes ParamIn | AllowTypedArrowFunction::No | CoverTypedParameters::No.
        // In P1 the extra args don't exist; we pass PARAM_IN.
        let consequent = self.parse_assignment_expression(PARAM_IN)?;

        // Eat ':' — required after '... ? ...'.
        if !self.eat(
            TokenKind::colon,
            GrammarContext::AllowRegExp,
            "in conditional expression after '... ? ...'",
        ) {
            let _ = question_range; // referenced only for the C++ error note
            return None;
        }

        // Parse the alternate (false branch).
        let alternate = self.parse_assignment_expression(param)?;

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
    /// - P7: TS type assertion `<Type>` — deferred (no parse-TS flag yet).
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

            // P7: TS type assertion `< Type > UnaryExpression`
            // Only active when context has parse-TS enabled and JSX disabled.
            // No parse-TS flag exists yet; this branch is unreachable in P1.
            TokenKind::less => {
                // P7: TS type assertion (context.getParseTS() && !getParseJSX())
                // fall through to postfix.
                self.parse_postfix_expression()
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
                if is_await && self.param_await {
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
        let expr = self.parse_left_hand_side_expression()?;

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
