/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Sema-informed identifier annotation: `crate::Annotation::Sem`'s payload.
//!
//! Ported from juno's `annotate_identifier` (`gen_js.rs:3966-3994`), called
//! right after an identifier's name is written (juno `gen_js.rs:1307`; this
//! crate's `arms/literal.rs::gen_identifier`). Task 4 left the call site
//! wired to a no-op stub, noting this task would fill it in; this is that
//! fill-in.
//!
//! # Deviation from juno's exact spelling
//!
//! juno resolves an identifier through a single
//! `SemContext::ident_decl(&NodeRc) -> Option<Resolution>` map and, for the
//! nine `DeclKind`s juno's own sema has (`Let`, `Const`, `Class`, `Import`,
//! `ES5Catch`, `FunctionExprName`, `ScopedFunction`, `Var`, `Parameter`),
//! collapses all of them to a bare `@D{decl_id}` — the *kind* is never
//! printed, only the numeric id. Only the two global kinds get their own
//! fixed marker text (`GlobalProperty` -> `@global`,
//! `UndeclaredGlobalProperty` -> `@uglobal`), and `Resolution::Unresolvable`
//! prints `@unresolvable`.
//!
//! Our `SemContext` (`hermes_sema::sem_context::SemContext`) has no
//! `Resolution`/`ident_decl`. Per the task brief,
//! `sema/examples/print_bindings.rs` (the maintained reference) is the
//! current way to resolve an identifier: the single juno lookup is split
//! into `SemContext::get_declaration_decl` (the identifier itself IS the
//! declared name), `SemContext::get_expression_decl` (the identifier is a
//! *use* of one — panics if called on an identifier marked unresolvable, per
//! its own doc comment), and a separate `unresolvable: Cell<bool>` field
//! living on the `Identifier` node. [`GenJS::annotate_identifier`] adopts
//! that split unchanged, in the same order `print_bindings.rs` uses:
//! declaration site, then unresolvable, then expression site, else print
//! nothing at all. That last "nothing" case is not a gap — it is the same
//! outcome juno's own `_ => {}` produces for a `None` map lookup, and it
//! covers `Identifier` nodes that name no binding to begin with: property
//! keys, member-access names, label names (`print_bindings.rs`'s own
//! "(not a variable reference)" case).
//!
//! The annotation *text* for a resolved declaration also deviates,
//! deliberately: this crate's `DeclKind` (`hermes_sema::sem_context::DeclKind`)
//! has sixteen variants — the five private-name kinds, `TypedBuiltin`,
//! `ClassExprName`, and a `Catch` alongside `ES5Catch` — none of which
//! juno's nine-variant enum carries, so juno's binary split ("these
//! known-by-name kinds print just a number; these two others print a fixed
//! word") does not generalize to it. Printing the kind's `Debug` spelling
//! (`sem_context.rs` derives `Debug` on `DeclKind`; it is the same string
//! `dump_context.rs`'s hand-written `decl_kind_str` spells out for the sema
//! dumper) generalizes to all sixteen for free, and is strictly more
//! informative to a human reading annotated output than an opaque `@D3` —
//! no need to keep a `SemContext` dump open in another window to know
//! whether `@D3` was a `Let` or a `Parameter`. The `@` marker, the decl id
//! alongside it, and the "right after the identifier's name, before the
//! `?`/type annotation" placement (juno `gen_js.rs:1307`;
//! `arms/literal.rs::gen_identifier`) all carry over unchanged.

use hermes_ast::context::GCLock;
use hermes_ast::node::Node;

#[cfg(feature = "annotate")]
use crate::out;
use crate::GenJS;

/// The no-op used when the `annotate` feature is off.
///
/// `Annotation::Sem` does not exist in that build (see [`crate::Annotation`]),
/// so there is never anything to print. Keeping the method — rather than
/// making `arms/literal.rs::gen_identifier`'s call site conditional — means
/// exactly one place in the crate knows the feature exists.
#[cfg(not(feature = "annotate"))]
impl GenJS<'_, '_> {
    /// Print nothing. See the module doc comment.
    pub(crate) fn annotate_identifier<'gc>(
        &mut self,
        _ctx: &GCLock<'_, '_>,
        _node: &'gc Node<'gc>,
    ) {
    }
}

