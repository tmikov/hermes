/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The 46 TypeScript node kinds (plan Task 13, spec §4).
//!
//! # There is no juno source for any arm in this file
//!
//! juno's generator contains **zero** TypeScript arms: `Node::TS` occurs 0
//! times in all 4174 lines of `unsupported/juno/crates/juno/src/gen_js.rs`,
//! and juno's own AST definition (`juno_ast/src/def.rs`) has no `TS*` kind
//! at all. So — exactly as in Task 12's `arms/newer.rs` — there is no
//! `// juno gen_js.rs:NNNN-MMMM` citation to give, and the specification for
//! each arm is **our parser's production**: the function that *builds* the
//! node defines the syntax that has to be emitted to reproduce it. Every
//! arm below cites the parser function it was derived from
//! (`crates/parser/src/js/ts/`, plus `js/expressions.rs` for
//! `TSAsExpression`/`TSTypeAssertion` and `js/classes.rs` for `TSModifiers`),
//! itself a port of `lib/Parser/JSParserImpl-ts.cpp` with C++ line citations
//! in place.
//!
//! # The TypeScript type-grammar precedence space
//!
//! `precedence.rs` gains a fourth numbering space for these kinds (alongside
//! the expression space, the Flow `UNION_TYPE`/`INTERSECTION_TYPE` pair, and
//! Task 12's `MATCH_*` trio) — see the `TS_*_TYPE` constants there. The
//! tiers are exactly the layers of our parser's own recursive descent
//! (`crates/parser/src/js/ts/types.rs`):
//!
//! ```text
//! parse_type_annotation_ts   -> predicate | `new` ctor type | `<T>` fn type
//!                               | union   [ `extends` T `?` T `:` T ]
//! parse_ts_union_type        -> intersection ( `|` intersection )*
//! parse_ts_intersection_type -> postfix ( `&` postfix )*
//! parse_ts_postfix_type      -> primary ( `[` `]` | `[` Type `]` )*
//! parse_ts_primary_type      -> keyword | literal | `this` | `*` | tuple
//!                               | `typeof` … | `{` … `}` | `interface` …
//!                               | type reference | `(` …
//! ```
//!
//! A parenthesized type is a *primary* (`parse_ts_primary_type`'s `l_paren`
//! arm), and its contents are a full `Type`
//! (`parse_ts_function_or_parenthesized_type`, which on the non-function
//! path calls `inc_parens` on the inner type and returns it — the parens
//! leave **no node** behind, only a paren count nothing in this crate
//! reads). That is what makes parenthesization able to rescue any tier into
//! any position, and it is why every field parsed at a tier narrower than
//! full-`Type` is printed through `print_child` rather than `gen_node`.
//!
//! Six fields are **narrowed** — parsed at a tier tighter than a full
//! `Type` — and go through `print_child` for that reason:
//!
//! | field | parsed at | tier |
//! |---|---|---|
//! | `TSArrayType::element_type` | `parse_ts_postfix_type` | primary |
//! | `TSIndexedAccessType::object_type` | `parse_ts_postfix_type` | primary |
//! | `TSIntersectionType::types` | `parse_ts_intersection_type` | postfix |
//! | `TSUnionType::types` | `parse_ts_union_type` | intersection |
//! | `TSConditionalType::check_type` | `parse_type_annotation_ts` | union (a `need_parens` threshold branch, not the plain comparison — see there) |
//! | `TSAsExpression::expression`, `TSTypeAssertion::expression` | `parse_binary_expression` / `parse_unary_expression` | expression tiers |
//!
//! Each of the six was mutation-tested: reverting it to a bare `gen_node`
//! makes a specific named test in `tests/roundtrip.rs` fail. The
//! field-by-field audit, with the wrong output each mutation produced, is in
//! `task-13-report.md`.
//!
//! **Every *other* TS type field goes through `print_child` too**, for a
//! different reason discovered in review round 1: not precedence, but the
//! fact that our parser can put an **expression-space** node in a type slot
//! at all. `parse_ts_function_or_parenthesized_type` hands
//! `parse_binding_element` results back AS the type when no `=>` follows, so
//! `type T = ({ a: A })[];` really is
//! `TSArrayType { element_type: ObjectPattern }`, and printed bare it
//! regenerates as `type T={a:A}[];` — which reparses to a `TSTypeLiteral`,
//! a different tree with no diagnostic. `precedence.rs`'s `is_ts_type_node`
//! allow-list plus `is_full_ts_type_field`/`is_narrowed_ts_type_field` is
//! the rule; those two predicates' doc comments carry the measured cases and
//! the reasoning. Note the earlier draft of this comment asserted that a
//! full-`Type` field "accepts anything, so nothing can need parens" — that
//! was a false universal, and it is what hid this family: a full-`Type` field
//! accepts anything the **type grammar** produces, which is not the same as
//! anything that can appear in the field.
//!
//! # Two defects found while deriving these arms, fixed in `precedence.rs`
//!
//! **1. An `as`-expression's right operand is a *type*, and the type grammar
//! keeps reading.** Task 12 classified `AsExpression`/`AsConstExpression` at
//! `In`'s binary precedence, reasoning (correctly) that `as` is built by the
//! same precedence-climbing loop as `in`/`instanceof` at the same operator
//! precedence 8. But the *right* operand is not an expression: the loop
//! calls `parse_type_annotation` for it
//! (`crates/parser/src/js/expressions.rs`, the `as_operator` branch of
//! `parse_binary_expression`), and the type grammar then greedily consumes
//! `|` (union), `&` (intersection), `[` (postfix), `<` (type arguments) and
//! `.` (qualified name) — tokens that the *enclosing expression* may have
//! meant for itself. Measured on the crate as Task 12 shipped it, under
//! `-parse-flow`:
//!
//! ```text
//! (x as A) | B;      -> x as A | B;      reparses to As(x, Union[A,B])  WRONG
//! (x as A) & B;      -> x as A & B;      reparses to As(x, Inter[A,B])  WRONG
//! (x as A) < B;      -> x as A < B;      FAILS to reparse
//! b + (x as A) | c;  -> b + x as A | c;  reparses to b + As(x, Union)   WRONG
//! ```
//!
//! The `[`/`.` cases were already safe (`MEMBER` outranks the as-expression
//! either way). The fix is a table change, not a special case: the
//! as-expression tier moves *below* every binary and logical operator
//! (`precedence.rs`'s new `AS_EXPRESSION`, with `BIN_START` bumped 6 -> 7 so
//! there is a slot for it), which is what "the right operand runs away with
//! anything an operator token could have started" actually means. The
//! grandparent case is why a `need_parens` branch keyed on the *direct*
//! parent's operator would not have been enough: in `b + (x as A) | c`, which
//! parses as `(b + (x as A)) | c`, the as-expression's direct parent operator
//! is the harmless `+`, and the `|` that the type grammar absorbs belongs to
//! the grandparent. (`b | (x as A) | c` is left-nested, so its direct parent
//! *is* a `|` — it demonstrates the breakage but not the need for the
//! grandparent: review round 1 finding M-2.) `TSAsExpression` joins the same arm — the TS type grammar
//! absorbs the identical token set (`parse_ts_union_type`,
//! `parse_ts_intersection_type`, `parse_ts_postfix_type`,
//! `parse_ts_type_reference`, `parse_ts_qualified_name`).
//!
//! **2. Every TS class *property* carries a `TSModifiers` node, so Task 7's
//! `ts_modifiers.is_some() => UnsupportedKind` bail made every TS class with
//! a property field ungeneratable.** `crates/parser/src/js/classes.rs` builds
//! a `TSModifiers` unconditionally under `-parse-ts` (both the
//! `ClassPrivateProperty` and the `ClassProperty` construction sites), with
//! `accessibility: null, readonly: false` for a plain `class C { x = 1; }`.
//! Measured with the old bail restored: `class C { x = 1; }`,
//! `class C { #p = 1; }` and `class C { public x: A; }` all failed to
//! generate, while `class C {}`, `class C { m() {} }` and
//! `class C { static m() {} get p() {} }` generated fine — the bail lived
//! only in `gen_class_property`/`gen_class_private_property`, so method-only
//! and empty classes were never affected. `arms/func.rs`'s two
//! arms now print it instead, and they cannot print it as one unit: the
//! parser accepts the modifiers only in the order **accessibility, static,
//! readonly** (`classes.rs`, the `parse_ts()` block: three sequential
//! `check`s in that order, each guarded by `can_follow_modifier_ts`), while
//! `static` lives on the class member itself, not on `TSModifiers`. So the
//! two halves are printed either side of `static` via
//! [`GenJS::print_ts_modifiers_accessibility`] and
//! [`GenJS::print_ts_modifiers_readonly`]; the `TSModifiers` dispatch arm
//! prints both in the same order for the (unreachable-from-our-parser) case
//! of a `TSModifiers` reached directly. Printing it as one unit before
//! `static` was tried first and is WRONG: `public readonly static x` makes
//! the parser take `static` as the *property name*
//! (`classes.rs`: "don't advance() when `readonly` or `static` is already
//! seen, so the current one can be regarded as an identifier").
//!
//! # One kind our parser never builds
//!
//! `TSModuleDeclaration` has **no construction site anywhere in
//! `crates/parser`** (`grep -rn 'Node::TSModuleDeclaration('` returns
//! nothing): our parser spells `namespace X { … }` as a `TSModuleMember`
//! (`crates/parser/src/js/ts/declarations.rs`'s
//! `parse_ts_namespace_declaration`). The arm below therefore prints the
//! `namespace id body` spelling that the *reparse* turns into the
//! corresponding `TSModuleMember`, and its test hand-builds the node rather
//! than parsing one — this module's own
//! `tests::ts_module_declaration_prints_a_namespace_with_its_block_body`
//! (not `tests/roundtrip.rs`, which can only drive whole parsed programs).

