/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The Flow declaration gate (`parseFlowDeclaration`), `type`/`opaque type`
//! alias declarations, and `interface` declarations. Port of the declaration
//! entry points of `lib/Parser/JSParserImpl-flow.cpp`.

use ast::node::{
    DeclareInterface, DeclareOpaqueType, DeclareTypeAlias,
    ExportNamedDeclaration, Identifier, InterfaceDeclaration,
    InterfaceExtends, Node, OpaqueType, TypeAlias,
};
use ast::node_child::{NodeList, NodeMetadata};
use support::location::SMLoc;

use crate::js::JSParserImpl;
use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

use super::{
    AllowAnonFunctionType, AllowProtoProperty, AllowSpreadProperty,
    AllowStaticProperty, TypeAliasKind,
};

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    // -----------------------------------------------------------------------
    // parseFlowDeclaration — 21 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a Flow declaration (`type`/`opaque type`/`interface`/`enum`/...).
    /// Port of `JSParserImpl::parseFlowDeclaration` (flow.cpp:21-93).
    /// Reached from `parse_declaration` only when `check_declaration()` is
    /// true, so (like the C++) it never falls through: `None` means an error
    /// was already reported.
    pub(in crate::js) fn parse_flow_declaration(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 22.
        assert!(self.check_declaration(), "invalid start for Flow declaration");
        let start = self.cur_start();

        // P6: component/hook declarations (gated on
        // getParseFlowComponentSyntax(), C++ 25-45) and record declarations
        // (gated on getParseFlowRecords(), C++ 47-49) — the Rust Context does
        // not implement those flags yet.

        // C++ 51-56.
        if self.check(TokenKind::rw_enum) {
            // P6: parseEnumDeclarationFlow (C++ 52-55).
            self.error_cur("Flow enum declarations are unsupported (parser phase P6)");
            return None;
        }

        // C++ 58-62. `checkAndEat(<ident>)` advances with the default
        // GrammarContext::AllowRegExp. NOTE: `check_declaration()` does not
        // accept `declare` as a declaration start, so (like the C++) the
        // Declare kind is unreachable through this gate — the `declare`
        // statement routing (parseDeclareFLow, C++ 95-110) lands in P6.
        let mut kind = TypeAliasKind::None;
        if self.check_name(b"declare") {
            self.advance(GrammarContext::AllowRegExp);
            kind = TypeAliasKind::Declare;
        } else if self.check_name(b"opaque") {
            self.advance(GrammarContext::AllowRegExp);
            kind = TypeAliasKind::Opaque;
        }

        // C++ 64-68.
        if kind == TypeAliasKind::Declare
            && !(self.check_name(b"type")
                || self.check_name(b"interface")
                || self.check(TokenKind::rw_interface))
        {
            self.error_cur("invalid token in type declaration");
            return None;
        }
        // C++ 69-72.
        if kind == TypeAliasKind::Opaque && !self.check_name(b"type") {
            self.error_cur("invalid token in opaque type declaration");
            return None;
        }

        // C++ 74-79.
        if self.check_name(b"type") {
            self.advance(GrammarContext::AllowRegExp);
            return self.parse_type_alias_flow(start, kind);
        }

        // C++ 81-87.
        if self.check_name(b"interface") || self.check(TokenKind::rw_interface) {
            return self.parse_interface_declaration_flow(
                if kind == TypeAliasKind::Declare {
                    Some(start)
                } else {
                    None
                },
            );
        }

        // C++ 89-92.
        unreachable!("checkDeclaration() returned true without 'type' or 'interface'");
    }

    // -----------------------------------------------------------------------
    // parseTypeAliasFlow — 1981 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a type alias, with `type` already consumed and `start` at the
    /// start of the declaration. Port of `JSParserImpl::parseTypeAliasFlow`
    /// (flow.cpp:1981-2071). All four `TypeAliasKind` paths are implemented;
    /// the `Declare`/`DeclareOpaque` kinds become reachable with the
    /// `declare` statement routing (parseDeclareFLow) in P6.
    pub(super) fn parse_type_alias_flow(
        &mut self,
        start: SMLoc,
        kind: TypeAliasKind,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 1984-1987.
        if !self.need(TokenKind::identifier, " in type alias") {
            return None;
        }

        // C++ 1988-1993.
        let id_range = self.cur_range();
        let id_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_identifier(),
            None,
            false,
        ));
        let id = self.set_location(id_range.start, id_range.end, id_node);
        self.advance(GrammarContext::Type);

        // C++ 1995-2002.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_params = Some(self.parse_type_params_flow()?);
        }

        // C++ 2004-2026: the Opaque/DeclareOpaque `super`/`extends` bounds
        // and the legacy `: Supertype` (only when neither bound was given).
        let mut lower_bound: Option<&'gc Node<'gc>> = None;
        let mut upper_bound: Option<&'gc Node<'gc>> = None;
        let mut legacy_supertype: Option<&'gc Node<'gc>> = None;
        if kind == TypeAliasKind::Opaque || kind == TypeAliasKind::DeclareOpaque
        {
            // C++ 2007-2012: the lower bound is a UNION type annotation, not
            // a full one.
            if self.check_and_eat(TokenKind::rw_super, GrammarContext::Type) {
                lower_bound = Some(self.parse_union_type_annotation_flow()?);
            }
            // C++ 2013-2018.
            if self.check_and_eat(TokenKind::rw_extends, GrammarContext::Type)
            {
                upper_bound = Some(self.parse_type_annotation_flow(
                    None,
                    AllowAnonFunctionType::Yes,
                )?);
            }
            // C++ 2019-2026.
            if lower_bound.is_none()
                && upper_bound.is_none()
                && self.check_and_eat(TokenKind::colon, GrammarContext::Type)
            {
                legacy_supertype = Some(self.parse_type_annotation_flow(
                    None,
                    AllowAnonFunctionType::Yes,
                )?);
            }
        }

        // C++ 2028-2042: DeclareOpaque has no `= T` right side.
        let mut right: Option<&'gc Node<'gc>> = None;
        if kind != TypeAliasKind::DeclareOpaque {
            if !self.eat(TokenKind::equal, GrammarContext::Type, " in type alias")
            {
                return None;
            }
            right = Some(
                self.parse_type_annotation_flow(
                    None,
                    AllowAnonFunctionType::Yes,
                )?,
            );
        }

        // C++ 2043-2044.
        if !self.eat_semi(false) {
            return None;
        }

        // C++ 2046-2070.
        let end = self.lexer.prev_token_end();
        match kind {
            // C++ 2046-2052.
            TypeAliasKind::DeclareOpaque => {
                let node = Node::DeclareOpaqueType(DeclareOpaqueType::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    type_params,
                    right, // impltype (always None here)
                    lower_bound,
                    upper_bound,
                    legacy_supertype,
                ));
                Some(self.set_location(start, end, node))
            }
            // C++ 2053-2058.
            TypeAliasKind::Declare => {
                let node = Node::DeclareTypeAlias(DeclareTypeAlias::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    type_params,
                    right.expect("non-DeclareOpaque alias has a right side"),
                ));
                Some(self.set_location(start, end, node))
            }
            // C++ 2059-2065.
            TypeAliasKind::Opaque => {
                let node = Node::OpaqueType(OpaqueType::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    type_params,
                    // impltype
                    right.expect("non-DeclareOpaque alias has a right side"),
                    lower_bound,
                    upper_bound,
                    legacy_supertype,
                ));
                Some(self.set_location(start, end, node))
            }
            // C++ 2066-2070.
            TypeAliasKind::None => {
                let node = Node::TypeAlias(TypeAlias::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    type_params,
                    right.expect("non-DeclareOpaque alias has a right side"),
                ));
                Some(self.set_location(start, end, node))
            }
        }
    }

    // -----------------------------------------------------------------------
    // parseInterfaceDeclarationFlow — 2073 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse an interface declaration, with the current token at the
    /// `interface` keyword (`rw_interface` in strict mode, a plain identifier
    /// spelled "interface" in loose mode). Port of
    /// `JSParserImpl::parseInterfaceDeclarationFlow` (flow.cpp:2073-2118).
    ///
    /// \param declare_start if `Some`, this is a `declare interface` and the
    ///   resulting `DeclareInterface` node spans from it (the `declare`
    ///   statement routing that passes `Some` lands in P6).
    pub(super) fn parse_interface_declaration_flow(
        &mut self,
        declare_start: Option<SMLoc>,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 2075.
        debug_assert!(
            self.check(TokenKind::rw_interface)
                || self.check_name(b"interface"),
            "must be at 'interface'"
        );
        // C++ 2076.
        let start = self.advance(GrammarContext::Type).start;

        // C++ 2078-2084.
        if !self.need(TokenKind::identifier, " in interface declaration") {
            return None;
        }

        // C++ 2086-2091.
        let id_range = self.cur_range();
        let id_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_identifier(),
            None,
            false,
        ));
        let id = self.set_location(id_range.start, id_range.end, id_node);
        self.advance(GrammarContext::Type);

        // C++ 2093-2099.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_params = Some(self.parse_type_params_flow()?);
        }

        // C++ 2101-2104.
        let mut extends: Vec<&'gc Node<'gc>> = Vec::new();
        let body = self.parse_interface_tail_flow(&mut extends)?;

        // C++ 2106-2117: the end location is the body node's end.
        let end = body.metadata().range.get().end;
        if let Some(declare_start) = declare_start {
            let node = Node::DeclareInterface(DeclareInterface::new(
                NodeMetadata::new(self.dummy_range()),
                id,
                type_params,
                NodeList::from_iter(self.gc, extends),
                body,
            ));
            Some(self.set_location(declare_start, end, node))
        } else {
            let node = Node::InterfaceDeclaration(InterfaceDeclaration::new(
                NodeMetadata::new(self.dummy_range()),
                id,
                type_params,
                NodeList::from_iter(self.gc, extends),
                body,
            ));
            Some(self.set_location(start, end, node))
        }
    }

    /// Parse the `extends`-clause-and-body tail shared by interface
    /// declarations and `interface` type annotations, pushing the
    /// `InterfaceExtends` entries into `extends` and returning the
    /// `ObjectTypeAnnotation` body. Port of
    /// `JSParserImpl::parseInterfaceTailFlow` (flow.cpp:2120-2141; the C++
    /// also takes the interface's start location, used only for the error
    /// notes that the Rust `need` does not carry).
    pub(super) fn parse_interface_tail_flow(
        &mut self,
        extends: &mut Vec<&'gc Node<'gc>>,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 2123: a bare `checkAndEat` — the default GrammarContext
        // (AllowRegExp), NOT Type; deliberate.
        if self.check_and_eat(TokenKind::rw_extends, GrammarContext::AllowRegExp)
        {
            // C++ 2124-2134: a do-while.
            loop {
                if !self.need(TokenKind::identifier, " in extends clause") {
                    return None;
                }
                if !self.parse_interface_extends(extends) {
                    return None;
                }
                if !self.check_and_eat(TokenKind::comma, GrammarContext::Type)
                {
                    break;
                }
            }
        }

        // C++ 2136-2137.
        if !self.need(TokenKind::l_brace, " in interface") {
            return None;
        }

        // C++ 2139-2140: spread properties are not allowed in interface
        // bodies.
        self.parse_object_type_annotation_flow(
            AllowProtoProperty::No,
            AllowStaticProperty::No,
            AllowSpreadProperty::No,
        )
    }

    /// Parse one entry of an interface `extends` clause: a generic type
    /// reference, unwrapped into an `InterfaceExtends` node spanning the same
    /// range, pushed onto `extends`. Returns false if an error was reported.
    /// Port of `JSParserImpl::parseInterfaceExtends` (flow.cpp:2143-2157; the
    /// C++ also takes an unused start location).
    fn parse_interface_extends(
        &mut self,
        extends: &mut Vec<&'gc Node<'gc>>,
    ) -> bool {
        // C++ 2146.
        debug_assert!(self.check(TokenKind::identifier));
        // C++ 2147-2150.
        let Some(generic) = self.parse_generic_type_flow() else {
            return false;
        };
        let Node::GenericTypeAnnotation(g) = generic else {
            unreachable!("parse_generic_type_flow returns a GenericTypeAnnotation")
        };
        // C++ 2151-2155: the InterfaceExtends node spans exactly the generic.
        let range = generic.metadata().range.get();
        let node = Node::InterfaceExtends(InterfaceExtends::new(
            NodeMetadata::new(self.dummy_range()),
            g.id,
            g.type_parameters,
        ));
        extends.push(self.set_location(range.start, range.end, node));
        true
    }

    // -----------------------------------------------------------------------
    // parseExportTypeDeclarationFlow — 2498 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse the tail of `export type ...` with the cursor at `type` and
    /// `start_loc` at `export`. Port of
    /// `JSParserImpl::parseExportTypeDeclarationFlow` (flow.cpp:2498-2575).
    ///
    /// The `export type A = ...` alias form (flow.cpp:2557-2566) is
    /// implemented; the `export type * FromClause;` re-export
    /// (flow.cpp:2503-2518) and the `export type { ... } [FromClause];`
    /// specifier-clause form (flow.cpp:2520-2556) are P6 — they report an
    /// honest deferral error instead of silently mis-parsing.
    pub(in crate::js) fn parse_export_type_declaration_flow(
        &mut self,
        start_loc: SMLoc,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 2500-2501.
        debug_assert!(self.check_name(b"type"));
        let type_ident_loc = self.advance(GrammarContext::AllowRegExp).start;

        if self.check(TokenKind::star) {
            // P6: export type * FromClause; (flow.cpp:2503-2518).
            self.error_cur(
                "'export type *' re-exports are unsupported (parser phase P6)",
            );
            return None;
        }

        if self.check(TokenKind::l_brace) {
            // P6: export type ExportClause [FromClause]; (flow.cpp:2520-2556).
            self.error_cur(
                "'export type {' export clauses are unsupported (parser phase P6)",
            );
            return None;
        }

        // C++ 2557-2566.
        if self.check(TokenKind::identifier) {
            let alias = self
                .parse_type_alias_flow(type_ident_loc, TypeAliasKind::None)?;
            let type_ident = self.gc.ctx().atom_table.atom_bytes(b"type");
            let node =
                Node::ExportNamedDeclaration(ExportNamedDeclaration::new(
                    NodeMetadata::new(self.dummy_range()),
                    Some(alias),
                    NodeList::empty(),
                    None,
                    type_ident,
                ));
            return Some(self.set_location(
                start_loc,
                alias.range().end,
                node,
            ));
        }

        // C++ 2569-2574: errorExpected(star, l_brace, identifier,
        // "in export type declaration", ...). note arg dropped per house
        // style.
        self.error_expected3(
            TokenKind::star,
            TokenKind::l_brace,
            TokenKind::identifier,
            " in export type declaration",
        );
        None
    }
}
