/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The exhaustive dispatch over every [`NodeKind`](hermes_ast::node::NodeKind)
//! variant: one call out to a per-kind method, rather than juno's single
//! ~2800-line `match` (`gen_js.rs:4000`-onward). This is a deliberate
//! structural divergence from juno — see the plan's File Structure section
//! for the rationale — but `gen_node` below still matches every kind, so
//! exhaustiveness is preserved.
//!
//! Filled in incrementally by Tasks 2-13. Task 13 deleted the temporary
//! catch-all Task 1 added, so this match is now exhaustive by name: the
//! compiler, not a runtime `UnsupportedKind`, is what proves all 271 kinds
//! are handled, and a kind added to `NodeKind` later is a build failure
//! here. The only kinds that still report [`GenJS::unsupported_kind`] are
//! the 7 internal ones (plus `TemplateElement`), each matched explicitly
//! below.

use hermes_ast::context::GCLock;
use hermes_ast::node::{
    Node, TSTypeParameterDeclaration, TSTypeParameterInstantiation, TypeParameterDeclaration,
    TypeParameterInstantiation,
};
use hermes_ast::visitor::Path;

use crate::{GenJS, GenJsError};

impl<'s, 'w> GenJS<'s, 'w> {
    /// Generates JS source for `node`, the dispatch entry point.
    ///
    /// `path` is the (parent, field) `node` occupies in its parent, or
    /// `None` at the root. Mirrors juno's `Visitor::visit` entry point
    /// (`gen_js.rs:4000`), but takes `ctx` and `path` as explicit
    /// parameters rather than through a trait impl: our
    /// [`hermes_ast::visitor::Visitor`] trait has a different shape
    /// (`visit_node(&mut self, node)`, no `ctx`/`path`) and `GenJS` never
    /// uses its default child-walk — see the plan's Adaptation Rules.
    ///
    /// `path` was unused at this dispatch level through Task 10
    /// (parenthesization is decided by the *caller* of `gen_node`, via
    /// `print_child`/`need_parens` in `precedence.rs`, not by `gen_node`
    /// itself — see that module's doc comment); Task 4 is the first to use
    /// `ctx`, for `gc.try_bytes_str` (arms/literal.rs's module doc comment)
    /// and for threading through recursive `gen_node` calls. Task 11 is the
    /// first to read `path` here: `arms/flow_decl.rs`'s four `Declare*` arms
    /// (`DeclareOpaqueType`/`DeclareClass`/`DeclareFunction`/
    /// `DeclareVariable`) each need to know whether *their own* parent is a
    /// `DeclareExportDeclaration` (which already printed `declare export `)
    /// to decide whether to print their own `declare ` keyword — see that
    /// module's `declare_prefix_needed`.
    ///
    /// `ctx`'s type is `&GCLock<'_, '_>` rather than `&GCLock<'gc, '_>`
    /// (Task 2 fix, signature-only — no arm below changed): `GCLock` is
    /// invariant in both its parameters, and `ParsedJS::with_program`'s
    /// closure bound instantiates `'gc` per call as a universally-quantified
    /// `for<'gc>` variable that can never be forced to equal `GCLock`'s
    /// fixed `'static` arena parameter. Tying them together here would make
    /// `gen_node` — and `generate`, which calls it — uncallable from
    /// `with_program`. See `generate`'s doc comment in `gen.rs` and
    /// `rust/crates/sema/examples/print_bindings.rs` for the same pattern.
    ///
    /// `node`'s type is `&'gc Node<'gc>` (Task 4 change from `&Node<'gc>`,
    /// an anonymously-lifetimed outer reference that Tasks 1-3 never needed
    /// to distinguish from `'gc'` since nothing yet built a `Path` from it).
    /// `arms::literal`'s methods that build `Path::new(node, ...)` need
    /// `node`'s own reference to be valid for `'gc` — the same lifetime a
    /// child's `&'gc Node<'gc>` field carries — for `Path<'gc>` to type
    /// check at all. Matching a `&'gc Node<'gc>` scrutinee against a
    /// tuple-variant pattern also happens to be what lets a bound field of
    /// declared type `&'gc Node<'gc>` (e.g. `TaggedTemplateExpression::tag`)
    /// pass directly to a `&'gc Node<'gc>`-expecting call: match ergonomics'
    /// single `ref` binding mode plus one step of deref coercion collapse
    /// back to exactly `&'gc Node<'gc>` only when the outer and field
    /// lifetimes are the same generic parameter, as they are here.
    pub fn gen_node<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        path: Option<Path<'gc>>,
    ) -> Result<(), GenJsError> {
        match node {
            // The 7 internal kinds (spec §4): cover-grammar productions and
            // compiler-internal nodes that never have JS source syntax, so
            // they always report `UnsupportedKind` rather than growing a
            // real arm in a later task.
            Node::CoverEmptyArgs(_) => self.unsupported_kind(node),
            Node::CoverInitializer(_) => self.unsupported_kind(node),
            Node::CoverRestElement(_) => self.unsupported_kind(node),
            Node::CoverTrailingComma(_) => self.unsupported_kind(node),
            Node::CoverTypedIdentifier(_) => self.unsupported_kind(node),
            Node::ImplicitCheckedCast(_) => self.unsupported_kind(node),
            Node::SHBuiltin(_) => self.unsupported_kind(node),

            // Task 4 (`arms::literal`): literals, identifiers, templates,
            // and string escaping. `NullLiteral`/`ThisExpression`/`Super`
            // have no fields besides `metadata`, so their whole inner struct
            // is matched with `_` (an established pattern already used by
            // `precedence.rs`'s `get_precedence`) rather than delegating a
            // value there is nothing to destructure.
            Node::BooleanLiteral(inner) => self.gen_boolean_literal(inner),
            Node::NullLiteral(_) => self.gen_null_literal(),
            Node::StringLiteral(inner) => self.gen_string_literal(ctx, inner),
            Node::NumericLiteral(inner) => self.gen_numeric_literal(inner),
            Node::BigIntLiteral(inner) => self.gen_bigint_literal(ctx, inner),
            Node::RegExpLiteral(inner) => self.gen_regexp_literal(ctx, inner),
            Node::ThisExpression(_) => self.gen_this_expression(),
            Node::Super(_) => self.gen_super(),
            Node::Identifier(inner) => self.gen_identifier(ctx, node, inner),
            Node::PrivateName(inner) => self.gen_private_name(ctx, node, inner),
            Node::MetaProperty(inner) => self.gen_meta_property(ctx, node, inner),
            Node::Directive(inner) => self.gen_directive(ctx, node, inner),
            Node::DirectiveLiteral(inner) => self.gen_directive_literal(ctx, inner),
            Node::TemplateLiteral(inner) => self.gen_template_literal(ctx, node, inner),
            Node::TaggedTemplateExpression(inner) => {
                self.gen_tagged_template_expression(ctx, node, inner)
            }
            // `TemplateElement` is only ever printed inline by
            // `gen_template_literal`, iterating `quasis` directly rather
            // than dispatching through `gen_node`; see `arms/literal.rs`'s
            // comment at the bottom of its `TaggedTemplateExpression` arm.
            // juno treats reaching it here as `unreachable!()`
            // (`gen_js.rs:1437-1439`); we report it the same way as the 7
            // internal kinds above instead, per spec §4's "never panic on a
            // malformed input tree" rule.
            Node::TemplateElement(_) => self.unsupported_kind(node),

            // Task 5 (`arms::expr`): ES expressions — `SequenceExpression`
            // through `Property`/`LogicalExpression`, plus `Property`'s
            // `visit_func_params_body` helper (see that module's doc
            // comment for why it lives there rather than in a later task's
            // `arms::func`).
            Node::SequenceExpression(inner) => self.gen_sequence_expression(ctx, node, inner),
            Node::ObjectExpression(inner) => self.gen_object_expression(ctx, node, inner),
            Node::ArrayExpression(inner) => self.gen_array_expression(ctx, node, inner),
            Node::SpreadElement(inner) => self.gen_spread_element(ctx, node, inner),
            Node::NewExpression(inner) => self.gen_new_expression(ctx, node, inner),
            Node::YieldExpression(inner) => self.gen_yield_expression(ctx, node, inner),
            Node::AwaitExpression(inner) => self.gen_await_expression(ctx, node, inner),
            Node::ImportExpression(inner) => self.gen_import_expression(ctx, node, inner),
            Node::CallExpression(inner) => self.gen_call_expression(ctx, node, inner),
            Node::OptionalCallExpression(inner) => {
                self.gen_optional_call_expression(ctx, node, inner)
            }
            Node::AssignmentExpression(inner) => self.gen_assignment_expression(ctx, node, inner),
            Node::UnaryExpression(inner) => self.gen_unary_expression(ctx, node, inner),
            Node::UpdateExpression(inner) => self.gen_update_expression(ctx, node, inner),
            Node::MemberExpression(inner) => self.gen_member_expression(ctx, node, inner),
            Node::OptionalMemberExpression(inner) => {
                self.gen_optional_member_expression(ctx, node, inner)
            }
            Node::BinaryExpression(inner) => self.gen_binary_expression(ctx, node, inner),
            Node::LogicalExpression(inner) => self.gen_logical_expression(ctx, node, inner),
            Node::ConditionalExpression(inner) => self.gen_conditional_expression(ctx, node, inner),
            Node::Property(inner) => self.gen_property(ctx, node, inner),

            // Task 6 (`arms::stmt`): ES statements and patterns —
            // `Program`/`Empty`/`Metadata`, `WhileStatement` through
            // `IfStatement`, `CatchClause`/`VariableDeclaration`/
            // `VariableDeclarator`, and `ObjectPattern` through
            // `AssignmentPattern`. `Node::Module` (juno `gen_js.rs:370-373`)
            // has no arm — we have no such kind.
            Node::Empty(_) => self.gen_empty(),
            Node::Metadata(_) => self.gen_metadata(),
            Node::Program(inner) => self.gen_program(ctx, node, inner),
            Node::WhileStatement(inner) => self.gen_while_statement(ctx, node, inner),
            Node::DoWhileStatement(inner) => self.gen_do_while_statement(ctx, node, inner),
            Node::ForInStatement(inner) => self.gen_for_in_statement(ctx, node, inner),
            Node::ForOfStatement(inner) => self.gen_for_of_statement(ctx, node, inner),
            Node::ForStatement(inner) => self.gen_for_statement(ctx, node, inner),
            Node::DebuggerStatement(_) => self.gen_debugger_statement(),
            Node::EmptyStatement(_) => self.gen_empty_statement(),
            Node::BlockStatement(inner) => self.gen_block_statement(ctx, node, inner),
            Node::BreakStatement(inner) => self.gen_break_statement(ctx, node, inner),
            Node::ContinueStatement(inner) => self.gen_continue_statement(ctx, node, inner),
            Node::ThrowStatement(inner) => self.gen_throw_statement(ctx, node, inner),
            Node::ReturnStatement(inner) => self.gen_return_statement(ctx, node, inner),
            Node::WithStatement(inner) => self.gen_with_statement(ctx, node, inner),
            Node::SwitchStatement(inner) => self.gen_switch_statement(ctx, node, inner),
            Node::SwitchCase(inner) => self.gen_switch_case(ctx, node, inner),
            Node::LabeledStatement(inner) => self.gen_labeled_statement(ctx, node, inner),
            Node::ExpressionStatement(inner) => self.gen_expression_statement(ctx, node, inner),
            Node::TryStatement(inner) => self.gen_try_statement(ctx, node, inner),
            Node::IfStatement(inner) => self.gen_if_statement(ctx, node, inner),
            Node::CatchClause(inner) => self.gen_catch_clause(ctx, node, inner),
            Node::VariableDeclaration(inner) => self.gen_variable_declaration(ctx, node, inner),
            Node::VariableDeclarator(inner) => self.gen_variable_declarator(ctx, node, inner),
            Node::ObjectPattern(inner) => self.gen_object_pattern(ctx, node, inner),
            Node::ArrayPattern(inner) => self.gen_array_pattern(ctx, node, inner),
            Node::RestElement(inner) => self.gen_rest_element(ctx, node, inner),
            Node::AssignmentPattern(inner) => self.gen_assignment_pattern(ctx, node, inner),

            // Task 7 (`arms::func`): functions, classes, methods, and
            // properties.
            Node::FunctionExpression(inner) => self.gen_function_expression(ctx, node, inner),
            Node::FunctionDeclaration(inner) => self.gen_function_declaration(ctx, node, inner),
            Node::ArrowFunctionExpression(inner) => {
                self.gen_arrow_function_expression(ctx, node, inner)
            }
            Node::ClassExpression(inner) => self.gen_class_expression(ctx, node, inner),
            Node::ClassDeclaration(inner) => self.gen_class_declaration(ctx, node, inner),
            Node::ClassBody(inner) => self.gen_class_body(ctx, node, inner),
            Node::ClassProperty(inner) => self.gen_class_property(ctx, node, inner),
            Node::ClassPrivateProperty(inner) => self.gen_class_private_property(ctx, node, inner),
            Node::MethodDefinition(inner) => self.gen_method_definition(ctx, node, inner),

            // Task 8 (`arms::module`): `import`/`export` declarations.
            Node::ImportDeclaration(inner) => self.gen_import_declaration(ctx, node, inner),
            Node::ImportSpecifier(inner) => self.gen_import_specifier(ctx, node, inner),
            Node::ImportDefaultSpecifier(inner) => {
                self.gen_import_default_specifier(ctx, node, inner)
            }
            Node::ImportNamespaceSpecifier(inner) => {
                self.gen_import_namespace_specifier(ctx, node, inner)
            }
            Node::ImportAttribute(inner) => self.gen_import_attribute(ctx, node, inner),
            Node::ExportNamedDeclaration(inner) => {
                self.gen_export_named_declaration(ctx, node, inner)
            }
            Node::ExportSpecifier(inner) => self.gen_export_specifier(ctx, node, inner),
            Node::ExportNamespaceSpecifier(inner) => {
                self.gen_export_namespace_specifier(ctx, node, inner)
            }
            Node::ExportDefaultDeclaration(inner) => {
                self.gen_export_default_declaration(ctx, node, inner)
            }
            Node::ExportAllDeclaration(inner) => self.gen_export_all_declaration(ctx, node, inner),

            // Task 9 (`arms::jsx`): the 16 JSX kinds (the brief's "14" plus
            // `JSXOpeningFragment`/`JSXClosingFragment`, which juno's own
            // `gen_js.rs:2000-2159` also covers in this same range).
            // `precedence.rs`'s
            // `get_precedence` already covers `JSXElement`/`JSXFragment`
            // (an earlier task; see that module's `get_precedence`), so this
            // task adds no `precedence.rs` changes of its own.
            Node::JSXIdentifier(inner) => self.gen_jsx_identifier(ctx, inner),
            Node::JSXMemberExpression(inner) => self.gen_jsx_member_expression(ctx, node, inner),
            Node::JSXNamespacedName(inner) => self.gen_jsx_namespaced_name(ctx, node, inner),
            Node::JSXEmptyExpression(_) => self.gen_jsx_empty_expression(),
            Node::JSXExpressionContainer(inner) => {
                self.gen_jsx_expression_container(ctx, node, inner)
            }
            Node::JSXSpreadChild(inner) => self.gen_jsx_spread_child(ctx, node, inner),
            Node::JSXOpeningElement(inner) => self.gen_jsx_opening_element(ctx, node, inner),
            Node::JSXClosingElement(inner) => self.gen_jsx_closing_element(ctx, node, inner),
            Node::JSXAttribute(inner) => self.gen_jsx_attribute(ctx, node, inner),
            Node::JSXSpreadAttribute(inner) => self.gen_jsx_spread_attribute(ctx, node, inner),
            Node::JSXStringLiteral(inner) => self.gen_jsx_string_literal(ctx, inner),
            Node::JSXText(inner) => self.gen_jsx_text(ctx, inner),
            Node::JSXElement(inner) => self.gen_jsx_element(ctx, node, inner),
            Node::JSXFragment(inner) => self.gen_jsx_fragment(ctx, node, inner),
            Node::JSXOpeningFragment(_) => self.gen_jsx_opening_fragment(),
            Node::JSXClosingFragment(_) => self.gen_jsx_closing_fragment(),

            // Task 10 (`arms::flow_type`): Flow type annotations — the
            // primitive keyword types, `FunctionTypeAnnotation`/
            // `FunctionTypeParam`, `NullableTypeAnnotation`,
            // `QualifiedTypeIdentifier`, `TypeofTypeAnnotation`,
            // `TupleTypeAnnotation`, `ArrayTypeAnnotation`,
            // `UnionTypeAnnotation`, `IntersectionTypeAnnotation`,
            // `GenericTypeAnnotation`, `IndexedAccessType`,
            // `OptionalIndexedAccessType`, `InterfaceTypeAnnotation`. Also
            // `TypeParameterDeclaration`/`TypeParameterInstantiation`
            // (`arms/flow_type.rs`'s module doc comment explains why those
            // two — technically past this task's own cited line range — are
            // ported here rather than left for Task 11: `GenericTypeAnnotation`/
            // `FunctionTypeAnnotation` above are non-functional without
            // them). Task 11, if you are reading this: do not re-add either.
            Node::ExistsTypeAnnotation(_) => self.gen_exists_type_annotation(),
            Node::EmptyTypeAnnotation(_) => self.gen_empty_type_annotation(),
            Node::StringTypeAnnotation(_) => self.gen_string_type_annotation(),
            Node::BigIntTypeAnnotation(_) => self.gen_bigint_type_annotation(),
            Node::NumberTypeAnnotation(_) => self.gen_number_type_annotation(),
            Node::StringLiteralTypeAnnotation(inner) => {
                self.gen_string_literal_type_annotation(ctx, inner)
            }
            Node::NumberLiteralTypeAnnotation(inner) => {
                self.gen_number_literal_type_annotation(ctx, inner)
            }
            Node::BigIntLiteralTypeAnnotation(inner) => {
                self.gen_bigint_literal_type_annotation(ctx, inner)
            }
            Node::BooleanTypeAnnotation(_) => self.gen_boolean_type_annotation(),
            Node::BooleanLiteralTypeAnnotation(inner) => {
                self.gen_boolean_literal_type_annotation(inner)
            }
            Node::NullLiteralTypeAnnotation(_) => self.gen_null_literal_type_annotation(),
            Node::SymbolTypeAnnotation(_) => self.gen_symbol_type_annotation(),
            Node::AnyTypeAnnotation(_) => self.gen_any_type_annotation(),
            Node::MixedTypeAnnotation(_) => self.gen_mixed_type_annotation(),
            Node::VoidTypeAnnotation(_) => self.gen_void_type_annotation(),
            Node::FunctionTypeAnnotation(inner) => {
                self.gen_function_type_annotation(ctx, node, inner)
            }
            Node::FunctionTypeParam(inner) => self.gen_function_type_param(ctx, node, inner),
            Node::NullableTypeAnnotation(inner) => {
                self.gen_nullable_type_annotation(ctx, node, inner)
            }
            Node::QualifiedTypeIdentifier(inner) => {
                self.gen_qualified_type_identifier(ctx, node, inner)
            }
            Node::TypeofTypeAnnotation(inner) => self.gen_typeof_type_annotation(ctx, node, inner),
            Node::TupleTypeAnnotation(inner) => self.gen_tuple_type_annotation(ctx, node, inner),
            Node::ArrayTypeAnnotation(inner) => self.gen_array_type_annotation(ctx, node, inner),
            Node::UnionTypeAnnotation(inner) => self.gen_union_type_annotation(ctx, node, inner),
            Node::IntersectionTypeAnnotation(inner) => {
                self.gen_intersection_type_annotation(ctx, node, inner)
            }
            Node::GenericTypeAnnotation(inner) => {
                self.gen_generic_type_annotation(ctx, node, inner)
            }
            Node::IndexedAccessType(inner) => self.gen_indexed_access_type(ctx, node, inner),
            Node::OptionalIndexedAccessType(inner) => {
                self.gen_optional_indexed_access_type(ctx, node, inner)
            }
            Node::InterfaceTypeAnnotation(inner) => {
                self.gen_interface_type_annotation(ctx, node, inner)
            }
            Node::TypeParameterDeclaration(TypeParameterDeclaration {
                metadata: _,
                params,
            }) => self.gen_type_parameter_list(ctx, node, *params),
            Node::TypeParameterInstantiation(TypeParameterInstantiation {
                metadata: _,
                params,
            }) => self.gen_type_parameter_list(ctx, node, *params),

            // Task 11 (`arms::flow_decl`): Flow declarations, object types
            // (and their five member kinds), and `enum`. The four `Declare*`
            // arms marked below are the first to read `path` — see this
            // method's own doc comment.
            Node::TypeAlias(inner) => self.gen_type_alias(ctx, node, inner),
            Node::DeclareTypeAlias(inner) => self.gen_declare_type_alias(ctx, node, inner),
            Node::OpaqueType(inner) => self.gen_opaque_type(ctx, node, inner),
            Node::InterfaceDeclaration(inner) => self.gen_interface_declaration(ctx, node, inner),
            Node::DeclareInterface(inner) => self.gen_declare_interface(ctx, node, inner),
            Node::DeclareOpaqueType(inner) => self.gen_declare_opaque_type(ctx, node, inner, path),
            Node::DeclareClass(inner) => self.gen_declare_class(ctx, node, inner, path),
            Node::DeclareFunction(inner) => self.gen_declare_function(ctx, node, inner, path),
            Node::DeclareVariable(inner) => self.gen_declare_variable(ctx, node, inner, path),
            Node::DeclareExportDeclaration(inner) => {
                self.gen_declare_export_declaration(ctx, node, inner)
            }
            Node::DeclareExportAllDeclaration(inner) => {
                self.gen_declare_export_all_declaration(ctx, node, inner)
            }
            Node::DeclareModule(inner) => self.gen_declare_module(ctx, node, inner),
            Node::DeclareModuleExports(inner) => self.gen_declare_module_exports(ctx, node, inner),
            Node::InterfaceExtends(inner) => self.gen_interface_extends(ctx, node, inner),
            Node::ClassImplements(inner) => self.gen_class_implements(ctx, node, inner),
            Node::TypeAnnotation(inner) => self.gen_type_annotation(ctx, node, inner),
            Node::ObjectTypeAnnotation(inner) => self.gen_object_type_annotation(ctx, node, inner),
            Node::ObjectTypeProperty(inner) => self.gen_object_type_property(ctx, node, inner),
            Node::ObjectTypeSpreadProperty(inner) => {
                self.gen_object_type_spread_property(ctx, node, inner)
            }
            Node::ObjectTypeInternalSlot(inner) => {
                self.gen_object_type_internal_slot(ctx, node, inner)
            }
            Node::ObjectTypeCallProperty(inner) => {
                self.gen_object_type_call_property(ctx, node, inner)
            }
            Node::ObjectTypeIndexer(inner) => self.gen_object_type_indexer(ctx, node, inner),
            Node::Variance(inner) => self.gen_variance(ctx, inner),
            Node::TypeParameter(inner) => self.gen_type_parameter(ctx, node, inner),
            Node::TypeCastExpression(inner) => self.gen_type_cast_expression(ctx, node, inner),
            Node::InferredPredicate(inner) => self.gen_inferred_predicate(inner),
            Node::DeclaredPredicate(inner) => self.gen_declared_predicate(ctx, node, inner),
            Node::EnumDeclaration(inner) => self.gen_enum_declaration(ctx, node, inner),
            Node::EnumStringBody(inner) => self.gen_enum_string_body(ctx, node, inner),
            Node::EnumNumberBody(inner) => self.gen_enum_number_body(ctx, node, inner),
            Node::EnumBooleanBody(inner) => self.gen_enum_boolean_body(ctx, node, inner),
            Node::EnumSymbolBody(inner) => self.gen_enum_symbol_body(ctx, node, inner),
            Node::EnumDefaultedMember(inner) => self.gen_enum_defaulted_member(ctx, node, inner),
            Node::EnumStringMember(inner) => self.gen_enum_string_member(ctx, node, inner),
            Node::EnumNumberMember(inner) => self.gen_enum_number_member(ctx, node, inner),
            Node::EnumBooleanMember(inner) => self.gen_enum_boolean_member(ctx, node, inner),

            // Task 12 (`arms::newer`): the 53 ES/Flow kinds juno's
            // generator predates entirely (no juno source for any arm
            // below — see that module's doc comment). Step 1 (ES-level):
            // `StaticBlock`, `Decorator`, `AsExpression`, `AsConstExpression`.
            Node::StaticBlock(inner) => self.gen_static_block(ctx, node, inner),
            Node::Decorator(inner) => self.gen_decorator(ctx, node, inner),
            Node::AsExpression(inner) => self.gen_as_expression(ctx, node, inner),
            Node::AsConstExpression(inner) => self.gen_as_const_expression(ctx, node, inner),

            // Step 2: the Flow `match` family (18 kinds).
            Node::MatchExpression(inner) => self.gen_match_expression(ctx, node, inner),
            Node::MatchStatement(inner) => self.gen_match_statement(ctx, node, inner),
            Node::MatchExpressionCase(inner) => self.gen_match_expression_case(ctx, node, inner),
            Node::MatchStatementCase(inner) => self.gen_match_statement_case(ctx, node, inner),
            Node::MatchArrayPattern(inner) => self.gen_match_array_pattern(ctx, node, inner),
            Node::MatchAsPattern(inner) => self.gen_match_as_pattern(ctx, node, inner),
            Node::MatchBindingPattern(inner) => self.gen_match_binding_pattern(ctx, node, inner),
            Node::MatchIdentifierPattern(inner) => {
                self.gen_match_identifier_pattern(ctx, node, inner)
            }
            Node::MatchInstanceObjectPattern(inner) => {
                self.gen_match_instance_object_pattern(ctx, node, inner)
            }
            Node::MatchInstancePattern(inner) => {
                self.gen_match_instance_pattern(ctx, node, inner)
            }
            Node::MatchLiteralPattern(inner) => self.gen_match_literal_pattern(ctx, node, inner),
            Node::MatchMemberPattern(inner) => self.gen_match_member_pattern(ctx, node, inner),
            Node::MatchObjectPattern(inner) => self.gen_match_object_pattern(ctx, node, inner),
            Node::MatchObjectPatternProperty(inner) => {
                self.gen_match_object_pattern_property(ctx, node, inner)
            }
            Node::MatchOrPattern(inner) => self.gen_match_or_pattern(ctx, node, inner),
            Node::MatchRestPattern(inner) => self.gen_match_rest_pattern(ctx, node, inner),
            Node::MatchUnaryPattern(inner) => self.gen_match_unary_pattern(ctx, node, inner),
            Node::MatchWildcardPattern(_) => self.gen_match_wildcard_pattern(),

            // Step 3: the Flow `record` family (7 kinds).
            Node::RecordDeclaration(inner) => self.gen_record_declaration(ctx, node, inner),
            Node::RecordDeclarationBody(inner) => {
                self.gen_record_declaration_body(ctx, node, inner)
            }
            Node::RecordDeclarationImplements(inner) => {
                self.gen_record_declaration_implements(ctx, node, inner)
            }
            Node::RecordDeclarationProperty(inner) => {
                self.gen_record_declaration_property(ctx, node, inner)
            }
            Node::RecordDeclarationStaticProperty(inner) => {
                self.gen_record_declaration_static_property(ctx, node, inner)
            }
            Node::RecordExpression(inner) => self.gen_record_expression(ctx, node, inner),
            Node::RecordExpressionProperties(inner) => {
                self.gen_record_expression_properties(ctx, node, inner)
            }

            // Step 4: Flow `component`/`hook` (8 kinds). `DeclareComponent`/
            // `DeclareHook` are the first new callers of `path` since Task
            // 11's four `Declare*` arms (see `arms::flow_decl`'s
            // `declare_prefix_needed`, bumped `pub(crate)` for this task).
            Node::ComponentDeclaration(inner) => self.gen_component_declaration(ctx, node, inner),
            Node::ComponentParameter(inner) => self.gen_component_parameter(ctx, node, inner),
            Node::ComponentTypeAnnotation(inner) => {
                self.gen_component_type_annotation(ctx, node, inner)
            }
            Node::ComponentTypeParameter(inner) => {
                self.gen_component_type_parameter(ctx, node, inner)
            }
            Node::DeclareComponent(inner) => self.gen_declare_component(ctx, node, inner, path),
            Node::DeclareHook(inner) => self.gen_declare_hook(ctx, node, inner, path),
            Node::HookDeclaration(inner) => self.gen_hook_declaration(ctx, node, inner),
            Node::HookTypeAnnotation(inner) => self.gen_hook_type_annotation(ctx, node, inner),

            // Step 5: the remaining type kinds (16), including
            // `DeclareEnum` (also a new `path` caller) and `DeclareNamespace`.
            Node::ConditionalTypeAnnotation(inner) => {
                self.gen_conditional_type_annotation(ctx, node, inner)
            }
            Node::InferTypeAnnotation(inner) => self.gen_infer_type_annotation(ctx, node, inner),
            Node::KeyofTypeAnnotation(inner) => self.gen_keyof_type_annotation(ctx, node, inner),
            Node::NeverTypeAnnotation(_) => self.gen_never_type_annotation(),
            Node::UndefinedTypeAnnotation(_) => self.gen_undefined_type_annotation(),
            Node::UnknownTypeAnnotation(_) => self.gen_unknown_type_annotation(),
            Node::TypeOperator(inner) => self.gen_type_operator(ctx, node, inner),
            Node::TypePredicate(inner) => self.gen_type_predicate(ctx, node, inner),
            Node::ObjectTypeMappedTypeProperty(inner) => {
                self.gen_object_type_mapped_type_property(ctx, node, inner)
            }
            Node::QualifiedTypeofIdentifier(inner) => {
                self.gen_qualified_typeof_identifier(ctx, node, inner)
            }
            Node::TupleTypeLabeledElement(inner) => {
                self.gen_tuple_type_labeled_element(ctx, node, inner)
            }
            Node::TupleTypeSpreadElement(inner) => {
                self.gen_tuple_type_spread_element(ctx, node, inner)
            }
            Node::DeclareEnum(inner) => self.gen_declare_enum(ctx, node, inner, path),
            Node::DeclareNamespace(inner) => self.gen_declare_namespace(ctx, node, inner),
            Node::EnumBigIntBody(inner) => self.gen_enum_bigint_body(ctx, node, inner),
            Node::EnumBigIntMember(inner) => self.gen_enum_bigint_member(ctx, node, inner),

            // Task 13 (`arms::ts`): the 46 TypeScript kinds. juno's
            // generator has no TS arm at all (`Node::TS` occurs 0 times in
            // `gen_js.rs`), so each arm's specification is our parser's own
            // production — see that module's doc comment.
            //
            // The temporary wildcard arm Task 1 added at the end of this
            // match is DELETED as of this task: every kind is now named, so
            // the compiler — not a runtime error — proves all 271 are
            // handled. Do not re-add it; `tests/exhaustive.rs`'s
            // `temporary_catch_all_is_gone` also guards the source text of
            // this file with a plain substring search, so a comment must not
            // quote that arm verbatim either.
            Node::TSTypeAnnotation(inner) => self.gen_ts_type_annotation(ctx, node, inner),
            Node::TSAnyKeyword(_) => self.gen_ts_any_keyword(),
            Node::TSNumberKeyword(_) => self.gen_ts_number_keyword(),
            Node::TSBooleanKeyword(_) => self.gen_ts_boolean_keyword(),
            Node::TSStringKeyword(_) => self.gen_ts_string_keyword(),
            Node::TSSymbolKeyword(_) => self.gen_ts_symbol_keyword(),
            Node::TSVoidKeyword(_) => self.gen_ts_void_keyword(),
            Node::TSUndefinedKeyword(_) => self.gen_ts_undefined_keyword(),
            Node::TSUnknownKeyword(_) => self.gen_ts_unknown_keyword(),
            Node::TSNeverKeyword(_) => self.gen_ts_never_keyword(),
            Node::TSBigIntKeyword(_) => self.gen_ts_bigint_keyword(),
            Node::TSThisType(_) => self.gen_ts_this_type(),
            Node::TSLiteralType(inner) => self.gen_ts_literal_type(ctx, node, inner),
            Node::TSIndexedAccessType(inner) => self.gen_ts_indexed_access_type(ctx, node, inner),
            Node::TSArrayType(inner) => self.gen_ts_array_type(ctx, node, inner),
            Node::TSTypeReference(inner) => self.gen_ts_type_reference(ctx, node, inner),
            Node::TSQualifiedName(inner) => self.gen_ts_qualified_name(ctx, node, inner),
            Node::TSFunctionType(inner) => self.gen_ts_function_type(ctx, node, inner),
            Node::TSConstructorType(inner) => self.gen_ts_constructor_type(ctx, node, inner),
            Node::TSTypePredicate(inner) => self.gen_ts_type_predicate(ctx, node, inner),
            Node::TSTupleType(inner) => self.gen_ts_tuple_type(ctx, node, inner),
            Node::TSTypeAssertion(inner) => self.gen_ts_type_assertion(ctx, node, inner),
            Node::TSAsExpression(inner) => self.gen_ts_as_expression(ctx, node, inner),
            Node::TSParameterProperty(inner) => self.gen_ts_parameter_property(ctx, node, inner),
            Node::TSTypeAliasDeclaration(inner) => {
                self.gen_ts_type_alias_declaration(ctx, node, inner)
            }
            Node::TSInterfaceDeclaration(inner) => {
                self.gen_ts_interface_declaration(ctx, node, inner)
            }
            Node::TSInterfaceHeritage(inner) => self.gen_ts_interface_heritage(ctx, node, inner),
            Node::TSInterfaceBody(inner) => self.gen_ts_interface_body(ctx, node, inner),
            Node::TSEnumDeclaration(inner) => self.gen_ts_enum_declaration(ctx, node, inner),
            Node::TSEnumMember(inner) => self.gen_ts_enum_member(ctx, node, inner),
            Node::TSModuleDeclaration(inner) => self.gen_ts_module_declaration(ctx, node, inner),
            Node::TSModuleBlock(inner) => self.gen_ts_module_block(ctx, node, inner),
            Node::TSModuleMember(inner) => self.gen_ts_module_member(ctx, node, inner),
            // The two TS type-parameter list kinds reuse Task 10's shared
            // `gen_type_parameter_list` (`arms/flow_type.rs`), exactly as
            // their Flow counterparts `TypeParameterDeclaration`/
            // `TypeParameterInstantiation` do above: both print `<p, p, …>`
            // and differ only in which side of a generic they appear on
            // (`parse_ts_type_parameters` vs. `parse_ts_type_arguments`,
            // `crates/parser/src/js/ts/params.rs`). `TSTypeParameter` itself
            // — unlike Flow's `TypeParameter` — has its own arm, since its
            // fields differ (`constraint`/`default`, no variance).
            Node::TSTypeParameterDeclaration(TSTypeParameterDeclaration {
                metadata: _,
                params,
            }) => self.gen_type_parameter_list(ctx, node, *params),
            // NOT the shared `gen_type_parameter_list`: a type-*argument*
            // list's elements are full types and need `print_child` for the
            // TS intruder rule — see `arms/ts.rs`'s
            // `gen_ts_type_argument_list`.
            Node::TSTypeParameterInstantiation(TSTypeParameterInstantiation {
                metadata: _,
                params,
            }) => self.gen_ts_type_argument_list(ctx, node, *params),
            Node::TSTypeParameter(inner) => self.gen_ts_type_parameter(ctx, node, inner),
            Node::TSUnionType(inner) => self.gen_ts_union_type(ctx, node, inner),
            Node::TSIntersectionType(inner) => self.gen_ts_intersection_type(ctx, node, inner),
            Node::TSTypeQuery(inner) => self.gen_ts_type_query(ctx, node, inner),
            Node::TSConditionalType(inner) => self.gen_ts_conditional_type(ctx, node, inner),
            Node::TSTypeLiteral(inner) => self.gen_ts_type_literal(ctx, node, inner),
            Node::TSPropertySignature(inner) => self.gen_ts_property_signature(ctx, node, inner),
            Node::TSMethodSignature(inner) => self.gen_ts_method_signature(ctx, node, inner),
            Node::TSIndexSignature(inner) => self.gen_ts_index_signature(ctx, node, inner),
            Node::TSCallSignatureDeclaration(inner) => {
                self.gen_ts_call_signature_declaration(ctx, node, inner)
            }
            Node::TSModifiers(inner) => self.gen_ts_modifiers(ctx, inner),
        }
    }
}
