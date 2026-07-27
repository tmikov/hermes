/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Port of `hermes::sema::SemContextDumper` (`lib/Sema/SemContext.cpp:415-563`,
//! declared `include/hermes/Sema/SemContext.h:694-756`). This is the
//! byte-exact text dumper the differential oracle depends on
//! (`hermesc -dump-sema` output), so every space and quote below is
//! transcribed straight from the C++ `<<` chain it replaces — do not
//! "clean up" the formatting.
//!
//! ## Deviations
//!
//! - The public entry points that print a name (`print_sem_context`,
//!   `print_decl_ref`) take an extra `gc: &GCLock` parameter that the C++
//!   signatures don't have. C++ resolves `d->name` (a `UniqueString*`)
//!   in-process; the Rust `Decl::name` is an interned [`Atom`] (`AtomBytes`),
//!   and its bytes only exist behind a [`GCLock`] (see `ast::context`), so
//!   the dumper has to be handed one. `print_scope_ref` doesn't print a
//!   name (see `printScopeRef`, cpp:530-534) and so needs no `gc`.
//! - `printSemContext`/`printFunction` build a `std::map<const FunctionInfo*
//!   /* or LexicalScope* */, SmallVector<...>>` keyed by pointer and then
//!   iterate it (cpp:429-450, 466-489). A `std::map` over pointers into a
//!   `std::deque` iterates in address order, which for a deque equals
//!   allocation order (deques never move already-allocated elements). Our
//!   storages are index-order `Vec`s, i.e. already in allocation order, so
//!   building the children lists by a single `0..len` pass over the
//!   storage (see `print_sem_context`, `print_function`) and walking each
//!   list in push order reproduces the exact same node — and hence text —
//!   order as the C++ `std::map`, without needing a real ordered map.
//! - `processedCount` in C++ `printSemContext` (cpp:441, 447) is computed
//!   but never asserted against anything (unlike the one in `printFunction`,
//!   cpp:483-489, which is): it's dead. It's omitted here; the analogous
//!   count inside the `printFunction` port (`print_function`) IS checked,
//!   matching the C++ `assert`.

use std::collections::HashMap;

use ast::context::GCLock;
use ast::SemaId;
use atom_table::INVALID_ATOM_BYTES;

use crate::ids::{DeclId, FunctionInfoId, ScopeId};
use crate::sem_context::{Atom, DeclKind, DeclSpecial, SemContext};

/// Port of `PtrNumberingImpl`/`Numbering<T>` (SemContext.h:730-756,
/// cpp:565-570): assigns each distinct id an increasing number, starting
/// at 1, the first time it is printed. C++ keys this by pointer identity;
/// here the id itself (already a stable, unique identity — see
/// `crate::ids`) is the key.
struct Numbering<Id> {
    next_number: usize,
    numbers: HashMap<Id, usize>,
}

impl<Id: Copy + Eq + std::hash::Hash> Numbering<Id> {
    fn new() -> Self {
        Numbering {
            next_number: 1,
            numbers: HashMap::new(),
        }
    }

    /// Port of `PtrNumberingImpl::getNumberImpl` (cpp:565-570).
    fn get_number(&mut self, id: Id) -> usize {
        if let Some(&n) = self.numbers.get(&id) {
            return n;
        }
        let n = self.next_number;
        self.next_number += 1;
        self.numbers.insert(id, n);
        n
    }
}

/// Bundles the two pieces of read-only state threaded through every
/// recursive dump helper below (the `SemContext` storages, and the
/// `GCLock` needed to resolve atom text) into one `Copy` value, so the
/// helpers stay within a sane argument count instead of repeating both
/// references in every signature.
#[derive(Clone, Copy)]
struct Env<'e, 'ast, 'ctx> {
    gc: &'e GCLock<'ast, 'ctx>,
    sc: &'e SemContext,
}

/// Port of `AnnotateDeclFunc` (SemContext.h:696-697). C++ passes the
/// `Decl*` itself; here the stable `DeclId` is passed instead (the
/// callback can look the decl back up in the `SemContext` it already has
/// if it needs fields).
pub type AnnotateDeclFunc = Box<dyn Fn(&mut String, DeclId)>;

