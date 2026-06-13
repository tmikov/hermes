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
    BigIntLiteral, BooleanLiteral, DeclareEnum, DeclareInterface,
    DeclareOpaqueType, DeclareTypeAlias, EnumBigIntBody, EnumBigIntMember,
    EnumBooleanBody, EnumBooleanMember, EnumDeclaration, EnumDefaultedMember,
    EnumNumberBody, EnumNumberMember, EnumStringBody, EnumStringMember,
    EnumSymbolBody, ExportNamedDeclaration, Identifier, InterfaceDeclaration,
    InterfaceExtends, Node, NumericLiteral, OpaqueType, StringLiteral,
    TypeAlias,
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

/// The kind of a Flow `enum`. Port of `JSParserImpl::EnumKind`
/// (JSParserImpl.h:1550-1556 — keep the SAME variant order).
#[derive(Clone, Copy, PartialEq, Eq)]
enum EnumKind {
    String,
    Number,
    BigInt,
    Boolean,
    Symbol,
}

/// The user-facing name of an enum kind. Port of
/// `JSParserImpl::enumKindStrFlow` (JSParserImpl.h:1558-1572).
fn enum_kind_str_flow(kind: EnumKind) -> &'static str {
    match kind {
        EnumKind::String => "string",
        EnumKind::Number => "number",
        EnumKind::BigInt => "bigint",
        EnumKind::Boolean => "boolean",
        EnumKind::Symbol => "symbol",
    }
}

