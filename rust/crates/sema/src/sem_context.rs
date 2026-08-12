/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Port of `hermes::sema::{Decl, LexicalScope, FunctionInfo, SemContext}`
//! (`include/hermes/Sema/SemContext.h`, whole file) and the free-standing
//! `SemContext` methods (`lib/Sema/SemContext.cpp:1-415`).
//!
//! ## Deviations (locked design decisions; see the S0 sema spec)
//!
//! - `Decl::customData` (a `void*` slot for consumers to stash their own
//!   data) is NOT ported: consumers keep their own `DeclId`-keyed side
//!   tables instead (spec §3.1 principle). `LexicalScope::customData` is
//!   dropped for the same reason (`FunctionInfo` has no `customData` in
//!   C++, so there is nothing to drop there).
//! - The C++ parent/child `SemContext` tree (`shared_ptr<SemContext>
//!   parent_`, `root_`, `parentLexScope_`, SemContext.h:638-653, used to
//!   share a binding table across `eval` contexts) is out of scope here —
//!   that's S5. This port has exactly one `SemContext`, which is always its
//!   own root; accessors that in C++ go through `root_` (`getGlobalFunction`,
//!   `getGlobalScope`, `getBindingTable`, `getBindingTableGlobalScope`)
//!   therefore read `self` directly.
//! - C++ `std::deque<T>` storages (`functions_`, `scopes_`, `decls_`) become
//!   `Vec<T>` indexed by the typed ids from `hermes_sema::ids`; C++ raw/owning
//!   pointers between these records become the corresponding typed id.
//! - The two side tables that back the identifier decl-state machine
//!   (`sideIdentifierDeclarationDecl_`, `promotedFunctionDecls_`) are keyed
//!   by `hermes_ast::NodeId` here rather than by `ESTree::IdentifierNode *`: the
//!   node itself carries the state bits and value slot (`decl_state`/`decl`
//!   Cells, `rust/crates/ast/src/node.rs:1758`), and `NodeId` is the stable,
//!   non-aliasing identity for a node (see `hermes_ast::NodeId`'s doc comment).
//! - Node backreferences (`hoistedFunctions`, `imports`,
//!   `builtinDeclarations_`) become `hermes_ast::context::NodeRc`, which keeps the
//!   referenced node alive independent of any `GCLock`.
//! - `SourceVisibility`/`CustomDirectives` are ports of
//!   `include/hermes/AST/Context.h:128-166`; they live in `sema` for now
//!   rather than in the (not yet ported) `hermes_ast::Context`.
//! - `SemContext::customData1`/`customData2` (opaque `shared_ptr<void>`
//!   slots for downstream consumers — e.g. IRGen state across lazy
//!   compilation, and a `LexicalScope`-lookup cache — SemContext.h:624-636)
//!   are NOT ported, for the same reason as `Decl::customData`/
//!   `LexicalScope::customData` above: nothing in SemContext.cpp itself
//!   reads or writes them, and consumers can keep their own side tables
//!   instead (spec §3.1 principle). Unlike the other two `customData`
//!   fields, the task brief's field list didn't call these out explicitly;
//!   flagged here for the same reason it flags the other two.

use std::collections::HashMap;
use std::rc::Rc;

use hermes_ast::context::{GCLock, NodeRc};
use hermes_ast::node::{Identifier, Node};
use hermes_ast::{NodeId, SemaId};
use hermes_support::persistent_scoped_map::{PersistentScopedMap, Scope, ScopePtr};

use crate::ids::{DeclId, FunctionInfoId, ScopeId};
use crate::keywords::Keywords;

/// The atom type used throughout sema for identifier/keyword text. Same
/// type as `hermes_ast::node_child::NodeLabel` (both alias `hermes_atom_table::AtomBytes`).
pub type Atom = hermes_atom_table::AtomBytes;

/// Get or create the atom for the string *value* of a private name whose
/// source spelling (the `IdentifierNode::_name` inside a `PrivateNameNode`,
/// which never includes the `#`) is `name`. Port of
/// `Context::getPrivateNameIdentifier` (`include/hermes/AST/Context.h:
/// 389-393`), whose whole body is
/// `getIdentifier(llvh::Twine("#") + str->str())` — i.e. the mangling is
/// exactly a `#` prefix, and it interns into the same string table ordinary
/// identifiers use, which is why a `Decl` named `#x` can never collide with a
/// JS variable named `x`.
///
/// Deviation: C++ hangs this off the AST `Context`, which this port has not
/// ported as a sema-visible object (see `resolver/mod.rs`'s note on
/// `astContext_`); the `GCLock` is what owns the atom table here, so it is a
/// free function in the sema crate taking the lock. Every private-name
/// consumer (`declarePrivateName`/`resolvePrivateName` today, the FlowChecker
/// and IRGen later) must go through it so the mangling stays in one place.
pub fn private_name_identifier(gc: &GCLock, name: Atom) -> Atom {
    let name_bytes = gc.bytes(name);
    let mut mangled = Vec::with_capacity(1 + name_bytes.len());
    mangled.push(b'#');
    mangled.extend_from_slice(name_bytes);
    gc.atom_bytes(mangled)
}

/// Binding between an identifier and its declaration in a scope. Port of
/// `hermes::sema::Binding` (SemContext.h:28-44).
///
/// Deviation: C++ models "no binding" as `decl == nullptr` and provides
/// `isValid()`/`invalidate()` for that. `DeclId` has no null sentinel, so
/// for S0 this only ports the struct and its 2-arg constructor; an
/// "invalid" binding is expected to be modeled as `Option<Binding>` at use
/// sites once a use site exists (S1).
#[derive(Debug, Clone)]
pub struct Binding {
    pub decl: DeclId,
    /// The declaring node. Note that this is nullable (SemContext.h:31).
    pub ident: Option<NodeRc>,
}

impl Binding {
    /// Port of the 2-arg C++ constructor (SemContext.h:34-35).
    pub fn new(decl: DeclId, ident: Option<NodeRc>) -> Binding {
        Binding { decl, ident }
    }
}

