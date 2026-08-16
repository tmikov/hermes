/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! ES expressions: `SequenceExpression` through `Property`, plus the
//! `visit_props` helper.
//!
//! Ported from juno `gen_js.rs:868-1259` (`SequenceExpression` through
//! `BinaryExpression`), `gen_js.rs:1267-1298` (`ConditionalExpression`),
//! `gen_js.rs:1442-1552` (`Property`, `LogicalExpression`), and
//! `gen_js.rs:3353-3364` (`visit_props`). This is the plan's Task 5.
//!
//! # Two known juno bugs, already fixed elsewhere — not re-litigated here
//!
//! `precedence.rs` (Task 3) already fixed juno's `**` right-associativity
//! bug in `get_precedence`, and `arms/literal.rs` (Task 4) already fixed
//! juno's `BigIntLiteral` arm dropping the `n` suffix. Neither touches this
//! module's line ranges, so nothing here re-does that work; they're
//! mentioned only because this module's own arms were checked against the
//! same "assume juno has bugs" standard — see the deviations below for what
//! that check actually found here.
//!
//! # Adaptations specific to this module
//!
//! **`ImportExpression`'s second field is `options`, not `attributes`.**
//! juno's AST (`juno_ast/src/def.rs:190-193`) names it `attributes`; our
//! `ImportExpression` struct (`crates/ast/src/node.rs`) names the
//! structurally identical `Option<&Node>` field `options` — a field-naming
//! divergence between the two ESTree definitions, not a semantic one, so
//! `NodeField::options` is what [`GenJS::gen_import_expression`] builds its
//! `Path` from.
//!
//! **Operator/kind fields print through the classify-then-`as_str` path.**
//! `precedence.rs`'s four operator enums (`BinaryExpressionOperator` and its
//! three siblings) existed only for `get_precedence`'s classification before
//! this task; this module is the first to also need the *printed* spelling
//! back, so each grew an `as_str()` (`precedence.rs`, right after its
//! `from_label`) mirroring juno's typed enum's own `as_str()`
//! (`juno_support/src/str_enum.rs:49-53`). `Property`'s `kind` field
//! ("init"/"get"/"set") gets the same treatment via a local
//! [`PropertyKind`], defined in this module rather than `precedence.rs`
//! since — unlike the other four — nothing in `get_precedence`/`need_parens`
//! ever needs to classify it; `Property` never appears where
//! parenthesization is a question, so `precedence.rs`'s own `get_precedence`
//! match has no `Property` arm at all.
//!
//! **`ForceSpace` becomes load-bearing, not just plumbing.** Task 4's
//! `Identifier` arm was `GenJS::space`'s only caller, always with
//! `ForceSpace::No`. `BinaryExpression`'s and `UnaryExpression`'s arms below
//! are the first to pass `ForceSpace::Yes` — for `in`/`instanceof`/`delete`/
//! `void`/`typeof`, omitting the space even in compact mode would merge the
//! operator into the next token (`a in b` → `ainb`, reparsing as a single
//! identifier), unlike `a+b`, which stays unambiguous without one. `gen.rs`
//! now takes `ForceSpace` directly rather than `bool` (see its own doc
//! comment) so call sites read `self.space(ForceSpace::Yes)`, not
//! `self.space(true)`.
//!
//! # `Property`'s forward reference to `visit_func_params_body`
//!
//! juno's `Property` arm (`gen_js.rs:1496-1522`) prints a method/getter/
//! setter's parameter list and body through `visit_func_params_body`
//! (`gen_js.rs:3365-3399`), a helper the plan's Task 7 brief also lists
//! under "Produces" (for `FunctionExpression`/`FunctionDeclaration`/
//! `ArrowFunctionExpression`). `Property` is explicitly this task's own
//! (Task 5's "Produces" list), and its method-shaped branch is not
//! functional without that helper, so [`GenJS::visit_func_params_body`] is
//! implemented here — porting exactly `gen_js.rs:3365-3399`, not
//! `visit_func_type_params` (`gen_js.rs:3401-3452`, unrelated and still
//! Task 7's alone) — rather than leaving `Property` half-working until Task
//! 7 lands. It is a `pub(crate)` method on `GenJS` like every other arm in
//! this crate, so Task 7's `arms/func.rs` calls it the same way it would
//! have if it had defined it there; nothing needs to move. One adaptation
//! beyond the field-access mechanics every arm here has: juno's per-element
//! `Path` for each parameter uses `NodeField::param` (`gen_js.rs:3383`) —
//! but juno's own `NodeField` enum (`juno_ast/src/field.rs`) is a single
//! deduplicated set of names used across *all* node kinds, and `param` only
//! exists there as `CatchClause`'s field name; `FunctionExpression`'s field
//! is `params` (plural, `def.rs:43`). Since `need_parens`/`get_precedence`
//! never read `Path::field` (only `Path::parent`, see `precedence.rs`'s
//! `need_parens`), this is inert either way today, but it reads as a
//! copy-paste slip rather than a deliberate choice, so this port uses
//! `NodeField::params` — the name that actually matches the field being
//! walked — instead of carrying the mismatch forward.
//!
//! `Property`'s two `_ => unreachable!()` arms (`gen_js.rs:1472`, `1521`,
//! guarding that a method/getter/setter's `value` is a `FunctionExpression`)
//! become `Err(GenJsError::UnsupportedKind(value.kind()))` instead, per spec
//! §4's "never panic on a malformed input tree" rule — the same substitution
//! `arms/literal.rs` already made for `TemplateElement` reached outside
//! `TemplateLiteral`.

use hermes_ast::context::GCLock;
use hermes_ast::node::{
    ArrayExpression, AssignmentExpression, AwaitExpression, BinaryExpression, CallExpression,
    ConditionalExpression, FunctionExpression, ImportExpression, LogicalExpression,
    MemberExpression, NewExpression, Node, NodeField, NumericLiteral, ObjectExpression,
    OptionalCallExpression, OptionalMemberExpression, Property, SequenceExpression, SpreadElement,
    UnaryExpression, UpdateExpression, YieldExpression,
};
use hermes_ast::node_child::{NodeLabel, NodeList};
use hermes_ast::visitor::Path;

