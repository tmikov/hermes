/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! S2 T4/T5: classes — the `ClassContext`, the untyped class-as-expression
//! path, class properties, method definitions, `super`, and (S2 T5) the
//! private-name early-error machinery plus static blocks. A further
//! `impl<'bt, 'sc, 'sm, 'ad> SemanticResolver<'bt, 'sc, 'sm, 'ad>` block,
//! split out of `resolver/mod.rs` the same way `identifiers.rs` (S1 T4),
//! `declarations.rs` (S1 T5), `expressions.rs` (S1 T6), `functions.rs`
//! (S1 T7) and `statements.rs` (S2 T1) were — see `identifiers.rs`'s module
//! doc for why a child module sees `mod.rs`'s private fields and helpers.
//!
//! Ports `hermes::sema::ClassContext` (declared at SemanticResolver.h:
//! 630-677, defined at SemanticResolver.cpp:3081-3181),
//! `SemanticResolver::visit(ClassDeclarationNode *)` (cpp:891-907),
//! `visit(ClassExpressionNode *)` (cpp:909-911), `visitClassAsExpr`
//! (cpp:913-950), `visit(ClassPropertyNode *)` (cpp:1008-1051),
//! `visit(MethodDefinitionNode *, Node *)` (cpp:1094-1115) and
//! `visit(SuperNode *, Node *)` (cpp:1086-1092), plus the four `ESTree::`
//! free functions they reach (`getSuperClass`, `getClassID`, `getClassBody`,
//! `getDecorators` — `lib/AST/ESTree.cpp:228-277`).
//!
//! S2 T5 adds `collectDeclaredPrivateIdentifiers` (cpp:2143-2260),
//! `visit(PrivateNameNode *)` (cpp:952-963),
//! `visit(ClassPrivatePropertyNode *)` (cpp:965-1006) and
//! `visit(StaticBlockNode *)` (cpp:1053-1084). The two halves the private
//! names need that do NOT live here are `declarePrivateName`/
//! `resolvePrivateName` (`identifiers.rs`, next to their C++ neighbour
//! `checkIdentifierResolved`), the `#`-prefix mangling
//! (`sem_context::private_name_identifier`), and the `MemberExpression`/
//! `OptionalMemberExpression` restriction checks (cpp:1207-1295,
//! `expressions.rs`).
//!
//! ## Where the synthetic `FunctionInfo` ids live
//!
//! A class can own up to three `FunctionInfo`s that no `FunctionLikeNode`
//! ever produced, and they are what makes the class dump shape non-obvious.
//! ALL THREE live in `Cell`s on the CLASS node itself — the
//! `ClassLikeDecoration` fields (`ESTree.h:409-425`,
//! `gen_nodes.py`'s `ClassLikeDecoration`), **not** in the `ClassContext`:
//!
//! - `createImplicitConstructorFunctionInfo` (cpp:3088-3114) writes
//!   `implicit_ctor_function_info`; its `ConstructorKind` is `Derived` if the
//!   class has a superclass, else `Base`.
//! - `getOrCreateInstanceElementsInitFunctionInfo` (cpp:3116-3138) writes
//!   `instance_elements_init_function_info`; `ConstructorKind::None`.
//! - `getOrCreateStaticElementsInitFunctionInfo` (cpp:3140-3163) writes
//!   `static_elements_init_function_info`; `ConstructorKind::None`.
//!
//! `createStaticBlockFunctionInfo` (cpp:3165-3177) is the fourth creator and
//! the only one whose id does NOT land on the class: it goes on the
//! `StaticBlock` node's own `function_info` `Cell` (`StaticBlockDecoration`),
//! and its `FunctionInfo` is flagged `is_static_block` (which is what makes
//! the dump print `StaticBlock strict` instead of `Func strict`). It is
//! ported here because it is a `ClassContext` member; its only caller is
//! [`SemanticResolver::visit_static_block`] (S2 T5).
//!
//! The only thing `ClassContext` itself stores is `has_constructor` (plus
//! the class node it decorates, and C++'s `prevContext_` — this port's
//! stack). Everything else is on the node, which is what lets IRGen find
//! them later without a side table.
//!
//! All three getters create a `LexicalScope` "for the side effect of
//! associating the new scope with" the synthetic function and mark it as
//! that function's body scope — the parent scope being `curScope_`, i.e. the
//! CLASS scope, since these are all called while `visitClassAsExpr`'s
//! `ScopeRAII` is live.
//!
//! ## THE decorate-after-children exception of this task
//!
//! `resolver/mod.rs`'s "decorate before recursing" invariant says a `Cell`
//! written after a node's children have been visited is lost if a child
//! visit returned `Changed`, because the generated builders snapshot every
//! `Cell` at `from_node`. Classes violate the *precondition* on both of the
//! decorations above:
//!
//! - `implicit_ctor_function_info` is written by
//!   `createImplicitConstructorFunctionInfo`, which C++ calls at the very
//!   END of `visitClassAsExpr` (cpp:949) — after the body walk, because it
//!   depends on `hasConstructor`, which only the body walk can set.
//! - `instance_elements_init_function_info` /
//!   `static_elements_init_function_info` are written from DEEP INSIDE that
//!   body walk (`visit(ClassPropertyNode *)`, `visit(MethodDefinitionNode
//!   *)`, `visit(StaticBlockNode *)`), i.e. while the class node's children
//!   are being visited.
//!
//! A fold in a field initializer (`class C { x = 1 + 2; }`) or a rewritten
//! arrow in a method body rebuilds the `ClassBody`, hence the class node —
//! so a builder snapshotted before the walk would hand back a class with all
//! three `Cell`s empty, silently detaching the synthetic functions from the
//! class. IRGen would then generate no field initialization at all.
//!
//! The fix is the same one `statements.rs`'s `visit(SwitchStatementNode *)`
//! uses, one step further: every write goes to the ORIGINAL node (which is
//! also what `ClassContext` holds, so the getters are idempotent across the
//! whole walk), and [`SemanticResolver::visit_class_as_expr`] creates the
//! builder only *after* the last write — seeding it with the already-visited
//! superclass and body. The rebuilt node therefore carries all three ids.
//! `a_rebuilt_class_keeps_its_synthetic_function_infos` in
//! `tests/resolver.rs` is the pin; the differential is only *indirectly*
//! sensitive to this (the synthetic functions appear in the `-dump-sema`
//! function tree either way — what it cannot see is which class node they
//! are attached to).
//!
//! ## Two decls on one identifier
//!
//! `visitClassAsExpr` declares the class name a second time, as a
//! `ClassExprName` in the class's own scope, and installs it as the
//! identifier's **expression** decl (cpp:923-935). For a `class C {}`
//! *declaration* the same `Identifier` already carries the hoisted `Class`
//! declaration decl, so the node ends up with two different decls — the
//! `Decl::Kind::ClassExprName` obeys const-variable rules, which is what
//! makes `class C { m() { C = 1; } }` an error while the outer binding stays
//! assignable. This is precisely the case `SemContext`'s side table exists
//! for (S0 T4); the dump renders it as `Id 'C' [D:%d.1 E:%d.65 'C']`.
//!
//! ## Private names: where the declarations come from
//!
//! Private names are declared by `collect_declared_private_identifiers`
//! (cpp:2143-2260) into the SAME scope as the `ClassExprName`, i.e. the class
//! node's own scope, under the `#`-mangled name
//! ([`crate::sem_context::private_name_identifier`]) — which is exactly why a
//! `#x` can never be confused with, or shadow, an ordinary `x`. It runs
//! BEFORE the class body is walked (cpp:939, and after the superclass
//! expression), which is what lets a member declared early reference a
//! private name declared late.
//!
//! Two properties of that function are easy to lose in a port and are pinned
//! by `tests/resolver.rs`:
//!
//! - A legal getter+setter pair ends up as ONE `Decl`, whose `kind` the
//!   SECOND accessor MUTATES in place from `PrivateGetter`/`PrivateSetter` to
//!   `PrivateGetterSetter` (cpp:2255), with both accessors' `Identifier`s
//!   bound to it via `setBothDecl`. That mutation targets `SemContext` state,
//!   not a node `Cell`, so it is exempt from the decorate-before-recurse
//!   invariant — noted at the site.
//! - Every diagnostic points at the *`Identifier`* inside the private name
//!   (`id->getSourceRange()`), never at the element or the `#`.
//!
//! ## What's dormant
//!
//! - **The `typed_` branch of `visit(ClassDeclarationNode *)`**
//!   (cpp:892-901) takes a different path entirely (no extra class-body
//!   variable, `collectDeclaredPrivateIdentifiers` + a generic children
//!   walk). `typed_` is `false` in this port (see [`TYPED`]), so the branch
//!   is ported in shape only, as a documented panic — typed dialects are
//!   their own future track.
//! - **The `@Hermes.overload` duplicate-private-method exemption**
//!   (cpp:2197-2200) is `typed_`-only for the same reason, and is a panic
//!   inside the `if TYPED` that guards it.