/// The scoped binding table mapping from string to binding. Port of
/// `hermes::sema::BindingTableTy` (SemContext.h:47).
pub type BindingTable = PersistentScopedMap<Atom, Binding>;
/// Port of `hermes::sema::BindingTableScopeTy` (SemContext.h:48-49).
pub type BindingTableScope<'m> = Scope<'m, Atom, Binding>;
/// Port of `hermes::sema::BindingTableScopePtrTy` (SemContext.h:50-51).
pub type BindingTableScopePtr = ScopePtr<Atom, Binding>;

/// The kind of variable declaration. Determines scoping, among other
/// things. Port of `hermes::sema::Decl::Kind` (SemContext.h:58-105).
///
/// The variant order is load-bearing: every predicate below is ported
/// EXACTLY from SemContext.h:130-189 and relies on comparing variants with
/// `<=`/`>=`, i.e. on this exact declaration order matching the C++ enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum DeclKind {
    // ==== Let-like declarations ===
    Let,
    Const,
    Class,
    Import,
    /// A catch variable bound with let-like rules (non-ES5).
    /// ES14.0 B.3.4 handles ES5-style catch bindings differently.
    Catch,
    /// Function declaration visible only in its lexical scope.
    ScopedFunction,
    /// A single catch variable declared like this "catch (e)", see
    /// ES10 B.3.5 VariableStatements in Catch Blocks
    ES5Catch,

    // ==== other declarations ===
    /// Name of a function expression, which is visible within the function
    /// but not outside it.
    FunctionExprName,
    /// Name of a class expression, which is visible within the class
    /// but not outside it.
    ClassExprName,
    /// Name of a builtin function for typed mode compilation.
    TypedBuiltin,

    // ==== Private name declarations ===
    /// This name defines a field.
    PrivateField,
    /// This name defines a method.
    PrivateMethod,
    /// This name defines only a getter.
    PrivateGetter,
    /// This name defines only a setter.
    PrivateSetter,
    /// This name defines both a getter and setter.
    PrivateGetterSetter,

    // ==== Var-like declarations ===
    /// "var" in function scope.
    Var,
    Parameter,
    /// "var" in global scope.
    GlobalProperty,
    /// Ambient global property,
    /// Used implicitly without corresponding "var" in global scope.
    UndeclaredGlobalProperty,
}

/// Certain identifiers must be treated differently by later parts of the
/// program, e.g. this identifier is specially treated "arguments" or "eval".
/// Can also store information on a private name decl static level. Port of
/// `hermes::sema::Decl::Special` (SemContext.h:110-116).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclSpecial {
    NotSpecial,
    Arguments,
    Eval,
    /// Can only be set for a private method or accessor.
    PrivateStatic,
}

/// This type describes what kind of constness the decl is. Port of
/// `hermes::sema::Decl::Constness` (SemContext.h:119-127).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Constness {
    /// The decl is never treated as const.
    Never,
    /// The decl is treated as const only when the assignment occurs in
    /// strict mode code.
    StrictModeOnly,
    /// The decl is always treated as const.
    Always,
}

impl DeclKind {
    /// \return true if this declaration kind obeys the TDZ. Port of
    /// `isKindTDZ` (SemContext.h:130-132).
    pub fn is_tdz(self) -> bool {
        self <= DeclKind::Class
    }

    /// \return true if this kind of declaration is function scope (and can
    /// be re-declared). Port of `isKindVarLike` (SemContext.h:136-138).
    pub fn is_var_like(self) -> bool {
        self >= DeclKind::Var
    }

    /// Port of `isKindVarLikeOrScopedFunction` (SemContext.h:139-141).
    pub fn is_var_like_or_scoped_function(self) -> bool {
        self.is_var_like() || self == DeclKind::ScopedFunction
    }

    /// \return true if this kind of declaration is lexically scoped (and
    /// cannot be re-declared). Port of `isKindLetLike` (SemContext.h:144-146).
    pub fn is_let_like(self) -> bool {
        self <= DeclKind::ES5Catch
    }

    /// \return true if this kind of declaration is a global property. Port
    /// of `isKindGlobal` (SemContext.h:148-150).
    pub fn is_global(self) -> bool {
        self >= DeclKind::GlobalProperty
    }

    /// \return the const level of this kind. Port of `getKindConstness`
    /// (SemContext.h:153-184).
    pub fn constness(self) -> Constness {
        // ES 2025 defines 9.1.1.1.3 CreateImmutableBinding ( N, S ) where N is
        // the name of a binding and S is a 'strict' parameter. A strict
        // binding means that attempting to rebind that name will always
        // result in a TypeError being thrown, whereas a non-strict immutable
        // binding means that a TypeError is only thrown when executing in
        // strict mode. From this defintion, we can split `Decl::Kind`s into
        // three categories: mutable bindings (never 'const'), immutable
        // bindings made with S=true (always 'const'), immutable bindings
        // made with S=false ('const' only in strict mode.)
        match self {
            // ES 2025 14.2.3
            // 3.a.i. If IsConstantDeclaration of d is true, then
            //   1. Perform ! lexEnv.CreateImmutableBinding(dn, true).
            DeclKind::Const
            // ES 2025 15.7.14
            // 1.a. Perform ! classEnv.CreateImmutableBinding(classBinding, true).
            | DeclKind::ClassExprName
            // ES2025 16.2.1.7.3.1
            // 7.b.ii. Perform ! env.CreateImmutableBinding(in.[[LocalName]], true).
            | DeclKind::Import => Constness::Always,

            // ES2025 15.2.5
            // 5. Perform ! funcEnv.CreateImmutableBinding(name, false).
            DeclKind::FunctionExprName => Constness::StrictModeOnly,

            _ => Constness::Never,
        }
    }

    /// \return true if this declaration kind is a private name. Port of
    /// `isKindPrivateName` (SemContext.h:187-189).
    pub fn is_private_name(self) -> bool {
        self >= DeclKind::PrivateField && self <= DeclKind::PrivateGetterSetter
    }
}

/// Variable declaration. Port of `hermes::sema::Decl` (SemContext.h:54-227).
///
/// Deviation: `customData` is deliberately NOT ported — see the module doc.
#[derive(Debug, Clone)]
pub struct Decl {
    /// Identifier that is declared (SemContext.h:192).
    pub name: Atom,
    /// What kind of declaration it is.
    pub kind: DeclKind,
    /// Whether this is a generic declaration.
    /// The type checker can use this flag to keep track of which
    /// declarations are generic. SemanticResolver itself doesn't set this
    /// flag because it has no understanding of types.
    pub generic: bool,
    /// If this is a special declaration, identify which one.
    pub special: DeclSpecial,
    /// The lexical scope of the declaration. `None` for special
    /// declarations, since they are technically unscoped
    /// (SemContext.h:204-206).
    pub scope: Option<ScopeId>,
}