/// Port of `hermes::sema::SemContextDumper` (SemContext.h:694-756).
pub struct SemContextDumper {
    /// Optional callback printing a `Decl` annotation. Port of
    /// `annotateDecl_` (SemContext.h:728); the FlowChecker hook — ported
    /// now as shape only, exercised solely by a trivial unit test until the
    /// typed component lands.
    annotate_decl: Option<AnnotateDeclFunc>,
    /// Port of `declNumbers_` (SemContext.h:751).
    decl_numbers: Numbering<DeclId>,
    /// Port of `scopeNumbers_` (SemContext.h:752).
    scope_numbers: Numbering<ScopeId>,
}

impl Default for SemContextDumper {
    fn default() -> Self {
        Self::new()
    }
}

impl SemContextDumper {
    /// Port of the default constructor (SemContext.h:700).
    pub fn new() -> Self {
        SemContextDumper {
            annotate_decl: None,
            decl_numbers: Numbering::new(),
            scope_numbers: Numbering::new(),
        }
    }

    /// Port of the templated `explicit SemContextDumper(F f)` constructor
    /// (SemContext.h:701-702), which stores an `AnnotateDeclFunc` callback.
    pub fn new_annotated(f: AnnotateDeclFunc) -> Self {
        SemContextDumper {
            annotate_decl: Some(f),
            decl_numbers: Numbering::new(),
            scope_numbers: Numbering::new(),
        }
    }

    /// Print the whole `SemContext` starting from `root_func` if provided,
    /// otherwise from the first function in storage (id 0 — always the
    /// global function). Port of `printSemContext` (cpp:417-451).
    pub fn print_sem_context(
        &mut self,
        out: &mut String,
        gc: &GCLock,
        ctx: &SemContext,
        root_func: Option<FunctionInfoId>,
    ) {
        let root_func = root_func
            .unwrap_or_else(|| FunctionInfoId::from_sema_id(SemaId(0)));

        out.push_str("SemContext\n");

        // Bucket every function other than `root_func` under its parent —
        // see the module doc for why a plain index-order pass reproduces
        // the C++ `std::map<const FunctionInfo*, ...>` iteration order.
        let mut children: HashMap<Option<FunctionInfoId>, Vec<FunctionInfoId>> =
            HashMap::new();
        for i in 0..ctx.functions_len() {
            let fid = FunctionInfoId::from_sema_id(SemaId(i as u32));
            if fid == root_func {
                continue;
            }
            let parent = ctx.function(fid).parent_function;
            children.entry(parent).or_default().push(fid);
        }

        let env = Env { gc, sc: ctx };
        self.dump_function(out, env, &children, root_func, 0);
    }

    /// Recursive helper walking the function tree built by
    /// `print_sem_context`. Port of the `dumpFunction` lambda (cpp:438-449).
    fn dump_function(
        &mut self,
        out: &mut String,
        env: Env<'_, '_, '_>,
        children: &HashMap<Option<FunctionInfoId>, Vec<FunctionInfoId>>,
        f: FunctionInfoId,
        level: u32,
    ) {
        self.print_function(out, env, f, level);
        if let Some(kids) = children.get(&Some(f)) {
            for &child in kids {
                self.dump_function(out, env, children, child, level + 1);
            }
        }
    }

    /// Port of `printFunction` (cpp:453-475).
    fn print_function(
        &mut self,
        out: &mut String,
        env: Env<'_, '_, '_>,
        f: FunctionInfoId,
        level: u32,
    ) {
        let info = env.sc.function(f);
        push_indent(out, level);
        out.push_str(if info.is_static_block {
            "StaticBlock "
        } else {
            "Func "
        });
        out.push_str(if info.strict { "strict" } else { "loose" });
        out.push('\n');

        let scopes = info.get_scopes();
        debug_assert!(
            !scopes.is_empty(),
            "every FunctionInfo has at least one scope"
        );
        let first = scopes[0];

        // Same reasoning as the function-level children map in
        // `print_sem_context` above, applied to this function's own scopes
        // (`sc->parentScope`, cpp:463-465).
        let mut children: HashMap<Option<ScopeId>, Vec<ScopeId>> =
            HashMap::new();
        for &sc_id in &scopes[1..] {
            let parent = env.sc.scope(sc_id).parent_scope;
            children.entry(parent).or_default().push(sc_id);
        }

        let processed = self.dump_scope(out, env, &children, first, level + 1);
        debug_assert_eq!(
            processed,
            scopes.len(),
            "not all scopes were visited"
        );
    }

