/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Port of `hermes::sema::SemanticResolver` (`lib/Sema/SemanticResolver.h`,
//! `lib/Sema/SemanticResolver.cpp`) — S0 subset.
//!
//! ## What "S0 subset" means
//!
//! This is the *entry path* only: the constructor, `run`,
//! `visit(ProgramNode *)` and everything that path reaches
//! (`scanDirectives`, `processAmbientDecls`, `ScopeRAII`, the
//! `FunctionContext` constructor that owns a `DeclCollector`). It is enough
//! to reproduce `hermesc -dump-sema` byte-for-byte for programs made of
//! literals and empty statements, which is what `tests/sema_differential.rs`
//! enforces against the real compiler.
//!
//! Everything else is *deliberately absent rather than approximated*:
//! `visit_node` panics with `sema S0: unhandled node kind ...` for any node
//! kind outside the handled set, and `process_collected_declarations` panics
//! if the `DeclCollector` actually collected anything. An honest panic keeps
//! the differential meaningful — a silently-wrong resolution would look like
//! a passing test on a corpus that never exercised it. The S1+ tasks replace
//! each panic with the ported code.
//!
//! ## Structural deviations from the C++
//!
//! - **`ScopeRAII` / `FunctionContext` are explicit push/pop pairs.** Both
//!   are C++ RAII objects that mutate the resolver from their
//!   constructor/destructor while the resolver is also being used. A Rust
//!   `Drop` guard cannot hold `&mut SemanticResolver` while its methods run,
//!   so they become `enter_scope`/`exit_scope` and
//!   `enter_function`/`exit_function` pairs returning a small saved-state
//!   struct, following the parser's established `SaveFunctionState` shape
//!   (`rust/crates/parser/src/js/functions.rs`).
//! - **`kw_` is an accessor, not a field.** C++ holds `const Keywords &kw_`
//!   borrowed from the `Context`; here `Keywords` lives inside `SemContext`,
//!   which the resolver holds by `&mut`, so a stored `&Keywords` would be a
//!   second (immutable) borrow of the same object. `kw()` reads it through
//!   the `&mut` instead; every keyword field is a `Copy` `AtomBytes`.
//! - **`bindingTable_` is reached through a separate `&BindingTable`**
//!   derived from an `Rc` clone the caller owns — see `SemContext`'s
//!   `binding_table` field note and `resolve::resolve_ast`.
//! - **`astContext_`** is not stored: the only things the S0 path reads from
//!   it are `getSourceErrorManager()` (held directly as `sm`),
//!   `isStrictMode()` and `getDebugInfoSetting()`, which are read from the
//!   `GCLock` / handled at the call sites that need them.
//! - **`bufferMessages_`** (`SourceErrorManager::SaveAndBufferMessages`,
//!   SemanticResolver.h:34) is ported as an `enable_buffering()` in
//!   [`SemanticResolver::new`] paired with a `disable_buffering()` in the
//!   `Drop` impl — the one place a `Drop` guard *does* fit here, because the
//!   resolver owns the `&mut SourceErrorManager` it flushes through. That
//!   matches the C++ member's lifetime exactly: buffering spans the whole
//!   resolver, and the flush (stable-sorted by source position) happens on
//!   destruction, not at the end of `run`.
//! - **`saveDecls_`, `typed_`, `curClassContext_`, `canReferenceSuper_`, the
//!   four `forbid*` flags** are not ported: nothing on the S0 path reads
//!   them, and each belongs to a later stage that will introduce it with its
//!   first use.
//! - **`DebugInfoSetting::ALL`**: the port has no debug-info setting yet, so
//!   both tests against it on this path are ported as a documented constant
//!   `false` — see [`DEBUG_INFO_SETTING_ALL`].

use std::collections::{HashMap, HashSet};

use ast::context::{GCLock, NodeRc};
use ast::node::Node;
use ast::node_child::{NodeLabel, Strictness};
use ast::visitor::Visitor;
use ast::SemaId;
use support::diag::{Subsystem, Warning};
use support::manager::SourceErrorManager;
use support::persistent_scoped_map::Scope;

use crate::decl_collector::DeclCollector;
use crate::ids::{DeclId, FunctionInfoId, ScopeId};
use crate::keywords::Keywords;
use crate::sem_context::{
    Atom, Binding, BindingTable, BindingTableScopePtr, ConstructorKind,
    CustomDirectives, DeclKind, SemContext, SourceVisibility,
};

/// Port of `ESTree::kASTMaxRecursionDepth` (RecursiveVisitor.h:686-692) for
/// the non-`HERMES_LIMIT_STACK_DEPTH`, non-MSVC configuration this port
/// targets; the initial value of `RecursionDepthTracker::recursionDepth_`
/// (RecursiveVisitor.h:712-713).
const AST_MAX_RECURSION_DEPTH: u32 = 1024;

/// Port of `astContext_.getDebugInfoSetting() == DebugInfoSetting::ALL`.
///
/// `DebugInfoSetting` (`include/hermes/AST/Context.h`) is not ported yet — it
/// is a compiler-driver knob (`-g3`), not something sema computes — and
/// nothing on the S0 path can set it, so both uses on this path (`ScopeRAII`,
/// SemanticResolver.cpp:2934-2936; `visit(ProgramNode *)`, cpp:219-221) test
/// this constant instead. The `if` statements are kept in the exact shape of
/// the C++ code so that porting the real setting later is a one-line change.
const DEBUG_INFO_SETTING_ALL: bool = false;

/// Port of `FunctionContext::Label` (SemanticResolver.h:531-538).
///
/// S0 never constructs one — `FunctionContext::label_map` is always empty —
/// but the type is defined now so S1's label handling drops in without
/// re-plumbing `FunctionContext`.
#[derive(Debug, Clone)]
pub struct Label {
    /// Where it was declared.
    pub declaration_node: NodeRc,
    /// Statement targeted by the label. It is either a LoopStatement or a
    /// LabeledStatement.
    pub target_statement: NodeRc,
}

/// Port of `hermes::sema::FunctionContext` (SemanticResolver.h:525-621).
///
/// C++'s `resolver_`/`prevContext_` fields implement the intrusive context
/// stack; here the stack is `SemanticResolver::function_stack` and those two
/// fields are not needed. See the module doc for the RAII deviation.
pub struct FunctionContext {
    /// The associated seminfo object.
    pub sem_info: FunctionInfoId,
    /// The AST node of the function. `None` for the contexts created by the
    /// `ExistingGlobalScopeTag`/`FunctionInfo *` constructors (S1+).
    pub node: Option<NodeRc>,
    /// The currently active labels in the function. Always empty in S0.
    pub label_map: HashMap<NodeLabel, Label>,
    /// Most nested active loop statement. Always `None` in S0.
    pub current_loop: Option<NodeRc>,
    /// The most nested active loop or switch statement. Always `None` in S0.
    pub current_loop_or_switch: Option<NodeRc>,
    /// True if we are validating a formal parameter list.
    pub is_formal_params: bool,
    /// All declarations in the function. `None` for the constructors that
    /// don't run a `DeclCollector` (S1+).
    pub decls: Option<DeclCollector>,
    /// The map of names that have been promoted to function scope by
    /// `promoteScopedFunctionDecls` in this function, mapped to their Var
    /// declaration in function scope. Always empty in S0.
    pub promoted_func_decls: HashMap<Atom, DeclId>,
    /// The depth of the function's scope in the binding table. Populated
    /// when a scope is entered within the function.
    pub binding_table_scope_depth: u32,
}

/// Port of `SemanticResolver::FoundDirectives` (SemanticResolver.h:471-484).
#[derive(Debug, Clone, Copy, Default)]
struct FoundDirectives<'ast> {
    /// The *first* "use strict" directive statement, if any. Kept as the
    /// node (not just a flag) because C++ points a diagnostic at it — see
    /// `visitFunctionLikeInFunctionContext`'s "'use strict' not allowed
    /// inside function with non-simple parameter list" error
    /// (SemanticResolver.cpp:1748-1751).
    use_strict_node: Option<&'ast Node<'ast>>,
    /// The strongest source-visibility directive seen.
    source_visibility: SourceVisibility,
    /// Whether an "inline" directive was seen (and not cancelled).
    always_inline: bool,
    /// Whether a "noinline" directive was seen (and not cancelled).
    no_inline: bool,
    /// Whether a "builtin" directive was seen. Read by
    /// `hasBuiltinDirective` (cpp:2816-2824), which is S2 scope.
    #[allow(dead_code)]
    builtin: bool,
}

