/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! S1 T5: declarations — hoisting, validation, blocks. A second `impl<'bt,
//! 'sc, 'sm, 'ad> SemanticResolver<'bt, 'sc, 'sm, 'ad>` block, split out of
//! `resolver/mod.rs` the same way `identifiers.rs` was (S1 T4) — see that
//! file's module doc for why a child module sees `mod.rs`'s private fields.
//!
//! Ports `SemanticResolver::extractIdentsFromDecl` (SemanticResolver.cpp:
//! 2262-2352), `extractDeclaredIdentsFromID` (cpp:2353-2405),
//! `processDeclarations` (cpp:2095-2127, replacing `mod.rs`'s S0 guard in
//! `process_collected_declarations`), `validateAndDeclareIdentifier`
//! (cpp:2407-2639), `validateDeclarationName` (cpp:2641-2677),
//! `visit(VariableDeclarationNode *)` (cpp:325-403) and
//! `visit(BlockStatementNode *, Node *)` (cpp:502-518).
//!
//! ## What's dormant
//!
//! - **`typed_`** is always `false` in this port (typed-mode/FlowChecker
//!   integration is S2 scope — see `resolver/mod.rs`'s module doc). The
//!   `processDeclarations` builtin-skip branch it guards (cpp:2108-2121)
//!   additionally needs `hasBuiltinDirective`/`hasBuiltinDecoration`
//!   (cpp:2816-2836), which are themselves unported S2 helpers — so rather
//!   than fabricate stand-ins for methods a later stage owns, the branch is
//!   ported as a `const TYPED: bool = false` guard around an `unreachable!`,
//!   matching the `DEBUG_INFO_SETTING_ALL` precedent in `mod.rs`.
//! - **`promotedFuncDecls`** (`FunctionContext::promoted_func_decls`) was
//!   always empty until S3 T1, which lands `ScopedFunctionPromoter`
//!   (`resolver/promoter.rs`) and the `processPromotedFuncDecls` that fills
//!   the map (`resolver/mod.rs`). Every branch that reads it here — the
//!   `Var, ScopedFunc` and `ES5Catch, ScopedFunc` redeclaration rows and the
//!   two-declarations-per-promoted-function block below — is live as of
//!   that task; `tests/sema_corpus/promotion-basic.js` and
//!   `promotion-blocked-by-let.js` exercise them end to end, on top of the
//!   hand-populated-map unit tests that covered them while the producer was
//!   still missing.
//! - **`ClassDeclaration`/`CatchClause`/`ImportDeclaration`** classification
//!   in `extract_idents_from_decl`/`extract_declared_idents_from_id` is
//!   ported in full but not corpus-reachable yet: `visit_node` (`mod.rs`)
//!   has no dispatch arm for those three node kinds (classes are S1 T8+,
//!   catch clauses and modules are later stages), so a corpus file
//!   containing one would panic on the "unhandled node kind" boundary
//!   before ever reaching this code. Unit-tested directly instead (see the
//!   `tests` module below) by calling `extract_idents_from_decl` on a
//!   hand-built node, bypassing the full visitor walk.

use ast::context::{GCLock, NodeRc};
use ast::node::Node;
use ast::visitor::TransformResult;
use support::diag::Subsystem;
use support::manager::SourceErrorManager;

use crate::ids::{DeclId, ScopeId};
use crate::sem_context::{Atom, Binding, DeclKind};

use super::SemanticResolver;

/// Port of the `typed_` member (SemanticResolver.h:84) as seen from
/// `processDeclarations` — always `false` in this port. See the module doc.
const TYPED: bool = false;

/// The text of `atom`, for error/warning messages. Same pattern as
/// `identifiers.rs`'s inline `String::from_utf8_lossy(gc.bytes(a))`, pulled
/// out here since this file needs it at several call sites.
pub(super) fn atom_str(gc: &GCLock, atom: Atom) -> String {
    String::from_utf8_lossy(gc.bytes(atom)).into_owned()
}

/// The body of `SemanticResolver::extractDeclaredIdentsFromID`
/// (SemanticResolver.cpp:2353-2405), as a free function over the only piece
/// of the resolver it touches. See the forwarding method of the same name
/// below for why it lives out here.
pub(super) fn extract_declared_idents_from_id<'gc>(
    sm: &mut SourceErrorManager,
    node: Option<&'gc Node<'gc>>,
    idents: &mut Vec<&'gc Node<'gc>>,
) -> bool {
    // The identifier is sometimes optional, in which case it is valid.
    let node = match node {
        Some(n) => n,
        None => return false,
    };

    if let Node::Identifier(_) = node {
        idents.push(node);
        return false;
    }

    if let Node::Empty(_) = node {
        return false;
    }

    if let Node::AssignmentPattern(ap) = node {
        extract_declared_idents_from_id(sm, Some(ap.left), idents);
        return true;
    }

    if let Node::ArrayPattern(arr) = node {
        let mut contains_expr = false;
        for elem in arr.elements.iter() {
            contains_expr |=
                extract_declared_idents_from_id(sm, Some(elem), idents);
        }
        return contains_expr;
    }

    if let Node::RestElement(re) = node {
        return extract_declared_idents_from_id(sm, Some(re.argument), idents);
    }

    if let Node::ObjectPattern(obj) = node {
        let mut contains_expr = false;
        for prop_node in obj.properties.iter() {
            match prop_node {
                Node::Property(p) => {
                    contains_expr |= extract_declared_idents_from_id(
                        sm,
                        Some(p.value),
                        idents,
                    );
                }
                Node::RestElement(re) => {
                    contains_expr |= extract_declared_idents_from_id(
                        sm,
                        Some(re.argument),
                        idents,
                    );
                }
                _ => panic!(
                    "cast<RestElementNode> failed: unexpected \
                     ObjectPattern property kind {}",
                    prop_node.node_type_str()
                ),
            }
        }
        return contains_expr;
    }

    if let Node::ComponentParameter(param) = node {
        return extract_declared_idents_from_id(sm, Some(param.local), idents);
    }

    sm.error_range(node.range(), "invalid destructuring target");
    false
}

impl<'bt, 'sc, 'sm, 'ad> SemanticResolver<'bt, 'sc, 'sm, 'ad> {
    // ---- Scope-shape helpers -----------------------------------------

    /// \return the function-body scope of the function owning `scope`. Port
    /// of the repeated `scope->parentFunction->getFunctionBodyScope()`
    /// idiom (cpp:354, 377, 2449).
    fn function_body_scope_of(&self, scope: ScopeId) -> ScopeId {
        let parent_function = self.sem_ctx.scope(scope).parent_function;
        self.sem_ctx
            .function(parent_function)
            .get_function_body_scope()
    }

    /// \return true if the current scope IS the current function's own
    /// body scope (as opposed to some scope nested inside it). Port of the
    /// repeated `curScope_ == curScope_->parentFunction->
    /// getFunctionBodyScope()` / `curScope_ == curFunctionInfo()->
    /// getFunctionBodyScope()` idiom (cpp:287, 353-354, 2449).
    fn cur_scope_is_function_body_scope(&self) -> bool {
        let cur = self.cur_scope.expect("no active scope");
        cur == self.function_body_scope_of(cur)
    }

    /// \p decl must be non-null (always true here: every `Decl` this port
    /// creates carries a real `scope`, so `.expect` never fires in
    /// practice — see the module doc on `Decl::scope`'s C++ nullability).
    /// \return whether the specified declaration is in the current
    /// function. Port of `declInCurFunction` (SemanticResolver.h:507-510).
    fn decl_in_cur_function(&self, decl: DeclId) -> bool {
        let scope = self
            .sem_ctx
            .decl(decl)
            .scope
            .expect("declInCurFunction requires a scoped decl");
        self.sem_ctx.scope(scope).parent_function == self.cur_function_info()
    }

    // ---- extractIdentsFromDecl / extractDeclaredIdentsFromID ----------

