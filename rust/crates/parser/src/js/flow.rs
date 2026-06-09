/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Flow type-grammar parsing for the JS parser. Port of
//! `lib/Parser/JSParserImpl-flow.cpp`.
//!
//! P5.0 scope: the Flow declaration gate (`parseFlowDeclaration`), the plain
//! `type X = T;` alias pipeline (`parseTypeAliasFlow` →
//! `parseTypeAnnotationFlow` → ... → `parsePrimaryTypeAnnotationFlow`), and
//! the primitive/literal primary type annotations. Every other production
//! emits an honest "unsupported (parser phase P5.x)" error at the marked
//! site; the later P5 sub-tasks (P5.1 core type grammar, P5.2 objects /
//! functions / type params, P5.3 interfaces / declare / opaque, P6
//! enum / component / hook / record / match) replace those markers with the
//! real grammar.

use ast::node::{
    AnyTypeAnnotation, BigIntLiteralTypeAnnotation, BigIntTypeAnnotation,
    BooleanLiteralTypeAnnotation, BooleanTypeAnnotation, EmptyTypeAnnotation,
    ExistsTypeAnnotation, Identifier, MixedTypeAnnotation, NeverTypeAnnotation,
    Node, NullLiteralTypeAnnotation, NumberLiteralTypeAnnotation,
    NumberTypeAnnotation, StringLiteralTypeAnnotation, StringTypeAnnotation,
    SymbolTypeAnnotation, TypeAlias, TypeAnnotation, UndefinedTypeAnnotation,
    UnknownTypeAnnotation, VoidTypeAnnotation,
};
use ast::node_child::{NodeLabel, NodeMetadata};
use support::location::SMLoc;

use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

use super::JSParserImpl;

/// Which alias declaration form is being parsed.
/// Port of `JSParserImpl::TypeAliasKind` (JSParserImpl.h:1390).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)] // DeclareOpaque is constructed by parseDeclareFLow (P5.3).
pub(super) enum TypeAliasKind {
    None,
    Declare,
    Opaque,
    DeclareOpaque,
}

/// Whether an anonymous function type (`T => U` without parentheses) is
/// allowed when parsing a type annotation.
/// Port of `JSParserImpl::AllowAnonFunctionType` (JSParserImpl.h:1207).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)] // `No` is passed from P5.1 on (e.g. function params).
pub(super) enum AllowAnonFunctionType {
    No,
    Yes,
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
    pub(super) fn parse_flow_declaration(&mut self) -> Option<&'gc Node<'gc>> {
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
        // GrammarContext::AllowRegExp.
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
            // P5.3: parseInterfaceDeclarationFlow (C++ 82-86).
            self.error_cur("interface declarations are unsupported (parser phase P5.3)");
            return None;
        }

