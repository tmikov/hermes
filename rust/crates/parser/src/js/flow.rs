/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Flow type-grammar parsing for the JS parser. Port of
//! `lib/Parser/JSParserImpl-flow.cpp`.
//!
//! P5.0 implemented the Flow declaration gate (`parseFlowDeclaration`) and
//! the plain `type X = T;` alias pipeline; P5.1 the full type-annotation
//! precedence hierarchy (`parseConditionalTypeAnnotationFlow` →
//! `parsePrimaryTypeAnnotationFlow`), generic types, type arguments,
//! `typeof`/tuple/`keyof`/`infer` types, and the reparse helpers; P5.2
//! function types, object types, type-parameter declarations, variance,
//! predicates, and return-type annotations. The remaining productions emit
//! an honest "unsupported (parser phase P5.x)" error at the marked site; the
//! later sub-tasks (P5.3 interfaces / declare / opaque, P6 enum / component /
//! hook / record / match) replace those markers with the real grammar.

use ast::node::{
    AnyTypeAnnotation, ArrayTypeAnnotation, BigIntLiteralTypeAnnotation,
    BigIntTypeAnnotation, BooleanLiteralTypeAnnotation, BooleanTypeAnnotation,
    ConditionalTypeAnnotation, DeclaredPredicate, EmptyTypeAnnotation,
    ExistsTypeAnnotation, FunctionTypeAnnotation, FunctionTypeParam,
    GenericTypeAnnotation, Identifier, IndexedAccessType, InferTypeAnnotation,
    InferredPredicate, IntersectionTypeAnnotation, KeyofTypeAnnotation,
    MixedTypeAnnotation, NeverTypeAnnotation, Node, NullLiteralTypeAnnotation,
    NullableTypeAnnotation, NumberLiteralTypeAnnotation, NumberTypeAnnotation,
    ObjectTypeAnnotation, ObjectTypeCallProperty, ObjectTypeIndexer,
    ObjectTypeInternalSlot, ObjectTypeMappedTypeProperty, ObjectTypeProperty,
    ObjectTypeSpreadProperty, OptionalIndexedAccessType,
    QualifiedTypeIdentifier, QualifiedTypeofIdentifier,
    StringLiteralTypeAnnotation, StringTypeAnnotation, SymbolTypeAnnotation,
    TupleTypeAnnotation, TupleTypeLabeledElement, TupleTypeSpreadElement,
    TypeAlias, TypeAnnotation, TypeParameter, TypeParameterDeclaration,
    TypeParameterInstantiation, TypePredicate, TypeofTypeAnnotation,
    UndefinedTypeAnnotation, UnionTypeAnnotation, UnknownTypeAnnotation,
    Variance, VoidTypeAnnotation,
};
use ast::node_child::{NodeLabel, NodeList, NodeMetadata, NodeString};
use atom_table::INVALID_ATOM_BYTES;
use support::location::SMLoc;

use crate::lexer::GrammarContext;
use crate::token_kinds::{ord, TokenKind};

use super::expressions::inc_parens;
use super::{JSParserImpl, PARAM_IN};

/// Check if the given token kind can follow a contextual variance keyword
/// (`readonly` or `writeonly`) in Flow mode. Used to disambiguate the
/// keyword as a variance annotation from the keyword used as a property name.
/// Port of `JSParserImpl::canFollowVarianceKeywordFlow`
/// (JSParserImpl.h:1666-1689).
fn can_follow_variance_keyword_flow(
    opt_token_kind: Option<TokenKind>,
) -> bool {
    let Some(kind) = opt_token_kind else {
        return false;
    };
    // Reserved words (e.g. `with`, `enum`, `default`, `new`) are valid
    // property names, so `readonly <reservedWord>:` should be parsed as a
    // variance modifier on a reserved-word property — the same way
    // `+<reservedWord>:` already is.
    if ord(kind) > ord(TokenKind::_first_resword)
        && ord(kind) < ord(TokenKind::_last_resword)
    {
        return true;
    }
    matches!(
        kind,
        TokenKind::identifier
            | TokenKind::private_identifier
            | TokenKind::string_literal
            | TokenKind::numeric_literal
            | TokenKind::bigint_literal
            | TokenKind::l_square
    )
}

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
pub(super) enum AllowAnonFunctionType {
    No,
    Yes,
}

/// Whether a `proto` property modifier is allowed in an object type.
/// Port of `JSParserImpl::AllowProtoProperty` (JSParserImpl.h:1444).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)] // `Yes` is passed by declare-class bodies (P6).
pub(super) enum AllowProtoProperty {
    No,
    Yes,
}

/// Whether a `static` property modifier is allowed in an object type.
/// Port of `JSParserImpl::AllowStaticProperty` (JSParserImpl.h:1447).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)] // `Yes` is passed by declare-class bodies (P6).
pub(super) enum AllowStaticProperty {
    No,
    Yes,
}

