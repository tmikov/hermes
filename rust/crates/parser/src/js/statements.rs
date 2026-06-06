/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Statement parsing for the JS parser. Port of the statement-parsing section
//! of `lib/Parser/JSParserImpl.cpp`.

use ast::node::{
    BreakStatement, ContinueStatement, DebuggerStatement, EmptyStatement,
    ExpressionStatement, Identifier, LabeledStatement, Node, ReturnStatement,
    StringLiteral, ThrowStatement, WithStatement,
};
use ast::node_child::NodeMetadata;
use atom_table::INVALID_ATOM_BYTES;
use support::location::{SMLoc, SMRange};

use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

use super::{AllowImportExport, JSParserImpl, Param, PARAM_IN, PARAM_RETURN};

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    // -----------------------------------------------------------------------
    // parseStatementList — 948 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a statement list until `until` (typically `eof` or `r_brace`).
    ///
    /// If `parse_directives` is true, leading string-literal statements are
    /// treated as directives (e.g., "use strict") before the general loop.
    ///
    /// Port of `JSParserImpl::parseStatementList` (lines 948-971).
    pub(super) fn parse_statement_list(
        &mut self,
        param: Param,
        until: TokenKind,
        parse_directives: bool,
        allow_import_export: AllowImportExport,
        stmt_list: &mut Vec<&'gc Node<'gc>>,
    ) -> bool {
        // Parse leading directive prologue if requested. C++ lines 956-961.
        if parse_directives {
            while self.check(TokenKind::string_literal) {
                match self.parse_directive() {
                    Some(dir_stmt) => stmt_list.push(dir_stmt),
                    None => break, // not a directive (non-simple string)
                }
            }
        }

        // Parse statement-list items until EOF or `until` token. C++ 964-968.
        while !self.check(TokenKind::eof) && !self.check(until) {
            if !self.parse_statement_list_item(param, allow_import_export, stmt_list) {
                return false;
            }
        }

        true
    }

    // -----------------------------------------------------------------------
    // parseStatementListItem — 879 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse one statement-list item (declaration or statement) and push it
    /// onto `stmt_list`. Returns false on unrecoverable error.
    ///
    /// Port of `JSParserImpl::parseStatementListItem` (lines 879-946).
    ///
    /// P1.1 deferral: declarations are not yet supported. The
    /// `checkDeclaration()` path emits an honest error and returns false.
    /// `import` / `export` statements are similarly deferred.
    pub(super) fn parse_statement_list_item(
        &mut self,
        param: Param,
        _allow_import_export: AllowImportExport,
        stmt_list: &mut Vec<&'gc Node<'gc>>,
    ) -> bool {
        // P1.1: checkDeclaration() deferred.
        // The C++ would dispatch to parseDeclaration here if a declaration
        // keyword is present. For P1.1 we check for the most common ones and
        // emit an honest error.
        if self.check_declaration_start() {
            self.error_cur("declarations not yet supported (parser phase P2)");
            return false;
        }

        // P1.1: import / export deferred.
        if self.check(TokenKind::rw_import) || self.check(TokenKind::rw_export) {
            self.error_cur("import/export declarations not yet supported (parser phase P2)");
            return false;
        }

        // Fall through to parseStatement.
        match self.parse_statement(param.get(PARAM_RETURN)) {
            Some(stmt) => {
                stmt_list.push(stmt);
                true
            }
            None => false,
        }
    }

    // -----------------------------------------------------------------------
    // checkDeclarationStart — helper (C++ checkDeclaration is in the header)
    // -----------------------------------------------------------------------

    /// Returns true if the current token begins a declaration that is not yet
    /// supported in P1.1. Used by `parseStatementListItem` to emit an honest
    /// "not yet supported" error instead of silently misparsing.
    ///
    /// Mirrors `JSParserImpl::checkDeclaration()` (JSParserImpl.h:565-645)
    /// without the Flow/TS extensions (those are P3+ anyway).
    fn check_declaration_start(&self) -> bool {
        // rw_function, rw_const, rw_class, at (@decorator)
        if self.check_n4(
            TokenKind::rw_function,
            TokenKind::rw_const,
            TokenKind::rw_class,
            TokenKind::at,
        ) {
            return true;
        }
        // 'let' — only a declaration when followed by a declaration start,
        // or always in strict mode. We approximate: if the current token is
        // the identifier `let`, treat it as a potential declaration and let
        // the error message guide the user. In loose mode this may be a false
        // positive for `let(...)` (extremely rare edge case acceptable for P1.1).
        if self.check(TokenKind::identifier)
            && self
                .lexer
                .get_string_table()
                .bytes(self.lexer.token().get_identifier())
                == b"let"
        {
            // In strict mode always a declaration; in loose mode defer to the
            // lexer's isLetFollowedByDeclStart (faithful C++ path). For P1.1
            // we take the conservative approach: always flag it, since the
            // next phase will implement it properly anyway.
            return true;
        }
        // 'async function' — not yet supported.
        // The async check (checkAsyncFunction) is a lookahead. We conservatively
        // skip it for P1.1; async identifiers will parse as plain identifiers.
        false
    }

    // -----------------------------------------------------------------------
    // parseStatement — 669 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a statement. Port of `JSParserImpl::parseStatement` (lines 669-729).
    ///
    /// P1.1: only `semi` → `parseEmptyStatement` and the `default`
    ///       → `parseExpressionOrLabelledStatement` are implemented.
    ///       Every other keyword case emits an honest "not yet supported" error.
    pub(super) fn parse_statement(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        let _guard = self.check_recursion()?;

        match self.cur_kind() {
            TokenKind::l_brace => {
                self.error_cur("block statements not yet supported (parser phase P2)");
                None
            }
            TokenKind::rw_var => {
                self.error_cur("var statement not yet supported (parser phase P2)");
                None
            }
            TokenKind::semi => self.parse_empty_statement(),
            TokenKind::rw_if => {
                self.error_cur("if statement not yet supported (parser phase P2)");
                None
            }
            TokenKind::rw_while => {
                self.error_cur("while statement not yet supported (parser phase P2)");
                None
            }
            TokenKind::rw_do => {
                self.error_cur("do statement not yet supported (parser phase P2)");
                None
            }
            TokenKind::rw_for => {
                self.error_cur("for statement not yet supported (parser phase P2)");
                None
            }
            TokenKind::rw_continue => self.parse_continue_statement(),
            TokenKind::rw_break => self.parse_break_statement(),
            TokenKind::rw_return => {
                // Return guard. C++ lines 698-701.
                // P-future: context_.allowReturnOutsideFunction().
                const ALLOW_RETURN_OUTSIDE_FUNCTION: bool = false;
                if !param.has(PARAM_RETURN) && !ALLOW_RETURN_OUTSIDE_FUNCTION {
                    // Illegal location for a return statement, but we can keep
                    // parsing.
                    self.error_cur("'return' not in a function");
                }
                self.parse_return_statement()
            }
            TokenKind::rw_with => self.parse_with_statement(param.get(PARAM_RETURN)),
            TokenKind::rw_switch => {
                self.error_cur("switch statement not yet supported (parser phase P2)");
                None
            }
            TokenKind::rw_throw => self.parse_throw_statement(),
            TokenKind::rw_try => {
                self.error_cur("try statement not yet supported (parser phase P2)");
                None
            }
            TokenKind::rw_debugger => self.parse_debugger_statement(),
            // default: parseExpressionOrLabelledStatement
            _ => self.parse_expression_or_labelled_statement(param),
        }
    }

    // -----------------------------------------------------------------------
    // parseEmptyStatement — 1591 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `;` (empty statement). Port of
    /// `JSParserImpl::parseEmptyStatement` (lines 1591-1598).
    fn parse_empty_statement(&mut self) -> Option<&'gc Node<'gc>> {
        assert!(self.check(TokenKind::semi));
        let tok_start = self.lexer.token().start_loc();
        let tok_end = self.lexer.token().end_loc();
        let node = Node::EmptyStatement(EmptyStatement::new(
            NodeMetadata::new(self.dummy_range()),
        ));
        let res = self.set_location(tok_start, tok_end, node);
        self.advance(GrammarContext::AllowRegExp);
        Some(res)
    }

    // -----------------------------------------------------------------------
    // parseExpressionOrLabelledStatement — 1600 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse an expression statement or labelled statement. Port of
    /// `JSParserImpl::parseExpressionOrLabelledStatement` (lines 1600-1677).
    fn parse_expression_or_labelled_statement(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        let starts_with_identifier = self.check(TokenKind::identifier);

        // ES9.0 13.5
        // Lookahead cannot be any of: {, function, async function, class, let [
        // Allow execution to continue because the expression may be parsed,
        // but report an error because it will be ambiguous whether the parse was
        // correct.
        // C++ lines 1609-1615.
        if self.check_n3(
            TokenKind::l_brace,
            TokenKind::rw_function,
            TokenKind::rw_class,
        ) {
            // P3: async function — `(checkUnescaped(asyncIdent_) &&
            // checkAsyncFunction())` is deferred to phase P3.
            // There's no need to stop reporting errors.
            self.error_cur("declaration not allowed as expression statement");
        }

        // `let` disambiguation. C++ lines 1617-1627.
        if self.check_unescaped_name(b"let") {
            let let_loc = self.advance(GrammarContext::AllowRegExp).start;
            if self.check(TokenKind::l_square) {
                // let [
                self.error_at(
                    SMRange {
                        start: let_loc,
                        end: self.lexer.token().end_loc(),
                    },
                    "ambiguous 'let [': either a 'let' binding or a member expression",
                );
            }
            self.lexer.seek(let_loc);
            self.advance(GrammarContext::AllowRegExp);
        }

        let start_loc = self.cur_start();
        let opt_expr = self.parse_expression(PARAM_IN)?;

        // Check whether this is a label. The expression must have started with an
        // identifier, be just an identifier and be followed by ':'.
        // C++ lines 1637-1666.
        let is_identifier = matches!(opt_expr, Node::Identifier(_));
        if starts_with_identifier
            && is_identifier
            && self.check_and_eat(TokenKind::colon, GrammarContext::AllowRegExp)
        {
            let id = opt_expr;

            let body: &'gc Node<'gc> = if self.check(TokenKind::rw_function) {
                // ES9.0 13.13.1
                // It is a Syntax Error if any source text matches this rule.
                // LabelledItem : FunctionDeclaration
                // P3: function declarations as labeled items are not yet
                // supported.
                self.error_cur(
                    "function declaration as labeled statement not yet supported (parser phase P3)",
                );
                return None;
            } else {
                // Statement.
                self.parse_statement(param.get(PARAM_RETURN))?
            };

            let label_start = id.metadata().range.get().start;
            let body_end = body.metadata().range.get().end;
            let node = Node::LabeledStatement(LabeledStatement::new(
                NodeMetadata::new(self.dummy_range()),
                id,
                body,
            ));
            return Some(self.set_location(label_start, body_end, node));
        }

        // Expression statement path. C++ lines 1668-1676.
        if !self.eat_semi(false) {
            return None;
        }

        let end_loc = self.lexer.prev_token_end();
        let node = Node::ExpressionStatement(ExpressionStatement::new(
            NodeMetadata::new(self.dummy_range()),
            opt_expr,
            INVALID_ATOM_BYTES, // directive = null for non-directives
        ));
        Some(self.set_location(start_loc, end_loc, node))
    }

    // -----------------------------------------------------------------------
    // parseDebuggerStatement — 2467 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `debugger` statement. Port of
    /// `JSParserImpl::parseDebuggerStatement` (lines 2467-2479).
    fn parse_debugger_statement(&mut self) -> Option<&'gc Node<'gc>> {
        assert!(self.check(TokenKind::rw_debugger));
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        if !self.eat_semi(false) {
            return None;
        }

        let end_loc = self.lexer.prev_token_end();
        let node = Node::DebuggerStatement(DebuggerStatement::new(
            NodeMetadata::new(self.dummy_range()),
        ));
        Some(self.set_location(start_loc, end_loc, node))
    }

    // -----------------------------------------------------------------------
    // parseThrowStatement — 2342 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `throw` statement. Port of
    /// `JSParserImpl::parseThrowStatement` (lines 2342-2364).
    fn parse_throw_statement(&mut self) -> Option<&'gc Node<'gc>> {
        assert!(self.check(TokenKind::rw_throw));
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        if self.lexer.is_new_line_before_current_token() {
            self.error_cur("'throw' argument must be on the same line");
            // C++ also emits sm_.note(startLoc, "location of the 'throw'");
            // message-note fidelity is a tracked carry-forward.
            return None;
        }

        let argument = self.parse_expression(PARAM_IN)?;

        if !self.eat_semi(false) {
            return None;
        }

        let end_loc = self.lexer.prev_token_end();
        let node = Node::ThrowStatement(ThrowStatement::new(
            NodeMetadata::new(self.dummy_range()),
            argument,
        ));
        Some(self.set_location(start_loc, end_loc, node))
    }

    // -----------------------------------------------------------------------
    // parseReturnStatement — 2160 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `return` statement. Port of
    /// `JSParserImpl::parseReturnStatement` (lines 2160-2181).
    fn parse_return_statement(&mut self) -> Option<&'gc Node<'gc>> {
        assert!(self.check(TokenKind::rw_return));
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        if self.eat_semi(true) {
            let end_loc = self.lexer.prev_token_end();
            let node = Node::ReturnStatement(ReturnStatement::new(
                NodeMetadata::new(self.dummy_range()),
                None,
            ));
            return Some(self.set_location(start_loc, end_loc, node));
        }

        let argument = self.parse_expression(PARAM_IN)?;

        if !self.eat_semi(false) {
            return None;
        }

        let end_loc = self.lexer.prev_token_end();
        let node = Node::ReturnStatement(ReturnStatement::new(
            NodeMetadata::new(self.dummy_range()),
            Some(argument),
        ));
        Some(self.set_location(start_loc, end_loc, node))
    }

    // -----------------------------------------------------------------------
    // parseBreakStatement — 2128 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `break` statement. Port of
    /// `JSParserImpl::parseBreakStatement` (lines 2128-2158).
    fn parse_break_statement(&mut self) -> Option<&'gc Node<'gc>> {
        assert!(self.check(TokenKind::rw_break));
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        if self.eat_semi(true) {
            let end_loc = self.lexer.prev_token_end();
            let node = Node::BreakStatement(BreakStatement::new(
                NodeMetadata::new(self.dummy_range()),
                None,
            ));
            return Some(self.set_location(start_loc, end_loc, node));
        }

        if !self.need(TokenKind::identifier, " after 'break'") {
            return None;
        }
        let id = self.make_label_identifier();
        self.advance(GrammarContext::AllowRegExp);

        if !self.eat_semi(false) {
            return None;
        }

        let end_loc = self.lexer.prev_token_end();
        let node = Node::BreakStatement(BreakStatement::new(
            NodeMetadata::new(self.dummy_range()),
            Some(id),
        ));
        Some(self.set_location(start_loc, end_loc, node))
    }

    // -----------------------------------------------------------------------
    // parseContinueStatement — 2095 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `continue` statement. Port of
    /// `JSParserImpl::parseContinueStatement` (lines 2095-2126).
    fn parse_continue_statement(&mut self) -> Option<&'gc Node<'gc>> {
        assert!(self.check(TokenKind::rw_continue));
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        if self.eat_semi(true) {
            let end_loc = self.lexer.prev_token_end();
            let node = Node::ContinueStatement(ContinueStatement::new(
                NodeMetadata::new(self.dummy_range()),
                None,
            ));
            return Some(self.set_location(start_loc, end_loc, node));
        }

        if !self.need(TokenKind::identifier, " after 'continue'") {
            return None;
        }
        let id = self.make_label_identifier();
        self.advance(GrammarContext::AllowRegExp);

        if !self.eat_semi(false) {
            return None;
        }

        let end_loc = self.lexer.prev_token_end();
        let node = Node::ContinueStatement(ContinueStatement::new(
            NodeMetadata::new(self.dummy_range()),
            Some(id),
        ));
        Some(self.set_location(start_loc, end_loc, node))
    }

    /// Build an `Identifier` node from the current token, used as a `break` /
    /// `continue` label. Mirrors the C++ `setLocation(tok_, tok_,
    /// new IdentifierNode(tok_->getIdentifier(), nullptr, false))` idiom.
    fn make_label_identifier(&self) -> &'gc Node<'gc> {
        let tok_start = self.lexer.token().start_loc();
        let tok_end = self.lexer.token().end_loc();
        let name = self.lexer.token().get_identifier();
        let node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            name,
            None,  // type = null
            false, // optional = false
        ));
        self.set_location(tok_start, tok_end, node)
    }

    // -----------------------------------------------------------------------
    // parseWithStatement — 2183 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `with` statement. Port of
    /// `JSParserImpl::parseWithStatement` (lines 2183-2218).
    fn parse_with_statement(&mut self, param: Param) -> Option<&'gc Node<'gc>> {
        assert!(self.check(TokenKind::rw_with));
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        if !self.eat(
            TokenKind::l_paren,
            GrammarContext::AllowRegExp,
            " after 'with'",
        ) {
            return None;
        }

        let object = self.parse_expression(PARAM_IN)?;

        if !self.eat(
            TokenKind::r_paren,
            GrammarContext::AllowRegExp,
            " after 'with (...'",
        ) {
            return None;
        }

        let body = self.parse_statement(param.get(PARAM_RETURN))?;

        let end = body.metadata().range.get().end;
        let node = Node::WithStatement(WithStatement::new(
            NodeMetadata::new(self.dummy_range()),
            object,
            body,
        ));
        Some(self.set_location(start_loc, end, node))
    }

    // -----------------------------------------------------------------------
    // eatSemi — 323 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Consume a semicolon (or perform Automatic Semicolon Insertion).
    /// Port of `JSParserImpl::eatSemi` (lines 323-338).
    ///
    /// Returns true if a semicolon was consumed or ASI applies:
    ///   - explicit `;`
    ///   - current token is `}` or EOF
    ///   - a newline precedes the current token
    ///
    /// Returns false otherwise. When `optional` is false, an error is also
    /// reported in that case; when `optional` is true no error is reported.
    pub(super) fn eat_semi(&mut self, optional: bool) -> bool {
        if self.check(TokenKind::semi) {
            self.advance(GrammarContext::AllowRegExp);
            return true;
        }
        if self.check(TokenKind::r_brace)
            || self.check(TokenKind::eof)
            || self.lexer.is_new_line_before_current_token()
        {
            return true;
        }
        if !optional {
            self.error_cur("';' expected");
        }
        false
    }

    // -----------------------------------------------------------------------
    // parseDirective — 7469 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Try to parse a directive (a leading string-literal ExpressionStatement).
    /// Returns Some(node) if successful, None if the string is not a
    /// well-formed directive (non-simple string literal at this position).
    ///
    /// Port of `JSParserImpl::parseDirective` (lines 7469-7510).
    ///
    /// Side effects:
    ///   - "use strict" → sets strict mode on the lexer.
    ///   - "use static builtin" → sets `use_static_builtin`.
    pub(super) fn parse_directive(&mut self) -> Option<&'gc Node<'gc>> {
        // Is the current token a directive? (A simple string literal without
        // expressions inside.) Port of `lexer_.isCurrentTokenADirective()`.
        if !self.lexer.is_current_token_a_directive() {
            return None;
        }

        // Allocate a StringLiteralNode for the string expression.
        let str_value = self.lexer.token().get_string_literal();
        let tok_start: SMLoc = self.lexer.token().start_loc();
        let tok_end: SMLoc = self.lexer.token().end_loc();
        let str_node = Node::StringLiteral(StringLiteral::new(
            NodeMetadata::new(self.dummy_range()),
            str_value,
        ));
        let str_lit = self.set_location(tok_start, tok_end, str_node);

        let mut end_loc = tok_end;

        // Determine the raw directive string. C++ lines 7483-7492.
        // If the string has no escapes, the raw directive equals the string
        // literal value (no second uniquing needed). If there are escapes, the
        // raw is the source text of the string minus the enclosing quotes.
        //
        // In the Rust port we do the equivalent: when no escapes, raw ==
        // str_value. When there are escapes, we intern the slice of the source
        // buffer between the quote characters.
        let contains_escapes =
            self.lexer.token().get_string_literal_contains_escapes();
        let raw_directive: atom_table::AtomBytes = if !contains_escapes {
            str_value
        } else {
            // Raw is the source text minus the enclosing quote characters.
            // buf_start is the offset of the first byte of the buffer;
            // tok_start / tok_end are absolute offsets (SMLoc = u32 offset).
            let buf_start = self.lexer.get_buffer_start();
            let buf = self.lexer.buffer_bytes();
            // tok_start and tok_end are absolute offsets; subtract buf_start.
            let start_off = (tok_start.offset - buf_start) as usize;
            let end_off = (tok_end.offset - buf_start) as usize;
            // Slice is +1/-1 to skip the enclosing quote characters.
            let raw_slice = &buf[start_off + 1..end_off - 1];
            self.lexer.get_identifier(raw_slice)
        };

        // Process the directive BEFORE advancing (strictness can affect
        // subsequent token interpretation). C++ lines 7494-7498.
        self.process_directive(raw_directive);

        self.advance(GrammarContext::AllowDiv);

        // Consume the optional semicolon. C++ lines 7502-7503.
        if self.check(TokenKind::semi) {
            end_loc = self.advance(GrammarContext::AllowRegExp).end;
        }

        // Allocate an ExpressionStatementNode with the directive field set.
        // C++ line 7506-7509.
        let expr_stmt = Node::ExpressionStatement(ExpressionStatement::new(
            NodeMetadata::new(self.dummy_range()),
            str_lit,
            raw_directive, // directive field = the raw string atom
        ));
        Some(self.set_location(tok_start, end_loc, expr_stmt))
    }

    // -----------------------------------------------------------------------
    // processDirective — 340 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Process a recognised directive string. Port of
    /// `JSParserImpl::processDirective` (lines 340-346).
    fn process_directive(&mut self, directive: atom_table::AtomBytes) {
        // Compare as slices and capture the booleans first so the immutable
        // borrow of the atom table ends before the following `&mut self` calls.
        let bytes = self.lexer.get_string_table().bytes(directive);
        let is_use_strict = bytes == b"use strict";
        let is_static_builtin = bytes == b"use static builtin";
        if is_use_strict {
            self.lexer.set_strict_mode(true);
        }
        if is_static_builtin {
            self.use_static_builtin = true;
        }
    }
}