use std::collections::hash_map::Entry;
use std::collections::HashMap;

use ast::context::{GCLock, NodeRc};
use ast::node::{builder, Node, NodeField};
use ast::visitor::{Path, TransformResult, VisitorMut};
use ast::SemaId;

use crate::ids::{DeclId, FunctionInfoId};
use crate::sem_context::{
    Atom, Binding, ConstructorKind, CustomDirectives, DeclKind, FuncIsArrow,
};

use super::expressions::replacement_of;
use super::{SemanticResolver, DEBUG_INFO_SETTING_ALL};

/// Port of the `typed_` member (SemanticResolver.h:84) — always `false` in
/// this port, matching `declarations.rs`'s and `functions.rs`'s constants of
/// the same name (typed dialects are their own future track). Read by
/// `visit(ClassDeclarationNode *)` (cpp:892), `visit(ClassPropertyNode *)`
/// (cpp:1041) and `visit(MethodDefinitionNode *, Node *)` (cpp:1097).
const TYPED: bool = false;

/// Port of `hermes::sema::ClassContext` (SemanticResolver.h:630-677).
///
/// C++'s `resolver_`/`prevContext_` fields implement the intrusive context
/// stack; here the stack is `SemanticResolver::class_stack` and those two
/// fields are not needed. See `resolver/mod.rs`'s module doc for the RAII
/// deviation, and this module's doc for why the three synthetic
/// `FunctionInfo` ids are NOT fields here.
pub(super) struct ClassContext {
    /// Whether the class has an explicit constructor. Only valid after the
    /// body of the class has been visited.
    pub(super) has_constructor: bool,
    /// The (decorator of) the class of the context. Will store the member
    /// initializer function info if one is required.
    ///
    /// C++ holds a bare `ClassLikeNode *`; a `NodeRc` is this port's
    /// equivalent (same choice as `FunctionContext::node`). It always names
    /// the node the visit was ENTERED with, never a rebuilt copy — see the
    /// module doc's decorate-after-children section.
    class_node: NodeRc,
}

/// Information about a private accessor. Port of
/// `collectDeclaredPrivateIdentifiers`'s local `struct PrivateAccessorInfo`
/// (cpp:2146-2162).
///
/// C++'s `PrivateAccessorInfo{}` is an aggregate, so value-initialization
/// zero-initializes `originalNameDecl` to `nullptr` in the default
/// (field/method) cases; `isAccessor == false` keeps it from ever being
/// read. `Option<DeclId>::None` models that exactly, and the one read site
/// `expect`s it.
#[derive(Clone, Copy, Default)]
struct PrivateAccessorInfo {
    /// The rest of the fields in the struct are only meaningful if
    /// `is_accessor` is true. Fields & methods will have this as false.
    is_accessor: bool,
    is_static: bool,
    is_getter: bool,
    is_setter: bool,
    /// True if the first declaration was a private method decorated with
    /// `@Hermes.overload`. Subsequent overloaded methods with the same name
    /// are accepted; non-overload duplicates remain an error.
    is_overloaded_method: bool,
    /// In the case of a pair of accessors defined with the same name, we
    /// should reuse the same decl to define the declaration & expression
    /// decl of the second accessor. This field stores that original decl for
    /// the second accessor to find later.
    original_name_decl: Option<DeclId>,
}

/// Insert `value` under `key` only if `key` is vacant, leaving an existing
/// entry untouched. Port of the "only if not already present" semantics
/// shared by `llvh::DenseMap::try_emplace` and `llvh::DenseMap::insert`,
/// whose `.second` this returns.
///
/// `std::collections::HashMap` has no single-lookup equivalent on stable Rust
/// (`try_insert` is unstable, and `insert` overwrites), and the `Entry` API
/// cannot be used inline at the call sites because the borrow it holds on the
/// map would outlive the `&mut self` resolver calls that follow each insert.
///
/// \return true if the insert happened.
fn insert_if_vacant(
    map: &mut HashMap<Atom, PrivateAccessorInfo>,
    key: Atom,
    value: PrivateAccessorInfo,
) -> bool {
    match map.entry(key) {
        Entry::Occupied(_) => false,
        Entry::Vacant(e) => {
            e.insert(value);
            true
        }
    }
}

/// The state a class entry saves so `exit_class` can restore it. C++ keeps
/// this in `ClassContext::prevContext_`; with the stack in the resolver
/// there is nothing left to save, so this type exists purely to make the
/// push/pop pairing a compile-time obligation (like [`super::ScopeState`]).
#[must_use = "every enter_class must be paired with exit_class"]
pub(super) struct ClassState;

/// Port of `ESTree::getSuperClass(ClassLikeNode *)`
/// (`lib/AST/ESTree.cpp:228-238`). C++ returns the `Node *&` so callers can
/// assign through it; nothing in this port needs to, so it is an
/// `Option<&Node>` (C++'s null being `None`).
fn class_like_super_class<'gc>(node: &'gc Node<'gc>) -> Option<&'gc Node<'gc>> {
    match node {
        Node::ClassExpression(n) => n.super_class,
        Node::ClassDeclaration(n) => n.super_class,
        _ => panic!("invalid ClassLikeNode: {}", node.node_type_str()),
    }
}

/// Port of `ESTree::getClassID(ClassLikeNode *)`
/// (`lib/AST/ESTree.cpp:240-253`), including its
/// `dyn_cast_or_null<IdentifierNode>`: a `ClassDeclaration`'s id is optional
/// (`export default class {}`) and a `ClassExpression`'s always is.
fn class_like_id<'gc>(node: &'gc Node<'gc>) -> Option<&'gc Node<'gc>> {
    let id = match node {
        Node::ClassExpression(n) => n.id,
        Node::ClassDeclaration(n) => n.id,
        _ => panic!("invalid ClassLikeNode: {}", node.node_type_str()),
    };
    match id {
        Some(n) if matches!(n, Node::Identifier(_)) => Some(n),
        _ => None,
    }
}