/// Whether a `...T` spread property is allowed in an object type.
/// Port of `JSParserImpl::AllowSpreadProperty` (JSParserImpl.h:1450).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)] // `No` is passed by interface bodies (P5.3).
pub(super) enum AllowSpreadProperty {
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
    /// (flow.cpp:1981-2071). P5.0/P5.2 implement the `TypeAliasKind::None`
    /// path (a plain `TypeAlias` node, with optional type parameters).
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
    // parseReturnTypeAnnotationFlow — 2883 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a function return type annotation, which may be a plain type or
    /// a type predicate (`asserts x [is T]`, `implies x is T`, `x is T`).
    /// Port of `parseReturnTypeAnnotationFlow` (flow.cpp:2883-3009).
    ///
    /// \param wrapped_start like `parse_type_annotation_flow`'s: if `Some`,
    ///   the result is wrapped in a `TypeAnnotation` node.
    pub(super) fn parse_return_type_annotation_flow(
        &mut self,
        wrapped_start: Option<SMLoc>,
        allow_anon_function_type: AllowAnonFunctionType,
    ) -> Option<&'gc Node<'gc>> {
        let start = self.cur_start();
        let return_type: &'gc Node<'gc>;
        if self.check_name(b"asserts") {
            // C++ 2888-2924.
            // TypePredicate (asserts = true) or TypeAnnotation:
            //   TypeAnnotation
            //   asserts IdentifierName
            //   asserts IdentifierName is TypeAnnotation
            let opt_type = self
                .parse_type_annotation_flow(None, allow_anon_function_type)?;

            if self.check(TokenKind::identifier) {
                // Validate the "asserts" token was an identifier not a more
                // complex type (C++ 2898-2901; the reparsed node itself is
                // unused).
                self.reparse_type_annotation_as_identifier_flow(opt_type)?;
                // C++ 2902-2907.
                let id_range = self.cur_range();
                let id_node = Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    self.lexer.token().get_identifier(),
                    None,
                    false,
                ));
                let id = self.set_location(
                    id_range.start,
                    id_range.end,
                    id_node,
                );
                self.advance(GrammarContext::Type);
                // C++ 2908-2916: checkAndEat(isIdent_, Type).
                let mut type_annotation: Option<&'gc Node<'gc>> = None;
                if self.check_name(b"is") {
                    self.advance(GrammarContext::Type);
                    // assert IdentifierName is TypeAnnotation
                    //                          ^
                    type_annotation = Some(self.parse_type_annotation_flow(
                        None,
                        allow_anon_function_type,
                    )?);
                }
                // C++ 2917-2921.
                let node = Node::TypePredicate(TypePredicate::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    type_annotation,
                    self.lexer.get_identifier(b"asserts"),
                ));
                return_type = self.set_location(
                    start,
                    self.lexer.prev_token_end(),
                    node,
                );
            } else {
                return_type = opt_type;
            }
        } else if self.check_name(b"implies") {
            // C++ 2925-2976.
            // TypePredicate (implies = true) or TypeAnnotation:
            //   TypeAnnotation
            //   implies IdentifierName is TypeAnnotation

            //   implies IdentifierName is TypeAnnotation
            //   ^
            let opt_type = self
                .parse_type_annotation_flow(None, allow_anon_function_type)?;

            if self.check2(TokenKind::identifier, TokenKind::rw_this) {
                // Validate the "implies" token was an identifier not a more
                // complex type (C++ 2938-2944).
                let is_bare_generic = matches!(
                    opt_type,
                    Node::GenericTypeAnnotation(generic)
                        if generic.type_parameters.is_none()
                );
                if !is_bare_generic {
                    self.error_at_loc(
                        self.cur_start(),
                        "invalid return annotation. 'implies' type guard needs to be followed by identifier",
                    );
                    return None;
                }

                //   implies IdentifierName is TypeAnnotation
                //           ^
                let id_range = self.cur_range();
                let id_node = Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    self.lexer.token().get_res_word_or_identifier(),
                    None,
                    false,
                ));
                let id = self.set_location(
                    id_range.start,
                    id_range.end,
                    id_node,
                );
                self.advance(GrammarContext::Type);

                //   implies IdentifierName is TypeAnnotation
                //                          ^
                // C++ 2957-2962: checkAndEat(isIdent_, Type).
                if self.check_name(b"is") {
                    self.advance(GrammarContext::Type);
                } else {
                    self.error_at_loc(
                        self.cur_start(),
                        "expecting 'is' after parameter of 'implies' type guard",
                    );
                    return None;
                }
                //   implies IdentifierName is TypeAnnotation
                //                             ^
                let type_t = self.parse_type_annotation_flow(
                    None,
                    allow_anon_function_type,
                )?;
                // C++ 2968-2972.
                let node = Node::TypePredicate(TypePredicate::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    Some(type_t),
                    self.lexer.get_identifier(b"implies"),
                ));
                return_type = self.set_location(
                    start,
                    self.lexer.prev_token_end(),
                    node,
                );
            } else {
                // implies (as type -- okay)
                return_type = opt_type;
            }
        } else {
            // C++ 2977-2999.
            // TypePredicate (asserts = false && implies = false) or
            // TypeAnnotation:
            //   TypeAnnotation
            //   IdentifierName is TypeAnnotation
            let opt_type = self
                .parse_type_annotation_flow(None, allow_anon_function_type)?;

            // C++ 2986: checkAndEat(isIdent_, Type).
            if self.check_name(b"is") {
                self.advance(GrammarContext::Type);
                let id =
                    self.reparse_type_annotation_as_identifier_flow(opt_type)?;
                let type_annotation = self.parse_type_annotation_flow(
                    None,
                    allow_anon_function_type,
                )?;
                // C++ 2993-2996: the C++ passes a null UniqueString for
                // `kind` on an unprefixed predicate; the dumper emits
                // `"kind": null` — INVALID_ATOM_BYTES is the Rust null
                // NodeString.
                let node = Node::TypePredicate(TypePredicate::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    Some(type_annotation),
                    INVALID_ATOM_BYTES,
                ));
                return_type = self.set_location(
                    start,
                    self.lexer.prev_token_end(),
                    node,
                );
            } else {
                return_type = opt_type;
            }
        }

        // C++ 3002-3008.
        if let Some(wrapped_start) = wrapped_start {
            let node = Node::TypeAnnotation(TypeAnnotation::new(
                NodeMetadata::new(self.dummy_range()),
                return_type,
            ));
            return Some(self.set_location(
                wrapped_start,
                self.lexer.prev_token_end(),
                node,
            ));
        }
        Some(return_type)
    }

    // -----------------------------------------------------------------------
    // The type-annotation precedence hierarchy:
    // conditional → union → intersection → anon-fn-without-parens → prefix →
    // postfix → primary.
    // -----------------------------------------------------------------------

    /// Parse a type annotation that may be used where a colon could follow
    /// (e.g. a possibly-labeled tuple element or function parameter). Port of
    /// `parseTypeAnnotationBeforeColonFlow` (flow.cpp:3011-3076).
    fn parse_type_annotation_before_colon_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // P6: the component-syntax lookahead paths (the `component`/`hook`/
        // `renders` contextual keywords, C++ 3014-3072) are gated on
        // getParseFlowComponentSyntax(), which the Rust Context does not
        // implement yet.

        // C++ 3075.
        self.parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)
    }

    /// Port of `parseConditionalTypeAnnotationFlow` (flow.cpp:3096-3145).
    fn parse_conditional_type_annotation_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 3098: conditional types are allowed while parsing the check
        // type.
        let _guard = self.save_allow_conditional_type(true);
        let check_type = self.parse_union_type_annotation_flow()?;
        // C++ 3102-3104.
        if !self.check_and_eat(TokenKind::rw_extends, GrammarContext::Type) {
            return Some(check_type);
        }

        let extends_type = {
            // C++ 3106-3110: We need to enter the state of parsing the
            // extends_type disallowing conditional types not wrapped by
            // parantheses, so that the following sequence
            // `A extends infer B extends C ? D : E` will be interpreted
            // as `A extends (infer B extends C) ? D : E`.
            let _guard = self.save_allow_conditional_type(false);
            self.parse_union_type_annotation_flow()
        }?;

        // C++ 3117-3123.
        if !self.eat(
            TokenKind::question,
            GrammarContext::Type,
            " in conditional type",
        ) {
            return None;
        }

        // C++ 3125-3126.
        let true_type =
            self.parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

        // C++ 3128-3134.
        if !self.eat(
            TokenKind::colon,
            GrammarContext::Type,
            " in conditional type",
        ) {
            return None;
        }

        // C++ 3136-3138.
        let false_type =
            self.parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

        // C++ 3140-3144: located from the check type's start (NOT the start
        // of this production — they only differ if error recovery moved us).
        let node = Node::ConditionalTypeAnnotation(
            ConditionalTypeAnnotation::new(
                NodeMetadata::new(self.dummy_range()),
                check_type,
                extends_type,
                true_type,
                false_type,
            ),
        );
        Some(self.set_location(
            check_type.metadata().range.get().start,
            self.lexer.prev_token_end(),
            node,
        ))
    }

    /// Port of `parseUnionTypeAnnotationFlow` (flow.cpp:3147-3174).
    fn parse_union_type_annotation_flow(&mut self) -> Option<&'gc Node<'gc>> {
        // C++ 3148-3149: `start` is captured BEFORE the optional leading `|`.
        let start = self.cur_start();
        self.check_and_eat(TokenKind::pipe, GrammarContext::Type);

        let first = self.parse_intersection_type_annotation_flow()?;

        if !self.check(TokenKind::pipe) {
            // Done with the union, move on.
            return Some(first);
        }

        let mut types: Vec<&'gc Node<'gc>> = vec![first];
        while self.check_and_eat(TokenKind::pipe, GrammarContext::Type) {
            types.push(self.parse_intersection_type_annotation_flow()?);
        }

        // C++ 3170-3173.
        let node = Node::UnionTypeAnnotation(UnionTypeAnnotation::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, types),
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    /// Port of `parseIntersectionTypeAnnotationFlow` (flow.cpp:3176-3204).
    fn parse_intersection_type_annotation_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 3177-3178: `start` is captured BEFORE the optional leading `&`.
        let start = self.cur_start();
        self.check_and_eat(TokenKind::amp, GrammarContext::Type);

        let first =
            self.parse_anon_function_without_parens_type_annotation_flow()?;

        if !self.check(TokenKind::amp) {
            // Done with the union, move on.
            return Some(first);
        }

        let mut types: Vec<&'gc Node<'gc>> = vec![first];
        while self.check_and_eat(TokenKind::amp, GrammarContext::Type) {
            types.push(
                self.parse_anon_function_without_parens_type_annotation_flow()?,
            );
        }

        // C++ 3199-3202.
        let node =
            Node::IntersectionTypeAnnotation(IntersectionTypeAnnotation::new(
                NodeMetadata::new(self.dummy_range()),
                NodeList::from_iter(self.gc, types),
            ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    /// Port of `parseAnonFunctionWithoutParensTypeAnnotationFlow`
    /// (flow.cpp:3206-3230).
    fn parse_anon_function_without_parens_type_annotation_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        let start = self.cur_start();
        let param = self.parse_prefix_type_annotation_flow()?;

        // C++ 3212-3228.
        if self.allow_anon_function_type.get()
            && self.check(TokenKind::equalgreater)
        {
            // ParamType => ReturnType
            //           ^
            // "Reparse" the param into a FunctionTypeParam so it can be used
            // for parseFunctionTypeAnnotationWithParamsFlow. C++ 3216-3221:
            // it spans exactly the param's range.
            let param_range = param.metadata().range.get();
            let ftp_node = Node::FunctionTypeParam(FunctionTypeParam::new(
                NodeMetadata::new(self.dummy_range()),
                None, // name
                param,
                false, // optional
            ));
            let ftp =
                self.set_location(param_range.start, param_range.end, ftp_node);
            return self.parse_function_type_annotation_with_params_flow(
                start,
                vec![ftp],
                None,  // this constraint
                None,  // rest
                None,  // type params
                false, // hook
            );
        }

        Some(param)
    }

    /// Parse the `=> ReturnType` tail of a function type whose parameters
    /// have already been parsed. Port of
    /// `parseFunctionTypeAnnotationWithParamsFlow` (flow.cpp:3865-3897).
    fn parse_function_type_annotation_with_params_flow(
        &mut self,
        start: SMLoc,
        params: Vec<&'gc Node<'gc>>,
        this_constraint: Option<&'gc Node<'gc>>,
        rest: Option<&'gc Node<'gc>>,
        type_params: Option<&'gc Node<'gc>>,
        hook: bool,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 3873-3874.
        assert!(self.check(TokenKind::equalgreater));
        self.advance(GrammarContext::Type);

        // C++ 3876: `parseReturnTypeAnnotationFlow()` with its declaration
        // defaults (wrappedStart=None, AllowAnonFunctionType::Yes;
        // JSParserImpl.h:1283-1286).
        let return_type = self
            .parse_return_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

        // C++ 3880-3896.
        if !hook {
            let node =
                Node::FunctionTypeAnnotation(FunctionTypeAnnotation::new(
                    NodeMetadata::new(self.dummy_range()),
                    NodeList::from_iter(self.gc, params),
                    this_constraint,
                    return_type,
                    rest,
                    type_params,
                ));
            Some(self.set_location(start, self.lexer.prev_token_end(), node))
        } else {
            // P6: HookTypeAnnotation (C++ 3891-3895) — hook syntax is gated on
            // getParseFlowComponentSyntax(), which the Rust Context does not
            // implement yet; no caller passes hook=true in P5.
            self.error_cur(
                "hook type annotations are unsupported (parser phase P6)",
            );
            None
        }
    }

    /// Port of `parsePrefixTypeAnnotationFlow` (flow.cpp:3232-3244).
    fn parse_prefix_type_annotation_flow(&mut self) -> Option<&'gc Node<'gc>> {
        let start = self.cur_start();
        // C++ 3234-3242: nullable `?T` (right-recursive, so `??T` nests).
        if self.check_and_eat(TokenKind::question, GrammarContext::Type) {
            let prefix = self.parse_prefix_type_annotation_flow()?;
            let node =
                Node::NullableTypeAnnotation(NullableTypeAnnotation::new(
                    NodeMetadata::new(self.dummy_range()),
                    prefix,
                ));
            return Some(self.set_location(
                start,
                self.lexer.prev_token_end(),
                node,
            ));
        }
        self.parse_postfix_type_annotation_flow()
    }

    /// Port of `parsePostfixTypeAnnotationFlow` (flow.cpp:3246-3303).
    fn parse_postfix_type_annotation_flow(&mut self) -> Option<&'gc Node<'gc>> {
        let start = self.cur_start();
        let mut result = self.parse_primary_type_annotation_flow()?;
        let mut seen_optional_indexed_access = false;

        // C++ 3255-3256.
        while self.check2(TokenKind::l_square, TokenKind::questiondot)
            && !self.lexer.is_new_line_before_current_token()
        {
            // C++ 3257: `checkAndEat(questiondot)` uses the DEFAULT grammar
            // context (AllowRegExp), NOT Type — deliberate; keep it.
            let optional = self.check_and_eat(
                TokenKind::questiondot,
                GrammarContext::AllowRegExp,
            );
            seen_optional_indexed_access =
                seen_optional_indexed_access || optional;

            // C++ 3260-3266.
            if !self.eat(
                TokenKind::l_square,
                GrammarContext::Type,
                " in indexed access type or postfix array type syntax",
            ) {
                return None;
            }

            if !optional
                && self.check_and_eat(TokenKind::r_square, GrammarContext::Type)
            {
                // Legacy Array syntax `T[]` (C++ 3268-3274; spans from this
                // production's start).
                let node = Node::ArrayTypeAnnotation(ArrayTypeAnnotation::new(
                    NodeMetadata::new(self.dummy_range()),
                    result,
                ));
                result = self.set_location(
                    start,
                    self.lexer.prev_token_end(),
                    node,
                );
            } else {
                // Indexed Access `T[K]` (`T?.[K]` if `optional`),
                // C++ 3276-3298.
                let index_type = self.parse_type_annotation_flow(
                    None,
                    AllowAnonFunctionType::Yes,
                )?;
                if !self.need(TokenKind::r_square, " in indexed access type") {
                    return None;
                }
                // Once a `?.[` has been seen, all the enclosing accesses
                // become OptionalIndexedAccessType (with optional=false for
                // the plain `[` ones).
                if seen_optional_indexed_access {
                    let node = Node::OptionalIndexedAccessType(
                        OptionalIndexedAccessType::new(
                            NodeMetadata::new(self.dummy_range()),
                            result,
                            index_type,
                            optional,
                        ),
                    );
                    let end = self.advance(GrammarContext::Type).end;
                    result = self.set_location(start, end, node);
                } else {
                    let node = Node::IndexedAccessType(IndexedAccessType::new(
                        NodeMetadata::new(self.dummy_range()),
                        result,
                        index_type,
                    ));
                    let end = self.advance(GrammarContext::Type).end;
                    result = self.set_location(start, end, node);
                }
            }
        }

        Some(result)
    }

    // -----------------------------------------------------------------------
    // parsePrimaryTypeAnnotationFlow — 3305 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a primary type annotation. Port of
    /// `JSParserImpl::parsePrimaryTypeAnnotationFlow` (flow.cpp:3305-3602).
    /// P5.0-P5.2 implement all arms except `interface` types (P5.3) — see
    /// the per-arm markers.
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
            TokenKind::less => self.parse_function_type_annotation_flow(),

            // C++ 3315-3316.
            TokenKind::l_paren => {
                self.parse_function_or_group_type_annotation_flow()
            }

            // C++ 3317-3322.
            TokenKind::l_brace | TokenKind::l_bracepipe => self
                .parse_object_type_annotation_flow(
                    AllowProtoProperty::No,
                    AllowStaticProperty::No,
                    AllowSpreadProperty::Yes,
                ),

            // C++ 3323-3334.
            TokenKind::rw_interface => {
                // P5.3: InterfaceTypeAnnotation (parseInterfaceTailFlow).
                self.error_cur(
                    "interface type annotations are unsupported (parser phase P5.3)",
                );
                None
            }

            // C++ 3335-3336.
            TokenKind::rw_typeof => self.parse_typeof_type_annotation_flow(),

            // C++ 3338-3339.
            TokenKind::l_square => self.parse_tuple_type_annotation_flow(),

            // C++ 3340-3511. The C++ compares `tok_->getResWordOrIdentifier()`
            // against the pre-interned `anyIdent_`/`mixedIdent_`/... atoms
            // (escape-insensitive); we compare the token's interned name bytes
            // directly. Each named-primitive arm is
            // `setLocation(start, advance(GrammarContext::Type).End, new
            // <Name>Node())` (C++ 3343-3408).
            TokenKind::rw_static | TokenKind::rw_this | TokenKind::identifier => {
                /// Dispatch outcome of the named-type match below: either a
                /// finished primitive node (consume the token and return), or
                /// one of the multi-token productions.
                enum NamedType<'gc> {
                    Prim(Node<'gc>),
                    Keyof,
                    Interface,
                    Infer,
                    Generic,
                }
                let arm = {
                    let name = self.lexer.get_string_table().bytes(
                        self.lexer.token().get_res_word_or_identifier(),
                    );
                    let md = NodeMetadata::new(self.dummy_range());
                    match name {
                        // C++ 3343-3347.
                        b"any" => NamedType::Prim(Node::AnyTypeAnnotation(
                            AnyTypeAnnotation::new(md),
                        )),
                        // C++ 3348-3353.
                        b"mixed" => NamedType::Prim(Node::MixedTypeAnnotation(
                            MixedTypeAnnotation::new(md),
                        )),
                        // C++ 3354-3359.
                        b"empty" => NamedType::Prim(Node::EmptyTypeAnnotation(
                            EmptyTypeAnnotation::new(md),
                        )),
                        // C++ 3360-3365.
                        b"unknown" => NamedType::Prim(
                            Node::UnknownTypeAnnotation(
                                UnknownTypeAnnotation::new(md),
                            ),
                        ),
                        // C++ 3366-3371.
                        b"never" => NamedType::Prim(Node::NeverTypeAnnotation(
                            NeverTypeAnnotation::new(md),
                        )),
                        // C++ 3372-3377.
                        b"undefined" => NamedType::Prim(
                            Node::UndefinedTypeAnnotation(
                                UndefinedTypeAnnotation::new(md),
                            ),
                        ),
                        // C++ 3378-3384.
                        b"boolean" | b"bool" => NamedType::Prim(
                            Node::BooleanTypeAnnotation(
                                BooleanTypeAnnotation::new(md),
                            ),
                        ),
                        // C++ 3385-3390.
                        b"number" => NamedType::Prim(
                            Node::NumberTypeAnnotation(
                                NumberTypeAnnotation::new(md),
                            ),
                        ),
                        // C++ 3391-3396.
                        b"symbol" => NamedType::Prim(
                            Node::SymbolTypeAnnotation(
                                SymbolTypeAnnotation::new(md),
                            ),
                        ),
                        // C++ 3397-3402.
                        b"string" => NamedType::Prim(
                            Node::StringTypeAnnotation(
                                StringTypeAnnotation::new(md),
                            ),
                        ),
                        // C++ 3403-3408.
                        b"bigint" => NamedType::Prim(
                            Node::BigIntTypeAnnotation(
                                BigIntTypeAnnotation::new(md),
                            ),
                        ),
                        // C++ 3410-3418.
                        b"keyof" => NamedType::Keyof,
                        // P6: `renders`/`component`/`hook` (C++ 3420-3446)
                        // are gated on getParseFlowComponentSyntax(), which
                        // the Rust Context does not implement yet.
                        // C++ 3447-3457.
                        b"interface" => NamedType::Interface,
                        // C++ 3459-3504.
                        b"infer" => NamedType::Infer,
                        // C++ 3506-3511.
                        _ => NamedType::Generic,
                    }
                };
                match arm {
                    NamedType::Prim(node) => {
                        let end = self.advance(GrammarContext::Type).end;
                        Some(self.set_location(start, end, node))
                    }
                    NamedType::Keyof => {
                        // C++ 3410-3418.
                        self.advance(GrammarContext::Type);
                        let body = self.parse_prefix_type_annotation_flow()?;
                        let node =
                            Node::KeyofTypeAnnotation(KeyofTypeAnnotation::new(
                                NodeMetadata::new(self.dummy_range()),
                                body,
                            ));
                        Some(self.set_location(
                            start,
                            self.lexer.prev_token_end(),
                            node,
                        ))
                    }
                    NamedType::Interface => {
                        // P5.3: InterfaceTypeAnnotation via
                        // parseInterfaceTailFlow (C++ 3447-3457).
                        self.error_cur(
                            "interface type annotations are unsupported (parser phase P5.3)",
                        );
                        None
                    }
                    NamedType::Infer => {
                        // C++ 3459-3504.
                        self.advance(GrammarContext::Type);

                        // C++ 3461-3462.
                        if !self.need(
                            TokenKind::identifier,
                            " in type parameter",
                        ) {
                            return None;
                        }
                        let name = self.lexer.token().get_identifier();
                        self.advance(GrammarContext::Type);

                        let mut bound: Option<&'gc Node<'gc>> = None;
                        if self.check(TokenKind::rw_extends) {
                            // When we see an extends keyword,
                            // we enter the parsing logic that might need
                            // backtracking.
                            //
                            // For `infer A extends B ...`, is the `extends B`
                            // part of an infer type, or part of a larger
                            // conditional type like
                            // `infer A extends B ? C : D`?
                            //
                            // We don't know, so we assume it's part of the
                            // infer type for now, and later backtrack if the
                            // assumption is wrong.
                            //
                            // NOTE: like the C++, diagnostics are NOT
                            // suppressed during the speculative bound parse —
                            // a failed bound emits its errors and then still
                            // restores.
                            let save_point = self.lexer.save_point();
                            self.advance(GrammarContext::Type);
                            let parsed_bound =
                                self.parse_union_type_annotation_flow();
                            if (self.allow_conditional_type.get()
                                && self.check(TokenKind::question))
                                || parsed_bound.is_none()
                            {
                                // If we look ahead and see `?`, it might be
                                // the case that we are parsing a conditional
                                // type like `infer A extends B ? C : D`. If
                                // the current context allow parsing
                                // conditional type, then we must backtrack so
                                // that only `infer A` is treated as part of
                                // the infer type.
                                //
                                // Of course, if we fail to parse the type
                                // after extends, we also need to backtrack.
                                save_point.restore(&mut self.lexer);
                            } else {
                                bound = parsed_bound;
                            }
                        }

                        // C++ 3496-3503: the TypeParameter spans the same
                        // range as the InferTypeAnnotation.
                        let end = self.lexer.prev_token_end();
                        let type_param_node =
                            Node::TypeParameter(TypeParameter::new(
                                NodeMetadata::new(self.dummy_range()),
                                name,
                                false, // const
                                bound,
                                None, // variance
                                None, // default
                                true, // usesExtendsBound
                            ));
                        let type_param =
                            self.set_location(start, end, type_param_node);
                        let node =
                            Node::InferTypeAnnotation(InferTypeAnnotation::new(
                                NodeMetadata::new(self.dummy_range()),
                                type_param,
                            ));
                        Some(self.set_location(start, end, node))
                    }
                    NamedType::Generic => self.parse_generic_type_flow(),
                }
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
                self.advance(GrammarContext::Type);
                if self.check(TokenKind::numeric_literal) {
                    // Negate the literal (C++ 3553-3563). The raw text spans
                    // from the `-` through the end of the literal token.
                    let value = -self.lexer.token().get_numeric_literal();
                    let raw =
                        self.source_bytes_atom(start, self.cur_range().end);
                    let node = Node::NumberLiteralTypeAnnotation(
                        NumberLiteralTypeAnnotation::new(
                            NodeMetadata::new(self.dummy_range()),
                            value,
                            raw,
                        ),
                    );
                    let end = self.advance(GrammarContext::Type).end;
                    Some(self.set_location(start, end, node))
                } else if self.check(TokenKind::bigint_literal) {
                    // C++ 3564-3572: the BigInt raw keeps the `-` prefix.
                    let raw =
                        self.source_bytes_atom(start, self.cur_range().end);
                    let node = Node::BigIntLiteralTypeAnnotation(
                        BigIntLiteralTypeAnnotation::new(
                            NodeMetadata::new(self.dummy_range()),
                            raw,
                        ),
                    );
                    let end = self.advance(GrammarContext::Type).end;
                    Some(self.set_location(start, end, node))
                } else {
                    // C++ 3573-3578: errorExpected(numeric_literal,
                    // "in type annotation", ...).
                    self.need(TokenKind::numeric_literal, " in type annotation");
                    None
                }
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
                    // C++ 3593-3597.
                    return self.parse_generic_type_flow();
                }
                self.error_cur("unexpected token in type annotation");
                None
            }
        }
    }

    // -----------------------------------------------------------------------
    // parseTypeofTypeAnnotationFlow — 3604 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a `typeof X.Y<Args>` type annotation, with the current token at
    /// `typeof`. Port of `parseTypeofTypeAnnotationFlow`
    /// (flow.cpp:3604-3666).
    fn parse_typeof_type_annotation_flow(&mut self) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check(TokenKind::rw_typeof));
        // C++ 3606: a bare `advance()` — the default GrammarContext
        // (AllowRegExp), NOT Type; deliberate.
        let start = self.advance(GrammarContext::AllowRegExp).start;
        let mut paren_count: u32 = 0;

        // C++ 3609-3610: default grammar context again.
        while self.check_and_eat(TokenKind::l_paren, GrammarContext::AllowRegExp)
        {
            paren_count += 1;
        }

        // C++ 3612-3613.
        if !self.need(TokenKind::identifier, " in typeof type") {
            return None;
        }

        // C++ 3615-3620.
        let ident_range = self.cur_range();
        let ident_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_identifier(),
            None,
            false,
        ));
        let mut ident =
            self.set_location(ident_range.start, ident_range.end, ident_node);
        self.advance(GrammarContext::Type);

        // C++ 3622: `checkAndEat(period)` with the default grammar context.
        while self.check_and_eat(TokenKind::period, GrammarContext::AllowRegExp)
        {
            // C++ 3623-3630.
            if !self.check(TokenKind::identifier)
                && !self.lexer.token().is_res_word()
            {
                // errorExpected(identifier, "in qualified typeof type", ...).
                self.need(TokenKind::identifier, " in qualified typeof type");
                return None;
            }
            // C++ 3631-3636.
            let next_range = self.cur_range();
            let next_node = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                self.lexer.token().get_res_word_or_identifier(),
                None,
                false,
            ));
            let next = self.set_location(
                next_range.start,
                next_range.end,
                next_node,
            );
            self.advance(GrammarContext::Type);
            // C++ 3637-3640: spans from the qualification's start to the new
            // id's end.
            let q_node = Node::QualifiedTypeofIdentifier(
                QualifiedTypeofIdentifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    ident,
                    next,
                ),
            );
            ident = self.set_location(
                ident.metadata().range.get().start,
                next_range.end,
                q_node,
            );
        }

        // C++ 3643-3651: close the wrapping parens, recording them on the
        // (possibly qualified) identifier node.
        for _ in 0..paren_count {
            if !self.eat(
                TokenKind::r_paren,
                GrammarContext::Type,
                " in typeof type",
            ) {
                return None;
            }
            inc_parens(ident);
        }

        // C++ 3653-3660: `parseTypeArgsFlow()` is called with its default
        // trailing grammar context (Type, per JSParserImpl.h:1506).
        let mut type_arguments: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less)
            && !self.lexer.is_new_line_before_current_token()
        {
            type_arguments =
                Some(self.parse_type_args_flow(GrammarContext::Type)?);
        }

        // C++ 3662-3665.
        let node = Node::TypeofTypeAnnotation(TypeofTypeAnnotation::new(
            NodeMetadata::new(self.dummy_range()),
            ident,
            type_arguments,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseTupleTypeAnnotationFlow — 3668 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a tuple type annotation, with the current token at `[`.
    /// Port of `parseTupleTypeAnnotationFlow` (flow.cpp:3668-3712).
    fn parse_tuple_type_annotation_flow(&mut self) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check(TokenKind::l_square));
        // C++ 3670.
        let start = self.advance(GrammarContext::Type).start;

        let mut element_types: Vec<&'gc Node<'gc>> = Vec::new();
        let mut inexact = false;

        // C++ 3675-3698.
        while !self.check(TokenKind::r_square) {
            let elem_start = self.cur_start();
            let starts_with_dotdotdot =
                self.check_and_eat(TokenKind::dotdotdot, GrammarContext::Type);

            if starts_with_dotdotdot && self.check(TokenKind::r_square) {
                // ...]
                inexact = true;
            } else if starts_with_dotdotdot && self.check(TokenKind::comma) {
                // ...,
                self.error_cur(
                    "trailing commas after inexact tuple types are not allowed",
                );
                self.advance(GrammarContext::Type);
            } else {
                let elem = self.parse_tuple_element_flow(
                    elem_start,
                    starts_with_dotdotdot,
                )?;
                element_types.push(elem);

                if !self.check_and_eat(TokenKind::comma, GrammarContext::Type) {
                    break;
                }
            }
        }

        // C++ 3700-3705.
        if !self.need(TokenKind::r_square, " at end of tuple type annotation") {
            return None;
        }

        // C++ 3707-3711.
        let node = Node::TupleTypeAnnotation(TupleTypeAnnotation::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, element_types),
            inexact,
        ));
        let end = self.advance(GrammarContext::Type).end;
        Some(self.set_location(start, end, node))
    }

    /// Parse one tuple type element, with `start` at the element's start
    /// (including a leading `...`, already consumed iff
    /// `starts_with_dotdotdot`). Port of `parseTupleElementFlow`
    /// (flow.cpp:3714-3814).
    fn parse_tuple_element_flow(
        &mut self,
        start: SMLoc,
        starts_with_dotdotdot: bool,
    ) -> Option<&'gc Node<'gc>> {
        let mut variance: Option<&'gc Node<'gc>> = None;

        // ...Identifier : Type
        // ...Type
        // ^
        if starts_with_dotdotdot {
            // C++ 3725-3748.
            let ty = self.parse_type_annotation_before_colon_flow()?;
            if self.check_and_eat(TokenKind::colon, GrammarContext::Type) {
                let label =
                    self.reparse_type_annotation_as_identifier_flow(ty)?;
                let element_type = self.parse_type_annotation_flow(
                    None,
                    AllowAnonFunctionType::Yes,
                )?;
                let node = Node::TupleTypeSpreadElement(
                    TupleTypeSpreadElement::new(
                        NodeMetadata::new(self.dummy_range()),
                        Some(label),
                        element_type,
                    ),
                );
                return Some(self.set_location(
                    start,
                    self.lexer.prev_token_end(),
                    node,
                ));
            }

            let node = Node::TupleTypeSpreadElement(
                TupleTypeSpreadElement::new(
                    NodeMetadata::new(self.dummy_range()),
                    None, // label
                    ty,
                ),
            );
            return Some(self.set_location(
                start,
                self.lexer.prev_token_end(),
                node,
            ));
        }

        // +Identifier : Type
        // -Identifier : Type
        // readonly Identifier : Type
        // writeonly Identifier : Type
        // ^
        if self.check2(TokenKind::plus, TokenKind::minus) {
            // C++ 3756-3762: the Variance kind is the interned "plus" /
            // "minus" atom (plusIdent_ / minusIdent_).
            let kind: &[u8] = if self.check(TokenKind::plus) {
                b"plus"
            } else {
                b"minus"
            };
            let v_range = self.cur_range();
            let v_node = Node::Variance(Variance::new(
                NodeMetadata::new(self.dummy_range()),
                self.lexer.get_identifier(kind),
            ));
            variance =
                Some(self.set_location(v_range.start, v_range.end, v_node));
            self.advance(GrammarContext::Type);
        } else if (self.check_name(b"readonly")
            || self.check_name(b"writeonly"))
            && can_follow_variance_keyword_flow(
                self.lexer.lookahead1::<true>(None),
            )
        {
            // C++ 3763-3768.
            let v_range = self.cur_range();
            let v_node = Node::Variance(Variance::new(
                NodeMetadata::new(self.dummy_range()),
                self.lexer.token().get_identifier(),
            ));
            variance =
                Some(self.set_location(v_range.start, v_range.end, v_node));
            self.advance(GrammarContext::Type);
        }

        // Identifier [?] : Type
        // Type
        // ^
        let ty = self.parse_type_annotation_before_colon_flow()?;

        // Identifier [?] : Type
        //             ^
        if self.check2(TokenKind::colon, TokenKind::question) {
            // C++ 3781-3783.
            let optional =
                self.check_and_eat(TokenKind::question, GrammarContext::Type);

            // C++ 3785-3792.
            if !self.eat(
                TokenKind::colon,
                GrammarContext::Type,
                " in labeled tuple type element",
            ) {
                return None;
            }

            let label = self.reparse_type_annotation_as_identifier_flow(ty)?;
            let element_type = self
                .parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

            // C++ 3800-3804.
            let node = Node::TupleTypeLabeledElement(
                TupleTypeLabeledElement::new(
                    NodeMetadata::new(self.dummy_range()),
                    label,
                    element_type,
                    optional,
                    variance,
                ),
            );
            return Some(self.set_location(
                start,
                self.lexer.prev_token_end(),
                node,
            ));
        }

        // C++ 3806-3811.
        if let Some(variance) = variance {
            let range = variance.metadata().range.get();
            self.error_at(
                range,
                "Variance can only be used with labeled tuple elements",
            );
        }

        Some(ty)
    }

    // -----------------------------------------------------------------------
    // parseFunctionTypeAnnotationFlow — 3823 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a (possibly generic) function type annotation
    /// `<T>(params) => R`. Port of `parseFunctionTypeAnnotationFlow`
    /// (flow.cpp:3823-3825).
    fn parse_function_type_annotation_flow(&mut self) -> Option<&'gc Node<'gc>> {
        self.parse_function_or_hook_type_annotation_flow(false)
    }

    /// Parse a function (or, P6, hook) type annotation with the current token
    /// at `<` or `(`. Port of `parseFunctionOrHookTypeAnnotationFlow`
    /// (flow.cpp:3827-3863). `hook` is threaded like the C++ bool; the only
    /// P5 caller passes false (`parseHookTypeAnnotationFlow` is P6).
    fn parse_function_or_hook_type_annotation_flow(
        &mut self,
        hook: bool,
    ) -> Option<&'gc Node<'gc>> {
        let start = self.cur_start();

        // C++ 3831-3837.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_params = Some(self.parse_type_params_flow()?);
        }

        // C++ 3839-3844.
        if !self.need(TokenKind::l_paren, " in function type annotation") {
            return None;
        }

        // C++ 3846-3852.
        let mut params: Vec<&'gc Node<'gc>> = Vec::new();
        let mut this_constraint: Option<&'gc Node<'gc>> = None;
        let rest = self.parse_function_type_annotation_params_flow(
            &mut params,
            &mut this_constraint,
            hook,
        )?;

        // C++ 3854-3859.
        if !self.need(TokenKind::equalgreater, " in function type annotation") {
            return None;
        }

        // C++ 3861-3862.
        self.parse_function_type_annotation_with_params_flow(
            start,
            params,
            this_constraint,
            rest,
            type_params,
            hook,
        )
    }

    // -----------------------------------------------------------------------
    // parseFunctionOrGroupTypeAnnotationFlow — 3899 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a parenthesized group type `(T)` or a parenthesized function
    /// type `(params) => R`, with the current token at `(`. Port of
    /// `parseFunctionOrGroupTypeAnnotationFlow` (flow.cpp:3899-4032).
    fn parse_function_or_group_type_annotation_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check(TokenKind::l_paren));
        // This is either
        // ( Type )
        // ^
        // or
        // ( ParamList ) => Type
        // ^
        // so we use a similar approach to arrow function parameters by
        // keeping track and reparsing in certain cases.
        let start = self.advance(GrammarContext::Type).start;

        let mut is_function = false;
        let mut ty: Option<&'gc Node<'gc>> = None;
        let mut rest: Option<&'gc Node<'gc>> = None;
        let mut params: Vec<&'gc Node<'gc>> = Vec::new();
        let mut this_constraint: Option<&'gc Node<'gc>> = None;

        // C++ 3918-3937: a leading `this: T` constraint.
        if self.check(TokenKind::rw_this) {
            let opt_next = self.lexer.lookahead1::<true>(None);
            if opt_next == Some(TokenKind::colon) {
                let this_start = self.advance(GrammarContext::Type).start;
                self.advance(GrammarContext::Type);
                let type_annotation = self.parse_type_annotation_flow(
                    None,
                    AllowAnonFunctionType::Yes,
                )?;

                let ftp_node = Node::FunctionTypeParam(FunctionTypeParam::new(
                    NodeMetadata::new(self.dummy_range()),
                    None, // name
                    type_annotation,
                    false, // optional
                ));
                this_constraint = Some(self.set_location(
                    this_start,
                    self.lexer.prev_token_end(),
                    ftp_node,
                ));
                self.check_and_eat(TokenKind::comma, GrammarContext::Type);
            } else if opt_next == Some(TokenKind::question) {
                self.error_cur("'this' constraint may not be optional");
            }
        }

        // C++ 3939-3965.
        if self.allow_anon_function_type.get()
            && self.check_and_eat(TokenKind::dotdotdot, GrammarContext::Type)
        {
            is_function = true;
            // Must be parameters, and this must be the last one.
            // Rest param must be the last param.
            rest = Some(self.parse_function_type_annotation_param_flow()?);
        } else if self.check(TokenKind::r_paren) {
            is_function = true;
            // ( )
            //   ^
            // No parameters, but this must be an empty param list.
        } else {
            // Not sure yet whether this is a param or simply a type.
            let param = self.parse_function_type_annotation_param_flow()?;
            let ftp = param
                .as_function_type_param()
                .expect("param parser returns FunctionTypeParam");
            ty = Some(ftp.type_annotation);
            if ftp.name.is_some() || ftp.optional.get() {
                // Must be a param if it has a name or if it was optional.
                is_function = true;
            }
            params.push(param);
        }

        // If isFunction was already forced by something previously then we
        // have no choice but to attempt to parse as a function type
        // annotation. C++ 3969-3990.
        if (is_function || self.allow_anon_function_type.get())
            && self.check_and_eat(TokenKind::comma, GrammarContext::Type)
        {
            is_function = true;
            while !self.check(TokenKind::r_paren) {
                let is_rest = rest.is_none()
                    && self.check_and_eat(
                        TokenKind::dotdotdot,
                        GrammarContext::Type,
                    );

                let param = self.parse_function_type_annotation_param_flow()?;
                if is_rest {
                    rest = Some(param);
                    self.check_and_eat(TokenKind::comma, GrammarContext::Type);
                    break;
                } else {
                    params.push(param);
                }

                if !self.check_and_eat(TokenKind::comma, GrammarContext::Type)
                {
                    break;
                }
            }
        }

        // C++ 3992-3998.
        if !self.eat(
            TokenKind::r_paren,
            GrammarContext::Type,
            " at end of function annotation parameters",
        ) {
            return None;
        }

        // C++ 4000-4012.
        if is_function {
            if !self.eat(
                TokenKind::equalgreater,
                GrammarContext::Type,
                " in function type annotation",
            ) {
                return None;
            }
        } else if self.allow_anon_function_type.get()
            && self.check_and_eat(TokenKind::equalgreater, GrammarContext::Type)
        {
            is_function = true;
        }

        // C++ 4014-4017: a plain parenthesized group — return the inner type
        // with its paren count bumped.
        if !is_function {
            let ty =
                ty.expect("non-function group type must have an inner type");
            inc_parens(ty);
            return Some(ty);
        }

        // C++ 4019-4024.
        let return_type = self.parse_return_type_annotation_flow(
            None,
            if self.allow_anon_function_type.get() {
                AllowAnonFunctionType::Yes
            } else {
                AllowAnonFunctionType::No
            },
        )?;

        // C++ 4026-4031: a function type reached through the group cover
        // never has type parameters.
        let node = Node::FunctionTypeAnnotation(FunctionTypeAnnotation::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, params),
            this_constraint,
            return_type,
            rest,
            None, // typeParams
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseObjectTypeAnnotationFlow — 4034 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse an object type annotation, with the current token at `{` or
    /// `{|`. Port of `parseObjectTypeAnnotationFlow` (flow.cpp:4034-4085).
    pub(super) fn parse_object_type_annotation_flow(
        &mut self,
        allow_proto_property: AllowProtoProperty,
        allow_static_property: AllowStaticProperty,
        allow_spread_property: AllowSpreadProperty,
    ) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check2(TokenKind::l_brace, TokenKind::l_bracepipe));
        let exact = self.check(TokenKind::l_bracepipe);
        let start = self.advance(GrammarContext::Type).start;

        let mut properties: Vec<&'gc Node<'gc>> = Vec::new();
        let mut indexers: Vec<&'gc Node<'gc>> = Vec::new();
        let mut call_properties: Vec<&'gc Node<'gc>> = Vec::new();
        let mut internal_slots: Vec<&'gc Node<'gc>> = Vec::new();
        let mut inexact = false;

        // C++ 4048-4057.
        if !self.parse_object_type_properties_flow(
            allow_proto_property,
            allow_static_property,
            allow_spread_property,
            &mut properties,
            &mut indexers,
            &mut call_properties,
            &mut internal_slots,
            &mut inexact,
        ) {
            return None;
        }

        // C++ 4059-4064.
        if exact && inexact {
            // Doesn't prevent parsing from continuing, but it is an error.
            self.error_at_loc(
                start,
                "Explicit inexact syntax cannot appear inside an explicit exact object type",
            );
        }

        // C++ 4066-4073.
        let end = self.cur_range().end;
        if !self.eat(
            if exact {
                TokenKind::piper_brace
            } else {
                TokenKind::r_brace
            },
            GrammarContext::Type,
            " at end of exact object type annotation",
        ) {
            return None;
        }

        // C++ 4075-4084.
        let node = Node::ObjectTypeAnnotation(ObjectTypeAnnotation::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, properties),
            NodeList::from_iter(self.gc, indexers),
            NodeList::from_iter(self.gc, call_properties),
            NodeList::from_iter(self.gc, internal_slots),
            inexact,
            exact,
        ));
        Some(self.set_location(start, end, node))
    }

    /// Parse the members of an object type into the four out-lists, leaving
    /// the closing `}`/`|}` as the current token. Returns false if an error
    /// was reported. Port of `parseObjectTypePropertiesFlow`
    /// (flow.cpp:4087-4151).
    #[allow(clippy::too_many_arguments)] // faithful to the C++ signature.
    fn parse_object_type_properties_flow(
        &mut self,
        allow_proto_property: AllowProtoProperty,
        allow_static_property: AllowStaticProperty,
        allow_spread_property: AllowSpreadProperty,
        properties: &mut Vec<&'gc Node<'gc>>,
        indexers: &mut Vec<&'gc Node<'gc>>,
        call_properties: &mut Vec<&'gc Node<'gc>>,
        internal_slots: &mut Vec<&'gc Node<'gc>>,
        inexact: &mut bool,
    ) -> bool {
        while !self.check2(TokenKind::r_brace, TokenKind::piper_brace) {
            let start = self.cur_start();
            if self.check(TokenKind::dotdotdot) {
                // Spread property or explicit '...' for inexact.
                self.advance(GrammarContext::Type);
                if self.check2(TokenKind::comma, TokenKind::semi) {
                    // C++ 4101-4105.
                    *inexact = true;
                    self.advance(GrammarContext::Type);
                    // Explicit '...' must be the last element in the type
                    // annotation.
                    return true;
                } else if self.check2(
                    TokenKind::r_brace,
                    TokenKind::piper_brace,
                ) {
                    // C++ 4106-4108.
                    *inexact = true;
                    return true;
                } else {
                    // C++ 4109-4121.
                    if allow_spread_property == AllowSpreadProperty::No {
                        self.error_at_loc(
                            start,
                            "Spreading a type is only allowed inside an object type",
                        );
                    }
                    let Some(spread_type) = self.parse_type_annotation_flow(
                        None,
                        AllowAnonFunctionType::Yes,
                    ) else {
                        return false;
                    };
                    let node = Node::ObjectTypeSpreadProperty(
                        ObjectTypeSpreadProperty::new(
                            NodeMetadata::new(self.dummy_range()),
                            spread_type,
                        ),
                    );
                    let located = self.set_location(
                        start,
                        self.lexer.prev_token_end(),
                        node,
                    );
                    properties.push(located);
                }
            } else {
                // C++ 4122-4131.
                if !self.parse_property_type_annotation_flow(
                    allow_proto_property,
                    allow_static_property,
                    properties,
                    indexers,
                    call_properties,
                    internal_slots,
                ) {
                    return false;
                }
            }

            // C++ 4133-4147.
            if self.check2(TokenKind::comma, TokenKind::semi) {
                self.advance(GrammarContext::Type);
            } else if self.check2(TokenKind::r_brace, TokenKind::piper_brace) {
                return true;
            } else {
                self.error_expected4(
                    TokenKind::comma,
                    TokenKind::semi,
                    TokenKind::r_brace,
                    TokenKind::piper_brace,
                    " after property",
                );
                return false;
            }
        }

        true
    }

    /// Parse one object-type member (property, method, accessor, call
    /// property, indexer, mapped type, or internal slot), pushing it into the
    /// appropriate out-list. Returns false if an error was reported. Port of
    /// `parsePropertyTypeAnnotationFlow` (flow.cpp:4153-4439).
    fn parse_property_type_annotation_flow(
        &mut self,
        allow_proto_property: AllowProtoProperty,
        allow_static_property: AllowStaticProperty,
        properties: &mut Vec<&'gc Node<'gc>>,
        indexers: &mut Vec<&'gc Node<'gc>>,
        call_properties: &mut Vec<&'gc Node<'gc>>,
        internal_slots: &mut Vec<&'gc Node<'gc>>,
    ) -> bool {
        let start_range = self.cur_range();
        let start = start_range.start;

        let mut variance: Option<&'gc Node<'gc>> = None;
        let mut is_static = false;
        let mut proto = false;

        // C++ 4167-4170.
        if self.check_name(b"proto") {
            proto = true;
            self.advance(GrammarContext::Type);
        }

        // C++ 4172-4175.
        if !proto
            && (self.check(TokenKind::rw_static) || self.check_name(b"static"))
        {
            is_static = true;
            self.advance(GrammarContext::Type);
        }

        // C++ 4177-4190.
        if self.check2(TokenKind::plus, TokenKind::minus) {
            let kind: &[u8] = if self.check(TokenKind::plus) {
                b"plus"
            } else {
                b"minus"
            };
            let v_range = self.cur_range();
            let v_node = Node::Variance(Variance::new(
                NodeMetadata::new(self.dummy_range()),
                self.lexer.get_identifier(kind),
            ));
            variance =
                Some(self.set_location(v_range.start, v_range.end, v_node));
            self.advance(GrammarContext::Type);
        } else if (self.check_name(b"readonly")
            || self.check_name(b"writeonly"))
            && can_follow_variance_keyword_flow(
                self.lexer.lookahead1::<true>(None),
            )
        {
            let v_range = self.cur_range();
            let v_node = Node::Variance(Variance::new(
                NodeMetadata::new(self.dummy_range()),
                self.lexer.token().get_identifier(),
            ));
            variance =
                Some(self.set_location(v_range.start, v_range.end, v_node));
            self.advance(GrammarContext::Type);
        }

        // C++ 4192-4315.
        if self.check_and_eat(TokenKind::l_square, GrammarContext::Type) {
            if self.check_and_eat(TokenKind::l_square, GrammarContext::Type) {
                // Internal slot `[[id]]` (C++ 4193-4274).
                if let Some(variance) = variance {
                    let range = variance.metadata().range.get();
                    self.error_at(range, "Unexpected variance sigil");
                }
                if proto {
                    self.error_at(start_range, "invalid 'proto' modifier");
                }
                if is_static
                    && allow_static_property == AllowStaticProperty::No
                {
                    self.error_at(start_range, "invalid 'static' modifier");
                }
                // C++ 4204-4211.
                if !self.check(TokenKind::identifier)
                    && !self.lexer.token().is_res_word()
                {
                    // errorExpected(identifier, "in internal slot", ...).
                    self.need(TokenKind::identifier, " in internal slot");
                    return false;
                }
                // C++ 4212-4217.
                let id_range = self.cur_range();
                let id_node = Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    self.lexer.token().get_res_word_or_identifier(),
                    None,
                    false,
                ));
                let id = self.set_location(
                    id_range.start,
                    id_range.end,
                    id_node,
                );
                self.advance(GrammarContext::Type);

                // C++ 4219-4232.
                if !self.eat(
                    TokenKind::r_square,
                    GrammarContext::Type,
                    " at end of internal slot",
                ) {
                    return false;
                }
                if !self.eat(
                    TokenKind::r_square,
                    GrammarContext::Type,
                    " at end of internal slot",
                ) {
                    return false;
                }

                let mut optional = false;
                let method;
                let value;

                if self.check2(TokenKind::less, TokenKind::l_paren) {
                    // Type params and method (C++ 4238-4251).
                    method = true;
                    let mut type_params: Option<&'gc Node<'gc>> = None;
                    if self.check(TokenKind::less) {
                        let Some(tp) = self.parse_type_params_flow() else {
                            return false;
                        };
                        type_params = Some(tp);
                    }
                    let Some(methodish) = self
                        .parse_methodish_type_annotation_flow(
                            start,
                            type_params,
                        )
                    else {
                        return false;
                    };
                    value = methodish;
                } else {
                    // Standard type annotation (C++ 4252-4267).
                    method = false;
                    optional = self.check_and_eat(
                        TokenKind::question,
                        GrammarContext::Type,
                    );
                    if !self.eat(
                        TokenKind::colon,
                        GrammarContext::Type,
                        " in type annotation",
                    ) {
                        return false;
                    }
                    let Some(v) = self.parse_type_annotation_flow(
                        None,
                        AllowAnonFunctionType::Yes,
                    ) else {
                        return false;
                    };
                    value = v;
                }

                // C++ 4269-4274.
                let node = Node::ObjectTypeInternalSlot(
                    ObjectTypeInternalSlot::new(
                        NodeMetadata::new(self.dummy_range()),
                        id,
                        value,
                        optional,
                        is_static,
                        method,
                    ),
                );
                let located = self.set_location(
                    start,
                    self.lexer.prev_token_end(),
                    node,
                );
                internal_slots.push(located);
            } else {
                // Indexer or Mapped Type (C++ 4275-4313).
                // We can have
                // [ Identifier : TypeAnnotation ]
                //   ^
                // or
                // [ TypeAnnotation ]
                //   ^
                // or
                // [ TypeParameter in TypeAnnotation ]
                //   ^
                // Because we cannot differentiate without looking ahead for
                // the `in` or `:`, we call `parseTypeAnnotation`, check for
                // the next token and then convert the TypeAnnotation to the
                // appropriate node.
                let Some(left) = self.parse_type_annotation_before_colon_flow()
                else {
                    return false;
                };

                if self.check_and_eat(TokenKind::rw_in, GrammarContext::Type) {
                    let Some(prop) = self.parse_type_mapped_type_property_flow(
                        start, left, variance,
                    ) else {
                        return false;
                    };
                    properties.push(prop);
                } else {
                    let Some(indexer) = self.parse_type_indexer_property_flow(
                        start, left, variance, is_static,
                    ) else {
                        return false;
                    };
                    indexers.push(indexer);
                }

                // C++ 4307-4312.
                if proto {
                    self.error_at(start_range, "invalid 'proto' modifier");
                }
                if is_static
                    && allow_static_property == AllowStaticProperty::No
                {
                    self.error_at(start_range, "invalid 'static' modifier");
                }
            }
            return true;
        }

        // C++ 4319-4351.
        if self.check2(TokenKind::less, TokenKind::l_paren) {
            // C++ 4320-4337: a consumed `static`/`proto` that is not allowed
            // as a modifier here was actually the method name.
            if (is_static && allow_static_property == AllowStaticProperty::No)
                || (proto && allow_proto_property == AllowProtoProperty::No)
            {
                let key_node = Node::Identifier(Identifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    self.lexer.get_identifier(if is_static {
                        b"static"
                    } else {
                        b"proto"
                    }),
                    None,
                    false,
                ));
                let key = self.set_location(
                    start_range.start,
                    start_range.end,
                    key_node,
                );
                // The C++ (4327-4328) also clears `proto`; it is never read
                // again on this path, so only `is_static` (passed below) is
                // reset here.
                is_static = false;
                if let Some(variance) = variance {
                    let range = variance.metadata().range.get();
                    self.error_at(range, "Unexpected variance sigil");
                }
                let Some(prop) =
                    self.parse_method_type_property_flow(start, is_static, key)
                else {
                    return false;
                };
                properties.push(prop);
                return true;
            }
            // C++ 4338-4350.
            if let Some(variance) = variance {
                let range = variance.metadata().range.get();
                self.error_at(range, "call property must not specify variance");
            }
            if proto {
                self.error_at(start_range, "invalid 'proto' modifier");
            }
            let Some(call) = self.parse_type_call_property_flow(start, is_static)
            else {
                return false;
            };
            call_properties.push(call);
            return true;
        }

        // C++ 4353-4369: a consumed `static`/`proto` directly followed by
        // `:`/`?` was actually the property name.
        if (is_static || proto)
            && self.check2(TokenKind::colon, TokenKind::question)
        {
            if let Some(variance) = variance {
                let range = variance.metadata().range.get();
                self.error_at(range, "Unexpected variance sigil");
            }
            let key_node = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                self.lexer.get_identifier(if is_static {
                    b"static"
                } else {
                    b"proto"
                }),
                None,
                false,
            ));
            let key = self.set_location(
                start_range.start,
                start_range.end,
                key_node,
            );
            is_static = false;
            proto = false;
            let Some(prop) = self.parse_type_property_flow(
                start, variance, is_static, proto, key,
            ) else {
                return false;
            };
            properties.push(prop);
            return true;
        }

        // C++ 4371-4374.
        let Some(key) = self.parse_property_name() else {
            return false;
        };

        // C++ 4376-4391.
        if self.check2(TokenKind::less, TokenKind::l_paren) {
            if let Some(variance) = variance {
                let range = variance.metadata().range.get();
                self.error_at(range, "Unexpected variance sigil");
            }
            if proto {
                self.error_at(start_range, "invalid 'proto' modifier");
            }
            if is_static && allow_static_property == AllowStaticProperty::No {
                self.error_at(start_range, "invalid 'static' modifier");
            }
            let Some(prop) =
                self.parse_method_type_property_flow(start, is_static, key)
            else {
                return false;
            };
            properties.push(prop);
            return true;
        }

        // C++ 4393-4405.
        if self.check2(TokenKind::colon, TokenKind::question) {
            if proto && allow_proto_property == AllowProtoProperty::No {
                self.error_at(start_range, "invalid 'proto' modifier");
            }
            if is_static && allow_static_property == AllowStaticProperty::No {
                self.error_at(start_range, "invalid 'static' modifier");
            }
            let Some(prop) = self.parse_type_property_flow(
                start, variance, is_static, proto, key,
            ) else {
                return false;
            };
            properties.push(prop);
            return true;
        }

        // C++ 4407-4431: a `get`/`set` accessor — the parsed key was the
        // accessor specifier and the real key follows.
        if let Node::Identifier(ident) = key {
            let (is_getter, is_setter) = {
                let bytes =
                    self.lexer.get_string_table().bytes(ident.name.get());
                (bytes == b"get", bytes == b"set")
            };
            if is_getter || is_setter {
                if let Some(variance) = variance {
                    let range = variance.metadata().range.get();
                    self.error_at(
                        range,
                        "accessor property must not specify variance",
                    );
                }
                if proto {
                    self.error_at(start_range, "invalid 'proto' modifier");
                }
                if is_static
                    && allow_static_property == AllowStaticProperty::No
                {
                    self.error_at(start_range, "invalid 'static' modifier");
                }
                let Some(key) = self.parse_property_name() else {
                    return false;
                };
                let Some(get_set) = self.parse_get_or_set_type_property_flow(
                    start, is_static, is_getter, key,
                ) else {
                    return false;
                };
                properties.push(get_set);
                return true;
            }
        }

        // C++ 4433-4438.
        self.error_expected2(
            TokenKind::colon,
            TokenKind::question,
            " in property type annotation",
        );
        false
    }

    /// Parse the `[?] : T` tail of a plain object-type property. Port of
    /// `parseTypePropertyFlow` (flow.cpp:4441-4472).
    fn parse_type_property_flow(
        &mut self,
        start: SMLoc,
        variance: Option<&'gc Node<'gc>>,
        is_static: bool,
        proto: bool,
        key: &'gc Node<'gc>,
    ) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check2(TokenKind::colon, TokenKind::question));

        // C++ 4449-4450.
        let optional =
            self.check_and_eat(TokenKind::question, GrammarContext::Type);
        // C++ 4451-4457.
        if !self.eat(
            TokenKind::colon,
            GrammarContext::Type,
            " in type property",
        ) {
            return None;
        }

        // C++ 4459-4462.
        let value = self
            .parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

        // C++ 4464-4471.
        let node = Node::ObjectTypeProperty(ObjectTypeProperty::new(
            NodeMetadata::new(self.dummy_range()),
            key,
            value,
            false, // method
            optional,
            is_static,
            proto,
            variance,
            self.lexer.get_identifier(b"init"),
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    /// Parse the `<T>(params): R` tail of an object-type method property.
    /// Port of `parseMethodTypePropertyFlow` (flow.cpp:4474-4510).
    fn parse_method_type_property_flow(
        &mut self,
        start: SMLoc,
        is_static: bool,
        key: &'gc Node<'gc>,
    ) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check2(TokenKind::less, TokenKind::l_paren));

        // C++ 4480-4486.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_params = Some(self.parse_type_params_flow()?);
        }

        // C++ 4488-4491.
        let value =
            self.parse_methodish_type_annotation_flow(start, type_params)?;

        // C++ 4493-4509.
        let node = Node::ObjectTypeProperty(ObjectTypeProperty::new(
            NodeMetadata::new(self.dummy_range()),
            key,
            value,
            true,  // method
            false, // optional
            is_static,
            false, // proto
            None,  // variance
            self.lexer.get_identifier(b"init"),
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    /// Parse the `(params): R` tail of an object-type accessor property,
    /// checking the accessor arity. Port of `parseGetOrSetTypePropertyFlow`
    /// (flow.cpp:4512-4550).
    fn parse_get_or_set_type_property_flow(
        &mut self,
        start: SMLoc,
        is_static: bool,
        is_getter: bool,
        key: &'gc Node<'gc>,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 4517-4519.
        let value = self.parse_methodish_type_annotation_flow(start, None)?;
        let fta = value
            .as_function_type_annotation()
            .expect("methodish parser returns FunctionTypeAnnotation");

        // Check the number of parameters, but we can continue parsing anyway
        // (C++ 4528-4537).
        if is_getter {
            if !fta.params.is_empty() {
                let range = value.metadata().range.get();
                self.error_at(range, "Getter must have 0 parameters");
            }
        } else if fta.params.iter().count() != 1 {
            let range = value.metadata().range.get();
            self.error_at(range, "Setter must have 1 parameter");
        }

        // C++ 4539-4543.
        if let Some(this_constraint) = fta.this {
            let range = this_constraint.metadata().range.get();
            self.error_at(range, "Accessors must not have 'this' annotations");
        }

        // C++ 4545-4549.
        let kind: &[u8] = if is_getter { b"get" } else { b"set" };
        let node = Node::ObjectTypeProperty(ObjectTypeProperty::new(
            NodeMetadata::new(self.dummy_range()),
            key,
            value,
            false, // method
            false, // optional
            is_static,
            false, // proto
            None,  // variance
            self.lexer.get_identifier(kind),
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    /// Parse the rest of a mapped type member `[K in T][+?/-?/?]: V`, with
    /// `left` the already-parsed key and `in` consumed. Port of
    /// `parseTypeMappedTypePropertyFlow` (flow.cpp:4552-4620).
    fn parse_type_mapped_type_property_flow(
        &mut self,
        start: SMLoc,
        left: &'gc Node<'gc>,
        variance: Option<&'gc Node<'gc>>,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 4556-4564: the key reparses as a bare type parameter spanning
        // exactly `left`'s range.
        let id = self.reparse_type_annotation_as_id_flow(left)?;
        let left_range = left.metadata().range.get();
        let key_tparam_node = Node::TypeParameter(TypeParameter::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            false, // const
            None,  // bound
            None,  // variance
            None,  // default
            false, // usesExtendsBound
        ));
        let key_tparam = self.set_location(
            left_range.start,
            left_range.end,
            key_tparam_node,
        );

        // C++ 4566-4568.
        let source_type = self
            .parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

        // C++ 4570-4576.
        if !self.eat(
            TokenKind::r_square,
            GrammarContext::Type,
            " in mapped type",
        ) {
            return None;
        }

        // C++ 4578-4601: the optionality sigil. The C++ passes a null
        // UniqueString when there is no sigil; the dumper emits
        // `"optional": null` — INVALID_ATOM_BYTES is the Rust null
        // NodeString.
        let mut optional: NodeString = INVALID_ATOM_BYTES;
        if self.check_and_eat(TokenKind::plus, GrammarContext::Type) {
            if !self.eat(
                TokenKind::question,
                GrammarContext::Type,
                " in mapped type",
            ) {
                return None;
            }
            optional = self.lexer.get_identifier(b"PlusOptional");
        } else if self.check_and_eat(TokenKind::minus, GrammarContext::Type) {
            if !self.eat(
                TokenKind::question,
                GrammarContext::Type,
                " in mapped type",
            ) {
                return None;
            }
            optional = self.lexer.get_identifier(b"MinusOptional");
        } else if self.check_and_eat(TokenKind::question, GrammarContext::Type)
        {
            optional = self.lexer.get_identifier(b"Optional");
        }

        // C++ 4603-4609.
        if !self.eat(
            TokenKind::colon,
            GrammarContext::Type,
            " in mapped type",
        ) {
            return None;
        }

        // C++ 4611-4613.
        let prop_type = self
            .parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

        // C++ 4615-4619.
        let node = Node::ObjectTypeMappedTypeProperty(
            ObjectTypeMappedTypeProperty::new(
                NodeMetadata::new(self.dummy_range()),
                key_tparam,
                prop_type,
                source_type,
                variance,
                optional,
            ),
        );
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    /// Parse the rest of an indexer member `[id: K]: V` / `[K]: V`, with
    /// `left` the already-parsed bracket contents (or its `id` part). Port of
    /// `parseTypeIndexerPropertyFlow` (flow.cpp:4622-4669).
    fn parse_type_indexer_property_flow(
        &mut self,
        start: SMLoc,
        left: &'gc Node<'gc>,
        variance: Option<&'gc Node<'gc>>,
        is_static: bool,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 4627-4641.
        let id: Option<&'gc Node<'gc>>;
        let key: &'gc Node<'gc>;
        if self.check_and_eat(TokenKind::colon, GrammarContext::Type) {
            id = Some(self.reparse_type_annotation_as_identifier_flow(left)?);
            key = self
                .parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;
        } else {
            id = None;
            key = left;
        }

        // C++ 4643-4649.
        if !self.eat(TokenKind::r_square, GrammarContext::Type, " in indexer")
        {
            return None;
        }

        // C++ 4651-4657.
        if !self.eat(TokenKind::colon, GrammarContext::Type, " in indexer") {
            return None;
        }

        // C++ 4659-4662.
        let value = self
            .parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

        // C++ 4664-4668.
        let node = Node::ObjectTypeIndexer(ObjectTypeIndexer::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            key,
            value,
            is_static,
            variance,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    /// Parse an object-type call property `<T>(params): R`. Port of
    /// `parseTypeCallPropertyFlow` (flow.cpp:4671-4688).
    fn parse_type_call_property_flow(
        &mut self,
        start: SMLoc,
        is_static: bool,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 4674-4680.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_params = Some(self.parse_type_params_flow()?);
        }
        // C++ 4681-4683.
        let value =
            self.parse_methodish_type_annotation_flow(start, type_params)?;
        // C++ 4684-4687.
        let node = Node::ObjectTypeCallProperty(ObjectTypeCallProperty::new(
            NodeMetadata::new(self.dummy_range()),
            value,
            is_static,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseTypeParamsFlow — 4690 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a type-parameter declaration `<T, U: B, ...>`, with the current
    /// token at `<`. At least one parameter is required (empty `<>` is an
    /// error); a trailing comma is allowed. Port of `parseTypeParamsFlow`
    /// (flow.cpp:4690-4719).
    pub(super) fn parse_type_params_flow(&mut self) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check(TokenKind::less));
        // C++ 4692.
        let start = self.advance(GrammarContext::Type).start;

        let mut params: Vec<&'gc Node<'gc>> = Vec::new();

        // C++ 4696-4704: a do-while — at least one parameter is required.
        loop {
            params.push(self.parse_type_param_flow()?);

            if !self.check_and_eat(TokenKind::comma, GrammarContext::Type) {
                break;
            }
            if self.check(TokenKind::greater) {
                break;
            }
        }

        // C++ 4706-4713.
        let end = self.cur_range().end;
        if !self.eat(
            TokenKind::greater,
            GrammarContext::Type,
            " at end of type parameters",
        ) {
            return None;
        }

        // C++ 4715-4718.
        let node = Node::TypeParameterDeclaration(
            TypeParameterDeclaration::new(
                NodeMetadata::new(self.dummy_range()),
                NodeList::from_iter(self.gc, params),
            ),
        );
        Some(self.set_location(start, end, node))
    }

    /// Parse a single type parameter `[const] [variance] name [: B|extends B]
    /// [= D]`. Port of `parseTypeParamFlow` (flow.cpp:4721-4814).
    fn parse_type_param_flow(&mut self) -> Option<&'gc Node<'gc>> {
        let start = self.cur_start();
        // C++ 4723-4728.
        let mut is_const = false;
        let mut variance: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::rw_const) {
            is_const = true;
            self.advance(GrammarContext::Type);
        }

        // `in` and `out` are both ambiguous: variance modifier (`<in T>`,
        // `<out T>`) vs name (`<in>`, `<out>`, `<in: T>`, `<in extends Foo>`).
        // Defer the decision: consume the keyword here, and below — once we
        // know the *actual* next token — either promote it to variance or
        // treat it as the name itself. (C++ 4730-4749.)
        let mut variance_keyword_range = self.dummy_range();
        let mut variance_keyword_kind: Option<NodeLabel> = None;

        if self.check2(TokenKind::plus, TokenKind::minus) {
            let kind: &[u8] = if self.check(TokenKind::plus) {
                b"plus"
            } else {
                b"minus"
            };
            let v_range = self.cur_range();
            let v_node = Node::Variance(Variance::new(
                NodeMetadata::new(self.dummy_range()),
                self.lexer.get_identifier(kind),
            ));
            variance =
                Some(self.set_location(v_range.start, v_range.end, v_node));
            self.advance(GrammarContext::Type);
        } else if self.check(TokenKind::rw_in) || self.check_name(b"out") {
            variance_keyword_kind =
                Some(self.lexer.token().get_res_word_or_identifier());
            variance_keyword_range = self.cur_range();
            self.advance(GrammarContext::Type);
        }

        // Type-param name: identifier or `in` (rw_in). `in` is accepted
        // because Flow reclassifies it to an identifier in TYPE lex mode
        // (matching `<in>`, `<in: T>`, `<in extends T>`, `<X, in, Y>`). `out`
        // is already a plain identifier in Hermes, so `<out>` etc. work
        // without special handling. (C++ 4751-4776.)
        let name: NodeLabel;
        if self.check(TokenKind::identifier) || self.check(TokenKind::rw_in) {
            if let Some(kind) = variance_keyword_kind {
                // The deferred `in` was variance, and the current token is
                // the name.
                let v_node = Node::Variance(Variance::new(
                    NodeMetadata::new(self.dummy_range()),
                    kind,
                ));
                variance = Some(self.set_location(
                    variance_keyword_range.start,
                    variance_keyword_range.end,
                    v_node,
                ));
            }
            name = self.lexer.token().get_res_word_or_identifier();
            self.advance(GrammarContext::Type);
        } else if let Some(kind) = variance_keyword_kind {
            // The deferred `in`/`out` was the type-param name itself, not
            // variance. Reached when the next token is `>`, `,`, `:`, `=`,
            // or `rw_extends` (none of which are name tokens). E.g. `<in>`,
            // `<out: T>`, `<in extends T>`, `<out = T>`, `<X, in, Y>`.
            name = kind;
        } else {
            // errorExpected(identifier, "in type parameter", ...).
            self.need(TokenKind::identifier, " in type parameter");
            return None;
        }

        // C++ 4778-4799.
        let mut bound: Option<&'gc Node<'gc>> = None;
        let mut uses_extends_bound = false;
        if self.check(TokenKind::colon) {
            let bound_start = self.advance(GrammarContext::Type).start;
            let bound_type = self
                .parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;
            let bound_node = Node::TypeAnnotation(TypeAnnotation::new(
                NodeMetadata::new(self.dummy_range()),
                bound_type,
            ));
            bound = Some(self.set_location(
                bound_start,
                self.lexer.prev_token_end(),
                bound_node,
            ));
        } else if self.check(TokenKind::rw_extends) {
            uses_extends_bound = true;
            let bound_start = self.advance(GrammarContext::Type).start;
            let bound_type = self
                .parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;
            let bound_node = Node::TypeAnnotation(TypeAnnotation::new(
                NodeMetadata::new(self.dummy_range()),
                bound_type,
            ));
            bound = Some(self.set_location(
                bound_start,
                self.lexer.prev_token_end(),
                bound_node,
            ));
        }

        // C++ 4801-4807.
        let mut initializer: Option<&'gc Node<'gc>> = None;
        if self.check_and_eat(TokenKind::equal, GrammarContext::Type) {
            initializer = Some(self.parse_type_annotation_flow(
                None,
                AllowAnonFunctionType::Yes,
            )?);
        }

        // C++ 4809-4813.
        let node = Node::TypeParameter(TypeParameter::new(
            NodeMetadata::new(self.dummy_range()),
            name,
            is_const,
            bound,
            variance,
            initializer,
            uses_extends_bound,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseTypeArgsFlow — 4816 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse type arguments `<T, U>`, with the current token at `<`.
    /// \param trailing_grammar_context the grammar context with which the
    ///   closing `>` is consumed (the C++ parameter defaults to Type, per
    ///   JSParserImpl.h:1506-1508; Rust callers pass it explicitly).
    /// Port of `parseTypeArgsFlow` (flow.cpp:4816-4846).
    pub(super) fn parse_type_args_flow(
        &mut self,
        trailing_grammar_context: GrammarContext,
    ) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check(TokenKind::less));
        // C++ 4819.
        let start = self.advance(GrammarContext::Type).start;

        let mut params: Vec<&'gc Node<'gc>> = Vec::new();

        // C++ 4823-4831: a while-loop (not do-while) — empty `<>` IS allowed
        // for type *arguments* (unlike type-parameter declarations).
        while !self.check(TokenKind::greater) {
            params.push(self.parse_type_annotation_flow(
                None,
                AllowAnonFunctionType::Yes,
            )?);

            if !self.check_and_eat(TokenKind::comma, GrammarContext::Type) {
                break;
            }
        }

        // C++ 4833-4840: `end` is the `>` token's end, captured before
        // consuming it with the caller's trailing grammar context.
        let end = self.cur_range().end;
        if !self.eat(
            TokenKind::greater,
            trailing_grammar_context,
            " at end of type parameters",
        ) {
            return None;
        }

        // C++ 4842-4845.
        let node = Node::TypeParameterInstantiation(
            TypeParameterInstantiation::new(
                NodeMetadata::new(self.dummy_range()),
                NodeList::from_iter(self.gc, params),
            ),
        );
        Some(self.set_location(start, end, node))
    }

    // -----------------------------------------------------------------------
    // parseMethodishTypeAnnotationFlow — 4848 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a method-ish type annotation `(params): R` starting at the
    /// current `(` (used by object-type methods, accessors, call properties,
    /// and internal slots — the return type follows a `:`, not `=>`). Returns
    /// a `FunctionTypeAnnotation` node. Port of
    /// `parseMethodishTypeAnnotationFlow` (flow.cpp:4848-4879).
    fn parse_methodish_type_annotation_flow(
        &mut self,
        start: SMLoc,
        type_params: Option<&'gc Node<'gc>>,
    ) -> Option<&'gc Node<'gc>> {
        let mut params: Vec<&'gc Node<'gc>> = Vec::new();
        let mut this_constraint: Option<&'gc Node<'gc>> = None;

        // C++ 4855-4860.
        if !self.need(TokenKind::l_paren, " at start of parameters") {
            return None;
        }
        let rest = self.parse_function_type_annotation_params_flow(
            &mut params,
            &mut this_constraint,
            false, // hook
        )?;

        // C++ 4862-4868.
        if !self.eat(
            TokenKind::colon,
            GrammarContext::Type,
            " in function type annotation",
        ) {
            return None;
        }

        // C++ 4870: `parseReturnTypeAnnotationFlow()` declaration defaults.
        let return_type = self
            .parse_return_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

        // C++ 4874-4878.
        let node = Node::FunctionTypeAnnotation(FunctionTypeAnnotation::new(
            NodeMetadata::new(self.dummy_range()),
            NodeList::from_iter(self.gc, params),
            this_constraint,
            return_type,
            rest,
            type_params,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseFunctionTypeAnnotationParamsFlow — 4881 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse the parenthesized parameter list of a function type, with the
    /// current token at `(`. Parameters are appended to `params`, an optional
    /// leading `this: T` constraint is stored in `this_constraint`, and the
    /// optional rest parameter is returned (the C++ returns
    /// `Optional<FunctionTypeParamNode*>` — outer `None` here means an error
    /// was reported, inner `None` means no rest parameter). Port of
    /// `parseFunctionTypeAnnotationParamsFlow` (flow.cpp:4881-4944).
    fn parse_function_type_annotation_params_flow(
        &mut self,
        params: &mut Vec<&'gc Node<'gc>>,
        this_constraint: &mut Option<&'gc Node<'gc>>,
        hook: bool,
    ) -> Option<Option<&'gc Node<'gc>>> {
        debug_assert!(self.check(TokenKind::l_paren));
        // C++ 4887.
        self.advance(GrammarContext::Type);

        let mut rest: Option<&'gc Node<'gc>> = None;
        *this_constraint = None;

        // C++ 4892-4911: a leading `this: T` constraint.
        if self.check(TokenKind::rw_this) && !hook {
            let opt_next = self.lexer.lookahead1::<true>(None);
            if opt_next == Some(TokenKind::colon) {
                let this_start = self.advance(GrammarContext::Type).start;
                self.advance(GrammarContext::Type);
                let type_annotation = self.parse_type_annotation_flow(
                    None,
                    AllowAnonFunctionType::Yes,
                )?;

                let ftp_node = Node::FunctionTypeParam(FunctionTypeParam::new(
                    NodeMetadata::new(self.dummy_range()),
                    None, // name
                    type_annotation,
                    false, // optional
                ));
                *this_constraint = Some(self.set_location(
                    this_start,
                    self.lexer.prev_token_end(),
                    ftp_node,
                ));
                self.check_and_eat(TokenKind::comma, GrammarContext::Type);
            } else if opt_next == Some(TokenKind::question) {
                self.error_cur("'this' constraint may not be optional");
            }
        }

        // C++ 4913-4933.
        while !self.check(TokenKind::r_paren) {
            let is_rest =
                self.check_and_eat(TokenKind::dotdotdot, GrammarContext::Type);

            let param = if hook {
                // P6: parseHookTypeAnnotationParamFlow (C++ 4917) — hook
                // syntax is gated on getParseFlowComponentSyntax(); no P5
                // caller passes hook=true.
                self.error_cur(
                    "hook type annotations are unsupported (parser phase P6)",
                );
                return None;
            } else {
                self.parse_function_type_annotation_param_flow()?
            };

            if is_rest {
                // Rest param must be the last param.
                rest = Some(param);
                self.check_and_eat(TokenKind::comma, GrammarContext::Type);
                break;
            } else {
                params.push(param);
                if !self.check_and_eat(TokenKind::comma, GrammarContext::Type)
                {
                    break;
                }
            }
        }

        // C++ 4935-4941.
        if !self.eat(
            TokenKind::r_paren,
            GrammarContext::Type,
            " at end of function annotation parameters",
        ) {
            return None;
        }

        Some(rest)
    }

    /// Parse one function-type parameter, which is either a bare type or a
    /// named `name[?]: T`. Port of `parseFunctionTypeAnnotationParamFlow`
    /// (flow.cpp:4957-5005).
    fn parse_function_type_annotation_param_flow(
        &mut self,
    ) -> Option<&'gc Node<'gc>> {
        let start = self.cur_start();

        // C++ 4961-4968.
        if self.check(TokenKind::rw_this) {
            let opt_next = self.lexer.lookahead1::<true>(None);
            if opt_next == Some(TokenKind::colon) {
                self.error_cur("'this' constraint must be the first parameter");
            }
        }

        // C++ 4970-4972.
        let left = self.parse_type_annotation_before_colon_flow()?;

        let mut name: Option<&'gc Node<'gc>> = None;
        let type_annotation: &'gc Node<'gc>;
        let mut optional = false;

        // C++ 4978-4998.
        if self.check2(TokenKind::colon, TokenKind::question) {
            // The node is actually supposed to be an identifier, not a
            // TypeAnnotation.
            name = Some(self.reparse_type_annotation_as_identifier_flow(left)?);
            optional =
                self.check_and_eat(TokenKind::question, GrammarContext::Type);
            if !self.eat(
                TokenKind::colon,
                GrammarContext::Type,
                " in function parameter type annotation",
            ) {
                return None;
            }
            type_annotation = self
                .parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;
        } else {
            type_annotation = left;
        }

        // C++ 5000-5004.
        let node = Node::FunctionTypeParam(FunctionTypeParam::new(
            NodeMetadata::new(self.dummy_range()),
            name,
            type_annotation,
            optional,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parseGenericTypeFlow — 5007 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a (possibly qualified) generic type reference
    /// `Foo.Bar<Args>`, with the current token at the first identifier or
    /// reserved word. Port of `parseGenericTypeFlow` (flow.cpp:5007-5050).
    fn parse_generic_type_flow(&mut self) -> Option<&'gc Node<'gc>> {
        debug_assert!(
            self.check(TokenKind::identifier)
                || self.lexer.token().is_res_word()
        );
        let start = self.cur_start();

        // C++ 5012-5017.
        let id_range = self.cur_range();
        let id_node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            self.lexer.token().get_res_word_or_identifier(),
            None,
            false,
        ));
        let mut id = self.set_location(id_range.start, id_range.end, id_node);
        self.advance(GrammarContext::Type);

        // C++ 5019: GrammarContext::Type here (unlike the qualified-typeof
        // chain, which uses the default).
        while self.check_and_eat(TokenKind::period, GrammarContext::Type) {
            // C++ 5020-5027.
            if !self.check(TokenKind::identifier)
                && !self.lexer.token().is_res_word()
            {
                // errorExpected(identifier, "in qualified generic type name",
                // ...).
                self.need(
                    TokenKind::identifier,
                    " in qualified generic type name",
                );
                return None;
            }
            // C++ 5028-5033.
            let next_range = self.cur_range();
            let next_node = Node::Identifier(Identifier::new(
                NodeMetadata::new(self.dummy_range()),
                self.lexer.token().get_res_word_or_identifier(),
                None,
                false,
            ));
            let next = self.set_location(
                next_range.start,
                next_range.end,
                next_node,
            );
            self.advance(GrammarContext::Type);
            // C++ 5034-5035.
            let q_node = Node::QualifiedTypeIdentifier(
                QualifiedTypeIdentifier::new(
                    NodeMetadata::new(self.dummy_range()),
                    id,
                    next,
                ),
            );
            id = self.set_location(
                id.metadata().range.get().start,
                next_range.end,
                q_node,
            );
        }

        // C++ 5037-5044: `parseTypeArgsFlow()` is called with its default
        // trailing grammar context (Type, per JSParserImpl.h:1506).
        let mut type_parameters: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_parameters =
                Some(self.parse_type_args_flow(GrammarContext::Type)?);
        }

        // C++ 5046-5049.
        let node = Node::GenericTypeAnnotation(GenericTypeAnnotation::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            type_parameters,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }

    // -----------------------------------------------------------------------
    // parsePredicateFlow — 5078 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse a `%checks` / `%checks(expr)` predicate, with the current token
    /// at the `%checks` identifier (lexed as a single identifier in Type
    /// grammar context). Port of `parsePredicateFlow` (flow.cpp:5078-5098).
    // Wired into function declarations (`function f(): T %checks {}`) in
    // P5.4; until then only unit tests reach it.
    #[allow(dead_code)]
    pub(super) fn parse_predicate_flow(&mut self) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check_name(b"%checks"));
        // C++ 5080.
        let checks_rng = self.advance(GrammarContext::Type);
        // C++ 5081: `checkAndEat(l_paren)` with GrammarContext::AllowRegExp —
        // deliberate; what follows is a JS expression, not a type.
        if self.check_and_eat(TokenKind::l_paren, GrammarContext::AllowRegExp) {
            // C++ 5082: `parseConditionalExpression()` with its declaration
            // defaults (ParamIn, CoverTypedParameters::Yes; the Rust port's
            // cover-typed handling inside is a P6 omission).
            let cond = self.parse_conditional_expression(PARAM_IN)?;
            // C++ 5085-5092.
            let end = self.cur_range().end;
            if !self.eat(
                TokenKind::r_paren,
                GrammarContext::Type,
                " in declared predicate",
            ) {
                return None;
            }
            // C++ 5093-5094.
            let node = Node::DeclaredPredicate(DeclaredPredicate::new(
                NodeMetadata::new(self.dummy_range()),
                cond,
            ));
            return Some(self.set_location(checks_rng.start, end, node));
        }
        // C++ 5096-5097: the InferredPredicate spans the `%checks` token.
        let node = Node::InferredPredicate(InferredPredicate::new(
            NodeMetadata::new(self.dummy_range()),
        ));
        Some(self.set_location(checks_rng.start, checks_rng.end, node))
    }

    // -----------------------------------------------------------------------
    // reparseTypeAnnotationAsIdFlow — 5100 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Map a type-annotation node back to the identifier atom it would have
    /// parsed as, reporting "identifier expected" at the node if impossible.
    /// Port of `reparseTypeAnnotationAsIdFlow` (flow.cpp:5100-5133).
    ///
    /// NOTE: BooleanTypeAnnotation maps to "boolean" even when the source
    /// spelled it `bool` — the C++ maps both spellings to booleanIdent_ the
    /// same way (5105-5106).
    fn reparse_type_annotation_as_id_flow(
        &mut self,
        type_annotation: &'gc Node<'gc>,
    ) -> Option<NodeLabel> {
        let id: Option<NodeLabel> = match type_annotation {
            Node::AnyTypeAnnotation(_) => {
                Some(self.lexer.get_identifier(b"any"))
            }
            Node::EmptyTypeAnnotation(_) => {
                Some(self.lexer.get_identifier(b"empty"))
            }
            Node::BooleanTypeAnnotation(_) => {
                Some(self.lexer.get_identifier(b"boolean"))
            }
            Node::NumberTypeAnnotation(_) => {
                Some(self.lexer.get_identifier(b"number"))
            }
            Node::StringTypeAnnotation(_) => {
                Some(self.lexer.get_identifier(b"string"))
            }
            Node::SymbolTypeAnnotation(_) => {
                Some(self.lexer.get_identifier(b"symbol"))
            }
            Node::NullLiteralTypeAnnotation(_) => {
                Some(self.lexer.get_identifier(b"null"))
            }
            // C++ 5117-5125: a generic without type arguments whose id is a
            // plain Identifier reparses as that identifier.
            Node::GenericTypeAnnotation(generic)
                if generic.type_parameters.is_none() =>
            {
                if let Node::Identifier(generic_id) = generic.id {
                    Some(generic_id.name.get())
                } else {
                    None
                }
            }
            _ => None,
        };

        if id.is_none() {
            // C++ 5127-5131.
            let range = type_annotation.metadata().range.get();
            self.error_at(range, "identifier expected");
        }
        id
    }

    /// Reparse a type-annotation node as an `Identifier` node spanning the
    /// original node's source range. Port of
    /// `reparseTypeAnnotationAsIdentifierFlow` (flow.cpp:5135-5146).
    fn reparse_type_annotation_as_identifier_flow(
        &mut self,
        type_annotation: &'gc Node<'gc>,
    ) -> Option<&'gc Node<'gc>> {
        let id = self.reparse_type_annotation_as_id_flow(type_annotation)?;

        // C++ 5141-5145.
        let range = type_annotation.metadata().range.get();
        let node = Node::Identifier(Identifier::new(
            NodeMetadata::new(self.dummy_range()),
            id,
            None,
            false,
        ));
        Some(self.set_location(range.start, range.end, node))
    }

    /// Intern the raw source text of the current token. The Rust equivalent
    /// of the C++ `lexer_.getStringLiteral(tok_->inputStr())` idiom used by
    /// the literal type annotations.
    fn cur_token_source_atom(&self) -> NodeLabel {
        let range = self.lexer.token().source_range();
        self.source_bytes_atom(range.start, range.end)
    }
}