/// The state a scope entry saves so `exit_scope` can restore it. Port of
/// `SemanticResolver::ScopeRAII`'s members (SemanticResolver.h:336-342)
/// minus `resolver_` (implicit in the method receiver) and `bindingScope_`
/// (owned by `SemanticResolver::binding_scopes`, since a
/// `persistent_scoped_map::Scope` borrows the table and so cannot be moved
/// into a value the caller holds).
#[must_use = "every enter_scope must be paired with exit_scope"]
pub struct ScopeState {
    /// Old `LexicalScope` to restore on pop.
    old_scope: Option<ScopeId>,
}

/// The state a function entry saves so `exit_function` can restore it. C++
/// keeps this in `FunctionContext::prevContext_` plus the `SaveAndRestore`
/// of `globalFunctionContext_` at the call site (cpp:203).
#[must_use = "every enter_function must be paired with exit_function"]
pub struct FunctionState {
    /// Whether this context was installed as `globalFunctionContext_` and
    /// therefore must be uninstalled.
    was_global_function_context: bool,
}

/// Port of `hermes::sema::SemanticResolver` — see the module doc for what
/// the S0 subset covers and how it deviates.
pub struct SemanticResolver<'bt, 'sc, 'sm, 'ad> {
    /// All semantic tables are persisted here.
    sem_ctx: &'sc mut SemContext,
    /// A copy of `Context::getSM()` for easier access.
    ///
    /// Also stands in for C++'s `bufferMessages_`
    /// (`SourceErrorManager::SaveAndBufferMessages`,
    /// SourceErrorManager.h:633-643): buffering is enabled on this manager
    /// by [`SemanticResolver::new`] and disabled — i.e. flushed, sorted by
    /// source position — by the `Drop` impl below, giving the C++ member's
    /// exact lifetime without a separate field.
    sm: &'sm mut SourceErrorManager,
    /// The currently lexically visible names. See the module doc for why
    /// this is a separate borrow rather than `sem_ctx.binding_table()`.
    binding_table: &'bt BindingTable,
    /// If not empty, a list of parsed files containing global ambient
    /// declarations that should be inserted in the global scope. C++ uses a
    /// nullable `const DeclarationFileListTy *`; an empty slice means the
    /// same thing at every use on this path (`processAmbientDecls` returns
    /// immediately for null and iterates otherwise).
    ambient_decls: &'ad [NodeRc],
    /// A set of names that are restricted in the global scope.
    /// <https://262.ecma-international.org/14.0/#sec-hasrestrictedglobalproperty>
    /// ES14.0 9.1.1.4.14 HasRestrictedGlobalProperty:
    ///   Any global properties that are defined to be non-configurable
    ///   are restricted.
    restricted_global_properties: HashSet<Atom>,
    /// True if we are preparing the AST to be compiled by Hermes, including
    /// erroring on features which we parse but don't compile and
    /// transforming the AST. False if we just want to validate the AST.
    compile: bool,
    /// Current lexical scope.
    cur_scope: Option<ScopeId>,
    /// The global scope.
    global_scope: BindingTableScopePtr,
    /// The stack of function contexts; the last one is C++'s
    /// `curFunctionContext_`.
    function_stack: Vec<FunctionContext>,
    /// Index into `function_stack` of C++'s `globalFunctionContext_`.
    /// `None` until populated.
    global_function_context: Option<usize>,
    /// The stack of open binding-table scopes, innermost last — the
    /// `bindingScope_` members of the C++ `ScopeRAII` objects currently
    /// alive. Popped from the back by `exit_scope`; note the elements must
    /// be dropped back-to-front (a `Scope` may only be popped when it is the
    /// current one), which `Vec`'s own front-to-back drop would violate, so
    /// every push must be matched by an `exit_scope`.
    binding_scopes: Vec<Scope<'bt, Atom, Binding>>,
    /// `ESTree::kASTMaxRecursionDepth` minus the current AST nesting level.
    /// Port of `RecursionDepthTracker::recursionDepth_`
    /// (RecursiveVisitor.h:706).
    recursion_depth: u32,
}

