/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! S3 T1: port of `lib/Sema/ScopedFunctionPromoter.{h,cpp}` (the whole file).
//!
//! "This function checks whether it is safe to promote block-scoped function
//! declarations to function scope. i.e. whether it is safe to replace one
//! with "var" without creating a conflict.
//!
//! A conflict exists if a let-like declaration is visible in the declaration
//! scope. The checker starts with a list of all block scoped function
//! declarations. Then it visits all scopes recursively, maintaining a scoped
//! table of let-like declarations with matching names. When it encounters a
//! block-scoped function declaration, it checks whether a matching let-like
//! declaration is visible. If not, it is safe to promote.
//!
//! The input is the list of block-scoped function declarations collected in
//! the current function. \return the ones that can be safely promoted."
//! (ScopedFunctionPromoter.h:17-29.)
//!
//! Nothing is deleted from any scope and nothing is added to the function
//! scope here — `getPromotedScopedFuncDecls` only *returns* the promotable
//! list, and the caller (`SemanticResolver::processPromotedFuncDecls`,
//! cpp:2143-2155) declares the names in the function/global scope while the
//! block's own `ScopedFunction` declaration survives. The header used to
//! promise otherwise, and `processDeclarations` used to carry a write-only
//! `newDecls` local as the last vestige of it; both were removed upstream in
//! `9232443cf` and this port follows.
//!
//! ## Deviations
//!
//! - **Node identity is `hermes_ast::NodeId`, not a raw pointer.** C++'s `funcDecls_`
//!   is a `SmallDenseSet<FunctionDeclarationNode *>` keyed by pointer
//!   identity; this port keys it by `NodeId`, the same substitution
//!   `DeclCollector` makes for its `scopes_` map (see that module's doc). The
//!   promoter is a read-only pass that runs to completion inside one visit,
//!   so no node it looks at can be rebuilt underneath it.
//! - **The result is `Vec<NodeRc>`, not `Vec<&Node>`.** The promotable
//!   declarations are handed back to the caller as the `NodeRc`s the
//!   `DeclCollector` already holds: `NodeRc::node` ties the returned `&Node`'s
//!   lifetime to the borrow of the `NodeRc` it came from, i.e. to the borrow
//!   of the resolver, so a `Vec<&Node>` could not outlive this call.
//!   `SemanticResolver::process_promoted_func_decls` therefore takes
//!   `&[NodeRc]`, exactly like `process_declarations` takes its `ScopeDecls`.
//! - **The C++ `SemanticResolver &resolver_` member becomes the three pieces
//!   of the resolver this pass actually uses** — the `DeclCollector`, the
//!   `SemContext` (for the parameter scope and the `Keywords`) and the
//!   `SourceErrorManager` (which is all `extractDeclaredIdentsFromID` needs).
//!   Holding the whole `&mut SemanticResolver` would make the shared borrow
//!   of `functionContext()->decls` — live across the entire walk, since every
//!   `processDeclarations` reads it — conflict with the `&mut` the extraction
//!   needs. Splitting the borrow at the entry point instead (three disjoint
//!   fields) keeps the C++ code shape without cloning the collector's tables.
//!   That is why `declarations.rs` exposes the *body* of
//!   `extractDeclaredIdentsFromID` as a free function over
//!   `&mut SourceErrorManager`, with the resolver method forwarding to it: it
//!   is the same code, in one place, callable from both borrow shapes.
//! - **`incRecursionDepth`/`decRecursionDepth`** (cpp:69-74) exist only to
//!   satisfy `RecursiveVisitorDispatch` and are unconditional no-ops
//!   (`return true;` / `{}`), i.e. this pass has no depth limit in C++
//!   either. `hermes_ast::visitor::Visitor` has no depth hooks, so they have no
//!   counterpart — same as `unresolver.rs`.
//! - **`acquirePromotedFuncDecls`** (cpp:32-34) is a move-out accessor; here
//!   the entry point simply consumes the promoter value's field.
//!
//! The two `#if HERMES_PARSE_FLOW`/`#if HERMES_PARSE_TS` guards
//! (`TypeAlias`/`TSTypeAliasDeclaration` in `processDeclarations`,
//! `HookDeclaration`/`ComponentDeclaration` in `extractDeclaredIdents`) are
//! ported UNCONDITIONALLY: this port has a single node set containing every
//! dialect's nodes (see the crate doc), the same call
//! `declarations.rs`'s `process_declarations` and `extract_idents_from_decl`
//! already make.

use std::collections::HashSet;