/// Lexical scopes within a function. Port of `hermes::sema::LexicalScope`
/// (SemContext.h:230-285). The partial-cloning constructor (SemContext.h:280,
/// only used by `ESTreeClone`) is out of scope for this task — see S5.
///
/// Deviation: `customData` is deliberately NOT ported — see the module doc.
///
/// Not `Debug`: `binding_table_scope` (`ScopePtr`) doesn't implement it (see
/// `hermes_support::persistent_scoped_map`'s module doc on why it can't derive it).
pub struct LexicalScope {
    /// The global depth of this scope, where 0 is the root scope (globally).
    pub depth: u32,
    /// The function owning this lexical scope. May never be null (C++:
    /// `FunctionInfo *const parentFunction`).
    pub parent_function: FunctionInfoId,
    /// The enclosing lexical scope (it could be in another function). `None`
    /// if this is the root scope.
    pub parent_scope: Option<ScopeId>,
    /// The index in the owning `parent_function`'s scopes list where this
    /// scope lives. Set when this scope is inserted into a `FunctionInfo`
    /// (`SemContext::new_scope`, mirroring `FunctionInfo::addScope`).
    pub idx_in_parent_function: u32,
    /// All declarations made in this scope.
    pub decls: Vec<DeclId>,
    /// A list of functions that need to be hoisted and materialized before
    /// we can generate the rest of the scope.
    pub hoisted_functions: Vec<NodeRc>,
    /// True if this scope or any descendent scopes have a local eval call.
    /// If any descendent uses local eval, it's impossible to know whether
    /// local variables are modified.
    pub local_eval: bool,
    /// The scope in the binding table that this lexical scope is associated
    /// with. This is used to restore the binding table to this point when
    /// performing compilation for an eval.
    pub binding_table_scope: BindingTableScopePtr,
}

/// An enum indicating whether a function expression is an arrow function.
/// Port of `hermes::sema::FuncIsArrow` (SemContext.h:288).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuncIsArrow {
    Yes,
    No,
}

/// The possible kinds of constructors. Port of
/// `hermes::sema::FunctionInfo::ConstructorKind` (SemContext.h:327).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructorKind {
    None,
    Base,
    Derived,
}

/// An enum to track the "source visibility" of functions. This notion is
/// coined to implement "directives" such as 'hide source' and 'sensitive'
/// defined by https://github.com/tc39/proposal-function-implementation-hiding,
/// as well as 'show source' Hermes proposed to explicitly preserve source
/// for `toString`.
///
/// Members are ordered in an increasingly stronger manner, where only later
/// source visibility can override the earlier but not vice versa. Port of
/// `hermes::SourceVisibility` (AST/Context.h:126-146); lives in `sema` for
/// now — see the module doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum SourceVisibility {
    /// The implementation-default behavior, e.g. `toString` prints
    /// `{ [bytecode] }` in Hermes.
    #[default]
    Default,
    /// Enforce the source code text to be available for the `toString` use.
    ShowSource,
    /// Enforce to have the syntax of NativeFunction, e.g. `toString` prints
    /// `{ [native code] }`.
    HideSource,
    /// Considered security-sensitive, e.g. `toString` printed as
    /// NativeFunction; hidden from error stack trace to protect from
    /// leaking its existence.
    Sensitive,
}

/// Custom directives which were specified on a given function. Port of
/// `hermes::CustomDirectives` (AST/Context.h:147-166); lives in `sema` for
/// now — see the module doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CustomDirectives {
    /// Source visibility of the given function.
    pub source_visibility: SourceVisibility,
    /// Whether we should _always_ attempt to inline the function,
    /// regardless of the number of callsites it has. It's possible the
    /// function can't be inlined if it contains code which can't be
    /// inlined, but the heuristic won't reject it.
    pub always_inline: bool,
    /// Whether we should _never_ attempt to inline the function. Useful (at
    /// least) in tests.
    pub no_inline: bool,
    /// Whether the function is a builtin function.
    pub builtin: bool,
}

/// Semantic information about functions. Port of
/// `hermes::sema::FunctionInfo` (SemContext.h:291-427). The partial-cloning
/// constructor (SemContext.h:397, only used by `ESTreeClone`) is out of
/// scope for this task — see S5.
///
/// Not `Debug`: `binding_table_scope` (`ScopePtr`) doesn't implement it.
pub struct FunctionInfo {
    /// All lexical scopes in this function. The first one is the function
    /// scope (or, if `has_parameter_expressions`, the parameter scope —
    /// see `has_parameter_expressions`'s doc comment).
    scopes: Vec<ScopeId>,
    /// The function surrounding this function. `None` if this is the root
    /// function.
    pub parent_function: Option<FunctionInfoId>,
    /// The enclosing lexical scope. `None` if this is the root function.
    pub parent_scope: Option<ScopeId>,
    /// A list of imports that need to be hoisted and materialized before we
    /// can generate the rest of the function. Any line of the file may use
    /// the imported values.
    pub imports: Vec<NodeRc>,
    /// The implicitly declared "arguments" object. It is declared only if
    /// it is used. Should be populated by calling
    /// `SemContext::func_arguments_decl`. C++ stores this as
    /// `OptValue<Decl *>`, which never observes `nullptr`; `Option<DeclId>`
    /// ports that directly (no separate "is set" flag is needed).
    pub arguments_decl: Option<DeclId>,
    /// Index of the function scope in the scopes vector. The index isn't
    /// constant in the case of parameter expressions which introduce new
    /// scopes (e.g. for function expression names), so this has to be
    /// stored separately. `u32::MAX` (`NO_FUNCTION_BODY_SCOPE`) is used
    /// until this field is set.
    pub function_body_scope_idx: u32,
    /// True if the function is strict mode.
    pub strict: bool,
    /// Custom directives found in this function.
    pub custom_directives: CustomDirectives,
    /// True if this function is an arrow function.
    pub arrow: bool,
    /// The kind of constructor this function is.
    pub constructor_kind: ConstructorKind,
    /// False if the parameter list contains any patterns.
    pub simple_parameter_list: bool,
    /// True if the parameter list contains any expressions. If there are
    /// expressions, then the first scope in the `scopes` list will be the
    /// parameter scope, and the second scope will be the function scope.
    pub has_parameter_expressions: bool,
    /// Whether this function references "arguments" identifier.
    pub uses_arguments: bool,
    /// Whether this function contains arrow functions.
    pub contains_arrow_functions: bool,
    /// This is a logical or of the `uses_arguments` flags of all contained
    /// arrow functions. This will be used as a conservative estimate of
    /// whether a non-arrow function needs to eagerly create and capture its
    /// Arguments object.
    pub contains_arrow_functions_using_arguments: bool,
    /// Whether the function might execute the implicit 'undefined' return at
    /// the end. This is determined conservatively, so there may be some
    /// functions that in reality can't reach the implicit return, but this
    /// bool is set to 'true' anyway.
    pub may_reach_implicit_return: bool,
    /// True if this function came from a program node.
    pub is_program_node: bool,
    /// True if this function came from a static block node.
    pub is_static_block: bool,
    /// The parent binding table scope of this function. Storing the parent
    /// of the code we want to eventually compile for lazy compilation -
    /// we're trying to compile this function itself.
    pub binding_table_scope: BindingTableScopePtr,
    /// How many labels have been allocated in this function so far.
    pub num_labels: u32,
}

