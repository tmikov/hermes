/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Flow object-type annotations: the `{ ... }` / `{| ... |}` bodies with
//! their property/method/accessor/indexer/mapped/call/internal-slot/spread
//! members. Port of the corresponding sections of
//! `lib/Parser/JSParserImpl-flow.cpp`.

use hermes_ast::node::{
    Identifier, Node, ObjectTypeAnnotation, ObjectTypeCallProperty,
    ObjectTypeIndexer, ObjectTypeInternalSlot, ObjectTypeMappedTypeProperty,
    ObjectTypeProperty, ObjectTypeSpreadProperty, TypeParameter, Variance,
};
use hermes_ast::node_child::{NodeList, NodeMetadata, NodeString};
use hermes_atom_table::INVALID_ATOM_BYTES;
use hermes_support::location::SMLoc;

use crate::js::JSParserImpl;
use crate::lexer::GrammarContext;
use crate::token_kinds::TokenKind;

use super::{
    can_follow_variance_keyword_flow, AllowAnonFunctionType,
    AllowProtoProperty, AllowSpreadProperty, AllowStaticProperty,
};

impl<'gc, 'ast, 'ctx, 'a> JSParserImpl<'gc, 'ast, 'ctx, 'a> {
    // -----------------------------------------------------------------------
    // parseObjectTypeAnnotationFlow — 4049 in JSParserImpl-flow.cpp
    // -----------------------------------------------------------------------

    /// Parse an object type annotation, with the current token at `{` or
    /// `{|`. Port of `parseObjectTypeAnnotationFlow` (flow.cpp:4047-4098).
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

        // C++ 4060-4069.
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

        // C++ 4071-4076.
        if exact && inexact {
            // Doesn't prevent parsing from continuing, but it is an error.
            self.error_at_loc(
                start,
                "Explicit inexact syntax cannot appear inside an explicit exact object type",
            );
        }

        // C++ 4078-4085.
        let end = self.cur_range().end;
        if !self.eat_at(
            if exact {
                TokenKind::piper_brace
            } else {
                TokenKind::r_brace
            },
            GrammarContext::Type,
            " at end of exact object type annotation",
            Some("start of object"),
            start,
        ) {
            return None;
        }