impl<'bt, 'sc, 'sm, 'ad> SemanticResolver<'bt, 'sc, 'sm, 'ad> {
    /// Port of the primary constructor (SemanticResolver.cpp:40-63).
    ///
    /// \param binding_table `sem_ctx`'s binding table, borrowed
    ///   independently — see the module doc.
    /// \param sem_ctx the result of resolution will be stored here.
    /// \param ambient_decls parsed files containing global ambient
    ///   declarations; empty for "none" (C++'s null pointer).
    /// \param compile whether this resolution is intended to compile or just
    ///   parsing.
    ///
    /// The C++ `saveDecls`/`typed` parameters are not ported — see the
    /// module doc.
    pub fn new(
        binding_table: &'bt BindingTable,
        sem_ctx: &'sc mut SemContext,
        sm: &'sm mut SourceErrorManager,
        ambient_decls: &'ad [NodeRc],
        compile: bool,
    ) -> SemanticResolver<'bt, 'sc, 'sm, 'ad> {
        // ES14.0 19.1 Value properties of the global object
        // https://262.ecma-international.org/14.0/#sec-value-properties-of-the-global-object
        // These are the only non-configurable properties.
        let mut restricted_global_properties = HashSet::new();
        restricted_global_properties.insert(sem_ctx.kw.ident_na_n);
        restricted_global_properties.insert(sem_ctx.kw.ident_undefined);
        restricted_global_properties.insert(sem_ctx.kw.ident_infinity);

        // Buffer all generated messages and print them sorted in the end.
        // Port of the `bufferMessages_{&sm_}` member initializer
        // (SemanticResolver.cpp:49); the matching `disableBuffering` is in
        // the `Drop` impl below.
        sm.enable_buffering();

        SemanticResolver {
            sem_ctx,
            sm,
            binding_table,
            ambient_decls,
            restricted_global_properties,
            compile,
            cur_scope: None,
            global_scope: BindingTableScopePtr::default(),
            function_stack: Vec::new(),
            global_function_context: None,
            binding_scopes: Vec::new(),
            recursion_depth: AST_MAX_RECURSION_DEPTH,
        }
    }