    /// Recursive helper walking the scope tree built by `print_function`.
    /// Port of the `dumpScope` lambda (cpp:467-478). \return the number of
    /// scopes visited (itself plus all descendants), mirroring the C++
    /// `processedCount` this port checks in `print_function`.
    fn dump_scope(
        &mut self,
        out: &mut String,
        env: Env<'_, '_, '_>,
        children: &HashMap<Option<ScopeId>, Vec<ScopeId>>,
        s: ScopeId,
        level: u32,
    ) -> usize {
        self.print_scope(out, env, s, level);
        let mut processed = 1;
        if let Some(kids) = children.get(&Some(s)) {
            for &child in kids {
                processed +=
                    self.dump_scope(out, env, children, child, level + 1);
            }
        }
        processed
    }

    /// Port of `printScope` (cpp:492-506).
    fn print_scope(
        &mut self,
        out: &mut String,
        env: Env<'_, '_, '_>,
        s: ScopeId,
        level: u32,
    ) {
        push_indent(out, level);
        out.push_str("Scope %s.");
        let n = self.scope_numbers.get_number(s);
        out.push_str(&n.to_string());
        out.push('\n');

        let scope = env.sc.scope(s);
        for &d in &scope.decls {
            push_indent(out, level + 1);
            self.print_decl(out, env, d);
            out.push('\n');
        }
        for fd in &scope.hoisted_functions {
            push_indent(out, level + 1);
            out.push_str("hoistedFunction ");
            // C++: `cast<IdentifierNode>(fd->_id)->_name->str()` — an
            // unconditional cast, since a hoisted function always has a
            // name. Faithfully unwrap rather than silently skip.
            let node = fd.node(env.gc);
            let func_decl = node.as_function_declaration().expect(
                "SemContext::hoistedFunctions entries are always \
                 FunctionDeclaration nodes",
            );
            let id_node = func_decl
                .id
                .expect("a hoisted FunctionDeclaration always has an id");
            let ident = id_node
                .as_identifier()
                .expect("FunctionDeclaration.id is always an Identifier");
            push_atom(out, env.gc, ident.name.get());
            out.push('\n');
        }
    }

    /// Port of `printScopeRef` (cpp:508-512). Unlike `print_scope`, this
    /// never prints a name, so it needs no `GCLock`.
    pub fn print_scope_ref(&mut self, out: &mut String, s: ScopeId) {
        out.push_str("Scope %s.");
        out.push_str(&self.scope_numbers.get_number(s).to_string());
    }

    /// Port of `printDecl` (cpp:514-554), including the CASE-macro switches
    /// for `Decl::Kind` and `Decl::Special` (cpp:519-543, 546-554).
    fn print_decl(
        &mut self,
        out: &mut String,
        env: Env<'_, '_, '_>,
        d: DeclId,
    ) {
        out.push_str("Decl %d.");
        let n = self.decl_numbers.get_number(d);
        out.push_str(&n.to_string());
        out.push_str(" '");
        let decl = env.sc.decl(d);
        push_atom(out, env.gc, decl.name);
        out.push_str("' ");
        out.push_str(decl_kind_str(decl.kind));

        if decl.special != DeclSpecial::NotSpecial {
            out.push(' ');
            out.push_str(decl_special_str(decl.special));
        }

        if let Some(annotate) = &self.annotate_decl {
            annotate(out, d);
        }
    }

    /// Port of `printDeclRef` (cpp:556-562).
    pub fn print_decl_ref(
        &mut self,
        out: &mut String,
        gc: &GCLock,
        ctx: &SemContext,
        d: DeclId,
        print_name: bool,
    ) {
        out.push_str("%d.");
        out.push_str(&self.decl_numbers.get_number(d).to_string());
        if print_name {
            let decl = ctx.decl(d);
            if decl.name != INVALID_ATOM_BYTES {
                out.push_str(" '");
                push_atom(out, gc, decl.name);
                out.push('\'');
            }
        }
    }
}

/// Port of `ind(level)` (cpp:15-17): `level * 4` spaces.
fn push_indent(out: &mut String, level: u32) {
    for _ in 0..level * 4 {
        out.push(' ');
    }
}

