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
    CallExpression, ConditionalExpression, Identifier, LogicalExpression, MemberExpression,
    MetaProperty, NewExpression, Node, NullLiteral, NumericLiteral, OptionalCallExpression,
    OptionalMemberExpression, PrivateName, SequenceExpression, SpreadElement, StringLiteral,
    ThisExpression, UnaryExpression, UpdateExpression,
};
use ast::node_child::{NodeList, NodeMetadata};
use support::location::SMLoc;

use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

use super::{IsClassHeritageArgument, IsConstructorCall, JSParserImpl, Param, PARAM_IN};

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
    /// P6/P7: Flow/TS type-argument and record expression blocks are gated by
    /// context flags that don't exist yet; they are omitted.
    pub(super) fn parse_left_hand_side_expression_tail(
        &mut self,
        start_loc: support::location::SMLoc,
        mut expr: &'gc Node<'gc>,
        _is_class_heritage_argument: IsClassHeritageArgument,
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

        // P6/P7: Flow/TS type-arguments block (4036-4062) — gated on
        // context_.getParseFlow()/getParseTS() which don't exist yet. Skip.
        // typeArgs stays None (null) in P1.
        let type_args: Option<&'gc Node<'gc>> = None;

        // Is this a CallExpression? (4065-4074)
        if self.check2(
            TokenKind::no_substitution_template,
            TokenKind::template_head,
        ) {
            // Tagged template — P1.9 deferral.
            self.error_cur(
                "tagged template literals not yet supported (parser phase P1.9)",
            );
            return None;
        }
        if self.check(TokenKind::l_paren) {
            expr = self.parse_call_expression(
                start_loc,
                expr,
                type_args,
                seen_optional_chain,
                optional,
            )?;
        }

        // P6: Flow record expression (4075-4086) — gated, skip.

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
    /// P6/P7: Flow/TS `typeArgs` block is gated and omitted; `type_arguments`
    /// is always `None` in P1.
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

        // P6/P7: typeArgs block (3957-3975) — gated; skip. type_args = None.
        let type_args: Option<&'gc Node<'gc>> = None;

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
    /// Deferrals:
    /// - `rw_super` — P3: emit error and return `None`.
    /// - `rw_import` — P4: emit error and return `None`.
    fn parse_optional_expression_except_new(
        &mut self,
        is_constructor_call: IsConstructorCall,
    ) -> Option<&'gc Node<'gc>> {
        let start_loc = self.cur_start();

        let expr: &'gc Node<'gc> = if self.check(TokenKind::rw_super) {
            // P3: `super.prop` / `super(args)` / `super[expr]`.
            self.error_cur("'super' not yet supported (parser phase P3)");
            return None;
        } else if self.check(TokenKind::rw_import) {
            // P4: `import.meta` and `import(source)`.
            self.error_cur(
                "import expressions not yet supported (parser phase P4)",
            );
            return None;
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
    /// ### Deferrals
    /// - Template literal as tag (tagged template) — P1.9: error + `None`.
    fn parse_optional_expression_except_new_tail(
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
            if new_depth > super::MAX_RECURSION_DEPTH {
                let range = self.cur_range();
                self.error_at(
                    range,
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
                // Template literal branch — P1.9 deferral.
                self.error_cur(
                    "tagged template literals not yet supported (parser phase P1.9)",
                );
                self.recursion_depth.set(saved_depth);
                return None;
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
    fn parse_arguments(
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

                let arg = self.parse_assignment_expression(PARAM_IN)?;

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
    /// P6/P7: Flow/TS `typeArgs` blocks (3744-3777) are gated on parse-Flow /
    /// parse-TS context flags that don't exist yet; omitted.  `type_args` is
    /// always `None` in P1.
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
            let prop_expr = self.parse_expression(PARAM_IN)?;
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
        //
        // In P1, `getParseFlow()` is false, so the condition simplifies to:
        //   checkAndEat(period) || (optional && !check(l_paren))
        let ate_period =
            self.check_and_eat(TokenKind::period, GrammarContext::AllowDiv);
        if ate_period || (optional && !self.check(TokenKind::l_paren)) {
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

        // The only remaining case is `?.(args)` — optional call on `?.`.
        // C++ assert: `optional && check(l_paren)`.
        debug_assert!(optional && self.check(TokenKind::l_paren));

        // P6/P7: typeArgs block (3744-3777) — gated; skip.
        let type_args: Option<&'gc Node<'gc>> = None;

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
    /// `type_args` carries Flow/TS type arguments from the caller; always
    /// `None` in P1. After each `(args)` call the type-args are consumed
    /// (reset to `None`) to allow the next call in the chain to supply its own.
    ///
    /// P6/P7: Flow/TS type-argument parsing (3809-3828) — gated; omitted.
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
            // P6/P7: Flow/TS type-argument block (3809-3828) — gated; skip.

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
                // Tagged template — P1.9 deferral.
                self.error_cur(
                    "tagged template literals not yet supported (parser phase P1.9)",
                );
                return None;
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