impl FunctionInfo {
    /// Sentinel for `function_body_scope_idx` before it's been set. Port of
    /// the C++ default member initializer `UINT32_MAX` (SemContext.h:319).
    pub const NO_FUNCTION_BODY_SCOPE: u32 = u32::MAX;

    /// The `is_arrow` arg indicates whether the function is an arrow
    /// function. Port of the primary C++ constructor (SemContext.h:377-389).
    pub fn new(
        is_arrow: FuncIsArrow,
        cons_kind: ConstructorKind,
        parent_function: Option<FunctionInfoId>,
        parent_scope: Option<ScopeId>,
        strict: bool,
        custom_directives: CustomDirectives,
    ) -> FunctionInfo {
        FunctionInfo {
            scopes: Vec::new(),
            parent_function,
            parent_scope,
            imports: Vec::new(),
            arguments_decl: None,
            function_body_scope_idx: Self::NO_FUNCTION_BODY_SCOPE,
            strict,
            custom_directives,
            arrow: is_arrow == FuncIsArrow::Yes,
            constructor_kind: cons_kind,
            simple_parameter_list: true,
            has_parameter_expressions: false,
            uses_arguments: false,
            contains_arrow_functions: false,
            contains_arrow_functions_using_arguments: false,
            may_reach_implicit_return: true,
            is_program_node: false,
            is_static_block: false,
            binding_table_scope: BindingTableScopePtr::default(),
            num_labels: 0,
        }
    }

    /// \pre the `function_body_scope_idx` field has been set.
    /// \return the top-level lexical scope of the function body. Port of
    /// `getFunctionBodyScope` (SemContext.h:404-407).
    pub fn get_function_body_scope(&self) -> ScopeId {
        debug_assert!(
            (self.function_body_scope_idx as usize) < self.scopes.len(),
            "functionScopeIdx not set"
        );
        self.scopes[self.function_body_scope_idx as usize]
    }

    /// \return the lexical scope which contains the parameter declarations,
    /// which may be the same as the function scope itself. The scope is
    /// guaranteed to contain all Parameter Decls, though it may contain
    /// other Decls as well. Port of `getParameterScope` (SemContext.h:413-416).
    pub fn get_parameter_scope(&self) -> ScopeId {
        debug_assert!(!self.scopes.is_empty(), "no parameter scope added yet");
        self.scopes[0]
    }

    /// \return the list of scopes declared in this FunctionInfo. Port of
    /// `getScopes` (SemContext.h:419-421).
    pub fn get_scopes(&self) -> &[ScopeId] {
        &self.scopes
    }

    /// Add `scope` to the list of scopes in this FunctionInfo. \return the
    /// index the scope was assigned, so the caller can set the owning
    /// `LexicalScope::idx_in_parent_function` (the two fields live in
    /// separate storages in this port; see `SemContext::new_scope`). Port
    /// of `addScope` (SemContext.h:422-426).
    pub fn add_scope(&mut self, scope: ScopeId) -> u32 {
        let idx = self.scopes.len() as u32;
        self.scopes.push(scope);
        idx
    }

    /// Allocate a new label and return its index. Port of `allocateLabel`
    /// (SemContext.h:370-373).
    pub fn allocate_label(&mut self) -> u32 {
        let label = self.num_labels;
        self.num_labels += 1;
        label
    }
}

/// IdentifierDecoration bit values, read (not assumed) from
/// `include/hermes/AST/ESTree.h:479-486`.
mod decl_state_bits {
    /// There is an "expression decl" stored in `decl`.
    pub const HAVE_EXPR: u8 = 1;
    /// There is a "declaration decl" stored in `decl`.
    pub const HAVE_DECL: u8 = 2;
    /// There is a "declaration decl" stored in a side table.
    pub const SIDE_DECL: u8 = 4;
}
use decl_state_bits::{HAVE_DECL, HAVE_EXPR, SIDE_DECL};

/// `HAVE_EXPR | HAVE_DECL`: both decls set, sharing one value.
const HAVE_EXPR_AND_DECL: u8 = HAVE_EXPR | HAVE_DECL;
/// `HAVE_EXPR | SIDE_DECL`: an expression decl plus a *different*
/// declaration decl, spilled into the side table.
const HAVE_EXPR_AND_SIDE: u8 = HAVE_EXPR | SIDE_DECL;

/// Semantic information regarding the program. Storage for `FunctionInfo`,
/// `LexicalScope`, and `Decl` records. Port of `hermes::sema::SemContext`
/// (SemContext.h:442-692) — see the module doc for the deviations (no
/// parent/child tree, no `customData`, `Vec`-backed storages).
///
/// Not `Debug`: `Keywords`, `BindingTable`, and `BindingTableScopePtr` don't
/// implement it.
pub struct SemContext {
    /// Convenient storage of "keyword" identifiers used by various parts of
    /// the infrastructure.
    pub kw: Keywords,

