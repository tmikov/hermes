/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Statement parsing for the JS parser. Port of the statement-parsing section
//! of `lib/Parser/JSParserImpl.cpp`.

use ast::node::{EmptyStatement, ExpressionStatement, Node, StringLiteral};
use ast::node_child::NodeMetadata;
use atom_table::INVALID_ATOM_BYTES;
use support::location::SMLoc;

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
            TokenKind::rw_continue => {
                self.error_cur("continue statement not yet supported (parser phase P2)");
                None
            }
            TokenKind::rw_break => {
                self.error_cur("break statement not yet supported (parser phase P2)");
                None
            }
            TokenKind::rw_return => {
                self.error_cur("return statement not yet supported (parser phase P2)");
                None
            }
            TokenKind::rw_with => {
                self.error_cur("with statement not yet supported (parser phase P2)");
                None
            }
            TokenKind::rw_switch => {
                self.error_cur("switch statement not yet supported (parser phase P2)");
                None
            }
            TokenKind::rw_throw => {
                self.error_cur("throw statement not yet supported (parser phase P2)");
                None
            }
            TokenKind::rw_try => {
                self.error_cur("try statement not yet supported (parser phase P2)");
                None
            }
            TokenKind::rw_debugger => {
                self.error_cur("debugger statement not yet supported (parser phase P2)");
                None
            }
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
    ///
    /// P1.1: only the expression-statement path is implemented.
    /// The labelled-statement path (identifier followed by `:`) emits an
    /// honest "not yet supported" error.
    fn parse_expression_or_labelled_statement(
        &mut self,
        _param: Param,
    ) -> Option<&'gc Node<'gc>> {
        let starts_with_identifier = self.check(TokenKind::identifier);

        // C++ lines 1607-1615: warn about ambiguous declaration-as-expression.
        // In P1.1 these are simply handled by parse_statement's early returns.

        let start_loc = self.cur_start();
        let opt_expr = self.parse_expression(PARAM_IN)?;

        // Check for labelled statement: `identifier ":"`. C++ lines 1637-1666.
        if starts_with_identifier {
            if let Node::Identifier(_) = opt_expr {
                if self.check(TokenKind::colon) {
                    // P1.1: labelled statements not yet supported.
                    self.error_cur(
                        "labelled statements not yet supported (parser phase P2)",
                    );
                    return None;
                }
            }
        }

        // Expression statement path. C++ lines 1668-1676.
        if !self.eat_semi() {
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
    /// Returns false and reports an error otherwise.
    pub(super) fn eat_semi(&mut self) -> bool {
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
        self.error_cur("';' expected");
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
