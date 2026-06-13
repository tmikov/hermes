/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Module (`import`/`export` declaration) parsing for the JS parser. Port of
//! the module-declaration section of `lib/Parser/JSParserImpl.cpp`
//! (parseFromClause / parseWithClause / parseImportDeclaration /
//! parseImportClause / parseNameSpaceImport / parseNamedImports /
//! parseImportSpecifier, C++ lines 6611-7125; parseExportDeclaration /
//! parseExportClause / parseExportSpecifier, C++ lines 7127-7467).
//!
//! Flow/TS productions (the `import type` / `import typeof` kind detection and
//! the per-specifier `type`/`typeof` forms) are gated off by
//! `context_.getParseFlow()`/`getParseTS()` in C++; they are omitted here. The
//! corresponding `// P6/P7` comments mark each omission site. Until those
//! land, the import kind is always `value`.

use std::collections::hash_map::Entry;
use std::collections::HashMap;

use ast::node::{
    ExportAllDeclaration, ExportDefaultDeclaration, ExportNamedDeclaration,
    ExportNamespaceSpecifier, ExportSpecifier, Identifier, ImportAttribute,
    ImportDeclaration, ImportDefaultSpecifier, ImportNamespaceSpecifier,
    ImportSpecifier, Node, StringLiteral,
};
use ast::node_child::{NodeLabel, NodeList, NodeMetadata};
use support::location::{SMLoc, SMRange};

use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