use hermes_ast::context::GCLock;
use hermes_ast::node::{
    Node, NodeField, TSArrayType, TSAsExpression, TSCallSignatureDeclaration, TSConditionalType,
    TSConstructorType, TSEnumDeclaration, TSEnumMember, TSFunctionType, TSIndexSignature,
    TSIndexedAccessType, TSInterfaceBody, TSInterfaceDeclaration, TSInterfaceHeritage,
    TSIntersectionType, TSLiteralType, TSMethodSignature, TSModifiers, TSModuleBlock,
    TSModuleDeclaration, TSModuleMember, TSParameterProperty, TSPropertySignature, TSQualifiedName,
    TSTupleType, TSTypeAliasDeclaration, TSTypeAnnotation, TSTypeAssertion, TSTypeLiteral,
    TSTypeParameter, TSTypePredicate, TSTypeQuery, TSTypeReference, TSUnionType,
};
use hermes_ast::node_child::{NodeLabel, NodeList};
use hermes_ast::visitor::Path;

use crate::gen::Pretty;
use crate::out;
use crate::precedence::{ChildPos, ForceSpace};
use crate::{GenJS, GenJsError};

impl<'s, 'w> GenJS<'s, 'w> {
    // -----------------------------------------------------------------------
    // Shared helpers.
    // -----------------------------------------------------------------------