    /// Run semantic resolution and store the result in `sem_ctx`. Port of
    /// `SemanticResolver::run` (cpp:65-70).
    ///
    /// \param root the top-level program node to run resolution on.
    /// \return false on error.
    pub fn run<'ast>(
        &mut self,
        gc: &'ast GCLock,
        root: &'ast Node<'ast>,
    ) -> bool {
        if self.sm.error_count() != 0 {
            return false;
        }
        self.visit_node(gc, root);
        self.sm.error_count() == 0
    }

    /// True if we are preparing the AST to be compiled by Hermes. Port of
    /// the `compile_` field (SemanticResolver.h:84); an accessor here so
    /// that S1, which is where the flag is first *read*, doesn't have to
    /// widen the field's visibility.
    pub fn compile(&self) -> bool {
        self.compile
    }

    /// \return true if `name` is a non-configurable global property. Port of
    /// the `restrictedGlobalProperties_` field (SemanticResolver.h:54), read
    /// by `validateDeclarationNames` (S1).
    pub fn is_restricted_global_property(&self, name: Atom) -> bool {
        self.restricted_global_properties.contains(&name)
    }

    /// \return true if the innermost function context is the "global scope"
    /// context, in other words not a real function. Port of
    /// `FunctionContext::isGlobalScope` (SemanticResolver.h:613-617), which
    /// reads `globalFunctionContext_`.
    pub fn in_global_scope_context(&self) -> bool {
        match self.global_function_context {
            Some(idx) => idx + 1 == self.function_stack.len(),
            None => false,
        }
    }

    /// Keywords we will be checking for. See the module doc: C++ has this as
    /// the `kw_` field.
    fn kw(&self) -> &Keywords {
        &self.sem_ctx.kw
    }

    /// Port of `SemanticResolver::functionContext()`
    /// (SemanticResolver.h:174-176).
    fn function_context(&self) -> &FunctionContext {
        self.function_stack
            .last()
            .expect("no active function context")
    }

    /// Mutable form of [`Self::function_context`].
    fn function_context_mut(&mut self) -> &mut FunctionContext {
        self.function_stack
            .last_mut()
            .expect("no active function context")
    }

    /// Port of `SemanticResolver::curFunctionInfo()`
    /// (SemanticResolver.h:623-625).
    fn cur_function_info(&self) -> FunctionInfoId {
        self.function_context().sem_info
    }

    // ---- FunctionContext -------------------------------------------------

    /// Port of the `FunctionContext` constructor that creates a brand new
    /// `FunctionInfo` and a `DeclCollector` for `node`
    /// (SemanticResolver.cpp:2963-2992), fused with the `SaveAndRestore` of
    /// `globalFunctionContext_` its S0-reachable call site wraps it in
    /// (cpp:203).
    ///
    /// \param install_as_global_context port of that `SaveAndRestore`.
    #[allow(clippy::too_many_arguments)]
    fn enter_function<'ast>(
        &mut self,
        gc: &'ast GCLock,
        node: &'ast Node<'ast>,
        parent_sem_info: Option<FunctionInfoId>,
        strict: bool,
        cons_kind: ConstructorKind,
        custom_directives: CustomDirectives,
        install_as_global_context: bool,
    ) -> FunctionState {
        let sem_info = self.sem_ctx.new_function(
            SemContext::node_is_arrow(Some(node)),
            cons_kind,
            parent_sem_info,
            self.cur_scope,
            strict,
            custom_directives,
        );
        // C++'s depth-exceeded lambda (cpp:2985-2989) mutates the resolver
        // from inside the collector's walk. That closure cannot borrow
        // `self` mutably here (the `kw` argument already borrows it), so it
        // only records the offending node and the two effects are applied
        // right after the walk. Not observable: nothing reads
        // `recursion_depth` during the walk (the collector took its own
        // copy), and nothing else emits a diagnostic during it either, so
        // neither the value nor the diagnostic order can differ.
        let mut depth_exceeded_at: Option<NodeRc> = None;
        let decls = DeclCollector::run(
            node,
            gc,
            &self.sem_ctx.kw,
            self.recursion_depth,
            &mut |n| depth_exceeded_at = Some(NodeRc::from_node(gc, n)),
        );
        if let Some(n) = depth_exceeded_at {
            // Inform the resolver that we have gone too deep.
            self.recursion_depth = 0;
            self.recursion_depth_exceeded(gc, &n);
        }

        self.function_stack.push(FunctionContext {
            sem_info,
            node: Some(NodeRc::from_node(gc, node)),
            label_map: HashMap::new(),
            current_loop: None,
            current_loop_or_switch: None,
            is_formal_params: false,
            decls: Some(decls),
            promoted_func_decls: HashMap::new(),
            binding_table_scope_depth: 0,
        });
        if install_as_global_context {
            self.global_function_context = Some(self.function_stack.len() - 1);
        }
        set_node_sem_info(node, sem_info);
        FunctionState {
            was_global_function_context: install_as_global_context,
        }
    }

    /// Port of `FunctionContext::~FunctionContext`
    /// (SemanticResolver.cpp:3049-3070) plus the call site's
    /// `SaveAndRestore` restore.
    fn exit_function(&mut self, state: FunctionState) {
        self.function_stack
            .pop()
            .expect("no active function context");
        if state.was_global_function_context {
            self.global_function_context = None;
        }
    }

    /// Port of `SemanticResolver::recursionDepthExceeded`
    /// (SemanticResolver.cpp:2759-2762).
    fn recursion_depth_exceeded(&mut self, gc: &GCLock, node: &NodeRc) {
        let end_loc = node.node(gc).range().end;
        self.sm.error(
            end_loc,
            "Too many nested expressions/statements/declarations",
        );
    }

    // ---- ScopeRAII -------------------------------------------------------

    /// Create a binding scope and push a semantic scope. Port of
    /// `SemanticResolver::ScopeRAII::ScopeRAII`
    /// (SemanticResolver.cpp:2919-2944); the C++ member-initializer list
    /// runs before the constructor body, so the binding scope is pushed
    /// first.
    ///
    /// \param scope_node the AST node with which to associate the scope.
    /// \param is_function_body_scope whether this is the scope for the
    ///   function body of the current `FunctionInfo`.
    fn enter_scope(
        &mut self,
        scope_node: Option<&Node>,
        is_function_body_scope: bool,
    ) -> ScopeState {
        let old_scope = self.cur_scope;
        // `binding_table` is a `&'bt` copied out of `self` before the `&mut
        // self` uses below, so the resulting `Scope<'bt, ..>` does not
        // borrow `self` — see the module doc.
        let binding_table = self.binding_table;
        self.binding_scopes.push(Scope::new(binding_table));

        // Create a new scope.
        let scope = self
            .sem_ctx
            .new_scope(self.cur_function_info(), self.cur_scope);
        self.cur_scope = Some(scope);
        // Optionally associate the scope with the node.
        if let Some(scope_node) = scope_node {
            set_node_scope(scope_node, scope);
        }

        if DEBUG_INFO_SETTING_ALL {
            let ptr = self.binding_table.current_scope();
            self.sem_ctx.scope_mut(scope).binding_table_scope = ptr;
        }

        if is_function_body_scope {
            let func = self.cur_function_info();
            let idx = self.sem_ctx.function(func).get_scopes().len() as u32 - 1;
            self.sem_ctx.function_mut(func).function_body_scope_idx = idx;
            let depth = self.cur_binding_scope().depth();
            self.function_context_mut().binding_table_scope_depth = depth;
        }
        ScopeState { old_scope }
    }

    /// Pops the created scope. Port of
    /// `SemanticResolver::ScopeRAII::~ScopeRAII`
    /// (SemanticResolver.cpp:2945-2947) plus the implicit destruction of the
    /// `bindingScope_` member (which, being declared last, is destroyed
    /// first).
    fn exit_scope(&mut self, state: ScopeState) {
        self.binding_scopes.pop().expect("no open binding scope");
        self.cur_scope = state.old_scope;
    }

    /// \return the innermost open binding scope, i.e. the `bindingScope_` of
    /// the innermost live `ScopeRAII` (C++ `ScopeRAII::getBindingScope()`).
    fn cur_binding_scope(&self) -> &Scope<'bt, Atom, Binding> {
        self.binding_scopes.last().expect("no open binding scope")
    }

    // ---- Visitors --------------------------------------------------------

    /// Dispatch to the `visit()` overload for `node`'s kind. Port of
    /// `visitESTreeNodeNoReplace(*this, node)`, whose C++ dispatch is
    /// generated by the RecursiveVisitor machinery.
    ///
    /// S0 only implements the kinds its corpus can produce; see the module
    /// doc for why the fallback is a panic rather than a generic recursion.
    fn visit_node<'ast>(&mut self, gc: &'ast GCLock, node: &'ast Node<'ast>) {
        match node {
            Node::Program(_) => self.visit_program(gc, node),
            Node::ExpressionStatement(n) => {
                // There is no `visit(ExpressionStatementNode *)` overload in
                // C++; the generic dispatch visits the children, which for
                // this node is just `_expression` (`_directive` is a
                // NodeString, not a node).
                self.visit_node(gc, n.expression);
            }
            // Leaves: no `visit()` overload and no children.
            Node::EmptyStatement(_)
            | Node::NumericLiteral(_)
            | Node::StringLiteral(_)
            | Node::BooleanLiteral(_)
            | Node::NullLiteral(_) => {}
            _ => panic!(
                "sema S0: unhandled node kind {} — S1+",
                node.node_type_str()
            ),
        }
    }

    /// Port of `SemanticResolver::visit(ESTree::ProgramNode *node)`
    /// (cpp:193-231).
    ///
    /// `node` is the enclosing `Node` (rather than the `Program` payload)
    /// because the ported helpers all take `&Node`.
    fn visit_program<'ast>(
        &mut self,
        gc: &'ast GCLock,
        node: &'ast Node<'ast>,
    ) {
        let program = match node {
            Node::Program(p) => p,
            _ => unreachable!("visit_program called on a non-Program node"),
        };
        // C++ reads `astContext_.isStrictMode()`; this port has no
        // `astContext_` field and `GCLock::strict_mode()` is the same flag.
        let ctx_strict_mode = gc.ctx().strict_mode();
        let func_state = self.enter_function(
            gc,
            node,
            None,
            ctx_strict_mode,
            ConstructorKind::None,
            CustomDirectives {
                source_visibility: SourceVisibility::Default,
                always_inline: false,
                ..Default::default()
            },
            /* install_as_global_context */ true,
        );
        let directives = self.scan_directives(program.body.iter());
        if directives.use_strict_node.is_some() {
            let f = self.cur_function_info();
            self.sem_ctx.function_mut(f).strict = true;
        }
        let f = self.cur_function_info();
        program
            .strictness
            .set(make_strictness(self.sem_ctx.function(f).strict));
        if directives.source_visibility
            > self.sem_ctx.function(f).custom_directives.source_visibility
        {
            self.sem_ctx
                .function_mut(f)
                .custom_directives
                .source_visibility = directives.source_visibility;
        }
        self.sem_ctx.function_mut(f).is_program_node = true;

        {
            let scope_state =
                self.enter_scope(Some(node), /* functionScope */ true);
            // C++ wraps this assignment in `llvh::SaveAndRestore<...>
            // saveGlobalScope(globalScope_, ...)` (SemanticResolver.cpp:227)
            // so `globalScope_` reverts to its enclosing value on return —
            // needed once `visitProgram` can recurse (lazy compilation,
            // direct `eval`). The S0 entry path only ever visits one
            // `Program`, so the restore is not yet observable; it is
            // deliberately not ported until the S5 lazy/eval work lands.
            self.global_scope = self.cur_binding_scope().ptr();
            self.sem_ctx
                .set_binding_table_global_scope(self.global_scope.clone());
            if DEBUG_INFO_SETTING_ALL {
                let f = self.cur_function_info();
                self.sem_ctx.function_mut(f).binding_table_scope =
                    self.global_scope.clone();
            }

            self.process_collected_declarations(node);
            if !self.sem_ctx.function(self.cur_function_info()).strict {
                // Promote hoisted functions.
                //
                // `getPromotedScopedFuncDecls`/`processPromotedFuncDecls`
                // are S3 scope. Nothing S0 can parse produces a scoped
                // function declaration, so assert that rather than silently
                // skipping the promotion.
                assert!(
                    self.function_context()
                        .decls
                        .as_ref()
                        .expect("Program FunctionContext always has decls")
                        .scoped_func_decls()
                        .is_empty(),
                    "sema S0: scoped function declarations are S3 scope"
                );
            }
            self.process_ambient_decls(gc);
            // visitESTreeChildren(*this, node): a Program's only child list
            // is `_body`.
            for child in program.body.iter() {
                self.visit_node(gc, child);
            }
            self.exit_scope(scope_state);
        }
        self.exit_function(func_state);
    }

    /// Port of `SemanticResolver::processCollectedDeclarations`
    /// (cpp:2088-2093).
    ///
    /// S0 stops at the lookup: `processDeclarations` (cpp:2095-2127) and
    /// everything it calls is S1 scope. Reaching a non-empty list means the
    /// corpus grew past what this resolver models, so panic rather than
    /// silently drop the declarations.
    fn process_collected_declarations(&mut self, scope_node: &Node) {
        let decls_opt = self
            .function_context()
            .decls
            .as_ref()
            .expect("FunctionContext without a DeclCollector")
            .scope_decls_for_node(scope_node.node_id());
        if decls_opt.is_some() {
            // `scope_decls_for_node` only ever returns non-empty lists (see
            // `DeclCollector::close_scope`).
            panic!("sema S0: declarations are S1 scope");
        }
    }

    /// Scan the directive prologue of `body`. Port of
    /// `SemanticResolver::scanDirectives` (cpp:2764-2814).
    ///
    /// The C++ `else if (directive == X) { if (cond) ... }` chain is written
    /// here as `else if (directive == X && cond)`. That is equivalent: the
    /// keyword atoms are pairwise distinct, so a directive that matched `X`
    /// can never match a later arm of the chain.
    fn scan_directives<'ast, I>(&mut self, body: I) -> FoundDirectives<'ast>
    where
        I: IntoIterator<Item = &'ast Node<'ast>>,
    {
        let kw_use_strict = self.kw().ident_use_strict;
        let kw_show_source = self.kw().ident_show_source;
        let kw_hide_source = self.kw().ident_hide_source;
        let kw_sensitive = self.kw().ident_sensitive;
        let kw_inline = self.kw().ident_inline;
        let kw_no_inline = self.kw().ident_no_inline;
        let kw_builtin = self.kw().ident_builtin;

        let mut directives = FoundDirectives::default();
        for node in body {
            let expr_st = match node {
                Node::ExpressionStatement(e) => e,
                _ => break,
            };
            let directive = expr_st.directive.get();
            if directive == atom_table::INVALID_ATOM_BYTES {
                break;
            }

            if directive == kw_use_strict {
                // `get_or_insert`: C++'s `if (!useStrictNode) useStrictNode
                // = exprSt;` keeps the FIRST such statement.
                directives.use_strict_node.get_or_insert(node);
            } else if directive == kw_show_source
                && SourceVisibility::ShowSource > directives.source_visibility
            {
                directives.source_visibility = SourceVisibility::ShowSource;
            } else if directive == kw_hide_source
                && SourceVisibility::HideSource > directives.source_visibility
            {
                directives.source_visibility = SourceVisibility::HideSource;
            } else if directive == kw_sensitive
                && SourceVisibility::Sensitive > directives.source_visibility
            {
                directives.source_visibility = SourceVisibility::Sensitive;
            }

            // Shouldn't have both 'inline' and 'noinline'.  But this
            // shouldn't prevent compilation.  So, give a warning, and take
            // the most recent directive.
            if directive == kw_inline {
                if directives.no_inline {
                    self.sm.warning_range(
                        Warning::Misc,
                        node.range(),
                        "Should not declare both 'inline' and 'noinline'.",
                        Subsystem::Unspecified,
                    );
                    directives.no_inline = false;
                }
                directives.always_inline = true;
            }
            if directive == kw_no_inline {
                if directives.always_inline {
                    self.sm.warning_range(
                        Warning::Misc,
                        node.range(),
                        "Should not declare both 'inline' and 'noinline'.",
                        Subsystem::Unspecified,
                    );
                    directives.always_inline = false;
                }
                directives.no_inline = true;
            }
            if directive == kw_builtin {
                directives.builtin = true;
            }
        }
        directives
    }

    /// Declare the list of ambient decls that was passed to the constructor.
    /// Port of `SemanticResolver::processAmbientDecls` (cpp:2846-2917).
    fn process_ambient_decls(&mut self, gc: &GCLock) {
        assert!(
            !self.global_scope.is_null(),
            "global scope must be created when declaring ambient globals"
        );

        let ambient_decls = self.ambient_decls;
        if ambient_decls.is_empty() {
            return;
        }

        for program_node in ambient_decls {
            let mut dh = DeclHoisting::default();
            dh.visit_node(program_node.node(gc));
            // Create variable declarations for each of the hoisted
            // variables.
            for vd in &dh.decls {
                self.declare_ambient_global(*vd);
            }
            for fd in &dh.closures {
                self.declare_ambient_global(*fd);
            }
        }
    }

    /// Port of the `declareAmbientGlobal` lambda (cpp:2897-2906).
    ///
    /// \param name the `_name` of the `IdentifierNode` being declared; C++
    ///   takes the node and casts it, but the name is all it uses.
    fn declare_ambient_global(&mut self, name: Atom) {
        // If we find the binding, do nothing.
        if self.binding_table.count(&name) == 0 {
            let decl = self
                .sem_ctx
                .new_global(name, DeclKind::UndeclaredGlobalProperty);
            self.binding_table.try_emplace_into_scope(
                &self.global_scope,
                name,
                Binding::new(decl, None),
            );
        }
    }
}