use hermes_ast::context::{GCLock, NodeRc};
use hermes_ast::node::Node;
use hermes_ast::visitor::Visitor;
use hermes_ast::NodeId;
use hermes_support::manager::SourceErrorManager;
use hermes_support::persistent_scoped_map::{PersistentScopedMap, Scope};

use crate::decl_collector::{DeclCollector, ScopeDecls};
use crate::ids::FunctionInfoId;
use crate::sem_context::{Atom, DeclKind, SemContext};

use super::declarations::extract_declared_idents_from_id;
use super::functions::function_like_body;
use super::SemanticResolver;

/// Port of `ScopedFunctionPromoter::BindingTableTy`
/// (`hermes::ScopedHashTable<UniqueString *, bool>`, cpp:112-114). This
/// port's analog of `ScopedHashTable` is `PersistentScopedMap` — the same
/// type `SemContext::binding_table` uses; the "persistent" (scope-retaining)
/// half is simply unused here.
type PromoterBindingTable = PersistentScopedMap<Atom, bool>;

/// Port of the anonymous-namespace `ScopedFunctionPromoter` visitor class
/// (cpp:22-118).
///
/// `gc`'s outer reference lifetime is tied to `'ast` while `GCLock`'s own two
/// parameters stay independent, for exactly the reason spelled out on
/// `decl_collector::Collector::gc`.
struct ScopedFunctionPromoter<'ast, 'g_ast, 'g_ctx, 'd, 'sc, 'sm, 'tb> {
    gc: &'ast GCLock<'g_ast, 'g_ctx>,
    /// `resolver_.functionContext()->decls` (cpp:123, 162).
    decls: &'d DeclCollector,
    /// `resolver_.keywords()` (cpp:246-251) and the parameter scope
    /// (cpp:148); see the module doc on splitting `resolver_`.
    sem_ctx: &'sc SemContext,
    /// Everything `extractDeclaredIdentsFromID` needs (cpp:243, 261, ...).
    sm: &'sm mut SourceErrorManager,

    /// The result list of promoted function declarations. Port of
    /// `promotedFuncDecls_` (cpp:102-103).
    promoted_func_decls: Vec<NodeRc>,

    /// The names of the scoped functions. We will ignore all other
    /// identifiers. Port of `funcNames_` (cpp:105-106).
    func_names: HashSet<Atom>,

    /// The scoped function declarations. We remove each from this set once
    /// we encounter it. Port of `funcDecls_` (cpp:108-110), keyed by
    /// `NodeId` — see the module doc.
    func_decls: HashSet<NodeId>,

    /// The currently lexically visible names. Port of `bindingTable_`
    /// (cpp:116-117); owned by [`get_promoted_scoped_func_decls`] so that a
    /// [`Scope`] can borrow it while this struct is mutably borrowed.
    binding_table: &'tb PromoterBindingTable,
}

