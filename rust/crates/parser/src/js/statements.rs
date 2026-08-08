/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Statement parsing for the JS parser. Port of the statement-parsing section
//! of `lib/Parser/JSParserImpl.cpp`.

use ast::node::{
    ArrayPattern, AssignmentPattern, BlockStatement, BreakStatement,
    CatchClause, ContinueStatement, DebuggerStatement, DoWhileStatement, Empty,
    EmptyStatement, ExpressionStatement, ForInStatement, ForOfStatement,
    ForStatement, Identifier, IfStatement, LabeledStatement, Node,
    ObjectPattern, Property, RestElement, ReturnStatement, StringLiteral,
    SwitchCase, SwitchStatement, ThrowStatement, TryStatement,
    VariableDeclaration, VariableDeclarator, WhileStatement, WithStatement,
};
use ast::node_child::{NodeList, NodeMetadata};
use atom_table::INVALID_ATOM_BYTES;
use support::location::{SMLoc, SMRange};

use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

use super::flow::{AllowAnonFunctionType, AllowTypedArrowFunction, CoverTypedParameters};
use super::{
    AllowImportExport, IsClassHeritageArgument, JSParserImpl, Param, PARAM_IN,
    PARAM_RETURN,
};

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

    /// Parse a statement list until any of the `until` tokens (typically `eof`,
    /// `r_brace`, or the switch-clause set `default`/`case`/`r_brace`).
    ///
    /// If `parse_directives` is true, leading string-literal statements are
    /// treated as directives (e.g., "use strict") before the general loop.
    ///
    /// Port of `JSParserImpl::parseStatementList` (lines 948-971). The C++ is a
    /// variadic template `parseStatementList(param, TokenKind until, ...,
    /// Tail... otherUntil)` whose loop condition is
    /// `!check(eof) && !checkN(until, otherUntil...)`. There are exactly two
    /// arities: 1 until (block/program) and 3 untils (switch). We preserve that
    /// monomorphization by making the `until` set a const-generic
    /// `[TokenKind; N]` array (not a runtime slice), so each arity is a separate
    /// instantiation exactly as in C++.
    pub(super) fn parse_statement_list<const N: usize>(
        &mut self,
        param: Param,
        until: [TokenKind; N],
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

        // Parse statement-list items until EOF or any `until` token. C++ 964-968:
        // `!check(eof) && !checkN(until, otherUntil...)`.
        while !self.check(TokenKind::eof) && !until.contains(&self.cur_kind()) {
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
    /// branch (`checkDeclareType` + `parseDeclareFLow`, 890-897) is P6.
    pub(super) fn parse_statement_list_item(
        &mut self,
        param: Param,
        allow_import_export: AllowImportExport,
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
        } else if self.parse_flow() && self.check_declare_type() {
            // C++ 889-897: declare var/function/interface/etc. `declare` is a
            // contextual ident, lookahead-gated by `check_declare_type`.
            let start = self.advance(GrammarContext::Type).start;
            match self.parse_declare_flow(start) {
                Some(decl) => stmt_list.push(decl),
                None => return false,
            }
        } else if self.check(TokenKind::rw_import) {
            // 'import' can indicate an import declaration, but it's also
            // possible a Statement begins with a call to `import()`, so do a
            // lookahead to see if the next token is '('. It can also be
            // import.meta, so check for '.'. C++ 898-923.
            let opt_next = self.lexer.lookahead1::<true>(None);
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
                // import declaration. C++ 911-922. Note that C++ ALWAYS pushes
                // the declaration onto the list (even when it is disallowed
                // here), then reports the "must be at top level" error.
                let import_decl = match self.parse_import_declaration() {
                    Some(d) => d,
                    None => return false,
                };
                let range = import_decl.range();
                stmt_list.push(import_decl);
                if allow_import_export == AllowImportExport::No {
                    self.error_at(
                        range,
                        "import declaration must be at top level of module",
                    );
                }
            }
        } else if self.check(TokenKind::rw_export) {
            // export declaration. C++ 924-936. NOTE the asymmetry vs import:
            // import ALWAYS pushes (then reports the error); export pushes ONLY
            // when allowed here, otherwise it just reports the error.
            let export_decl = match self.parse_export_declaration() {
                Some(d) => d,
                None => return false,
            };
            if allow_import_export == AllowImportExport::Yes {
                stmt_list.push(export_decl);
            } else {
                self.error_at(
                    export_decl.range(),
                    "export declaration must be at top level of module",
                );
            }
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
    // checkDeclareType — JSParserImpl.h:647
    // -----------------------------------------------------------------------

    /// Whether the current `declare` ident begins a `declare` statement (rather
    /// than being a plain identifier). Port of `JSParserImpl::checkDeclareType`
    /// (JSParserImpl.h:647-661). `declare` is a contextual ident
    /// (escape-insensitive → check_name) and the lookahead uses
    /// `RequireNoNewLine = true` (JSLexer.h default; → `lookahead1::<true>`).
    pub(super) fn check_declare_type(&mut self) -> bool {
        // C++ 649.
        if !self.check_name(b"declare") {
            return false;
        }
        // C++ 650-657.
        matches!(
            self.lexer.lookahead1::<true>(None),
            Some(
                TokenKind::identifier
                    | TokenKind::rw_interface
                    | TokenKind::rw_var
                    | TokenKind::rw_const
                    | TokenKind::rw_function
                    | TokenKind::rw_class
                    | TokenKind::rw_export
                    | TokenKind::rw_enum
            )
        )
    }

    // -----------------------------------------------------------------------
    // checkDeclaration — JSParserImpl.h:565
    // -----------------------------------------------------------------------

    /// Returns true if the current token begins a declaration. Port of the
    /// header method `JSParserImpl::checkDeclaration()` (JSParserImpl.h:565-645)
    /// including the Flow extension block (597-627), without the TS block
    /// (629-642).
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
        if self.param_await.get()
            && self.check_unescaped_name(b"await")
            && self
                .lexer
                .is_await_using_followed_by_identifier(ident_using)
        {
            return true;
        }

        // Flow declarations, gated on getParseFlow(). C++ 597-627.
        if self.parse_flow() {
            // C++ 599-608: component/hook declarations (gated on
            // getParseFlowComponentSyntax()).
            if self.parse_flow_component_syntax()
                && (self.check_component_declaration_flow()
                    || (self.check_unescaped_name(b"async")
                        && self.check_async_component_flow()))
            {
                return true;
            }
            if self.parse_flow_component_syntax()
                && (self.check_hook_declaration_flow()
                    || (self.check_unescaped_name(b"async")
                        && self.check_async_hook_flow()))
            {
                return true;
            }
            //
            // The record check (`checkRecordDeclarationFlow()`, C++ 609-611)
            // is GATED on getParseFlowRecords() here — a deliberate deviation.
            // The C++ checkDeclaration() record check is UNgated. Consequence
            // in C++ with records DISABLED: on `record R {}` checkDeclaration()
            // answers true, but parseFlowDeclaration matches nothing (its own
            // record arm IS gated, flow.cpp:47) and silently returns None
            // (flow.cpp:89-92 — the `kind == None` assert passes), so
            // `hermesc -parse-flow` exits 2 with ZERO diagnostics. With the
            // Rust gate, records-disabled input takes the ordinary
            // expression-statement path and reports one normal syntax error —
            // a deliberate, better-behaved deviation. With records ENABLED the
            // gate is transparent and behaves exactly like the C++ (P6.4).
            if self.parse_flow_records()
                && self.check_record_declaration_flow()
            {
                return true;
            }

            // `opaque` followed by an identifier (`type`). C++ 612-615.
            // The C++ `check(<ident>)` overload is escape-insensitive.
            if self.check_name(b"opaque") {
                let opt_next = self.lexer.lookahead1::<true>(None);
                return opt_next == Some(TokenKind::identifier);
            }
            // `type`/`interface` followed by an identifier. C++ 616-619.
            if self.check_name(b"type") || self.check_name(b"interface") {
                let opt_next = self.lexer.lookahead1::<true>(None);
                return opt_next == Some(TokenKind::identifier);
            }
            // C++ 620-622.
            if self.check(TokenKind::rw_interface) {
                return true;
            }
            // C++ 623-625.
            if self.check(TokenKind::rw_enum) {
                return true;
            }
        }

        // TS declarations, gated on getParseTS(). C++ 629-641.
        if self.parse_ts() {
            // `type`/`interface`/`namespace` followed by an identifier. C++
            // 631-634. The C++ `check(<ident>)` overload is escape-insensitive,
            // so each disjunct uses `check_name`.
            if self.check_name(b"type")
                || self.check_name(b"interface")
                || self.check_name(b"namespace")
            {
                let opt_next = self.lexer.lookahead1::<true>(None);
                return opt_next == Some(TokenKind::identifier);
            }
            // C++ 635-637.
            if self.check(TokenKind::rw_interface) {
                return true;
            }
            // C++ 638-640.
            if self.check(TokenKind::rw_enum) {
                return true;
            }
        }

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
        self.lexer.lookahead1::<true>(None) == Some(TokenKind::rw_function)
    }

    // -----------------------------------------------------------------------
    // parseDeclaration — 815 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a declaration. Port of `JSParserImpl::parseDeclaration`
    /// (lines 815-877). Assumes `check_declaration()` is true.
    ///
    /// The Flow block (857-863) dispatches to `parse_flow_declaration` when
    /// Flow parsing is enabled; the TS block (866-872) is omitted (P7).
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
            return self.parse_function_declaration(Param::default(), false);
        }

        // C++ 829-835.
        if self.check2(TokenKind::at, TokenKind::rw_class) {
            return self.parse_class_declaration(param);
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

        // Flow declarations. C++ 857-863. Binary like the C++: `None` means
        // an error was already reported (no fall-through — when
        // `check_declaration()` is true and no earlier arm matched, the
        // declaration must be a Flow declaration).
        if self.parse_flow() {
            return self.parse_flow_declaration();
        }

        // TS declarations. C++ 866-872. Binary like the C++: `None` means an
        // error was already reported (no fall-through — when
        // `check_declaration()` is true and no earlier arm matched, the
        // declaration must be a TS declaration).
        if self.parse_ts() {
            return self.parse_ts_declaration();
        }

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
                    // C++ 1244-1250: errorExpected(identifier, "in
                    // declaration", "declaration started here", declLoc).
                    // `declLoc` is real, so route through `error_expected_msg`
                    // for the same-line combined-range caret; on the
                    // different-line arm "declaration started here" surfaces
                    // as a note at `declLoc`.
                    self.error_expected_msg(
                        "'identifier' expected in declaration",
                        Some("declaration started here"),
                        Some(decl_loc),
                    );
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

        // C++ 1266-1270: parseAssignmentExpression(param, /* eagerly */ false,
        // AllowTypedArrowFunction::Yes, CoverTypedParameters::No).
        let expr = self.parse_assignment_expression(param, false, AllowTypedArrowFunction::Yes, CoverTypedParameters::No, None)?;

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
                // C++ 679-680: parseBlock(param) with the default args
                // grammarContext=AllowRegExp, parseDirectives=false.
                self.parse_block(param, GrammarContext::AllowRegExp, false)
            }
            TokenKind::rw_var => {
                // C++ 678-684: parseVariableStatement(Param{}).
                self.parse_variable_statement(Param::default())
            }
            TokenKind::semi => self.parse_empty_statement(),
            TokenKind::rw_if => {
                // C++ 685-686.
                self.parse_if_statement(param.get(PARAM_RETURN))
            }
            TokenKind::rw_while => {
                // C++ 687-688.
                self.parse_while_statement(param.get(PARAM_RETURN))
            }
            TokenKind::rw_do => {
                // C++ 689-690.
                self.parse_do_while_statement(param.get(PARAM_RETURN))
            }
            TokenKind::rw_for => {
                // C++ 691-692.
                self.parse_for_statement(param.get(PARAM_RETURN))
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
                // C++ 705-706.
                self.parse_switch_statement(param.get(PARAM_RETURN))
            }
            TokenKind::rw_throw => self.parse_throw_statement(),
            TokenKind::rw_try => {
                // C++ 709-710.
                self.parse_try_statement(param.get(PARAM_RETURN))
            }
            TokenKind::rw_debugger => self.parse_debugger_statement(),
            // default. C++ 714-725.
            _ => {
                // Flow match statement. C++ JSParserImpl.cpp:715-723.
                if self.parse_flow()
                    && self.parse_flow_match()
                    && self.check_maybe_flow_match()
                {
                    // Tri-state: None → hard error (propagate); Some(Some(n)) →
                    // a match statement; Some(None) → not a match, fall through
                    // to an expression-statement.
                    if let Some(node) =
                        self.try_parse_match_statement_flow(param.get(PARAM_RETURN))?
                    {
                        return Some(node);
                    }
                }
                self.parse_expression_or_labelled_statement(param.get(PARAM_RETURN))
            }
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
        // C++ lines 1609-1615. `check_async_function` asserts the current
        // token is `async`, so it is guarded by the `&&` short-circuit.
        let is_async_function =
            self.check_unescaped_name(b"async") && self.check_async_function();
        if self.check_n3(
            TokenKind::l_brace,
            TokenKind::rw_function,
            TokenKind::rw_class,
        ) || is_async_function
        {
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
        // C++ 1630: parseExpression(ParamIn, CoverTypedParameters::No) — a bare
        // `ident:` here is a labelled statement, NOT a cover type annotation.
        let opt_expr = self.parse_expression(PARAM_IN, CoverTypedParameters::No)?;

        // Check whether this is a label. The expression must have started with an
        // identifier, be just an identifier and be followed by ':'.
        // C++ lines 1637-1666.
        let is_identifier = matches!(opt_expr, Node::Identifier(_));
        if starts_with_identifier
            && is_identifier
            && self.check_and_eat(TokenKind::colon, GrammarContext::AllowRegExp)
        {
            let id = opt_expr;

            // C++ 1641-1663.
            let body: &'gc Node<'gc> = if self.check(TokenKind::rw_function) {
                let func = self.parse_function_declaration(param, false)?;
                // ES9.0 13.13.1
                // It is a Syntax Error if any source text matches this rule.
                // LabelledItem : FunctionDeclaration
                // NOTE: GeneratorDeclarations are disallowed as part of the
                // grammar as well, so all FunctionDeclarations are disallowed as
                // labeled items, except via an AnnexB extension which is
                // unsupported in Hermes.
                // Point location, NOT `func`'s range: C++ (cpp:1653-1655)
                // calls `error(optFunc.getValue()->getSourceRange().Start,
                // ...)` — the `error(SMLoc, Twine)` overload — so the caret
                // is bare, not an underline over the whole declaration.
                self.error_at_loc(
                    func.range().start,
                    "Function declaration not allowed as body of labeled statement",
                );
                func
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

        let argument = self.parse_expression(PARAM_IN, CoverTypedParameters::Yes)?;

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

        let argument = self.parse_expression(PARAM_IN, CoverTypedParameters::Yes)?;

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

        // C++ 2138-2143: need(identifier, "after 'break'", "location of
        // 'break'", startLoc).
        if !self.need_at(
            TokenKind::identifier,
            " after 'break'",
            Some("location of 'break'"),
            start_loc,
        ) {
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

        // C++ 2106-2111: need(identifier, "after 'continue'", "location of
        // 'continue'", startLoc).
        if !self.need_at(
            TokenKind::identifier,
            " after 'continue'",
            Some("location of 'continue'"),
            start_loc,
        ) {
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

        // C++ 2188: SMLoc lparenLoc = tok_->getStartLoc(), captured before
        // the '(' eat below and reused by the ')' eat's whatLoc.
        let lparen_loc = self.cur_start();
        // C++ 2189-2195: eat(l_paren, "after 'with'", "location of 'with'",
        // startLoc).
        if !self.eat_at(
            TokenKind::l_paren,
            GrammarContext::AllowRegExp,
            " after 'with'",
            Some("location of 'with'"),
            start_loc,
        ) {
            return None;
        }

        let object = self.parse_expression(PARAM_IN, CoverTypedParameters::Yes)?;

        // C++ 2201-2207: eat(r_paren, "after 'with (...'", "location of
        // '('", lparenLoc).
        if !self.eat_at(
            TokenKind::r_paren,
            GrammarContext::AllowRegExp,
            " after 'with (...'",
            Some("location of '('"),
            lparen_loc,
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
    // parseBlock — 973 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `{ StatementList }` block. Port of `JSParserImpl::parseBlock`
    /// (lines 973-1006).
    ///
    /// C++ has default arguments `grammarContext = AllowRegExp`,
    /// `parseDirectives = false`; we make them explicit parameters and the
    /// callers pass the C++ defaults.
    pub(super) fn parse_block(
        &mut self,
        param: Param,
        grammar_context: GrammarContext,
        parse_directives: bool,
    ) -> Option<&'gc Node<'gc>> {
        // {
        assert!(self.check(TokenKind::l_brace));
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        let mut stmt_list: Vec<&'gc Node<'gc>> = Vec::new();

        // C++ 983-990.
        if !self.parse_statement_list(
            param,
            [TokenKind::r_brace],
            parse_directives,
            AllowImportExport::No,
            &mut stmt_list,
        ) {
            return None;
        }

        // }
        // C++ 993-996: BlockStatementNode(body, /*implicit*/ false). The end
        // location is the current token (the `}`), matching C++ `setLocation(
        // startLoc, tok_, ...)`.
        let end_loc = self.lexer.token().end_loc();
        let node = Node::BlockStatement(BlockStatement::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, stmt_list),
            false, // implicit
        ));
        let body = self.set_location(start_loc, end_loc, node);

        // C++ 997-1003: eat(r_brace, grammarContext, "at end of block",
        // "block starts here", startLoc).
        if !self.eat_at(
            TokenKind::r_brace,
            grammar_context,
            " at end of block",
            Some("block starts here"),
            start_loc,
        ) {
            return None;
        }

        Some(body)
    }

    // -----------------------------------------------------------------------
    // parseIfStatement — 1679 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse an `if ( Expr ) Stmt [else Stmt]` statement. Port of
    /// `JSParserImpl::parseIfStatement` (lines 1679-1762).
    pub(super) fn parse_if_statement(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        assert!(self.check(TokenKind::rw_if));
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        // C++ 1684: SMLoc condLoc = tok_->getStartLoc(), captured before the
        // '(' eat below and reused by the ')' eat's whatLoc.
        let cond_loc = self.cur_start();
        // C++ 1685-1691: eat(l_paren, "after 'if'", "location of 'if'",
        // startLoc).
        if !self.eat_at(
            TokenKind::l_paren,
            GrammarContext::AllowRegExp,
            " after 'if'",
            Some("location of 'if'"),
            start_loc,
        ) {
            return None;
        }
        let test = self.parse_expression(PARAM_IN, CoverTypedParameters::Yes)?;
        // C++ 1695-1701: eat(r_paren, "at end of 'if' condition", "'if'
        // condition starts here", condLoc).
        if !self.eat_at(
            TokenKind::r_paren,
            GrammarContext::AllowRegExp,
            " at end of 'if' condition",
            Some("'if' condition starts here"),
            cond_loc,
        ) {
            return None;
        }

        // C++ 1739-1741.
        let consequent =
            self.parse_statement_or_function_declaration(param)?;

        if self.check_and_eat(TokenKind::rw_else, GrammarContext::AllowRegExp) {
            // C++ 1743-1754.
            let alternate =
                self.parse_statement_or_function_declaration(param)?;
            let end = alternate.range().end;
            let node = Node::IfStatement(IfStatement::new(
                NodeMetadata::new(self.dummy_range()),
                test,
                consequent,
                Some(alternate),
            ));
            Some(self.set_location(start_loc, end, node))
        } else {
            // C++ 1755-1761.
            let end = consequent.range().end;
            let node = Node::IfStatement(IfStatement::new(
                NodeMetadata::new(self.dummy_range()),
                test,
                consequent,
                None,
            ));
            Some(self.set_location(start_loc, end, node))
        }
    }

    /// Parse a statement or (only in loose mode) a function declaration, the
    /// consequent/alternate of an `if`. Port of the C++ lambda
    /// `parseStatementOrFunctionDeclaration` (lines 1709-1737).
    ///
    /// ES2022 B.3.3 allows FunctionDeclaration as consequent and alternate.
    /// These FunctionDeclarations are supposed to be processed precisely as if
    /// they were surrounded by BlockStatement, including function promotion. To
    /// allow this, surround them with a synthetic BlockStatement and set the
    /// 'implicit' flag to true to indicate that it wasn't in the original
    /// source. Port of the C++ lambda `parseStatementOrFunctionDeclaration`
    /// (lines 1709-1737).
    ///
    /// Implemented as a method rather than a closure (as in C++) because a Rust
    /// closure capturing `&mut self` cannot be called while `self` is also
    /// borrowed mutably by the surrounding parse method.
    fn parse_statement_or_function_declaration(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        if self.check(TokenKind::rw_function) {
            // C++ 1711-1732.
            let function = self.parse_function_declaration(Param::default(), false)?;
            let func_decl = function
                .as_function_declaration()
                .expect("parseFunctionDeclaration returns a FunctionDeclaration");
            // Point location, NOT `function`'s range: C++ (cpp:1716-1723)
            // calls `error((*optFunction)->getStartLoc(), ...)` — the
            // `error(SMLoc, Twine)` overload — for both checks, so the caret
            // is bare rather than underlining the whole declaration.
            if self.lexer.is_strict_mode() {
                self.error_at_loc(
                    function.range().start,
                    "In strict mode, functions cannot be declared in if statements",
                );
            }
            if func_decl.generator.get() || func_decl.r#async.get() {
                self.error_at_loc(
                    function.range().start,
                    "Functions in if statements cannot be generator/async",
                );
            }
            let range = function.range();
            let stmts: Vec<&'gc Node<'gc>> = vec![function];
            let node = Node::BlockStatement(BlockStatement::new(
                NodeMetadata::new(self.dummy_range()),
                NodeList::from_iter(self.gc, stmts),
                true, // implicit
            ));
            return Some(self.set_location(range.start, range.end, node));
        }
        self.parse_statement(param.get(PARAM_RETURN))
    }

    // -----------------------------------------------------------------------
    // parseWhileStatement — 1764 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `while ( Expr ) Stmt` statement. Port of
    /// `JSParserImpl::parseWhileStatement` (lines 1764-1796).
    pub(super) fn parse_while_statement(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        assert!(self.check(TokenKind::rw_while));
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        // C++ 1769-1775: eat(l_paren, "after 'while'", "location of
        // 'while'", startLoc).
        if !self.eat_at(
            TokenKind::l_paren,
            GrammarContext::AllowRegExp,
            " after 'while'",
            Some("location of 'while'"),
            start_loc,
        ) {
            return None;
        }
        let test = self.parse_expression(PARAM_IN, CoverTypedParameters::Yes)?;
        // C++ 1779-1785: eat(r_paren, "at end of 'while' condition",
        // "location of 'while'", startLoc).
        if !self.eat_at(
            TokenKind::r_paren,
            GrammarContext::AllowRegExp,
            " at end of 'while' condition",
            Some("location of 'while'"),
            start_loc,
        ) {
            return None;
        }

        let body = self.parse_statement(param.get(PARAM_RETURN))?;

        // C++ 1791-1795: WhileStatementNode(body, test) — body FIRST.
        let end = body.range().end;
        let node = Node::WhileStatement(WhileStatement::new(
            NodeMetadata::new(self.dummy_range()),
            body,
            test,
        ));
        Some(self.set_location(start_loc, end, node))
    }

    // -----------------------------------------------------------------------
    // parseDoWhileStatement — 1798 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `do Stmt while ( Expr ) ;` statement. Port of
    /// `JSParserImpl::parseDoWhileStatement` (lines 1798-1841).
    pub(super) fn parse_do_while_statement(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        assert!(self.check(TokenKind::rw_do));
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        let body = self.parse_statement(param.get(PARAM_RETURN))?;

        // C++ 1807: SMLoc whileLoc = tok_->getStartLoc(), captured before
        // the 'while' eat below and reused by the '(' / ')' eats' whatLoc.
        let while_loc = self.cur_start();
        // C++ 1808-1814: eat(rw_while, "at end of 'do-while'", "'do-while'
        // starts here", startLoc).
        if !self.eat_at(
            TokenKind::rw_while,
            GrammarContext::AllowRegExp,
            " at end of 'do-while'",
            Some("'do-while' starts here"),
            start_loc,
        ) {
            return None;
        }

        // C++ 1816-1822: eat(l_paren, "after 'do-while'", "location of
        // 'while'", whileLoc).
        if !self.eat_at(
            TokenKind::l_paren,
            GrammarContext::AllowRegExp,
            " after 'do-while'",
            Some("location of 'while'"),
            while_loc,
        ) {
            return None;
        }
        let test = self.parse_expression(PARAM_IN, CoverTypedParameters::Yes)?;
        // C++ 1826-1832: eat(r_paren, "at end of 'do-while' condition",
        // "location of 'while'", whileLoc).
        if !self.eat_at(
            TokenKind::r_paren,
            GrammarContext::AllowRegExp,
            " at end of 'do-while' condition",
            Some("location of 'while'"),
            while_loc,
        ) {
            return None;
        }

        // C++ 1834: optional semicolon.
        self.eat_semi(true);

        // C++ 1836-1840: DoWhileStatementNode(body, test) — body FIRST.
        let end_loc = self.lexer.prev_token_end();
        let node = Node::DoWhileStatement(DoWhileStatement::new(
            NodeMetadata::new(self.dummy_range()),
            body,
            test,
        ));
        Some(self.set_location(start_loc, end_loc, node))
    }

    // -----------------------------------------------------------------------
    // parseForStatement — 1843 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `for` statement: C-style `for`, `for-in`, `for-of`, and the
    /// `for await ( ... of ... )` form, with `var`/`let`/`const`/`using`/
    /// `await using` heads and destructuring-pattern reparse on the LHS. Port of
    /// `JSParserImpl::parseForStatement` (lines 1843-2093).
    pub(super) fn parse_for_statement(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 1844-1845.
        assert!(self.check(TokenKind::rw_for));
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        // C++ 1847-1852: `for await` prologue. (Won't fire in the P2 corpus since
        // `param_await` is false at the top level, but ported for fidelity.)
        let mut await_kw = false;
        let mut await_rng = SMRange {
            start: start_loc,
            end: start_loc,
        };
        if self.param_await.get() && self.check_unescaped_name(b"await") {
            await_rng = self.advance(GrammarContext::AllowRegExp);
            await_kw = true;
        }

        // C++ 1854: SMLoc lparenLoc = tok_->getStartLoc(), captured before
        // the '(' eat below and reused by the later `eat` calls' whatLoc.
        let lparen_loc = self.cur_start();
        // C++ 1855-1861: eat(l_paren, "after 'for'", "location of 'for'",
        // startLoc).
        if !self.eat_at(
            TokenKind::l_paren,
            GrammarContext::AllowRegExp,
            " after 'for'",
            Some("location of 'for'"),
            start_loc,
        ) {
            return None;
        }

        // C++ 1863-1864. `decl` is a `VariableDeclaration` wrapped as `Node`.
        let mut decl: Option<&'gc Node<'gc>> = None;
        let mut expr1: Option<&'gc Node<'gc>> = None;

        // -------------------------------------------------------------------
        // Head dispatch. C++ 1866-1972.
        // -------------------------------------------------------------------
        if self.check2(TokenKind::rw_var, TokenKind::rw_const)
            || self.check_unescaped_name(b"let")
        {
            // Productions valid here:
            //   for ( var/let/const VariableDeclarationList
            //   for [await] ( var/let/const VariableDeclaration
            // C++ 1868-1884.
            let var_start_loc = self.cur_start();
            let decl_ident = self.lexer.token().get_res_word_or_identifier();
            self.advance(GrammarContext::AllowRegExp);

            let mut decl_list: Vec<&'gc Node<'gc>> = Vec::new();
            if !self.parse_variable_declaration_list(
                Param::default(),
                &mut decl_list,
                var_start_loc,
                VariableDeclAllowPattern::Yes,
            ) {
                return None;
            }

            let end_loc = decl_list
                .last()
                .expect("variable declaration list is non-empty")
                .range()
                .end;
            let node = Node::VariableDeclaration(VariableDeclaration::new(
                NodeMetadata::new(self.dummy_range()),
                decl_ident,
                NodeList::from_iter(self.gc, decl_list),
            ));
            decl = Some(self.set_location(var_start_loc, end_loc, node));
        } else if self.check_unescaped_name(b"using")
            && self.lexer.is_using_followed_by_identifier()
        {
            // for ( using Identifier
            //       ^
            // NOTE: lookahead must not be 'using of', so we check that below.
            // C++ 1885-1923.
            let var_start_loc = self.advance(GrammarContext::AllowRegExp).start;

            if self.check_unescaped_name(b"of") {
                // for (using of ....)
                //            ^
                // Not actually a 'using' declaration. C++ 1892-1900.
                let using_atom =
                    self.gc.ctx().atom_table.atom_bytes(b"using");
                let node = Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    using_atom,
                    None,  // type = null
                    false, // optional = false
                ));
                expr1 = Some(self.set_location(
                    var_start_loc,
                    self.lexer.prev_token_end(),
                    node,
                ));
            } else {
                // for ( using [no LineTerminator here] ForBinding
                //                                      ^
                // ForBinding: BindingIdentifier. C++ 1901-1922.
                assert!(
                    !self.lexer.is_new_line_before_current_token(),
                    "newline checked by isUsingFollowedByIdentifier"
                );
                let ident = self.parse_binding_identifier(param)?;
                let declarator =
                    Node::VariableDeclarator(VariableDeclarator::new(
                        NodeMetadata::new(self.dummy_range()),
                        None,
                        ident,
                    ));
                let declarator = self.set_location(
                    ident.range().start,
                    ident.range().end,
                    declarator,
                );
                let decl_list: Vec<&'gc Node<'gc>> = vec![declarator];

                let using_atom =
                    self.gc.ctx().atom_table.atom_bytes(b"using");
                let node = Node::VariableDeclaration(VariableDeclaration::new(
                    NodeMetadata::new(self.dummy_range()),
                    using_atom,
                    NodeList::from_iter(self.gc, decl_list),
                ));
                decl = Some(self.set_location(
                    var_start_loc,
                    self.lexer.prev_token_end(),
                    node,
                ));
            }
        } else if self.check_unescaped_name(b"await") && {
            // await using Identifier. C++ 1924-1926.
            let using_atom = self.gc.ctx().atom_table.atom_bytes(b"using");
            self.lexer.is_await_using_followed_by_identifier(using_atom)
        } {
            // C++ 1927-1946.
            let var_start_loc = self.advance(GrammarContext::AllowRegExp).start;
            self.advance(GrammarContext::AllowRegExp); // consume `using`

            let ident = self.parse_binding_identifier(param)?;
            let declarator = Node::VariableDeclarator(VariableDeclarator::new(
                NodeMetadata::new(self.dummy_range()),
                None,
                ident,
            ));
            let declarator = self.set_location(
                ident.range().start,
                ident.range().end,
                declarator,
            );
            let decl_list: Vec<&'gc Node<'gc>> = vec![declarator];

            let await_using_atom =
                self.gc.ctx().atom_table.atom_bytes(b"await using");
            let node = Node::VariableDeclaration(VariableDeclaration::new(
                NodeMetadata::new(self.dummy_range()),
                await_using_atom,
                NodeList::from_iter(self.gc, decl_list),
            ));
            decl = Some(self.set_location(
                var_start_loc,
                self.lexer.prev_token_end(),
                node,
            ));
        } else {
            // C++ 1947-1972.
            if !self.check(TokenKind::semi) {
                let opt_expr1 = if await_kw {
                    //   for await ( LeftHandSideExpression
                    //               ^
                    // C++ 1950-1953.
                    self.parse_left_hand_side_expression(
                        IsClassHeritageArgument::No,
                    )?
                } else {
                    // ForStatement:
                    //   for ( Expression_opt
                    //         ^
                    // ForInOfStatement:
                    //   for ( LeftHandSideExpression
                    //         ^
                    // Lookahead for LeftHandSideExpression cannot be 'let' or
                    // 'async of'. We've handled `let` above. To distinguish the
                    // two productions here, we let the resolver check that the
                    // LHS of the `of` or `in` is valid (the resolver throws the
                    // error instead of the parser). C++ 1954-1966.
                    self.parse_expression(Param::default(), CoverTypedParameters::Yes)?
                };
                expr1 = Some(opt_expr1);
            }
        }

        // -------------------------------------------------------------------
        // Branch: for-in/for-of vs C-style for. C++ 1974-2092.
        // -------------------------------------------------------------------
        if self.check(TokenKind::rw_in) || self.check_unescaped_name(b"of") {
            // Productions valid here:
            //   for [await] ( var/let/const VariableDeclaration[In] in/of
            //   for [await] ( LeftHandSideExpression in/of
            // C++ 1974-2029.

            // C++ 1979-1984: only one binding allowed.
            if let Some(d) = decl {
                if let Node::VariableDeclaration(vd) = d {
                    if vd.declarations.iter().count() > 1 {
                        self.error_at(
                            d.range(),
                            "Only one binding must be declared in a for-in/for-of loop",
                        );
                        return None;
                    }
                }
            }

            // Check for a destructuring pattern on the left and reparse it.
            // C++ 1986-1994.
            if let Some(e) = expr1 {
                if matches!(
                    e,
                    Node::ArrayExpression(_) | Node::ObjectExpression(_)
                ) {
                    expr1 = Some(self.reparse_assignment_pattern(e, false)?);
                }
            }

            // Remember whether we are parsing for-in or for-of. C++ 1996-1998.
            let for_in_loop = self.check(TokenKind::rw_in);
            self.advance(GrammarContext::AllowRegExp);

            // C++ 2000-2001.
            if for_in_loop && await_kw {
                self.error_at(await_rng, "unexpected 'await' in for..in loop");
            }

            // C++ 2003-2004: `parseExpression()` for in, `parseAssignment
            // Expression(ParamIn)` for of. NB the bare C++ `parseExpression()`
            // uses the header default `Param = ParamIn` (JSParserImpl.h:1141),
            // so `in` is recognized as a binary operator in the right-hand side.
            let opt_right = if for_in_loop {
                self.parse_expression(PARAM_IN, CoverTypedParameters::Yes)
            } else {
                self.parse_assignment_expression(PARAM_IN, false, AllowTypedArrowFunction::Yes, CoverTypedParameters::Yes, None)
            };

            // C++ 2006-2012: eat(r_paren, "after 'for(... in/of ...'",
            // "location of '('", lparenLoc).
            if !self.eat_at(
                TokenKind::r_paren,
                GrammarContext::AllowRegExp,
                " after 'for(... in/of ...'",
                Some("location of '('"),
                lparen_loc,
            ) {
                return None;
            }

            // C++ 2014-2016: check body and right together.
            let body = self.parse_statement(param.get(PARAM_RETURN));
            let (body, right) = match (body, opt_right) {
                (Some(b), Some(r)) => (b, r),
                _ => return None,
            };

            // left = decl ? decl : expr1. C++ 2021/2024.
            let left = decl.unwrap_or_else(|| {
                expr1.expect("for-in/of head must have decl or expr1")
            });

            // C++ 2018-2029.
            let body_end = body.range().end;
            let node = if for_in_loop {
                Node::ForInStatement(ForInStatement::new(
                    NodeMetadata::new(self.dummy_range()),
                    left,
                    right,
                    body,
                ))
            } else {
                Node::ForOfStatement(ForOfStatement::new(
                    NodeMetadata::new(self.dummy_range()),
                    left,
                    right,
                    body,
                    await_kw,
                ))
            };
            Some(self.set_location(start_loc, body_end, node))
        } else if self.check_and_eat(TokenKind::semi, GrammarContext::AllowRegExp)
        {
            // Productions valid here:
            //   for ( var/let/const VariableDeclarationList[In] ; Expression_opt
            //         ; Expression_opt ) Statement
            //   for ( Expression[In]_opt ; Expression_opt ; Expression_opt )
            //         Statement
            // C++ 2030-2083.

            // C++ 2037-2038.
            if await_kw {
                self.error_at(
                    await_rng,
                    "unexpected 'await' in for loop without 'of'",
                );
            }

            // C++ 2040-2041.
            if let Some(d) = decl {
                self.ensure_destructuring_initialized(d);
            }

            // C++ 2043-2049. Bare C++ `parseExpression()` → default `ParamIn`.
            let test = if self.check(TokenKind::semi) {
                None
            } else {
                Some(self.parse_expression(PARAM_IN, CoverTypedParameters::Yes)?)
            };

            // C++ 2051-2057: eat(semi, "after 'for( ... ; ...'", "location
            // of '('", lparenLoc).
            if !self.eat_at(
                TokenKind::semi,
                GrammarContext::AllowRegExp,
                " after 'for( ... ; ...'",
                Some("location of '('"),
                lparen_loc,
            ) {
                return None;
            }

            // C++ 2059-2065. Bare C++ `parseExpression()` → default `ParamIn`.
            let update = if self.check(TokenKind::r_paren) {
                None
            } else {
                Some(self.parse_expression(PARAM_IN, CoverTypedParameters::Yes)?)
            };

            // C++ 2067-2073: eat(r_paren, "after 'for( ... ; ... ; ...'",
            // "location of '('", lparenLoc).
            if !self.eat_at(
                TokenKind::r_paren,
                GrammarContext::AllowRegExp,
                " after 'for( ... ; ... ; ...'",
                Some("location of '('"),
                lparen_loc,
            ) {
                return None;
            }

            // C++ 2075-2077.
            let body = self.parse_statement(param.get(PARAM_RETURN))?;

            // init = decl ? decl : expr1. C++ 2079-2083.
            let init = decl.or(expr1);
            let body_end = body.range().end;
            let node = Node::ForStatement(ForStatement::new(
                NodeMetadata::new(self.dummy_range()),
                init,
                test,
                update,
                body,
            ));
            Some(self.set_location(start_loc, body_end, node))
        } else {
            // C++ 2084-2091: errorExpected(semi, rw_in, "inside 'for'",
            // "location of the 'for'", startLoc).
            self.error_expected2(
                TokenKind::semi,
                TokenKind::rw_in,
                " inside 'for'",
                Some("location of the 'for'"),
                start_loc,
            );
            None
        }
    }

    // -----------------------------------------------------------------------
    // parseSwitchStatement — 2220 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `switch ( Expr ) { CaseClauses }` statement. Port of
    /// `JSParserImpl::parseSwitchStatement` (lines 2220-2340).
    pub(super) fn parse_switch_statement(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        assert!(self.check(TokenKind::rw_switch));
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        // C++ 2225: SMLoc lparenLoc = tok_->getStartLoc(), captured before
        // the '(' eat below and reused by the ')' eat's whatLoc.
        let lparen_loc = self.cur_start();
        // C++ 2226-2232: eat(l_paren, "after 'switch'", "location of
        // 'switch'", startLoc).
        if !self.eat_at(
            TokenKind::l_paren,
            GrammarContext::AllowRegExp,
            " after 'switch'",
            Some("location of 'switch'"),
            start_loc,
        ) {
            return None;
        }

        let discriminant = self.parse_expression(PARAM_IN, CoverTypedParameters::Yes)?;

        // C++ 2238-2244: eat(r_paren, "after 'switch (...'", "location of
        // '('", lparenLoc).
        if !self.eat_at(
            TokenKind::r_paren,
            GrammarContext::AllowRegExp,
            " after 'switch (...'",
            Some("location of '('"),
            lparen_loc,
        ) {
            return None;
        }

        // C++ 2246: SMLoc lbraceLoc = tok_->getStartLoc(), captured before
        // the '{' eat below and reused by the closing '}' eat's whatLoc.
        let lbrace_loc = self.cur_start();
        // C++ 2247-2253: eat(l_brace, "after 'switch (...)'", "'switch'
        // starts here", startLoc).
        if !self.eat_at(
            TokenKind::l_brace,
            GrammarContext::AllowRegExp,
            " after 'switch (...)'",
            Some("'switch' starts here"),
            start_loc,
        ) {
            return None;
        }

        let mut clause_list: Vec<&'gc Node<'gc>> = Vec::new();
        // location of the 'default' clause. C++ 2256.
        let mut default_location: Option<SMLoc> = None;

        // Parse the switch body. C++ 2259-2324.
        while !self.check(TokenKind::r_brace) {
            let clause_start_loc = self.lexer.token().start_loc();

            let mut test_expr: Option<&'gc Node<'gc>> = None;
            // Set to true in error recovery when we want to parse but ignore the
            // parsed statements. C++ 2263-2264.
            let mut ignore_clause = false;
            let mut stmt_list: Vec<&'gc Node<'gc>> = Vec::new();

            let case_loc = self.lexer.token().start_loc();
            if self.check_and_eat(TokenKind::rw_case, GrammarContext::AllowRegExp)
            {
                // C++ 2269: parseExpression(ParamIn, CoverTypedParameters::No) —
                // the `:` after the case test must NOT be eaten as a cover type
                // annotation.
                test_expr = Some(self.parse_expression(PARAM_IN, CoverTypedParameters::No)?);
            } else if self
                .check_and_eat(TokenKind::rw_default, GrammarContext::AllowRegExp)
            {
                // C++ 2273-2282.
                if default_location.is_some() {
                    self.error_at(
                        SMRange {
                            start: clause_start_loc,
                            end: clause_start_loc,
                        },
                        "more than one 'default' clause in 'switch'",
                    );
                    // C++ also emits sm_.note(defaultLocation, "first 'default'
                    // clause was defined here"); the note is dropped per house
                    // style.

                    // We want to continue parsing but ignore the statements.
                    ignore_clause = true;
                } else {
                    default_location = Some(clause_start_loc);
                }
            } else {
                // C++ 2284-2290: errorExpected(rw_case, rw_default, "inside
                // 'switch'", "location of 'switch'", startLoc).
                self.error_expected2(
                    TokenKind::rw_case,
                    TokenKind::rw_default,
                    " inside 'switch'",
                    Some("location of 'switch'"),
                    start_loc,
                );
                return None;
            }

            // save the location in case the clause is empty. C++ 2293-2294.
            let colon_loc = self.lexer.token().end_loc();
            // C++ 2295-2301: eat(colon, "after 'case ...' or 'default'",
            // "location of 'case'/'default'", caseLoc).
            if !self.eat_at(
                TokenKind::colon,
                GrammarContext::AllowRegExp,
                " after 'case ...' or 'default'",
                Some("location of 'case'/'default'"),
                case_loc,
            ) {
                return None;
            }

            // case Expression : StatementList[opt]. C++ 2305-2313.
            if !self.parse_statement_list(
                param.get(PARAM_RETURN),
                [
                    TokenKind::rw_default,
                    TokenKind::rw_case,
                    TokenKind::r_brace,
                ],
                false,
                AllowImportExport::No,
                &mut stmt_list,
            ) {
                return None;
            }

            // C++ 2315-2323.
            if !ignore_clause {
                let clause_end_loc = match stmt_list.last() {
                    Some(last) => last.range().end,
                    None => colon_loc,
                };
                let node = Node::SwitchCase(SwitchCase::new(
                    NodeMetadata::new(self.dummy_range()),
                    test_expr,
                    NodeList::from_iter(self.gc, stmt_list),
                ));
                clause_list.push(self.set_location(
                    clause_start_loc,
                    clause_end_loc,
                    node,
                ));
            }
        }

        // C++ 2326-2333: eat(r_brace, "at end of 'switch' statement",
        // "location of '{'", lbraceLoc).
        let end_loc = self.lexer.token().end_loc();
        if !self.eat_at(
            TokenKind::r_brace,
            GrammarContext::AllowRegExp,
            " at end of 'switch' statement",
            Some("location of '{'"),
            lbrace_loc,
        ) {
            return None;
        }

        // C++ 2335-2339.
        let node = Node::SwitchStatement(SwitchStatement::new(
            NodeMetadata::new(self.dummy_range()),
            discriminant,
            NodeList::from_iter(self.gc, clause_list),
        ));
        Some(self.set_location(start_loc, end_loc, node))
    }

    // -----------------------------------------------------------------------
    // parseTryStatement — 2366 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `try Block [catch ...] [finally Block]` statement. Port of
    /// `JSParserImpl::parseTryStatement` (lines 2366-2465).
    pub(super) fn parse_try_statement(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        assert!(self.check(TokenKind::rw_try));
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        // C++ 2371: need(l_brace, "after 'try'", "location of 'try'",
        // startLoc). `startLoc` is the 'try' keyword, so when the offending
        // token is on a later line the diagnostic is followed by a
        // "location of 'try'" note pointing back at it.
        if !self.need_at(
            TokenKind::l_brace,
            " after 'try'",
            Some("location of 'try'"),
            start_loc,
        ) {
            return None;
        }
        let try_body = self.parse_block(
            param.get(PARAM_RETURN),
            GrammarContext::AllowRegExp,
            false,
        )?;

        let mut catch_handler: Option<&'gc Node<'gc>> = None;
        let mut finally_handler: Option<&'gc Node<'gc>> = None;

        // Parse the optional 'catch' handler. C++ 2380-2428.
        let handler_start_loc = self.lexer.token().start_loc();
        if self.check_and_eat(TokenKind::rw_catch, GrammarContext::AllowRegExp) {
            let mut catch_param: Option<&'gc Node<'gc>> = None;
            // CatchClause param is optional. C++ 2384-2411.
            if self.check_and_eat(TokenKind::l_paren, GrammarContext::AllowRegExp)
            {
                catch_param = Some(
                    if self.check2(TokenKind::l_square, TokenKind::l_brace) {
                        self.parse_binding_pattern(Param::default())?
                    } else {
                        match self.parse_binding_identifier(Param::default()) {
                            Some(ident) => ident,
                            None => {
                                // C++ 2393-2399: errorExpected(identifier,
                                // "inside catch list", "location of
                                // 'catch'", handlerStartLoc).
                                self.error_expected_msg(
                                    "'identifier' expected inside catch list",
                                    Some("location of 'catch'"),
                                    Some(handler_start_loc),
                                );
                                return None;
                            }
                        }
                    },
                );

                // C++ 2404-2410: eat(r_paren, "after 'catch (...'",
                // "location of 'catch'", handlerStartLoc).
                if !self.eat_at(
                    TokenKind::r_paren,
                    GrammarContext::AllowRegExp,
                    " after 'catch (...'",
                    Some("location of 'catch'"),
                    handler_start_loc,
                ) {
                    return None;
                }
            }

            // C++ 2413-2418: need(l_brace, "after 'catch(...)'", "location
            // of 'catch'", handlerStartLoc).
            if !self.need_at(
                TokenKind::l_brace,
                " after 'catch(...)'",
                Some("location of 'catch'"),
                handler_start_loc,
            ) {
                return None;
            }
            let catch_body = self.parse_block(
                param.get(PARAM_RETURN),
                GrammarContext::AllowRegExp,
                false,
            )?;

            // C++ 2423-2427.
            let catch_end = catch_body.range().end;
            let node = Node::CatchClause(CatchClause::new(
                NodeMetadata::new(self.dummy_range()),
                catch_param,
                catch_body,
            ));
            catch_handler =
                Some(self.set_location(handler_start_loc, catch_end, node));
        }

        // Parse the optional 'finally' handler. C++ 2430-2444.
        let finally_loc = self.lexer.token().start_loc();
        if self.check_and_eat(TokenKind::rw_finally, GrammarContext::AllowRegExp)
        {
            // C++ 2433-2437: need(l_brace, "after 'finally'", "location of
            // 'finally'", finallyLoc).
            if !self.need_at(
                TokenKind::l_brace,
                " after 'finally'",
                Some("location of 'finally'"),
                finally_loc,
            ) {
                return None;
            }
            let finally_body = self.parse_block(
                param.get(PARAM_RETURN),
                GrammarContext::AllowRegExp,
                false,
            )?;
            finally_handler = Some(finally_body);
        }

        // At least one handler must be present. C++ 2447-2454:
        // errorExpected(rw_catch, rw_finally, "after 'try' block", "location
        // of 'try'", startLoc).
        if catch_handler.is_none() && finally_handler.is_none() {
            self.error_expected2(
                TokenKind::rw_catch,
                TokenKind::rw_finally,
                " after 'try' block",
                Some("location of 'try'"),
                start_loc,
            );
            return None;
        }

        // Use the last handler's location as the end location. C++ 2457-2459.
        let end_loc = match finally_handler {
            Some(f) => f.range().end,
            None => catch_handler.unwrap().range().end,
        };
        // C++ 2460-2464.
        let node = Node::TryStatement(TryStatement::new(
            NodeMetadata::new(self.dummy_range()),
            try_body,
            catch_handler,
            finally_handler,
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
            // Point location, NOT the current token's range: C++ `eatSemi`
            // (JSParserImpl.cpp:336) calls `error(tok_->getStartLoc(), ...)`,
            // i.e. the `error(SMLoc, Twine)` overload (JSParserImpl.h:
            // 472-474), which renders a bare caret. `error_cur` underlines
            // the whole token (`^~~~` instead of `^`) on any multi-character
            // token following the missing `;`.
            let loc = self.cur_start();
            self.error_at_loc(loc, "';' expected");
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
            // Raw is the source text minus the enclosing quote characters
            // (the +1/-1 below skip the quotes). The C++ interns it via
            // `lexer_.getIdentifier` (NOT `getStringLiteral` — no surrogate
            // re-encoding), so we use `get_identifier` on the shared
            // `source_bytes` slice instead of `source_bytes_atom`.
            let raw_slice = self.source_bytes(
                SMLoc {
                    source: tok_start.source,
                    offset: tok_start.offset + 1,
                },
                SMLoc {
                    source: tok_end.source,
                    offset: tok_end.offset - 1,
                },
            );
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
        // Record the directive for lazy pass recovery before setting strict
        // mode. Port of `seenDirectives_.push_back` (JSParserImpl.cpp:341).
        self.seen_directives.push(bytes.to_vec());
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
    /// `param_yield`/`param_await`/strict-mode state. With types enabled the
    /// trailing `?` (optional marker) and `: TypeAnnotation` are parsed.
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

        // `?` optional marker and `: TypeAnnotation`. C++ 1063-1080.
        let mut type_annotation: Option<&'gc Node<'gc>> = None;
        let mut optional = false;
        if self.parse_types() {
            if self.check(TokenKind::question) {
                optional = true;
                self.advance(GrammarContext::Type);
            }

            if self.check(TokenKind::colon) {
                let annot_start = self.advance(GrammarContext::Type).start;
                type_annotation = Some(self.parse_type_annotation(
                    Some(annot_start),
                    AllowAnonFunctionType::Yes,
                )?);
            }
        }

        // C++ 1082-1085.
        let node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            type_annotation,
            optional,
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

        // C++ 1335-1341: eat(r_square, AllowDiv, "at end of array binding
        // pattern '[...'", "location of '['", startLoc). Closing eat uses
        // AllowDiv.
        if !self.eat_at(
            TokenKind::r_square,
            GrammarContext::AllowDiv,
            " at end of array binding pattern '[...'",
            Some("location of '['"),
            start_loc,
        ) {
            return None;
        }

        // `: TypeAnnotation`. C++ 1343-1354.
        let mut type_annotation: Option<&'gc Node<'gc>> = None;
        if self.parse_types() && self.check(TokenKind::colon) {
            let annot_start = self.advance(GrammarContext::Type).start;
            type_annotation = Some(self.parse_type_annotation(
                Some(annot_start),
                AllowAnonFunctionType::Yes,
            )?);
        }

        // C++ 1356-1359.
        let node = Node::ArrayPattern(ArrayPattern::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, elem_list),
            type_annotation,
        ));
        Some(self.set_location(start_loc, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseBindingElement — 1362 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `BindingElement` (a binding target with optional initializer).
    /// Port of `JSParserImpl::parseBindingElement` (lines 1362-1390).
    pub(super) fn parse_binding_element(
        &mut self,
        param: Param,
    ) -> Option<&'gc Node<'gc>> {
        let _guard = self.check_recursion()?;

        // C++ 1366-1380.
        let elem: &'gc Node<'gc> =
            if self.check(TokenKind::l_square) || self.check(TokenKind::l_brace) {
                self.parse_binding_pattern(param)?
            } else {
                match self.parse_binding_identifier(param) {
                    Some(ident) => ident,
                    None => {
                        // Point location, NOT the current token's range:
                        // C++ (cpp:1374-1376) calls `error(tok_->
                        // getStartLoc(), ...)` — the `error(SMLoc, Twine)`
                        // overload — so the caret is bare.
                        self.error_at_loc(
                            self.cur_start(),
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
    pub(super) fn parse_binding_rest_element(
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
    pub(super) fn parse_binding_initializer(
        &mut self,
        param: Param,
        left: &'gc Node<'gc>,
    ) -> Option<&'gc Node<'gc>> {
        assert!(self.check(TokenKind::equal), "binding initializer requires '='");

        // Parse the initializer. C++ 1421.
        let debug_loc = self.advance(GrammarContext::AllowRegExp).start;

        let expr = self.parse_assignment_expression(PARAM_IN.plus(param), false, AllowTypedArrowFunction::Yes, CoverTypedParameters::Yes, None)?;

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

        // C++ 1466-1472: eat(r_brace, AllowDiv, "at end of object binding
        // pattern '{...'", "location of '{'", startLoc). Closing eat uses
        // AllowDiv.
        if !self.eat_at(
            TokenKind::r_brace,
            GrammarContext::AllowDiv,
            " at end of object binding pattern '{...'",
            Some("location of '{'"),
            start_loc,
        ) {
            return None;
        }

        // `: TypeAnnotation`. C++ 1474-1485.
        let mut type_annotation: Option<&'gc Node<'gc>> = None;
        if self.parse_types() && self.check(TokenKind::colon) {
            let annot_start = self.advance(GrammarContext::Type).start;
            type_annotation = Some(self.parse_type_annotation(
                Some(annot_start),
                AllowAnonFunctionType::Yes,
            )?);
        }

        // C++ 1487-1490.
        let node = Node::ObjectPattern(ObjectPattern::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, prop_list),
            type_annotation,
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