    /// Port of `SemanticResolver::extractIdentsFromDecl`
    /// (SemanticResolver.cpp:2262-2352). Appends every declared
    /// `Identifier` node reachable from `node`'s binding pattern(s) to
    /// `idents` and returns the `Decl::Kind` `node` as a whole should be
    /// declared with.
    ///
    /// The `ClassDeclaration`/`CatchClause`/`ImportDeclaration` arms are
    /// not corpus-reachable yet — see the module doc.
    pub(super) fn extract_idents_from_decl<'gc>(
        &mut self,
        node: &'gc Node<'gc>,
        idents: &mut Vec<&'gc Node<'gc>>,
    ) -> DeclKind {
        match node {
            Node::VariableDeclaration(vd) => {
                for decl in vd.declarations.iter() {
                    let vdecl = decl.as_variable_declarator().expect(
                        "VariableDeclaration child must be a \
                         VariableDeclarator",
                    );
                    self.extract_declared_idents_from_id(
                        Some(vdecl.id),
                        idents,
                    );
                }
                let kind_atom = vd.kind.get();
                if kind_atom == self.kw().ident_var {
                    if self.in_global_scope_context() {
                        DeclKind::GlobalProperty
                    } else {
                        DeclKind::Var
                    }
                } else if kind_atom == self.kw().ident_let {
                    DeclKind::Let
                } else {
                    DeclKind::Const
                }
            }

            Node::FunctionDeclaration(fd) => {
                self.extract_declared_idents_from_id(fd.id, idents);
                if self.cur_scope_is_function_body_scope() {
                    // It is possible to still have ScopedFunctions in the
                    // global function, for example if we have
                    // ```
                    // let foo;
                    // {
                    //   function foo() {}
                    // }
                    // ```
                    // then `foo` won't be promoted to functionScope of the
                    // global function.
                    //
                    // However, if `funcDecl` has been promoted to the
                    // functionScope of the global function, it should be
                    // declared as a GlobalProperty, just like `var` would
                    // be.
                    //
                    // See ScopedFunctionPromoter for rules on when function
                    // declarations are promoted out of the child scoped in
                    // which they are declared.
                    //
                    // If the FunctionDeclaration is not at global scope but
                    // it is a top-level declaration within a function, it's
                    // handled as Var. See ES10.0 13.2.7 for how scoped
                    // function declarations are treated specially in
                    // top-level.
                    if self.in_global_scope_context() {
                        DeclKind::GlobalProperty
                    } else {
                        DeclKind::Var
                    }
                } else {
                    DeclKind::ScopedFunction
                }
            }

            Node::ClassDeclaration(cd) => {
                self.extract_declared_idents_from_id(cd.id, idents);
                DeclKind::Class
            }

            Node::CatchClause(cc) => {
                self.extract_declared_idents_from_id(cc.param, idents);
                if matches!(cc.param, Some(Node::Identifier(_))) {
                    // For compatibility with ES5, we need to treat a single
                    // catch variable specially, see:
                    // B.3.5 VariableStatements in Catch Blocks
                    // https://www.ecma-international.org/ecma-262/10.0/index.html#sec-variablestatements-in-catch-blocks
                    DeclKind::ES5Catch
                } else {
                    DeclKind::Catch
                }
            }

            Node::ImportDeclaration(import_decl) => {
                for spec in import_decl.specifiers.iter() {
                    match spec {
                        Node::ImportSpecifier(s) => {
                            self.extract_declared_idents_from_id(
                                Some(s.local),
                                idents,
                            );
                        }
                        Node::ImportDefaultSpecifier(s) => {
                            self.extract_declared_idents_from_id(
                                Some(s.local),
                                idents,
                            );
                        }
                        Node::ImportNamespaceSpecifier(s) => {
                            self.extract_declared_idents_from_id(
                                Some(s.local),
                                idents,
                            );
                        }
                        _ => {}
                    }
                }
                DeclKind::Import
            }

            _ => {
                self.sm
                    .error_range(node.range(), "unsuppported declaration kind");
                DeclKind::Var
            }
        }
    }

    /// Port of `SemanticResolver::extractDeclaredIdentsFromID`
    /// (SemanticResolver.cpp:2353-2405). Appends every `Identifier` in the
    /// binding pattern `node` to `idents`. \return whether `node` contains
    /// an expression that isn't purely a binding pattern (an
    /// `AssignmentPattern`'s default value) — the "invalid destructuring
    /// target" error case returns `false`, matching the C++'s implicit
    /// fallthrough.
    ///
    /// The body lives in the free function of the same name below, which
    /// takes only the `&mut SourceErrorManager` this code actually needs.
    /// S3 T1's `ScopedFunctionPromoter` (`promoter.rs`) calls it while
    /// holding a shared borrow of the resolver's `DeclCollector`, which a
    /// `&mut self` receiver would forbid — see that module's doc. Same code,
    /// one place, two borrow shapes.
    pub(super) fn extract_declared_idents_from_id<'gc>(
        &mut self,
        node: Option<&'gc Node<'gc>>,
        idents: &mut Vec<&'gc Node<'gc>>,
    ) -> bool {
        extract_declared_idents_from_id(self.sm, node, idents)
    }

    // ---- processCollectedDeclarations / processDeclarations -----------

    /// Port of `SemanticResolver::processDeclarations`
    /// (SemanticResolver.cpp:2095-2127).
    ///
    /// Takes an owned slice rather than borrowing straight from the
    /// `DeclCollector` (whose `ScopeDecls` lives behind
    /// `self.function_context()`): every declaration below needs `&mut
    /// self`, which a live borrow of `self.function_context()` would
    /// forbid. See `process_collected_declarations` (`mod.rs`), the sole
    /// caller, which clones the `NodeRc`s (cheap refcount bumps) up front
    /// for exactly this reason.
    pub(super) fn process_declarations(
        &mut self,
        gc: &GCLock,
        decls: &[NodeRc],
    ) {
        for decl_rc in decls {
            let decl_node = decl_rc.node(gc);

            // TypeAlias/TSTypeAliasDeclaration are type-only declarations;
            // they don't participate in value binding. Port of the `#if
            // HERMES_PARSE_FLOW`/`#if HERMES_PARSE_TS`-guarded `continue`s
            // (cpp:2097-2104) — this port's single node set always
            // includes both dialects (see the crate doc), so both checks
            // apply unconditionally.
            if matches!(
                decl_node,
                Node::TypeAlias(_) | Node::TSTypeAliasDeclaration(_)
            ) {
                continue;
            }

            let mut idents: Vec<&Node> = Vec::new();
            let kind = self.extract_idents_from_decl(decl_node, &mut idents);

            // In typed mode, ignore function declarations marked as
            // builtin (either via the "builtin" directive or a
            // Hermes.builtin decoration). They're going to be resolved by
            // the FlowChecker. `TYPED` is always `false` in this port —
            // see the module doc for why this is stubbed rather than
            // transcribed.
            if TYPED {
                unreachable!(
                    "typed-mode builtin-function skip is S2 scope \
                     (cpp:2108-2121)"
                );
            }

            for ident in idents {
                self.validate_and_declare_identifier(gc, kind, ident);
            }
        }
    }

    // ---- validateAndDeclareIdentifier ----------------------------------

    /// Port of `SemanticResolver::validateAndDeclareIdentifier`
    /// (SemanticResolver.cpp:2407-2639).
    pub(super) fn validate_and_declare_identifier<'gc>(
        &mut self,
        gc: &'gc GCLock,
        kind: DeclKind,
        ident_node: &'gc Node<'gc>,
    ) {
        let identifier = ident_node
            .as_identifier()
            .expect("validate_and_declare_identifier: not an Identifier");

        if !self.validate_declaration_name(gc, kind, ident_node) {
            return;
        }

        let mut prev_name: Option<Binding> =
            self.binding_table.lookup(&identifier.name.get());

        // IMPORTANT: this is not spec compliant!
        // For now, treat "var" declarations of "arguments" simply as a new
        // variable instead of as an alias for the Arguments object. It is
        // simpler and makes a difference only in the following obscure
        // case:
        // - non-strict mode
        // - "var arguments" without an initializer.
        // I am willing to live with this sacrifice.
        // Aliasing of "arguments" becomes especially iffy when type
        // annotations are added.
        #[allow(clippy::overly_complex_bool_expr, clippy::needless_bool)]
        if false {
            // Redeclaration of `arguments` in non-strict mode is allowed at
            // the function level, so we don't need to declare a new
            // variable.
            if !self.sem_ctx.function(self.cur_function_info()).strict
                && identifier.name.get() == self.kw().ident_arguments
                && kind == DeclKind::Var
            {
                return;
            }
        }

        // Ignore declarations in enclosing functions.
        if let Some(pn) = &prev_name {
            if !self.decl_in_cur_function(pn.decl) {
                prev_name = None;
            }
        }

        let mut decl: Option<DeclId> = None;

        // Whether to reuse the decl (above) for a new binding when it's not
        // `None`.
        let mut reuse_decl_for_new_binding = false;

        // Handle re-declarations, ignoring ambient properties.
        if let Some(pn) = &prev_name {
            if self.sem_ctx.decl(pn.decl).kind
                != DeclKind::UndeclaredGlobalProperty
            {
                let prev_kind = self.sem_ctx.decl(pn.decl).kind;
                let cur_scope = self.cur_scope.expect("no active scope");
                let same_scope =
                    self.sem_ctx.decl(pn.decl).scope == Some(cur_scope);
                let top_level = self.cur_scope_is_function_body_scope();
                let prev_in_prev_scope = self.sem_ctx.decl(pn.decl).scope
                    == self.sem_ctx.scope(cur_scope).parent_scope;

                // Check whether the redeclaration is invalid.
                // Note that since "var" declarations have been hoisted to
                // the function scope, we cannot catch cases where "var"
                // follows something declared in a surrounding lexical
                // scope. See visit(VariableDeclarationNode *) for when
                // those are handled.
                //
                // The two rules in the spec ES10.0 (e.g. B.3.3.4) are:
                // * LexicallyDeclaredNames (in the same scope) can't
                //   conflict.
                // * LexicallyDeclaredNames can't conflict with
                //   VarDeclarationNames in their own scope or any of their
                //   child scopes (recursively).
                //
                // Parameter names must also not conflict with lexically
                // scoped names in the top-level of the function
                // (ES10.0 14.1.2):
                // * It is a Syntax Error if any element of the BoundNames
                //   of FormalParameters also occurs in the
                //   LexicallyDeclaredNames of FunctionBody.
                //
                // Catch (non-ES5) clause variables must not conflict with
                // the lexically scoped names or var-declared names in
                // their block:
                // * It is a Syntax Error if BoundNames of CatchParameter
                //   contains any duplicate elements.
                // * It is a Syntax Error if any element of the BoundNames
                //   of CatchParameter also occurs in the
                //   LexicallyDeclaredNames of Block.
                //   NOTE: It's possible that a function in the body of the
                //   catch has been promoted to a Var at function scope, so
                //   it has to be accounted for.
                // * It is a Syntax Error if any element of the BoundNames
                //   of CatchParameter also occurs in the VarDeclaredNames
                //   of Block unless CatchParameter is CatchParameter :
                //   BindingIdentifier.
                //   visit(VariableDeclarationNode *) will handle this final
                //   case.
                //
                // Case by case explanations for our representation:
                //
                // ES5Catch, var
                //          -> valid, special case ES10 B.3.5, but we can't
                //             catch it here. See
                //             visit(VariableDeclarationNode *)
                // var, var
                //          -> always valid
                // scopedFunction, var
                //          -> can't happen because var is at top-level only
                // var, scopedFunction
                //          -> valid because scopedFunction is not at
                //             top-level
                // scopedFunction, scopedFunction
                //          -> strict mode: valid if not in the same scope
                //             loose mode: always valid
                //             See ES10.0 13.2.7
                //             scoped function declarations are treated
                //             specially if they're at the top-level of the
                //             function/script/module.
                //             'var' case is handled in
                //             visit(VariableDeclarationNode *).
                // let, var
                //          -> always invalid
                // let, scopedFunction
                //          -> invalid if same scope
                // var|scopedFunction|let, let
                //          -> invalid if the same scope
                // parameter, let
                //          -> invalid if let is top-level

                assert!(
                    !(prev_kind == DeclKind::ScopedFunction
                        && kind == DeclKind::Var),
                    "invalid state, scopedFunctions are not at top-level"
                );

                if (prev_kind.is_let_like() && kind.is_var_like())
                    || (prev_kind.is_var_like()
                        && kind.is_let_like()
                        && same_scope)
                    || (prev_kind.is_let_like()
                        && kind.is_let_like()
                        && same_scope
                        // ES10.0 B.3.3.4
                        // Annex B exception: non-strict mode ScopedFunctions
                        // are OK.
                        && !(!self
                            .sem_ctx
                            .function(self.cur_function_info())
                            .strict
                            && prev_kind == DeclKind::ScopedFunction
                            && kind == DeclKind::ScopedFunction))
                    || (prev_kind == DeclKind::Parameter
                        && kind.is_let_like()
                        && top_level)
                    // LexicallyDeclaredNames of CatchBlock are only in the
                    // block scope itself, so check prevInPrevScope (it's
                    // like checking topLevel for parameters).
                    // This is an error regardless of if it's an ES5 or ES6
                    // catch.
                    || ((prev_kind == DeclKind::Catch
                        || prev_kind == DeclKind::ES5Catch)
                        && kind.is_let_like()
                        && prev_in_prev_scope)
                {
                    self.sm.error_range(
                        ident_node.range(),
                        format!(
                            "Identifier '{}' is already declared",
                            atom_str(gc, identifier.name.get())
                        ),
                    );
                    if let Some(prev_ident) = &pn.ident {
                        self.sm.note_range(
                            prev_ident.node(gc).range(),
                            "previous declaration",
                            Subsystem::Unspecified,
                        );
                    }
                    return;
                }

                // When to create a new declaration?
                //
                // Var, Var -> use prev
                if prev_kind.is_var_like() && kind.is_var_like() {
                    decl = Some(pn.decl);
                }
                // Var, ScopedFunc -> if non-param non-strict or same scope,
                //                    then use prev, else declare new
                else if prev_kind.is_var_like()
                    && kind == DeclKind::ScopedFunction
                {
                    decl = None;
                    if same_scope {
                        decl = Some(pn.decl);
                    } else if let Some(&d) = self
                        .function_context()
                        .promoted_func_decls
                        .get(&identifier.name.get())
                    {
                        // We've already promoted this function, so add a
                        // new binding and point it to the original Decl.
                        reuse_decl_for_new_binding = true;
                        decl = Some(d);
                    }
                }
                // ES5Catch, ScopedFunc ->
                //   if promoted, use promoted function, else declare new
                //   ES5Catch doesn't prevent promotion, so we have to check
                //   it specially.
                else if prev_kind == DeclKind::ES5Catch
                    && kind == DeclKind::ScopedFunction
                {
                    if let Some(&d) = self
                        .function_context()
                        .promoted_func_decls
                        .get(&identifier.name.get())
                    {
                        reuse_decl_for_new_binding = true;
                        decl = Some(d);
                    } else {
                        decl = None;
                    }
                }
                // ScopedFunc, ScopedFunc same scope -> error
                // ScopedFunc, ScopedFunc new scope -> declare new
                else if prev_kind == DeclKind::ScopedFunction
                    && kind == DeclKind::ScopedFunction
                {
                    decl = None;
                }
            }
        }

        // Special case: this is a lexically-scoped declaration in global
        // scope which is a restricted global.
        // ES14.0 16.1.7 GlobalDeclarationInstantiation
        // For each element name of lexNames, do
        //  a. If env.HasVarDeclaration(name) is true,
        //    throw a SyntaxError exception.
        //  b. If env.HasLexicalDeclaration(name) is true,
        //    throw a SyntaxError exception.
        //  c. Let hasRestrictedGlobal be ?
        //    env.HasRestrictedGlobalProperty(name).
        //  d. If hasRestrictedGlobal is true,
        //    throw a SyntaxError exception.
        //  (a-b) are handled by the checks above, so just do (c-d) here.
        if self.cur_scope == Some(self.sem_ctx.get_global_scope())
            && kind.is_let_like()
            && self.is_restricted_global_property(identifier.name.get())
        {
            self.sm.error_range(
                ident_node.range(),
                format!(
                    "Can't create duplicate variable that shadows a global \
                     property: '{}'",
                    atom_str(gc, identifier.name.get())
                ),
            );
        }

        // A promoted function involves two declarations: one for the
        // global scope and one for the block scope. This statement handles
        // the scenario where an identifier already has an associated
        // declaration and focuses on creating the promoted declaration
        // instead.
        //  1. A block-scoped declaration is created and linked with the
        //     identifier.
        //  2. The binding table is updated to associate the identifier
        //     name with the correct declaration. It is necessary to use
        //     `put` instead of `try_emplace` as there could be multiple
        //     identifiers with the same name, requiring replacement of the
        //     previous binding.
        if self.sem_ctx.get_declaration_decl(identifier).is_some()
            && self
                .function_context()
                .promoted_func_decls
                .contains_key(&identifier.name.get())
        {
            let cur_scope = self.cur_scope.expect("no active scope");
            let new_decl = self.sem_ctx.new_decl_in_scope_default(
                identifier.name.get(),
                kind,
                cur_scope,
            );
            self.binding_table.put(
                identifier.name.get(),
                Binding::new(new_decl, Some(NodeRc::from_node(gc, ident_node))),
            );
            self.sem_ctx
                .set_promoted_decl(ident_node.node_id(), new_decl);
            return;
        }

        // Create new decl.
        if let Some(d) = decl {
            if reuse_decl_for_new_binding {
                self.binding_table.try_emplace(
                    identifier.name.get(),
                    Binding::new(d, Some(NodeRc::from_node(gc, ident_node))),
                );
            }
        } else {
            let new_decl = if kind.is_global() {
                self.sem_ctx.new_global(identifier.name.get(), kind)
            } else {
                let cur_scope = self.cur_scope.expect("no active scope");
                self.sem_ctx.new_decl_in_scope_default(
                    identifier.name.get(),
                    kind,
                    cur_scope,
                )
            };
            self.binding_table.try_emplace(
                identifier.name.get(),
                Binding::new(new_decl, Some(NodeRc::from_node(gc, ident_node))),
            );
            decl = Some(new_decl);
        }

        self.sem_ctx.set_declaration_decl(
            ident_node.node_id(),
            identifier,
            decl,
        );
    }

    /// Port of `SemanticResolver::validateDeclarationName`
    /// (SemanticResolver.cpp:2641-2677).
    pub(super) fn validate_declaration_name(
        &mut self,
        gc: &GCLock,
        decl_kind: DeclKind,
        id_node: &Node,
    ) -> bool {
        let identifier = id_node
            .as_identifier()
            .expect("validate_declaration_name: not an Identifier");

        if self.sem_ctx.function(self.cur_function_info()).strict {
            // - 'arguments' cannot be redeclared in strict mode.
            // - 'eval' cannot be redeclared in strict mode.
            if identifier.name.get() == self.kw().ident_arguments
                || identifier.name.get() == self.kw().ident_eval
            {
                self.sm.error_range(
                    id_node.range(),
                    format!(
                        "cannot declare '{}' in strict mode",
                        atom_str(gc, identifier.name.get())
                    ),
                );
                return false;
            }

            // Parameter cannot be named "let".
            if decl_kind == DeclKind::Parameter
                && identifier.name.get() == self.kw().ident_let
            {
                self.sm.error_range(
                    id_node.range(),
                    "invalid parameter name 'let' in strict mode",
                );
                return false;
            }
        }

        if (decl_kind == DeclKind::Let || decl_kind == DeclKind::Const)
            && identifier.name.get() == self.kw().ident_let
        {
            // ES9.0 13.3.1.1
            // LexicalDeclaration : LetOrConst BindingList
            // It is a Syntax Error if the BoundNames of BindingList
            // contains "let".
            self.sm.error_range(
                id_node.range(),
                "'let' is disallowed as a lexically bound name",
            );
            return false;
        }

        true
    }

    // ---- visit(VariableDeclarationNode *) ------------------------------

    /// Port of `SemanticResolver::visit(ESTree::VariableDeclarationNode
    /// *node)` (SemanticResolver.cpp:325-403).
    pub(super) fn visit_variable_declaration<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
        let vd = match node {
            Node::VariableDeclaration(vd) => vd,
            _ => unreachable!(
                "visit_variable_declaration: not a VariableDeclaration"
            ),
        };

        if self.compile()
            && (vd.kind.get() == self.kw().ident_using
                || vd.kind.get() == self.kw().ident_await_using)
        {
            // 'using' declarations are not supported in compiled code.
            self.sm.error_range(
                node.range(),
                "using declarations are not yet supported",
            );
            return TransformResult::Unchanged;
        }

        let result = node.visit_children_mut(gc, self);

        // ES5Catch, var
        //          -> valid, special case ES10 B.3.5
        // let, var
        //          -> always invalid
        // Ordinarily, we check this in validateAndDeclareIdentifier, but if
        // the declarations are in a nested scope like x or y here:
        //
        // function f() {
        //   { let x; var x; }
        //   { let y; { var y; } }
        // }
        //
        // then the var has been hoisted to the function-level scope by
        // DeclCollector and we aren't able to detect that both
        // declarations are actually in the same scope and conflict.
        // Only perform this check for nested scopes, because the var will
        // have been hoisted into a different scope.
        if vd.kind.get() == self.kw().ident_var
            && !self.cur_scope_is_function_body_scope()
        {
            let mut idents: Vec<&Node> = Vec::new();
            self.extract_idents_from_decl(node, &mut idents);
            // Check every identifier declared as a 'var'.
            for ident_node in idents {
                let identifier = ident_node.as_identifier().expect(
                    "extract_idents_from_decl only ever pushes Identifiers",
                );
                let name = identifier.name.get();
                let Some((prev_binding, prev_depth)) =
                    self.binding_table.find_with_depth(&name)
                else {
                    // No existing declaration, move on.
                    continue;
                };

                // Whether the prevName is the lexical binding for a
                // promoted function which reuses the same Decl.
                // If it is a lexical binding of a promoted function,
                // that's an error due to a lexically-scoped and
                // var-scoped naming conflict.
                let prev_is_lexical_binding_of_promoted_func = self
                    .function_context()
                    .promoted_func_decls
                    .contains_key(&name)
                    && prev_depth
                        != self.function_context().binding_table_scope_depth;

                let prev_scope = self
                    .sem_ctx
                    .decl(prev_binding.decl)
                    .scope
                    .expect("decl must be scoped");

                if prev_scope == self.function_body_scope_of(prev_scope)
                    && !prev_is_lexical_binding_of_promoted_func
                {
                    // If the previous declaration is in the function
                    // scope, the error would have been reported when
                    // validating declarations in the function scope.
                    continue;
                }

                // Report an error if the var is trying to override a
                // let-like declaration.
                //
                // ES10.0 B.3.4: ES5Catch (only used for simple binding
                // ident in catch block) is not an error if it conflicts
                // with VarDeclaredNames in its body.
                let prev_kind = self.sem_ctx.decl(prev_binding.decl).kind;
                if (prev_kind.is_let_like() && prev_kind != DeclKind::ES5Catch)
                    || prev_is_lexical_binding_of_promoted_func
                {
                    self.sm.error_range(
                        ident_node.range(),
                        format!(
                            "Identifier '{}' is already declared",
                            atom_str(gc, name)
                        ),
                    );
                    if let Some(prev_ident) = &prev_binding.ident {
                        self.sm.note_range(
                            prev_ident.node(gc).range(),
                            "previous declaration",
                            Subsystem::Unspecified,
                        );
                    }
                }
            }
        }

        result
    }

    // ---- visit(BlockStatementNode *, Node *) ---------------------------

    /// Port of `SemanticResolver::visit(ESTree::BlockStatementNode *node,
    /// ESTree::Node *parent)` (SemanticResolver.cpp:502-518).
    pub(super) fn visit_block_statement<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
        path: Option<ast::visitor::Path<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>> {
        // Some nodes with attached BlockStatement have already dealt with
        // the scope.
        if let Some(p) = path {
            if matches!(
                p.parent,
                Node::FunctionDeclaration(_)
                    | Node::FunctionExpression(_)
                    | Node::ArrowFunctionExpression(_)
            ) {
                return node.visit_children_mut(gc, self);
            }
        }

        let scope_state = self.enter_scope(Some(node), false);
        self.process_collected_declarations(gc, node);
        let result = node.visit_children_mut(gc, self);
        self.exit_scope(scope_state);
        result
    }
}