impl<'ast, 'd, 'sc, 'sm, 'tb>
    ScopedFunctionPromoter<'ast, '_, '_, 'd, 'sc, 'sm, 'tb>
{
    /// Run the AST pass. Port of `ScopedFunctionPromoter::run`
    /// (cpp:120-139).
    ///
    /// \param func_sem_info C++ reads `funcNode->getSemInfo()` (cpp:148); at
    ///   both ported call sites `func_node` IS the current function context's
    ///   node, whose `sem_info` decoration `enter_function` set from this
    ///   very `FunctionInfo`, so the caller passes it directly rather than
    ///   re-deriving it from the node.
    fn run(
        &mut self,
        func_node: &'ast Node<'ast>,
        func_sem_info: FunctionInfoId,
    ) {
        let binding_scope = Scope::new(self.binding_table);
        let decls = self.decls.scoped_func_decls();

        // Populate the sets.
        for node in decls {
            let node = node.node(self.gc);
            let func_decl = match node {
                Node::FunctionDeclaration(fd) => fd,
                _ => panic!(
                    "cast<FunctionDeclarationNode> failed: scoped func decl \
                     is a {}",
                    node.node_type_str()
                ),
            };
            let id = func_decl
                .id
                .expect("cast<IdentifierNode>(funcDecl->_id) on a nameless \
                         scoped function declaration");
            self.func_names.insert(identifier_name(id));
            self.func_decls.insert(node.node_id());
        }

        self.process_parameters(func_sem_info);
        self.process_declarations(func_node);
        if matches!(func_node, Node::Program(_)) {
            func_node.visit_children(self);
        } else {
            // `getBlockStatement(funcNode)` (lib/AST/ESTree.cpp:58-81). Both
            // ported call sites guard on the body being a `BlockStatement`
            // (cpp:1919's `if (blockBody)`; a `Program` took the branch
            // above), which is what makes the `cast` safe there and this
            // `debug_assert` a restatement of it rather than a new rule.
            let body = function_like_body(func_node);
            debug_assert!(
                matches!(body, Node::BlockStatement(_)),
                "getBlockStatement: expression-bodied function"
            );
            body.visit_children(self);
        }
        drop(binding_scope);
    }

    /// Visit any statement starting a scope. Port of
    /// `ScopedFunctionPromoter::visitScope` (cpp:141-145).
    fn visit_scope(&mut self, node: &'ast Node<'ast>) {
        let binding_scope = Scope::new(self.binding_table);
        self.process_declarations(node);
        node.visit_children(self);
        drop(binding_scope);
    }

    /// Add the formal parameters of \p func to the binding table if they have
    /// names we care about, because they must also prevent function
    /// promotion. ES2022 B.3.2.1 29.a.ii. Needed to check "parameterNames
    /// does not contain F". Port of
    /// `ScopedFunctionPromoter::processParameters` (cpp:84, 147-158).
    fn process_parameters(&self, func_sem_info: FunctionInfoId) {
        let sem_ctx = self.sem_ctx;
        let param_scope = sem_ctx.function(func_sem_info).get_parameter_scope();
        for &decl_id in &sem_ctx.scope(param_scope).decls {
            let decl = sem_ctx.decl(decl_id);
            if decl.kind == DeclKind::Parameter {
                let name = decl.name;
                if self.func_names.contains(&name) {
                    // Found a parameter with a name we care about, add it to
                    // the binding table.
                    self.binding_table.try_emplace(name, true);
                }
            }
        }
    }

    /// Process the declarations in a scope. This is the core of the
    /// algorithm, it updates the binding tables, etc. Port of
    /// `ScopedFunctionPromoter::processDeclarations` (cpp:86-88, 160-236).
    fn process_declarations(&mut self, scope: &Node) {
        // Copy the shared reference out of `self` first: the `ScopeDecls`
        // borrow below must not keep `*self` borrowed, since the loop needs
        // `&mut self` for `extract_declared_idents`.
        let collector = self.decls;
        let decls: &ScopeDecls =
            match collector.scope_decls_for_node(scope.node_id()) {
                Some(d) => d,
                None => return,
            };

        let mut idents: Vec<&Node> = Vec::new();
        // Whenever we encounter one of the scoped func decls we are trying to
        // promote, we store the address of its list entry here (so we can
        // clear it if we want to).
        let mut found_decls: Vec<&NodeRc> = Vec::new();

        for node_ref in decls {
            // C++ opens with `Node *node = nodeRef; if (!node) continue;`,
            // guarding against an entry a (never-implemented) removal pass
            // would have nulled out. A `ScopeDecls` element is a `NodeRc`,
            // which cannot be null, so the guard has no counterpart.
            let node = node_ref.node(self.gc);

            // DeclCollector collects type aliases, but ScopedFunctionPromoter
            // should skip them.
            if matches!(
                node,
                Node::TypeAlias(_) | Node::TSTypeAliasDeclaration(_)
            ) {
                continue;
            }

            if matches!(node, Node::FunctionDeclaration(_)) {
                if self.func_decls.contains(&node.node_id()) {
                    // We encountered one of the candidate declarations.
                    // Add it to the found_decls list and move on.
                    found_decls.push(node_ref);
                }
                continue;
            }

            // Extract idents, report errors.
            idents.clear();
            let decl_kind = self.extract_declared_idents(node, &mut idents);

            // We are only interested in let-like declarations, but not
            // ES5Catch. ES5Catch doesn't conflict with Var declarations.
            // See ES14.0 B.3.4.
            if !decl_kind.is_let_like() || decl_kind == DeclKind::ES5Catch {
                continue;
            }

            // Remember only idents matching the set.
            for id_node in &idents {
                let name = identifier_name(id_node);
                if self.func_names.contains(&name) {
                    self.binding_table.try_emplace(name, true);
                }
            }
        }

        if found_decls.is_empty() {
            // No work to do.
            return;
        }

        // Did we finally encounter one of the scoped function declarations?
        for func_decl_ref in found_decls {
            let node = func_decl_ref.node(self.gc);
            let func_decl = match node {
                Node::FunctionDeclaration(fd) => fd,
                _ => panic!(
                    "cast<FunctionDeclarationNode> failed: found decl is a {}",
                    node.node_type_str()
                ),
            };
            // Remove it from the set, since we are no longer interested in it.
            self.func_decls.remove(&node.node_id());

            if let Some(id) = func_decl.id {
                // C++'s `bindingTable_.lookup(name)` returns a
                // default-constructed `false` for a name that is not in the
                // table, so "absent" and "present as false" are the same
                // thing there; only `true` is ever inserted.
                if !self
                    .binding_table
                    .lookup(&identifier_name(id))
                    .unwrap_or_default()
                {
                    // There's no visible let-like declaration with the same
                    // name. So this decl can be promoted because it would not
                    // shadow a `let`.
                    // Add it to the function scope list.
                    self.promoted_func_decls.push(func_decl_ref.clone());
                }
            }
        }
    }

    /// Extract the list of declared identifiers in a declaration node into
    /// `idents`. \return the declaration kind of the node. Function
    /// declarations are always returned as `ScopedFunction`, so they can be
    /// distinguished. Port of
    /// `ScopedFunctionPromoter::extractDeclaredIdents` (cpp:90-97, 238-306).
    ///
    /// This is deliberately NOT
    /// `SemanticResolver::extract_idents_from_decl` (cpp:2276-2366,
    /// `declarations.rs`): C++ keeps the two apart and the kind mapping
    /// differs — here a `FunctionDeclaration` is *always* `ScopedFunction`,
    /// while the resolver's version maps a top-level one to `Var`/
    /// `GlobalProperty`, and a `VariableDeclaration`'s `var` is `Var` here
    /// but `GlobalProperty` there at global scope.
    fn extract_declared_idents<'a>(
        &mut self,
        node: &'a Node<'a>,
        idents: &mut Vec<&'a Node<'a>>,
    ) -> DeclKind {
        if let Node::VariableDeclaration(var_declaration) = node {
            for declarator in var_declaration.declarations.iter() {
                let vd = match declarator {
                    Node::VariableDeclarator(vd) => vd,
                    _ => panic!(
                        "cast<VariableDeclaratorNode> failed: {}",
                        declarator.node_type_str()
                    ),
                };
                extract_declared_idents_from_id(self.sm, Some(vd.id), idents);
            }
            let kind = var_declaration.kind.get();
            return if kind == self.sem_ctx.kw.ident_var {
                DeclKind::Var
            } else if kind == self.sem_ctx.kw.ident_let {
                DeclKind::Let
            } else {
                // `const`, `using` and `await using` are all lexically scoped
                // and block function promotion the same way. Note that
                // `using` declarations reach this point even though they are
                // not supported yet, because the promoter runs before the
                // resolver reports them. This mirrors
                // SemanticResolver::extractIdentsFromDecl().
                //
                // The pinned C++ landmine here — an assert that the kind is
                // `var` once it is neither `let` nor `const`, which aborted a
                // Debug hermesc on `using x = 1; { function f() {} }` — was
                // fixed upstream in `4ad67c992`.
                DeclKind::Const
            };
        }

        if let Node::FunctionDeclaration(fd) = node {
            extract_declared_idents_from_id(self.sm, fd.id, idents);
            return DeclKind::ScopedFunction;
        }

        if let Node::HookDeclaration(hd) = node {
            extract_declared_idents_from_id(self.sm, Some(hd.id), idents);
            return DeclKind::ScopedFunction;
        }

        if let Node::ComponentDeclaration(cd) = node {
            extract_declared_idents_from_id(self.sm, Some(cd.id), idents);
            return DeclKind::ScopedFunction;
        }

        if let Node::ClassDeclaration(cd) = node {
            extract_declared_idents_from_id(self.sm, cd.id, idents);
            return DeclKind::Class;
        }

        if let Node::CatchClause(catch_clause) = node {
            extract_declared_idents_from_id(
                self.sm,
                catch_clause.param,
                idents,
            );
            return if matches!(catch_clause.param, Some(Node::Identifier(_))) {
                DeclKind::ES5Catch
            } else {
                DeclKind::Catch
            };
        }

        // C++'s final block is an unconditional `cast<ImportDeclarationNode>`
        // (cpp:292): any other node kind aborts there, so it does here
        // too. Unreachable for `DeclCollector`-collected scope decls — the
        // collector only ever records the kinds handled above plus
        // `ImportDeclaration` and the two type aliases `processDeclarations`
        // skipped before calling this.
        let id = match node {
            Node::ImportDeclaration(id) => id,
            _ => panic!(
                "cast<ImportDeclarationNode> failed: unexpected scope decl \
                 kind {}",
                node.node_type_str()
            ),
        };
        for spec in id.specifiers.iter() {
            match spec {
                Node::ImportSpecifier(is) => {
                    extract_declared_idents_from_id(
                        self.sm,
                        Some(is.local),
                        idents,
                    );
                }
                Node::ImportDefaultSpecifier(ids) => {
                    extract_declared_idents_from_id(
                        self.sm,
                        Some(ids.local),
                        idents,
                    );
                }
                _ => {
                    let ins = match spec {
                        Node::ImportNamespaceSpecifier(ins) => ins,
                        _ => panic!(
                            "cast<ImportNamespaceSpecifierNode> failed: {}",
                            spec.node_type_str()
                        ),
                    };
                    extract_declared_idents_from_id(
                        self.sm,
                        Some(ins.local),
                        idents,
                    );
                }
            }
        }
        DeclKind::Import
    }
}