use crate::precedence::{
    BinaryExpressionOperator, ChildPos, ForceSpace, LogicalExpressionOperator,
    UnaryExpressionOperator, UpdateExpressionOperator,
};
use crate::{out, GenJS, GenJsError};

// ---------------------------------------------------------------------------
// `PropertyKind`, this module's own operator-shaped classifier (module doc
// comment explains why it lives here rather than `precedence.rs`).
// ---------------------------------------------------------------------------

/// `Property::kind`, classified from its raw spelling.
///
/// Variant set and spellings from juno `juno_ast/src/node_enums.rs:103-108`
/// (`define_str_enum!(PropertyKind, ..., (Init, "init"), (Get, "get"),
/// (Set, "set"))`); confirmed against the bundled parser's own atoms
/// (`crates/parser/src/js/expressions.rs`'s `init_kind`/`get_kind`/
/// `set_kind` are exactly `atom_bytes(b"init"/b"get"/b"set")`).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum PropertyKind {
    /// `init`: an ordinary (non-accessor) property.
    Init,
    /// `get`: a getter.
    Get,
    /// `set`: a setter.
    Set,
}

impl PropertyKind {
    /// Classify `label`, the raw contents of a `Property`'s `kind` field.
    ///
    /// # Errors
    /// `Err(GenJsError::UnknownOperator { .. })` if `label`'s spelling is
    /// none of the 3 above. Reuses `GenJsError::UnknownOperator` — whose
    /// variant doc names `BinaryExpression`/`LogicalExpression`/
    /// `UnaryExpression`/`UpdateExpression`'s `operator` fields specifically
    /// — rather than adding a dedicated error variant: the failure mode is
    /// identical (an enum-shaped field holding a spelling outside its fixed
    /// set, from a hand-built or JSON-deserialized tree per spec §4), and a
    /// `kind: "Property"` payload conveys it precisely enough that a second
    /// variant would only add surface area for one call site.
    fn from_label(gc: &GCLock<'_, '_>, label: NodeLabel) -> Result<Self, GenJsError> {
        Ok(match gc.bytes_str_lossy(label) {
            "init" => Self::Init,
            "get" => Self::Get,
            "set" => Self::Set,
            other => {
                return Err(GenJsError::UnknownOperator {
                    kind: "Property",
                    spelling: other.to_string(),
                })
            }
        })
    }

    /// The canonical spelling, for printing (`get`/`set` before the key;
    /// `init` is never printed — see [`GenJS::gen_property`]).
    fn as_str(self) -> &'static str {
        match self {
            Self::Init => "init",
            Self::Get => "get",
            Self::Set => "set",
        }
    }
}