/// Port of `ESTree::getClassBody(ClassLikeNode *)`
/// (`lib/AST/ESTree.cpp:255-265`), whose `cast<ClassBodyNode>` is the
/// `expect` below.
fn class_like_body<'gc>(node: &'gc Node<'gc>) -> &'gc Node<'gc> {
    let body = match node {
        Node::ClassExpression(n) => n.body,
        Node::ClassDeclaration(n) => n.body,
        _ => panic!("invalid ClassLikeNode: {}", node.node_type_str()),
    };
    assert!(
        matches!(body, Node::ClassBody(_)),
        "a ClassLikeNode's body is not a ClassBody"
    );
    body
}

/// Port of `ESTree::getDecorators(ClassLikeNode *)`
/// (`lib/AST/ESTree.cpp:267-277`), reduced to the one thing its caller asks
/// (`.empty()`, cpp:914).
fn class_like_has_decorators(node: &Node) -> bool {
    let decorators = match node {
        Node::ClassExpression(n) => n.decorators,
        Node::ClassDeclaration(n) => n.decorators,
        _ => panic!("invalid ClassLikeNode: {}", node.node_type_str()),
    };
    !decorators.is_empty()
}

/// Read `ClassLikeDecoration::implicitCtorFunctionInfo`, for the assert in
/// `create_implicit_constructor_function_info` (cpp:3094).
fn class_like_implicit_ctor(node: &Node) -> Option<SemaId> {
    match node {
        Node::ClassExpression(n) => n.implicit_ctor_function_info.get(),
        Node::ClassDeclaration(n) => n.implicit_ctor_function_info.get(),
        _ => panic!("invalid ClassLikeNode: {}", node.node_type_str()),
    }
}

/// Port of `classDecoration->implicitCtorFunctionInfo = implicitCtor`
/// (cpp:3113).
fn set_class_like_implicit_ctor(node: &Node, info: FunctionInfoId) {
    let id = Some(info.sema_id());
    match node {
        Node::ClassExpression(n) => n.implicit_ctor_function_info.set(id),
        Node::ClassDeclaration(n) => n.implicit_ctor_function_info.set(id),
        _ => panic!("invalid ClassLikeNode: {}", node.node_type_str()),
    }
}

/// Read `ClassLikeDecoration::instanceElementsInitFunctionInfo` (cpp:3118).
fn class_like_instance_elements_init(node: &Node) -> Option<SemaId> {
    match node {
        Node::ClassExpression(n) => {
            n.instance_elements_init_function_info.get()
        }
        Node::ClassDeclaration(n) => {
            n.instance_elements_init_function_info.get()
        }
        _ => panic!("invalid ClassLikeNode: {}", node.node_type_str()),
    }
}

/// Port of `classDecoration->instanceElementsInitFunctionInfo = ...`
/// (cpp:3135).
fn set_class_like_instance_elements_init(node: &Node, info: FunctionInfoId) {
    let id = Some(info.sema_id());
    match node {
        Node::ClassExpression(n) => {
            n.instance_elements_init_function_info.set(id)
        }
        Node::ClassDeclaration(n) => {
            n.instance_elements_init_function_info.set(id)
        }
        _ => panic!("invalid ClassLikeNode: {}", node.node_type_str()),
    }
}

/// Read `ClassLikeDecoration::staticElementsInitFunctionInfo` (cpp:3142).
fn class_like_static_elements_init(node: &Node) -> Option<SemaId> {
    match node {
        Node::ClassExpression(n) => n.static_elements_init_function_info.get(),
        Node::ClassDeclaration(n) => n.static_elements_init_function_info.get(),
        _ => panic!("invalid ClassLikeNode: {}", node.node_type_str()),
    }
}

/// Port of `classDecoration->staticElementsInitFunctionInfo = ...`
/// (cpp:3160).
fn set_class_like_static_elements_init(node: &Node, info: FunctionInfoId) {
    let id = Some(info.sema_id());
    match node {
        Node::ClassExpression(n) => {
            n.static_elements_init_function_info.set(id)
        }
        Node::ClassDeclaration(n) => {
            n.static_elements_init_function_info.set(id)
        }
        _ => panic!("invalid ClassLikeNode: {}", node.node_type_str()),
    }
}

/// Rebuild a class node with the (possibly) replaced `_superClass`/`_body`.
///
/// C++ needs no counterpart: `visitESTreeNode(*this, getSuperClass(node),
/// node)` writes any replacement straight into the field it was handed. The
/// builder is created HERE, i.e. by the caller *after* its last decoration
/// write, which is what carries the three `ClassLikeDecoration` `Cell`s onto
/// the rebuilt node — see the module doc.
fn build_class_replacement<'gc>(
    gc: &'gc GCLock,
    node: &'gc Node<'gc>,
    super_class: Option<&'gc Node<'gc>>,
    body: Option<&'gc Node<'gc>>,
) -> TransformResult<&'gc Node<'gc>> {
    match node {
        Node::ClassDeclaration(n) => {
            let mut b = builder::ClassDeclaration::from_node(n);
            if let Some(v) = super_class {
                b.super_class(Some(v));
            }
            if let Some(v) = body {
                b.body(v);
            }
            b.build(gc)
        }
        Node::ClassExpression(n) => {
            let mut b = builder::ClassExpression::from_node(n);
            if let Some(v) = super_class {
                b.super_class(Some(v));
            }
            if let Some(v) = body {
                b.body(v);
            }
            b.build(gc)
        }
        _ => panic!("invalid ClassLikeNode: {}", node.node_type_str()),
    }
}

impl<'bt, 'sc, 'sm, 'ad> SemanticResolver<'bt, 'sc, 'sm, 'ad> {
    // ---- ClassContext ----------------------------------------------------