impl<'ast> Visitor<'ast>
    for ScopedFunctionPromoter<'ast, '_, '_, '_, '_, '_, '_>
{
    /// The `visit` overload set (cpp:36-67), resolved the way C++ overload
    /// resolution would.
    fn visit_node(&mut self, node: &'ast Node<'ast>) {
        match node {
            // Do not descend into nested functions.
            // `void visit(FunctionLikeNode *) {}` (cpp:42-43) — the six
            // `FunctionLike` kinds of ESTree.def:35-103.
            Node::Program(_)
            | Node::FunctionExpression(_)
            | Node::ArrowFunctionExpression(_)
            | Node::FunctionDeclaration(_)
            | Node::ComponentDeclaration(_)
            | Node::HookDeclaration(_) => {}

            // All nodes with scopes (cpp:45-67).
            Node::SwitchStatement(_)
            | Node::BlockStatement(_)
            | Node::ForStatement(_)
            | Node::ForInStatement(_)
            | Node::ForOfStatement(_)
            | Node::WithStatement(_)
            | Node::CatchClause(_) => self.visit_scope(node),

            // Handle the default case for all nodes which we ignore, but we
            // still want to visit their children (cpp:36-40).
            _ => node.visit_children(self),
        }
    }
}

/// `cast<IdentifierNode>(node)->_name`, the idiom C++ open-codes at cpp:128
/// and 229. It also stands in for the plain `idNode->_name` of cpp:211, where
/// C++ already holds an `IdentifierNode *` because `extractDeclaredIdents`
/// fills a `SmallVectorImpl<IdentifierNode *>`; this port's `idents` is a
/// `Vec<&Node>` (the shape `extract_declared_idents_from_id` appends to), so
/// the cast is what recovers the type.
fn identifier_name(node: &Node) -> Atom {
    node.as_identifier()
        .expect("cast<IdentifierNode> failed: not an Identifier")
        .name
        .get()
}

