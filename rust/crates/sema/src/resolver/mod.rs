/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Port of `hermes::sema::SemanticResolver` (`lib/Sema/SemanticResolver.h`,
//! `lib/Sema/SemanticResolver.cpp`) — S0 entry path plus S1 T4's identifier
//! resolution core, S1 T5's declarations (hoisting, validation, blocks) and
//! S1 T6's expression visits (constant folding, assignment/update/unary
//! validation).
//!
//! ## What's covered so far
//!
//! The S0 *entry path*: the constructor, `run`, `visit(ProgramNode *)` and
//! everything that path reaches (`scanDirectives`, `processAmbientDecls`,
//! `ScopeRAII`, the `FunctionContext` constructor that owns a
//! `DeclCollector`). From S1 T4, `visit(IdentifierNode *, Node *)`,
//! `resolveIdentifier`, `checkIdentifierResolved` and `declareArguments` —
//! identifier reads/writes resolve against the binding table or become
//! ambient globals, with the strict-mode `UndefinedVariable` warning and the
//! `await`/`arguments` forbid-flag errors. From S1 T5 (`declarations.rs`),
//! `extractIdentsFromDecl`/`extractDeclaredIdentsFromID`,
//! `processDeclarations`, `validateAndDeclareIdentifier`,
//! `validateDeclarationName`, `visit(VariableDeclarationNode *)` and
//! `visit(BlockStatementNode *, Node *)` — `var`/`let`/`const` (including
//! destructuring patterns) hoist, validate against the redeclaration
//! decision table, and shadow correctly across nested block scopes. From S1
//! T6 (`expressions.rs`), `visit(BinaryExpressionNode *, Node **)`,
//! `visit(AssignmentExpressionNode *)`, `visit(UpdateExpressionNode *)`,
//! `visit(UnaryExpressionNode *, Node **)`, `validateAssignmentTarget` and
//! `isLValue` — `+`/`-` and `=` chains are walked iteratively, constants
//! fold, and assignment/update targets are validated (const, strict
//! `eval`/`arguments`, the loose-mode `arguments` quirk), together with the
//! strict `delete`-of-a-variable and `delete super.x` errors. From S1 T7
//! (`functions.rs`), `visit(FunctionDeclarationNode *, Node *)`,
//! `visit(FunctionExpressionNode *, Node *)`, `visitFunctionLike`,
//! `visitFunctionLikeInFunctionContext`,
//! `visitFunctionBodyAfterParamsVisited`, `visitFunctionExpression` and
//! `visit(ReturnStatementNode *)` — function declarations and expressions
//! get their own `FunctionInfo`, their parameters are declared and
//! validated (including the dual parameter/body scope layout that
//! parameter expressions force), the implicit `arguments` object is
//! declared where the spec-deviating rule says it should be, and bodies
//! resolve inside the function scope. From S2 T1 (`statements.rs`),
//! `visit(SwitchStatementNode *)`, `visit(ForIn/ForOfStatementNode *)` +
//! `visitForInOf`, `visit(For/DoWhile/WhileStatementNode *)`,
//! `visit(LabeledStatementNode *)`, `getLabelDecorationBase` and
//! `visit(Break/ContinueStatementNode *)` — every loop shape gets its label
//! index and (where C++ creates one) its own scope with the declarations
//! hoisted into it, labeled statements maintain the per-function
//! `labelMap`, and `break`/`continue` resolve to a label index or report
//! the four "not within a loop"/"label is not defined"/"not a loop label"
//! diagnostics. From S2 T2, `visit(ArrowFunctionExpressionNode *, Node *)`
//! (`functions.rs`) — **spec §3.4 rewrite #1**: an expression-bodied arrow
//! is rewritten into a block with a single `return` before it is visited,
//! and the `containsArrowFunctions`/
//! `containsArrowFunctionsUsingArguments` bookkeeping propagates outward —
//! together with `visit(Yield/Await/SpreadElement/MetaProperty)` and the
//! five `visit(Cover*Node *)` error stubs (`expressions.rs`), which is what
//! turns S1 T7's dormant arrow branches (super/`arguments` inheritance, the
//! async-arrow `await` rule in `visitParams`) live. From S2 T3,
//! `visit(TryStatementNode *)`, `visit(CatchClauseNode *)` and
//! `visit(WithStatementNode *)` (`statements.rs`) — **spec §3.4 rewrite
//! #2**: a `try` with both a handler and a finalizer becomes two nested
//! `try`s before it is visited, catch clauses get their own scope holding
//! only the catch parameter's bindings, and `with` reports its
//! not-supported error and then runs the new `Unresolver` pass
//! (`unresolver.rs`, the port of SemanticResolver.h:679-711) over its body,
//! which is what turns S1 T5's dormant `Catch`/`ES5Catch` redeclaration
//! rows live — plus `visit(RegExpLiteralNode *)` (`expressions.rs`), whose
//! regex-engine boundary is documented at its site. From S2 T4
//! (`classes.rs`), `ClassContext` and
//! `visit(ClassDeclaration/ClassExpressionNode *)` + `visitClassAsExpr`,
//! `visit(ClassPropertyNode *)`, `visit(MethodDefinitionNode *, Node *)` and
//! `visit(SuperNode *, Node *)` — classes get their own scope holding the
//! inner `ClassExprName` binding (so a class declaration's one `Identifier`
//! carries two decls), force strict mode on the enclosing function while
//! they are being visited, and grow up to three SYNTHETIC `FunctionInfo`s
//! (the implicit constructor and the instance/static elements initializers,
//! all three recorded in `Cell`s on the class node), which is also what
//! turns S1 T7's dormant `MethodDefinition` constructor-kind branch live.
//! From S2 T5, `collectDeclaredPrivateIdentifiers` (cpp:2157-2274),
//! `declarePrivateName`/`resolvePrivateName` (cpp:2047-2080,
//! `identifiers.rs`) and the `#` mangling
//! (`sem_context::private_name_identifier`), plus
//! `visit(PrivateNameNode *)`, `visit(ClassPrivatePropertyNode *)` and
//! `visit(StaticBlockNode *)` (`classes.rs`) and
//! `visit(Member/OptionalMemberExpressionNode *, Node *)`
//! (`expressions.rs`) — private fields/methods/accessors are declared in the
//! class scope under their mangled names, the ES2024 15.7.1 duplicate and
//! accessor-pairing early errors are reported, private member access is
//! restricted (no `delete`, no `super.#x`, no load from a setter-only name,
//! no store to a getter-only name or a method), and a static block becomes
//! its OWN synthetic `is_static_block` `FunctionInfo` with a function-body
//! scope that `var`s hoist into — which is also what turns S2 T4's dormant
//! private-instance-method initializer hook (cpp:1119-1121) and
//! `create_static_block_function_info` live, and what makes S1 T4's
//! documented `typeof` double-fire quirk reachable (see
//! `tests/sema_corpus/error-static-block-typeof-arguments.js`).
//! From S2 T6, `visit(CallExpressionNode *)` (cpp:1127-1219) and
//! `registerLocalEval` (cpp:2849-2857, `calls.rs`) — a direct call to
//! `eval()` warns and marks its whole scope chain as a local-`eval` user,
//! **spec §3.4 rewrite #3** turns `$SHBuiltin.prop(...)` into a call whose
//! callee's object is an `SHBuiltin` node, and `super()` outside a derived
//! class constructor is reported, which is what turns S2 T4's
//! `MethodDefinition` `ConstructorKind` seam observable and (being a
//! `SpreadElement` whitelist parent) S2 T2's spread handling reachable in
//! call arguments.
//! From S3 T1, `getPromotedScopedFuncDecls` (the whole of
//! `lib/Sema/ScopedFunctionPromoter.cpp`, ported as `promoter.rs`) and
//! `processPromotedFuncDecls` (cpp:2143-2155, below) — a loose-mode
//! function or program containing a block-nested `function f() {}` now runs
//! the Annex B 3.3 promotion check instead of asserting: the declaration is
//! declared a SECOND time, in function (`Var`) or global
//! (`GlobalProperty`) scope, whenever no let-like binding of that name and
//! no formal parameter of that name is visible from the block. That is what
//! turns S1 T5's dormant `promotedFuncDecls` redeclaration rows
//! (`declarations.rs`) live. The THIRD C++ call site, `runInScope`
//! (cpp:158), belongs to S5's `resolve_ast_in_scope`.
//! Together this reproduces `hermesc -dump-sema`
//! byte-for-byte for programs made of literals, empty statements, bare
//! identifier reads (including through non-computed member/property
//! positions, which are skipped rather than resolved),
//! `var`/`let`/`const` declarations in blocks, functions of every
//! parameter shape, arrow functions of every body/parameter shape,
//! generators and `async` functions, the loop/label/switch statements above
//! and the expression forms above, classes with fields, methods,
//! `super.x` member access, private members and static blocks, and calls of
//! every shape (plain, optional, `new`, `eval`, `$SHBuiltin.prop(...)`,
//! `super()`) — which is what `tests/sema_differential.rs` enforces against
//! the real compiler. Imports/exports are ported too, as of S4a T3
//! (`modules.rs`'s four visit arms), but only the `compile = false` tool
//! pair can dump them, so their pins live in
//! `tests/sema_corpus_parser` rather than in the driver corpus. The
//! `$SHBuiltin` CommonJS-module protocol (`calls.rs`'s S4b-tagged panics)
//! and everything else the C++ resolver handles remain later tasks' scope —
//! see `declarations.rs`'s, `functions.rs`'s, `expressions.rs`'s,
//! `statements.rs`'s, `classes.rs`'s and `calls.rs`'s module docs for
//! exactly which of *their* branches are ported but not yet
//! corpus-reachable.
//!
//! Everything not covered is *deliberately absent rather than
//! approximated*: `visit_node` panics with `sema: unhandled node kind ...
//! (S3+/dialect phases)` for any node kind outside the handled set. An
//! honest panic keeps the differential meaningful — a silently-wrong
//! resolution would look like a passing test on a corpus that never
//! exercised it. Later tasks replace each panic with the ported code.
//! (The tag was `(S3+/typed phases)` through S4a T4; renamed there because
//! it had started overclaiming "typed" for the untyped `-parse-flow` gaps
//! this port already had — `TypeCastExpression`/`AsExpression` were two
//! such cases, closed by that task's fix review, but the tag's imprecision
//! for the *next* one outlives them, so "dialect" is the honest umbrella
//! for "Flow/TS-only, whether or not `-typed`" going forward.)
//!
//! ## The dispatch protocol: `ast::VisitorMut`
//!
//! C++'s resolver is a `RecursiveVisitor` that mutates the AST *in place*:
//! an overload declared as `visit(NodeType *n, Node **ppNode)` writes
//! through `ppNode` to replace the current node
//! (`RecursiveVisitor.h:132-142`; `visit(BinaryExpressionNode *, Node **)`,
//! SemanticResolver.cpp:405-436, uses it for constant folding). The pointer
//! it writes to is literally the child field inside the parent node.
//!
//! This port's AST is immutable in its structural fields, so the same
//! capability comes from the `ast` crate's phase-3 transform machinery: the
//! resolver implements [`ast::visitor::VisitorMut`] and every visit returns
//! a [`TransformResult`]. `TransformResult::Changed(n)` *is* `*ppNode = n`;
//! the generated `Node::visit_children_mut` re-runs the per-kind builder and
//! rebuilds an ancestor only when one of its children changed, so an
//! unrewritten tree comes back pointer-identical and unchanged subtrees stay
//! shared. `Removed`/`Expanded`, which C++ can only express by editing an
//! intrusive `NodeList` (`kEnableNodeListMutation`,
//! RecursiveVisitor.h:157-161), come for free.
//!
//! The trait is implemented directly rather than wrapped in a
//! sema-private protocol: its `call(gc, node, path)` signature already
//! carries everything the C++ dispatcher hands a visit overload — the
//! `GCLock`, the node, and (through `Path`) the parent plus the field of the
//! parent the node occupies — while `&mut self` carries the resolver state.
//! Implementing it means every node kind the resolver does *not* override
//! recurses through the generated `visit_children_mut`, which is the only
//! rebuild path in the crate; no per-node rebuild is ever hand-written here.
//!
//! Two invariants this buys, which every later stage depends on:
//!
//! - **Decorate before recursing.** `visit_children_mut` snapshots the
//!   node's `Cell` decorations (`scope`, `sem_info`, `strictness`, ...) into
//!   the builder when it starts, so annotations written *before* the
//!   children are visited survive into the rebuilt node, and annotations
//!   written after would be lost on a node that gets rebuilt. This is a
//!   requirement on each ported visit, **not** a universal property of the
//!   C++: it has been verified for `visit(ProgramNode *)` (which sets
//!   everything, then visits children last) and holds for the S1 visits, and
//!   spec §3.4 obligation (b) is the standing audit of every remaining
//!   visit.
//!
//!   **The known exception, DISCHARGED in S2 T1.**
//!   `visit(SwitchStatementNode *)` (SemanticResolver.cpp:520-539) visits
//!   `_discriminant` FIRST (:522) and only then writes
//!   `node->setLabelIndex(...)` (:526). Under this mechanism a folded
//!   discriminant (`switch (1+2)`) rebuilds the `SwitchStatement`, and a
//!   naive "snapshot the builder, then write the label" would drop
//!   `label_index` — a break-target miscompile.
//!   `statements::SemanticResolver::visit_switch_statement` handles it by
//!   writing both decorations on the original node and creating the builder
//!   only afterwards, so the rebuilt node carries them; see
//!   `statements.rs`'s module doc for the full argument, and
//!   `a_rebuilt_switch_keeps_its_label_index_and_scope` in
//!   `tests/resolver.rs` for the regression test (the differential is blind
//!   to it — `label_index` is never dumped). Spec §3.4 obligation (b)
//!   remains the standing audit for the visits still to be ported.
//!
//!   **S2 T2 audit result: no new exception, but one related trap.**
//!   `visit(ArrowFunctionExpressionNode *)` (cpp:249-275) writes the
//!   `Cell<bool>` `_expression` *before* recursing, so the invariant holds
//!   as stated — but the node it must write it on is the REWRITTEN arrow the
//!   visit builds (rewrite #1), not the arrow it was handed, which is
//!   discarded. Writing it on the incoming node would be silently lost.
//!   Same rule, same reason: the write has to land on the node that will be
//!   returned. See `functions.rs`'s "Rewrite #1" section and
//!   `a_rewritten_arrow_whose_body_folds_keeps_its_decorations`.
//!
//!   **S2 T4: the second real exception, and the widest one.** All three
//!   `ClassLikeDecoration` `Cell`s are written either from DEEP INSIDE the
//!   class body walk (the elements-init ids, from
//!   `visit(ClassPropertyNode *)`) or AFTER it (the implicit-constructor id,
//!   cpp:949, because it depends on `hasConstructor`, which only the walk can
//!   set). A fold in a field initializer or a rewritten arrow in a method
//!   rebuilds the `ClassBody` and hence the class node, so a builder
//!   snapshotted up front would hand back a class with all three empty —
//!   silently detaching the synthetic functions IRGen looks for.
//!   `classes::SemanticResolver::visit_class_as_expr` therefore drives the
//!   two children by hand and creates the builder only after the last write;
//!   see `classes.rs`'s module doc and
//!   `a_rebuilt_class_keeps_its_synthetic_function_infos`.
//!
//!   **S2 T6 audit result: no exception.** `visit(CallExpressionNode *)`
//!   (cpp:1127-1219) writes NO decoration at all — `CallExpression` has no
//!   `Cell` fields — and rewrite #3's other rebuilt node, the callee
//!   `MemberExpression`, carries only `computed`, which the generated
//!   builder's `from_node` copies. The one thing that visit does have to get
//!   right is the mirror image: the rewrite must be reported as `Changed`
//!   even when the rebuilt node's own children walk reports `Unchanged`
//!   (`$SHBuiltin.bar()` — no arguments, nothing below changes), which
//!   `calls::visit_call_expression`'s tail does explicitly and
//!   `tests/sema_corpus/shbuiltin-calls.js` pins.
//! - **Node identity is not stable across a rebuild.** A rebuilt node is a
//!   new allocation with a fresh `NodeId` (`NodeMetadata::duplicate`), so
//!   anything keyed by node identity — a `NodeRc` held in a side table, or a
//!   `NodeId` key like `DeclCollector::scope_decls_for_node`'s — must be
//!   consulted *before* that node's children are transformed. Again the C++
//!   order already does it: `processCollectedDeclarations(node)` runs on
//!   entry to a scope, before the scope's children are visited. Leaves
//!   (`Identifier` above all) are never rebuilt — a parent rebuild copies
//!   the child *pointer* — so the decorations the resolver writes on them,
//!   and the `NodeRc`s the binding table keeps to them, stay valid.
//!
//!   **Backref fixup obligation (spec §3.4 (a)).** The two sema records that
//!   keep a `NodeRc` to an *interior* node —
//!   `LexicalScope::hoisted_functions` (`sem_context.rs`) and
//!   `FunctionInfo::imports` — are not covered by the "leaves are never
//!   rebuilt" argument: a fold anywhere inside a hoisted
//!   `FunctionDeclaration` rebuilds it, stranding the recorded `NodeRc` on a
//!   node that is no longer part of the returned tree. Whenever a node that
//!   carries `sem_info`/`scope` (or is recorded in `hoisted_functions` /
//!   `imports`) ends up on a rebuilt spine, the corresponding sema-record
//!   `NodeRc` must be patched to the new node.
//!
//!   `hoisted_functions` is populated as of S1 T7 (`visit(
//!   FunctionDeclarationNode *)`, cpp:236) and DISCHARGES the obligation
//!   there: `functions::visit_function_declaration` remembers the scope and
//!   index it pushed into and rewrites that slot when the visit returns
//!   `Changed` — see `functions.rs`'s module doc for the mechanism and for
//!   why only a unit test (not the differential) can catch a regression.
//!   `FunctionInfo::imports` is populated as of S4a T3 (`visit(
//!   ImportDeclarationNode *)`, cpp:887) and DISCHARGES the obligation
//!   there too, by the same mechanism: `modules::visit_import_declaration`
//!   remembers the `FunctionInfo` and index it pushed into and rewrites
//!   that slot when the children walk returns `Changed` — see `modules.rs`'s
//!   module doc, and `import_backref_is_untouched_without_a_rebuild` in
//!   `tests/resolver.rs` for the unit test that pins it (the differential
//!   is blind: `imports` is never dumped). With both records covered, spec
//!   §3.4 (a) is fully discharged.
//!
//! A visit that needs to do work *between* two children (C++
//! `visit(AssignmentExpressionNode *)` validates `_left` before visiting
//! `_right`, cpp:457-461) cannot use the all-or-nothing
//! `visit_children_mut`. It uses the same generated builder that
//! `visit_children_mut` uses, driving one child at a time:
//!
//! ```ignore
//! let builder::Builder::AssignmentExpression(mut b) =
//!     builder::Builder::from_node(node) else { unreachable!() };
//! let path = Path::new(node, NodeField::left);
//! if let TransformResult::Changed(v) = self.call(gc, n.left, Some(path)) {
//!     b.left(v);
//! }
//! // ... work that must happen between the two children ...
//! b.build(gc)
//! ```
//!
//! `self.call` (not `self.visit_node`) is what keeps the recursion-depth
//! brackets on the child. The `NodeChild::visit_child_mut` shim that the
//! generated code uses to map `Removed`/`Expanded` onto a single child is
//! `pub(crate)` inside `ast`, so such a visit must decide for itself what a
//! `Removed` child would mean — no resolver visit produces one.
//!
//! The same rule has a sharp edge in the *fold loops* over a linearized
//! operator chain (`visit(BinaryExpressionNode *, Node **)`, cpp:420-429):
//! C++ folds `list[i]` into `list[i+1]->_left` and `break`s out of the loop
//! the first time `astFoldBinaryExpression` fails, because every later fold
//! depends on this one's result. In this port a fold produces a *new* node,
//! so the loop must keep going after the `break`-equivalent — not to keep
//! folding, but to keep REBUILDING the remaining links of the chain, each
//! with the replacement produced below it — and finally return the new
//! outermost node as `Changed`. Stopping at the failed fold the way C++ does
//! would silently discard every fold that already succeeded (they live only
//! in nodes nothing points at yet). Same shape for
//! `visit(AssignmentExpressionNode *)`'s chain. Both are implemented in
//! `expressions.rs` (S1 T6) — see that module's doc for the full mapping,
//! including why the loop's `folding` flag is the port of C++'s `break` and
//! why the visit ORDER and recursion-depth accounting still match.
//!
//! The recursion-depth protocol the C++ dispatcher implements
//! (`incRecursionDepth`/`decRecursionDepth` bracketing every dispatched
//! node, RecursiveVisitor.h:197-232) is ported onto
//! [`SemanticResolver::call`], which is this port's dispatcher entry: every
//! child reaches the resolver through it.
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
//! - **`saveDecls_` and `typed_`** are not ported: nothing on this path
//!   reads them, and each belongs to a later stage that will introduce it
//!   with its first use (`typed_` is a documented `const TYPED: bool =
//!   false` at each of the three sites that branch on it).
//!   **`curClassContext_`** was in this list until S2 T4, which ports it as
//!   the `class_stack` field — see `classes.rs`. `canReferenceSuper_` and
//!   `forbidAwaitExpression_` are ported as fields (S1 T4 adds all five
//!   `SemanticResolver.h:79-104` flags together, since later tasks
//!   save/restore them as one group); the visits that first *read* them
//!   landed later — `forbidAwaitExpression_` in S2 T2
//!   (`visit(AwaitExpressionNode *)`) and `canReferenceSuper_` in S2 T4
//!   (`visit(SuperNode *, Node *)`). `forbidAwaitAsIdentifier_`,
//!   `forbidSpecialArgumentsReference_` and `forbidArgumentsAsIdentifier_`
//!   are read by `resolveIdentifier` (below) starting with S1 T4.
//! - **`DebugInfoSetting::ALL`**: the port has no debug-info setting yet, so
//!   both tests against it on this path are ported as a documented constant
//!   `false` — see [`DEBUG_INFO_SETTING_ALL`].

use std::collections::{HashMap, HashSet};

mod calls;
mod classes;
mod declarations;
mod expressions;
mod functions;
mod identifiers;
mod modules;
mod promoter;
mod statements;
mod unresolver;

use ast::context::{GCLock, NodeRc};
use ast::node::Node;
use ast::node_child::{NodeLabel, Strictness};
use ast::visitor::{Path, TransformResult, Visitor, VisitorMut};
use ast::SemaId;
use support::diag::{Subsystem, Warning};
use support::manager::SourceErrorManager;
use support::persistent_scoped_map::Scope;

use crate::decl_collector::DeclCollector;
use classes::ClassContext;
use promoter::get_promoted_scoped_func_decls;
use crate::ids::{DeclId, FunctionInfoId, ScopeId};
use crate::keywords::Keywords;
use crate::sem_context::{
    Atom, Binding, BindingTable, BindingTableScopePtr, ConstructorKind,
    CustomDirectives, DeclKind, SemContext, SourceVisibility,
};

/// Port of `ESTree::kASTMaxRecursionDepth` (RecursiveVisitor.h:686-692); the
/// initial value of `RecursionDepthTracker::recursionDepth_`
/// (RecursiveVisitor.h:712-713), which `SemanticResolver` derives from
/// (SemanticResolver.h:28). The C++ `#if` is:
/// ```text
///   HERMES_LIMIT_STACK_DEPTH || _MSC_VER  -> 512
///   otherwise                             -> 1024
/// ```
/// `HERMES_LIMIT_STACK_DEPTH` is defined for AddressSanitizer/UBSan builds
/// (Support/Compiler.h:106-110).
///
/// RUST MAPPING: same reasoning as the parser's `MAX_RECURSION_DEPTH` (see
/// `parser::js::MAX_RECURSION_DEPTH`) — Rust has no stable
/// `cfg(sanitize = ...)`, so `debug_assertions` stands in: a debug Rust build
/// has ASan-sized frames and is what the differential gates pair against the
/// ASan `hermesc`/`sema-parser-dump` oracles; a release build gets C++'s
/// release value.
///
/// CAVEAT: a RELEASE Rust build differentialed against the ASan oracles
/// mismatches on deep-nesting inputs (1024 vs 512). Pair the tools by profile.
const AST_MAX_RECURSION_DEPTH: u32 =
    if cfg!(debug_assertions) { 512 } else { 1024 };

/// Port of `astContext_.getDebugInfoSetting() == DebugInfoSetting::ALL`.
///
/// `DebugInfoSetting` (`include/hermes/AST/Context.h`) is not ported yet — it
/// is a compiler-driver knob (`-g3`), not something sema computes — and
/// nothing on the S0 path can set it, so both uses on this path (`ScopeRAII`,
/// SemanticResolver.cpp:2948-2950; `visit(ProgramNode *)`, cpp:219-221) test
/// this constant instead. The `if` statements are kept in the exact shape of
/// the C++ code so that porting the real setting later is a one-line change.
const DEBUG_INFO_SETTING_ALL: bool = false;

/// Port of `FunctionContext::Label` (SemanticResolver.h:531-538).
///
/// Constructed by `resolver/statements.rs`'s `visit_labeled_statement`
/// (S2 T1) — `FunctionContext::label_map` was always empty before then.
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
    /// declaration in function scope. Filled by
    /// `process_promoted_func_decls` (S3 T1); empty before that task.
    ///
    /// C++'s `DenseMap<UniqueString *, Decl *>` can hold a null `Decl *`;
    /// this `DeclId` cannot — see `process_promoted_func_decls` for why that
    /// state is unreachable.
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
    /// (SemanticResolver.cpp:1762-1765).
    use_strict_node: Option<&'ast Node<'ast>>,
    /// The strongest source-visibility directive seen.
    source_visibility: SourceVisibility,
    /// Whether an "inline" directive was seen (and not cancelled).
    always_inline: bool,
    /// Whether a "noinline" directive was seen (and not cancelled).
    no_inline: bool,
    /// Whether a "builtin" directive was seen. Copied into
    /// `FunctionInfo::custom_directives.builtin` by
    /// `visitFunctionLikeInFunctionContext` (cpp:1730); also read by
    /// `hasBuiltinDirective` (cpp:2830-2838), which is S2 scope.
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
    /// The stack of class contexts; the last one is C++'s
    /// `curClassContext_` (SemanticResolver.h:63-64). Empty outside a
    /// class. Pushed/popped by `classes.rs`'s `enter_class`/`exit_class`,
    /// the port of `ClassContext`'s constructor/destructor.
    class_stack: Vec<ClassContext>,
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

    // ---- The five S1 T4 forbid/permission flags (SemanticResolver.h:79-104)
    // ---- see the module doc for why all five are added together.
    /// Whether this function can currently make super references. When
    /// entering a function that was defined using method syntax, a super
    /// binding exists. Arrow functions inherit this flag. The only other
    /// super bindings exist in class field initializer values and static
    /// blocks. Port of `canReferenceSuper_` (SemanticResolver.h:79);
    /// save/restored around every function by `visit_function_like` (S1 T7)
    /// and, since S2 T4, read by `classes::visit_super` and save/restored
    /// by `classes::visit_class_property` (`false` for a computed key,
    /// `true` for a field initializer).
    can_reference_super: bool,
    /// 'await' isn't allowed to be an identifier anywhere in the parameters
    /// of an async arrow function, including the parameters of nested arrow
    /// functions in the parameter initializers. Ordinarily we'd check for
    /// this in the parser, but async arrow functions have a reparse step, so
    /// we avoid revisiting the entire tree by checking in
    /// `SemanticResolver`. Port of `forbidAwaitAsIdentifier_`
    /// (SemanticResolver.h:95); read by `resolve_identifier`.
    forbid_await_as_identifier: bool,
    /// True if we are forbidding await expressions. Port of
    /// `forbidAwaitExpression_` (SemanticResolver.h:98); save/restored
    /// around every function by `visit_function_like_in_function_context`
    /// (S1 T7) and read by `expressions::visit_await_expression` (S2 T2).
    forbid_await_expression: bool,
    /// True if we are forbidding the reference to the special 'arguments'
    /// object. Port of `forbidSpecialArgumentsReference_`
    /// (SemanticResolver.h:101); read by `resolve_identifier`.
    forbid_special_arguments_reference: bool,
    /// True if 'arguments' cannot be used as any identifier. Port of
    /// `forbidArgumentsAsIdentifier_` (SemanticResolver.h:104); read by
    /// `resolve_identifier`.
    forbid_arguments_as_identifier: bool,
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
            class_stack: Vec::new(),
            binding_scopes: Vec::new(),
            recursion_depth: AST_MAX_RECURSION_DEPTH,
            can_reference_super: false,
            forbid_await_as_identifier: false,
            forbid_await_expression: false,
            forbid_special_arguments_reference: false,
            forbid_arguments_as_identifier: false,
        }
    }

    /// Run semantic resolution and store the result in `sem_ctx`. Port of
    /// `SemanticResolver::run` (cpp:65-70).
    ///
    /// \param root the top-level program node to run resolution on.
    /// \return the (possibly new) root, or `None` on error.
    ///
    /// C++ returns a plain `bool` because it mutates in place, and it
    /// dispatches through `visitESTreeNodeNoReplace`
    /// (RecursiveVisitor.h:644-652), which asserts the root itself was not
    /// replaced. Here a rewrite anywhere in the tree rebuilds every ancestor
    /// up to the root, so the root must be handed back; `None` is C++'s
    /// `false`. The root node itself is still never *replaced* by a visit —
    /// only rebuilt — and never removed, hence the `expect` below.
    pub fn run<'gc>(
        &mut self,
        gc: &'gc GCLock,
        root: &'gc Node<'gc>,
    ) -> Option<&'gc Node<'gc>> {
        if self.sm.error_count() != 0 {
            return None;
        }
        let new_root = root.visit_mut(gc, self, None);
        if self.sm.error_count() != 0 {
            return None;
        }
        Some(new_root.expect("the resolver never removes the root"))
    }

    /// Run semantic resolution and return the (possibly rebuilt) root
    /// REGARDLESS of whether resolution reported errors. For callers that
    /// must dump the tree even after errors — currently only
    /// `resolve_ast_for_parser`, the `compile = false` port of
    /// `resolveASTForParser` (`SemResolve.cpp:295-306`), whose C++ oracle
    /// (`tools/sema-parser-dump`) always dumps because `SemanticResolver`
    /// mutates the AST in place: `SemanticResolver::run`
    /// (`SemanticResolver.cpp:65-70`) has the SAME two `sm_.getErrorCount()`
    /// gates as [`Self::run`] below, but they only affect its `bool` return
    /// value — the caller's `root` pointer is unaffected either way, so it
    /// can always be handed to `semDump`.
    ///
    /// This port's resolver is a transforming visitor instead (see the
    /// module doc): a rewrite anywhere in the tree rebuilds every ancestor,
    /// so [`Self::run`] must hand back the rebuilt root — and folds that
    /// together with success/failure into a single `Option`, because its
    /// only caller ([`crate::resolve::resolve_ast`], the `compile = true`
    /// driver path) never needs the tree on failure (`hermesc` never dumps
    /// after a `resolveAST` failure either — see `resolve_ast`'s doc). This
    /// method exists SEPARATELY, rather than changing [`Self::run`]'s
    /// contract, so that driver-path behavior is untouched.
    ///
    /// \return the ORIGINAL `root` if the entry gate fires (mirroring C++,
    ///   which never starts visiting in that case either, so nothing is
    ///   rebuilt there — `SemanticResolver.cpp:66-67`); otherwise the
    ///   rebuilt tree from the walk, whether or not it reported errors.
    ///   Callers that need to know whether resolution itself succeeded
    ///   must check `sm.error_count()` afterwards (as the C++ tool does —
    ///   it ignores `resolveASTForParser`'s bool return entirely).
    pub fn run_always<'gc>(
        &mut self,
        gc: &'gc GCLock,
        root: &'gc Node<'gc>,
    ) -> &'gc Node<'gc> {
        if self.sm.error_count() != 0 {
            return root;
        }
        root.visit_mut(gc, self, None)
            .expect("the resolver never removes the root")
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
    /// (SemanticResolver.cpp:2977-3006), fused with the `SaveAndRestore` of
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
        // C++'s depth-exceeded lambda (cpp:2999-3003) mutates the resolver
        // from inside the collector's walk. That closure cannot borrow
        // `self` mutably here (the `kw` argument already borrows it), so it
        // only records the offending node and the two effects are applied
        // right after the walk. Not observable: nothing reads
        // `recursion_depth` during the walk (the collector took its own
        // copy), and nothing else emits a diagnostic during it either, so
        // neither the value nor the diagnostic order can differ.
        let mut depth_exceeded_at: Option<&'ast Node<'ast>> = None;
        let decls = DeclCollector::run(
            node,
            gc,
            &self.sem_ctx.kw,
            self.recursion_depth,
            &mut |n| depth_exceeded_at = Some(n),
        );
        if let Some(n) = depth_exceeded_at {
            // Inform the resolver that we have gone too deep.
            self.recursion_depth = 0;
            self.recursion_depth_exceeded(n);
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

    /// Port of the `FunctionContext(SemanticResolver &, FunctionInfo *)`
    /// constructor (SemanticResolver.cpp:3008-3016) — the one that adopts an
    /// ALREADY-CREATED `FunctionInfo` and runs no `DeclCollector`.
    ///
    /// Its callers are `classes.rs`'s `visit_class_property` and
    /// `visit_class_private_property` (both ports of the corresponding
    /// `visit(ClassPropertyNode *)`/`visit(ClassPrivatePropertyNode *)`,
    /// cpp:1039-1043), which push a context for one of the class's
    /// synthetic elements-initializer functions so a field initializer
    /// resolves as if it were inside that function. Consequently `node` is
    /// `None` (C++ sets it to `nullptr`) and `decls` is `None`: there is no
    /// AST node for the synthesized function, hence nothing to collect
    /// declarations from, and `setSemInfo` is not called on anything.
    fn enter_function_with_info(
        &mut self,
        sem_info: FunctionInfoId,
    ) -> FunctionState {
        self.function_stack.push(FunctionContext {
            sem_info,
            node: None,
            label_map: HashMap::new(),
            current_loop: None,
            current_loop_or_switch: None,
            is_formal_params: false,
            decls: None,
            promoted_func_decls: HashMap::new(),
            binding_table_scope_depth: 0,
        });
        FunctionState {
            was_global_function_context: false,
        }
    }

    /// Port of the `FunctionContext(SemanticResolver &, StaticBlockNode *,
    /// FunctionInfo *)` constructor (SemanticResolver.cpp:3018-3037) — the
    /// one that adopts an already-created `FunctionInfo` (the one
    /// `ClassContext::createStaticBlockFunctionInfo` just made) AND runs a
    /// `DeclCollector` over the static block, so that `var`s inside it hoist
    /// to the block rather than to the enclosing function.
    ///
    /// Its only caller is `classes.rs`'s `visit(StaticBlockNode *)`
    /// (cpp:1072). Like `enter_function_with_info`, `node` is `None` (C++
    /// sets it to `nullptr` even though it HAS a node — which is why
    /// `getFunctionName` reports no name for a static block) and
    /// `setSemInfo` is not called; unlike it, `decls` is populated.
    fn enter_function_static_block<'ast>(
        &mut self,
        gc: &'ast GCLock,
        node: &'ast Node<'ast>,
        sem_info: FunctionInfoId,
    ) -> FunctionState {
        // Same deviation as `enter_function`'s: C++'s depth-exceeded lambda
        // mutates the resolver from inside the collector's walk, which this
        // port cannot do while `&self.sem_ctx.kw` is borrowed, so the two
        // effects are applied right after the walk. See `enter_function` for
        // why that is not observable.
        let mut depth_exceeded_at: Option<&'ast Node<'ast>> = None;
        let decls = DeclCollector::run(
            node,
            gc,
            &self.sem_ctx.kw,
            self.recursion_depth,
            &mut |n| depth_exceeded_at = Some(n),
        );
        if let Some(n) = depth_exceeded_at {
            // Inform the resolver that we have gone too deep.
            self.recursion_depth = 0;
            self.recursion_depth_exceeded(n);
        }

        self.function_stack.push(FunctionContext {
            sem_info,
            node: None,
            label_map: HashMap::new(),
            current_loop: None,
            current_loop_or_switch: None,
            is_formal_params: false,
            decls: Some(decls),
            promoted_func_decls: HashMap::new(),
            binding_table_scope_depth: 0,
        });
        FunctionState {
            was_global_function_context: false,
        }
    }

    /// Port of `FunctionContext::~FunctionContext`
    /// (SemanticResolver.cpp:3063-3084) plus the call site's
    /// `SaveAndRestore` restore.
    fn exit_function(&mut self, state: FunctionState) {
        self.function_stack
            .pop()
            .expect("no active function context");
        if state.was_global_function_context {
            self.global_function_context = None;
        }
    }

    // ---- RecursionDepthTracker -------------------------------------------

    /// Port of `SemanticResolver::recursionDepthExceeded`
    /// (SemanticResolver.cpp:2773-2776).
    fn recursion_depth_exceeded(&mut self, node: &Node) {
        self.sm.error(
            node.range().end,
            "Too many nested expressions/statements/declarations",
        );
    }

    /// Port of `RecursionDepthTracker::incRecursionDepth`
    /// (RecursiveVisitor.h:721-730), which `SemanticResolver` inherits
    /// (SemanticResolver.h:27-28). It maintains the current AST nesting
    /// level, and generates an error the first time it exceeds the maximum
    /// nesting level. Once that happens, it always returns false.
    ///
    /// \return true if everything is normal, false if we should not visit
    ///   the current node.
    fn inc_recursion_depth(&mut self, node: &Node) -> bool {
        if self.recursion_depth == 0 {
            return false;
        }
        self.recursion_depth -= 1;
        if self.recursion_depth == 0 {
            self.recursion_depth_exceeded(node);
            return false;
        }
        true
    }

    /// Port of `RecursionDepthTracker::decRecursionDepth`
    /// (RecursiveVisitor.h:735-738). Once we have reached the maximum
    /// nesting level, it does nothing. Otherwise it decrements the nesting
    /// level.
    fn dec_recursion_depth(&mut self) {
        if self.recursion_depth != 0 {
            self.recursion_depth += 1;
        }
    }

    // ---- ScopeRAII -------------------------------------------------------

    /// Create a binding scope and push a semantic scope. Port of
    /// `SemanticResolver::ScopeRAII::ScopeRAII`
    /// (SemanticResolver.cpp:2933-2958); the C++ member-initializer list
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
    /// (SemanticResolver.cpp:2959-2961) plus the implicit destruction of the
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

    /// Dispatch to the `visit()` overload for `node`'s kind. Port of the
    /// `switch (node->getKind())` inside `RecursiveVisitorDispatch::visit`
    /// (RecursiveVisitor.h:204-229), which C++ generates from
    /// `ESTree.def`; the recursion-depth brackets around it live in
    /// [`SemanticResolver::call`], like the C++ dispatcher's.
    ///
    /// \param path the field of the parent node `node` occupies, or `None`
    ///   for the root. C++ passes the bare `parent` pointer; `Path::parent`
    ///   is the same thing.
    ///
    /// Only implements the kinds the S0/S1-T4 corpus can produce; see the
    /// module doc for why the fallback is a panic rather than a generic
    /// recursion.
    fn visit_node<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
        path: Option<Path<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>> {
        match node {
            Node::Program(_) => self.visit_program(gc, node),
            // `visit(IdentifierNode*, Node*)` (cpp:277-323) — see
            // `visit_identifier`.
            Node::Identifier(_) => self.visit_identifier(gc, node, path),
            // `visit(VariableDeclarationNode*)` (cpp:325-403) — see
            // `declarations::visit_variable_declaration` (S1 T5).
            Node::VariableDeclaration(_) => {
                self.visit_variable_declaration(gc, node)
            }
            // `visit(BlockStatementNode*, Node*)` (cpp:502-518) — see
            // `declarations::visit_block_statement` (S1 T5).
            Node::BlockStatement(_) => {
                self.visit_block_statement(gc, node, path)
            }
            // `visit(BinaryExpressionNode*, Node**)` (cpp:405-436),
            // `visit(AssignmentExpressionNode*)` (cpp:438-462),
            // `visit(UpdateExpressionNode*)` (cpp:464-473) and
            // `visit(UnaryExpressionNode*, Node**)` (cpp:475-500) — see
            // `expressions::*` (S1 T6).
            Node::BinaryExpression(_) => self.visit_binary_expression(gc, node),
            Node::AssignmentExpression(_) => {
                self.visit_assignment_expression(gc, node)
            }
            Node::UpdateExpression(_) => self.visit_update_expression(gc, node),
            Node::UnaryExpression(_) => self.visit_unary_expression(gc, node),
            // `visit(FunctionDeclarationNode*, Node*)` (cpp:233-243),
            // `visit(FunctionExpressionNode*, Node*)` (cpp:244-248) and
            // `visit(ReturnStatementNode*)` (cpp:1483-1489) — see
            // `functions::*` (S1 T7); `visit(ArrowFunctionExpressionNode*,
            // Node*)` (cpp:249-275), which carries rewrite #1 — see
            // `functions::visit_arrow_function_expression` (S2 T2).
            Node::FunctionDeclaration(_) => {
                self.visit_function_declaration(gc, node, path)
            }
            Node::FunctionExpression(_) => {
                self.visit_function_expression(gc, node, path)
            }
            Node::ArrowFunctionExpression(_) => {
                self.visit_arrow_function_expression(gc, node, path)
            }
            Node::ReturnStatement(_) => self.visit_return_statement(gc, node),
            // `visit(SwitchStatementNode*)` (cpp:520-539),
            // `visit(ForInStatementNode*)`/`visit(ForOfStatementNode*)` +
            // `visitForInOf` (cpp:541-598),
            // `visit(ForStatementNode*)` (cpp:600-614),
            // `visit(DoWhileStatementNode*)` (cpp:616-625),
            // `visit(WhileStatementNode*)` (cpp:626-635),
            // `visit(LabeledStatementNode*)` (cpp:637-678),
            // `visit(BreakStatementNode*)` (cpp:695-721) and
            // `visit(ContinueStatementNode*)` (cpp:723-755) — see
            // `statements::*` (S2 T1).
            Node::SwitchStatement(_) => self.visit_switch_statement(gc, node),
            Node::ForInStatement(_) | Node::ForOfStatement(_) => {
                self.visit_for_in_of(gc, node)
            }
            Node::ForStatement(_) => self.visit_for_statement(gc, node),
            Node::WhileStatement(_) | Node::DoWhileStatement(_) => {
                self.visit_while_like(gc, node)
            }
            Node::LabeledStatement(_) => {
                self.visit_labeled_statement(gc, node)
            }
            Node::BreakStatement(_) => self.visit_break_statement(gc, node),
            Node::ContinueStatement(_) => {
                self.visit_continue_statement(gc, node)
            }
            // `visit(YieldExpressionNode*)` (cpp:1490-1506),
            // `visit(AwaitExpressionNode*)` (cpp:1508-1522),
            // `visit(SpreadElementNode*, Node*)` (cpp:1469-1481),
            // `visit(MetaPropertyNode*)` (cpp:837-872) and the five
            // `visit(Cover*Node*)` overloads (cpp:1572-1591) — see
            // `expressions::*` (S2 T2).
            Node::YieldExpression(_) => self.visit_yield_expression(gc, node),
            Node::AwaitExpression(_) => self.visit_await_expression(gc, node),
            Node::SpreadElement(_) => {
                self.visit_spread_element(gc, node, path)
            }
            Node::MetaProperty(_) => self.visit_meta_property(gc, node),
            Node::CoverEmptyArgs(_)
            | Node::CoverTrailingComma(_)
            | Node::CoverInitializer(_)
            | Node::CoverRestElement(_)
            | Node::CoverTypedIdentifier(_) => self.visit_cover_node(node),
            // `visit(TypeCastExpressionNode *)` (cpp:1605-1608) and
            // `visit(AsExpressionNode *)` (cpp:1610-1613), both `#if
            // HERMES_PARSE_FLOW`: "visit the expression, but not the type
            // annotation" — see `expressions::visit_type_cast_expression`'s
            // doc for why that is not the override-free generic arm below
            // (S4a T4 fix-review). Reachable under plain untyped
            // `-parse-flow`, no `-typed` needed: `(x: number);` and
            // `x as number;` both resolve at exit 0.
            Node::TypeCastExpression(_) => {
                self.visit_type_cast_expression(gc, node)
            }
            Node::AsExpression(_) => self.visit_as_expression(gc, node),
            // `visit(WithStatementNode*)` (cpp:757-769),
            // `visit(TryStatementNode*)` (cpp:771-811, rewrite #2) and
            // `visit(CatchClauseNode*)` (cpp:813-819) — see `statements::*`
            // (S2 T3); `visit(RegExpLiteralNode*)` (cpp:821-835) — see
            // `expressions::visit_regexp_literal` (S2 T3).
            Node::WithStatement(_) => self.visit_with_statement(gc, node),
            Node::TryStatement(_) => self.visit_try_statement(gc, node),
            Node::CatchClause(_) => self.visit_catch_clause(gc, node),
            Node::RegExpLiteral(_) => self.visit_regexp_literal(gc, node),
            // `visit(ClassDeclarationNode*)` (cpp:891-907),
            // `visit(ClassExpressionNode*)` (cpp:909-911) +
            // `visitClassAsExpr` (cpp:913-950),
            // `visit(ClassPropertyNode*)` (cpp:1013-1061),
            // `visit(MethodDefinitionNode*, Node*)` (cpp:1104-1125) and
            // `visit(SuperNode*, Node*)` (cpp:1096-1102) — see `classes::*`
            // (S2 T4).
            Node::ClassDeclaration(_) => {
                self.visit_class_declaration(gc, node)
            }
            Node::ClassExpression(_) => self.visit_class_expression(gc, node),
            Node::ClassProperty(_) => self.visit_class_property(gc, node),
            Node::MethodDefinition(_) => {
                self.visit_method_definition(gc, node)
            }
            Node::Super(_) => self.visit_super(path),
            // `visit(PrivateNameNode*)` (cpp:952-963),
            // `visit(ClassPrivatePropertyNode*)` (cpp:965-1011) and
            // `visit(StaticBlockNode*)` (cpp:1063-1094) — see `classes::*`
            // (S2 T5); `visit(MemberExpressionNode*, Node*)` (cpp:1221-1267)
            // and `visit(OptionalMemberExpressionNode*, Node*)`
            // (cpp:1269-1309), the private-name restriction checks — see
            // `expressions::visit_member_like_expression` (S2 T5), which
            // took both kinds out of the override-free generic arm below.
            Node::PrivateName(_) => self.visit_private_name(gc, node),
            Node::ClassPrivateProperty(_) => {
                self.visit_class_private_property(gc, node)
            }
            Node::StaticBlock(_) => self.visit_static_block(gc, node),
            Node::MemberExpression(_) | Node::OptionalMemberExpression(_) => {
                self.visit_member_like_expression(gc, node, path)
            }
            // `visit(CallExpressionNode*)` (cpp:1127-1219) — the direct-`eval`
            // detection, rewrite #3 (`$SHBuiltin.prop(...)` → `SHBuiltin`) and
            // the `super()` check; see `calls::visit_call_expression` (S2 T6),
            // whose module doc also records why `OptionalCallExpression` and
            // `NewExpression` are NOT routed here.
            Node::CallExpression(_) => self.visit_call_expression(gc, node),
            // Kinds with no `visit()` overload in C++: the generic dispatch
            // visits their children (`ExpressionStatement`'s `_expression`
            // — `_directive` is a NodeString, not a node) and here also
            // rebuilds the node if any child was replaced. The literals and
            // `EmptyStatement` have no children at all, so this is exactly
            // `TransformResult::Unchanged` for them.
            //
            // `MemberExpression`/`OptionalMemberExpression` were served by
            // this arm until S2 T5: their C++ override (cpp:1221-1309) only
            // validates a `PrivateNameNode` `_property`, so before private
            // names existed here it reduced to `visitESTreeChildren(*this,
            // node)`. S2 T5 ported it for real — see
            // `expressions::visit_member_like_expression`.
            // `Property`/`ObjectExpression` have no override at all.
            // `VariableDeclarator`/`RestElement`/`AssignmentPattern`/`Empty`
            // (S1 T5, destructuring declarations) likewise have no C++
            // override — see SemanticResolver.cpp's `visit(...)` list, none
            // of which names any of these kinds.
            //
            // `ObjectPattern`/`ArrayPattern` used to be served by this arm,
            // because their C++ overrides reduce to it for patterns with no
            // type annotation — the only ones this port could produce before
            // untyped `-parse-flow` existed. The capstone review found that
            // restriction had stopped being self-enforcing (annotated
            // patterns are reachable and were panicking where hermesc
            // resolves), so both now have their own arms below.
            //
            // S1 T6 adds the remaining override-free *expression* kinds,
            // each checked against the `SemanticResolver::visit` inventory
            // in SemanticResolver.h:200-304: `ArrayExpression`,
            // `ConditionalExpression`, `LogicalExpression`,
            // `SequenceExpression`, `TemplateLiteral` and `TemplateElement`
            // appear nowhere in it, so C++ reaches them through
            // `visitESTreeChildren` exactly like this arm does. (Neighbors
            // that DO have an override — `CallExpression` cpp:1127,
            // `SpreadElement` cpp:1469, `RegExpLiteral` cpp:821,
            // `Super` cpp:1096, `MetaProperty` cpp:837,
            // `YieldExpression`/`AwaitExpression` cpp:1490/1508 — are
            // deliberately left panicking until their own task ports them.)
            //
            // S2 T1 adds `SwitchCase`, the only child kind its statement
            // visits reach that has no override of its own: it appears
            // nowhere in the SemanticResolver.h:200-304 `visit` inventory,
            // so C++ walks its `_test`/`_consequent` through
            // `visitESTreeChildren` exactly like this arm does.
            //
            // S2 T2 adds `NewExpression` on the same grounds — it appears
            // nowhere in that inventory either (only `CallExpression` and the
            // two `MemberExpressionLike` kinds do), so C++ reaches its
            // `_callee`/`_arguments` through `visitESTreeChildren`. It is
            // here because it is one of the five parents
            // `visit(SpreadElementNode *)` whitelists (cpp:1474) and the only
            // one of them the corpus could reach before S2 T6. Its Flow-only
            // `_typeArguments` child is self-enforcing in exactly the way
            // `ObjectPattern`'s `_typeAnnotation` is, above.
            //
            // S2 T6 adds `OptionalCallExpression` — the sibling of the ONE
            // call-family kind that does have an override (`CallExpression`,
            // cpp:1127, now `calls::visit_call_expression`). It is a sibling,
            // not a subclass: ESTree.def:304-319 makes both children of the
            // `CallExpressionLike` GROUP, so `visit(CallExpressionNode *)` is
            // not viable for it and C++ picks the catch-all `visit(Node *)`
            // (SemanticResolver.h:191-193). See `calls.rs`'s module doc for
            // what that means observably (`eval?.()` warns about nothing,
            // `$SHBuiltin.foo?.(1)` is not rewritten). Same task adds
            // `SHBuiltin`, the node rewrite #3 CREATES: it is an
            // `ESTREE_NODE_0_ARGS` kind (ESTree.def:1505) with no override,
            // so this arm is exactly `Unchanged` for it — but it must be
            // present, because the rewritten callee's children walk reaches
            // it. And `IfStatement`, which the static-block corpus needs: it
            // appears nowhere in the SemanticResolver.h:200-304 inventory, so
            // C++ walks its `_test`/`_consequent`/`_alternate` through
            // `visitESTreeChildren`, and `DeclCollector` has no override for
            // it either, so it creates no scope. (Its only mentions anywhere
            // in `lib/Sema/` are `CheckImplicitReturn.cpp:96-97` and the
            // `FlowChecker` — two later/typed passes, neither of them this
            // resolver.)
            //
            // S2 T3 adds `ThrowStatement`, which `throw`-inside-`try` corpus
            // files need: `ThrowStatement` appears nowhere in
            // `lib/Sema/` at all (only `CheckImplicitReturn.cpp:155`
            // mentions it), so it has no `SemanticResolver::visit` override
            // and C++ reaches its one child (`_argument`, non-null,
            // ESTree.def:187) through `visitESTreeChildren`, exactly like
            // this arm. `DeclCollector` has no override for it either, so it
            // creates no scope.
            // S2 T4 adds `ClassBody`, the one child kind its class visits
            // reach that has no override of its own: it appears nowhere in
            // the SemanticResolver.h:200-304 `visit` inventory, so C++ walks
            // its `_body` list through `visitESTreeChildren` exactly like
            // this arm does — which is how each `ClassProperty`/
            // `MethodDefinition`/`StaticBlock` element gets dispatched. It
            // also adds `ThisExpression`, which class corpus files need
            // everywhere (`this.x` in a method or a field initializer):
            // `ThisExpression` appears nowhere in `lib/Sema/` outside the
            // FlowChecker (`FlowChecker-expr.cpp:498`), so it has no
            // `SemanticResolver::visit` override, and it is an
            // `ESTREE_NODE_0_ARGS` kind (ESTree.def:274) — no children at
            // all, so this arm is exactly `Unchanged` for it.
            //
            // S2 T7 adds `DebuggerStatement`, which the tests for
            // `check_implicit_return` need in order to reach its
            // `DebuggerStatement` arm (CheckImplicitReturn.cpp:175). Like
            // `ThrowStatement`, its only mention anywhere in `lib/Sema/` is
            // that arm — so no `SemanticResolver::visit` override, no
            // `DeclCollector` override, and it is an `ESTREE_NODE_0_ARGS`
            // kind (ESTree.def:171), i.e. this arm is exactly `Unchanged`
            // for it.
            // S2 T8's corpus sweep adds `BigIntLiteral` (ESTree.def:270-272),
            // `TaggedTemplateExpression` (:483-485) and `ImportExpression`
            // (:299-302). All three are plain untyped-JS constructs
            // `hermesc -dump-sema` resolves and dumps happily while this arm
            // was panicking on them; the sweep found them by running
            // `sema-dump` over `test/Parser`, `test/IRGen`, `test/BCGen` and
            // `test/Optimizer`. None appears anywhere in `lib/Sema/` outside
            // the FlowChecker (`FlowChecker-expr.cpp:1244` handles
            // `BigIntLiteral` as a *typed* expression), so none has a
            // `SemanticResolver::visit` override in the
            // SemanticResolver.h:200-304 inventory and none has a
            // `DeclCollector` override: C++ reaches `_tag`/`_quasi`,
            // `_source`/`_options` and (for the `NodeLabel`-only
            // `BigIntLiteral`) nothing at all through `visitESTreeChildren`,
            // creating no scope — exactly this arm. Pinned by
            // `expr-visit-generic-2.js`, which also pins that BigInt operands
            // are NOT folded (`ASTEval`'s folds only accept `NumericLiteral`).
            //
            // S4a T3 adds the SIX module-specifier kinds the four module
            // visits below reach as children — `ImportSpecifier`,
            // `ImportDefaultSpecifier`, `ImportNamespaceSpecifier`,
            // `ImportAttribute` (ESTree.def:597-611),
            // `ExportSpecifier` and `ExportNamespaceSpecifier`
            // (ESTree.def:625-632). Same argument as `ClassBody`'s above:
            // none of the six appears in the SemanticResolver.h:200-304
            // `visit` inventory (which names ONLY `ImportDeclarationNode`
            // and the three `Export*DeclarationNode`s, at :256 and
            // :280-282), so C++ reaches each one's children —
            // `_imported`/`_local`, `_local`, `_local`, `_key`/`_value`,
            // `_exported`/`_local`, `_exported` — through
            // `visitESTreeChildren`, exactly like this arm. Neither does
            // any of the six have a `DeclCollector` override
            // (DeclCollector.h:81-99 names only `ImportDeclarationNode` of
            // the ten module kinds), so none creates a scope. This is
            // observable in the parser-entry corpus:
            // `module-imports.js`'s dump resolves an `ImportSpecifier`'s
            // `_imported` as an ordinary identifier (`Id 'a'
            // [D:E:%d.4 'a']`, an `UndeclaredGlobalProperty`) alongside its
            // `_local`'s `Import` decl — precisely because the children walk
            // reaches both.
            Node::ExpressionStatement(_)
            | Node::ImportSpecifier(_)
            | Node::ImportDefaultSpecifier(_)
            | Node::ImportNamespaceSpecifier(_)
            | Node::ImportAttribute(_)
            | Node::ExportSpecifier(_)
            | Node::ExportNamespaceSpecifier(_)
            | Node::DebuggerStatement(_)
            | Node::BigIntLiteral(_)
            | Node::TaggedTemplateExpression(_)
            | Node::ImportExpression(_)
            | Node::ClassBody(_)
            | Node::ThisExpression(_)
            | Node::ThrowStatement(_)
            | Node::EmptyStatement(_)
            | Node::NumericLiteral(_)
            | Node::StringLiteral(_)
            | Node::BooleanLiteral(_)
            | Node::NullLiteral(_)
            | Node::Property(_)
            | Node::ObjectExpression(_)
            | Node::VariableDeclarator(_)
            | Node::RestElement(_)
            | Node::AssignmentPattern(_)
            | Node::ArrayExpression(_)
            | Node::ConditionalExpression(_)
            | Node::LogicalExpression(_)
            | Node::SequenceExpression(_)
            | Node::TemplateLiteral(_)
            | Node::TemplateElement(_)
            | Node::SwitchCase(_)
            | Node::NewExpression(_)
            | Node::OptionalCallExpression(_)
            | Node::SHBuiltin(_)
            | Node::IfStatement(_)
            | Node::Empty(_) => node.visit_children_mut(gc, self),
            // The two pattern overrides, SemanticResolver.h:209-214 — inline
            // header one-liners that visit ONLY `_properties`/`_elements`
            // and deliberately SKIP `_typeAnnotation`. They served the
            // generic arm above until the capstone review found annotated
            // destructuring (`var {a}: Obj = ...`) reachable under untyped
            // `-parse-flow`; see `declarations.rs`'s two visits.
            Node::ObjectPattern(_) => self.visit_object_pattern(gc, node),
            Node::ArrayPattern(_) => self.visit_array_pattern(gc, node),
            // The three Flow do-nothing visits, all `#if HERMES_PARSE_FLOW`:
            // `visit(TypeAliasNode *)` (SemanticResolver.cpp:1593-1595),
            // `visit(TypeParameterDeclarationNode *)` (cpp:1597-1599) and
            // `visit(TypeParameterInstantiationNode *)` (cpp:1601-1603).
            // Each has an empty body — a TRUE no-op, unlike the generic arm
            // above: it does NOT call `visitESTreeChildren`, so the
            // children are never visited and never get `[D:E:...]`
            // resolution annotations in the dump. For `TypeAlias` that is
            // `type-alias-children.js`'s whole point ("children of type
            // alias AST node are not resolved as variables").
            //
            // `TypeParameterInstantiation` is the type-argument list of a
            // call/`new`/optional call (`f<number>(1)`, `new C<number>()`,
            // `f?.<number>(1)`) — reachable under untyped `-parse-flow`
            // through those three nodes' children walks, and one of the ten
            // shapes the capstone review (finding F1) found panicking here
            // where hermesc resolves. Pinned by
            // `sema_corpus/flow-type-args.js`.
            // `TypeParameterDeclaration` is the *declaration* list
            // (`function f<T>(){}`, `interface J<T>{}`, `declare class
            // DC<V>{}`, `opaque type OT<W> = …`). The function/class visits
            // hand-drive their children and never dispatch it — but the
            // Flow-STATEMENT parents (`InterfaceDeclaration`, `DeclareClass`,
            // `OpaqueType`, …) reach it through their own `typeParameters`
            // field via the override-free range arm further down, so it IS
            // reachable in practice, just not from every parent. And it is
            // load-bearing, not merely carried for completeness: because the
            // visit is a TRUE no-op, a type parameter's `bound`/`default`
            // never gets resolved even though the enclosing body does —
            // pinned by `sema_corpus/flow-interface-enum.js`'s `interface
            // J<T: typeof host, U = typeof host> { b: typeof host }`.
            Node::TypeAlias(_)
            | Node::TypeParameterDeclaration(_)
            | Node::TypeParameterInstantiation(_) => TransformResult::Unchanged,
            // The four ES-module declaration visits (S4a T3):
            // `visit(ImportDeclarationNode *)` (cpp:874-890),
            // `visit(ExportNamedDeclarationNode *)` (cpp:1524-1531),
            // `visit(ExportDefaultDeclarationNode *)` (cpp:1533-1561, which
            // carries rewrite #4) and `visit(ExportAllDeclarationNode *)`
            // (cpp:1563-1568) — see `modules.rs`, including the two
            // bug-for-bug quirks it preserves (the `ExportAll` message
            // wording and rewrite #4's `/* async */ false`) and the
            // `FunctionInfo::imports` backref fixup. The `$SHBuiltin`
            // CommonJS-module protocol (`calls.rs`'s three phase-tagged
            // panics) is deliberately NOT part of this: it is S4b, together
            // with `-commonjs` itself.
            Node::ImportDeclaration(_) => {
                self.visit_import_declaration(gc, node)
            }
            Node::ExportNamedDeclaration(_) => {
                self.visit_export_named_declaration(gc, node)
            }
            Node::ExportDefaultDeclaration(_) => {
                self.visit_export_default_declaration(gc, node)
            }
            Node::ExportAllDeclaration(_) => {
                self.visit_export_all_declaration(gc, node)
            }
            // The rest of the Flow node range: `visit(ESTree::Node *node) {
            // visitESTreeChildren(*this, node); }` (SemanticResolver.h:
            // 191-193), C++'s default for every kind with no `visit()`
            // overload of its own — same mechanism as the override-free
            // generic arm far above, expressed as a range test because the
            // range is where the argument is uniform.
            //
            // The whole `#if HERMES_PARSE_FLOW` block of the header's
            // inventory (SemanticResolver.h:289-298) names exactly eight
            // Flow kinds, and only FIVE of them are inside the AST's `Flow`
            // range (ESTree.def:854-1272, the `ESTREE_FIRST(Flow, Base)`/
            // `ESTREE_LAST(Flow)` markers inside the `#if HERMES_PARSE_FLOW`
            // block at :852-1274; `NodeKind::_Flow_First
            // .._Flow_Last`): `TypeAlias`, `TypeParameterDeclaration`,
            // `TypeParameterInstantiation`, `TypeCastExpression` and
            // `AsExpression` — all five have their own arms ABOVE this one,
            // so they never reach it. The other three are outside the
            // range: `CoverTypedIdentifier` is a `Cover*` node (handled by
            // `visit_cover_node`), and `ComponentDeclaration`/
            // `HookDeclaration` are function-like statement nodes that need
            // `visitFunctionLike` — they keep falling through to the panic
            // below, which is correct: they require
            // `-parse-component-syntax` and are a dialect phase.
            //
            // Everything else in the range — the type-annotation universe
            // (`TypeAnnotation`, `GenericTypeAnnotation`,
            // `ObjectTypeAnnotation`, …), the Flow statement kinds
            // (`InterfaceDeclaration`, `OpaqueType`, `EnumDeclaration` and
            // its bodies/members, the whole `Declare*` family) and the
            // component/record kinds — appears NOWHERE in the header's
            // `visit` inventory, so C++ reaches each one's children through
            // `visitESTreeChildren`, exactly like this arm. That is
            // observable rather than assumed: hermesc's dump for
            // `interface I { x: number }` resolves the interface's own `Id
            // 'I'` (and the property keys inside its body) as ordinary
            // `UndeclaredGlobalProperty` identifiers, which only the
            // children walk can produce.
            //
            // The capstone review (finding F1) found the four Flow
            // statement kinds panicking here where hermesc resolves them;
            // `sema_corpus/flow-interface-enum.js` and
            // `flow-declare-opaque.js` pin them. The arm is written as the
            // range rather than as those four kinds for the same reason
            // C++ writes a default: the rule is "no override ⇒ walk the
            // children", and enumerating ~90 kinds would only invite the
            // next gap of this exact shape.
            //
            // `DeclCollector` needs no counterpart: it already has the two
            // Flow arms C++ gives it (`TypeAlias`, `InterfaceDeclaration`
            // — DeclCollector.h:95-99, `decl_collector.rs:445-447`), and
            // every other Flow kind is likewise a plain children walk there.
            n if n.is_flow() => node.visit_children_mut(gc, self),
            _ => panic!(
                "sema: unhandled node kind {} (S3+/dialect phases)",
                node.node_type_str()
            ),
        }
    }

    /// Port of `SemanticResolver::visit(ESTree::ProgramNode *node)`
    /// (cpp:193-231).
    ///
    /// `node` is the enclosing `Node` (rather than the `Program` payload)
    /// because the ported helpers all take `&Node`.
    fn visit_program<'gc>(
        &mut self,
        gc: &'gc GCLock,
        node: &'gc Node<'gc>,
    ) -> TransformResult<&'gc Node<'gc>> {
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

        let result = {
            let scope_state =
                self.enter_scope(Some(node), /* functionScope */ true);
            // C++ wraps this assignment in `llvh::SaveAndRestore<...>
            // saveGlobalScope(globalScope_, ...)`
            // (SemanticResolver.cpp:216-217)
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

            self.process_collected_declarations(gc, node);
            if !self.sem_ctx.function(self.cur_function_info()).strict {
                // Promote hoisted functions.
                //
                // S5: the third C++ call site is `runInScope`
                // (SemanticResolver.cpp:158), the lazy/`eval` entry point —
                // it promotes BEFORE `processCollectedDeclarations`, not
                // after. It arrives with `resolve_ast_in_scope`; do not
                // port it here.
                let promoted =
                    get_promoted_scoped_func_decls(self, gc, node);
                self.process_promoted_func_decls(gc, &promoted);
            }
            self.process_ambient_decls(gc);
            // visitESTreeChildren(*this, node): a Program's only child list
            // is `_body`. The generated `visit_children_mut` walks it and
            // rebuilds `node` if (and only if) an element was replaced —
            // still inside the scope, exactly where C++ visits the children.
            // The rebuild copies `node`'s decorations, which is why all of
            // them are written above, before this line (see the module
            // doc's "decorate before recursing").
            let result = node.visit_children_mut(gc, self);
            self.exit_scope(scope_state);
            result
        };
        self.exit_function(func_state);
        result
    }

    /// Port of `SemanticResolver::processCollectedDeclarations`
    /// (cpp:2102-2107).
    ///
    /// Clones the looked-up `ScopeDecls` (a `Vec<NodeRc>` — cheap refcount
    /// bumps, not a deep copy) before calling `process_declarations`: that
    /// call needs `&mut self` for every declaration it processes, which a
    /// live borrow of `self.function_context().decls` (where the
    /// `ScopeDecls` lives) would forbid. `scope_decls_for_node` only ever
    /// returns non-empty lists (see `DeclCollector::close_scope`), so the
    /// `Some` branch below is never called with an empty slice.
    fn process_collected_declarations(
        &mut self,
        gc: &GCLock,
        scope_node: &Node,
    ) {
        let decls: Option<Vec<NodeRc>> = self
            .function_context()
            .decls
            .as_ref()
            .expect("FunctionContext without a DeclCollector")
            .scope_decls_for_node(scope_node.node_id())
            .cloned();
        if let Some(decls) = decls {
            self.process_declarations(gc, &decls);
        }
    }

    /// Declare all the function declarations in `promoted_func_decls` with
    /// Var in function scope or GlobalProperty in global scope. Add the
    /// names to the function context's `promoted_func_decls` list. Port of
    /// `SemanticResolver::processPromotedFuncDecls`
    /// (SemanticResolver.h:428-432, cpp:2143-2155).
    ///
    /// Takes the `NodeRc`s `get_promoted_scoped_func_decls` returns rather
    /// than `&Node`s, for the reason `promoter.rs`'s module doc gives, and
    /// for the same reason `process_declarations` takes a `&[NodeRc]`.
    ///
    /// NOTE: this runs AFTER `visit_program`/
    /// `visit_function_body_after_params_visited` have written their node
    /// decorations but BEFORE the children are visited, and it writes no
    /// node `Cell` at all: everything it mutates lives in `SemContext`
    /// (`Decl`s, the binding table, `promoted_function_decls`) or in the
    /// `FunctionContext`, none of which a node rebuild ever snapshots or
    /// copies. It is therefore exempt from this module's "decorate before
    /// recursing" invariant, which constrains only `Cell` writes on AST
    /// nodes — same exemption, same reason, as `classes.rs`'s `decl_mut`
    /// site (classes.rs:990-997).
    fn process_promoted_func_decls(
        &mut self,
        gc: &GCLock,
        promoted_func_decls: &[NodeRc],
    ) {
        // Use GlobalProperty in global scope.
        let kind = if self.in_global_scope_context() {
            DeclKind::GlobalProperty
        } else {
            DeclKind::Var
        };
        for func_decl_rc in promoted_func_decls {
            let func_decl_node = func_decl_rc.node(gc);
            let func_decl = match func_decl_node {
                Node::FunctionDeclaration(fd) => fd,
                _ => panic!(
                    "cast<FunctionDeclarationNode> failed: promoted decl is \
                     a {}",
                    func_decl_node.node_type_str()
                ),
            };
            let ident_node = func_decl.id.expect(
                "cast<IdentifierNode>(funcDecl->_id) on a nameless promoted \
                 function declaration",
            );
            self.validate_and_declare_identifier(gc, kind, ident_node);
            let identifier = ident_node
                .as_identifier()
                .expect("a promoted function's id is an Identifier");
            // C++ stores `semCtx_.getDeclarationDecl(ident)` even when it is
            // null; `promoted_func_decls` is a `HashMap<Atom, DeclId>` with
            // no way to express that, so the null is an `expect` instead.
            // Unreachable: `validateAndDeclareIdentifier` has three early
            // returns that could leave the decl unset, none reachable here.
            // (1) A `validateDeclarationName` failure (all three of its
            // rules are strict-mode-only or Let/Const-only, and `kind` here
            // is Var/GlobalProperty inside a `!strict` guard). (2) The
            // "already declared" error, which needs a let-like declaration
            // of the same name visible in this function — exactly what the
            // promoter refuses to promote past. (3) The "two declarations
            // put" path (cpp:2633-2639), whose guard requires
            // `semCtx_.getDeclarationDecl(ident)` to already be non-null —
            // impossible here, since this is the promoted function's own
            // identifier node, being declared for the first time.
            let decl = self
                .sem_ctx
                .get_declaration_decl(identifier)
                .expect("a promoted function declaration always gets a decl");
            self.function_context_mut()
                .promoted_func_decls
                .entry(identifier.name.get())
                .or_insert(decl);
        }
    }

    /// Scan the directive prologue of `body`. Port of
    /// `SemanticResolver::scanDirectives` (cpp:2778-2828).
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
    /// Port of `SemanticResolver::processAmbientDecls` (cpp:2860-2931).
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

    /// Port of the `declareAmbientGlobal` lambda (cpp:2911-2920).
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