        // C++ 4087-4096.
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
    /// (flow.cpp:4100-4164).
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
                    // C++ 4113-4117.
                    *inexact = true;
                    self.advance(GrammarContext::Type);
                    // Explicit '...' must be the last element in the type
                    // annotation.
                    return true;
                } else if self.check2(
                    TokenKind::r_brace,
                    TokenKind::piper_brace,
                ) {
                    // C++ 4118-4120.
                    *inexact = true;
                    return true;
                } else {
                    // C++ 4121-4133.
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
                // C++ 4134-4143.
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

            // C++ 4145-4159.
            if self.check2(TokenKind::comma, TokenKind::semi) {
                self.advance(GrammarContext::Type);
            } else if self.check2(TokenKind::r_brace, TokenKind::piper_brace) {
                return true;
            } else {
                // C++ 4145-4159: whatLoc is `start` (this property's start).
                self.error_expected4(
                    TokenKind::comma,
                    TokenKind::semi,
                    TokenKind::r_brace,
                    TokenKind::piper_brace,
                    " after property",
                    Some("start of property"),
                    start,
                );
                return false;
            }
        }

        true
    }

    /// Parse one object-type member (property, method, accessor, call
    /// property, indexer, mapped type, or internal slot), pushing it into the
    /// appropriate out-list. Returns false if an error was reported. Port of
    /// `parsePropertyTypeAnnotationFlow` (flow.cpp:4166-4452).
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

        // C++ 4179-4182.
        if self.check_name(b"proto") {
            proto = true;
            self.advance(GrammarContext::Type);
        }

        // C++ 4184-4187.
        if !proto
            && (self.check(TokenKind::rw_static) || self.check_name(b"static"))
        {
            is_static = true;
            self.advance(GrammarContext::Type);
        }

        // C++ 4189-4202.
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

        // C++ 4204-4327.
        if self.check_and_eat(TokenKind::l_square, GrammarContext::Type) {
            if self.check_and_eat(TokenKind::l_square, GrammarContext::Type) {
                // Internal slot `[[id]]` (C++ 4205-4286).
                if let Some(variance) = variance {
                    let range = variance.metadata().range();
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
                // C++ 4216-4223.
                if !self.check(TokenKind::identifier)
                    && !self.lexer.token().is_res_word()
                {
                    // C++ 4216-4223: errorExpected(identifier, "in internal
                    // slot", "start of internal slot", start). `start` is
                    // real, so this routes through `need_at`.
                    self.need_at(
                        TokenKind::identifier,
                        " in internal slot",
                        Some("start of internal slot"),
                        start,
                    );
                    return false;
                }
                // C++ 4224-4229.
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

                // C++ 4231-4244.
                if !self.eat_at(
                    TokenKind::r_square,
                    GrammarContext::Type,
                    " at end of internal slot",
                    Some("start of internal slot"),
                    start,
                ) {
                    return false;
                }
                if !self.eat_at(
                    TokenKind::r_square,
                    GrammarContext::Type,
                    " at end of internal slot",
                    Some("start of internal slot"),
                    start,
                ) {
                    return false;
                }

                let mut optional = false;
                let method;
                let value;

                if self.check2(TokenKind::less, TokenKind::l_paren) {
                    // Type params and method (C++ 4250-4263).
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
                    // Standard type annotation (C++ 4264-4279).
                    method = false;
                    optional = self.check_and_eat(
                        TokenKind::question,
                        GrammarContext::Type,
                    );
                    if !self.eat_at(
                        TokenKind::colon,
                        GrammarContext::Type,
                        " in type annotation",
                        Some("start of annotation"),
                        start,
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

                // C++ 4281-4286.
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
                // Indexer or Mapped Type (C++ 4287-4325).
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

                // C++ 4319-4324.
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

        // C++ 4331-4363.
        if self.check2(TokenKind::less, TokenKind::l_paren) {
            // C++ 4332-4349: a consumed `static`/`proto` that is not allowed
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
                    let range = variance.metadata().range();
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
            // C++ 4350-4362.
            if let Some(variance) = variance {
                let range = variance.metadata().range();
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

        // C++ 4365-4381: a consumed `static`/`proto` directly followed by
        // `:`/`?` was actually the property name.
        if (is_static || proto)
            && self.check2(TokenKind::colon, TokenKind::question)
        {
            if let Some(variance) = variance {
                let range = variance.metadata().range();
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

        // C++ 4383-4386.
        let Some(key) = self.parse_property_name() else {
            return false;
        };

        // C++ 4388-4403.
        if self.check2(TokenKind::less, TokenKind::l_paren) {
            if let Some(variance) = variance {
                let range = variance.metadata().range();
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

        // C++ 4405-4417.
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

        // C++ 4419-4443: a `get`/`set` accessor — the parsed key was the
        // accessor specifier and the real key follows.
        if let Node::Identifier(ident) = key {
            let (is_getter, is_setter) = {
                let bytes =
                    self.lexer.get_string_table().bytes(ident.name.get());
                (bytes == b"get", bytes == b"set")
            };
            if is_getter || is_setter {
                if let Some(variance) = variance {
                    let range = variance.metadata().range();
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

        // C++ 4445-4450: whatLoc is `start`.
        self.error_expected2(
            TokenKind::colon,
            TokenKind::question,
            " in property type annotation",
            Some("start of properties"),
            start,
        );
        false
    }

    /// Parse the `[?] : T` tail of a plain object-type property. Port of
    /// `parseTypePropertyFlow` (flow.cpp:4454-4485).
    fn parse_type_property_flow(
        &mut self,
        start: SMLoc,
        variance: Option<&'gc Node<'gc>>,
        is_static: bool,
        proto: bool,
        key: &'gc Node<'gc>,
    ) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check2(TokenKind::colon, TokenKind::question));

        // C++ 4461-4462.
        let optional =
            self.check_and_eat(TokenKind::question, GrammarContext::Type);
        // C++ 4463-4469.
        if !self.eat_at(
            TokenKind::colon,
            GrammarContext::Type,
            " in type property",
            Some("start of property"),
            start,
        ) {
            return None;
        }

        // C++ 4471-4474.
        let value = self
            .parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

        // C++ 4476-4483.
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
    /// Port of `parseMethodTypePropertyFlow` (flow.cpp:4487-4523).
    fn parse_method_type_property_flow(
        &mut self,
        start: SMLoc,
        is_static: bool,
        key: &'gc Node<'gc>,
    ) -> Option<&'gc Node<'gc>> {
        debug_assert!(self.check2(TokenKind::less, TokenKind::l_paren));

        // C++ 4492-4498.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_params = Some(self.parse_type_params_flow()?);
        }

        // C++ 4500-4503.
        let value =
            self.parse_methodish_type_annotation_flow(start, type_params)?;

        // C++ 4505-4521.
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
    /// (flow.cpp:4525-4563).
    fn parse_get_or_set_type_property_flow(
        &mut self,
        start: SMLoc,
        is_static: bool,
        is_getter: bool,
        key: &'gc Node<'gc>,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 4529-4531.
        let value = self.parse_methodish_type_annotation_flow(start, None)?;
        let fta = value
            .as_function_type_annotation()
            .expect("methodish parser returns FunctionTypeAnnotation");

        // Check the number of parameters, but we can continue parsing anyway
        // (C++ 4540-4549).
        if is_getter {
            if !fta.params.is_empty() {
                let range = value.metadata().range();
                self.error_at(range, "Getter must have 0 parameters");
            }
        } else if fta.params.iter().count() != 1 {
            let range = value.metadata().range();
            self.error_at(range, "Setter must have 1 parameter");
        }

        // C++ 4551-4555.
        if let Some(this_constraint) = fta.this {
            let range = this_constraint.metadata().range();
            self.error_at(range, "Accessors must not have 'this' annotations");
        }

        // C++ 4557-4561.
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
    /// `parseTypeMappedTypePropertyFlow` (flow.cpp:4565-4633).
    fn parse_type_mapped_type_property_flow(
        &mut self,
        start: SMLoc,
        left: &'gc Node<'gc>,
        variance: Option<&'gc Node<'gc>>,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 4568-4576: the key reparses as a bare type parameter spanning
        // exactly `left`'s range.
        let id = self.reparse_type_annotation_as_id_flow(left)?;
        let left_range = left.metadata().range();
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

        // C++ 4578-4580.
        let source_type = self
            .parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

        // C++ 4582-4588.
        if !self.eat_at(
            TokenKind::r_square,
            GrammarContext::Type,
            " in mapped type",
            Some("start of mapped type"),
            start,
        ) {
            return None;
        }

        // C++ 4590-4613: the optionality sigil. The C++ passes a null
        // UniqueString when there is no sigil; the dumper emits
        // `"optional": null` — INVALID_ATOM_BYTES is the Rust null
        // NodeString.
        let mut optional: NodeString = INVALID_ATOM_BYTES;
        if self.check_and_eat(TokenKind::plus, GrammarContext::Type) {
            if !self.eat_at(
                TokenKind::question,
                GrammarContext::Type,
                " in mapped type",
                Some("start of mapped type"),
                start,
            ) {
                return None;
            }
            optional = self.lexer.get_identifier(b"PlusOptional");
        } else if self.check_and_eat(TokenKind::minus, GrammarContext::Type) {
            if !self.eat_at(
                TokenKind::question,
                GrammarContext::Type,
                " in mapped type",
                Some("start of mapped type"),
                start,
            ) {
                return None;
            }
            optional = self.lexer.get_identifier(b"MinusOptional");
        } else if self.check_and_eat(TokenKind::question, GrammarContext::Type)
        {
            optional = self.lexer.get_identifier(b"Optional");
        }

        // C++ 4615-4621.
        if !self.eat_at(
            TokenKind::colon,
            GrammarContext::Type,
            " in mapped type",
            Some("start of mapped type"),
            start,
        ) {
            return None;
        }

        // C++ 4623-4625.
        let prop_type = self
            .parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

        // C++ 4627-4631.
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
    /// `parseTypeIndexerPropertyFlow` (flow.cpp:4635-4682).
    fn parse_type_indexer_property_flow(
        &mut self,
        start: SMLoc,
        left: &'gc Node<'gc>,
        variance: Option<&'gc Node<'gc>>,
        is_static: bool,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 4639-4653.
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

        // C++ 4655-4661.
        if !self.eat_at(
            TokenKind::r_square,
            GrammarContext::Type,
            " in indexer",
            Some("start of indexer"),
            start,
        ) {
            return None;
        }

        // C++ 4663-4669.
        if !self.eat_at(
            TokenKind::colon,
            GrammarContext::Type,
            " in indexer",
            Some("start of indexer"),
            start,
        ) {
            return None;
        }

        // C++ 4671-4674.
        let value = self
            .parse_type_annotation_flow(None, AllowAnonFunctionType::Yes)?;

        // C++ 4676-4680.
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
    /// `parseTypeCallPropertyFlow` (flow.cpp:4684-4701).
    fn parse_type_call_property_flow(
        &mut self,
        start: SMLoc,
        is_static: bool,
    ) -> Option<&'gc Node<'gc>> {
        // C++ 4686-4692.
        let mut type_params: Option<&'gc Node<'gc>> = None;
        if self.check(TokenKind::less) {
            type_params = Some(self.parse_type_params_flow()?);
        }
        // C++ 4693-4695.
        let value =
            self.parse_methodish_type_annotation_flow(start, type_params)?;
        // C++ 4696-4699.
        let node = Node::ObjectTypeCallProperty(ObjectTypeCallProperty::new(
            NodeMetadata::new(self.dummy_range()),
            value,
            is_static,
        ));
        Some(self.set_location(start, self.lexer.prev_token_end(), node))
    }
}
