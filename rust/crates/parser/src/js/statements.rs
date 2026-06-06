/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Statement parsing for the JS parser. Port of the statement-parsing section
//! of `lib/Parser/JSParserImpl.cpp`.

use ast::node::{
    ArrayPattern, AssignmentPattern, BreakStatement, ContinueStatement,
    DebuggerStatement, Empty, EmptyStatement, ExpressionStatement, Identifier,
    LabeledStatement, Node, ObjectPattern, Property, RestElement,
    ReturnStatement, StringLiteral, ThrowStatement, VariableDeclaration,
    VariableDeclarator, WithStatement,
};
use ast::node_child::{NodeList, NodeMetadata};
use atom_table::INVALID_ATOM_BYTES;
use support::location::{SMLoc, SMRange};

use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

use super::{AllowImportExport, JSParserImpl, Param, PARAM_IN, PARAM_RETURN};

/// Whether `parseVariableDeclaration`/`parseVariableDeclarationList` may parse
/// a binding *pattern* (`[...]`/`{...}`) as the declaration target. Port of the
/// C++ enum `JSParserImpl::VariableDeclAllowPattern`. `using` declarations pass
/// `No`; everything else defaults to `Yes`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum VariableDeclAllowPattern {
    Yes,
    No,
}

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
    /// `function`/`async function`/`class`/`@decorator` declarations are
    /// dispatched through `parse_declaration` which emits honest P3 errors;
    /// `import`/`export` declarations emit honest P4 errors. The Flow `declare`
    /// branch (890-897) is omitted.
    pub(super) fn parse_statement_list_item(
        &mut self,
        param: Param,
        _allow_import_export: AllowImportExport,
        stmt_list: &mut Vec<&'gc Node<'gc>>,
    ) -> bool {
        if self.check_declaration() {
            // C++ 883-888.
            match self.parse_declaration(Param::default()) {
                Some(decl) => {
                    stmt_list.push(decl);
                }
                None => return false,
            }
        } else if self.check(TokenKind::rw_import) {
            // 'import' can indicate an import declaration, but it's also
            // possible a Statement begins with a call to `import()`, so do a
            // lookahead to see if the next token is '('. It can also be
            // import.meta, so check for '.'. C++ 898-923.
            let opt_next = self.lexer.lookahead1::<false>(None);
            if matches!(
                opt_next,
                Some(TokenKind::l_paren) | Some(TokenKind::period)
            ) {
                // import() / import.meta — parse as a Statement (which will
                // itself emit the appropriate P4 error if unsupported).
                match self.parse_statement(param.get(PARAM_RETURN)) {
                    Some(stmt) => stmt_list.push(stmt),
                    None => return false,
                }
            } else {
                // P4: import declarations are deferred.
                self.error_cur(
                    "import declarations not yet supported (parser phase P4)",
                );
                return false;
            }
        } else if self.check(TokenKind::rw_export) {
            // P4: export declarations are deferred. C++ 924-936.
            self.error_cur(
                "export declarations not yet supported (parser phase P4)",
            );
            return false;
        } else {
            // C++ 937-942.
            match self.parse_statement(param.get(PARAM_RETURN)) {
                Some(stmt) => stmt_list.push(stmt),
                None => return false,
            }
        }

        true
    }

    // -----------------------------------------------------------------------
    // checkDeclaration — JSParserImpl.h:565
    // -----------------------------------------------------------------------

    /// Returns true if the current token begins a declaration. Port of the
    /// header method `JSParserImpl::checkDeclaration()` (JSParserImpl.h:565-645)
    /// without the Flow/TS extension blocks (597-642).
    ///
    /// Needs `&mut self` because the `let`/`using`/`await using` disambiguation
    /// performs a lexer lookahead.
    pub(super) fn check_declaration(&mut self) -> bool {
        // rw_function, rw_const, rw_class, at; or 'async [no LT] function'.
        // C++ 566-573.
        if self.check_n4(
            TokenKind::rw_function,
            TokenKind::rw_const,
            TokenKind::rw_class,
            TokenKind::at,
        ) || (self.check_unescaped_name(b"async") && self.check_async_function())
        {
            return true;
        }

        // 'let' — a declaration when in strict mode, otherwise only when
        // followed by a declaration start ('let Identifier', 'let [', 'let {').
        // In loose mode 'let' can also be an Identifier. C++ 575-586.
        if self.check_unescaped_name(b"let") {
            if self.lexer.is_strict_mode() {
                return true;
            }
            return self.lexer.is_let_followed_by_decl_start();
        }

        // 'using' — a declaration when followed by an identifier on the same
        // line. C++ 588-590.
        if self.check_unescaped_name(b"using") {
            return self.lexer.is_using_followed_by_identifier();
        }

        // 'await using' — only inside an await context. C++ 592-595.
        // The lexer helper takes the interned `using` atom (C++ kw.identUsing).
        let ident_using = self.gc.ctx().atom_table.atom_bytes(b"using");
        if self.param_await
            && self.check_unescaped_name(b"await")
            && self
                .lexer
                .is_await_using_followed_by_identifier(ident_using)
        {
            return true;
        }

        // Flow/TS blocks (597-642) omitted.
        false
    }

    // -----------------------------------------------------------------------
    // checkAsyncFunction — 308 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Check for `async [no LineTerminator here] function`, with the cursor at
    /// `async`. Port of `JSParserImpl::checkAsyncFunction` (lines 308-321).
    ///
    /// Idempotent: it restores the lexer state via `lookahead1`.
    pub(super) fn check_async_function(&mut self) -> bool {
        assert!(
            self.check_unescaped_name(b"async"),
            "check for async function must occur at 'async'"
        );
        // Avoid passing rw_function to lookahead1; parseFunctionHelper relies on
        // seeing `async`. C++ 314-320.
        self.lexer.lookahead1::<false>(None) == Some(TokenKind::rw_function)
    }

    // -----------------------------------------------------------------------
    // parseDeclaration — 815 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a declaration. Port of `JSParserImpl::parseDeclaration`
    /// (lines 815-877). Assumes `check_declaration()` is true.
    ///
    /// `function`/`async function` (820-827) and `@`/`class` (829-835)
    /// declarations emit honest P3 errors; the Flow/TS blocks (857-873) are
    /// omitted.
    pub(super) fn parse_declaration(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        let _guard = self.check_recursion()?;

        assert!(self.check_declaration(), "invalid start for declaration");

        // C++ 820-827.
        if self.check(TokenKind::rw_function)
            || (self.check_unescaped_name(b"async") && self.check_async_function())
        {
            // P3: function declarations are deferred.
            self.error_cur(
                "function declarations not yet supported (parser phase P3)",
            );
            return None;
        }

        // C++ 829-835.
        if self.check2(TokenKind::at, TokenKind::rw_class) {
            // P3: class declarations are deferred.
            self.error_cur(
                "class declarations not yet supported (parser phase P3)",
            );
            return None;
        }

        // C++ 837-843.
        if self.check(TokenKind::rw_const) || self.check_unescaped_name(b"let") {
            return self.parse_lexical_declaration(PARAM_IN);
        }

        // using Identifier / await using Identifier. C++ 845-855.
        if self.check_unescaped_name(b"using")
            || self.check_unescaped_name(b"await")
        {
            return self.parse_using_declaration(param);
        }

        // Flow/TS blocks (857-873) omitted.
        unreachable!("check_declaration() returned true without a declaration");
    }

    // -----------------------------------------------------------------------
    // parseLexicalDeclaration — 1088 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a lexical (`var`/`let`/`const`) declaration. Port of
    /// `JSParserImpl::parseLexicalDeclaration` (lines 1088-1133).
    pub(super) fn parse_lexical_declaration(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        assert!(
            self.check(TokenKind::rw_var)
                || self.check(TokenKind::rw_const)
                || self.check_unescaped_name(b"let"),
            "parseLexicalDeclaration() expects var/const/let"
        );
        // C++ 1094-1095.
        let is_const = self.check(TokenKind::rw_const);
        let kind_ident = self.lexer.token().get_res_word_or_identifier();

        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        // C++ 1099-1101.
        let mut decl_list: Vec<&'gc Node<'gc>> = Vec::new();
        if !self.parse_variable_declaration_list(
            param,
            &mut decl_list,
            start_loc,
            VariableDeclAllowPattern::Yes,
        ) {
            return None;
        }

        if !self.eat_semi(false) {
            return None;
        }

        // C++ 1106-1122: const bindings must have an initializer.
        if is_const {
            for decl in &decl_list {
                let var_decl = decl
                    .as_variable_declarator()
                    .expect("declaration list element is a VariableDeclarator");
                if var_decl.init.is_none() {
                    // ES9.0 13.3.1.1: It is a Syntax Error if Initializer is not
                    // present and IsConstantDeclaration is true. (Not done in the
                    // SemanticValidator because `const` in `for` loops don't need
                    // initializers.)
                    self.error_at(
                        decl.range(),
                        "missing initializer in const declaration",
                    );
                }
            }
        }

        // C++ 1124-1128.
        let end_loc = self.lexer.prev_token_end();
        let node = Node::VariableDeclaration(VariableDeclaration::new(
            NodeMetadata::new(self.dummy_range()),
            kind_ident,
            NodeList::from_iter(self.gc, decl_list),
        ));
        let res = self.set_location(start_loc, end_loc, node);

        // C++ 1130.
        self.ensure_destructuring_initialized(res);

        Some(res)
    }

    // -----------------------------------------------------------------------
    // parseUsingDeclaration — 1135 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `using` / `await using` declaration. Port of
    /// `JSParserImpl::parseUsingDeclaration` (lines 1135-1175).
    pub(super) fn parse_using_declaration(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        assert!(
            self.check_unescaped_name(b"await")
                || self.check_unescaped_name(b"using")
        );

        // Determine if this is 'using' or 'await using'. C++ 1140-1141.
        let is_await_using = self.check_unescaped_name(b"await");
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;
        let mut kind_ident = self.gc.ctx().atom_table.atom_bytes(b"using");

        if is_await_using {
            // await using Identifier
            //       ^
            // C++ 1144-1150.
            assert!(self.check_unescaped_name(b"using"));
            self.advance(GrammarContext::AllowRegExp);
            kind_ident = self.gc.ctx().atom_table.atom_bytes(b"await using");
        }

        // C++ 1152-1155: 'using' declarations may not bind a pattern.
        let mut decl_list: Vec<&'gc Node<'gc>> = Vec::new();
        if !self.parse_variable_declaration_list(
            param,
            &mut decl_list,
            start_loc,
            VariableDeclAllowPattern::No,
        ) {
            return None;
        }

        if !self.eat_semi(false) {
            return None;
        }

        // 'using' declarations require initializers. C++ 1160-1168.
        for decl in &decl_list {
            let var_decl = decl
                .as_variable_declarator()
                .expect("declaration list element is a VariableDeclarator");
            if var_decl.init.is_none() {
                self.error_at(
                    decl.range(),
                    "missing initializer in using declaration",
                );
            }
        }

        // C++ 1170-1174.
        let end_loc = self.lexer.prev_token_end();
        let node = Node::VariableDeclaration(VariableDeclaration::new(
            NodeMetadata::new(self.dummy_range()),
            kind_ident,
            NodeList::from_iter(self.gc, decl_list),
        ));
        Some(self.set_location(start_loc, end_loc, node))
    }

    // -----------------------------------------------------------------------
    // parseVariableStatement — 1177 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `var` statement. Port of `JSParserImpl::parseVariableStatement`
    /// (lines 1177-1180). (A `var` statement is a lexical declaration with
    /// `[In]` always set.)
    pub(super) fn parse_variable_statement(
        &mut self,
        _param: Param,
    ) -> Option<&'gc Node<'gc>> {
        self.parse_lexical_declaration(PARAM_IN)
    }

    // -----------------------------------------------------------------------
    // parseVariableDeclarationList — 1197 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a comma-separated list of variable declarators into `decl_list`.
    /// Port of `JSParserImpl::parseVariableDeclarationList` (lines 1197-1210).
    /// Returns false on an unrecoverable error.
    pub(super) fn parse_variable_declaration_list(
        &mut self,
        param: Param,
        decl_list: &mut Vec<&'gc Node<'gc>>,
        decl_loc: SMLoc,
        allow_pattern: VariableDeclAllowPattern,
    ) -> bool {
        // do { ... } while (checkAndEat(comma)). C++ 1202-1207.
        loop {
            match self.parse_variable_declaration(param, decl_loc, allow_pattern) {
                Some(decl) => decl_list.push(decl),
                None => return false,
            }
            if !self.check_and_eat(TokenKind::comma, GrammarContext::AllowRegExp)
            {
                break;
            }
        }
        true
    }

    // -----------------------------------------------------------------------
    // ensureDestructuringInitialized — 1212 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Error if any declarator whose target is a destructuring pattern lacks an
    /// initializer. Port of `JSParserImpl::ensureDestructuringInitialized`
    /// (lines 1212-1224). (The "destucturing" typo is faithful to C++.)
    fn ensure_destructuring_initialized(
        &mut self,
        decl_node: &'gc Node<'gc>,
    ) {
        let var_decl = decl_node
            .as_variable_declaration()
            .expect("ensure_destructuring_initialized expects VariableDeclaration");
        for elem in var_decl.declarations.iter() {
            let declarator = elem
                .as_variable_declarator()
                .expect("declaration list element is a VariableDeclarator");

            // isa<PatternNode>(_id) — ArrayPattern/ObjectPattern.
            let is_pattern = matches!(
                declarator.id,
                Node::ArrayPattern(_) | Node::ObjectPattern(_)
            );
            if !is_pattern || declarator.init.is_some() {
                continue;
            }

            self.error_at(
                declarator.id.range(),
                "destucturing declaration must be initialized",
            );
        }
    }

    // -----------------------------------------------------------------------
    // parseVariableDeclaration — 1226 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a single `VariableDeclarator` (a binding target with an optional
    /// initializer). Port of `JSParserImpl::parseVariableDeclaration`
    /// (lines 1226-1279).
    pub(super) fn parse_variable_declaration(
        &mut self,
        param: Param,
        decl_loc: SMLoc,
        allow_pattern: VariableDeclAllowPattern,
    ) -> Option<&'gc Node<'gc>> {
        let start_loc = self.lexer.token().start_loc();

        // C++ 1234-1253.
        let target: &'gc Node<'gc> = if allow_pattern
            == VariableDeclAllowPattern::Yes
            && self.check2(TokenKind::l_square, TokenKind::l_brace)
        {
            self.parse_binding_pattern(param)?
        } else {
            match self.parse_binding_identifier(Param::default()) {
                Some(ident) => ident,
                None => {
                    // C++ errorExpected(identifier, "in declaration",
                    // "declaration started here", declLoc). The note arg is
                    // dropped per house style; report at the declaration start.
                    let _ = decl_loc;
                    self.error_cur("'identifier' expected in declaration");
                    return None;
                }
            }
        };

        // No initializer? C++ 1255-1261.
        if !self.check(TokenKind::equal) {
            let end_loc = self.lexer.prev_token_end();
            let node = Node::VariableDeclarator(VariableDeclarator::new(
                NodeMetadata::new(self.dummy_range()),
                None,
                target,
            ));
            return Some(self.set_location(start_loc, end_loc, node));
        }

        // Parse the initializer. C++ 1263-1278.
        let debug_loc = self.advance(GrammarContext::AllowRegExp).start;

        // C++ passes AllowTypedArrowFunction::Yes / CoverTypedParameters::No;
        // P1's parse_assignment_expression takes only `param`.
        let expr = self.parse_assignment_expression(param)?;

        let end_loc = self.lexer.prev_token_end();
        let node = Node::VariableDeclarator(VariableDeclarator::new(
            NodeMetadata::new(self.dummy_range()),
            Some(expr),
            target,
        ));
        Some(self.set_location_d(start_loc, end_loc, debug_loc, node))
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
                // C++ 678-684: parseVariableStatement(Param{}).
                self.parse_variable_statement(Param::default())
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

    // -----------------------------------------------------------------------
    // parseBindingIdentifier — 1047 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `BindingIdentifier`. Port of
    /// `JSParserImpl::parseBindingIdentifier` (lines 1047-1086).
    ///
    /// Returns `None` if the current token is neither an identifier nor a
    /// reserved word, or if `validate_binding_identifier` rejects the kind.
    ///
    /// The `param` argument mirrors the C++ signature but, as in C++, it is not
    /// directly consumed here — `validate_binding_identifier` reads the parser's
    /// `param_yield`/`param_await`/strict-mode state. The Flow/TS
    /// `getParseTypes()` block (`?`/`:` type annotation) is skipped (P6/P7);
    /// `type` is `None` and `optional` is `false`.
    pub(super) fn parse_binding_identifier(
        &mut self,
        _param: Param,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 1049-1051.
        if !self.check(TokenKind::identifier)
            && !self.lexer.token().is_res_word()
        {
            return None;
        }
        let ident_rng: SMRange = self.lexer.token().source_range();

        // If we have an identifier or reserved word, then store it and the
        // kind, and pass it to the validateBindingIdentifier function.
        // C++ 1056-1060.
        let id = self.lexer.token().get_res_word_or_identifier();
        let kind = self.lexer.token().kind();
        // validateBindingIdentifier compares the *passed-in* `id` atom; resolve
        // its interned bytes to an owned buffer so the immutable borrow of the
        // atom table ends before the `&mut self` validate call.
        let id_bytes = self.gc.ctx().atom_table.bytes(id).to_owned();
        if !self.validate_binding_identifier(ident_rng, &id_bytes, kind) {
            return None;
        }
        self.advance(GrammarContext::AllowRegExp);

        // P6/P7: context_.getParseTypes() `?`/`:` block skipped. type = None,
        // optional = false. C++ 1063-1080.

        // C++ 1082-1085.
        let node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            None,  // type = null
            false, // optional = false
        ));
        Some(self.set_location(ident_rng.start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseBindingPattern — 1281 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `BindingPattern` (`[...]` array or `{...}` object). Port of
    /// `JSParserImpl::parseBindingPattern` (lines 1281-1296).
    pub(super) fn parse_binding_pattern(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        assert!(
            self.check(TokenKind::l_square) || self.check(TokenKind::l_brace),
            "BindingPattern expects '{{' or '['"
        );
        if self.check(TokenKind::l_square) {
            self.parse_array_binding_pattern(param)
        } else {
            self.parse_object_binding_pattern(param)
        }
    }

    // -----------------------------------------------------------------------
    // parseArrayBindingPattern — 1298 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse an `ArrayBindingPattern`. Port of
    /// `JSParserImpl::parseArrayBindingPattern` (lines 1298-1360).
    fn parse_array_binding_pattern(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        assert!(self.check(TokenKind::l_square), "ArrayBindingPattern expects '['");

        // Eat the '[', recording the start location. C++ 1303.
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        let mut elem_list: Vec<&'gc Node<'gc>> = Vec::new();

        if !self.check(TokenKind::r_square) {
            loop {
                if self.check(TokenKind::comma) {
                    // Elision. C++ 1310-1312.
                    let tok_start = self.lexer.token().start_loc();
                    let tok_end = self.lexer.token().end_loc();
                    let empty = Node::Empty(Empty::new(
                        NodeMetadata::new(self.dummy_range()),
                    ));
                    elem_list.push(self.set_location(tok_start, tok_end, empty));
                } else if self.check(TokenKind::dotdotdot) {
                    // BindingRestElement. C++ 1313-1319.
                    let rest_elem = self.parse_binding_rest_element(param)?;
                    elem_list.push(rest_elem);
                    break;
                } else {
                    // BindingElement. C++ 1320-1326.
                    let elem = self.parse_binding_element(param)?;
                    elem_list.push(elem);
                }

                if !self.check_and_eat(TokenKind::comma, GrammarContext::AllowRegExp) {
                    break;
                }
                if self.check(TokenKind::r_square) {
                    // Check for ",]".
                    break;
                }
            }
        }

        // C++ 1335-1341. Closing eat uses AllowDiv.
        if !self.eat(
            TokenKind::r_square,
            GrammarContext::AllowDiv,
            " at end of array binding pattern '[...'",
        ) {
            return None;
        }

        // P6/P7: context_.getParseTypes() type block skipped. type = None.

        // C++ 1356-1359.
        let node = Node::ArrayPattern(ArrayPattern::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, elem_list),
            None, // type = null
        ));
        Some(self.set_location(start_loc, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseBindingElement — 1362 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `BindingElement` (a binding target with optional initializer).
    /// Port of `JSParserImpl::parseBindingElement` (lines 1362-1390).
    fn parse_binding_element(&mut self, param: Param) -> Option<&'gc Node<'gc>> {
        let _guard = self.check_recursion()?;

        // C++ 1366-1380.
        let elem: &'gc Node<'gc> =
            if self.check(TokenKind::l_square) || self.check(TokenKind::l_brace) {
                self.parse_binding_pattern(param)?
            } else {
                match self.parse_binding_identifier(param) {
                    Some(ident) => ident,
                    None => {
                        self.error_cur(
                            "identifier, '{' or '[' expected in binding pattern",
                        );
                        return None;
                    }
                }
            };

        // No initializer? C++ 1382-1384.
        if !self.check(TokenKind::equal) {
            return Some(elem);
        }

        // C++ 1386-1389.
        self.parse_binding_initializer(param, elem)
    }

    // -----------------------------------------------------------------------
    // parseBindingRestElement — 1392 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `BindingRestElement` (`...target`). Port of
    /// `JSParserImpl::parseBindingRestElement` (lines 1392-1413).
    fn parse_binding_rest_element(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        assert!(
            self.check(TokenKind::dotdotdot),
            "BindingRestElement expected to start with '...'"
        );

        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        let elem = self.parse_binding_element(param)?;
        // A rest element may not have a default initializer. C++ 1402-1407.
        // NOTE: the C++ error message has the typo "elemenent"; preserved here.
        if matches!(elem, Node::AssignmentPattern(_)) {
            let range = elem.range();
            self.error_at(range, "rest elemenent may not have a default initializer");
            return None;
        }

        // C++ 1409-1412.
        let node = Node::RestElement(RestElement::new(
            NodeMetadata::new(self.dummy_range()),
            elem,
        ));
        Some(self.set_location(start_loc, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseBindingInitializer — 1415 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a binding `Initializer` (`= AssignmentExpression`) and wrap the
    /// already-parsed `left` target in an `AssignmentPattern`. Port of
    /// `JSParserImpl::parseBindingInitializer` (lines 1415-1432).
    fn parse_binding_initializer(
        &mut self,
        param: Param,
        left: &'gc Node<'gc>,
    ) -> Option<&'gc Node<'gc>> {
        assert!(self.check(TokenKind::equal), "binding initializer requires '='");

        // Parse the initializer. C++ 1421.
        let debug_loc = self.advance(GrammarContext::AllowRegExp).start;

        let expr = self.parse_assignment_expression(PARAM_IN.plus(param))?;

        // C++ 1427-1431.
        let left_start = left.range().start;
        let node = Node::AssignmentPattern(AssignmentPattern::new(
            NodeMetadata::new(self.dummy_range()),
            left,
            expr,
        ));
        Some(self.set_location_d(
            left_start,
            self.lexer.prev_token_end(),
            debug_loc,
            node,
        ))
    }

    // -----------------------------------------------------------------------
    // parseObjectBindingPattern — 1434 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse an `ObjectBindingPattern`. Port of
    /// `JSParserImpl::parseObjectBindingPattern` (lines 1434-1491).
    fn parse_object_binding_pattern(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        assert!(self.check(TokenKind::l_brace), "ObjectBindingPattern expects '{{'");

        // Eat the '{', recording the start location. C++ 1439.
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        let mut prop_list: Vec<&'gc Node<'gc>> = Vec::new();

        if !self.check(TokenKind::r_brace) {
            loop {
                if self.check(TokenKind::dotdotdot) {
                    // BindingRestProperty. C++ 1445-1451.
                    let rest_elem = self.parse_binding_rest_property(param)?;
                    prop_list.push(rest_elem);
                    break;
                }
                let prop = self.parse_binding_property(param)?;
                prop_list.push(prop);

                if !self.check_and_eat(TokenKind::comma, GrammarContext::AllowRegExp) {
                    break;
                }
                if self.check(TokenKind::r_brace) {
                    // check for ",}"
                    break;
                }
            }
        }

        // C++ 1466-1472. Closing eat uses AllowDiv.
        if !self.eat(
            TokenKind::r_brace,
            GrammarContext::AllowDiv,
            " at end of object binding pattern '{...'",
        ) {
            return None;
        }

        // P6/P7: context_.getParseTypes() type block skipped. type = None.

        // C++ 1487-1490.
        let node = Node::ObjectPattern(ObjectPattern::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, prop_list),
            None, // type = null
        ));
        Some(self.set_location(start_loc, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseBindingProperty — 1493 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `BindingProperty`. Port of
    /// `JSParserImpl::parseBindingProperty` (lines 1493-1561).
    fn parse_binding_property(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 1495-1500.
        let computed = self.check(TokenKind::l_square);
        let start_loc = self.lexer.token().start_loc();
        let key = self.parse_property_name()?;

        let value: &'gc Node<'gc>;
        let shorthand: bool;

        if self.check_and_eat(TokenKind::colon, GrammarContext::AllowRegExp) {
            // PropertyName ":" BindingElement
            //               ^
            // C++ 1505-1511.
            value = self.parse_binding_element(Param::default())?;
            shorthand = false;
        } else {
            // SingleNameBinding :
            //   BindingIdentifier Initializer[opt]
            //                     ^
            // C++ 1512-1553.

            // Must validate BindingIdentifier, because there are certain
            // identifiers which are valid as PropertyName but not as
            // BindingIdentifier. C++ 1517-1528.
            let ident = match key {
                Node::Identifier(id) if !computed => id,
                _ => {
                    self.error_at(
                        SMRange { start: start_loc, end: start_loc },
                        "identifier expected in object binding pattern",
                    );
                    return None;
                }
            };
            let ident_name = ident.name.get();
            let ident_range = key.range();
            let name_bytes =
                self.gc.ctx().atom_table.bytes(ident_name).to_owned();
            if !self.validate_binding_identifier(
                ident_range,
                &name_bytes,
                TokenKind::identifier,
            ) {
                self.error_at(
                    SMRange { start: start_loc, end: start_loc },
                    "identifier expected in object binding pattern",
                );
                return None;
            }

            shorthand = true;

            if self.check(TokenKind::equal) {
                // BindingIdentifier Initializer
                //                   ^
                // Clone the key because parseBindingInitializer will wrap it.
                // C++ 1535-1539.
                let left_node = Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    ident_name,
                    None,
                    false,
                ));
                let left = self.set_location(
                    ident_range.start,
                    ident_range.end,
                    left_node,
                );
                // C++ 1540-1544.
                value = self.parse_binding_initializer(param.plus(PARAM_IN), left)?;
            } else {
                // BindingIdentifier
                //                   ^
                // Shorthand property initialization, clone the key directly.
                // C++ 1548-1552.
                let cloned = Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    ident_name,
                    None,
                    false,
                ));
                value = self.set_location(
                    ident_range.start,
                    ident_range.end,
                    cloned,
                );
            }
        }

        // C++ 1556-1560.
        let init_kind = self.gc.ctx().atom_table.atom_bytes(b"init");
        let node = Node::Property(Property::new(
            NodeMetadata::new(self.dummy_range()),
            key,
            value,
            init_kind,
            computed,
            false, // method
            shorthand,
        ));
        Some(self.set_location(start_loc, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseBindingRestProperty — 1563 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `BindingRestProperty` (`...identifier`). Port of
    /// `JSParserImpl::parseBindingRestProperty` (lines 1563-1589).
    fn parse_binding_rest_property(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        assert!(
            self.check(TokenKind::dotdotdot),
            "BindingRestProperty expected to start with '...'"
        );

        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        // NOTE: the spec says that this cannot be another pattern, even though
        // it would make sense (the `#if 0` parseBindingElement branch is dead).
        // C++ 1571-1577.
        let elem = match self.parse_binding_identifier(param) {
            Some(ident) => ident,
            None => {
                self.error_cur(
                    "identifier expected after '...' in object pattern",
                );
                return None;
            }
        };

        // C++ 1585-1588.
        let node = Node::RestElement(RestElement::new(
            NodeMetadata::new(self.dummy_range()),
            elem,
        ));
        Some(self.set_location(start_loc, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // Test-only wrappers (binding patterns are not reachable from a statement
    // until P2.3; drive the leaves directly for unit tests).
    // -----------------------------------------------------------------------

    /// Test-only entry point: parse a binding pattern starting at the current
    /// token (the constructor already lexed the first token).
    #[cfg(test)]
    pub(crate) fn parse_binding_pattern_for_test(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        self.parse_binding_pattern(Param::default())
    }
}