/// Port of `RecursiveVisitorDispatch::visit`
/// (RecursiveVisitor.h:197-232) — the dispatcher entry every node the
/// resolver sees goes through, including the root. It brackets the kind
/// dispatch with the recursion-depth protocol and, when the depth budget is
/// spent, returns without visiting `node` at all (C++: `if
/// (LLVM_UNLIKELY(!v.incRecursionDepth(node))) return;`, which leaves the
/// node exactly as it was — hence `Unchanged`).
///
/// C++'s `afterCaller` (the optional `afterVisit` hook, RecursiveVisitor.h:
/// 230) has no counterpart: `SemanticResolver` defines no `afterVisit`.
impl<'gc> VisitorMut<'gc> for SemanticResolver<'_, '_, '_, '_> {
    fn call(
        &mut self,
        gc: &'gc GCLock<'_, '_>,
        node: &'gc Node<'gc>,
        path: Option<Path<'gc>>,
    ) -> TransformResult<&'gc Node<'gc>> {
        if !self.inc_recursion_depth(node) {
            return TransformResult::Unchanged;
        }
        let result = self.visit_node(gc, node, path);
        self.dec_recursion_depth();
        result
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
/// `struct DeclHoisting` (cpp:2870-2909); its `enter`/`leave` are empty and
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
/// (SemanticResolver.cpp:2945-2946), i.e.
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

/// Port of `node->setSemInfo(semInfo)` (SemanticResolver.cpp:3005), i.e.
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

#[cfg(test)]
mod tests {
    use super::*;
    use ast::context::Context;
    use ast::node::EmptyStatement;
    use ast::node_child::NodeMetadata;
    use support::location::{SMLoc, SMRange};

    /// The `RecursionDepthTracker` protocol, exercised directly (the visits
    /// that drive it are covered end-to-end by `tests/resolver.rs`): exactly
    /// `kASTMaxRecursionDepth - 1` levels are allowed, the next one reports
    /// the error, and from then on every `incRecursionDepth` refuses —
    /// including after a `decRecursionDepth`, which must not resurrect a
    /// spent budget (RecursiveVisitor.h:721-738).
    #[test]
    fn recursion_depth_tracker_trips_once_at_the_nesting_limit() {
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let mut sem_ctx = SemContext::new(Keywords::new(&gc));
        let mut sm = SourceErrorManager::new();
        let buf = sm.add_buffer_bytes("depth.js", b"x");
        let loc = SMLoc {
            source: buf,
            offset: 0,
        };
        let node = gc.alloc(Node::EmptyStatement(EmptyStatement::new(
            NodeMetadata::new(SMRange {
                start: loc,
                end: loc,
            }),
        )));

        {
            let binding_table = sem_ctx.binding_table_rc();
            let mut resolver = SemanticResolver::new(
                &binding_table,
                &mut sem_ctx,
                &mut sm,
                &[],
                /* compile */ true,
            );
            for level in 1..AST_MAX_RECURSION_DEPTH {
                assert!(
                    resolver.inc_recursion_depth(node),
                    "nesting level {level} must be allowed"
                );
            }
            assert!(
                !resolver.inc_recursion_depth(node),
                "level {AST_MAX_RECURSION_DEPTH} must trip the limit"
            );
            assert!(!resolver.inc_recursion_depth(node));
            resolver.dec_recursion_depth();
            assert!(
                !resolver.inc_recursion_depth(node),
                "a spent budget must stay spent"
            );
        }
        // Reported once, no matter how many refused visits followed.
        assert_eq!(sm.error_count(), 1);
    }

    /// Pin for the `n if n.is_flow()` range arm above: the AST's Flow
    /// section (`ESTree.def:854-1272`, `ESTREE_FIRST(Flow, Base)`..
    /// `ESTREE_LAST(Flow)`) currently generates 97 `NodeKind` sentinels
    /// between `_Flow_First` and `_Flow_Last` — the same precedent as
    /// `keywords.rs`'s `count_is_133`. If a future `.def` change adds or
    /// removes a Flow kind, this trips loudly instead of silently changing
    /// which kinds fall into the range arm.
    #[test]
    fn flow_range_size_is_97() {
        use ast::node::NodeKind;
        let count =
            NodeKind::_Flow_Last as u32 - NodeKind::_Flow_First as u32 - 1;
        assert_eq!(count, 97);
    }
}