    /// Storage for all functions.
    functions: Vec<FunctionInfo>,
    /// Storage for all lexical scopes.
    scopes: Vec<LexicalScope>,
    /// Storage for all variable declarations.
    decls: Vec<Decl>,

    /// The currently lexically visible names.
    ///
    /// Deviation: `Rc`-wrapped, where C++ holds the table by value
    /// (SemContext.h:610). Every `PersistentScopedMap` operation takes
    /// `&self` (it is interior-mutable), but a `Scope` guard borrows the
    /// table for as long as the scope is open — and the resolver needs that
    /// guard to coexist with a `&mut SemContext` (it calls `new_scope`/
    /// `new_global` while a scope is open). Rust cannot split a `&mut
    /// SemContext` into "`&BindingTable` + `&mut` everything else" across a
    /// crate boundary, so `SemanticResolver` instead holds a `&BindingTable`
    /// derived from its own `Rc` clone (see `binding_table_rc` and
    /// `resolve::resolve_ast`). C++ needs no such thing: `bindingTable_` is
    /// a plain reference member there.
    binding_table: Rc<BindingTable>,
    /// Global binding table scope.
    binding_table_global_scope: BindingTableScopePtr,

    /// This side table is used to associate a "declaration decl" with an
    /// identifier node, when the "declaration decl" and the "expression
    /// decl" are both set and are not the same value.
    side_identifier_declaration_decl: HashMap<NodeId, DeclId>,
    /// This side table is used to associate a "declaration decl" with an
    /// identifier node in scenarios where a scoped function is promoted to
    /// the global scope. In `promoted_function_decls` we store the scoped
    /// declaration, while in `side_identifier_declaration_decl` the
    /// global-like declaration.
    promoted_function_decls: HashMap<NodeId, DeclId>,

    /// A list of function declarations with "builtin" directive that were
    /// skipped during semantic resolution. FlowChecker needs to process
    /// these.
    builtin_declarations: Vec<NodeRc>,
}

impl SemContext {
    /// Construct an empty `SemContext`. Port of the C++ constructor
    /// (SemContext.cpp:57-70), minus the parent/child tree machinery (S5;
    /// see the module doc) — this `SemContext` is always its own root.
    pub fn new(kw: Keywords) -> SemContext {
        SemContext {
            kw,
            functions: Vec::new(),
            scopes: Vec::new(),
            decls: Vec::new(),
            binding_table: Rc::new(BindingTable::new()),
            binding_table_global_scope: BindingTableScopePtr::default(),
            side_identifier_declaration_decl: HashMap::new(),
            promoted_function_decls: HashMap::new(),
            builtin_declarations: Vec::new(),
        }
    }

    /// Assert that the global function and the global scope have been
    /// created. Port of `assertGlobalFunctionAndScope` (SemContext.h:507-510).
    pub fn assert_global_function_and_scope(&self) {
        debug_assert!(!self.functions.is_empty(), "global function has not been created");
        debug_assert!(!self.scopes.is_empty(), "global scope has not been created");
    }