/// Port of the `bufferMessages_` member's destruction
/// (`SourceErrorManager::SaveAndBufferMessages::~SaveAndBufferMessages`,
/// SourceErrorManager.h:640-642): flush every diagnostic the resolver
/// produced, stable-sorted by source position.
///
/// This is the one C++ RAII object on this path that maps onto a Rust `Drop`
/// guard rather than an `enter_*`/`exit_*` pair (see the module doc): it
/// needs only the `&mut SourceErrorManager` the resolver already owns, not
/// `&mut SemanticResolver`, so there is no borrow conflict. Its lifetime is
/// the resolver's, exactly like the C++ member's — the flush therefore
/// happens when the resolver is dropped, not at the end of `run`.
impl Drop for SemanticResolver<'_, '_, '_, '_> {
    fn drop(&mut self) {
        // `binding_scopes` must be unwound back-to-front (innermost first),
        // matching `exit_scope`'s own `Vec::pop`: each `Scope`'s `Drop`
        // requires it to be the *current* (i.e. innermost) binding-table
        // scope, enforced by a `debug_assert!` in `pop_scope`. `Vec`'s
        // implicit `Drop` instead drops front-to-back (outermost first), so
        // if this resolver is dropped mid-unwind with >= 2 scopes still
        // open (e.g. a panic while resolving a nested block), that
        // debug_assert fires while already unwinding from another panic —
        // a double panic, which aborts the process instead of propagating
        // the original panic. Draining back-to-front here sidesteps that
        // regardless of how this resolver gets dropped.
        while self.binding_scopes.pop().is_some() {}
        self.sm.disable_buffering();
    }
}