#[cfg(feature = "annotate")]
impl GenJS<'_, '_> {
    /// Print `@<KindDebug>(D<id>)` (a resolved binding), `@unresolvable` (an
    /// identifier an enclosing `eval`/`with` keeps the resolver from
    /// committing to), or nothing at all (an identifier that names no
    /// binding, or [`crate::Annotation::No`]) right after `node`'s name.
    ///
    /// juno `gen_js.rs:3966-3994`; see the module doc comment for how the
    /// annotation text differs from juno's and why. `ctx` is unused — unlike
    /// juno's `NodeRc::from_node(lock, node)`, resolution here reads
    /// `Cell`s stored directly on the `Identifier` node (the same fields
    /// `print_bindings.rs` reads) and needs no lock — but is kept in the
    /// signature to match the call site in `arms/literal.rs::gen_identifier`
    /// juno `gen_js.rs:1307` shares.
    pub(crate) fn annotate_identifier<'gc>(&mut self, _ctx: &GCLock<'_, '_>, node: &'gc Node<'gc>) {
        let Some(sem) = self.sem_context() else {
            // `Annotation::No`.
            return;
        };
        let Node::Identifier(id) = node else {
            // `gen_identifier` (the only caller) always passes the
            // `Identifier` node whose arm it is generating.
            return;
        };
        if let Some(decl_id) = sem.get_declaration_decl(id) {
            let kind = sem.decl(decl_id).kind;
            out!(self, "@{:?}(D{})", kind, decl_id.index());
        } else if id.unresolvable.get() {
            out!(self, "@unresolvable");
        } else if let Some(decl_id) = sem.get_expression_decl(id) {
            let kind = sem.decl(decl_id).kind;
            out!(self, "@{:?}(D{})", kind, decl_id.index());
        }
        // Else: not a variable reference at all — nothing to print.
    }
}

#[cfg(all(test, feature = "annotate"))]
mod tests {
    use hermes_ast::context::GCLock;
    use hermes_ast::node::Node;
    use hermes_sema::{resolve_for_compile, CompileOptions};

    use crate::{generate, Annotation, Opt};

    /// Parse+resolve `src`, then generate it once with `Annotation::Sem` and
    /// once with `Annotation::No` over the *same* resolved tree, returning
    /// both outputs.
    fn gen_both(src: &str) -> (String, String) {
        let parsed =
            hermes_parser::parse(src, hermes_parser::ParseFlags::default()).expect("parse");
        let mut resolved =
            resolve_for_compile(parsed, &CompileOptions::default()).expect("resolve");
        resolved.with_program(|gc: &GCLock<'static, '_>, root: &Node, sem| {
            let mut sem_out = Vec::new();
            generate(
                &mut sem_out,
                gc,
                root,
                Opt {
                    annotation: Annotation::Sem(sem),
                    ..Opt::new()
                },
            )
            .expect("generate with Annotation::Sem");
            let mut plain_out = Vec::new();
            generate(&mut plain_out, gc, root, Opt::new()).expect("generate with Annotation::No");
            (
                String::from_utf8(sem_out).expect("valid UTF-8"),
                String::from_utf8(plain_out).expect("valid UTF-8"),
            )
        })
    }

    /// Exercises all three annotation-relevant outcomes from one source: a
    /// `let` binding (declaration site), a parameter referenced inside the
    /// function it belongs to (expression site), and an undeclared global
    /// (`console`, also an expression site) — the same source
    /// `sema/examples/print_bindings.rs`'s doc comment traces through
    /// `Let`/`Parameter`/`UndeclaredGlobalProperty`. `Annotation::No` on the
    /// same resolved tree must carry none of that text, which is what proves
    /// the option is actually doing something rather than the substrings
    /// coincidentally appearing in ordinary source text.
    #[test]
    fn sem_annotation_labels_each_binding() {
        let src = "let counter = 0; function f(step) { console.log(counter, step); }";
        let (sem_js, plain_js) = gen_both(src);

        assert!(sem_js.contains("@Let("), "{sem_js}");
        assert!(sem_js.contains("@Parameter("), "{sem_js}");
        assert!(sem_js.contains("@UndeclaredGlobalProperty("), "{sem_js}");

        assert!(!plain_js.contains("@Let("), "{plain_js}");
        assert!(!plain_js.contains("@Parameter("), "{plain_js}");
        assert!(
            !plain_js.contains("@UndeclaredGlobalProperty("),
            "{plain_js}"
        );
        assert!(!plain_js.contains('@'), "{plain_js}");
    }

    /// [`GenJS::annotate_identifier`]'s "else: nothing to print" branch:
    /// `log` in `console.log` is a property key, an `Identifier` node that
    /// names no binding at all, so it must carry no `@` annotation even
    /// under `Annotation::Sem` — unlike `console` right next to it, which
    /// does resolve (to `UndeclaredGlobalProperty`). This is the named test
    /// that fails if that branch is deleted (making every `Identifier`
    /// print `@unresolvable` or similar instead of staying silent).
    #[test]
    fn non_binding_identifier_is_not_annotated() {
        let src = "console.log(1);";
        let (sem_js, _plain_js) = gen_both(src);
        assert!(
            sem_js.contains("console@UndeclaredGlobalProperty("),
            "{sem_js}"
        );
        assert!(!sem_js.contains("log@"), "{sem_js}");
    }
}