use super::flow::{AllowTypedArrowFunction, CoverTypedParameters};
use super::{JSParserImpl, Param, PARAM_DEFAULT, PARAM_IN};

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    // -----------------------------------------------------------------------
    // parseFromClause — 6611 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `from 'module'` clause and return the module-source
    /// `StringLiteral`. Port of `JSParserImpl::parseFromClause` (6611-6634).
    pub(super) fn parse_from_clause(&mut self) -> Option<&'gc Node<'gc>> {
        let start_loc = self.cur_start();

        // `from` is a contextual identifier — compare the interned name
        // (escape-insensitive), the C++ `check(fromIdent_)` overload. C++ 6614.
        if self.check_name(b"from") {
            self.advance(GrammarContext::AllowRegExp);
        } else {
            // C++ `error(startLoc, "'from' expected")` reports at the saved
            // start loc with a zero-width range.
            self.error_at(
                SMRange {
                    start: start_loc,
                    end: start_loc,
                },
                "'from' expected",
            );
            return None;
        }

        // C++ 6619-6625. note arg dropped per house style.
        if !self.need(TokenKind::string_literal, " after 'from'") {
            return None;
        }

        // C++ 6627-6631.
        let tok_start = self.lexer.token().start_loc();
        let tok_end = self.lexer.token().end_loc();
        let value = self.lexer.token().get_string_literal();
        let node = Node::StringLiteral(StringLiteral::new(
            NodeMetadata::new(self.dummy_range()),
            value,
        ));
        let source = self.set_location(tok_start, tok_end, node);
        self.advance(GrammarContext::AllowRegExp);
        Some(source)
    }

    // -----------------------------------------------------------------------
    // parseWithClause — 6636 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `with { key: 'value', ... }` import-assertion clause, appending
    /// each `ImportAttribute` to `attributes`. Port of
    /// `JSParserImpl::parseWithClause` (6636-6720).
    pub(super) fn parse_with_clause(
        &mut self,
        attributes: &mut Vec<&'gc Node<'gc>>,
    ) -> bool {
        debug_assert!(self.check(TokenKind::rw_with));
        let _start = self.advance(GrammarContext::AllowRegExp).start;

        // with { }
        // with { WithEntries ,[opt] }
        //      ^
        // C++ 6644-6650. note arg dropped per house style.
        if !self.eat(
            TokenKind::l_brace,
            GrammarContext::AllowRegExp,
            " in import assertion",
        ) {
            return false;
        }

        while !self.check(TokenKind::r_brace) {
            // AssertionKey : StringLiteral
            // ^
            // C++ 6655-6676.
            let key: &'gc Node<'gc>;
            if self.check(TokenKind::string_literal) {
                let tok_start = self.lexer.token().start_loc();
                let tok_end = self.lexer.token().end_loc();
                let value = self.lexer.token().get_string_literal();
                let node = Node::StringLiteral(StringLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    value,
                ));
                key = self.set_location(tok_start, tok_end, node);
                self.advance(GrammarContext::AllowRegExp);
            } else {
                // note arg dropped per house style.
                if !self.need(TokenKind::identifier, " in import assertion") {
                    return false;
                }
                let tok_start = self.lexer.token().start_loc();
                let tok_end = self.lexer.token().end_loc();
                let name = self.lexer.token().get_identifier();
                let node = Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    name,
                    None,
                    false,
                ));
                key = self.set_location(tok_start, tok_end, node);
                self.advance(GrammarContext::AllowRegExp);
            }

            // C++ 6678-6684. note arg dropped per house style.
            if !self.eat(
                TokenKind::colon,
                GrammarContext::AllowRegExp,
                " in import assertion",
            ) {
                return false;
            }

            // AssertionKey : StringLiteral
            //                ^
            // C++ 6689-6694. note arg dropped per house style.
            if !self.need(TokenKind::string_literal, " in import assertion") {
                return false;
            }

            let tok_start = self.lexer.token().start_loc();
            let tok_end = self.lexer.token().end_loc();
            let val = self.lexer.token().get_string_literal();
            let val_node = Node::StringLiteral(StringLiteral::new(
                NodeMetadata::new(self.dummy_range()),
                val,
            ));
            let value = self.set_location(tok_start, tok_end, val_node);
            self.advance(GrammarContext::AllowRegExp);

            // setLocation(key, value, ...): start = key start, end = value end.
            // C++ 6701-6702.
            let attr = Node::ImportAttribute(ImportAttribute::new(
                NodeMetadata::new(self.dummy_range()),
                key,
                value,
            ));
            let attr = self.set_location(
                key.range().start,
                value.range().end,
                attr,
            );
            attributes.push(attr);

            // C++ 6704: `checkAndEat(comma)` — the default grammar context is
            // AllowRegExp (JSParserImpl.h:507-510).
            if !self.check_and_eat(TokenKind::comma, GrammarContext::AllowRegExp)
            {
                break;
            }
        }

        // with { AssertEntries ,[opt] }
        //                              ^
        // C++ 6711-6717. note arg dropped per house style.
        if !self.eat(
            TokenKind::r_brace,
            GrammarContext::AllowRegExp,
            " in import assertion",
        ) {
            return false;
        }

        true
    }

    // -----------------------------------------------------------------------
    // parseImportDeclaration — 6722 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse an `import` declaration. Port of
    /// `JSParserImpl::parseImportDeclaration` (6722-6782).
    pub(super) fn parse_import_declaration(&mut self) -> Option<&'gc Node<'gc>> {
        debug_assert!(
            self.check(TokenKind::rw_import),
            "import declaration must start with 'import'"
        );
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        // The `value` import kind label (used until Flow/TS `import type` lands).
        let value_ident = self.gc.ctx().atom_table.atom_bytes(b"value");

        if self.check(TokenKind::string_literal) {
            // import ModuleSpecifier ;
            // If the first token is a string literal, there are no specifiers,
            // so the import clause should not be parsed. C++ 6729-6754.
            let tok_start = self.lexer.token().start_loc();
            let tok_end = self.lexer.token().end_loc();
            let value = self.lexer.token().get_string_literal();
            let node = Node::StringLiteral(StringLiteral::new(
                NodeMetadata::new(self.dummy_range()),
                value,
            ));
            let source = self.set_location(tok_start, tok_end, node);
            self.advance(GrammarContext::AllowRegExp);

            let mut attributes: Vec<&'gc Node<'gc>> = Vec::new();
            // Nested `if` mirrors the C++ 6740-6743 structure (an inner
            // `if (!parseWithClause(...)) return None`); kept rather than
            // collapsed for faithfulness.
            #[allow(clippy::collapsible_if)]
            if self.check(TokenKind::rw_with)
                && !self.lexer.is_new_line_before_current_token()
            {
                if !self.parse_with_clause(&mut attributes) {
                    return None;
                }
            }

            if !self.eat_semi(false) {
                return None;
            }

            let node = Node::ImportDeclaration(ImportDeclaration::new(
                NodeMetadata::new(self.dummy_range()),
                NodeList::empty(),
                source,
                NodeList::from_iter(self.gc, attributes),
                value_ident,
            ));
            return Some(self.set_location(
                start_loc,
                self.lexer.prev_token_end(),
                node,
            ));
        }

        // C++ 6756-6781. `parseImportClause` returns the specifiers; the kind it
        // returns in C++ is always `value` until Flow/TS lands (see
        // `parse_import_clause`).
        let specifiers = self.parse_import_clause()?;

        let source = self.parse_from_clause()?;

        let mut attributes: Vec<&'gc Node<'gc>> = Vec::new();
        // Nested `if` mirrors the C++ 6768-6771 structure; kept rather than
        // collapsed for faithfulness.
        #[allow(clippy::collapsible_if)]
        if self.check(TokenKind::rw_with)
            && !self.lexer.is_new_line_before_current_token()
        {
            if !self.parse_with_clause(&mut attributes) {
                return None;
            }
        }

        if !self.eat_semi(false) {
            return None;
        }

        let node = Node::ImportDeclaration(ImportDeclaration::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, specifiers),
            source,
            NodeList::from_iter(self.gc, attributes),
            value_ident,
        ));
        Some(self.set_location(start_loc, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseImportClause — 6784 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse the import clause (default binding, namespace import, and/or named
    /// imports) and return the specifiers. Port of
    /// `JSParserImpl::parseImportClause` (6784-6871).
    ///
    /// ## Documented simplification
    /// C++ returns `Optional<UniqueString* kind>` and fills `specifiers` by
    /// reference. Since `kind` is always `value` until Flow/TS `import type`
    /// lands (P6/P7), we return just the `Vec` of specifiers and let the caller
    /// supply the `value` kind. The kind return is reintroduced when
    /// `import type` is implemented.
    fn parse_import_clause(&mut self) -> Option<Vec<&'gc Node<'gc>>> {
        let mut specifiers: Vec<&'gc Node<'gc>> = Vec::new();
        let start_loc = self.cur_start();

        // P6/P7: Flow/TS import-kind (type/typeof) detection omitted.
        // C++ 6790-6805.

        if self.check(TokenKind::identifier) {
            // The `check(fromIdent_) && kind == typeIdent_` sub-branch
            // (C++ 6808-6818) can't fire because kind is always `value`, so we
            // port only the `else` (C++ 6819-6837).
            //
            // ImportedDefaultBinding
            // ImportedDefaultBinding , NameSpaceImport
            // ImportedDefaultBinding , NamedImports
            let default_binding =
                match self.parse_binding_identifier(Param::default()) {
                    Some(b) => b,
                    None => {
                        // C++ errorExpected(identifier, "in import clause", ...).
                        // note arg dropped per house style.
                        let _ = start_loc;
                        self.error_cur("'identifier' expected in import clause");
                        return None;
                    }
                };
            let spec = Node::ImportDefaultSpecifier(
                ImportDefaultSpecifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    default_binding,
                ),
            );
            let rng = default_binding.range();
            specifiers.push(self.set_location(rng.start, rng.end, spec));

            if !self.check_and_eat(TokenKind::comma, GrammarContext::AllowRegExp)
            {
                // If there was no comma, there's no more bindings to parse,
                // so return immediately. C++ 6838-6842.
                return Some(specifiers);
            }
        }

        // At this point, either:
        // - the ImportedDefaultBinding was parsed and had a comma after it
        // - there was no ImportedDefaultBinding and we simply continue
        // C++ 6849-6857.
        if self.check(TokenKind::star) {
            // NameSpaceImport
            let ns = self.parse_name_space_import()?;
            specifiers.push(ns);
            return Some(specifiers);
        }

        // NamedImports is the only remaining possibility. C++ 6860-6866.
        // note arg dropped per house style. NOTE: when the brace is missing C++
        // returns the accumulated kind WITHOUT propagating an error-None, so we
        // replicate that and return the accumulated specifiers.
        if !self.need(TokenKind::l_brace, " in import specifier clause") {
            return Some(specifiers);
        }

        if !self.parse_named_imports(&mut specifiers) {
            return None;
        }
        Some(specifiers)
    }

    // -----------------------------------------------------------------------
    // parseNameSpaceImport — 6873 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a namespace import (`* as ns`). Port of
    /// `JSParserImpl::parseNameSpaceImport` (6873-6896).
    fn parse_name_space_import(&mut self) -> Option<&'gc Node<'gc>> {
        debug_assert!(
            self.check(TokenKind::star),
            "import namespace must start with *"
        );

        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        // `as` is a contextual identifier (escape-insensitive). C++ 6877-6880:
        // `if (!checkAndEat(asIdent_)) error(tok_->getStartLoc(), ...)` — reports
        // at the CURRENT token start, which is what `error_cur` does.
        if self.check_name(b"as") {
            self.advance(GrammarContext::AllowRegExp);
        } else {
            self.error_cur("'as' expected");
            return None;
        }

        let local = match self.parse_binding_identifier(Param::default()) {
            Some(l) => l,
            None => {
                // C++ errorExpected(identifier, "in namespace import", ...).
                // note arg dropped per house style.
                self.error_cur("'identifier' expected in namespace import");
                return None;
            }
        };

        // setLocation(startLoc, *optLocal, ...): end = local's end. C++ 6892.
        let node = Node::ImportNamespaceSpecifier(
            ImportNamespaceSpecifier::new(
                NodeMetadata::new(self.dummy_range()),
                local,
            ),
        );
        Some(self.set_location(start_loc, local.range().end, node))
    }

    // -----------------------------------------------------------------------
    // parseNamedImports — 6898 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `{ a, b as c, ... }` named-imports clause, appending each
    /// `ImportSpecifier` to `specifiers`. Port of
    /// `JSParserImpl::parseNamedImports` (6898-6941).
    fn parse_named_imports(
        &mut self,
        specifiers: &mut Vec<&'gc Node<'gc>>,
    ) -> bool {
        debug_assert!(
            self.check(TokenKind::l_brace),
            "named imports must start with {{"
        );
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        // BoundNames to check for duplicate entries in ImportDeclaration.
        // Values are the local IdentifierNode source ranges, used for error
        // reporting. C++ 6902-6904. The C++ "first usage of name" note is
        // dropped per house style.
        let mut bound_names: HashMap<NodeLabel, SMRange> = HashMap::new();

        while !self.check(TokenKind::r_brace) {
            let spec = match self.parse_import_specifier(start_loc) {
                Some(s) => s,
                None => return false,
            };

            // Check if the bound name was duplicated. C++ 6912-6925.
            let (local_name, local_range) = match spec {
                Node::ImportSpecifier(is) => match is.local {
                    Node::Identifier(id) => {
                        (id.name.get(), id.metadata.range.get())
                    }
                    _ => unreachable!("import specifier local is an Identifier"),
                },
                _ => unreachable!("parseImportSpecifier returns an ImportSpecifier"),
            };
            // C++ `boundNames.try_emplace(...)` (6915): insert-if-absent, then
            // branch on whether it was inserted. The Entry API mirrors that.
            match bound_names.entry(local_name) {
                Entry::Vacant(e) => {
                    e.insert(local_range);
                    specifiers.push(spec);
                }
                Entry::Occupied(_) => {
                    // Report the error but continue parsing to see if there's
                    // any others. (first-usage note dropped per house style.)
                    self.error_at(
                        local_range,
                        "Duplicate entry in import declaration list",
                    );
                }
            }

            if !self.check_and_eat(TokenKind::comma, GrammarContext::AllowRegExp)
            {
                break;
            }
        }

        // C++ 6931-6938: NOTE grammar context AllowDiv. note arg dropped per
        // house style.
        if !self.eat(
            TokenKind::r_brace,
            GrammarContext::AllowDiv,
            " at end of named imports",
        ) {
            return false;
        }

        true
    }

    // -----------------------------------------------------------------------
    // parseImportSpecifier — 6943 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a single named-import specifier (`a` or `a as b`). Port of the
    /// non-Flow `else` branch of `JSParserImpl::parseImportSpecifier`
    /// (the JS-only path, C++ 7074-7124).
    fn parse_import_specifier(
        &mut self,
        import_loc: support::location::SMLoc,
    ) -> Option<&'gc Node<'gc>> {
        // ImportSpecifier:
        //   ImportedBinding
        //   IdentifierName as ImportedBinding
        let start_loc = self.cur_start();
        let _ = import_loc;

        let value_ident = self.gc.ctx().atom_table.atom_bytes(b"value");

        // P6: Flow type/typeof import specifier omitted (C++ 6955-7073).

        // Not attempting to parse a type identifier. C++ 7074-7109.
        if !self.check(TokenKind::identifier)
            && !self.lexer.token().is_res_word()
        {
            // C++ errorExpected(identifier, "in import specifier", ...).
            // note arg dropped per house style.
            self.error_cur("'identifier' expected in import specifier");
            return None;
        }
        let tok_start = self.lexer.token().start_loc();
        let tok_end = self.lexer.token().end_loc();
        let imported_name = self.lexer.token().get_res_word_or_identifier();
        let imported_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            imported_name,
            None,
            false,
        ));
        let imported = self.set_location(tok_start, tok_end, imported_node);
        // When there's no `as`, `imported` and `local` are the SAME node (C++
        // sets `local = imported`, the same pointer).
        let mut local = imported;
        let mut local_kind = self.cur_kind();
        self.advance(GrammarContext::AllowRegExp);

        // `as` is a contextual identifier (escape-insensitive). C++ 7093.
        if self.check_name(b"as") {
            self.advance(GrammarContext::AllowRegExp);
            if !self.check(TokenKind::identifier)
                && !self.lexer.token().is_res_word()
            {
                // note arg dropped per house style.
                self.error_cur("'identifier' expected in import specifier");
                return None;
            }
            let tok_start = self.lexer.token().start_loc();
            let tok_end = self.lexer.token().end_loc();
            let local_name = self.lexer.token().get_res_word_or_identifier();
            let local_node = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                local_name,
                None,
                false,
            ));
            local = self.set_location(tok_start, tok_end, local_node);
            local_kind = self.cur_kind();
            self.advance(GrammarContext::AllowRegExp);
        }

        // Only the local name must be parsed as a binding identifier.
        // We need to check for 'as' before knowing what the local name is.
        // Thus, we need to validate the binding identifier for the local name
        // after the fact. C++ 7112-7119.
        let local_id = match local {
            Node::Identifier(id) => id,
            _ => unreachable!("import specifier local is an Identifier"),
        };
        // Bind the interned bytes to an owned buffer so the immutable borrow of
        // the atom table ends before the `&mut self` validate call.
        let local_range = local_id.metadata.range.get();
        let id_bytes =
            self.gc.ctx().atom_table.bytes(local_id.name.get()).to_owned();
        if !self.validate_binding_identifier(local_range, &id_bytes, local_kind)
        {
            self.error_at(local_range, "Invalid local name for import");
        }

        // C++ 7121-7124.
        let node = Node::ImportSpecifier(ImportSpecifier::new(
            NodeMetadata::new(self.dummy_range()),
            imported,
            local,
            value_ident,
        ));
        Some(self.set_location(start_loc, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseExportDeclaration — 7127 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse an `export` declaration. Port of
    /// `JSParserImpl::parseExportDeclaration` (7127-7375).
    ///
    /// The Flow `export type` dispatch (C++ 7133-7137) and the Flow
    /// export-kind detection (C++ 7361-7368) are implemented; the Flow
    /// default-export forms (component/hook/enum/record, C++ 7209-7279) are
    /// P6, marked by a `// P6` comment at the omission site.
    pub(super) fn parse_export_declaration(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        debug_assert!(
            self.check(TokenKind::rw_export),
            "parseExportDeclaration requires 'export'"
        );
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        // The `value` export kind label.
        let value_ident = self.gc.ctx().atom_table.atom_bytes(b"value");

        // Flow `export type ...`: every such form dispatches to
        // parseExportTypeDeclarationFlow. C++ 7133-7137, gated on
        // getParseFlow(); `check(typeIdent_)` is escape-insensitive.
        if self.parse_flow() && self.check_name(b"type") {
            return self.parse_export_type_declaration_flow(start_loc);
        }

        if self.check_and_eat(TokenKind::star, GrammarContext::AllowRegExp) {
            // export * FromClause;
            // export * as IdentifierName FromClause;
            // C++ 7139-7184.
            let export_as: Option<&'gc Node<'gc>> = if self.check_name(b"as") {
                // export * as IdentifierName FromClause;
                //             ^
                // `as` is a contextual identifier (escape-insensitive). C++ 7143.
                self.advance(GrammarContext::AllowRegExp);
                if !self.check(TokenKind::identifier)
                    && !self.lexer.token().is_res_word()
                {
                    // C++ errorExpected(identifier, "in export clause", ...).
                    // note arg dropped per house style.
                    self.error_cur("identifier expected in export clause");
                    return None;
                }
                let tok_start = self.lexer.token().start_loc();
                let tok_end = self.lexer.token().end_loc();
                let name = self.lexer.token().get_res_word_or_identifier();
                let id = Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    name,
                    None,
                    false,
                ));
                let id = self.set_location(tok_start, tok_end, id);
                self.advance(GrammarContext::AllowRegExp);
                Some(id)
            } else {
                None
            };

            let source = self.parse_from_clause()?;
            if !self.eat_semi(false) {
                return None;
            }

            if let Some(export_as) = export_as {
                // C++ 7168-7179.
                let spec = Node::ExportNamespaceSpecifier(
                    ExportNamespaceSpecifier::new(
                        NodeMetadata::new(self.dummy_range()),
                        export_as,
                    ),
                );
                let spec = self.set_location(
                    start_loc,
                    self.lexer.prev_token_end(),
                    spec,
                );
                let node = Node::ExportNamedDeclaration(
                    ExportNamedDeclaration::new(
                        NodeMetadata::new(self.dummy_range()),
                        None,
                        NodeList::from_iter(self.gc, vec![spec]),
                        Some(source),
                        value_ident,
                    ),
                );
                return Some(self.set_location(
                    start_loc,
                    self.lexer.prev_token_end(),
                    node,
                ));
            }
            // C++ 7180-7184.
            let node = Node::ExportAllDeclaration(ExportAllDeclaration::new(
                NodeMetadata::new(self.dummy_range()),
                source,
                value_ident,
            ));
            return Some(self.set_location(
                start_loc,
                self.lexer.prev_token_end(),
                node,
            ));
        } else if self
            .check_and_eat(TokenKind::rw_default, GrammarContext::AllowRegExp)
        {
            // export default ... — C++ 7185-7293.
            let _g = self.check_recursion()?;
            // `export default async function` detection uses checkUnescaped
            // (escape-SENSITIVE) for `async`, matching C++ 7189.
            if self.check(TokenKind::rw_function)
                || (self.check_unescaped_name(b"async")
                    && self.check_async_function())
            {
                // export default HoistableDeclaration
                // Currently, the only hoistable declarations are functions.
                // C++ 7188-7199.
                let fun = self.parse_function_declaration(PARAM_DEFAULT)?;
                let node = Node::ExportDefaultDeclaration(
                    ExportDefaultDeclaration::new(
                        NodeMetadata::new(self.dummy_range()),
                        fun,
                    ),
                );
                return Some(self.set_location(
                    start_loc,
                    fun.range().end,
                    node,
                ));
            } else if self.check2(TokenKind::rw_class, TokenKind::at) {
                // C++ 7200-7208.
                let cls = self.parse_class_declaration(PARAM_DEFAULT)?;
                let node = Node::ExportDefaultDeclaration(
                    ExportDefaultDeclaration::new(
                        NodeMetadata::new(self.dummy_range()),
                        cls,
                    ),
                );
                return Some(self.set_location(
                    start_loc,
                    cls.range().end,
                    node,
                ));
            } else {
                // P6: Flow default exports (component/hook/enum/record,
                // C++ 7209-7279) omitted.
                //
                // export default AssignmentExpression ;
                // C++ 7280-7293.
                let expr = self.parse_assignment_expression(PARAM_IN, AllowTypedArrowFunction::Yes, CoverTypedParameters::Yes, None)?;
                if !self.eat_semi(false) {
                    return None;
                }
                let node = Node::ExportDefaultDeclaration(
                    ExportDefaultDeclaration::new(
                        NodeMetadata::new(self.dummy_range()),
                        expr,
                    ),
                );
                return Some(self.set_location(
                    start_loc,
                    self.lexer.prev_token_end(),
                    node,
                ));
            }
        } else if self.check(TokenKind::l_brace) {
            // export ExportClause FromClause ;
            // export ExportClause ;
            // C++ 7294-7331.
            let mut specifiers: Vec<&'gc Node<'gc>> = Vec::new();
            let mut invalids: Vec<SMRange> = Vec::new();

            if !self.parse_export_clause(&mut specifiers, &mut invalids) {
                return None;
            }

            // `from` is a contextual identifier (escape-insensitive). C++ 7306.
            let source = if self.check_name(b"from") {
                // export ExportClause FromClause ;
                Some(self.parse_from_clause()?)
            } else {
                // export ExportClause ;
                // ES9.0 15.2.3.1: when there is no FromClause, any ranges added
                // to invalids are actually invalid, and should be reported as
                // errors. C++ 7313-7321.
                for range in &invalids {
                    self.error_at(*range, "Invalid exported name");
                }
                None
            };

            if !self.eat_semi(false) {
                return None;
            }

            let node =
                Node::ExportNamedDeclaration(ExportNamedDeclaration::new(
                    NodeMetadata::new(self.dummy_range()),
                    None,
                    NodeList::from_iter(self.gc, specifiers),
                    source,
                    value_ident,
                ));
            return Some(self.set_location(
                start_loc,
                self.lexer.prev_token_end(),
                node,
            ));
        } else if self.check(TokenKind::rw_var) {
            // Could find another AssignmentExpression without hitting
            // PrimaryExpression. C++ 7332-7346.
            let _g = self.check_recursion()?;
            // export VariableStatement
            let var = self.parse_variable_statement(Param::default())?;
            let node =
                Node::ExportNamedDeclaration(ExportNamedDeclaration::new(
                    NodeMetadata::new(self.dummy_range()),
                    Some(var),
                    NodeList::empty(),
                    None,
                    value_ident,
                ));
            return Some(self.set_location(start_loc, var.range().end, node));
        }

        // export Declaration [~Yield]
        // C++ 7348-7374.

        if !self.check_declaration() {
            self.error_at(self.cur_range(), "expected declaration in export");
            return None;
        }

        let decl = self.parse_declaration(Param::default())?;

        // Flow export-kind detection: exporting a Flow type declaration makes
        // the export kind `type` instead of `value`. C++ 7361-7368, guarded
        // only by compile-time `#if HERMES_PARSE_FLOW` — there is no runtime
        // getParseFlow() check, so none here either (these node kinds can only
        // be produced when Flow parsing is enabled anyway).
        let kind = if matches!(
            decl,
            Node::TypeAlias(_)
                | Node::OpaqueType(_)
                | Node::DeclareTypeAlias(_)
                | Node::InterfaceDeclaration(_)
        ) {
            self.gc.ctx().atom_table.atom_bytes(b"type")
        } else {
            value_ident
        };

        let node = Node::ExportNamedDeclaration(ExportNamedDeclaration::new(
            NodeMetadata::new(self.dummy_range()),
            Some(decl),
            NodeList::empty(),
            None,
            kind,
        ));
        Some(self.set_location(start_loc, decl.range().end, node))
    }

    // -----------------------------------------------------------------------
    // parseExportClause — 7377 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a `{ a, b as c, ... }` export clause, appending each
    /// `ExportSpecifier` to `specifiers` and any potentially-invalid exported
    /// name ranges to `invalids`. Port of `JSParserImpl::parseExportClause`
    /// (7377-7407).
    fn parse_export_clause(
        &mut self,
        specifiers: &mut Vec<&'gc Node<'gc>>,
        invalids: &mut Vec<SMRange>,
    ) -> bool {
        // ExportClause:
        //   { }
        //   { ExportsList }
        //   { ExportsList , }
        debug_assert!(
            self.check(TokenKind::l_brace),
            "ExportClause requires '{{'"
        );
        let start_loc = self.advance(GrammarContext::AllowRegExp).start;

        while !self.check(TokenKind::r_brace) {
            // Read all the elements of the ExportsList.
            let spec = match self.parse_export_specifier(start_loc, invalids) {
                Some(s) => s,
                None => return false,
            };
            specifiers.push(spec);

            if !self.check_and_eat(TokenKind::comma, GrammarContext::AllowRegExp)
            {
                break;
            }
        }

        // C++ 7401-7406: NOTE grammar context AllowDiv. note arg dropped per
        // house style.
        self.eat(
            TokenKind::r_brace,
            GrammarContext::AllowDiv,
            " at end of export clause",
        )
    }

    // -----------------------------------------------------------------------
    // parseExportSpecifier — 7409 in JSParserImpl.cpp
    // -----------------------------------------------------------------------

    /// Parse a single export specifier (`a` or `a as b`). Port of
    /// `JSParserImpl::parseExportSpecifier` (7409-7467).
    fn parse_export_specifier(
        &mut self,
        _export_loc: SMLoc,
        invalids: &mut Vec<SMRange>,
    ) -> Option<&'gc Node<'gc>> {
        // ExportSpecifier:
        //   IdentifierName
        //   IdentifierName as IdentifierName
        if !self.check(TokenKind::identifier)
            && !self.lexer.token().is_res_word()
        {
            // C++ errorExpected(identifier, "in export clause", ...).
            // note arg dropped per house style.
            self.error_cur("identifier expected in export clause");
            return None;
        }

        // ES9.0 15.2.3.1 Early errors for ReferencedBindings in ExportClause.
        // Add potentially error-raising identifier ranges to the invalids list
        // here, and the owner of the invalids list will report the ranges as
        // errors if necessary. C++ 7425-7433. These contextual reserved-word
        // names use `check(UniqueString*)` (escape-insensitive) -> check_name.
        if self.lexer.token().is_res_word()
            || self.check_name(b"implements")
            || self.check_name(b"interface")
            || self.check_name(b"let")
            || self.check_name(b"package")
            || self.check_name(b"private")
            || self.check_name(b"protected")
            || self.check_name(b"public")
            || self.check_name(b"static")
        {
            invalids.push(self.cur_range());
        }

        let tok_start = self.lexer.token().start_loc();
        let tok_end = self.lexer.token().end_loc();
        let local_name = self.lexer.token().get_res_word_or_identifier();
        let local_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            local_name,
            None,
            false,
        ));
        let local = self.set_location(tok_start, tok_end, local_node);
        self.advance(GrammarContext::AllowRegExp);

        // `as` is a contextual identifier (escape-insensitive). C++ 7442.
        let exported = if self.check_name(b"as") {
            // IdentifierName as IdentifierName
            self.advance(GrammarContext::AllowRegExp);
            if !self.check(TokenKind::identifier)
                && !self.lexer.token().is_res_word()
            {
                // note arg dropped per house style.
                self.error_cur("identifier expected in export clause");
                return None;
            }
            let tok_start = self.lexer.token().start_loc();
            let tok_end = self.lexer.token().end_loc();
            let exported_name =
                self.lexer.token().get_res_word_or_identifier();
            let exported_node = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                exported_name,
                None,
                false,
            ));
            let e = self.set_location(tok_start, tok_end, exported_node);
            self.advance(GrammarContext::AllowRegExp);
            e
        } else {
            // IdentifierName
            local
        };

        // CRITICAL: ExportSpecifierNode(exported, local) — `exported` FIRST,
        // then `local` (C++ 7466; node.rs fields `exported, local`). This is
        // the OPPOSITE field order from ImportSpecifier(imported, local).
        // setLocation(local, exported, ...): start = local start, end =
        // exported end. C++ 7463-7466.
        let node = Node::ExportSpecifier(ExportSpecifier::new(
            NodeMetadata::new(self.dummy_range()),
            exported,
            local,
        ));
        Some(self.set_location(
            local.range().start,
            exported.range().end,
            node,
        ))
    }
}