    /// Port of `ClassContext::ClassContext` (SemanticResolver.cpp:3081-3086).
    fn enter_class<'gc>(
        &mut self,
        gc: &'gc GCLock,
        class_node: &'gc Node<'gc>,
    ) -> ClassState {
        self.class_stack.push(ClassContext {
            has_constructor: false,
            class_node: NodeRc::from_node(gc, class_node),
        });
        ClassState
    }

    /// Port of `ClassContext::~ClassContext` (cpp:3179-3181).
    fn exit_class(&mut self, _state: ClassState) {
        self.class_stack.pop().expect("no active class context");
    }

    /// Port of `SemanticResolver::curClassContext_` dereferenced, i.e. the
    /// innermost live `ClassContext`. C++ would fault on a null
    /// `curClassContext_`; every caller is inside a class body, so an empty
    /// stack is a programming error rather than a language construct.
    fn cur_class_context(&self) -> &ClassContext {
        self.class_stack.last().expect("no active class context")
    }

    /// Mutable form of [`Self::cur_class_context`] — the port of
    /// `curClassContext_->hasConstructor = true` (cpp:1656), whose only
    /// writer is `functions.rs`'s `visit_function_like`.
    pub(super) fn cur_class_context_mut(&mut self) -> &mut ClassContext {
        self.class_stack.last_mut().expect("no active class context")
    }

    /// \return true if the current class of this context is a derived class.
    /// Port of `ClassContext::isDerivedClass` (SemanticResolver.h:659-662).
    ///
    /// `pub(super)` because `functions.rs`'s `visit_function_like` reads it
    /// for the constructor kind (cpp:1657).
    pub(super) fn cur_class_is_derived(&self, gc: &GCLock) -> bool {
        // It's a derived class if it has a super class node.
        let class_node_rc = self.cur_class_context().class_node.clone();
        class_like_super_class(class_node_rc.node(gc)).is_some()
    }

    /// The `ClassContext`'s class node as an owned `NodeRc`.
    ///
    /// `NodeRc::node` ties its result to the `&self` it was reached through
    /// (`context.rs:938`), so reading the node straight out of the context
    /// would keep `self` borrowed and forbid the `self.sem_ctx` mutations
    /// every caller below performs. A `NodeRc` clone is a refcount bump, and
    /// the reference taken from *it* borrows only the local. Same pattern as
    /// `identifiers.rs`'s `node_rc` clone.
    fn cur_class_node_rc(&self) -> NodeRc {
        self.cur_class_context().class_node.clone()
    }

    /// Port of `ClassContext::createImplicitConstructorFunctionInfo`
    /// (cpp:3088-3114).
    ///
    /// May only be called after the body of the current class has been
    /// visited, and `has_constructor` is valid. If the current class has no
    /// explicit constructor, creates a `FunctionInfo` for an implicit
    /// constructor and stores it in the class node's decoration.
    fn create_implicit_constructor_function_info(&mut self, gc: &GCLock) {
        // Do nothing if the class has an explicit constructor.
        if self.cur_class_context().has_constructor {
            return;
        }
        let class_node_rc = self.cur_class_node_rc();
        let class_node = class_node_rc.node(gc);
        debug_assert!(class_like_implicit_ctor(class_node).is_none());
        // C++ reads `resolver_.curClassContext_->isDerivedClass()`, which is
        // `this` at every call site (the context is still live).
        let cons_kind = if self.cur_class_is_derived(gc) {
            ConstructorKind::Derived
        } else {
            ConstructorKind::Base
        };
        let parent = self.cur_function_info();
        let implicit_ctor = self.sem_ctx.new_function(
            FuncIsArrow::No,
            cons_kind,
            Some(parent),
            self.cur_scope,
            /* strict */ true,
            CustomDirectives::default(),
        );
        // This is called for the side effect of associating the new scope
        // with implicitCtor. We don't need the value now, but we will later.
        // Treat this new scope as the function body scope.
        let lex_scope = self.sem_ctx.new_scope(implicit_ctor, self.cur_scope);
        if DEBUG_INFO_SETTING_ALL {
            let ptr = self.binding_table.current_scope();
            self.sem_ctx.scope_mut(lex_scope).binding_table_scope = ptr;
        }
        let idx = self.sem_ctx.function(implicit_ctor).get_scopes().len() as u32
            - 1;
        self.sem_ctx.function_mut(implicit_ctor).function_body_scope_idx = idx;
        set_class_like_implicit_ctor(class_node, implicit_ctor);
    }

    /// Port of `ClassContext::getOrCreateInstanceElementsInitFunctionInfo`
    /// (cpp:3116-3138). On first call, creates a `FunctionInfo` for an
    /// implicit function to do the instance elements initializations. On
    /// subsequent calls, return that `FunctionInfo`.
    fn get_or_create_instance_elements_init_function_info(
        &mut self,
        gc: &GCLock,
    ) -> FunctionInfoId {
        let class_node_rc = self.cur_class_node_rc();
        let class_node = class_node_rc.node(gc);
        if class_like_instance_elements_init(class_node).is_none() {
            let field_init_func = self.new_elements_init_function_info();
            set_class_like_instance_elements_init(class_node, field_init_func);
        }
        FunctionInfoId::from_sema_id(
            class_like_instance_elements_init(class_node)
                .expect("just set, or already present"),
        )
    }

    /// Port of `ClassContext::getOrCreateStaticElementsInitFunctionInfo`
    /// (cpp:3140-3163) — get or create a synthetic function information for
    /// the static elements initializer of a class.
    fn get_or_create_static_elements_init_function_info(
        &mut self,
        gc: &GCLock,
    ) -> FunctionInfoId {
        let class_node_rc = self.cur_class_node_rc();
        let class_node = class_node_rc.node(gc);
        if class_like_static_elements_init(class_node).is_none() {
            let static_field_init_func = self.new_elements_init_function_info();
            set_class_like_static_elements_init(
                class_node,
                static_field_init_func,
            );
        }
        FunctionInfoId::from_sema_id(
            class_like_static_elements_init(class_node)
                .expect("just set, or already present"),
        )
    }

    /// The body the two `getOrCreate...ElementsInitFunctionInfo` getters
    /// share (cpp:3119-3134 and cpp:3143-3159 are the same code up to the
    /// local variable's name and the decoration field it lands in — this
    /// port factors it out rather than duplicating it, since the two C++
    /// bodies are textually identical).
    fn new_elements_init_function_info(&mut self) -> FunctionInfoId {
        let parent = self.cur_function_info();
        let field_init_func = self.sem_ctx.new_function(
            FuncIsArrow::No,
            ConstructorKind::None,
            Some(parent),
            self.cur_scope,
            /* strict */ true,
            CustomDirectives::default(),
        );
        // This is called for the side effect of associating the new scope
        // with fieldInitFunc. We don't need the value now, but we will
        // later. Treat this new scope as the function body scope.
        let lex_scope = self.sem_ctx.new_scope(field_init_func, self.cur_scope);
        if DEBUG_INFO_SETTING_ALL {
            let ptr = self.binding_table.current_scope();
            self.sem_ctx.scope_mut(lex_scope).binding_table_scope = ptr;
        }
        let idx = self.sem_ctx.function(field_init_func).get_scopes().len()
            as u32
            - 1;
        self.sem_ctx
            .function_mut(field_init_func)
            .function_body_scope_idx = idx;
        field_init_func
    }

    /// Port of `ClassContext::createStaticBlockFunctionInfo`
    /// (cpp:3165-3177) — create a synthetic function information for a
    /// static initialization block.
    ///
    /// Unlike the three above, the id lands on the `StaticBlock` node's own
    /// `function_info` `Cell` (see the module doc), and no scope is created
    /// here: `visit(StaticBlockNode *)`'s `ScopeRAII{..., /*
    /// isFunctionBodyScope */ true}` (cpp:1063) makes it. Its only caller is
    /// [`Self::visit_static_block`] (S2 T5).
    fn create_static_block_function_info(
        &mut self,
        node: &Node,
    ) -> FunctionInfoId {
        let parent = self.cur_function_info();
        let static_block_func = self.sem_ctx.new_function(
            FuncIsArrow::No,
            ConstructorKind::None,
            Some(parent),
            self.cur_scope,
            /* strict */ true,
            CustomDirectives::default(),
        );
        self.sem_ctx.function_mut(static_block_func).is_static_block = true;
        let block = node
            .as_static_block()
            .expect("create_static_block_function_info: not a StaticBlock");
        block.function_info.set(Some(static_block_func.sema_id()));
        static_block_func
    }

    // ---- visit(ClassDeclarationNode *) -----------------------------------

    /// Port of `SemanticResolver::visit(ESTree::ClassDeclarationNode *node)`
    /// (SemanticResolver.cpp:891-907).
    pub(super) fn visit_class_declaration<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        if TYPED {
            // Classes must be in strict mode.
            //   llvh::SaveAndRestore<bool> oldStrict{
            //       curFunctionInfo()->strict, true};
            //   ClassContext classCtx(*this, node);
            //   ScopeRAII scope{*this, node};
            //   collectDeclaredPrivateIdentifiers(node);
            //   visitESTreeChildren(*this, node);
            //   if (LLVM_UNLIKELY(recursionDepth_ == 0))
            //     return;
            //   curClassContext_->createImplicitConstructorFunctionInfo();
            //
            // The typed path differs from the untyped one below in more than
            // the class-body variable: it walks the children generically
            // (visiting `_typeParameters`, `_superTypeArguments` and
            // `_implements`, all of which are type nodes this port has no
            // visits for). Porting it needs the typed-dialect track, so it
            // is a panic rather than a guess — `TYPED` is a constant
            // `false`, so this is dead code that documents the shape.
            //
            // NOTE for whoever ports it: `visitESTreeChildren` cannot become
            // a plain `node.visit_children_mut(gc, self)` here, because
            // `createImplicitConstructorFunctionInfo` runs AFTER it and
            // `visit_children_mut` snapshots the class node's `Cell`s at the
            // start — the decoration would be dropped on any rebuild. It
            // needs the same hand-driven "build last" treatment
            // `visit_class_as_expr` uses; see the module doc.
            panic!(
                "sema: typed-mode class declarations need the typed-dialect \
                 track (cpp:892-901)"
            );
        }
        // In untyped mode, create an additional scope & variable for the
        // class body, which obeys const variable rules.
        self.visit_class_as_expr(gc, node)
    }

    // ---- visit(ClassExpressionNode *) ------------------------------------

    /// Port of `SemanticResolver::visit(ESTree::ClassExpressionNode *node)`
    /// (cpp:909-911).
    pub(super) fn visit_class_expression<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        self.visit_class_as_expr(gc, node)
    }

    // ---- visitClassAsExpr -----------------------------------------------

    /// Port of `SemanticResolver::visitClassAsExpr` (cpp:913-950).
    ///
    /// See the module doc for why the builder is created at the very end
    /// rather than up front.
    fn visit_class_as_expr<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        if self.compile() && class_like_has_decorators(node) {
            self.sm
                .error_range(node.range(), "decorators are not supported");
        }

        // Classes must be in strict mode.
        //   llvh::SaveAndRestore<bool> oldStrict{
        //       curFunctionInfo()->strict, true};
        // The reference the C++ `SaveAndRestore` binds is
        // `curFunctionInfo()->strict` evaluated ONCE, at construction, so
        // the restore targets that same `FunctionInfo` — hence the saved id.
        let strict_func = self.cur_function_info();
        let saved_strict = self.sem_ctx.function(strict_func).strict;
        self.sem_ctx.function_mut(strict_func).strict = true;

        let class_state = self.enter_class(gc, node);
        // Declare a new scope where we will put private names.
        let scope_state = self.enter_scope(Some(node), false);

        if let Some(ident_node) = class_like_id(node) {
            // If there is a name, declare it.
            if self.validate_declaration_name(
                gc,
                DeclKind::ClassExprName,
                ident_node,
            ) {
                let ident = ident_node
                    .as_identifier()
                    .expect("class_like_id only yields Identifiers");
                let name = ident.name.get();
                let cur_scope = self.cur_scope.expect("just entered a scope");
                let decl = self.sem_ctx.new_decl_in_scope_default(
                    name,
                    DeclKind::ClassExprName,
                    cur_scope,
                );
                // We declare this as an expression decl so that in the case
                // of class declarations, we can associate two different
                // decls with a single identifier node. The class body will
                // see this inner ClassExprName decl, which obeys const
                // variable rules.
                self.sem_ctx.set_expression_decl(
                    ident_node.node_id(),
                    ident,
                    Some(decl),
                );
                self.binding_table.try_emplace(
                    name,
                    Binding::new(
                        decl,
                        Some(NodeRc::from_node(gc, ident_node)),
                    ),
                );
            }
        }
        // Visit the super class expression before declaring private names,
        // but after the class name was declared.
        let super_repl = match class_like_super_class(node) {
            Some(super_class) => replacement_of(self.call(
                gc,
                super_class,
                Some(Path::new(node, NodeField::super_class)),
            )),
            None => None,
        };
        self.collect_declared_private_identifiers(gc, node);
        // Visit the body node.
        //
        // C++ dyn_casts to pick `CD->_body` vs `CE->_body` purely because
        // `ClassLikeNode` has no common accessor for it; `class_like_body`
        // is that accessor, so the two arms collapse.
        let body_repl = replacement_of(self.call(
            gc,
            class_like_body(node),
            Some(Path::new(node, NodeField::body)),
        ));
        if self.recursion_depth != 0 {
            self.create_implicit_constructor_function_info(gc);
        }
        // The builder must be created after the write above and after the
        // two child walks (which write the elements-init decorations) — see
        // the module doc.
        let result = build_class_replacement(gc, node, super_repl, body_repl);

        self.exit_scope(scope_state);
        self.exit_class(class_state);
        self.sem_ctx.function_mut(strict_func).strict = saved_strict;
        result
    }

    /// Port of `SemanticResolver::collectDeclaredPrivateIdentifiers`
    /// (cpp:2143-2260) — the ES2024 15.7.1 early-error machinery for private
    /// names, run over the class body BEFORE the body is visited (cpp:939,
    /// from `visitClassAsExpr`), so that every private reference
    /// anywhere inside the class — including in a member declared earlier
    /// than the one it names — resolves.
    ///
    /// Declare and validate the class private names. Enforce the rules laid
    /// out in ES2024 15.7.1 Early Errors: It is a Syntax Error if declared
    /// private names contains any duplicate entries, unless the name is used
    /// once for a getter and once for a setter and in no other entries, and
    /// the getter and setter are either both static or both non-static.
    fn collect_declared_private_identifiers<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) {
        // Map a private name to accessor information.
        let mut private_declarations: HashMap<Atom, PrivateAccessorInfo> =
            HashMap::new();

        const DEFAULT_DUP_ERR_MSG: &str =
            "Duplicate private identifier declaration.";
        let body = class_like_body(node)
            .as_class_body()
            .expect("class_like_body checked the kind");
        for elm in body.body.iter() {
            if let Node::ClassPrivateProperty(prop) = elm {
                // C++'s `cast<IdentifierNode>(prop->_key)`: unlike a
                // `MethodDefinition`, a `ClassPrivateProperty` stores the
                // bare `Identifier` as its key, with no `PrivateName`
                // wrapper (ESTree.def:562-564).
                let id_node = prop.key;
                let id = id_node
                    .as_identifier()
                    .expect("a ClassPrivateProperty key is an Identifier");
                // If we did not complete the insert, that means something
                // already exists with this identifier. Fields are not
                // allowed to be duplicated at all.
                if !insert_if_vacant(
                    &mut private_declarations,
                    id.name.get(),
                    PrivateAccessorInfo::default(),
                ) {
                    self.sm.error_range(id_node.range(), DEFAULT_DUP_ERR_MSG);
                } else {
                    self.declare_private_name(
                        gc,
                        id_node,
                        DeclKind::PrivateField,
                        /* isStatic */ false,
                    );
                }
                continue;
            }
            if let Node::MethodDefinition(method) = elm {
                let Node::PrivateName(private_name) = method.key else {
                    continue;
                };
                let id_node = private_name.id;
                let id = id_node
                    .as_identifier()
                    .expect("a PrivateName's id is an Identifier");
                let name = id.name.get();
                let meth_kind = method.kind.get();
                if meth_kind == self.kw().ident_method {
                    // Private methods decorated with @Hermes.overload may
                    // legally have duplicate names. The FlowChecker
                    // validates that all overloads of a given name are
                    // decorated and merges them into a single Field. The
                    // first overload creates the binding; subsequent
                    // overloads share it.
                    //
                    //   bool isOverload =
                    //       typed_ &&
                    //       hermes::findDecorator(
                    //           method->_decorators,
                    //           {kw_.identHermes, kw_.identOverload});
                    //
                    // `hermes::findDecorator` (`lib/AST/ESTree.cpp`) is part
                    // of the typed-dialect track, like the rest of the
                    // decorator machinery; `TYPED` is a constant `false`
                    // here, so the `&&` short-circuits and the call is
                    // never made. Ported as a panic rather than a guess, the
                    // same way `visit_class_declaration`'s typed branch is.
                    let is_overload = if TYPED {
                        panic!(
                            "sema: @Hermes.overload private methods need \
                             the typed-dialect track (cpp:2197-2200)"
                        )
                    } else {
                        false
                    };
                    let inserted = insert_if_vacant(
                        &mut private_declarations,
                        name,
                        PrivateAccessorInfo::default(),
                    );
                    if !inserted {
                        let existing = private_declarations[&name];
                        if !is_overload || !existing.is_overloaded_method {
                            self.sm.error_range(
                                id_node.range(),
                                DEFAULT_DUP_ERR_MSG,
                            );
                        }
                        // Resolve this private name's identifier to the
                        // existing decl.
                        self.resolve_private_name(gc, id_node);
                    } else {
                        private_declarations
                            .get_mut(&name)
                            .expect("just inserted")
                            .is_overloaded_method = is_overload;
                        self.declare_private_name(
                            gc,
                            id_node,
                            DeclKind::PrivateMethod,
                            method.r#static.get(),
                        );
                    }
                    continue;
                }
                debug_assert!(
                    meth_kind == self.kw().ident_set
                        || meth_kind == self.kw().ident_get,
                    "unrecognized method kind."
                );
                let is_setter = meth_kind == self.kw().ident_set;
                let cur_info = PrivateAccessorInfo {
                    is_accessor: true,
                    is_static: method.r#static.get(),
                    is_getter: !is_setter,
                    is_setter,
                    is_overloaded_method: false,
                    original_name_decl: None,
                };
                // `DenseMap::insert` never overwrites an existing entry, so
                // this is the same "insert only if vacant" primitive the two
                // `try_emplace`es above use.
                if insert_if_vacant(&mut private_declarations, name, cur_info) {
                    // We successfully inserted, so there's no possibility
                    // for an error. Save the decl made here for reuse later
                    // in the case of a pair of accessors.
                    let decl = self.declare_private_name(
                        gc,
                        id_node,
                        if is_setter {
                            DeclKind::PrivateSetter
                        } else {
                            DeclKind::PrivateGetter
                        },
                        method.r#static.get(),
                    );
                    private_declarations
                        .get_mut(&name)
                        .expect("just inserted")
                        .original_name_decl = Some(decl);
                    continue;
                }
                // There is already an private declaration of this
                // identifier. The only way this can be legal is if it is
                // also an accessor which is the complement accessor kind of
                // what we are trying to define here, with the same
                // static-ness. E.g., if we are trying to define a static
                // getter, then the only accepted duplicate is a static
                // setter.
                let existing_info = private_declarations[&name];
                if !existing_info.is_accessor
                    || (cur_info.is_setter && existing_info.is_setter)
                    || (cur_info.is_getter && existing_info.is_getter)
                {
                    self.sm.error_range(id_node.range(), DEFAULT_DUP_ERR_MSG);
                    continue;
                }
                if cur_info.is_static != existing_info.is_static {
                    self.sm.error_range(
                        id_node.range(),
                        "static and non-static private accessor with the \
                         same name",
                    );
                    continue;
                }
                // We can only survive through one duplicate declaration on
                // the same name. After this, we know this private name has
                // both a setter and getter.
                let original_name_decl = existing_info
                    .original_name_decl
                    .expect("an accessor entry always records its decl");
                {
                    let existing_info = private_declarations
                        .get_mut(&name)
                        .expect("looked up just above");
                    existing_info.is_getter = true;
                    existing_info.is_setter = true;
                }
                // NOTE: this MUTATES an already-created `Decl`'s kind
                // (cpp:2255). It is exempt from `resolver/mod.rs`'s
                // "decorate before recursing" invariant, which constrains
                // writes to `Cell` decorations ON AST NODES: a `Decl` lives
                // in `SemContext`, which no node rebuild ever snapshots or
                // copies, so the mutation is visible no matter when it
                // happens.
                self.sem_ctx.decl_mut(original_name_decl).kind =
                    DeclKind::PrivateGetterSetter;
                // Make sure to bind this id node to the correct decl.
                self.sem_ctx.set_both_decl(
                    id_node.node_id(),
                    id,
                    Some(original_name_decl),
                );
            }
        }
    }

    // ---- visit(PrivateNameNode *) ----------------------------------------

    /// Port of `SemanticResolver::visit(ESTree::PrivateNameNode *node)`
    /// (cpp:952-963).
    ///
    /// This is the ONLY place the "was not declared in any enclosing class"
    /// diagnostic is reported, which is why the `MemberExpression` branches
    /// (cpp:1224-1225) can silently skip their extra validation when
    /// `resolvePrivateName` fails: this visit runs on the same
    /// `PrivateNameNode` right afterwards, from `visitESTreeChildren`.
    pub(super) fn visit_private_name<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let private_name = node
            .as_private_name()
            .expect("visit_private_name: not a PrivateName");
        // C++'s `cast<IdentifierNode>(node->_id)`.
        let identifier_node = private_name.id;
        let decl = self.resolve_private_name(gc, identifier_node);
        if decl.is_none() {
            // Failed to resolve.
            let name = identifier_node
                .as_identifier()
                .expect("a PrivateName's id is an Identifier")
                .name
                .get();
            let name = String::from_utf8_lossy(gc.bytes(name)).into_owned();
            self.sm.error_range(
                identifier_node.range(),
                format!(
                    "the private name \"#{name}\" was not declared in any \
                     enclosing class"
                ),
            );
        }
        // `visitESTreeChildren(*this, node)`: a `PrivateName`'s only child is
        // `_id`, and `visit(IdentifierNode *, Node *)` returns early for a
        // `PrivateName` parent (cpp:316-320) — "Identifiers belonging to a
        // PrivateNameNode are validated in the PrivateNameNode visitor" —
        // so this can never rebuild anything. It is kept because the C++
        // keeps it.
        node.visit_children_mut(gc, self)
    }

    // ---- visit(ClassPrivatePropertyNode *) -------------------------------

    /// Port of `SemanticResolver::visit(ESTree::ClassPrivatePropertyNode
    /// *node)` (cpp:965-1006).
    ///
    /// The same shape as `visit(ClassPropertyNode *)` (see
    /// [`Self::visit_class_property`]) minus the computed-key branch: a
    /// private property's key can never be computed, and — note — the key is
    /// never VISITED either. It was already declared and bound (with
    /// `setBothDecl`) by `collect_declared_private_identifiers`, so visiting
    /// it would only try to resolve `x` as an ordinary variable.
    pub(super) fn visit_class_private_property<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let prop = node.as_class_private_property().expect(
            "visit_class_private_property: not a ClassPrivateProperty",
        );
        if self.compile() && !prop.decorators.is_empty() {
            self.sm
                .error_range(node.range(), "decorators are not supported");
        }

        // Visit the init expression, since it needs to be resolved.
        let mut value_repl = None;
        if let Some(value) = prop.value {
            // We visit the initializer expression in the context of a
            // synthesized method that performs the initializations.
            // Field initializers can always reference super.
            let saved_can_ref_super = self.can_reference_super;
            self.can_reference_super = true;
            let saved_forbid_await = self.forbid_await_expression;
            self.forbid_await_expression = true;
            // ES14.0 15.7.1
            // It is a Syntax Error if Initializer is present and
            // ContainsArguments of Initializer is true.
            let saved_forbid_arguments =
                self.forbid_special_arguments_reference;
            self.forbid_special_arguments_reference = true;

            let sem_info = if prop.r#static.get() {
                self.get_or_create_static_elements_init_function_info(gc)
            } else {
                self.get_or_create_instance_elements_init_function_info(gc)
            };
            let func_state = self.enter_function_with_info(sem_info);
            // We need to make sure that the special `arguments` object is
            // declared so that we can detect usages of it, and correctly
            // error out since field initializers are not allowed to
            // reference `arguments`. If we didn't do this then a class in
            // the global scope would allow a field initializer to reference
            // `arguments`, since it would treat it as a normal identifier.
            // This will insert the `arguments` identifer into the binding
            // table scope which is created by the class declaration /
            // expression node.
            self.declare_arguments();
            value_repl = replacement_of(self.call(
                gc,
                value,
                Some(Path::new(node, NodeField::value)),
            ));
            self.exit_function(func_state);

            self.forbid_special_arguments_reference = saved_forbid_arguments;
            self.forbid_await_expression = saved_forbid_await;
            self.can_reference_super = saved_can_ref_super;
        } else if !TYPED {
            // Create the these initializers even if no value initializer is
            // present, in untyped mode. Typed classes don't need these
            // initializers since we know the exact shape and construct it up
            // front.
            if prop.r#static.get() {
                self.get_or_create_static_elements_init_function_info(gc);
            } else {
                self.get_or_create_instance_elements_init_function_info(gc);
            }
        }
        build_class_private_property(gc, prop, value_repl)
    }

    // ---- visit(StaticBlockNode *) ----------------------------------------

    /// Port of `SemanticResolver::visit(ESTree::StaticBlockNode *node)`
    /// (cpp:1053-1084).
    ///
    /// Both `Cell`s this writes on `node` (`function_info`, from
    /// `create_static_block_function_info`, and `scope`, from `enter_scope`)
    /// are written BEFORE the children are visited, so the
    /// "decorate before recursing" invariant holds as stated — a fold or an
    /// arrow rewrite anywhere in the block rebuilds it and carries both. The
    /// OTHER decoration this visit causes, the static-elements-init id from
    /// cpp:1057, lands on the CLASS node instead, and it is
    /// `visit_class_as_expr` that keeps that one alive across a rebuild (see
    /// the module doc's decorate-after-children section).
    pub(super) fn visit_static_block<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        // Initialize the static elements info even though we don't use it as
        // the function info for the static block itself. IRGen needs this
        // info to generate IR for any static elements.
        self.get_or_create_static_elements_init_function_info(gc);
        // StaticBlockNodes should be treated as if they are actually a
        // function-level scope. For example, `var` declarations do not get
        // hoisted up to the function that the class resides in, but rather to
        // this static block scope.
        let static_block_info = self.create_static_block_function_info(node);
        let func_state =
            self.enter_function_static_block(gc, node, static_block_info);
        let scope_state =
            self.enter_scope(Some(node), /* isFunctionBodyScope */ true);
        self.process_collected_declarations(gc, node);
        if DEBUG_INFO_SETTING_ALL {
            // Store the current scope, for compiling children of this static
            // block node in 'eval'.
            let ptr = self.binding_table.current_scope();
            self.sem_ctx
                .function_mut(static_block_info)
                .binding_table_scope = ptr;
        }

        // ES14.0 15.7.1
        // It is a Syntax Error if ClassStaticBlockStatementList Contains
        // await is true.
        let saved_forbid_await = self.forbid_await_expression;
        self.forbid_await_expression = true;
        // Using await as an identifier is forbidden.
        let saved_forbid_await_ident = self.forbid_await_as_identifier;
        self.forbid_await_as_identifier = true;
        // Disallow 'arguments' usage as an identifier in static blocks.
        let saved_forbid_arguments_ident = self.forbid_arguments_as_identifier;
        self.forbid_arguments_as_identifier = true;
        // Static blocks can always reference super.
        let saved_can_ref_super = self.can_reference_super;
        self.can_reference_super = true;
        let result = node.visit_children_mut(gc, self);
        self.can_reference_super = saved_can_ref_super;
        self.forbid_arguments_as_identifier = saved_forbid_arguments_ident;
        self.forbid_await_as_identifier = saved_forbid_await_ident;
        self.forbid_await_expression = saved_forbid_await;

        self.exit_scope(scope_state);
        self.exit_function(func_state);
        result
    }

    // ---- visit(ClassPropertyNode *) --------------------------------------

    /// Port of `SemanticResolver::visit(ESTree::ClassPropertyNode *node)`
    /// (cpp:1008-1051).
    pub(super) fn visit_class_property<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let prop = node
            .as_class_property()
            .expect("visit_class_property: not a ClassProperty");
        if self.compile() && !prop.decorators.is_empty() {
            self.sm
                .error_range(node.range(), "decorators are not supported");
        }

        // If computed property, the key expression needs to be resolved.
        let mut key_repl = None;
        if prop.computed.get() {
            // Computed keys cannot reference super.
            let saved_can_ref_super = self.can_reference_super;
            self.can_reference_super = false;
            key_repl = replacement_of(self.call(
                gc,
                prop.key,
                Some(Path::new(node, NodeField::key)),
            ));
            self.can_reference_super = saved_can_ref_super;
            if self.recursion_depth == 0 {
                return build_class_property(gc, prop, key_repl, None);
            }
        }

        // Visit the init expression, since it needs to be resolved.
        let mut value_repl = None;
        if let Some(value) = prop.value {
            // We visit the initializer expression in the context of a
            // synthesized method that performs the initializations.
            // Field initializers can always reference super.
            let saved_can_ref_super = self.can_reference_super;
            self.can_reference_super = true;
            let saved_forbid_await = self.forbid_await_expression;
            self.forbid_await_expression = true;
            // ES14.0 15.7.1
            // It is a Syntax Error if Initializer is present and
            // ContainsArguments of Initializer is true.
            let saved_forbid_arguments =
                self.forbid_special_arguments_reference;
            self.forbid_special_arguments_reference = true;

            let sem_info = if prop.r#static.get() {
                self.get_or_create_static_elements_init_function_info(gc)
            } else {
                self.get_or_create_instance_elements_init_function_info(gc)
            };
            let func_state = self.enter_function_with_info(sem_info);
            // We need to make sure that the special `arguments` object is
            // declared so that we can detect usages of it, and correctly
            // error out since field initializers are not allowed to
            // reference `arguments`. If we didn't do this then a class in
            // the global scope would allow a field initializer to reference
            // `arguments`, since it would treat it as a normal identifier.
            // This will insert the `arguments` identifer into the binding
            // table scope which is created by the class declaration /
            // expression node.
            self.declare_arguments();
            value_repl = replacement_of(self.call(
                gc,
                value,
                Some(Path::new(node, NodeField::value)),
            ));
            self.exit_function(func_state);

            self.forbid_special_arguments_reference = saved_forbid_arguments;
            self.forbid_await_expression = saved_forbid_await;
            self.can_reference_super = saved_can_ref_super;
        } else if !TYPED {
            // Create the these initializers even if no value initializer is
            // present, in untyped mode. Typed classes don't need these
            // initializers since we know the exact shape and construct it up
            // front.
            if prop.r#static.get() {
                self.get_or_create_static_elements_init_function_info(gc);
            } else {
                self.get_or_create_instance_elements_init_function_info(gc);
            }
        }
        build_class_property(gc, prop, key_repl, value_repl)
    }

    // ---- visit(MethodDefinitionNode *, Node *) ---------------------------

    /// Port of `SemanticResolver::visit(ESTree::MethodDefinitionNode *node,
    /// ESTree::Node *parent)` (cpp:1094-1115). `parent` is unused by the
    /// C++ body (it exists only because the dispatcher's two-argument
    /// overload was chosen), so it is not a parameter here.
    pub(super) fn visit_method_definition<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let method = node
            .as_method_definition()
            .expect("visit_method_definition: not a MethodDefinition");
        if self.compile() && !TYPED && !method.decorators.is_empty() {
            self.sm
                .error_range(node.range(), "decorators are not supported");
        }

        // If computed property, the key expression needs to be resolved.
        let mut key_repl = None;
        if method.computed.get() {
            key_repl = replacement_of(self.call(
                gc,
                method.key,
                Some(Path::new(node, NodeField::key)),
            ));
        }
        // NOTE: this check is deliberately OUTSIDE the `if` above in the C++
        // (cpp:1104-1105) — a spent depth budget skips the body even for a
        // non-computed key.
        if self.recursion_depth == 0 {
            return build_method_definition(gc, method, key_repl, None);
        }

        // If there are private instance methods, we will need to make an
        // instance elements intializer function.
        //
        // Live as of S2 T5, which is what makes a `PrivateName` key
        // reachable at all.
        if matches!(method.key, Node::PrivateName(_)) && !method.r#static.get()
        {
            self.get_or_create_instance_elements_init_function_info(gc);
        }

        // Visit the body.
        let value_repl = replacement_of(self.call(
            gc,
            method.value,
            Some(Path::new(node, NodeField::value)),
        ));
        build_method_definition(gc, method, key_repl, value_repl)
    }

    // ---- visit(SuperNode *, Node *) --------------------------------------

    /// Port of `SemanticResolver::visit(ESTree::SuperNode *node,
    /// ESTree::Node *parent)` (cpp:1086-1092).
    ///
    /// The C++ body neither visits children (`Super` is an
    /// `ESTREE_NODE_0_ARGS` kind, ESTree.def:275) nor touches
    /// `node` at all — only `parent` — hence the `Unchanged` and the absent
    /// `node` parameter. `super(...)`'s own check lives in
    /// `visit(CallExpressionNode *)` (cpp:1195-1202) — `calls.rs`, S2 T6 —
    /// which is exactly why a `CallExpression` parent is NOT in the
    /// `MemberExpressionLike` range tested below.
    pub(super) fn visit_super<'gc>(
        &mut self,
        path: Option<Path<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>> {
        // Error if we try to reference super but there is currently no valid
        // binding to it.
        //
        // `llvh::isa<MemberExpressionLikeNode>(parent)`: the range
        // `ESTREE_FIRST(MemberExpressionLike, Base)` spans exactly
        // `MemberExpression` and `OptionalMemberExpression`
        // (ESTree.def:360-373). Only the first is reachable — in C++ just as
        // much as here, so the `isa<>` range test has a dead sub-case in
        // both: the parser requires `(`, `[` or `.` immediately after
        // `super` (`super?.a` is `'(', '[' or '.' expected after 'super'
        // keyword`), and in `super.a?.b` the `OptionalMemberExpression`
        // wraps a plain `MemberExpression` whose `_object` is the `Super`.
        // The range test is kept verbatim rather than narrowed to
        // `MemberExpression`, because it is the C++'s condition.
        // A `None` path means `Super` is the root of
        // the walk, which cannot happen (a `Program` is always the root);
        // C++ has no such case at all, since the dispatcher always passes
        // the real parent.
        if let Some(path) = path {
            if matches!(
                path.parent,
                Node::MemberExpression(_) | Node::OptionalMemberExpression(_)
            ) && !self.can_reference_super
            {
                self.sm.error_range(
                    path.parent.range(),
                    "super not allowed here",
                );
            }
        }
        TransformResult::Unchanged
    }
}