/// Push an atom's text into `out`, matching what the C++ dumper does when
/// it writes `d->name`/`_name->str()` straight to a `raw_ostream`: the raw
/// bytes, unescaped, verbatim.
///
/// Identifier text is WTF-8: `\uD800`-style escapes can produce lone
/// surrogate code points, whose 3-byte CESU-8-like encoding is NOT valid
/// UTF-8 (a `char`/`str` can't hold a surrogate code point). `String` must
/// hold valid UTF-8, so those bytes can't be pushed as-is; a
/// `String::from_utf8_lossy` fallback would silently replace them with
/// U+FFFD, producing output that looks plausible but is byte-wise wrong
/// versus the C++ oracle it's meant to match exactly. S0's tested names
/// are all plain ASCII (always valid UTF-8), so instead of silently
/// mangling we decode strictly and panic loudly on the lone-surrogate
/// case; a real WTF-8-safe renderer is deferred to whichever later task
/// first needs to dump such a name.
fn push_atom(out: &mut String, gc: &GCLock, atom: Atom) {
    let bytes = gc.bytes(atom);
    let s = std::str::from_utf8(bytes).expect(
        "atom text is not valid UTF-8 (WTF-8 lone surrogate?); not \
         supported by SemContextDumper in S0",
    );
    out.push_str(s);
}

/// The `Decl::Kind` → string table. Port of the `CASE` macro switch in
/// `printDecl` (cpp:519-543) — match-arm order here doesn't need to mirror
/// the C++ switch or the enum declaration order (a Rust `match` is checked
/// for exhaustiveness by the compiler either way); each arm just needs the
/// same string the C++ `#x` stringification produces.
fn decl_kind_str(kind: DeclKind) -> &'static str {
    match kind {
        DeclKind::Let => "Let",
        DeclKind::Const => "Const",
        DeclKind::Class => "Class",
        DeclKind::Catch => "Catch",
        DeclKind::Import => "Import",
        DeclKind::ES5Catch => "ES5Catch",
        DeclKind::FunctionExprName => "FunctionExprName",
        DeclKind::ClassExprName => "ClassExprName",
        DeclKind::TypedBuiltin => "TypedBuiltin",
        DeclKind::ScopedFunction => "ScopedFunction",
        DeclKind::Var => "Var",
        DeclKind::Parameter => "Parameter",
        DeclKind::GlobalProperty => "GlobalProperty",
        DeclKind::UndeclaredGlobalProperty => "UndeclaredGlobalProperty",
        DeclKind::PrivateField => "PrivateField",
        DeclKind::PrivateMethod => "PrivateMethod",
        DeclKind::PrivateGetter => "PrivateGetter",
        DeclKind::PrivateSetter => "PrivateSetter",
        DeclKind::PrivateGetterSetter => "PrivateGetterSetter",
    }
}