/// The enum kind implied by a member node, or `None` for a defaulted member.
/// Port of `JSParserImpl::getMemberEnumKindFlow` (JSParserImpl.h:1574-1587).
fn get_member_enum_kind_flow(member: &Node<'_>) -> Option<EnumKind> {
    match member {
        Node::EnumStringMember(_) => Some(EnumKind::String),
        Node::EnumNumberMember(_) => Some(EnumKind::Number),
        Node::EnumBigIntMember(_) => Some(EnumKind::BigInt),
        Node::EnumBooleanMember(_) => Some(EnumKind::Boolean),
        _ => None,
    }
}

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
            // C++ 52-55. The `declare enum` / `export declare enum` routing
            // (which calls this with declare=true) lands in P6.6.
            return self.parse_enum_declaration_flow(start, /*declare*/ false);
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

    // -----------------------------------------------------------------------
    // parseEnumDeclarationFlow — 5148 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a Flow `enum` declaration, with the cursor at `enum` and `start`
    /// at the start of the declaration. Port of
    /// `JSParserImpl::parseEnumDeclarationFlow` (flow.cpp:5148-5205).
    ///
    /// \param declare whether this is a `declare enum` (the `declare` routing
    ///   that passes `true` lands in P6.6).
    pub(in crate::js) fn parse_enum_declaration_flow(
        &mut self,
        start: SMLoc,
        declare: bool,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 5151-5152.
        debug_assert!(self.check(TokenKind::rw_enum));
        self.advance(GrammarContext::AllowRegExp);

        // C++ 5154-5161: errorExpected(identifier, "in enum declaration",
        // "start of declaration", start) — the single-token form renders
        // "'identifier' expected in enum declaration"; `need` matches it
        // exactly (note args dropped per house style).
        if !self.need(TokenKind::identifier, " in enum declaration") {
            return None;
        }
        // C++ 5162-5166.
        let id_range = self.cur_range();
        let id_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_identifier(),
            None,
            false,
        ));
        let id = self.set_location(id_range.start, id_range.end, id_node);
        // C++ 5167.
        self.advance(GrammarContext::Type);

        // C++ 5169-5185: an optional `of <kind>` explicit type.
        let mut opt_kind: Option<EnumKind> = None;
        let mut explicit_type_start: Option<SMLoc> = None;
        if self.check_name(b"of") {
            explicit_type_start = Some(self.advance(GrammarContext::AllowRegExp).start);

            // C++ 5174-5184: the five contextual kind idents. `checkAndEat` of
            // a contextual ident → `check_name` + advance (default
            // GrammarContext::AllowRegExp).
            if self.check_name(b"string") {
                self.advance(GrammarContext::AllowRegExp);
                opt_kind = Some(EnumKind::String);
            } else if self.check_name(b"number") {
                self.advance(GrammarContext::AllowRegExp);
                opt_kind = Some(EnumKind::Number);
            } else if self.check_name(b"bigint") {
                self.advance(GrammarContext::AllowRegExp);
                opt_kind = Some(EnumKind::BigInt);
            } else if self.check_name(b"boolean") {
                self.advance(GrammarContext::AllowRegExp);
                opt_kind = Some(EnumKind::Boolean);
            } else if self.check_name(b"symbol") {
                self.advance(GrammarContext::AllowRegExp);
                opt_kind = Some(EnumKind::Symbol);
            }
        }

        // C++ 5187-5192.
        if !self.need(TokenKind::l_brace, " in enum declaration") {
            return None;
        }

        // C++ 5194-5196.
        let body = self.parse_enum_body_flow(opt_kind, explicit_type_start)?;

        // C++ 5198-5204.
        let end = body.range().end;
        if declare {
            let node = Node::DeclareEnum(DeclareEnum::new(
                NodeMetadata::new(self.dummy_range()),
                id,
                body,
            ));
            return Some(self.set_location(start, end, node));
        }
        let node = Node::EnumDeclaration(EnumDeclaration::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            body,
        ));
        Some(self.set_location(start, end, node))
    }

    // -----------------------------------------------------------------------
    // parseEnumBodyFlow — 5207 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse the `{ ... }` body of an enum, with the cursor at `{`. `opt_kind`
    /// is the explicit kind from `of <kind>` (if any) and `explicit_type_start`
    /// is the location of that explicit type (if any). Port of
    /// `JSParserImpl::parseEnumBodyFlow` (flow.cpp:5207-5352).
    fn parse_enum_body_flow(
        &mut self,
        opt_kind: Option<EnumKind>,
        explicit_type_start: Option<SMLoc>,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 5210-5211.
        debug_assert!(self.check(TokenKind::l_brace));
        let mut start = self.advance(GrammarContext::AllowRegExp).start;

        // The kind may be inferred from the members if it was not explicit.
        let mut opt_kind = opt_kind;

        // C++ 5213-5261.
        let mut members: Vec<&'gc Node<'gc>> = Vec::new();
        let mut has_unknown_members = false;
        while !self.check(TokenKind::r_brace) {
            // C++ 5216-5227: the inexact `...`, which must come last.
            if self.check(TokenKind::dotdotdot) {
                let dotdotdot_loc =
                    self.advance(GrammarContext::Type).start;
                if !self.check(TokenKind::r_brace) {
                    self.error_at_loc(
                        dotdotdot_loc,
                        "The `...` must come after all enum members. \
                         Move it to the end of the enum body.",
                    );
                    return None;
                }
                has_unknown_members = true;
                break;
            }
            // C++ 5228-5233.
            if !self.need(TokenKind::identifier, " in enum declaration") {
                return None;
            }

            // C++ 5235-5239.
            let member = self.parse_enum_member_flow()?;
            let opt_member_kind = get_member_enum_kind_flow(member);

            // C++ 5241-5256.
            if let Some(kind) = opt_kind {
                // We've already figured out the type of the enum, so ensure
                // that the new member is compatible with this.
                if let Some(member_kind) = opt_member_kind {
                    if kind != member_kind {
                        let range = member.range();
                        self.error_at(
                            range,
                            &format!(
                                "cannot use {} initializer in {} enum",
                                enum_kind_str_flow(member_kind),
                                enum_kind_str_flow(kind),
                            ),
                        );
                        self.lexer.get_source_mgr_mut().note_at(
                            start,
                            None,
                            "start of enum body",
                            support::diag::Subsystem::Parser,
                        );
                        return None;
                    }
                }
            } else {
                opt_kind = opt_member_kind;
            }

            // C++ 5258-5260.
            members.push(member);
            if !self.check_and_eat(TokenKind::comma, GrammarContext::AllowRegExp)
            {
                break;
            }
        }

        // C++ 5263-5292.
        if !members.is_empty() {
            // Ensure that enum members use initializers consistently.
            // This is vacuously true when `members` is empty, so just make
            // sure all members use initializers iff the first member does.
            let uses_initializers = !matches!(
                members[0],
                Node::EnumDefaultedMember(_)
            );
            for member in &members {
                let member_uses =
                    !matches!(member, Node::EnumDefaultedMember(_));
                if uses_initializers != member_uses {
                    let range = member.range();
                    self.error_at(
                        range,
                        "enum members need to consistently either all use \
                         initializers, or use no initializers",
                    );
                    let first_range = members[0].range();
                    self.lexer.get_source_mgr_mut().note_range(
                        first_range,
                        "first enum member",
                        support::diag::Subsystem::Parser,
                    );
                    return None;
                }
            }

            // C++ 5283-5291.
            if !uses_initializers {
                // It's only legal to use defaulted members for string and
                // symbol enums, because other kinds of enums can't infer
                // values from names.
                if let Some(kind) = opt_kind {
                    if kind != EnumKind::String && kind != EnumKind::Symbol {
                        self.error_at_loc(
                            start,
                            "number and boolean enums must use initializers",
                        );
                        return None;
                    }
                }
            }
        }

        // C++ 5294-5301.
        let end = self.lexer.token().end_loc();
        if !self.eat(
            TokenKind::r_brace,
            GrammarContext::AllowRegExp,
            " in enum body",
        ) {
            return None;
        }

        // C++ 5303-5306.
        let has_explicit_type = explicit_type_start.is_some();
        if let Some(ets) = explicit_type_start {
            start = ets;
        }

        let members = NodeList::from_iter(self.gc, members);

        // C++ 5308-5314: an untyped/empty enum is a string-body enum.
        let Some(kind) = opt_kind else {
            let node = Node::EnumStringBody(EnumStringBody::new(
                NodeMetadata::new(self.dummy_range()),
                members,
                has_explicit_type,
                has_unknown_members,
            ));
            return Some(self.set_location(start, end, node));
        };

        // C++ 5316-5351: there are different node kinds per enum kind.
        let node = match kind {
            EnumKind::String => Node::EnumStringBody(EnumStringBody::new(
                NodeMetadata::new(self.dummy_range()),
                members,
                has_explicit_type,
                has_unknown_members,
            )),
            EnumKind::Number => Node::EnumNumberBody(EnumNumberBody::new(
                NodeMetadata::new(self.dummy_range()),
                members,
                has_explicit_type,
                has_unknown_members,
            )),
            EnumKind::BigInt => Node::EnumBigIntBody(EnumBigIntBody::new(
                NodeMetadata::new(self.dummy_range()),
                members,
                has_explicit_type,
                has_unknown_members,
            )),
            EnumKind::Boolean => Node::EnumBooleanBody(EnumBooleanBody::new(
                NodeMetadata::new(self.dummy_range()),
                members,
                has_explicit_type,
                has_unknown_members,
            )),
            EnumKind::Symbol => {
                // C++ 5343-5344: symbol enums can only be made via explicit
                // type. EnumSymbolBody has no `explicit_type` field.
                debug_assert!(
                    has_explicit_type,
                    "symbol enums can only be made via explicit type"
                );
                Node::EnumSymbolBody(EnumSymbolBody::new(
                    NodeMetadata::new(self.dummy_range()),
                    members,
                    has_unknown_members,
                ))
            }
        };
        Some(self.set_location(start, end, node))
    }

    // -----------------------------------------------------------------------
    // parseEnumMemberFlow — 5354 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse one enum member, with the cursor at the member's identifier. Port
    /// of `JSParserImpl::parseEnumMemberFlow` (flow.cpp:5354-5432).
    fn parse_enum_member_flow(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 5355-5360.
        debug_assert!(self.check(TokenKind::identifier));
        let id_range = self.cur_range();
        let id_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_identifier(),
            None,
            false,
        ));
        let id = self.set_location(id_range.start, id_range.end, id_node);
        // C++ 5361.
        self.advance(GrammarContext::AllowRegExp);

        // C++ 5363-5428.
        let member: &'gc Node<'gc>;
        if self.check_and_eat(TokenKind::equal, GrammarContext::AllowRegExp) {
            // Parse initializer.
            let tok_range = self.cur_range();
            if self.check2(TokenKind::rw_true, TokenKind::rw_false) {
                // C++ 5366-5372.
                let value = self.check(TokenKind::rw_true);
                let init_node = Node::BooleanLiteral(BooleanLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    value,
                ));
                let init = self.set_location(
                    tok_range.start,
                    tok_range.end,
                    init_node,
                );
                let m = Node::EnumBooleanMember(EnumBooleanMember::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    init,
                ));
                member = self.set_location(
                    id.range().start,
                    tok_range.end,
                    m,
                );
            } else if self.check(TokenKind::string_literal) {
                // C++ 5373-5379.
                let value = self.lexer.token().get_string_literal();
                let init_node = Node::StringLiteral(StringLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    value,
                ));
                let init = self.set_location(
                    tok_range.start,
                    tok_range.end,
                    init_node,
                );
                let m = Node::EnumStringMember(EnumStringMember::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    init,
                ));
                member = self.set_location(
                    id.range().start,
                    tok_range.end,
                    m,
                );
            } else if self.check(TokenKind::minus) {
                // C++ 5380-5397: a negated numeric literal.
                let minus_start = self.cur_start();
                self.advance(GrammarContext::AllowRegExp);
                if self.check(TokenKind::numeric_literal) {
                    let num_range = self.cur_range();
                    // Negate the literal.
                    let value = -self.lexer.token().get_numeric_literal();
                    let init_node = Node::NumericLiteral(NumericLiteral::new(
                        NodeMetadata::new(self.dummy_range()),
                        value,
                    ));
                    let init = self.set_location(
                        minus_start,
                        num_range.end,
                        init_node,
                    );
                    let m = Node::EnumNumberMember(EnumNumberMember::new(
                        NodeMetadata::new(self.dummy_range()),
                        id,
                        init,
                    ));
                    member = self.set_location(
                        id.range().start,
                        num_range.end,
                        m,
                    );
                } else {
                    // C++ 5390-5396: errorExpected(numeric_literal,
                    // "in negated enum member initializer", ...) — single-token
                    // form rendered via `need` (note args dropped per house
                    // style). `need` reports at the current token without
                    // consuming, matching errorExpected.
                    self.need(
                        TokenKind::numeric_literal,
                        " in negated enum member initializer",
                    );
                    return None;
                }
            } else if self.check(TokenKind::numeric_literal) {
                // C++ 5398-5404.
                let value = self.lexer.token().get_numeric_literal();
                let init_node = Node::NumericLiteral(NumericLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    value,
                ));
                let init = self.set_location(
                    tok_range.start,
                    tok_range.end,
                    init_node,
                );
                let m = Node::EnumNumberMember(EnumNumberMember::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    init,
                ));
                member = self.set_location(
                    id.range().start,
                    tok_range.end,
                    m,
                );
            } else if self.check(TokenKind::bigint_literal) {
                // C++ 5405-5411.
                let bigint = self.lexer.token().get_bigint_literal();
                let init_node = Node::BigIntLiteral(BigIntLiteral::new(
                    NodeMetadata::new(self.dummy_range()),
                    bigint,
                ));
                let init = self.set_location(
                    tok_range.start,
                    tok_range.end,
                    init_node,
                );
                let m = Node::EnumBigIntMember(EnumBigIntMember::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    init,
                ));
                member = self.set_location(
                    id.range().start,
                    tok_range.end,
                    m,
                );
            } else {
                // C++ 5412-5422: errorExpected over the five literal token
                // kinds. The four-token wrapper plus rw_false covers the set
                // {rw_true, rw_false, string_literal, numeric_literal,
                // bigint_literal}; note args dropped per house style.
                self.error_expected_enum_member_init();
                return None;
            }
            // C++ 5424.
            self.advance(GrammarContext::AllowRegExp);
        } else {
            // C++ 5425-5427.
            let m = Node::EnumDefaultedMember(EnumDefaultedMember::new(
                NodeMetadata::new(self.dummy_range()),
                id,
            ));
            member = self.set_location(id.range().start, id.range().end, m);
        }

        // C++ 5430-5431.
        Some(member)
    }

    /// Report the five-token `errorExpected` for an enum member initializer
    /// (`true`, `false`, a string, a number, or a bigint). Port of the
    /// initializer-list `errorExpected` at flow.cpp:5412-5422. The Rust
    /// `error_expected*` family tops out at four tokens, so render the
    /// five-token list directly to stay byte-faithful to the C++ message.
    fn error_expected_enum_member_init(&mut self) {
        use crate::token_kinds::token_kind_str;
        let msg = format!(
            "'{}', '{}', '{}', '{}' or '{}' expected in enum member initializer",
            token_kind_str(TokenKind::rw_true),
            token_kind_str(TokenKind::rw_false),
            token_kind_str(TokenKind::string_literal),
            token_kind_str(TokenKind::numeric_literal),
            token_kind_str(TokenKind::bigint_literal),
        );
        self.error_cur(&msg);
    }
}