/// Port of `hermes::sema::getPromotedScopedFuncDecls`
/// (ScopedFunctionPromoter.h:30-32, cpp:310-320).
///
/// \return the list of promoted function declarations — every entry is a
///   `FunctionDeclaration` node the caller must declare in function (or
///   global) scope. See the module doc for why this is `Vec<NodeRc>` rather
///   than C++'s `std::vector<FunctionDeclarationNode *>`.
pub(super) fn get_promoted_scoped_func_decls<'ast>(
    resolver: &mut SemanticResolver<'_, '_, '_, '_>,
    gc: &'ast GCLock,
    func_node: &'ast Node<'ast>,
) -> Vec<NodeRc> {
    let func_sem_info = resolver.cur_function_info();
    // Three DISJOINT field borrows of `*resolver` — see the module doc on
    // why the promoter does not hold the resolver itself. `function_context()`
    // is not used here because it borrows all of `*resolver`.
    let decls = resolver
        .function_stack
        .last()
        .expect("no active function context")
        .decls
        .as_ref()
        .expect("FunctionContext without a DeclCollector");
    if decls.scoped_func_decls().is_empty() {
        // No scoped function declarations, nothing to promote.
        return Vec::new();
    }
    let sem_ctx: &SemContext = resolver.sem_ctx;
    let sm: &mut SourceErrorManager = resolver.sm;

    let binding_table = PromoterBindingTable::new();
    let mut promoter = ScopedFunctionPromoter {
        gc,
        decls,
        sem_ctx,
        sm,
        promoted_func_decls: Vec::new(),
        func_names: HashSet::new(),
        func_decls: HashSet::new(),
        binding_table: &binding_table,
    };
    promoter.run(func_node, func_sem_info);
    // `acquirePromotedFuncDecls` (cpp:32-34).
    promoter.promoted_func_decls
}