    /// Print `( p1, p2, … )` for a TypeScript parameter list.
    ///
    /// Every caller's list comes from `parse_ts_function_type_params` /
    /// `parse_ts_function_or_parenthesized_type`
    /// (`crates/parser/src/js/ts/function_types.rs`), whose elements are
    /// whatever `parse_binding_element` returns — an `Identifier` (carrying
    /// its own `?` and `: T`, printed by `arms/literal.rs`'s
    /// `gen_identifier`), an `ObjectPattern`/`ArrayPattern`/
    /// `AssignmentPattern`, a `RestElement`, or a `TSParameterProperty`
    /// wrapper. None of those is parsed at a narrowed tier — the list is
    /// comma-delimited and each element is self-terminating — so they print
    /// through plain `gen_node`.
    fn visit_ts_params<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        params: NodeList<'gc>,
        field: NodeField,
    ) -> Result<(), GenJsError> {
        out!(self, "(");
        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            self.gen_node(ctx, param, Some(Path::new(node, field)))?;
        }
        out!(self, ")");
        Ok(())
    }

    /// Print `{ m1; m2; … }` for a TypeScript object-type member list
    /// (`TSTypeLiteral`'s `members` and `TSInterfaceBody`'s `body`).
    ///
    /// `;` rather than `,` as the separator: `parse_ts_object_type` and
    /// `parse_ts_interface_declaration`'s body loop both accept either
    /// (`check2(TokenKind::comma, TokenKind::semi)`) and treat the trailing
    /// one as optional, and `;` is the spelling that keeps a
    /// `TSMethodSignature`/`TSCallSignatureDeclaration` member readable.
    /// The trailing `;` before `}` is deliberate and accepted: the
    /// separator is eaten, then the `while !check(r_brace)` loop exits.
    fn visit_ts_object_members<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        members: NodeList<'gc>,
        field: NodeField,
    ) -> Result<(), GenJsError> {
        if members.is_empty() {
            out!(self, "{{}}");
            return Ok(());
        }
        out!(self, "{{");
        self.inc_indent();
        self.newline();
        for (i, member) in members.iter().enumerate() {
            if i > 0 {
                self.newline();
            }
            self.gen_node(ctx, member, Some(Path::new(node, field)))?;
            out!(self, ";");
        }
        self.dec_indent();
        self.newline();
        out!(self, "}}");
        Ok(())
    }

    /// Print `: T` (with a pretty-mode space after the colon) for an
    /// optional return/element type annotation.
    fn print_ts_colon_type<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        type_annotation: Option<&'gc Node<'gc>>,
        field: NodeField,
    ) -> Result<(), GenJsError> {
        if let Some(type_annotation) = type_annotation {
            out!(self, ":");
            self.space(ForceSpace::No);
            self.print_child(
                ctx,
                Some(type_annotation),
                Path::new(node, field),
                ChildPos::Anywhere,
            )?;
        }
        Ok(())
    }

    /// Print a `TSModifiers`' `accessibility` half (`public `/`private `/
    /// `protected `), which the parser accepts only *before* `static`.
    ///
    /// `accessibility` is a `NodeLabel` whose "absent" value is
    /// `INVALID_ATOM_BYTES` (`crates/parser/src/js/classes.rs` initializes it
    /// so, and `crates/ast/src/dump.rs` dumps that as JSON `null`).
    /// `ctx.try_bytes_str` returns `None` for it — `hermes_atom_table`'s own
    /// test asserts `try_bytes_str(INVALID_ATOM_BYTES) == None` — which is
    /// the same "is this label present at all" test `arms/stmt.rs`'s
    /// `gen_expression_statement` uses for `directive`, and avoids a
    /// dependency on `hermes_atom_table` just for the sentinel. The three
    /// spellings the parser can store are all ASCII, so the `None` return
    /// can only mean "absent" here, never "unpaired surrogate".
    pub(crate) fn print_ts_modifiers_accessibility(
        &mut self,
        ctx: &GCLock<'_, '_>,
        accessibility: NodeLabel,
    ) {
        if let Some(s) = ctx.try_bytes_str(accessibility) {
            self.write_utf8(s);
            out!(self, " ");
        }
    }

    /// Print a `TSModifiers`' `readonly` half, which the parser accepts only
    /// *after* `static` (`crates/parser/src/js/classes.rs`).
    pub(crate) fn print_ts_modifiers_readonly(&mut self, readonly: bool) {
        if readonly {
            out!(self, "readonly ");
        }
    }

    // -----------------------------------------------------------------------
    // Annotations and primitive keyword types.
    // -----------------------------------------------------------------------

    /// `TSTypeAnnotation`: transparent — prints only the type it wraps.
    ///
    /// Built by `parse_type_annotation_ts` when its `wrapped_start` argument
    /// is `Some` (`crates/parser/src/js/ts/types.rs`), i.e. for a `: T`
    /// annotation; the `:` itself belongs to whatever construct introduced
    /// it and is printed there, exactly as `arms/flow_decl.rs`'s Flow
    /// `TypeAnnotation` arm does.
    pub(crate) fn gen_ts_type_annotation<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSTypeAnnotation<'gc>,
    ) -> Result<(), GenJsError> {
        let TSTypeAnnotation {
            metadata: _,
            type_annotation,
        } = inner;
        self.print_child(
            ctx,
            Some(*type_annotation),
            Path::new(node, NodeField::type_annotation),
            ChildPos::Anywhere,
        )
    }

    /// `TSAnyKeyword`: `any`. `parse_ts_primary_type`'s identifier arm.
    pub(crate) fn gen_ts_any_keyword(&mut self) -> Result<(), GenJsError> {
        out!(self, "any");
        Ok(())
    }

    /// `TSNumberKeyword`: `number`. `parse_ts_primary_type`'s identifier arm.
    pub(crate) fn gen_ts_number_keyword(&mut self) -> Result<(), GenJsError> {
        out!(self, "number");
        Ok(())
    }

    /// `TSBooleanKeyword`: `boolean`. `parse_ts_primary_type`'s identifier arm.
    pub(crate) fn gen_ts_boolean_keyword(&mut self) -> Result<(), GenJsError> {
        out!(self, "boolean");
        Ok(())
    }

    /// `TSStringKeyword`: `string`. `parse_ts_primary_type`'s identifier arm.
    pub(crate) fn gen_ts_string_keyword(&mut self) -> Result<(), GenJsError> {
        out!(self, "string");
        Ok(())
    }

    /// `TSSymbolKeyword`: `symbol`. `parse_ts_primary_type`'s identifier arm.
    pub(crate) fn gen_ts_symbol_keyword(&mut self) -> Result<(), GenJsError> {
        out!(self, "symbol");
        Ok(())
    }

    /// `TSVoidKeyword`: `void`. `parse_ts_primary_type`'s `rw_void` arm —
    /// the one primitive spelled with a reserved word, not a contextual
    /// identifier.
    pub(crate) fn gen_ts_void_keyword(&mut self) -> Result<(), GenJsError> {
        out!(self, "void");
        Ok(())
    }

    /// `TSUndefinedKeyword`: `undefined`. `parse_ts_primary_type`'s
    /// identifier arm.
    pub(crate) fn gen_ts_undefined_keyword(&mut self) -> Result<(), GenJsError> {
        out!(self, "undefined");
        Ok(())
    }

    /// `TSUnknownKeyword`: `unknown`. `parse_ts_primary_type`'s identifier arm.
    pub(crate) fn gen_ts_unknown_keyword(&mut self) -> Result<(), GenJsError> {
        out!(self, "unknown");
        Ok(())
    }

    /// `TSNeverKeyword`: `never`. `parse_ts_primary_type`'s identifier arm.
    pub(crate) fn gen_ts_never_keyword(&mut self) -> Result<(), GenJsError> {
        out!(self, "never");
        Ok(())
    }

    /// `TSBigIntKeyword`: `bigint`. `parse_ts_primary_type`'s identifier arm.
    pub(crate) fn gen_ts_bigint_keyword(&mut self) -> Result<(), GenJsError> {
        out!(self, "bigint");
        Ok(())
    }

    /// `TSThisType`: `this`. `parse_ts_primary_type`'s `rw_this` arm.
    pub(crate) fn gen_ts_this_type(&mut self) -> Result<(), GenJsError> {
        out!(self, "this");
        Ok(())
    }

    /// `TSLiteralType`: a literal token used as a type (`"lit"`, `42`,
    /// `123n`, `true`, `false`, `null`).
    ///
    /// `parse_ts_primary_type` wraps exactly five literal node kinds —
    /// `StringLiteral`, `NumericLiteral`, `BigIntLiteral`, `BooleanLiteral`,
    /// `NullLiteral` — each built straight from one token, so `literal`
    /// prints through plain `gen_node`: none of those five is parsed at a
    /// narrowed tier and none can carry source parens (the type grammar
    /// reaches them from `parse_ts_primary_type` directly, not through an
    /// expression production).
    pub(crate) fn gen_ts_literal_type<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSLiteralType<'gc>,
    ) -> Result<(), GenJsError> {
        let TSLiteralType {
            metadata: _,
            literal,
        } = inner;
        self.gen_node(ctx, literal, Some(Path::new(node, NodeField::literal)))
    }

    // -----------------------------------------------------------------------
    // Type constructors.
    // -----------------------------------------------------------------------

    /// `TSArrayType`: `T[]`.
    ///
    /// `parse_ts_postfix_type` applies `[]` to whatever
    /// `parse_ts_primary_type` returned, so `element_type` is a **primary**
    /// tier position — narrower than full `Type` — and goes through
    /// `print_child` at `ChildPos::Left`. Without that, `(A | B)[]`
    /// regenerates as `A | B[]`, which reparses as `A | (B[])`: a different
    /// tree. `ChildPos::Left` with `Assoc::Ltr` keeps a chained
    /// `A[][]`/`A[K][]` bare (equal precedence on the safe side).
    pub(crate) fn gen_ts_array_type<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSArrayType<'gc>,
    ) -> Result<(), GenJsError> {
        let TSArrayType {
            metadata: _,
            element_type,
        } = inner;
        self.print_child(
            ctx,
            Some(*element_type),
            Path::new(node, NodeField::element_type),
            ChildPos::Left,
        )?;
        out!(self, "[]");
        Ok(())
    }

    /// `TSIndexedAccessType`: `T[K]`.
    ///
    /// Same `parse_ts_postfix_type` loop as [`GenJS::gen_ts_array_type`], so
    /// `object_type` is a primary-tier position and needs `print_child`;
    /// `index_type` sits between `[` and `]` and is parsed by a full
    /// `parse_type_annotation_ts(None)`, so it prints bare.
    pub(crate) fn gen_ts_indexed_access_type<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSIndexedAccessType<'gc>,
    ) -> Result<(), GenJsError> {
        let TSIndexedAccessType {
            metadata: _,
            object_type,
            index_type,
        } = inner;
        self.print_child(
            ctx,
            Some(*object_type),
            Path::new(node, NodeField::object_type),
            ChildPos::Left,
        )?;
        out!(self, "[");
        self.print_child(
            ctx,
            Some(*index_type),
            Path::new(node, NodeField::index_type),
            ChildPos::Anywhere,
        )?;
        out!(self, "]");
        Ok(())
    }

    /// `TSTypeReference`: `A`, `A.B.C`, `A<X, Y>`.
    ///
    /// `parse_ts_type_reference`: a qualified name followed by an optional
    /// `<…>` argument list. Both children are structural, not tiered —
    /// `type_name` is an `Identifier` or a `TSQualifiedName` built by
    /// `parse_ts_qualified_name`, and `type_parameters` is always a
    /// `TSTypeParameterInstantiation` — so both print bare.
    pub(crate) fn gen_ts_type_reference<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSTypeReference<'gc>,
    ) -> Result<(), GenJsError> {
        let TSTypeReference {
            metadata: _,
            type_name,
            type_parameters,
        } = inner;
        self.gen_node(ctx, type_name, Some(Path::new(node, NodeField::type_name)))?;
        if let Some(type_parameters) = type_parameters {
            self.gen_node(
                ctx,
                type_parameters,
                Some(Path::new(node, NodeField::type_parameters)),
            )?;
        }
        Ok(())
    }

    /// `TSQualifiedName`: `left.right`.
    ///
    /// `parse_ts_qualified_name` left-nests one node per `.`, so `left` is
    /// either an `Identifier` or another `TSQualifiedName`. `right` is
    /// `Option` in the AST but always `Some` from the parser (the loop only
    /// builds a node after it has an identifier in hand); a `None` prints
    /// just `left`, so a hand-built node degrades to its own left spine
    /// rather than emitting a trailing `.`.
    pub(crate) fn gen_ts_qualified_name<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSQualifiedName<'gc>,
    ) -> Result<(), GenJsError> {
        let TSQualifiedName {
            metadata: _,
            left,
            right,
        } = inner;
        self.gen_node(ctx, left, Some(Path::new(node, NodeField::left)))?;
        if let Some(right) = right {
            out!(self, ".");
            self.gen_node(ctx, right, Some(Path::new(node, NodeField::right)))?;
        }
        Ok(())
    }

    /// `TSFunctionType`: `<T>(a: A, ...rest: R) => Ret`.
    ///
    /// `parse_ts_function_or_parenthesized_type` builds it from the `(`
    /// cover (or, with leading `<T>`, from `parse_type_annotation_ts`'s
    /// `less` arm). `return_type` comes from a full
    /// `parse_type_annotation_ts(None)` that runs to the end of the type, so
    /// it prints bare — and that greediness is exactly why the *function
    /// type itself* must be parenthesized in any narrowed position (see this
    /// module's precedence discussion).
    pub(crate) fn gen_ts_function_type<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSFunctionType<'gc>,
    ) -> Result<(), GenJsError> {
        let TSFunctionType {
            metadata: _,
            params,
            return_type,
            type_parameters,
        } = inner;
        self.visit_ts_function_type_tail(ctx, node, *params, return_type, *type_parameters)
    }

    /// `TSConstructorType`: `new <T>(a: A) => Ret`.
    ///
    /// `parse_type_annotation_ts`'s `rw_new` arm, which then shares
    /// `parse_ts_function_or_parenthesized_type` with `TSFunctionType`
    /// (`IsConstructorType::Yes`). The trailing space after `new` is not
    /// strictly required (only `<` or `(` can follow) but is printed
    /// unconditionally, matching how every other keyword prefix in this
    /// crate is emitted.
    pub(crate) fn gen_ts_constructor_type<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSConstructorType<'gc>,
    ) -> Result<(), GenJsError> {
        let TSConstructorType {
            metadata: _,
            params,
            return_type,
            type_parameters,
        } = inner;
        out!(self, "new ");
        self.visit_ts_function_type_tail(ctx, node, *params, return_type, *type_parameters)
    }

    /// The shared `[<T>] (params) => Ret` tail of `TSFunctionType` and
    /// `TSConstructorType`.
    fn visit_ts_function_type_tail<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        params: NodeList<'gc>,
        return_type: &'gc Node<'gc>,
        type_parameters: Option<&'gc Node<'gc>>,
    ) -> Result<(), GenJsError> {
        if let Some(type_parameters) = type_parameters {
            self.gen_node(
                ctx,
                type_parameters,
                Some(Path::new(node, NodeField::type_parameters)),
            )?;
        }
        self.visit_ts_params(ctx, node, params, NodeField::params)?;
        self.space(ForceSpace::No);
        self.space_before_equals("=>");
        out!(self, "=>");
        self.space(ForceSpace::No);
        self.print_child(
            ctx,
            Some(return_type),
            Path::new(node, NodeField::return_type),
            ChildPos::Anywhere,
        )
    }

    /// `TSTypePredicate`: `x is T`.
    ///
    /// `parse_type_annotation_ts`'s leading-identifier backtrack: an
    /// `Identifier` followed by the contextual `is`, then a *wrapped*
    /// (`TSTypeAnnotation`) full type. Both children print bare — the type
    /// runs to the end of the annotation. The spaces around `is` are
    /// `ForceSpace::Yes`: it is a bare word, so `xisT` would lex as one
    /// identifier in compact mode.
    pub(crate) fn gen_ts_type_predicate<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSTypePredicate<'gc>,
    ) -> Result<(), GenJsError> {
        let TSTypePredicate {
            metadata: _,
            parameter_name,
            type_annotation,
        } = inner;
        self.gen_node(
            ctx,
            parameter_name,
            Some(Path::new(node, NodeField::parameter_name)),
        )?;
        self.space(ForceSpace::Yes);
        out!(self, "is");
        self.space(ForceSpace::Yes);
        self.print_child(
            ctx,
            Some(*type_annotation),
            Path::new(node, NodeField::type_annotation),
            ChildPos::Anywhere,
        )
    }

    /// `TSTupleType`: `[A, B]`.
    ///
    /// `parse_ts_tuple_type` parses each element with a full
    /// `parse_type_annotation_ts(None)` between the brackets and commas, so
    /// elements print bare.
    pub(crate) fn gen_ts_tuple_type<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSTupleType<'gc>,
    ) -> Result<(), GenJsError> {
        let TSTupleType {
            metadata: _,
            element_types,
        } = inner;
        out!(self, "[");
        for (i, ty) in element_types.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            self.print_child(
                ctx,
                Some(ty),
                Path::new(node, NodeField::element_types),
                ChildPos::Anywhere,
            )?;
        }
        out!(self, "]");
        Ok(())
    }

    /// `TSUnionType`: `A | B | C`.
    ///
    /// `parse_ts_union_type` builds the list out of
    /// `parse_ts_intersection_type` results, so each member sits at
    /// **intersection** tier — narrower than full `Type`. `ChildPos::Anywhere`
    /// (a member is neither the left nor the right end of the operator) makes
    /// an equal-precedence nested `TSUnionType` — reachable only through
    /// explicit source parens, `A | (B | C)` — keep them.
    pub(crate) fn gen_ts_union_type<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSUnionType<'gc>,
    ) -> Result<(), GenJsError> {
        let TSUnionType { metadata: _, types } = inner;
        for (i, ty) in types.iter().enumerate() {
            if i > 0 {
                self.space(ForceSpace::No);
                out!(self, "|");
                self.space(ForceSpace::No);
            }
            self.print_child(
                ctx,
                Some(ty),
                Path::new(node, NodeField::types),
                ChildPos::Anywhere,
            )?;
        }
        Ok(())
    }

    /// `TSIntersectionType`: `A & B & C`.
    ///
    /// `parse_ts_intersection_type` builds the list out of
    /// `parse_ts_postfix_type` results, so each member sits at **postfix**
    /// tier — a union member, a function type or a conditional type reached
    /// this position only through explicit parens and gets them back.
    pub(crate) fn gen_ts_intersection_type<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSIntersectionType<'gc>,
    ) -> Result<(), GenJsError> {
        let TSIntersectionType { metadata: _, types } = inner;
        for (i, ty) in types.iter().enumerate() {
            if i > 0 {
                self.space(ForceSpace::No);
                out!(self, "&");
                self.space(ForceSpace::No);
            }
            self.print_child(
                ctx,
                Some(ty),
                Path::new(node, NodeField::types),
                ChildPos::Anywhere,
            )?;
        }
        Ok(())
    }

    /// `TSTypeQuery`: `typeof x.y.z`.
    ///
    /// `parse_ts_type_query` reads `typeof` and then its own dotted-name
    /// loop (it does *not* recurse into the type grammar), so `expr_name` is
    /// always an `Identifier` or a `TSQualifiedName` and prints bare. The
    /// space after `typeof` is `ForceSpace::Yes` — `typeofx` would lex as
    /// one identifier.
    pub(crate) fn gen_ts_type_query<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSTypeQuery<'gc>,
    ) -> Result<(), GenJsError> {
        let TSTypeQuery {
            metadata: _,
            expr_name,
        } = inner;
        out!(self, "typeof");
        self.space(ForceSpace::Yes);
        self.gen_node(ctx, expr_name, Some(Path::new(node, NodeField::expr_name)))
    }

    /// `TSConditionalType`: `Check extends Extends ? True : False`.
    ///
    /// `parse_type_annotation_ts`'s trailing `extends` clause. `check_type`
    /// is whatever the *first* half of that function produced — a
    /// `parse_ts_union_type` result, or a constructor/generic-function/
    /// predicate — so it is a **union-tier** position and needs
    /// `print_child`. The other three fields are each a fresh full
    /// `parse_type_annotation_ts(None)` and print bare.
    ///
    /// A function type in `check_type` is the sharp case: printed bare,
    /// `(a: A) => B extends C ? D : E` gives the function type's *return
    /// type* the `extends` clause (`parse_type_annotation_ts` for the return
    /// type consumes it), producing a completely different tree. It gets
    /// parens because `TS_FUNCTION_TYPE` < `TS_UNION_TYPE`.
    pub(crate) fn gen_ts_conditional_type<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSConditionalType<'gc>,
    ) -> Result<(), GenJsError> {
        let TSConditionalType {
            metadata: _,
            check_type,
            extends_type,
            true_type,
            false_type,
        } = inner;
        self.print_child(
            ctx,
            Some(*check_type),
            Path::new(node, NodeField::check_type),
            ChildPos::Left,
        )?;
        self.space(ForceSpace::Yes);
        out!(self, "extends");
        self.space(ForceSpace::Yes);
        self.print_child(
            ctx,
            Some(*extends_type),
            Path::new(node, NodeField::extends_type),
            ChildPos::Anywhere,
        )?;
        self.space(ForceSpace::No);
        out!(self, "?");
        self.space(ForceSpace::No);
        self.print_child(
            ctx,
            Some(*true_type),
            Path::new(node, NodeField::true_type),
            ChildPos::Anywhere,
        )?;
        self.space(ForceSpace::No);
        out!(self, ":");
        self.space(ForceSpace::No);
        self.print_child(
            ctx,
            Some(*false_type),
            Path::new(node, NodeField::false_type),
            ChildPos::Anywhere,
        )
    }

    /// `TSTypeLiteral`: `{ a: A; m(): B; [k: string]: C; (): D }`.
    ///
    /// `parse_ts_object_type`.
    pub(crate) fn gen_ts_type_literal<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSTypeLiteral<'gc>,
    ) -> Result<(), GenJsError> {
        let TSTypeLiteral {
            metadata: _,
            members,
        } = inner;
        self.visit_ts_object_members(ctx, node, *members, NodeField::members)
    }

    // -----------------------------------------------------------------------
    // Object-type members (shared by `TSTypeLiteral` and `TSInterfaceBody`).
    // -----------------------------------------------------------------------

    /// `TSPropertySignature`: `[static] [export] [readonly] key[?][: T][= init]`
    /// (or `[key]` when `computed`).
    ///
    /// `parse_ts_object_type_member`'s property paths. Note that
    /// `readonly`/`static`/`export`/`initializer` are **never set by our
    /// parser**: that function hard-codes `let readonly = false; let
    /// is_static = false; let is_export = false;` behind a `// TODO: Parse
    /// modifiers.` and `let init: Option<…> = None;` behind a `// TODO:
    /// Parse initializer.`, both faithful to the C++ (`ts.cpp:1250-1258`).
    /// They are printed anyway, in TypeScript's own modifier order, so a
    /// hand-built or JSON-deserialized tree does not silently lose them —
    /// but that output is **not** reparsable by this parser today, and no
    /// test below exercises it (there is no way to build such a node by
    /// parsing).
    ///
    /// `key` prints bare: a non-computed key is an `Identifier` token, and a
    /// computed one is a full `parse_assignment_expression` between `[` and
    /// `]`.
    pub(crate) fn gen_ts_property_signature<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSPropertySignature<'gc>,
    ) -> Result<(), GenJsError> {
        let TSPropertySignature {
            metadata: _,
            key,
            type_annotation,
            initializer,
            optional,
            computed,
            readonly,
            r#static,
            export,
        } = inner;
        if r#static.get() {
            out!(self, "static ");
        }
        if export.get() {
            out!(self, "export ");
        }
        if readonly.get() {
            out!(self, "readonly ");
        }
        if computed.get() {
            out!(self, "[");
        }
        self.gen_node(ctx, key, Some(Path::new(node, NodeField::key)))?;
        if computed.get() {
            out!(self, "]");
        }
        if optional.get() {
            out!(self, "?");
        }
        self.print_ts_colon_type(ctx, node, *type_annotation, NodeField::type_annotation)?;
        if let Some(initializer) = initializer {
            self.space(ForceSpace::No);
            self.space_before_equals("=");
            out!(self, "=");
            self.space(ForceSpace::No);
            self.gen_node(
                ctx,
                initializer,
                Some(Path::new(node, NodeField::initializer)),
            )?;
        }
        Ok(())
    }

    /// `TSMethodSignature`: `key(params)[: Ret]` (or `[key](…)` when
    /// `computed`).
    ///
    /// `parse_ts_object_type_member`'s `l_paren`-after-key path.
    /// `return_type` is a `TSTypeAnnotation`-wrapped full type there, so it
    /// prints bare after the `:`.
    pub(crate) fn gen_ts_method_signature<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSMethodSignature<'gc>,
    ) -> Result<(), GenJsError> {
        let TSMethodSignature {
            metadata: _,
            key,
            params,
            return_type,
            computed,
        } = inner;
        if computed.get() {
            out!(self, "[");
        }
        self.gen_node(ctx, key, Some(Path::new(node, NodeField::key)))?;
        if computed.get() {
            out!(self, "]");
        }
        self.visit_ts_params(ctx, node, *params, NodeField::params)?;
        self.print_ts_colon_type(ctx, node, *return_type, NodeField::return_type)
    }

    /// `TSIndexSignature`: `[k: string]: T`.
    ///
    /// `parse_ts_index_signature`: the bracketed list holds
    /// `parse_binding_identifier` results (each carrying its own `: T`), and
    /// the trailing annotation is a `TSTypeAnnotation`-wrapped full type.
    pub(crate) fn gen_ts_index_signature<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSIndexSignature<'gc>,
    ) -> Result<(), GenJsError> {
        let TSIndexSignature {
            metadata: _,
            parameters,
            type_annotation,
        } = inner;
        out!(self, "[");
        for (i, param) in parameters.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            self.gen_node(ctx, param, Some(Path::new(node, NodeField::parameters)))?;
        }
        out!(self, "]");
        self.print_ts_colon_type(ctx, node, *type_annotation, NodeField::type_annotation)
    }

    /// `TSCallSignatureDeclaration`: `(params)[: Ret]`.
    ///
    /// `parse_ts_object_type_member`'s leading-`l_paren` path. Unlike
    /// `TSMethodSignature`, its `return_type` is an *unwrapped* full type
    /// (`parse_type_annotation_ts(None)`), which prints identically.
    pub(crate) fn gen_ts_call_signature_declaration<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSCallSignatureDeclaration<'gc>,
    ) -> Result<(), GenJsError> {
        let TSCallSignatureDeclaration {
            metadata: _,
            params,
            return_type,
        } = inner;
        self.visit_ts_params(ctx, node, *params, NodeField::params)?;
        self.print_ts_colon_type(ctx, node, *return_type, NodeField::return_type)
    }

    // -----------------------------------------------------------------------
    // Parameters, modifiers, and type parameters.
    // -----------------------------------------------------------------------

    /// `TSParameterProperty`: `[public|private|protected] [static] [export]
    /// [readonly] param`.
    ///
    /// `parse_ts_function_type_param`'s modifier loop. That loop re-runs from
    /// the top after each accepted modifier, so it accepts them in *any*
    /// order; this prints TypeScript's canonical order (accessibility first,
    /// `readonly` last), which the loop accepts — traced in
    /// `task-13-report.md`.
    pub(crate) fn gen_ts_parameter_property<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSParameterProperty<'gc>,
    ) -> Result<(), GenJsError> {
        let TSParameterProperty {
            metadata: _,
            parameter,
            accessibility,
            readonly,
            r#static,
            export,
        } = inner;
        self.print_ts_modifiers_accessibility(ctx, accessibility.get());
        if r#static.get() {
            out!(self, "static ");
        }
        if export.get() {
            out!(self, "export ");
        }
        self.print_ts_modifiers_readonly(readonly.get());
        self.gen_node(ctx, parameter, Some(Path::new(node, NodeField::parameter)))
    }

    /// `TSModifiers`: the `accessibility`/`readonly` pair a TS class member
    /// carries.
    ///
    /// Reaching this arm through `gen_node` means the node was printed on
    /// its own rather than by its owning `ClassProperty`/
    /// `ClassPrivateProperty` — which is not how our parser's trees are
    /// shaped, since those two arms interleave the halves around `static`
    /// (see this module's doc comment). Printing both halves in the parser's
    /// accepted order is the best a standalone rendering can do.
    pub(crate) fn gen_ts_modifiers(
        &mut self,
        ctx: &GCLock<'_, '_>,
        inner: &TSModifiers<'_>,
    ) -> Result<(), GenJsError> {
        let TSModifiers {
            metadata: _,
            accessibility,
            readonly,
        } = inner;
        self.print_ts_modifiers_accessibility(ctx, accessibility.get());
        self.print_ts_modifiers_readonly(readonly.get());
        Ok(())
    }

    /// `<A, B>` — a TypeScript type-*argument* list
    /// (`TSTypeParameterInstantiation`, `parse_ts_type_arguments`).
    ///
    /// Task 13 fix round 1. This does not reuse Task 10's shared
    /// [`GenJS::gen_type_parameter_list`] (which `TSTypeParameterDeclaration`
    /// still does) because each element here is a **full type**
    /// (`parse_type_annotation_ts(None)`) and so has to go through
    /// `print_child` for the intruder rule — `type T = A<({ b: B })>;` put an
    /// `ObjectPattern` in this list and regenerated as `A<{b: B}>`, which
    /// reparses as a `TSTypeLiteral`. Routing the *shared* helper through
    /// `print_child` instead was rejected: its other three callers are the
    /// Flow `TypeParameterDeclaration`/`TypeParameterInstantiation` and
    /// `TSTypeParameterDeclaration`, whose elements are not full types, and
    /// a Flow `Array<() => void>` would have gained a redundant
    /// `Array<(() => void)>` (`FunctionTypeAnnotation` is `ALWAYS_PAREN`).
    pub(crate) fn gen_ts_type_argument_list<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        params: NodeList<'gc>,
    ) -> Result<(), GenJsError> {
        out!(self, "<");
        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            self.print_child(
                ctx,
                Some(param),
                Path::new(node, NodeField::params),
                ChildPos::Anywhere,
            )?;
        }
        out!(self, ">");
        Ok(())
    }

    /// `TSTypeParameter`: `T`, `T extends C`, `T = D`, `T extends C = D`.
    ///
    /// `parse_ts_type_parameter`. Both `constraint` and `default` are full
    /// `parse_type_annotation_ts(None)` positions and print bare; the list
    /// separator is `,`, which no type production consumes.
    pub(crate) fn gen_ts_type_parameter<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSTypeParameter<'gc>,
    ) -> Result<(), GenJsError> {
        let TSTypeParameter {
            metadata: _,
            name,
            constraint,
            default,
        } = inner;
        self.gen_node(ctx, name, Some(Path::new(node, NodeField::name)))?;
        if let Some(constraint) = constraint {
            self.space(ForceSpace::Yes);
            out!(self, "extends");
            self.space(ForceSpace::Yes);
            self.print_child(
                ctx,
                Some(constraint),
                Path::new(node, NodeField::constraint),
                ChildPos::Anywhere,
            )?;
        }
        if let Some(default) = default {
            self.space(ForceSpace::No);
            self.space_before_equals("=");
            out!(self, "=");
            self.space(ForceSpace::No);
            self.print_child(
                ctx,
                Some(default),
                Path::new(node, NodeField::default),
                ChildPos::Anywhere,
            )?;
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Expressions.
    // -----------------------------------------------------------------------

    /// `TSAsExpression`: `expr as T`.
    ///
    /// `parse_binary_expression`'s `as_operator` branch under `parse_ts()`
    /// (`crates/parser/src/js/expressions.rs`'s `make_as_node`). The left
    /// operand is an ordinary expression at the `as` operator's own
    /// precedence, so it goes through `print_child`; the right operand is a
    /// full `parse_type_annotation(None, AllowAnonFunctionType::Yes)` and
    /// prints bare. See this module's doc comment for why the *whole*
    /// as-expression now sits below every binary operator in the precedence
    /// table.
    pub(crate) fn gen_ts_as_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSAsExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let TSAsExpression {
            metadata: _,
            expression,
            type_annotation,
        } = inner;
        self.print_child(
            ctx,
            Some(*expression),
            Path::new(node, NodeField::expression),
            ChildPos::Left,
        )?;
        self.space(ForceSpace::Yes);
        out!(self, "as");
        self.space(ForceSpace::Yes);
        self.print_child(
            ctx,
            Some(*type_annotation),
            Path::new(node, NodeField::type_annotation),
            ChildPos::Anywhere,
        )
    }

    /// `TSTypeAssertion`: `<T>expr`.
    ///
    /// `parse_unary_expression`'s `less` arm
    /// (`crates/parser/src/js/expressions.rs`), gated on `parse_ts() &&
    /// !parse_jsx()`. The type between the angle brackets is a full
    /// `parse_type_annotation_ts(None)` and prints bare (the closing `>` is
    /// eaten separately, in `AllowRegExp` context). The operand is a
    /// `parse_unary_expression` — a **narrowed** position — so it goes
    /// through `print_child` at `ChildPos::Right` against this kind's own
    /// `UNARY`/`Assoc::Rtl` entry: that keeps a nested `<T><U>x` and a
    /// `<T>-x` bare while parenthesizing anything looser, e.g.
    /// `<T>(a + b)`, `<T>(a, b)`, `<T>(a = b)`.
    pub(crate) fn gen_ts_type_assertion<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSTypeAssertion<'gc>,
    ) -> Result<(), GenJsError> {
        let TSTypeAssertion {
            metadata: _,
            type_annotation,
            expression,
        } = inner;
        out!(self, "<");
        self.print_child(
            ctx,
            Some(*type_annotation),
            Path::new(node, NodeField::type_annotation),
            ChildPos::Anywhere,
        )?;
        out!(self, ">");
        self.print_child(
            ctx,
            Some(*expression),
            Path::new(node, NodeField::expression),
            ChildPos::Right,
        )
    }

    // -----------------------------------------------------------------------
    // Declarations.
    // -----------------------------------------------------------------------

    /// `TSTypeAliasDeclaration`: `type Id<T> = Ty` (the `;` is added by
    /// `visit_stmt_in_block`, as for every other declaration in this crate).
    ///
    /// `parse_ts_type_alias_declaration`.
    pub(crate) fn gen_ts_type_alias_declaration<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSTypeAliasDeclaration<'gc>,
    ) -> Result<(), GenJsError> {
        let TSTypeAliasDeclaration {
            metadata: _,
            id,
            type_parameters,
            type_annotation,
        } = inner;
        out!(self, "type ");
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        if let Some(type_parameters) = type_parameters {
            self.gen_node(
                ctx,
                type_parameters,
                Some(Path::new(node, NodeField::type_parameters)),
            )?;
        }
        if self.pretty() == Pretty::Yes {
            out!(self, " = ");
        } else {
            self.space_before_equals("=");
            out!(self, "=");
        }
        self.print_child(
            ctx,
            Some(*type_annotation),
            Path::new(node, NodeField::type_annotation),
            ChildPos::Anywhere,
        )
    }

    /// `TSInterfaceDeclaration`: `interface Id<T> extends A, B<X> { … }`.
    ///
    /// `parse_ts_interface_declaration`. Reachable both as a statement and —
    /// via `parse_ts_primary_type`'s `rw_interface` arm — as a *type*, which
    /// is why it is classified as a primary type in `precedence.rs` rather
    /// than left unclassified.
    pub(crate) fn gen_ts_interface_declaration<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSInterfaceDeclaration<'gc>,
    ) -> Result<(), GenJsError> {
        let TSInterfaceDeclaration {
            metadata: _,
            id,
            body,
            extends,
            type_parameters,
        } = inner;
        out!(self, "interface ");
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        if let Some(type_parameters) = type_parameters {
            self.gen_node(
                ctx,
                type_parameters,
                Some(Path::new(node, NodeField::type_parameters)),
            )?;
        }
        if !extends.is_empty() {
            out!(self, " extends ");
            for (i, extend) in extends.iter().enumerate() {
                if i > 0 {
                    self.comma();
                }
                self.gen_node(ctx, extend, Some(Path::new(node, NodeField::extends)))?;
            }
        }
        self.space(ForceSpace::No);
        self.gen_node(ctx, body, Some(Path::new(node, NodeField::body)))
    }

    /// `TSInterfaceHeritage`: one `extends` entry, `Expr<Args>`.
    ///
    /// `parse_ts_interface_declaration`'s heritage loop moves the parsed
    /// `TSTypeReference`'s own type arguments into this node's
    /// `type_parameters` and rebuilds the reference without them, so the two
    /// fields print back-to-back to recover the original `A<X>` spelling.
    pub(crate) fn gen_ts_interface_heritage<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSInterfaceHeritage<'gc>,
    ) -> Result<(), GenJsError> {
        let TSInterfaceHeritage {
            metadata: _,
            expression,
            type_parameters,
        } = inner;
        self.gen_node(
            ctx,
            expression,
            Some(Path::new(node, NodeField::expression)),
        )?;
        if let Some(type_parameters) = type_parameters {
            self.gen_node(
                ctx,
                type_parameters,
                Some(Path::new(node, NodeField::type_parameters)),
            )?;
        }
        Ok(())
    }

    /// `TSInterfaceBody`: `{ member; member; }`.
    ///
    /// `parse_ts_interface_declaration`'s body loop, whose members are the
    /// same `parse_ts_object_type_member` results a `TSTypeLiteral` holds.
    pub(crate) fn gen_ts_interface_body<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSInterfaceBody<'gc>,
    ) -> Result<(), GenJsError> {
        let TSInterfaceBody { metadata: _, body } = inner;
        self.visit_ts_object_members(ctx, node, *body, NodeField::body)
    }

    /// `TSEnumDeclaration`: `enum Id { A, B = 2 }`.
    ///
    /// `parse_ts_enum_declaration`. Members are `,`-separated (unlike an
    /// object type's `;`) — that loop only accepts `comma`.
    pub(crate) fn gen_ts_enum_declaration<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSEnumDeclaration<'gc>,
    ) -> Result<(), GenJsError> {
        let TSEnumDeclaration {
            metadata: _,
            id,
            members,
        } = inner;
        out!(self, "enum ");
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        self.space(ForceSpace::No);
        if members.is_empty() {
            out!(self, "{{}}");
            return Ok(());
        }
        out!(self, "{{");
        self.inc_indent();
        self.newline();
        for (i, member) in members.iter().enumerate() {
            if i > 0 {
                out!(self, ",");
                self.newline();
            }
            self.gen_node(ctx, member, Some(Path::new(node, NodeField::members)))?;
        }
        self.dec_indent();
        self.newline();
        out!(self, "}}");
        Ok(())
    }

    /// `TSEnumMember`: `Name` or `Name = init`.
    ///
    /// `parse_ts_enum_member`: the initializer is a
    /// `parse_assignment_expression`, and members are separated by `,`, so a
    /// `SequenceExpression` initializer (only reachable through explicit
    /// source parens) must keep its parens or its comma would end the
    /// member. That is exactly `print_comma_expression`'s rule
    /// (parenthesize when the child's precedence is `<= SEQ`), so it is used
    /// here rather than a bare `gen_node`.
    pub(crate) fn gen_ts_enum_member<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSEnumMember<'gc>,
    ) -> Result<(), GenJsError> {
        let TSEnumMember {
            metadata: _,
            id,
            initializer,
        } = inner;
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        if let Some(initializer) = initializer {
            self.space(ForceSpace::No);
            self.space_before_equals("=");
            out!(self, "=");
            self.space(ForceSpace::No);
            self.print_comma_expression(ctx, initializer, Path::new(node, NodeField::initializer))?;
        }
        Ok(())
    }

    /// `TSModuleDeclaration`: `namespace Id { … }`.
    ///
    /// **No construction site in `crates/parser`** — see this module's doc
    /// comment. `namespace` is the spelling our parser can read back (into a
    /// `TSModuleMember`), so it is what this prints; the `body` is expected
    /// to be a `TSModuleBlock`, and anything else prints as itself.
    pub(crate) fn gen_ts_module_declaration<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSModuleDeclaration<'gc>,
    ) -> Result<(), GenJsError> {
        let TSModuleDeclaration {
            metadata: _,
            id,
            body,
        } = inner;
        out!(self, "namespace ");
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        self.space(ForceSpace::No);
        self.gen_node(ctx, body, Some(Path::new(node, NodeField::body)))
    }

    /// `TSModuleBlock`: `{ statements }`.
    ///
    /// `parse_ts_namespace_declaration`'s body: a plain statement-list-item
    /// loop (`AllowImportExport::Yes`), so this prints exactly like a
    /// `BlockStatement` — `visit_stmt_list` supplies each statement's own
    /// `;`.
    pub(crate) fn gen_ts_module_block<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSModuleBlock<'gc>,
    ) -> Result<(), GenJsError> {
        let TSModuleBlock { metadata: _, body } = inner;
        if body.is_empty() {
            out!(self, "{{}}");
            return Ok(());
        }
        out!(self, "{{");
        self.inc_indent();
        self.newline();
        self.visit_stmt_list(ctx, *body, Path::new(node, NodeField::body))?;
        self.dec_indent();
        self.newline();
        out!(self, "}}");
        Ok(())
    }

    /// `TSModuleMember`: `namespace Id.Sub { … }` — what our parser actually
    /// builds for a `namespace` declaration
    /// (`parse_ts_namespace_declaration`).
    ///
    /// `id` is a `parse_ts_qualified_name` result (an `Identifier` or a
    /// dotted `TSQualifiedName`); `initializer` is the `TSModuleBlock` body,
    /// `Option` in the AST but always `Some` from that function.
    pub(crate) fn gen_ts_module_member<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &TSModuleMember<'gc>,
    ) -> Result<(), GenJsError> {
        let TSModuleMember {
            metadata: _,
            id,
            initializer,
        } = inner;
        out!(self, "namespace ");
        self.gen_node(ctx, id, Some(Path::new(node, NodeField::id)))?;
        if let Some(initializer) = initializer {
            self.space(ForceSpace::No);
            self.gen_node(
                ctx,
                initializer,
                Some(Path::new(node, NodeField::initializer)),
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use hermes_ast::node::{Identifier, Program};
    use hermes_ast::node_child::NodeMetadata;
    use hermes_parser::{parse, ParseFlags};

    use super::*;
    use crate::{Opt, Pretty};

    /// Generate just `node` (not a whole program) and decode the result as a
    /// `String` — the same helper `arms/flow_type.rs`'s tests use, and for
    /// the same reason: the nodes exercised below cannot be reached through a
    /// full `generate()` call on parsed source.
    fn gen_node_to_string<'gc>(
        gc: &GCLock<'static, '_>,
        node: &'gc Node<'gc>,
        pretty: Pretty,
    ) -> String {
        let mut sink = Vec::new();
        {
            let mut gen_js = GenJS::for_test(
                &mut sink,
                Opt {
                    pretty,
                    ..Opt::default()
                },
            );
            gen_js.gen_node(gc, node, None).expect("node generates");
        }
        String::from_utf8(sink).expect("generator output is always valid UTF-8 (spec §5)")
    }

    /// TypeScript flags — `parse_ts` is a separate dialect from `parse_flow`.
    fn ts_flags() -> ParseFlags {
        ParseFlags {
            parse_ts: true,
            ..Default::default()
        }
    }

    /// `TSModuleDeclaration` is the one TS kind our parser never builds (see
    /// the module doc comment), so its arm cannot be covered by a
    /// parse -> generate -> reparse round trip the way the other 45 are.
    /// This hand-builds one out of a real `namespace N { let x = 1; }`'s own
    /// `id` and `TSModuleBlock` body — the very parts a
    /// `TSModuleDeclaration` would hold — and pins the printed text.
    ///
    /// The text is the same `namespace N { … }` spelling our parser reads
    /// back (into a `TSModuleMember`), which is the strongest correspondence
    /// available for a kind with no production: the round trip cannot be
    /// kind-preserving, because the kind has no source syntax of its own in
    /// this parser.
    #[test]
    fn ts_module_declaration_prints_a_namespace_with_its_block_body() {
        let mut parsed =
            parse("namespace N { let x = 1; }", ts_flags()).expect("namespace source must parse");
        parsed.with_program(|gc, node| {
            let Node::Program(Program { body, .. }) = node else {
                panic!("root is not a Program");
            };
            let stmt = body.iter().next().expect("has a statement");
            let Node::TSModuleMember(TSModuleMember {
                metadata,
                id,
                initializer,
            }) = stmt
            else {
                panic!("a namespace declaration parses to a TSModuleMember: {stmt:?}");
            };
            let block = initializer.expect("parse_ts_namespace_declaration always sets a body");
            assert!(
                matches!(block, Node::TSModuleBlock(_)),
                "the body is a TSModuleBlock: {block:?}"
            );
            let hand_built = gc.alloc(Node::TSModuleDeclaration(TSModuleDeclaration::new(
                NodeMetadata::new(metadata.range()),
                id,
                block,
            )));
            assert_eq!(
                gen_node_to_string(gc, hand_built, Pretty::No),
                "namespace N{let x=1;}"
            );
            assert_eq!(
                gen_node_to_string(gc, hand_built, Pretty::Yes),
                "namespace N {\n  let x = 1;\n}"
            );
        });
    }

    /// `TSPropertySignature`'s `readonly`/`static`/`export`/`initializer`
    /// fields are never set by our parser — `parse_ts_object_type_member`
    /// hard-codes them off behind `// TODO: Parse modifiers.` and
    /// `// TODO: Parse initializer.` — so no parsed tree can exercise the
    /// branches that print them, and (until that TODO is done) the output is
    /// not reparsable either. This hand-builds the node to pin that the
    /// fields are *printed* rather than silently dropped, in TypeScript's own
    /// modifier order.
    ///
    /// Do not read this as a round-trip claim: it is deliberately an
    /// output-text assertion only. `parse("type T = { static export readonly
    /// a: A = 1 };", ts_flags())` fails today, which is exactly why this test
    /// hand-builds instead.
    #[test]
    fn ts_property_signature_prints_the_modifiers_our_parser_cannot_yet_set() {
        let mut parsed = parse("type T = { a: A };", ts_flags()).expect("source must parse");
        parsed.with_program(|gc, node| {
            let Node::Program(Program { body, .. }) = node else {
                panic!("root is not a Program");
            };
            let stmt = body.iter().next().expect("has a statement");
            let Node::TSTypeAliasDeclaration(TSTypeAliasDeclaration {
                metadata,
                id: _,
                type_parameters: _,
                type_annotation,
            }) = stmt
            else {
                panic!("not a TSTypeAliasDeclaration: {stmt:?}");
            };
            let Node::TSTypeLiteral(TSTypeLiteral {
                metadata: _,
                members,
            }) = type_annotation
            else {
                panic!("not a TSTypeLiteral: {type_annotation:?}");
            };
            let Node::TSPropertySignature(TSPropertySignature {
                metadata: _,
                key,
                type_annotation: prop_type,
                initializer: _,
                optional: _,
                computed: _,
                readonly: _,
                r#static: _,
                export: _,
            }) = members.iter().next().expect("has a member")
            else {
                panic!("member is not a TSPropertySignature");
            };
            let one = gc.alloc(Node::Identifier(Identifier::new(
                NodeMetadata::new(metadata.range()),
                gc.atom_bytes(&b"init"[..]),
                None,
                false,
            )));
            let hand_built = gc.alloc(Node::TSPropertySignature(TSPropertySignature::new(
                NodeMetadata::new(metadata.range()),
                key,
                *prop_type,
                Some(one),
                true,
                false,
                true,
                true,
                true,
            )));
            assert_eq!(
                gen_node_to_string(gc, hand_built, Pretty::No),
                "static export readonly a?:A=init"
            );
        });
    }
}