/// This visitor struct collects declarations within a single closure without
/// descending into child closures. Port of `processAmbientDecls`'s local
/// `struct DeclHoisting` (cpp:2856-2895); its `enter`/`leave` are empty and
/// its `shouldVisit` is the body of `visit_node` below.
///
/// C++ collects the `VariableDeclaratorNode *`/`FunctionDeclarationNode *`
/// and reads `_id` at the use site; this port stores the declared *name*
/// directly, which is all `declareAmbientGlobal` needs and avoids rooting
/// nodes purely to re-read one field. A declarator whose `id` is not an
/// `Identifier` (a destructuring pattern) cannot occur in a
/// global-definitions file and would be a failing `cast<IdentifierNode>` in
/// C++; here it is an explicit panic.
#[derive(Default)]
struct DeclHoisting {
    /// The list of collected identifiers (variables).
    decls: Vec<Atom>,
    /// A list of functions that need to be hoisted and materialized before
    /// we can generate the rest of the function.
    closures: Vec<Atom>,
}

impl DeclHoisting {
    /// Extract the variable name from the nodes that can define new
    /// variables. The nodes that can define a new variable in the scope are:
    /// VariableDeclarator and FunctionDeclaration.
    fn collect_decls(&mut self, node: &Node) {
        match node {
            Node::VariableDeclarator(vd) => {
                self.decls.push(identifier_name(vd.id));
            }
            Node::FunctionDeclaration(fd) => {
                let id = fd
                    .id
                    .expect("ambient FunctionDeclaration must have a name");
                self.closures.push(identifier_name(id));
            }
            _ => {}
        }
    }
}