impl<'s, 'w> GenJS<'s, 'w> {
    /// `SequenceExpression`: `(expr0, expr1, ...)`.
    ///
    /// juno `gen_js.rs:868-889`.
    pub(crate) fn gen_sequence_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &SequenceExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let SequenceExpression {
            metadata: _,
            expressions,
        } = inner;
        out!(self, "(");
        for (i, expr) in expressions.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            self.print_child(
                ctx,
                Some(expr),
                Path::new(node, NodeField::expressions),
                if i == 1 {
                    ChildPos::Left
                } else {
                    ChildPos::Right
                },
            )?;
        }
        out!(self, ")");
        Ok(())
    }

    /// `ObjectExpression`: `{prop0, prop1, ...}`.
    ///
    /// juno `gen_js.rs:891-896`.
    pub(crate) fn gen_object_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ObjectExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let ObjectExpression {
            metadata: _,
            properties,
        } = inner;
        self.visit_props(ctx, *properties, Path::new(node, NodeField::properties))
    }

    /// `ArrayExpression`: `[elem0, elem1, ...]`, with `SpreadElement` and
    /// elided (`Node::Empty`) elements handled specially.
    ///
    /// juno `gen_js.rs:897-925`.
    pub(crate) fn gen_array_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ArrayExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let ArrayExpression {
            metadata: _,
            elements,
            trailing_comma,
        } = inner;
        out!(self, "[");
        for (i, elem) in elements.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            match elem {
                Node::SpreadElement(_) => {
                    self.gen_node(ctx, elem, Some(Path::new(node, NodeField::elements)))?;
                }
                Node::Empty(_) => {}
                _ => {
                    self.print_comma_expression(ctx, elem, Path::new(node, NodeField::elements))?;
                }
            }
        }
        if trailing_comma.get() {
            self.comma();
        }
        out!(self, "]");
        Ok(())
    }

    /// `SpreadElement`: `...argument`.
    ///
    /// juno `gen_js.rs:927-933`.
    pub(crate) fn gen_spread_element<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &SpreadElement<'gc>,
    ) -> Result<(), GenJsError> {
        let SpreadElement {
            metadata: _,
            argument,
        } = inner;
        out!(self, "...");
        self.gen_node(ctx, argument, Some(Path::new(node, NodeField::argument)))
    }

    /// `NewExpression`: `new callee<type_arguments>(arg0, arg1, ...)`.
    ///
    /// juno `gen_js.rs:935-968`.
    pub(crate) fn gen_new_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &NewExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let NewExpression {
            metadata: _,
            callee,
            type_arguments,
            arguments,
        } = inner;
        out!(self, "new ");
        self.print_child(
            ctx,
            Some(*callee),
            Path::new(node, NodeField::callee),
            ChildPos::Left,
        )?;
        if let Some(type_arguments) = type_arguments {
            self.gen_node(
                ctx,
                type_arguments,
                Some(Path::new(node, NodeField::type_arguments)),
            )?;
        }
        out!(self, "(");
        for (i, arg) in arguments.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            self.print_child(
                ctx,
                Some(arg),
                Path::new(node, NodeField::arguments),
                ChildPos::Anywhere,
            )?;
        }
        out!(self, ")");
        Ok(())
    }

    /// `YieldExpression`: `yield argument` or `yield* argument` or bare
    /// `yield`.
    ///
    /// juno `gen_js.rs:969-987`.
    pub(crate) fn gen_yield_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &YieldExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let YieldExpression {
            metadata: _,
            argument,
            delegate,
        } = inner;
        out!(self, "yield");
        if delegate.get() {
            out!(self, "*");
            self.space(ForceSpace::No);
        } else if argument.is_some() {
            out!(self, " ");
        }
        self.print_child(
            ctx,
            *argument,
            Path::new(node, NodeField::argument),
            ChildPos::Right,
        )
    }

    /// `AwaitExpression`: `await argument`.
    ///
    /// juno `gen_js.rs:988-999`.
    pub(crate) fn gen_await_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &AwaitExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let AwaitExpression {
            metadata: _,
            argument,
        } = inner;
        out!(self, "await ");
        self.print_child(
            ctx,
            Some(*argument),
            Path::new(node, NodeField::argument),
            ChildPos::Right,
        )
    }

    /// `ImportExpression`: `import(source)` or `import(source, options)`.
    ///
    /// juno `gen_js.rs:1001-1014`. Module doc comment: juno's second field
    /// is named `attributes`; ours is the structurally identical `options`.
    pub(crate) fn gen_import_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ImportExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let ImportExpression {
            metadata: _,
            source,
            options,
        } = inner;
        out!(self, "import(");
        self.gen_node(ctx, source, Some(Path::new(node, NodeField::source)))?;
        if let Some(options) = options {
            out!(self, ",");
            self.space(ForceSpace::No);
            self.gen_node(ctx, options, Some(Path::new(node, NodeField::options)))?;
        }
        out!(self, ")");
        Ok(())
    }

    /// `CallExpression`: `callee<type_arguments>(arg0, arg1, ...)`.
    ///
    /// juno `gen_js.rs:1016-1048`.
    pub(crate) fn gen_call_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &CallExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let CallExpression {
            metadata: _,
            callee,
            type_arguments,
            arguments,
        } = inner;
        self.print_child(
            ctx,
            Some(*callee),
            Path::new(node, NodeField::callee),
            ChildPos::Left,
        )?;
        if let Some(type_arguments) = type_arguments {
            self.gen_node(
                ctx,
                type_arguments,
                Some(Path::new(node, NodeField::type_arguments)),
            )?;
        }
        out!(self, "(");
        for (i, arg) in arguments.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            self.print_child(
                ctx,
                Some(arg),
                Path::new(node, NodeField::arguments),
                ChildPos::Anywhere,
            )?;
        }
        out!(self, ")");
        Ok(())
    }

    /// `OptionalCallExpression`: like `CallExpression`, but `?.(` instead of
    /// `(` when `optional`.
    ///
    /// juno `gen_js.rs:1049-1082`.
    ///
    /// # Deviation from juno: `?.` goes BEFORE the type arguments
    ///
    /// juno emits `callee`, then `type_arguments`, then `"?."`, then `"("`
    /// (`gen_js.rs:1056-1069`), so a Flow optional call with type arguments
    /// comes out as `f<T>?.(1)`. That does not parse: the `?.` token is what
    /// *introduces* the optional-chain link, and the type-argument list
    /// belongs to the call it introduces, so the only spelling the parser
    /// accepts is `f?.<T>(1)`. Confirmed both ways against our own parser —
    /// `sema_corpus/flow-type-args.js` contains `f?.<Baz>(1)`, and feeding
    /// juno's order back in fails with `invalid expression` at the `<`
    /// (`crates/parser/src/js/flow/expressions.rs`'s optional-chain tail,
    /// C++ `JSParserImpl.cpp`'s `parseOptionalExpressionExceptNew_tail`,
    /// which reads the `?.` first and only then looks for `<`). This was
    /// found by the Tier 1 corpus gate (`tests/corpus.rs`) and is pinned by
    /// [`optional_call_with_type_arguments_puts_the_question_dot_first`]
    /// in `tests/roundtrip.rs`.
    pub(crate) fn gen_optional_call_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &OptionalCallExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let OptionalCallExpression {
            metadata: _,
            callee,
            type_arguments,
            arguments,
            optional,
        } = inner;
        self.print_child(
            ctx,
            Some(*callee),
            Path::new(node, NodeField::callee),
            ChildPos::Left,
        )?;
        if optional.get() {
            out!(self, "?.");
        }
        if let Some(type_arguments) = type_arguments {
            self.gen_node(
                ctx,
                type_arguments,
                Some(Path::new(node, NodeField::type_arguments)),
            )?;
        }
        out!(self, "(");
        for (i, arg) in arguments.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            self.print_child(
                ctx,
                Some(arg),
                Path::new(node, NodeField::arguments),
                ChildPos::Anywhere,
            )?;
        }
        out!(self, ")");
        Ok(())
    }

    /// `AssignmentExpression`: `left op right` (e.g. `left = right`,
    /// `left += right`).
    ///
    /// juno `gen_js.rs:1084-1105`. `operator` prints straight from its raw
    /// atom (`ctx.bytes_str_lossy`), unlike `BinaryExpression`'s/
    /// `UnaryExpression`'s/`LogicalExpression`'s/`UpdateExpression`'s: those
    /// four go through a `precedence.rs` classifier because
    /// `get_precedence` needs to classify `BinaryExpression`/
    /// `LogicalExpression` operators for `**`'s right-associativity fix
    /// (module doc comment); `get_precedence`'s `AssignmentExpression` arm
    /// is a fixed `(ASSIGN, Assoc::Rtl)` regardless of which of the 16
    /// assignment operator spellings is present, so nothing here needs a
    /// dedicated classifier to exist — every assignment operator spelling
    /// is always plain ASCII from a fixed lexer token set, so
    /// `bytes_str_lossy` never substitutes a `U+FFFD`.
    pub(crate) fn gen_assignment_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &AssignmentExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let AssignmentExpression {
            metadata: _,
            operator,
            left,
            right,
        } = inner;
        self.print_child(
            ctx,
            Some(*left),
            Path::new(node, NodeField::left),
            ChildPos::Left,
        )?;
        self.space(ForceSpace::No);
        // `=` (and every compound assignment operator's tail) can be munched
        // onto whatever the left operand ended with; see
        // [`GenJS::space_before_equals`]. Only the plain `=` spelling starts
        // with `=`, so this fires only for it.
        let op = ctx.bytes_str_lossy(operator.get());
        self.space_before_equals(op);
        out!(self, "{}", op);
        self.space(ForceSpace::No);
        self.print_child(
            ctx,
            Some(*right),
            Path::new(node, NodeField::right),
            ChildPos::Right,
        )
    }

    /// `UnaryExpression`: prefix (`!x`, `typeof x`, ...) or postfix — though
    /// every unary operator is grammatically prefix-only; `prefix` is
    /// checked anyway, mirroring juno.
    ///
    /// juno `gen_js.rs:1106-1136`.
    pub(crate) fn gen_unary_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &UnaryExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let UnaryExpression {
            metadata: _,
            operator,
            argument,
            prefix,
        } = inner;
        let op = UnaryExpressionOperator::from_label(ctx, operator.get())?;
        let ident = op.as_str().chars().next().unwrap().is_alphabetic();
        if prefix.get() {
            out!(self, "{}", op.as_str());
            if ident {
                out!(self, " ");
            }
            self.print_child(
                ctx,
                Some(*argument),
                Path::new(node, NodeField::argument),
                ChildPos::Right,
            )?;
        } else {
            self.print_child(
                ctx,
                Some(*argument),
                Path::new(node, NodeField::argument),
                ChildPos::Left,
            )?;
            if ident {
                out!(self, " ");
            }
            out!(self, "{}", op.as_str());
        }
        Ok(())
    }

    /// `UpdateExpression`: prefix (`++x`) or postfix (`x++`).
    ///
    /// juno `gen_js.rs:1137-1160`.
    pub(crate) fn gen_update_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &UpdateExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let UpdateExpression {
            metadata: _,
            operator,
            argument,
            prefix,
        } = inner;
        let op = UpdateExpressionOperator::from_label(ctx, operator.get())?;
        if prefix.get() {
            out!(self, "{}", op.as_str());
            self.print_child(
                ctx,
                Some(*argument),
                Path::new(node, NodeField::argument),
                ChildPos::Right,
            )?;
        } else {
            self.print_child(
                ctx,
                Some(*argument),
                Path::new(node, NodeField::argument),
                ChildPos::Left,
            )?;
            out!(self, "{}", op.as_str());
        }
        Ok(())
    }

    /// `MemberExpression`: `object.property` or `object[property]`.
    ///
    /// juno `gen_js.rs:1161-1198`. Keeps juno's `50..toString()` special
    /// case (task-5 brief, `gen_js.rs:1168-1173`): a bare `NumericLiteral`
    /// object needs an extra `.` before non-computed member access, unless
    /// its printed form already contains `e`, `E`, or `.` (`1e5.toString()`,
    /// `1.5.toString()` are already unambiguous) — otherwise `50.toString()`
    /// would lex as `50.` (a float literal) followed by a syntax error at
    /// `toString()`, not as member access on `50`.
    pub(crate) fn gen_member_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &MemberExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let MemberExpression {
            metadata: _,
            object,
            property,
            computed,
        } = inner;
        match object {
            Node::NumericLiteral(NumericLiteral { metadata: _, value }) => {
                // Account for possible `50..toString()`.
                let string = hermes_support::json_emitter::number_to_string(value.get());
                // If there is an `e` or a decimal point, no need for an
                // extra `.`.
                let suffix = if string.contains(['E', 'e', '.']) {
                    ""
                } else {
                    "."
                };
                out!(self, "{}{}", string, suffix);
            }
            _ => {
                self.print_child(
                    ctx,
                    Some(*object),
                    Path::new(node, NodeField::object),
                    ChildPos::Left,
                )?;
            }
        }
        if computed.get() {
            out!(self, "[");
        } else {
            out!(self, ".");
        }
        self.print_child(
            ctx,
            Some(*property),
            Path::new(node, NodeField::property),
            ChildPos::Right,
        )?;
        if computed.get() {
            out!(self, "]");
        }
        Ok(())
    }

    /// `OptionalMemberExpression`: like `MemberExpression`, but `?.`
    /// prefixed onto the separator when `optional`. No numeric-literal
    /// special case: `50?.toString()` is unambiguous without an extra `.`
    /// (`?` is not a valid continuation of a numeric literal the way a bare
    /// `.` is), matching juno, which omits the special case here too.
    ///
    /// juno `gen_js.rs:1199-1226`.
    pub(crate) fn gen_optional_member_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &OptionalMemberExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let OptionalMemberExpression {
            metadata: _,
            object,
            property,
            computed,
            optional,
        } = inner;
        self.print_child(
            ctx,
            Some(*object),
            Path::new(node, NodeField::object),
            ChildPos::Left,
        )?;
        if computed.get() {
            out!(self, "{}[", if optional.get() { "?." } else { "" });
        } else {
            out!(self, "{}.", if optional.get() { "?" } else { "" });
        }
        self.print_child(
            ctx,
            Some(*property),
            Path::new(node, NodeField::property),
            ChildPos::Right,
        )?;
        if computed.get() {
            out!(self, "]");
        }
        Ok(())
    }

    /// `BinaryExpression`: `left op right`.
    ///
    /// juno `gen_js.rs:1228-1258`. The right-associativity fix for `**` is
    /// in `precedence.rs`'s `get_precedence`, not here — this arm always
    /// prints `left`, the operator, `right` in that order; parenthesization
    /// around `left`/`right` is entirely `print_child`'s decision.
    pub(crate) fn gen_binary_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &BinaryExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let BinaryExpression {
            metadata: _,
            left,
            right,
            operator,
        } = inner;
        let op = BinaryExpressionOperator::from_label(ctx, operator.get())?;
        let ident = op.as_str().chars().next().unwrap().is_alphabetic();
        let force = if ident {
            ForceSpace::Yes
        } else {
            ForceSpace::No
        };
        self.print_child(
            ctx,
            Some(*left),
            Path::new(node, NodeField::left),
            ChildPos::Left,
        )?;
        self.space(force);
        // `==`/`===` start with `=`, so they can be munched onto whatever
        // the left operand ended with. The one shape that reaches this in
        // real source is a self-closing JSX element: `t = <a/> == b;`
        // emitted `t=<a />==b;` in `Pretty::No`, whose `>=` the JSX-tag
        // lexer rejects with "'>' expected at end of JSX tag". See
        // [`GenJS::space_before_equals`].
        self.space_before_equals(op.as_str());
        out!(self, "{}", op.as_str());
        self.space(force);
        self.print_child(
            ctx,
            Some(*right),
            Path::new(node, NodeField::right),
            ChildPos::Right,
        )
    }

    /// `ConditionalExpression`: `test ? consequent : alternate`.
    ///
    /// juno `gen_js.rs:1267-1298`. Our `ConditionalExpression` struct
    /// declares its fields in a different order (`test`, `alternate`,
    /// `consequent`) than juno's (`test`, `consequent`, `alternate`) —
    /// Rust struct patterns bind by name, not position, so this has no
    /// effect on the print order below, which is `test`/`consequent`/
    /// `alternate`, matching the grammar.
    pub(crate) fn gen_conditional_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &ConditionalExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let ConditionalExpression {
            metadata: _,
            test,
            consequent,
            alternate,
        } = inner;
        self.print_child(
            ctx,
            Some(*test),
            Path::new(node, NodeField::test),
            ChildPos::Left,
        )?;
        self.space(ForceSpace::No);
        out!(self, "?");
        self.space(ForceSpace::No);
        self.print_child(
            ctx,
            Some(*consequent),
            Path::new(node, NodeField::consequent),
            ChildPos::Anywhere,
        )?;
        self.space(ForceSpace::No);
        out!(self, ":");
        self.space(ForceSpace::No);
        self.print_child(
            ctx,
            Some(*alternate),
            Path::new(node, NodeField::alternate),
            ChildPos::Right,
        )
    }

    /// `Property`: an `ObjectExpression` member — a data property, a
    /// shorthand property, a method, or a getter/setter.
    ///
    /// juno `gen_js.rs:1442-1528`. See the module doc comment for the
    /// `visit_func_params_body` forward reference and the `unreachable!()` →
    /// `GenJsError` substitution.
    pub(crate) fn gen_property<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &Property<'gc>,
    ) -> Result<(), GenJsError> {
        let Property {
            metadata: _,
            key,
            value,
            kind,
            computed,
            method,
            shorthand,
        } = inner;
        let kind = PropertyKind::from_label(ctx, kind.get())?;

        let mut need_sep = false;
        if kind != PropertyKind::Init {
            out!(self, "{}", kind.as_str());
            need_sep = true;
        } else if method.get() {
            match value {
                Node::FunctionExpression(FunctionExpression {
                    metadata: _,
                    id: _,
                    params: _,
                    body: _,
                    type_parameters: _,
                    return_type: _,
                    predicate: _,
                    generator,
                    r#async,
                    scope: _,
                    sem_info: _,
                    strictness: _,
                    is_method_definition: _,
                    decorations: _,
                }) => {
                    if r#async.get() {
                        out!(self, "async");
                        need_sep = true;
                    }
                    if generator.get() {
                        out!(self, "*");
                        need_sep = false;
                    }
                }
                _ => return Err(GenJsError::UnsupportedKind(value.kind())),
            };
        }
        if computed.get() {
            if need_sep {
                self.space(ForceSpace::No);
            }
            need_sep = false;
            out!(self, "[");
        }
        if need_sep {
            out!(self, " ");
        }
        if shorthand.get() {
            self.gen_node(ctx, value, None)?;
        } else {
            self.gen_node(ctx, key, None)?;
        }
        if computed.get() {
            out!(self, "]");
        }
        if shorthand.get() {
            return Ok(());
        }
        if kind != PropertyKind::Init || method.get() {
            match value {
                Node::FunctionExpression(FunctionExpression {
                    metadata: _,
                    // Name is handled by the property key.
                    id: _,
                    params,
                    body,
                    return_type,
                    predicate,
                    type_parameters,
                    // Handled above.
                    generator: _,
                    r#async: _,
                    scope: _,
                    sem_info: _,
                    strictness: _,
                    is_method_definition: _,
                    decorations: _,
                }) => {
                    self.visit_func_params_body(
                        ctx,
                        *params,
                        *type_parameters,
                        *return_type,
                        *predicate,
                        body,
                        value,
                    )?;
                }
                _ => return Err(GenJsError::UnsupportedKind(value.kind())),
            };
        } else {
            out!(self, ":");
            self.space(ForceSpace::No);
            self.gen_node(ctx, value, Some(Path::new(node, NodeField::value)))?;
        }
        Ok(())
    }

    /// `LogicalExpression`: `left && right`, `left || right`, or
    /// `left ?? right`.
    ///
    /// juno `gen_js.rs:1530-1551`. No `ident` check like `BinaryExpression`'s
    /// — `&&`/`||`/`??` are all symbolic, never alphabetic, so
    /// `ForceSpace::No` (pretty-mode-only spacing) is always correct here.
    pub(crate) fn gen_logical_expression<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        inner: &LogicalExpression<'gc>,
    ) -> Result<(), GenJsError> {
        let LogicalExpression {
            metadata: _,
            left,
            right,
            operator,
        } = inner;
        let op = LogicalExpressionOperator::from_label(ctx, operator.get())?;
        self.print_child(
            ctx,
            Some(*left),
            Path::new(node, NodeField::left),
            ChildPos::Left,
        )?;
        self.space(ForceSpace::No);
        out!(self, "{}", op.as_str());
        self.space(ForceSpace::No);
        self.print_child(
            ctx,
            Some(*right),
            Path::new(node, NodeField::right),
            ChildPos::Right,
        )
    }

    /// Print an `ObjectExpression`'s (or `ObjectPattern`'s) `{prop0, prop1,
    /// ...}` property list.
    ///
    /// juno `gen_js.rs:3353-3362`.
    pub(crate) fn visit_props<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        props: NodeList<'gc>,
        path: Path<'gc>,
    ) -> Result<(), GenJsError> {
        out!(self, "{{");
        for (i, prop) in props.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            self.gen_node(ctx, prop, Some(path))?;
        }
        out!(self, "}}");
        Ok(())
    }

    /// Print a function-shaped node's `(params)`, optional `: return_type`/
    /// `predicate`, and `body` — shared by `Property`'s method/getter/setter
    /// branch (this task) and, once Task 7 lands, `FunctionExpression`/
    /// `FunctionDeclaration`/`ArrowFunctionExpression`'s own arms. See the
    /// module doc comment's "`Property`'s forward reference" section for why
    /// this is implemented here rather than in Task 7's `arms/func.rs`, and
    /// for the one field-name fix (`NodeField::params`, not juno's
    /// `NodeField::param`) in the per-parameter `Path`.
    ///
    /// juno `gen_js.rs:3365-3399` (not `visit_func_type_params`,
    /// `gen_js.rs:3401-3452` — a different, TS/Flow-only helper, still
    /// Task 7's alone).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn visit_func_params_body<'gc>(
        &mut self,
        ctx: &GCLock<'_, '_>,
        params: NodeList<'gc>,
        type_parameters: Option<&'gc Node<'gc>>,
        return_type: Option<&'gc Node<'gc>>,
        predicate: Option<&'gc Node<'gc>>,
        body: &'gc Node<'gc>,
        node: &'gc Node<'gc>,
    ) -> Result<(), GenJsError> {
        if let Some(type_parameters) = type_parameters {
            self.gen_node(
                ctx,
                type_parameters,
                Some(Path::new(node, NodeField::type_parameters)),
            )?;
        }
        out!(self, "(");
        for (i, param) in params.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            self.gen_node(ctx, param, Some(Path::new(node, NodeField::params)))?;
        }
        out!(self, ")");
        if return_type.is_some() || predicate.is_some() {
            out!(self, ":");
        }
        if let Some(return_type) = return_type {
            self.space(ForceSpace::No);
            self.gen_node(
                ctx,
                return_type,
                Some(Path::new(node, NodeField::return_type)),
            )?;
        }
        if let Some(predicate) = predicate {
            self.space(ForceSpace::Yes);
            self.gen_node(ctx, predicate, Some(Path::new(node, NodeField::predicate)))?;
        }
        self.space(ForceSpace::No);
        self.gen_node(ctx, body, Some(Path::new(node, NodeField::body)))
    }
}