    /// \p node may be `None`, in which case the answer is `No`. Port of
    /// `nodeIsArrow` (SemContext.cpp:74-80).
    pub fn node_is_arrow<'gc>(node: Option<&Node<'gc>>) -> FuncIsArrow {
        if let Some(n) = node {
            if matches!(n, Node::ArrowFunctionExpression(_)) {
                return FuncIsArrow::Yes;
            }
        }
        FuncIsArrow::No
    }

    /// \param parent_function may be null.
    /// \param parent_scope may be null.
    /// \return a new function. Port of `newFunction` (SemContext.cpp:95-105).
    pub fn new_function(
        &mut self,
        is_arrow: FuncIsArrow,
        cons_kind: ConstructorKind,
        parent_function: Option<FunctionInfoId>,
        parent_scope: Option<ScopeId>,
        strict: bool,
        custom_directives: CustomDirectives,
    ) -> FunctionInfoId {
        self.functions.push(FunctionInfo::new(
            is_arrow,
            cons_kind,
            parent_function,
            parent_scope,
            strict,
            custom_directives,
        ));
        FunctionInfoId::from_sema_id(SemaId((self.functions.len() - 1) as u32))
    }

    /// \param parent_function the function in which to put the scope,
    /// nullable.
    /// \param parent_scope the parent lexical scope, nullable.
    /// \return a new lexical scope. Port of `newScope` (SemContext.cpp:115-122),
    /// including `FunctionInfo::addScope` setting `idxInParentFunction`
    /// (SemContext.h:422-426) — done here because the two fields it touches
    /// live in different storages in this port (see `FunctionInfo::add_scope`).
    pub fn new_scope(
        &mut self,
        parent_function: FunctionInfoId,
        parent_scope: Option<ScopeId>,
    ) -> ScopeId {
        let depth = match parent_scope {
            Some(ps) => self.scope(ps).depth + 1,
            None => 0,
        };
        self.scopes.push(LexicalScope {
            depth,
            parent_function,
            parent_scope,
            idx_in_parent_function: 0,
            decls: Vec::new(),
            hoisted_functions: Vec::new(),
            local_eval: false,
            binding_table_scope: BindingTableScopePtr::default(),
        });
        let id = ScopeId::from_sema_id(SemaId((self.scopes.len() - 1) as u32));
        let idx = self.function_mut(parent_function).add_scope(id);
        self.scope_mut(id).idx_in_parent_function = idx;
        id
    }

    /// \param scope not nullable.
    /// \return a new declaration in \p scope. Port of `newDeclInScope`
    /// (SemContext.cpp:134-143).
    pub fn new_decl_in_scope(
        &mut self,
        name: Atom,
        kind: DeclKind,
        scope: ScopeId,
        special: DeclSpecial,
    ) -> DeclId {
        self.decls.push(Decl {
            name,
            kind,
            generic: false,
            special,
            scope: Some(scope),
        });
        let id = DeclId::from_sema_id(SemaId((self.decls.len() - 1) as u32));
        self.scope_mut(scope).decls.push(id);
        id
    }

    /// 3-arg convenience matching the C++ default argument
    /// (`Special special = Decl::Special::NotSpecial`, SemContext.h:494-498).
    pub fn new_decl_in_scope_default(
        &mut self,
        name: Atom,
        kind: DeclKind,
        scope: ScopeId,
    ) -> DeclId {
        self.new_decl_in_scope(name, kind, scope, DeclSpecial::NotSpecial)
    }

    /// \return a new global property. Port of `newGlobal` (SemContext.cpp:152-158).
    pub fn new_global(&mut self, name: Atom, kind: DeclKind) -> DeclId {
        debug_assert!(kind.is_global(), "invalid global declaration kind");
        // Ensure that the global declaration is added to the (root) global
        // scope and doesn't get freed before references to it.
        let global_scope = self.get_global_scope();
        self.new_decl_in_scope(name, kind, global_scope, DeclSpecial::NotSpecial)
    }

    /// \return the global function. Port of `getGlobalFunction`
    /// (SemContext.h:512-515); no parent tree in this port, so `self` is
    /// always root.
    pub fn get_global_function(&self) -> FunctionInfoId {
        FunctionInfoId::from_sema_id(SemaId(0))
    }

    /// \return the global lexical scope. Port of `getGlobalScope`
    /// (SemContext.h:516-519); no parent tree in this port, so `self` is
    /// always root.
    pub fn get_global_scope(&self) -> ScopeId {
        ScopeId::from_sema_id(SemaId(0))
    }

    /// Returns the nearest non-arrow (non-proper) ancestor of the current
    /// FunctionInfo that is not for an arrow function. This will always
    /// exist. Port of `nearestNonArrow` (SemContext.cpp:82-93).
    pub fn nearest_non_arrow(&self, f: FunctionInfoId) -> FunctionInfoId {
        let mut cur = f;
        let global = self.get_global_function();
        // Top-level root program nodes that are not the global function are
        // debugger eval functions. We don't want to consider these when
        // trying to find the nearest non-arrow.
        while {
            let info = self.function(cur);
            info.arrow || (info.is_program_node && cur != global)
        } {
            cur = self
                .function(cur)
                .parent_function
                .expect("All FunctionInfo should have a non-arrow ancestor.");
        }
        cur
    }

    /// Create or retrieve the arguments declaration in \p func. If `func`
    /// is an arrow function, find the closest ancestor that is not an arrow
    /// function and use that function's `arguments`. If we end up looking
    /// for `arguments` in global scope, an ambient declaration is created
    /// and returned.
    /// \p arguments_name the object doesn't have access to the AST node, so
    ///   the "arguments" string has to be passed in.
    /// \return the special arguments declaration in the specified function.
    /// Port of `funcArgumentsDecl` (SemContext.cpp:160-198).
    pub fn func_arguments_decl(
        &mut self,
        func: FunctionInfoId,
        arguments_name: Atom,
    ) -> DeclId {
        // Find the closest non-arrow ancestor.
        let mut arguments_func = func;
        while self.function(arguments_func).arrow {
            match self.function(arguments_func).parent_function {
                Some(parent) => arguments_func = parent,
                None => break,
            }
        }

        // 'arguments' already exists, avoid redeclaring.
        if let Some(decl) = self.function(arguments_func).arguments_decl {
            return decl;
        }

        let decl = if arguments_func == self.get_global_function() {
            // `arguments` must simply be treated as a global property in
            // top level contexts.
            let scope = self.function(arguments_func).get_scopes()[0];
            self.new_decl_in_scope(
                arguments_name,
                DeclKind::UndeclaredGlobalProperty,
                scope,
                DeclSpecial::NotSpecial,
            )
        } else {
            // Otherwise, regular function-level "arguments" declaration.
            let scope = self.function(arguments_func).get_scopes()[0];
            self.new_decl_in_scope(
                arguments_name,
                DeclKind::Var,
                scope,
                DeclSpecial::Arguments,
            )
        };

        // Store it for future use.
        self.function_mut(arguments_func).arguments_decl = Some(decl);

        decl
    }

    /// \return the function record for \p id.
    pub fn function(&self, id: FunctionInfoId) -> &FunctionInfo {
        &self.functions[id.index()]
    }
    /// \return the mutable function record for \p id.
    pub fn function_mut(&mut self, id: FunctionInfoId) -> &mut FunctionInfo {
        &mut self.functions[id.index()]
    }
    /// \return the lexical scope record for \p id.
    pub fn scope(&self, id: ScopeId) -> &LexicalScope {
        &self.scopes[id.index()]
    }
    /// \return the mutable lexical scope record for \p id.
    pub fn scope_mut(&mut self, id: ScopeId) -> &mut LexicalScope {
        &mut self.scopes[id.index()]
    }
    /// \return the declaration record for \p id.
    pub fn decl(&self, id: DeclId) -> &Decl {
        &self.decls[id.index()]
    }
    /// \return the mutable declaration record for \p id.
    pub fn decl_mut(&mut self, id: DeclId) -> &mut Decl {
        &mut self.decls[id.index()]
    }

    /// \return the number of functions in storage. Not part of the C++ API
    /// (C++ has no such count — `printSemContext` just range-for's the
    /// private `functions_` deque directly, SemContext.cpp:429-436); added
    /// so `SemContextDumper` can walk every `FunctionInfoId` in storage
    /// (index/allocation) order the same way, from outside this module.
    pub fn functions_len(&self) -> usize {
        self.functions.len()
    }

    /// Set the binding table global scope. Port of
    /// `setBindingTableGlobalScope` (SemContext.h:526-533); the C++ assert
    /// that this can only be called on a root SemContext is vacuous here
    /// (there is no parent tree — `self` is always root).
    pub fn set_binding_table_global_scope(&mut self, scope: BindingTableScopePtr) {
        self.binding_table_global_scope = scope;
    }
    /// \return the binding table global scope. Port of
    /// `getBindingTableGlobalScope` (SemContext.h:535-538).
    pub fn get_binding_table_global_scope(&self) -> &BindingTableScopePtr {
        &self.binding_table_global_scope
    }
    /// \return the binding table. Port of `getBindingTable` (SemContext.h:613-618).
    pub fn binding_table(&self) -> &BindingTable {
        &self.binding_table
    }
    /// \return an owning handle to the binding table, whose lifetime is
    /// independent of any borrow of this `SemContext`. No C++ counterpart —
    /// see the `binding_table` field's deviation note.
    pub fn binding_table_rc(&self) -> Rc<BindingTable> {
        Rc::clone(&self.binding_table)
    }

    /// Add a builtin declaration for later processing by FlowChecker. Port
    /// of `addBuiltinDeclaration` (SemContext.h:592-595).
    pub fn add_builtin_declaration(&mut self, decl: NodeRc) {
        self.builtin_declarations.push(decl);
    }
    /// \return the list of builtin declarations. Port of
    /// `getBuiltinDeclarations` (SemContext.h:598-601).
    pub fn builtin_declarations(&self) -> &[NodeRc] {
        &self.builtin_declarations
    }

    // ---- Identifier decl-state machine -----------------------------------
    //
    // Port of SemContext.cpp:200-296 (`getDeclarationDecl`/
    // `setDeclarationDecl`) and :329-411 (`setExpressionDecl`), plus the
    // inline `getExpressionDecl` (SemContext.h:557-563). The node stores
    // `decl: Cell<Option<SemaId>>` where C++ stores the raw `Decl *`
    // (`node->decl_`); the three state bits live in `decl_state: Cell<u8>`
    // with the exact constants from ESTree.h:479-486 (`decl_state_bits`,
    // above). `node_id` identifies the identifier node in the two
    // `NodeId`-keyed side tables (see the module doc).

    /// \return the declaration in which this identifier participates.
    /// `None` if no resolution has been recorded. Port of
    /// `getDeclarationDecl` (SemContext.cpp:200-213).
    pub fn get_declaration_decl(&self, ident: &Identifier) -> Option<DeclId> {
        let state = ident.decl_state.get();
        if state & HAVE_DECL != 0 {
            ident.decl.get().map(DeclId::from_sema_id)
        } else if state & SIDE_DECL != 0 {
            let node_id = ident.metadata.id.get();
            let decl = *self
                .side_identifier_declaration_decl
                .get(&node_id)
                .expect(
                    "IdentifierNode with BitSideDecl must be in the side table",
                );
            Some(decl)
        } else {
            None
        }
    }

    /// \pre the identifier hasn't been marked "unresolvable".
    /// \return the declaration to which the identifier has been resolved,
    /// `None` if no resolution has been recorded. Port of
    /// `getExpressionDecl` (SemContext.h:557-563).
    pub fn get_expression_decl(&self, ident: &Identifier) -> Option<DeclId> {
        assert!(
            !ident.unresolvable.get(),
            "Attempt to read decl for unresolvable identifier"
        );
        if ident.decl_state.get() & HAVE_EXPR != 0 {
            ident.decl.get().map(DeclId::from_sema_id)
        } else {
            None
        }
    }

    /// Set the "declaration decl" of the specified identifier node. Port of
    /// `setDeclarationDecl` (SemContext.cpp:215-296).
    pub fn set_declaration_decl(
        &mut self,
        node_id: NodeId,
        ident: &Identifier,
        decl: Option<DeclId>,
    ) {
        debug_assert_eq!(
            node_id,
            ident.metadata.id.get(),
            "node_id must identify ident itself"
        );

        // Are we setting a "declaration decl" or erasing one?
        if let Some(decl) = decl {
            // We are setting a new "declaration decl". Update the state
            // correspondingly.
            match ident.decl_state.get() {
                // We have an existing "expression decl". Depending on
                // whether the "declaration decl" has the same value, either
                // just set its bit, or record it in the side table.
                HAVE_EXPR => {
                    if ident.decl.get() == Some(decl.sema_id()) {
                        ident.decl_state.set(HAVE_EXPR_AND_DECL);
                    } else {
                        ident.decl_state.set(HAVE_EXPR_AND_SIDE);
                        self.side_identifier_declaration_decl.insert(node_id, decl);
                    }
                }

                // We have both an existing "expression decl" and a
                // "declaration decl", which is in the side table. We have
                // been asked to update the "declaration decl". If the new
                // value happens to be the same as the "expression decl", we
                // no longer need the side table, otherwise we just update
                // the value in the side table.
                HAVE_EXPR_AND_SIDE => {
                    if ident.decl.get() == Some(decl.sema_id()) {
                        ident.decl_state.set(HAVE_EXPR_AND_DECL);
                        let erased =
                            self.side_identifier_declaration_decl.remove(&node_id);
                        debug_assert!(
                            erased.is_some(),
                            "IdentifierNode with BitSideDecl must be in side table"
                        );
                    } else {
                        self.side_identifier_declaration_decl.insert(node_id, decl);
                    }
                }

                // We don't have an "expression decl", so we just update the
                // "declaration decl" and set the bit to know it is there.
                state => {
                    debug_assert!(
                        state == 0 || state == HAVE_DECL,
                        "Invalid declState"
                    );
                    ident.decl.set(Some(decl.sema_id()));
                    ident.decl_state.set(HAVE_DECL);
                }
            }
        } else {
            // We are "unsetting" a "declaration decl". Update the state for
            // that.
            match ident.decl_state.get() {
                // We have a "declaration decl" and an "expression decl"
                // sharing the same value. Just unset the bit for the
                // "declaration decl".
                HAVE_EXPR_AND_DECL => {
                    ident.decl_state.set(HAVE_EXPR);
                }

                // We have only a "declaration decl". Unset the bit and
                // clear the value.
                HAVE_DECL => {
                    ident.decl_state.set(0);
                    ident.decl.set(None);
                }

                // We have a "declaration decl" in the side table and an
                // expression decl. Remove the former from the side table
                // and clear its bit.
                HAVE_EXPR_AND_SIDE => {
                    ident.decl_state.set(HAVE_EXPR);
                    let erased =
                        self.side_identifier_declaration_decl.remove(&node_id);
                    debug_assert!(
                        erased.is_some(),
                        "IdentifierNode with BitSideDecl must be in side table"
                    );
                }

                // We don't have a "declaration decl", so do nothing.
                state => {
                    debug_assert!(
                        state == 0 || state == HAVE_EXPR,
                        "Invalid declState"
                    );
                }
            }
        }
    }

    /// Set the "expression decl" of the specified identifier node.
    /// \pre the identifier hasn't been marked "unresolvable". Port of
    /// `setExpressionDecl` (SemContext.cpp:329-411).
    pub fn set_expression_decl(
        &mut self,
        node_id: NodeId,
        ident: &Identifier,
        decl: Option<DeclId>,
    ) {
        debug_assert_eq!(
            node_id,
            ident.metadata.id.get(),
            "node_id must identify ident itself"
        );

        // Are we setting an "expression decl" or erasing one?
        if let Some(decl) = decl {
            // We are setting a new "expression decl". Update the state
            // correspondingly.
            assert!(
                !ident.unresolvable.get(),
                "Attempt to set decl for unresolvable identifier"
            );

            match ident.decl_state.get() {
                // We already have a "declaration decl" and possibly an
                // "expression decl". Depending on whether the new
                // "expression decl" has the same value as the existing
                // "declaration decl" or not, we have to move the existing
                // "declaration decl" into the side table.
                HAVE_DECL | HAVE_EXPR_AND_DECL => {
                    if Some(decl.sema_id()) == ident.decl.get() {
                        ident.decl_state.set(HAVE_EXPR_AND_DECL);
                    } else {
                        ident.decl_state.set(HAVE_EXPR_AND_SIDE);
                        // The existing value (the "declaration decl") moves
                        // into the side table before being overwritten.
                        if let Some(old) = ident.decl.get() {
                            self.side_identifier_declaration_decl.insert(
                                node_id,
                                DeclId::from_sema_id(old),
                            );
                        }
                        ident.decl.set(Some(decl.sema_id()));
                    }
                }

                // We have an existing "expression decl" and a different
                // "declaration decl" stored in the side table. If the new
                // "expression decl" matches the "declaration decl", then we
                // need to remove the "declaration decl" from the side table.
                HAVE_EXPR_AND_SIDE => {
                    ident.decl.set(Some(decl.sema_id()));
                    let side = *self
                        .side_identifier_declaration_decl
                        .get(&node_id)
                        .expect(
                            "IdentifierNode with BitSideDecl must be in side table",
                        );
                    if decl == side {
                        ident.decl_state.set(HAVE_EXPR_AND_DECL);
                        self.side_identifier_declaration_decl.remove(&node_id);
                    }
                }

                // Just update the value and set the bit.
                state => {
                    debug_assert!(state == 0 || state == HAVE_EXPR);
                    ident.decl.set(Some(decl.sema_id()));
                    ident.decl_state.set(HAVE_EXPR);
                }
            }
        } else {
            // We are "unsetting" an "expression decl". Update the state for
            // that.
            match ident.decl_state.get() {
                // Unset the existing "expression decl" and clear the
                // pointer.
                HAVE_EXPR => {
                    ident.decl_state.set(0);
                    ident.decl.set(None);
                }

                // We have both an "expression decl" and a "declaration
                // decl", sharing a value. Clear the "have expression" bit.
                HAVE_EXPR_AND_DECL => {
                    ident.decl_state.set(HAVE_DECL);
                }

                // We have "expression decl" and a "declaration decl" with a
                // different value in a side table. Move the "declaration
                // decl" out of the side table.
                //
                // NOTE (faithful port, not fixed): the C++ original does
                // NOT erase the side-table entry here (SemContext.cpp:397-404),
                // unlike the symmetric branch in `set_declaration_decl`. The
                // entry becomes unreachable once the SIDE_DECL bit is
                // cleared, but it stays in the map until the map itself is
                // torn down or the same node key is reused/overwritten.
                HAVE_EXPR_AND_SIDE => {
                    let side = *self
                        .side_identifier_declaration_decl
                        .get(&node_id)
                        .expect(
                            "IdentifierNode with BitSideDecl must be in side table",
                        );
                    ident.decl.set(Some(side.sema_id()));
                    ident.decl_state.set(HAVE_DECL);
                }

                state => {
                    debug_assert!(
                        state == 0 || state == HAVE_DECL,
                        "Invalid declState"
                    );
                }
            }
        }
    }

    /// Set the "declaration decl" and the "expression decl" of the
    /// identifier node to the same value. Port of `setBothDecl`
    /// (SemContext.h:605-608).
    pub fn set_both_decl(
        &mut self,
        node_id: NodeId,
        ident: &Identifier,
        decl: Option<DeclId>,
    ) {
        self.set_expression_decl(node_id, ident, decl);
        self.set_declaration_decl(node_id, ident, decl);
    }

    /// Set a promoted "declaration decl" for the specified identifier node.
    /// Port of `setPromotedDecl` (SemContext.h:573-575).
    pub fn set_promoted_decl(&mut self, node_id: NodeId, decl: DeclId) {
        self.promoted_function_decls.insert(node_id, decl);
    }

    /// \return a global "declaration decl" associated with a promoted
    /// function identifier if one exists, else `None`. Port of
    /// `getPromotedDecl` (SemContext.h:579-585).
    pub fn get_promoted_decl(&self, node_id: NodeId) -> Option<DeclId> {
        self.promoted_function_decls.get(&node_id).copied()
    }

    /// Clears all promoted declarations. Port of `clearPromotedDecls`
    /// (SemContext.h:588-590).
    pub fn clear_promoted_decls(&mut self) {
        self.promoted_function_decls.clear();
    }

    /// Test-support accessor: current size of the side table backing the
    /// decl-state machine's spill path
    /// (`side_identifier_declaration_decl`). Not part of the C++ API; it
    /// exists so integration tests can assert the exact side-table-size
    /// transitions specified by each switch arm ported from
    /// SemContext.cpp:215-411.
    #[doc(hidden)]
    pub fn side_table_len_for_test(&self) -> usize {
        self.side_identifier_declaration_decl.len()
    }

    /// \return the constructor of a class, if it has one, else `None`. Port
    /// of `getConstructor` (SemContext.cpp:298-327).
    pub fn get_constructor<'gc>(
        &self,
        class_node: &'gc Node<'gc>,
    ) -> Option<&'gc Node<'gc>> {
        let class_body = match class_node {
            Node::ClassDeclaration(n) => n.body,
            Node::ClassExpression(n) => n.body,
            _ => {
                debug_assert!(false, "ClassLikeNode has only two subtypes.");
                return None;
            }
        };
        let body = class_body
            .as_class_body()
            .expect("ClassDeclaration/ClassExpression body must be a ClassBody");
        body.body.iter().find(|member| {
            member
                .as_method_definition()
                .is_some_and(|method| method.kind.get() == self.kw.ident_constructor)
        })
    }
}