impl<'gc> Visitor<'gc> for DeclHoisting {
    fn visit_node(&mut self, node: &'gc Node<'gc>) {
        // Collect declared names, even if we don't descend into children
        // nodes.
        self.collect_decls(node);

        // Do not descend to child closures because the variables they
        // define are not exposed to the outside function.
        if matches!(
            node,
            Node::FunctionDeclaration(_)
                | Node::FunctionExpression(_)
                | Node::ArrowFunctionExpression(_)
        ) {
            return;
        }
        node.visit_children(self);
    }
}

/// \return the `name` of an `Identifier` node.
fn identifier_name(node: &Node) -> Atom {
    match node {
        Node::Identifier(id) => id.name.get(),
        _ => panic!(
            "ambient declaration name is a {}, not an Identifier",
            node.node_type_str()
        ),
    }
}

/// Port of `ESTree::makeStrictness` (ESTree.h).
fn make_strictness(strict: bool) -> Strictness {
    if strict {
        Strictness::StrictMode
    } else {
        Strictness::NonStrictMode
    }
}

/// Port of `scopeNode->setScope(scope)` in `ScopeRAII`
/// (SemanticResolver.cpp:2931-2932), i.e.
/// `ESTree::ScopeDecorationBase::setScope`. Enumerates the same 15
/// scope-bearing node kinds as `sema::dump`'s `node_scope`.
fn set_node_scope(node: &Node, scope: ScopeId) {
    let id = Some(scope.sema_id());
    match node {
        Node::Program(n) => n.scope.set(id),
        Node::FunctionExpression(n) => n.scope.set(id),
        Node::ArrowFunctionExpression(n) => n.scope.set(id),
        Node::FunctionDeclaration(n) => n.scope.set(id),
        Node::ComponentDeclaration(n) => n.scope.set(id),
        Node::HookDeclaration(n) => n.scope.set(id),
        Node::ForInStatement(n) => n.scope.set(id),
        Node::ForOfStatement(n) => n.scope.set(id),
        Node::ForStatement(n) => n.scope.set(id),
        Node::BlockStatement(n) => n.scope.set(id),
        Node::StaticBlock(n) => n.scope.set(id),
        Node::SwitchStatement(n) => n.scope.set(id),
        Node::CatchClause(n) => n.scope.set(id),
        Node::ClassDeclaration(n) => n.scope.set(id),
        Node::ClassExpression(n) => n.scope.set(id),
        _ => {
            panic!("{} does not carry a scope decoration", node.node_type_str())
        }
    }
}

/// Port of `node->setSemInfo(semInfo)` (SemanticResolver.cpp:2991), i.e.
/// `ESTree::FunctionLikeDecoration::setSemInfo`. Enumerates the same six
/// function-like node kinds as `sema::dump`'s `function_like_sem_info`.
fn set_node_sem_info(node: &Node, sem_info: FunctionInfoId) {
    let id: Option<SemaId> = Some(sem_info.sema_id());
    match node {
        Node::Program(n) => n.sem_info.set(id),
        Node::FunctionExpression(n) => n.sem_info.set(id),
        Node::ArrowFunctionExpression(n) => n.sem_info.set(id),
        Node::FunctionDeclaration(n) => n.sem_info.set(id),
        Node::ComponentDeclaration(n) => n.sem_info.set(id),
        Node::HookDeclaration(n) => n.sem_info.set(id),
        _ => panic!("{} is not a function-like node", node.node_type_str()),
    }
}