#[cfg(test)]
mod tests {
    use ast::context::Context;
    use ast::node::{
        Identifier, NumericLiteral, VariableDeclaration, VariableDeclarator,
    };
    use ast::node_child::NodeList;
    use ast::node_child::NodeMetadata;
    use parser::js::JSParserImpl;
    use parser::lexer::{GrammarContext, JSLexer};
    use support::location::{SMLoc, SMRange};
    use support::manager::SourceErrorManager;
    use support::persistent_scoped_map::Scope;

    use super::*;
    use crate::keywords::Keywords;
    use crate::resolver::FunctionContext;
    use crate::sem_context::{
        ConstructorKind, CustomDirectives, FuncIsArrow, SemContext,
    };

    /// Parse `src` as a `Program` and return its root node, panicking on any
    /// parse error. Mirrors `tests/resolver.rs`'s `parse` helper.
    fn parse<'gc>(
        gc: &'gc GCLock,
        sm: &mut SourceErrorManager,
        src: &str,
    ) -> &'gc Node<'gc> {
        let buf_id = sm.add_buffer_bytes("input", src.as_bytes());
        let result: Option<&Node> = {
            let atoms = &gc.ctx().atom_table;
            let lexer =
                JSLexer::new(buf_id, sm, atoms, GrammarContext::AllowRegExp);
            let mut parser = JSParserImpl::new(gc, lexer);
            parser.parse()
        };
        assert_eq!(sm.error_count(), 0, "unexpected parse errors in: {src}");
        result.expect("parser returned no Program")
    }

    /// \return the first top-level statement of a parsed `Program`.
    fn first_statement<'gc>(program_node: &'gc Node<'gc>) -> &'gc Node<'gc> {
        match program_node {
            Node::Program(p) => p.body.iter().next().expect("empty program"),
            _ => unreachable!("first_statement: not a Program"),
        }
    }

    /// Allocate an `Identifier` node named `name` at `loc`. Same shape as
    /// `identifiers.rs`'s private helper of the same name (not reusable
    /// across the sibling test modules, so duplicated here).
    fn alloc_identifier<'gc>(
        gc: &'gc GCLock,
        name: &str,
        loc: SMLoc,
    ) -> &'gc Node<'gc> {
        let atom = gc.atom_bytes(name);
        gc.alloc(Node::Identifier(Identifier::new(
            NodeMetadata::new(SMRange {
                start: loc,
                end: loc,
            }),
            atom,
            None,
            false,
        )))
    }

    /// Allocate a `var <name>;` `VariableDeclaration` node whose single
    /// declarator's `id` is `ident_node` (no initializer).
    fn alloc_var_decl<'gc>(
        gc: &'gc GCLock,
        kw_var: Atom,
        ident_node: &'gc Node<'gc>,
        range: SMRange,
    ) -> &'gc Node<'gc> {
        let declarator = gc.alloc(Node::VariableDeclarator(
            VariableDeclarator::new(NodeMetadata::new(range), None, ident_node),
        ));
        gc.alloc(Node::VariableDeclaration(VariableDeclaration::new(
            NodeMetadata::new(range),
            kw_var,
            NodeList::from_iter(gc, [declarator]),
        )))
    }

    // ==== extractIdentsFromDecl classification (cpp:2262-2352) =========
    //
    // `FunctionDeclaration`/`ClassDeclaration`/`CatchClause`/
    // `ImportDeclaration` are not corpus-reachable yet (see the module
    // doc) — exercised here by calling `extract_idents_from_decl` directly
    // on a hand-parsed node, bypassing `visit_node`'s dispatch entirely.

    /// A `FunctionDeclaration` at the top level of the (installed-as-global)
    /// function is a `GlobalProperty`, exactly like `var` would be.
    #[test]
    fn function_declaration_at_global_top_level_is_global_property() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let root = parse(&gc, &mut sm, "function f() {}\n");
        let func_decl = first_statement(root);

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let binding_table = sem_ctx.binding_table_rc();
        let mut resolver = SemanticResolver::new(
            &binding_table,
            &mut sem_ctx,
            &mut sm,
            &[],
            true,
        );
        // `root` (a `Program`) stands in for the function-like node
        // `enter_function` decorates — see `identifiers.rs`'s
        // `resolve_identifier_typeof_creates_ambient_global_without_warning`
        // for the same placeholder trick.
        let func_state = resolver.enter_function(
            &gc,
            root,
            None,
            false,
            ConstructorKind::None,
            CustomDirectives::default(),
            /* install_as_global_context */ true,
        );
        let scope_state =
            resolver.enter_scope(None, /* functionScope */ true);

        let mut idents: Vec<&Node> = Vec::new();
        let kind = resolver.extract_idents_from_decl(func_decl, &mut idents);
        assert_eq!(kind, DeclKind::GlobalProperty);
        assert_eq!(idents.len(), 1);
        assert!(matches!(idents[0], Node::Identifier(_)));

        resolver.exit_scope(scope_state);
        resolver.exit_function(func_state);
    }

    /// The same top-level shape, but the enclosing function is NOT the
    /// global context: a `FunctionDeclaration` there is a `Var`.
    #[test]
    fn function_declaration_at_non_global_top_level_is_var() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let root = parse(&gc, &mut sm, "function f() {}\n");
        let func_decl = first_statement(root);

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let binding_table = sem_ctx.binding_table_rc();
        let mut resolver = SemanticResolver::new(
            &binding_table,
            &mut sem_ctx,
            &mut sm,
            &[],
            true,
        );
        let func_state = resolver.enter_function(
            &gc,
            root,
            None,
            false,
            ConstructorKind::None,
            CustomDirectives::default(),
            /* install_as_global_context */ false,
        );
        let scope_state = resolver.enter_scope(None, true);

        let mut idents: Vec<&Node> = Vec::new();
        let kind = resolver.extract_idents_from_decl(func_decl, &mut idents);
        assert_eq!(kind, DeclKind::Var);

        resolver.exit_scope(scope_state);
        resolver.exit_function(func_state);
    }

    /// A `FunctionDeclaration` NOT at the top level of its function (i.e.
    /// nested one scope deeper) is a `ScopedFunction`, regardless of
    /// whether the function is the global one.
    #[test]
    fn function_declaration_in_nested_scope_is_scoped_function() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let root = parse(&gc, &mut sm, "function f() {}\n");
        let func_decl = first_statement(root);

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let binding_table = sem_ctx.binding_table_rc();
        let mut resolver = SemanticResolver::new(
            &binding_table,
            &mut sem_ctx,
            &mut sm,
            &[],
            true,
        );
        let func_state = resolver.enter_function(
            &gc,
            root,
            None,
            false,
            ConstructorKind::None,
            CustomDirectives::default(),
            true,
        );
        let body_scope_state = resolver.enter_scope(None, true);
        let block_scope_state = resolver.enter_scope(None, false);

        let mut idents: Vec<&Node> = Vec::new();
        let kind = resolver.extract_idents_from_decl(func_decl, &mut idents);
        assert_eq!(kind, DeclKind::ScopedFunction);

        resolver.exit_scope(block_scope_state);
        resolver.exit_scope(body_scope_state);
        resolver.exit_function(func_state);
    }

    /// `ClassDeclaration` classification doesn't consult scope at all, so
    /// no function/scope setup is needed — a bare resolver suffices.
    #[test]
    fn class_declaration_is_class() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let root = parse(&gc, &mut sm, "class C {}\n");
        let class_decl = first_statement(root);

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let binding_table = sem_ctx.binding_table_rc();
        let mut resolver = SemanticResolver::new(
            &binding_table,
            &mut sem_ctx,
            &mut sm,
            &[],
            true,
        );

        let mut idents: Vec<&Node> = Vec::new();
        let kind = resolver.extract_idents_from_decl(class_decl, &mut idents);
        assert_eq!(kind, DeclKind::Class);
        assert_eq!(idents.len(), 1);
    }

    /// A single-`Identifier` catch parameter (`catch (e)`) is the special
    /// `ES5Catch` kind (ES10 B.3.5).
    #[test]
    fn catch_with_identifier_param_is_es5catch() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let root = parse(&gc, &mut sm, "try {} catch (e) {}\n");
        let try_stmt = first_statement(root);
        let handler = match try_stmt {
            Node::TryStatement(t) => t.handler.expect("try has a handler"),
            _ => unreachable!(),
        };

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let binding_table = sem_ctx.binding_table_rc();
        let mut resolver = SemanticResolver::new(
            &binding_table,
            &mut sem_ctx,
            &mut sm,
            &[],
            true,
        );

        let mut idents: Vec<&Node> = Vec::new();
        let kind = resolver.extract_idents_from_decl(handler, &mut idents);
        assert_eq!(kind, DeclKind::ES5Catch);
        assert_eq!(idents.len(), 1);
    }

    /// A destructuring catch parameter (`catch ({a, b})`) is the plain
    /// `Catch` kind — the ES5Catch special-case only applies to a bare
    /// identifier.
    #[test]
    fn catch_with_destructuring_param_is_catch() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let root = parse(&gc, &mut sm, "try {} catch ({a, b}) {}\n");
        let try_stmt = first_statement(root);
        let handler = match try_stmt {
            Node::TryStatement(t) => t.handler.expect("try has a handler"),
            _ => unreachable!(),
        };

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let binding_table = sem_ctx.binding_table_rc();
        let mut resolver = SemanticResolver::new(
            &binding_table,
            &mut sem_ctx,
            &mut sm,
            &[],
            true,
        );

        let mut idents: Vec<&Node> = Vec::new();
        let kind = resolver.extract_idents_from_decl(handler, &mut idents);
        assert_eq!(kind, DeclKind::Catch);
        assert_eq!(idents.len(), 2);
    }

    /// A parameterless catch (`catch {}`, ES2019 optional catch binding) is
    /// also plain `Catch` — `cc.param` is `None`, so the `dyn_cast_or_null`
    /// in the C++ is `nullptr`, which is NOT an `IdentifierNode`.
    #[test]
    fn catch_with_no_param_is_catch() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let root = parse(&gc, &mut sm, "try {} catch {}\n");
        let try_stmt = first_statement(root);
        let handler = match try_stmt {
            Node::TryStatement(t) => t.handler.expect("try has a handler"),
            _ => unreachable!(),
        };

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let binding_table = sem_ctx.binding_table_rc();
        let mut resolver = SemanticResolver::new(
            &binding_table,
            &mut sem_ctx,
            &mut sm,
            &[],
            true,
        );

        let mut idents: Vec<&Node> = Vec::new();
        let kind = resolver.extract_idents_from_decl(handler, &mut idents);
        assert_eq!(kind, DeclKind::Catch);
        assert_eq!(idents.len(), 0);
    }

    /// `ImportDeclaration` collects the `local` identifier of every
    /// specifier kind (`ImportSpecifier`/`ImportDefaultSpecifier`/
    /// `ImportNamespaceSpecifier`) and is always the `Import` kind.
    /// (Module-mode validation — `visit(ImportDeclarationNode *)`,
    /// cpp:874-891 — is a separate, unported code path; calling
    /// `extract_idents_from_decl` directly bypasses it entirely, same as
    /// the other dormant classifications above.)
    #[test]
    fn import_declaration_collects_default_and_named_locals() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let root = parse(&gc, &mut sm, "import def, {a, b as c} from \"m\";\n");
        let import_decl = first_statement(root);

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let binding_table = sem_ctx.binding_table_rc();
        let mut resolver = SemanticResolver::new(
            &binding_table,
            &mut sem_ctx,
            &mut sm,
            &[],
            true,
        );

        let mut idents: Vec<&Node> = Vec::new();
        let kind = resolver.extract_idents_from_decl(import_decl, &mut idents);
        assert_eq!(kind, DeclKind::Import);
        let names: Vec<String> = idents
            .iter()
            .map(|n| {
                let id = n.as_identifier().unwrap();
                String::from_utf8_lossy(gc.bytes(id.name.get())).into_owned()
            })
            .collect();
        // `def` (the default specifier's local), `a` (the shorthand
        // specifier's local — `import {a}` means imported name "a", local
        // name "a", a full `ImportSpecifier` in its own right), then `c`
        // (`b as c`'s local, NOT the imported name `b`).
        assert_eq!(names, vec!["def", "a", "c"]);
    }

    /// A namespace import (`import * as ns from "m"`) collects `ns`.
    #[test]
    fn import_declaration_collects_namespace_local() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let root = parse(&gc, &mut sm, "import * as ns from \"m\";\n");
        let import_decl = first_statement(root);

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let binding_table = sem_ctx.binding_table_rc();
        let mut resolver = SemanticResolver::new(
            &binding_table,
            &mut sem_ctx,
            &mut sm,
            &[],
            true,
        );

        let mut idents: Vec<&Node> = Vec::new();
        let kind = resolver.extract_idents_from_decl(import_decl, &mut idents);
        assert_eq!(kind, DeclKind::Import);
        assert_eq!(idents.len(), 1);
        let id = idents[0].as_identifier().unwrap();
        assert_eq!(String::from_utf8_lossy(gc.bytes(id.name.get())), "ns");
    }

    /// A node that isn't one of the five recognized declaration kinds
    /// reports "unsuppported declaration kind" (verbatim C++ typo,
    /// cpp:2349) and returns the dummy `Decl::Kind::Var`.
    #[test]
    fn unsupported_declaration_kind_reports_error() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let buf = sm.add_buffer_bytes("d.js", b"1");
        let loc = SMLoc {
            source: buf,
            offset: 0,
        };
        let range = SMRange {
            start: loc,
            end: loc,
        };
        let node = gc.alloc(Node::NumericLiteral(NumericLiteral::new(
            NodeMetadata::new(range),
            1.0,
        )));

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let binding_table = sem_ctx.binding_table_rc();
        {
            let mut resolver = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                true,
            );

            let mut idents: Vec<&Node> = Vec::new();
            let kind = resolver.extract_idents_from_decl(node, &mut idents);
            assert_eq!(kind, DeclKind::Var);
            assert!(idents.is_empty());
        }
        assert_eq!(sm.error_count(), 1);
    }

    // ==== extractDeclaredIdentsFromID (cpp:2353-2405) ===================

    /// A pattern-position node that isn't `Identifier`/`Empty`/
    /// `AssignmentPattern`/`ArrayPattern`/`RestElement`/`ObjectPattern`/
    /// `ComponentParameter` reports "invalid destructuring target"
    /// (cpp:2403) and returns `false` (no `containsExpr`).
    #[test]
    fn invalid_destructuring_target_reports_error() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let buf = sm.add_buffer_bytes("d.js", b"1");
        let loc = SMLoc {
            source: buf,
            offset: 0,
        };
        let range = SMRange {
            start: loc,
            end: loc,
        };
        let node = gc.alloc(Node::NumericLiteral(NumericLiteral::new(
            NodeMetadata::new(range),
            1.0,
        )));

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let binding_table = sem_ctx.binding_table_rc();
        {
            let mut resolver = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                true,
            );

            let mut idents: Vec<&Node> = Vec::new();
            let contains_expr = resolver
                .extract_declared_idents_from_id(Some(node), &mut idents);
            assert!(!contains_expr);
            assert!(idents.is_empty());
        }
        assert_eq!(sm.error_count(), 1);
    }

    // ==== validateDeclarationName (cpp:2641-2677) =======================

    /// Strict mode: `arguments`/`eval` can never be declared, regardless of
    /// `Decl::Kind`.
    #[test]
    fn validate_declaration_name_rejects_strict_arguments_and_eval() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let buf = sm.add_buffer_bytes("d.js", b"arguments eval");
        let loc_a = SMLoc {
            source: buf,
            offset: 0,
        };
        let loc_e = SMLoc {
            source: buf,
            offset: 10,
        };
        let arguments_node = alloc_identifier(&gc, "arguments", loc_a);
        let eval_node = alloc_identifier(&gc, "eval", loc_e);

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let func = sem_ctx.new_function(
            FuncIsArrow::No,
            ConstructorKind::None,
            None,
            None,
            /* strict */ true,
            CustomDirectives::default(),
        );
        let binding_table = sem_ctx.binding_table_rc();
        {
            let mut resolver = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                true,
            );
            resolver.function_stack.push(FunctionContext {
                sem_info: func,
                node: None,
                label_map: Default::default(),
                current_loop: None,
                current_loop_or_switch: None,
                is_formal_params: false,
                decls: None,
                promoted_func_decls: Default::default(),
                binding_table_scope_depth: 0,
            });

            assert!(!resolver.validate_declaration_name(
                &gc,
                DeclKind::Var,
                arguments_node
            ));
            assert!(!resolver.validate_declaration_name(
                &gc,
                DeclKind::Let,
                eval_node
            ));
            // `resolver` (and its `&mut sm` borrow) must drop before `sm` can
            // be read again — see `identifiers.rs`'s tests for the same
            // pattern.
        }
        assert_eq!(sm.error_count(), 2);
    }

    /// Strict mode: a `Parameter` literally named `let` is rejected.
    #[test]
    fn validate_declaration_name_rejects_strict_parameter_named_let() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let buf = sm.add_buffer_bytes("d.js", b"let");
        let loc = SMLoc {
            source: buf,
            offset: 0,
        };
        let let_node = alloc_identifier(&gc, "let", loc);

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let func = sem_ctx.new_function(
            FuncIsArrow::No,
            ConstructorKind::None,
            None,
            None,
            true,
            CustomDirectives::default(),
        );
        let binding_table = sem_ctx.binding_table_rc();
        {
            let mut resolver = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                true,
            );
            resolver.function_stack.push(FunctionContext {
                sem_info: func,
                node: None,
                label_map: Default::default(),
                current_loop: None,
                current_loop_or_switch: None,
                is_formal_params: false,
                decls: None,
                promoted_func_decls: Default::default(),
                binding_table_scope_depth: 0,
            });

            assert!(!resolver.validate_declaration_name(
                &gc,
                DeclKind::Parameter,
                let_node
            ));
        }
        assert_eq!(sm.error_count(), 1);
    }

    /// `let`/`const` can never bind the name `let`, in strict OR loose
    /// mode (ES9.0 13.3.1.1) — unlike the two checks above, which are
    /// strict-only.
    #[test]
    fn validate_declaration_name_rejects_let_named_let_in_loose_mode() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let buf = sm.add_buffer_bytes("d.js", b"let");
        let loc = SMLoc {
            source: buf,
            offset: 0,
        };
        let let_node = alloc_identifier(&gc, "let", loc);

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let func = sem_ctx.new_function(
            FuncIsArrow::No,
            ConstructorKind::None,
            None,
            None,
            /* strict */ false,
            CustomDirectives::default(),
        );
        let binding_table = sem_ctx.binding_table_rc();
        {
            let mut resolver = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                true,
            );
            resolver.function_stack.push(FunctionContext {
                sem_info: func,
                node: None,
                label_map: Default::default(),
                current_loop: None,
                current_loop_or_switch: None,
                is_formal_params: false,
                decls: None,
                promoted_func_decls: Default::default(),
                binding_table_scope_depth: 0,
            });

            assert!(!resolver.validate_declaration_name(
                &gc,
                DeclKind::Let,
                let_node
            ));
            assert!(!resolver.validate_declaration_name(
                &gc,
                DeclKind::Const,
                let_node
            ));
            // `Var`/`ScopedFunction` etc. are unaffected by the `let`-name
            // rule.
            assert!(resolver.validate_declaration_name(
                &gc,
                DeclKind::Var,
                let_node
            ));
        }
        assert_eq!(sm.error_count(), 2);
    }

    // ==== validateAndDeclareIdentifier (cpp:2407-2639) ==================
    //
    // These build the "previous declaration" state by hand (a `Decl` plus
    // a `binding_table` entry) rather than by parsing+declaring real source,
    // so each redeclaration-matrix row can be exercised in isolation without
    // needing the (later-task) machinery — function visiting, catch-clause
    // visiting, `ScopedFunctionPromoter` — that would otherwise be needed to
    // reach it through the full resolver.

    /// `Decl::Kind::Parameter` followed by a top-level `let` of the same
    /// name is invalid (ES10.0 14.1.2) — dormant until parameter
    /// declarations exist (S1 T7), so exercised here by hand-declaring a
    /// `Parameter` decl. The parameter lives in its own (parameter) scope,
    /// distinct from but a parent of the function's body scope — the
    /// `has_parameter_expressions` shape (SemContext.h's `FunctionInfo`
    /// doc) — which isolates the `Parameter`-specific row from the
    /// `var-like, let-like, same-scope` row (both would otherwise fire).
    #[test]
    fn parameter_then_toplevel_let_is_invalid() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let buf = sm.add_buffer_bytes("d.js", b"x x");
        let loc_param = SMLoc {
            source: buf,
            offset: 0,
        };
        let loc_let = SMLoc {
            source: buf,
            offset: 2,
        };
        let param_ident = alloc_identifier(&gc, "x", loc_param);
        let let_ident = alloc_identifier(&gc, "x", loc_let);

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let name = gc.atom_bytes("x");
        let func = sem_ctx.new_function(
            FuncIsArrow::No,
            ConstructorKind::None,
            None,
            None,
            false,
            CustomDirectives::default(),
        );
        let param_scope = sem_ctx.new_scope(func, None);
        let body_scope = sem_ctx.new_scope(func, Some(param_scope));
        // `getFunctionBodyScope` reads `functionBodyScopeIdx`, which
        // `ScopeRAII` normally sets via `is_function_body_scope` — set
        // directly here since this test bypasses `enter_scope`.
        sem_ctx.function_mut(func).function_body_scope_idx = 1;
        let param_decl = sem_ctx.new_decl_in_scope_default(
            name,
            DeclKind::Parameter,
            param_scope,
        );

        let binding_table = sem_ctx.binding_table_rc();
        let _bscope = Scope::new(&binding_table);
        binding_table.try_emplace(
            name,
            Binding::new(param_decl, Some(NodeRc::from_node(&gc, param_ident))),
        );

        {
            let mut resolver = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                true,
            );
            resolver.function_stack.push(FunctionContext {
                sem_info: func,
                node: None,
                label_map: Default::default(),
                current_loop: None,
                current_loop_or_switch: None,
                is_formal_params: false,
                decls: None,
                promoted_func_decls: Default::default(),
                binding_table_scope_depth: 0,
            });
            resolver.cur_scope = Some(body_scope);

            resolver.validate_and_declare_identifier(
                &gc,
                DeclKind::Let,
                let_ident,
            );
        }
        assert_eq!(sm.error_count(), 1);
        assert_eq!(sm.note_count(), 1);
        // No new decl was created for a rejected redeclaration.
        assert!(sem_ctx.scope(body_scope).decls.is_empty());
    }

    /// A non-ES5 `Catch` decl conflicts with a `let` in the catch's OWN
    /// (parameter) scope's child (its body block) — the
    /// `prevInPrevScope`/`Catch`/`ES5Catch` row (cpp:2525-2530). Catch
    /// clauses aren't corpus-reachable yet (no `visit(CatchClauseNode*)`),
    /// so exercised directly.
    #[test]
    fn catch_then_let_in_catch_body_is_invalid() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let buf = sm.add_buffer_bytes("d.js", b"e e");
        let loc_catch = SMLoc {
            source: buf,
            offset: 0,
        };
        let loc_let = SMLoc {
            source: buf,
            offset: 2,
        };
        let catch_ident = alloc_identifier(&gc, "e", loc_catch);
        let let_ident = alloc_identifier(&gc, "e", loc_let);

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let name = gc.atom_bytes("e");
        let func = sem_ctx.new_function(
            FuncIsArrow::No,
            ConstructorKind::None,
            None,
            None,
            false,
            CustomDirectives::default(),
        );
        let top_scope = sem_ctx.new_scope(func, None);
        sem_ctx.function_mut(func).function_body_scope_idx = 0;
        let catch_param_scope = sem_ctx.new_scope(func, Some(top_scope));
        let catch_body_scope = sem_ctx.new_scope(func, Some(catch_param_scope));
        let catch_decl = sem_ctx.new_decl_in_scope_default(
            name,
            DeclKind::Catch,
            catch_param_scope,
        );

        let binding_table = sem_ctx.binding_table_rc();
        let _bscope = Scope::new(&binding_table);
        binding_table.try_emplace(
            name,
            Binding::new(catch_decl, Some(NodeRc::from_node(&gc, catch_ident))),
        );

        {
            let mut resolver = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                true,
            );
            resolver.function_stack.push(FunctionContext {
                sem_info: func,
                node: None,
                label_map: Default::default(),
                current_loop: None,
                current_loop_or_switch: None,
                is_formal_params: false,
                decls: None,
                promoted_func_decls: Default::default(),
                binding_table_scope_depth: 0,
            });
            resolver.cur_scope = Some(catch_body_scope);

            resolver.validate_and_declare_identifier(
                &gc,
                DeclKind::Let,
                let_ident,
            );
        }
        assert_eq!(sm.error_count(), 1);
        assert_eq!(sm.note_count(), 1);
    }

    /// The `ES5Catch`-vs-`var` exception (ES10 B.3.5) does NOT live in
    /// `validateAndDeclareIdentifier` — it lives in the nested-scope `var`
    /// check inside `visit(VariableDeclarationNode *)` (cpp:336-352, the
    /// `prevKind != Decl::Kind::ES5Catch` exclusion). Proven directly by
    /// calling `visit_variable_declaration` on a hand-built `var e;` node
    /// with an `ES5Catch` decl for `e` already bound in an enclosing
    /// (non-function-body) scope: no error, even though `curScope_` is
    /// nested (which is exactly the condition that activates the check).
    #[test]
    fn es5catch_then_var_is_valid() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let buf = sm.add_buffer_bytes("d.js", b"e");
        let loc = SMLoc {
            source: buf,
            offset: 0,
        };
        let range = SMRange {
            start: loc,
            end: loc,
        };
        let catch_ident = alloc_identifier(&gc, "e", loc);
        let var_ident = alloc_identifier(&gc, "e", loc);

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let name = gc.atom_bytes("e");
        let kw_var = sem_ctx.kw.ident_var;
        let func = sem_ctx.new_function(
            FuncIsArrow::No,
            ConstructorKind::None,
            None,
            None,
            false,
            CustomDirectives::default(),
        );
        let body_scope = sem_ctx.new_scope(func, None);
        sem_ctx.function_mut(func).function_body_scope_idx = 0;
        let nested_scope = sem_ctx.new_scope(func, Some(body_scope));
        let es5catch_decl = sem_ctx.new_decl_in_scope_default(
            name,
            DeclKind::ES5Catch,
            nested_scope,
        );

        let binding_table = sem_ctx.binding_table_rc();
        let _bscope = Scope::new(&binding_table);
        binding_table.try_emplace(
            name,
            Binding::new(
                es5catch_decl,
                Some(NodeRc::from_node(&gc, catch_ident)),
            ),
        );

        {
            let mut resolver = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                true,
            );
            resolver.function_stack.push(FunctionContext {
                sem_info: func,
                node: None,
                label_map: Default::default(),
                current_loop: None,
                current_loop_or_switch: None,
                is_formal_params: false,
                decls: None,
                promoted_func_decls: Default::default(),
                binding_table_scope_depth: 0,
            });
            resolver.cur_scope = Some(nested_scope);

            let var_decl = alloc_var_decl(&gc, kw_var, var_ident, range);
            resolver.visit_variable_declaration(&gc, var_decl);
        }
        assert_eq!(sm.error_count(), 0);
        assert_eq!(sm.note_count(), 0);
    }

    /// The lexically-scoped-in-global-scope restricted-globals check
    /// (`let NaN;` at the top level) — a direct API-level counterpart to
    /// the `error-restricted-global.js` corpus file.
    #[test]
    fn restricted_global_property_rejects_lexical_shadow() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let buf = sm.add_buffer_bytes("d.js", b"NaN");
        let loc = SMLoc {
            source: buf,
            offset: 0,
        };
        let nan_node = alloc_identifier(&gc, "NaN", loc);

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let func = sem_ctx.new_function(
            FuncIsArrow::No,
            ConstructorKind::None,
            None,
            None,
            false,
            CustomDirectives::default(),
        );
        // The FIRST scope created is `ScopeId(0)` == `get_global_scope()`.
        let global_scope = sem_ctx.new_scope(func, None);
        assert_eq!(global_scope, sem_ctx.get_global_scope());

        let binding_table = sem_ctx.binding_table_rc();
        let _bscope = Scope::new(&binding_table);

        {
            let mut resolver = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                true,
            );
            resolver.function_stack.push(FunctionContext {
                sem_info: func,
                node: None,
                label_map: Default::default(),
                current_loop: None,
                current_loop_or_switch: None,
                is_formal_params: false,
                decls: None,
                promoted_func_decls: Default::default(),
                binding_table_scope_depth: 0,
            });
            resolver.cur_scope = Some(global_scope);

            resolver.validate_and_declare_identifier(
                &gc,
                DeclKind::Let,
                nan_node,
            );
        }
        assert_eq!(sm.error_count(), 1);
    }

    /// `Var, ScopedFunc` in DIFFERENT scopes, with the name already present
    /// in `promotedFuncDecls` (S3, always empty until `ScopedFunctionPromoter`
    /// lands — see the module doc): the redeclaration matrix's "reuse the
    /// promoted decl" branch (cpp:2548-2562) fires instead of declaring a
    /// fresh one, and the new binding points at the REUSED decl.
    #[test]
    fn var_then_scoped_function_reuses_promoted_decl_in_different_scope() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let buf = sm.add_buffer_bytes("d.js", b"foo foo");
        let loc_var = SMLoc {
            source: buf,
            offset: 0,
        };
        let loc_func = SMLoc {
            source: buf,
            offset: 4,
        };
        let func_ident = alloc_identifier(&gc, "foo", loc_func);

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let name = gc.atom_bytes("foo");
        let func = sem_ctx.new_function(
            FuncIsArrow::No,
            ConstructorKind::None,
            None,
            None,
            false,
            CustomDirectives::default(),
        );
        let top_scope = sem_ctx.new_scope(func, None);
        // `validateAndDeclareIdentifier` unconditionally reads `topLevel`
        // (`curScope_->parentFunction->getFunctionBodyScope() == curScope_`)
        // even though this row doesn't key off it — set it so that read
        // doesn't hit the `functionScopeIdx not set` debug_assert.
        sem_ctx.function_mut(func).function_body_scope_idx = 0;
        let var_decl =
            sem_ctx.new_decl_in_scope_default(name, DeclKind::Var, top_scope);
        let nested_scope = sem_ctx.new_scope(func, Some(top_scope));
        // The promoted decl: a distinct `Decl` standing in for the one
        // `processPromotedFuncDecls` (S3) would have already created at
        // function scope.
        let promoted_decl =
            sem_ctx.new_decl_in_scope_default(name, DeclKind::Var, top_scope);

        let binding_table = sem_ctx.binding_table_rc();
        let _bscope_top = Scope::new(&binding_table);
        binding_table.try_emplace(
            name,
            Binding::new(var_decl, Some(NodeRc::from_node(&gc, func_ident))),
        );
        // A SEPARATE binding-table scope for `nested_scope`: `try_emplace`
        // refuses a second insertion of the same key into the SAME
        // binding-table scope, so without this, the reused-decl binding
        // below would silently no-op against the still-active `top_scope`
        // entry instead of creating a fresh, shadowing one — the
        // binding-table's own notion of "current scope" is independent of
        // (but must be kept in step with) `resolver.cur_scope`'s `ScopeId`.
        let _bscope_nested = Scope::new(&binding_table);

        {
            let mut resolver = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                true,
            );
            resolver.function_stack.push(FunctionContext {
                sem_info: func,
                node: None,
                label_map: Default::default(),
                current_loop: None,
                current_loop_or_switch: None,
                is_formal_params: false,
                decls: None,
                promoted_func_decls: [(name, promoted_decl)]
                    .into_iter()
                    .collect(),
                binding_table_scope_depth: 0,
            });
            resolver.cur_scope = Some(nested_scope);

            resolver.validate_and_declare_identifier(
                &gc,
                DeclKind::ScopedFunction,
                func_ident,
            );
        }
        assert_eq!(sm.error_count(), 0);
        let new_binding = binding_table.lookup(&name).expect("binding present");
        assert_eq!(new_binding.decl, promoted_decl);
        assert_ne!(new_binding.decl, var_decl);
        let identifier = func_ident.as_identifier().unwrap();
        assert_eq!(
            sem_ctx.get_declaration_decl(identifier),
            Some(promoted_decl)
        );
        let _ = loc_var; // documents the "var foo;" the promoted decl models
    }

    /// The standalone "promoted function involves two declarations"
    /// side-table branch (cpp:2609-2625): when the SAME identifier node
    /// already carries a "declaration decl" (as it would after
    /// `processPromotedFuncDecls` ran on it, S3) AND its name is in
    /// `promotedFuncDecls`, a NEW decl is created in the current
    /// (block) scope, the binding table is `put` (not `try_emplace`) to
    /// point at it, and `setPromotedDecl` records it in the side table —
    /// crucially, the identifier's ORIGINAL "declaration decl" is left
    /// untouched (this branch never calls `setDeclarationDecl`).
    #[test]
    fn promoted_decl_side_table_branch_creates_a_new_block_scoped_decl() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sm = SourceErrorManager::new();
        let buf = sm.add_buffer_bytes("d.js", b"foo");
        let loc = SMLoc {
            source: buf,
            offset: 0,
        };
        let ident_node = alloc_identifier(&gc, "foo", loc);

        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let name = gc.atom_bytes("foo");
        let func = sem_ctx.new_function(
            FuncIsArrow::No,
            ConstructorKind::None,
            None,
            None,
            false,
            CustomDirectives::default(),
        );
        let top_scope = sem_ctx.new_scope(func, None);
        let original_decl = sem_ctx.new_decl_in_scope_default(
            name,
            DeclKind::GlobalProperty,
            top_scope,
        );
        // Simulate `processPromotedFuncDecls` having already run on this
        // exact node: it set a "declaration decl" and recorded the name.
        let identifier = ident_node.as_identifier().unwrap();
        sem_ctx.set_declaration_decl(
            ident_node.node_id(),
            identifier,
            Some(original_decl),
        );
        let block_scope = sem_ctx.new_scope(func, Some(top_scope));

        let binding_table = sem_ctx.binding_table_rc();
        let _bscope = Scope::new(&binding_table);

        {
            let mut resolver = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                true,
            );
            resolver.function_stack.push(FunctionContext {
                sem_info: func,
                node: None,
                label_map: Default::default(),
                current_loop: None,
                current_loop_or_switch: None,
                is_formal_params: false,
                decls: None,
                promoted_func_decls: [(name, original_decl)]
                    .into_iter()
                    .collect(),
                binding_table_scope_depth: 0,
            });
            resolver.cur_scope = Some(block_scope);

            resolver.validate_and_declare_identifier(
                &gc,
                DeclKind::ScopedFunction,
                ident_node,
            );
        }

        assert_eq!(sm.error_count(), 0);
        let promoted = sem_ctx
            .get_promoted_decl(ident_node.node_id())
            .expect("promoted decl side table populated");
        assert_ne!(promoted, original_decl);
        assert_eq!(sem_ctx.decl(promoted).scope, Some(block_scope));
        let new_binding = binding_table.lookup(&name).expect("binding present");
        assert_eq!(new_binding.decl, promoted);
        // The ORIGINAL "declaration decl" is untouched — this branch never
        // calls `setDeclarationDecl`.
        assert_eq!(
            sem_ctx.get_declaration_decl(identifier),
            Some(original_decl)
        );
    }

    /// `ScopedFunction, ScopedFunction` in the SAME scope: an error in
    /// strict mode, but the ES10.0 B.3.3.4 Annex B exception makes it
    /// valid (a fresh decl shadowing the first) in loose mode.
    #[test]
    fn scoped_function_redeclaration_same_scope_strict_vs_loose() {
        for strict in [true, false] {
            let mut ctx = Context::new();
            let gc = ctx.lock();
            let mut sm = SourceErrorManager::new();
            let buf = sm.add_buffer_bytes("d.js", b"foo foo");
            let loc_first = SMLoc {
                source: buf,
                offset: 0,
            };
            let loc_second = SMLoc {
                source: buf,
                offset: 4,
            };
            let first_ident = alloc_identifier(&gc, "foo", loc_first);
            let second_ident = alloc_identifier(&gc, "foo", loc_second);

            let mut sem_ctx = SemContext::new(Keywords::new(&gc));
            let name = gc.atom_bytes("foo");
            let func = sem_ctx.new_function(
                FuncIsArrow::No,
                ConstructorKind::None,
                None,
                None,
                strict,
                CustomDirectives::default(),
            );
            let scope = sem_ctx.new_scope(func, None);
            // See the sibling promoted-decl test for why this is set even
            // though this row doesn't key off `topLevel`.
            sem_ctx.function_mut(func).function_body_scope_idx = 0;
            let first_decl = sem_ctx.new_decl_in_scope_default(
                name,
                DeclKind::ScopedFunction,
                scope,
            );

            let binding_table = sem_ctx.binding_table_rc();
            let _bscope = Scope::new(&binding_table);
            binding_table.try_emplace(
                name,
                Binding::new(
                    first_decl,
                    Some(NodeRc::from_node(&gc, first_ident)),
                ),
            );

            {
                let mut resolver = SemanticResolver::new(
                    &binding_table,
                    &mut sem_ctx,
                    &mut sm,
                    &[],
                    true,
                );
                resolver.function_stack.push(FunctionContext {
                    sem_info: func,
                    node: None,
                    label_map: Default::default(),
                    current_loop: None,
                    current_loop_or_switch: None,
                    is_formal_params: false,
                    decls: None,
                    promoted_func_decls: Default::default(),
                    binding_table_scope_depth: 0,
                });
                resolver.cur_scope = Some(scope);

                resolver.validate_and_declare_identifier(
                    &gc,
                    DeclKind::ScopedFunction,
                    second_ident,
                );
            }

            if strict {
                assert_eq!(sm.error_count(), 1, "strict mode must reject it");
                assert_eq!(sm.note_count(), 1);
            } else {
                assert_eq!(sm.error_count(), 0, "loose mode must allow it");
                let identifier = second_ident.as_identifier().unwrap();
                let second_decl = sem_ctx
                    .get_declaration_decl(identifier)
                    .expect("a fresh decl was declared");
                assert_ne!(
                    second_decl, first_decl,
                    "loose mode declares a NEW decl, it doesn't reuse the first"
                );
            }
        }
    }
}