#[cfg(test)]
mod tests {
    use hermes_ast::node::Program;
    use hermes_parser::{parse, ParseFlags};

    use super::*;
    use crate::precedence::NeedParens;
    use crate::Opt;

    /// Parse `src` (expected to be a single expression-statement) and hand
    /// the enclosing `ExpressionStatement` node, its `.expression`, and the
    /// locked `GCLock` to `f`.
    ///
    /// Handing back the `ExpressionStatement` node too (not just the
    /// expression, unlike `arms/literal.rs`'s `with_declarator`) is what lets
    /// some tests below build a real `Path` with it as the parent, to
    /// exercise `need_parens`'s `Node::ExpressionStatement(_)` branch
    /// (`precedence.rs`, juno `gen_js.rs:675-687`) — the branch behind two of
    /// this task's ten required parenthesization cases — without needing
    /// `ExpressionStatement` itself to have a print arm yet (that's Task 6).
    fn with_expr_stmt<R>(
        src: &str,
        f: impl for<'gc> FnOnce(&'gc GCLock<'static, '_>, &'gc Node<'gc>, &'gc Node<'gc>) -> R,
    ) -> R {
        let mut parsed = parse(src, ParseFlags::default()).expect("test source must parse");
        parsed.with_program(|gc, node| {
            let Node::Program(Program {
                metadata: _,
                body,
                scope: _,
                sem_info: _,
                strictness: _,
                is_method_definition: _,
                decorations: _,
                dummy_param_list: _,
            }) = node
            else {
                panic!("root is not a Program");
            };
            let stmt = body.iter().next().expect("source has a statement");
            let Node::ExpressionStatement(es) = stmt else {
                panic!("statement is not an ExpressionStatement: {stmt:?}");
            };
            f(gc, stmt, es.expression)
        })
    }

    /// Generate just `node` (not a whole program) and decode the result as a
    /// `String`. Same helper shape as `arms/literal.rs`'s
    /// `gen_node_to_string`, reimplemented locally rather than shared: each
    /// arms module's test suite already does this (`precedence.rs`'s
    /// `with_expr` is the same story), and a shared test-only helper module
    /// is more machinery than three call sites justify.
    fn gen_node_to_string<'gc>(gc: &GCLock<'static, '_>, node: &'gc Node<'gc>) -> String {
        let mut sink = Vec::new();
        {
            let mut gen_js = GenJS::for_test(&mut sink, Opt::new());
            gen_js.gen_node(gc, node, None).expect("node generates");
        }
        String::from_utf8(sink).expect("generator output is always valid UTF-8 (spec §5)")
    }

    // -----------------------------------------------------------------
    // The task-5 brief's ten required parenthesization-sensitive cases.
    // Seven round-trip end to end through this module's own arms; three
    // (marked below) need a node kind or statement context this task does
    // not yet print, so they instead pin down the piece of the decision
    // that *is* available today and document what is still missing. See
    // task-5-report.md for the full breakdown.
    // -----------------------------------------------------------------

    /// Case 1/10, `(a, b) => c`: fully round-trips now that Task 7's
    /// `ArrowFunctionExpression` arm exists. `(a, b)` here is the arrow's
    /// parameter list, a plain `NodeList` of two `Identifier`s (confirmed
    /// against `parse_to_estree_json`'s output for this exact source) — not
    /// a `SequenceExpression`, so this does not exercise
    /// `SequenceExpression`'s own arm above; the parenthesization is
    /// unconditional (an arrow with more than one param is always wrapped in
    /// literal `(` `)` by `gen_arrow_function_expression`'s own
    /// two-or-more-params branch, not a `need_parens` decision).
    #[test]
    fn arrow_with_two_params_round_trips() {
        with_expr_stmt("(a, b) => c;", |gc, _stmt, expr| {
            assert!(matches!(expr, Node::ArrowFunctionExpression(_)));
            let js = gen_node_to_string(gc, expr);
            assert_eq!(js, "(a, b) => c");
        });
    }

    /// Case 2/10, `a ** b ** c`: fully round-trips. `**`'s real
    /// right-associativity (`precedence.rs`'s Task 3 fix) means the natural
    /// right-nested parse (`a ** (b ** c)`) needs no parens anywhere —
    /// confirming the fix holds through this task's actual `BinaryExpression`
    /// printing arm, not just `get_precedence` in isolation (already covered
    /// by `precedence.rs`'s own tests).
    #[test]
    fn exponentiation_chain_round_trips_without_unnecessary_parens() {
        with_expr_stmt("a ** b ** c;", |gc, _stmt, expr| {
            assert!(matches!(expr, Node::BinaryExpression(_)));
            let js = gen_node_to_string(gc, expr);
            assert_eq!(js, "a ** b ** c");
        });
    }

    /// Case 3/10, `(a + b) * c`: fully round-trips. `+` binds looser than
    /// `*` (`get_binary_precedence`), so the left child needs parens or
    /// `(a + b) * c` would reparse as `a + b * c` — a different value
    /// whenever `a`, `b`, `c` differ arithmetically from their product/sum
    /// order.
    #[test]
    fn mult_over_paren_plus_round_trips_with_left_parens() {
        with_expr_stmt("(a + b) * c;", |gc, _stmt, expr| {
            assert!(matches!(expr, Node::BinaryExpression(_)));
            let js = gen_node_to_string(gc, expr);
            assert_eq!(js, "(a + b) * c");
        });
    }

    /// Case 4/10, `a ?? (b || c)`: fully round-trips. `??` mixed with `||`
    /// (or `&&`) always needs parens regardless of precedence numbers
    /// (`need_parens`'s `check_and_or`/`check_nullish` branch,
    /// `precedence.rs`) — ECMA-262 makes `a ?? b || c` a syntax error
    /// outright, so omitting the parens here would not just reparse
    /// differently, it would fail to reparse at all.
    #[test]
    fn nullish_mixed_with_or_round_trips_with_parens() {
        with_expr_stmt("a ?? (b || c);", |gc, _stmt, expr| {
            assert!(matches!(expr, Node::LogicalExpression(_)));
            let js = gen_node_to_string(gc, expr);
            assert_eq!(js, "a ?? (b || c)");
        });
    }

    /// Case 5/10, `new (a.b())()`: fully round-trips. `need_parens`'s
    /// `NewExpression` branch (`precedence.rs`) requires parens around a
    /// `new` callee that contains a call anywhere in its subtree
    /// (`contains_call`) — without them, `new a.b()()` would parse as
    /// `(new a.b())()`: the call binds to `a.b` first, terminating `new`
    /// early, then the whole `new a.b()` is called again — a different
    /// expression (calling the *constructed instance* rather than passing
    /// no-arg `()` straight to the already-called `a.b()` inside `new`'s own
    /// argument list).
    #[test]
    fn new_over_callee_containing_call_round_trips_with_parens() {
        with_expr_stmt("new (a.b())();", |gc, _stmt, expr| {
            assert!(matches!(expr, Node::NewExpression(_)));
            let js = gen_node_to_string(gc, expr);
            assert_eq!(js, "new (a.b())()");
        });
    }

    /// Case 6/10, `(function(){})()`: fully round-trips now that Task 7's
    /// `FunctionExpression` arm exists. The parens are only needed because
    /// this whole expression sits in `ExpressionStatement` position
    /// (`need_parens`'s `Node::ExpressionStatement(_)` branch,
    /// `precedence.rs`) — as a variable initializer, say, `function(){}()`
    /// would already be unambiguous and need none. This test both confirms
    /// `need_parens` still says `Yes` for a `CallExpression` starting with a
    /// `FunctionExpression` callee (through `CallExpression`'s
    /// `expr_starts_with` case, wired in Task 3) and that the full print,
    /// routed through the same `ExpressionStatement` parent `Path` via
    /// `print_child`, actually adds them.
    ///
    /// The parens land around the *whole* `CallExpression`, not just the
    /// `FunctionExpression` callee — `need_parens` was asked about (and said
    /// `Yes` for) `expr` itself (the `CallExpression`), so `print_parens`
    /// wraps all of it: `(function() {}())`, not `(function() {})()`.
    /// Confirmed empirically (this is the same "assume juno has bugs,
    /// verify" standard applied to a case that turned out fine): both
    /// spellings are valid, equivalent JS — this one happens to be
    /// Crockford's "wrap the invocation" IIFE style rather than "wrap the
    /// function expression" — and juno's algorithm (ported unchanged) is
    /// what produces it, the same way `arms/expr.rs`'s case 7
    /// (`object_literal_member_as_expression_statement_round_trips_with_parens`)
    /// documents an analogous placement for `({}).x`.
    #[test]
    fn iife_as_expression_statement_round_trips_with_parens() {
        with_expr_stmt("(function(){})();", |gc, stmt, expr| {
            assert!(matches!(expr, Node::CallExpression(_)));
            let mut need_parens_sink = Vec::new();
            let need_parens_gen_js = GenJS::for_test(&mut need_parens_sink, Opt::new());
            let need = need_parens_gen_js
                .need_parens(
                    gc,
                    Path::new(stmt, NodeField::expression),
                    expr,
                    ChildPos::Anywhere,
                )
                .expect("well-formed tree classifies without error");
            assert_eq!(
                need,
                NeedParens::Yes,
                "(function(){{}})() must keep parens in expression-statement position"
            );

            let mut sink = Vec::new();
            {
                let mut gen_js = GenJS::for_test(&mut sink, Opt::new());
                gen_js
                    .print_child(
                        gc,
                        Some(expr),
                        Path::new(stmt, NodeField::expression),
                        ChildPos::Anywhere,
                    )
                    .expect("node generates");
            }
            let js = String::from_utf8(sink).expect("generator output is always valid UTF-8");
            assert_eq!(js, "(function() {}())");
        });
    }

    /// Case 7/10, `({}).x`: fully round-trips — though not back to its own
    /// spelling; see below. Same `need_parens` `ExpressionStatement` branch
    /// as case 6 (`root_starts_with` reaching an `ObjectExpression` this
    /// time), but every node kind involved (`ObjectExpression`,
    /// `MemberExpression`, `Identifier`) already has a print arm, so —
    /// unlike case 6 — this one prints real text: this uses `print_child`
    /// directly with a real `ExpressionStatement` parent `Path` (rather than
    /// `gen_node` with `path: None`, which would never invoke this branch at
    /// all and silently produce the unparenthesized, wrong-for-this-position
    /// `{}.x`) to prove the whole decision-plus-print pipeline agrees, not
    /// just the decision in isolation.
    ///
    /// The output is `({}.x)`, not `({}).x`: `need_parens`/`print_parens`
    /// decide parens for the *whole* child relative to its parent — here the
    /// entire `MemberExpression`, since that is what `ExpressionStatement`'s
    /// `.expression` field points at — not for whichever inner subexpression
    /// is the one that structurally "starts with" the disallowed
    /// `ObjectExpression` (`root_starts_with`/`expr_starts_with` only
    /// *locate* that subexpression to decide *whether* parens are needed at
    /// all; they are not where the parens get placed). Both spellings parse
    /// back to the identical `MemberExpression{object: ObjectExpression,
    /// property: x}` tree, so this is not a bug — it is simply where juno's
    /// algorithm (ported unchanged here) puts the parens, confirmed by
    /// running this test before writing the assertion, not assumed.
    #[test]
    fn object_literal_member_as_expression_statement_round_trips_with_parens() {
        with_expr_stmt("({}).x;", |gc, stmt, expr| {
            assert!(matches!(expr, Node::MemberExpression(_)));
            let mut sink = Vec::new();
            {
                let mut gen_js = GenJS::for_test(&mut sink, Opt::new());
                gen_js
                    .print_child(
                        gc,
                        Some(expr),
                        Path::new(stmt, NodeField::expression),
                        ChildPos::Anywhere,
                    )
                    .expect("node generates");
            }
            let js = String::from_utf8(sink).expect("generator output is always valid UTF-8");
            assert_eq!(js, "({}.x)");
        });
    }

    /// Case 8/10, `(a = b) => c`: fully round-trips now that Task 7's
    /// `ArrowFunctionExpression` arm exists. Parsing confirms the single
    /// param is an `AssignmentPattern` (`left: a, right: b`), not an
    /// `AssignmentExpression`; the single-parameter shortcut's `matches!`
    /// guard requires the sole param to be a bare `Identifier`
    /// (`arms/func.rs`'s `gen_arrow_function_expression`), so an
    /// `AssignmentPattern` param always takes the parenthesized branch —
    /// this is the case that proves the shortcut's condition isn't
    /// over-simplified to "exactly one param", the hazard the Task 7 brief
    /// calls out by name.
    #[test]
    fn arrow_with_default_param_round_trips_with_parens() {
        with_expr_stmt("(a = b) => c;", |gc, _stmt, expr| {
            assert!(matches!(expr, Node::ArrowFunctionExpression(_)));
            let js = gen_node_to_string(gc, expr);
            assert_eq!(js, "(a = b) => c");
        });
    }

    /// Case 9/10, `a?.b?.()`: fully round-trips. Every link in the chain
    /// uses `?.` explicitly, so `need_parens`'s "optional chain terminated
    /// by a non-optional access" branch never fires — this proves the
    /// *absence* of parens is also correct, not just their presence (cases
    /// 3-5, 7 all test the opposite direction).
    #[test]
    fn optional_chain_round_trips_without_unnecessary_parens() {
        with_expr_stmt("a?.b?.();", |gc, _stmt, expr| {
            assert!(matches!(expr, Node::OptionalCallExpression(_)));
            let js = gen_node_to_string(gc, expr);
            assert_eq!(js, "a?.b?.()");
        });
    }

    /// Case 10/10, `50..toString()`: fully round-trips.
    /// [`GenJS::gen_member_expression`]'s numeric-literal special case
    /// (juno `gen_js.rs:1168-1173`, task-5 brief) appends the extra `.`
    /// that turns `50.toString()` (`50.` lexes as one float literal token,
    /// then `toString()` is a syntax error) into the legal `50..toString()`.
    #[test]
    fn numeric_literal_member_access_round_trips_with_extra_dot() {
        with_expr_stmt("50..toString();", |gc, _stmt, expr| {
            assert!(matches!(expr, Node::CallExpression(_)));
            let js = gen_node_to_string(gc, expr);
            assert_eq!(js, "50..toString()");
        });
    }

    // -----------------------------------------------------------------
    // "Prove it can fail" (`prove-checks-can-fail`): pick one of the above
    // and show a concrete regression makes a *named* test fail with a
    // specific message, not just that the suite ran. This mutates
    // `precedence.rs`'s already-fixed `**` associativity back to the juno
    // bug — see `task-5-report.md` for the transcript of the failing run
    // (`exponentiation_chain_round_trips_without_unnecessary_parens`,
    // `assertion `left == right` failed`, left: `"a ** (b ** c)"`, right:
    // `"a ** b ** c"`) and its revert.
    // -----------------------------------------------------------------
}