/// Rebuild a `ClassProperty` with the (possibly) replaced `_key`/`_value`.
/// C++ needs no counterpart — `visitESTreeNode` writes through the field
/// reference it was handed. `ClassProperty` carries no sema decorations, so
/// unlike the class node itself the builder's snapshot point does not matter.
fn build_class_property<'gc>(
    gc: &'gc GCLock,
    prop: &'gc ast::node::ClassProperty<'gc>,
    key: Option<&'gc Node<'gc>>,
    value: Option<&'gc Node<'gc>>,
) -> TransformResult<&'gc Node<'gc>> {
    let mut b = builder::ClassProperty::from_node(prop);
    if let Some(v) = key {
        b.key(v);
    }
    if let Some(v) = value {
        b.value(Some(v));
    }
    b.build(gc)
}

/// Rebuild a `ClassPrivateProperty` with the (possibly) replaced `_value` —
/// see [`build_class_property`]. There is no `_key` parameter because
/// `visit(ClassPrivatePropertyNode *)` never visits the key.
fn build_class_private_property<'gc>(
    gc: &'gc GCLock,
    prop: &'gc ast::node::ClassPrivateProperty<'gc>,
    value: Option<&'gc Node<'gc>>,
) -> TransformResult<&'gc Node<'gc>> {
    let mut b = builder::ClassPrivateProperty::from_node(prop);
    if let Some(v) = value {
        b.value(Some(v));
    }
    b.build(gc)
}

/// Rebuild a `MethodDefinition` with the (possibly) replaced `_key`/`_value`
/// — see [`build_class_property`].
fn build_method_definition<'gc>(
    gc: &'gc GCLock,
    method: &'gc ast::node::MethodDefinition<'gc>,
    key: Option<&'gc Node<'gc>>,
    value: Option<&'gc Node<'gc>>,
) -> TransformResult<&'gc Node<'gc>> {
    let mut b = builder::MethodDefinition::from_node(method);
    if let Some(v) = key {
        b.key(v);
    }
    if let Some(v) = value {
        b.value(v);
    }
    b.build(gc)
}