        // C++ 89-92.
        unreachable!("checkDeclaration() returned true without 'type' or 'interface'");
    }

    // -----------------------------------------------------------------------
    // parseTypeAliasFlow — 1981 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a type alias, with `type` already consumed and `start` at the
    /// start of the declaration. Port of `JSParserImpl::parseTypeAliasFlow`
    /// (flow.cpp:1981-2071). P5.0 implements only the `TypeAliasKind::None`
    /// path (a plain `TypeAlias` node).
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
        let type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            // P5.2: parseTypeParamsFlow (C++ 1997-2001).
            self.error_cur("type parameters unsupported (parser phase P5.2)");
            return None;
        }

        // C++ 2004-2026: the Opaque/DeclareOpaque `super`/`extends`/legacy-`:`
        // bounds; C++ 2047-2065: the DeclareOpaqueType/DeclareTypeAlias/
        // OpaqueType result nodes.
        if kind != TypeAliasKind::None {
            // P5.3: declare/opaque type aliases.
            self.error_cur(
                "declare/opaque type aliases are unsupported (parser phase P5.3)",
            );
            return None;
        }

        // C++ 2029-2041 (the `kind != DeclareOpaque` path).
        if !self.eat(TokenKind::equal, GrammarContext::Type, " in type alias") {
            return None;
        }
        let right =
            self.parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

        // C++ 2043-2044.
        if !self.eat_semi(false) {
            return None;
        }

        // C++ 2066-2070.
        let node = Node::TypeAlias(TypeAlias::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            type_params,
            right,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseTypeAnnotationFlow — 3078 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a type annotation.
    /// Port of `JSParserImpl::parseTypeAnnotationFlow` (flow.cpp:3078-3094).
    ///
    /// \param wrapped_start if `Some`, the result is wrapped in a
    ///   `TypeAnnotation` node spanning from it to the previous token's end
    ///   (the C++ `wrappedStart` parameter, used for `: T` annotations).
    /// \param allow_anon_function_type value for `allow_anon_function_type`
    ///   while parsing this annotation (saved/restored around the parse).
    pub(super) fn parse_type_annotation_flow(
        &mut self,
        wrapped_start: Option<SMLoc>,
        allow_anon_function_type: AllowAnonFunctionType,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 3081-3083: llvh::SaveAndRestore<bool> on allowAnonFunctionType_.
        // The guard restores the old value on every exit path, including the
        // `?` early return below.
        let _guard = self.save_allow_anon_function_type(
            allow_anon_function_type == AllowAnonFunctionType::Yes,
        );
        let opt_type = self.parse_conditional_type_annotation_flow()?;
        if let Some(start) = wrapped_start {
            // C++ 3087-3092.
            let node = Node::TypeAnnotation(TypeAnnotation::new(
                NodeMetadata::new(self.dummy_range()),
                opt_type,
            ));
            return Some(self.set_location(
                start,
                self.lexer.prev_token_end(),
                node,
            ));
        }
        Some(opt_type)
    }

    // -----------------------------------------------------------------------
    // The type-annotation precedence hierarchy. P5.0 keeps each level as a
    // delegation to the next; P5.1 fills in the real productions.
    // -----------------------------------------------------------------------

    /// Port of `parseConditionalTypeAnnotationFlow` (flow.cpp:3096-3145).
    fn parse_conditional_type_annotation_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 3098: conditional types are allowed while parsing the check
        // type.
        let _guard = self.save_allow_conditional_type(true);
        let check_type = self.parse_union_type_annotation_flow()?;
        if self.check(TokenKind::rw_extends) {
            // P5.1: full ConditionalTypeAnnotation logic
            // (`Check extends Extends ? True : False`, C++ 3102-3144).
            self.error_cur("conditional types are unsupported (parser phase P5.1)");
            return None;
        }
        Some(check_type)
    }

    /// Port of `parseUnionTypeAnnotationFlow` (flow.cpp:3147-3174).
    fn parse_union_type_annotation_flow(&mut self) -> Option<&'gc Node<'gc>> {
        // P5.1: full UnionTypeAnnotation logic (leading `|` and
        // `A | B | C`, C++ 3148-3173).
        self.parse_intersection_type_annotation_flow()
    }

    /// Port of `parseIntersectionTypeAnnotationFlow` (flow.cpp:3176-3204).
    fn parse_intersection_type_annotation_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // P5.1: full IntersectionTypeAnnotation logic (leading `&` and
        // `A & B & C`, C++ 3177-3203).
        self.parse_anon_function_without_parens_type_annotation_flow()
    }

    /// Port of `parseAnonFunctionWithoutParensTypeAnnotationFlow`
    /// (flow.cpp:3206-3230).
    fn parse_anon_function_without_parens_type_annotation_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // P5.1: full anonymous-function-type logic
        // (`ParamType => ReturnType`, gated on allow_anon_function_type,
        // C++ 3207-3229).
        self.parse_prefix_type_annotation_flow()
    }

    /// Port of `parsePrefixTypeAnnotationFlow` (flow.cpp:3232-3244).
    fn parse_prefix_type_annotation_flow(&mut self) -> Option<&'gc Node<'gc>> {
        // P5.1: full prefix logic (nullable `?T`, C++ 3233-3242).
        self.parse_postfix_type_annotation_flow()
    }

    /// Port of `parsePostfixTypeAnnotationFlow` (flow.cpp:3246-3303).
    fn parse_postfix_type_annotation_flow(&mut self) -> Option<&'gc Node<'gc>> {
        // P5.1: full postfix logic (legacy array `T[]`, indexed access
        // `T[K]` / `T?.[K]`, C++ 3247-3302).
        self.parse_primary_type_annotation_flow()
    }

    // -----------------------------------------------------------------------
    // parsePrimaryTypeAnnotationFlow — 3305 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a primary type annotation. Port of
    /// `JSParserImpl::parsePrimaryTypeAnnotationFlow` (flow.cpp:3305-3602).
    /// P5.0 implements the `*`/`null`/`void`/literal/named-primitive arms;
    /// the rest are honest errors (see the per-arm markers).
    fn parse_primary_type_annotation_flow(&mut self) -> Option<&'gc Node<'gc>> {
        let start = self.cur_start();
        match self.cur_kind() {
            // C++ 3308-3312.
            TokenKind::star => {
                let node = Node::ExistsTypeAnnotation(ExistsTypeAnnotation::new(
                    NodeMetadata::new(self.dummy_range()),
                ));
                let end = self.advance(GrammarContext::Type).end;
                Some(self.set_location(start, end, node))
            }

            // C++ 3313-3314.
            TokenKind::less => {
                // P5.2: parseFunctionTypeAnnotationFlow.
                self.error_cur(
                    "function type annotations are unsupported (parser phase P5.2)",
                );
                None
            }

            // C++ 3315-3316. NOTE: the C++ group path (`(T)`) calls
            // `incParens()` on the inner type; the Rust AST has the
            // matching `metadata().parens` slot — use it when porting.
            TokenKind::l_paren => {
                // P5.2: parseFunctionOrGroupTypeAnnotationFlow.
                self.error_cur(
                    "function/group type annotations are unsupported (parser phase P5.2)",
                );
                None
            }

            // C++ 3317-3322.
            TokenKind::l_brace | TokenKind::l_bracepipe => {
                // P5.2: parseObjectTypeAnnotationFlow.
                self.error_cur(
                    "object type annotations are unsupported (parser phase P5.2)",
                );
                None
            }

            // C++ 3323-3334.
            TokenKind::rw_interface => {
                // P5.3: InterfaceTypeAnnotation (parseInterfaceTailFlow).
                self.error_cur(
                    "interface type annotations are unsupported (parser phase P5.3)",
                );
                None
            }

            // C++ 3335-3336.
            TokenKind::rw_typeof => {
                // P5.1: parseTypeofTypeAnnotationFlow.
                self.error_cur(
                    "typeof type annotations are unsupported (parser phase P5.1)",
                );
                None
            }

            // C++ 3338-3339.
            TokenKind::l_square => {
                // P5.1: parseTupleTypeAnnotationFlow.
                self.error_cur(
                    "tuple type annotations are unsupported (parser phase P5.1)",
                );
                None
            }

            // C++ 3340-3511. The C++ compares `tok_->getResWordOrIdentifier()`
            // against the pre-interned `anyIdent_`/`mixedIdent_`/... atoms
            // (escape-insensitive); we compare the token's interned name bytes
            // directly. Each named-primitive arm is
            // `setLocation(start, advance(GrammarContext::Type).End, new
            // <Name>Node())` (C++ 3343-3408).
            TokenKind::rw_static | TokenKind::rw_this | TokenKind::identifier => {
                let node = {
                    let name = self.lexer.get_string_table().bytes(
                        self.lexer.token().get_res_word_or_identifier(),
                    );
                    let md = NodeMetadata::new(self.dummy_range());
                    match name {
                        // C++ 3343-3347.
                        b"any" => {
                            Some(Node::AnyTypeAnnotation(AnyTypeAnnotation::new(md)))
                        }
                        // C++ 3348-3353.
                        b"mixed" => Some(Node::MixedTypeAnnotation(
                            MixedTypeAnnotation::new(md),
                        )),
                        // C++ 3354-3359.
                        b"empty" => Some(Node::EmptyTypeAnnotation(
                            EmptyTypeAnnotation::new(md),
                        )),
                        // C++ 3360-3365.
                        b"unknown" => Some(Node::UnknownTypeAnnotation(
                            UnknownTypeAnnotation::new(md),
                        )),
                        // C++ 3366-3371.
                        b"never" => Some(Node::NeverTypeAnnotation(
                            NeverTypeAnnotation::new(md),
                        )),
                        // C++ 3372-3377.
                        b"undefined" => Some(Node::UndefinedTypeAnnotation(
                            UndefinedTypeAnnotation::new(md),
                        )),
                        // C++ 3378-3384.
                        b"boolean" | b"bool" => Some(Node::BooleanTypeAnnotation(
                            BooleanTypeAnnotation::new(md),
                        )),
                        // C++ 3385-3390.
                        b"number" => Some(Node::NumberTypeAnnotation(
                            NumberTypeAnnotation::new(md),
                        )),
                        // C++ 3391-3396.
                        b"symbol" => Some(Node::SymbolTypeAnnotation(
                            SymbolTypeAnnotation::new(md),
                        )),
                        // C++ 3397-3402.
                        b"string" => Some(Node::StringTypeAnnotation(
                            StringTypeAnnotation::new(md),
                        )),
                        // C++ 3403-3408.
                        b"bigint" => Some(Node::BigIntTypeAnnotation(
                            BigIntTypeAnnotation::new(md),
                        )),
                        _ => None,
                    }
                };
                if let Some(node) = node {
                    let end = self.advance(GrammarContext::Type).end;
                    return Some(self.set_location(start, end, node));
                }
                // P5.1: keyof (C++ 3409-3419), renders/component/hook
                // (component syntax, C++ 3420-3446 — P6), interface-as-ident
                // (C++ 3447-3458 — P5.3), infer (C++ 3459-3505), and the
                // generic-type fallthrough (parseGenericTypeFlow,
                // C++ 3507-3511).
                self.error_cur("unsupported type annotation (parser phase P5.1)");
                None
            }

            // C++ 3513-3517.
            TokenKind::rw_null => {
                let node = Node::NullLiteralTypeAnnotation(
                    NullLiteralTypeAnnotation::new(NodeMetadata::new(
                        self.dummy_range(),
                    )),
                );
                let end = self.advance(GrammarContext::Type).end;
                Some(self.set_location(start, end, node))
            }

            // C++ 3519-3523.
            TokenKind::rw_void => {
                let node = Node::VoidTypeAnnotation(VoidTypeAnnotation::new(
                    NodeMetadata::new(self.dummy_range()),
                ));
                let end = self.advance(GrammarContext::Type).end;
                Some(self.set_location(start, end, node))
            }

            // C++ 3525-3532.
            TokenKind::string_literal => {
                let value = self.lexer.token().get_string_literal();
                // C++: `lexer_.getStringLiteral(tok_->inputStr())` — the raw
                // SOURCE text of the token (including the quotes), interned.
                let raw = self.cur_token_source_atom();
                let node = Node::StringLiteralTypeAnnotation(
                    StringLiteralTypeAnnotation::new(
                        NodeMetadata::new(self.dummy_range()),
                        value,
                        raw,
                    ),
                );
                let end = self.advance(GrammarContext::Type).end;
                Some(self.set_location(start, end, node))
            }

            // C++ 3534-3541.
            TokenKind::numeric_literal => {
                let value = self.lexer.token().get_numeric_literal();
                let raw = self.cur_token_source_atom();
                let node = Node::NumberLiteralTypeAnnotation(
                    NumberLiteralTypeAnnotation::new(
                        NodeMetadata::new(self.dummy_range()),
                        value,
                        raw,
                    ),
                );
                let end = self.advance(GrammarContext::Type).end;
                Some(self.set_location(start, end, node))
            }

            // C++ 3543-3549.
            TokenKind::bigint_literal => {
                let raw = self.lexer.token().get_bigint_literal_raw_value();
                let node = Node::BigIntLiteralTypeAnnotation(
                    BigIntLiteralTypeAnnotation::new(
                        NodeMetadata::new(self.dummy_range()),
                        raw,
                    ),
                );
                let end = self.advance(GrammarContext::Type).end;
                Some(self.set_location(start, end, node))
            }

            // C++ 3551-3581.
            TokenKind::minus => {
                // P5.1: negated Number/BigInt literal types (the raw text
                // spans from the `-` through the literal token).
                self.error_cur(
                    "negative literal types unsupported (parser phase P5.1)",
                );
                None
            }

            // C++ 3583-3591.
            TokenKind::rw_true | TokenKind::rw_false => {
                let value = self.check(TokenKind::rw_true);
                let raw = self.lexer.token().get_res_word_identifier();
                let node = Node::BooleanLiteralTypeAnnotation(
                    BooleanLiteralTypeAnnotation::new(
                        NodeMetadata::new(self.dummy_range()),
                        value,
                        raw,
                    ),
                );
                let end = self.advance(GrammarContext::Type).end;
                Some(self.set_location(start, end, node))
            }

            // C++ 3592-3600.
            _ => {
                if self.lexer.token().is_res_word() {
                    // P5.1: parseGenericTypeFlow (C++ 3593-3597).
                    self.error_cur(
                        "unsupported type annotation (parser phase P5.1)",
                    );
                    return None;
                }
                self.error_cur("unexpected token in type annotation");
                None
            }
        }
    }

    /// Intern the raw source text of the current token. The Rust equivalent
    /// of the C++ `lexer_.getStringLiteral(tok_->inputStr())` idiom used by
    /// the literal type annotations.
    fn cur_token_source_atom(&self) -> NodeLabel {
        let range = self.lexer.token().source_range();
        let buf_start = self.lexer.get_buffer_start();
        let buf = self.lexer.buffer_bytes();
        let start = (range.start.offset - buf_start) as usize;
        let end = (range.end.offset - buf_start) as usize;
        self.lexer.get_string_literal(&buf[start..end])
    }
}