/// The `Decl::Special` → string table. Port of the `CASE` macro switch in
/// `printDecl` (cpp:546-553). Callers only invoke this once `special !=
/// NotSpecial` has been checked (cpp:544), same as here.
fn decl_special_str(special: DeclSpecial) -> &'static str {
    match special {
        DeclSpecial::NotSpecial => {
            debug_assert!(false, "callers must filter out NotSpecial");
            "NotSpecial"
        }
        DeclSpecial::Arguments => "Arguments",
        DeclSpecial::Eval => "Eval",
        DeclSpecial::PrivateStatic => "PrivateStatic",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ast::context::Context;
    use crate::keywords::Keywords;
    use crate::sem_context::{ConstructorKind, FuncIsArrow};

    /// Trivial exercise of the `new_annotated` hook (the FlowChecker shape
    /// — unused for real until the typed component lands): confirms the
    /// callback runs and can append text after a `Decl`'s normal line.
    #[test]
    fn new_annotated_hook_runs_after_the_decl_line() {
        let mut ctx = Context::new();
        let gc = GCLock::new(&mut ctx);
        let mut sc = SemContext::new(Keywords::new(&gc));

        let f = sc.new_function(
            FuncIsArrow::No,
            ConstructorKind::None,
            None,
            None,
            false,
            Default::default(),
        );
        let s = sc.new_scope(f, None);
        sc.new_decl_in_scope(
            gc.atom_bytes("x"),
            DeclKind::Let,
            s,
            DeclSpecial::NotSpecial,
        );

        let annotate: AnnotateDeclFunc =
            Box::new(|out: &mut String, d: DeclId| {
                out.push_str(" /* annotated ");
                out.push_str(&d.index().to_string());
                out.push_str(" */");
            });
        let mut dumper = SemContextDumper::new_annotated(annotate);
        let mut out = String::new();
        dumper.print_sem_context(&mut out, &gc, &sc, None);

        let expected = "\
SemContext
Func loose
    Scope %s.1
        Decl %d.1 'x' Let /* annotated 0 */
";
        assert_eq!(out, expected);
    }

    /// `print_decl_ref` with `print_name = false` must omit the name even
    /// though the decl has one (cpp:558: `if (printName && ...)`).
    #[test]
    fn print_decl_ref_without_name_omits_the_quoted_name() {
        let mut ctx = Context::new();
        let gc = GCLock::new(&mut ctx);
        let mut sc = SemContext::new(Keywords::new(&gc));
        let f = sc.new_function(
            FuncIsArrow::No,
            ConstructorKind::None,
            None,
            None,
            false,
            Default::default(),
        );
        let s = sc.new_scope(f, None);
        let d = sc.new_decl_in_scope(
            gc.atom_bytes("x"),
            DeclKind::Let,
            s,
            DeclSpecial::NotSpecial,
        );

        let mut dumper = SemContextDumper::new();
        let mut out = String::new();
        dumper.print_decl_ref(&mut out, &gc, &sc, d, false);
        assert_eq!(out, "%d.1");

        let mut out2 = String::new();
        dumper.print_decl_ref(&mut out2, &gc, &sc, d, true);
        assert_eq!(out2, "%d.1 'x'");
    }

    /// `print_scope_ref` never prints a name and needs no `GCLock`.
    #[test]
    fn print_scope_ref_has_no_name() {
        let mut ctx = Context::new();
        let gc = GCLock::new(&mut ctx);
        let mut sc = SemContext::new(Keywords::new(&gc));
        let f = sc.new_function(
            FuncIsArrow::No,
            ConstructorKind::None,
            None,
            None,
            false,
            Default::default(),
        );
        let s = sc.new_scope(f, None);

        let mut dumper = SemContextDumper::new();
        let mut out = String::new();
        dumper.print_scope_ref(&mut out, s);
        assert_eq!(out, "Scope %s.1");
    }

    /// A `Decl` with a `special` must print the extra ` Special` suffix
    /// (cpp:544-554).
    #[test]
    fn decl_with_special_prints_the_special_suffix() {
        let mut ctx = Context::new();
        let gc = GCLock::new(&mut ctx);
        let mut sc = SemContext::new(Keywords::new(&gc));
        let f = sc.new_function(
            FuncIsArrow::No,
            ConstructorKind::None,
            None,
            None,
            false,
            Default::default(),
        );
        let s = sc.new_scope(f, None);
        sc.new_decl_in_scope(
            gc.atom_bytes("arguments"),
            DeclKind::Var,
            s,
            DeclSpecial::Arguments,
        );

        let mut dumper = SemContextDumper::new();
        let mut out = String::new();
        dumper.print_sem_context(&mut out, &gc, &sc, None);
        let expected = "\
SemContext
Func loose
    Scope %s.1
        Decl %d.1 'arguments' Var Arguments
";
        assert_eq!(out, expected);
    }

    /// A `StaticBlock` FunctionInfo prints `StaticBlock ` instead of
    /// `Func ` (cpp:456-457).
    #[test]
    fn static_block_function_prints_static_block_label() {
        let mut ctx = Context::new();
        let gc = GCLock::new(&mut ctx);
        let mut sc = SemContext::new(Keywords::new(&gc));
        let f = sc.new_function(
            FuncIsArrow::No,
            ConstructorKind::None,
            None,
            None,
            true,
            Default::default(),
        );
        sc.function_mut(f).is_static_block = true;
        sc.new_scope(f, None);

        let mut dumper = SemContextDumper::new();
        let mut out = String::new();
        dumper.print_sem_context(&mut out, &gc, &sc, None);
        assert_eq!(out, "SemContext\nStaticBlock strict\n    Scope %s.1\n");
    }
}
