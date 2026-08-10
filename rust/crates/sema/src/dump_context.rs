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
//! - **Output sink is `&mut Vec<u8>`, not `&mut String`.** C++ writes to a
//!   `llvh::raw_ostream`, which is just a byte sink (see
//!   `lib/Support/StringTable.cpp:13-15` for how `UniqueString::str()`
//!   bytes flow straight through it, unescaped and unvalidated).
//!   Identifier text is WTF-8: a `\uD800`-style escape can produce a lone
//!   surrogate code point, whose encoding is NOT valid UTF-8 (a Rust
//!   `char`/`str` can't hold a surrogate code point), so it can't be
//!   pushed into a `String`. Decl names come from identifiers and CAN
//!   contain such escapes, and this dumper's entire purpose is byte-exact
//!   output for a differential oracle — corrupting or rejecting those
//!   bytes would defeat the point. So every entry point here writes to a
//!   plain `Vec<u8>`: ASCII formatting (numbers, keywords) goes through
//!   the local `push_str` helper (`str::as_bytes`), and atom text goes
//!   through `push_atom`, which copies `gc.bytes(atom)` verbatim — no
//!   decoding, no validation, no possibility of a panic on this path.
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
/// if it needs fields). Appends to the same byte buffer the rest of the
/// dumper writes to.
pub type AnnotateDeclFunc = Box<dyn Fn(&mut Vec<u8>, DeclId)>;

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
        out: &mut Vec<u8>,
        gc: &GCLock,
        ctx: &SemContext,
        root_func: Option<FunctionInfoId>,
    ) {
        let root_func = root_func
            .unwrap_or_else(|| FunctionInfoId::from_sema_id(SemaId(0)));

        push_str(out, "SemContext\n");

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
        out: &mut Vec<u8>,
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
        out: &mut Vec<u8>,
        env: Env<'_, '_, '_>,
        f: FunctionInfoId,
        level: u32,
    ) {
        let info = env.sc.function(f);
        push_indent(out, level);
        push_str(
            out,
            if info.is_static_block {
                "StaticBlock "
            } else {
                "Func "
            },
        );
        push_str(out, if info.strict { "strict" } else { "loose" });
        out.push(b'\n');

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
        out: &mut Vec<u8>,
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
        out: &mut Vec<u8>,
        env: Env<'_, '_, '_>,
        s: ScopeId,
        level: u32,
    ) {
        push_indent(out, level);
        push_str(out, "Scope %s.");
        let n = self.scope_numbers.get_number(s);
        push_str(out, &n.to_string());
        out.push(b'\n');

        let scope = env.sc.scope(s);
        for &d in &scope.decls {
            push_indent(out, level + 1);
            self.print_decl(out, env, d);
            out.push(b'\n');
        }
        for fd in &scope.hoisted_functions {
            push_indent(out, level + 1);
            push_str(out, "hoistedFunction ");
            let node = fd.node(env.gc);
            let func_decl = node.as_function_declaration().expect(
                "SemContext::hoistedFunctions entries are always \
                 FunctionDeclaration nodes",
            );
            // An anonymous `export default function` is only rewritten to a
            // FunctionExpression when compiling, so when resolving on behalf
            // of a parser (`resolve_ast_for_parser`) a hoisted function may
            // have no name. Mirrors upstream `918158cb0`, which replaced the
            // unconditional `cast<IdentifierNode>(fd->_id)` with this
            // null check (`SemContext.cpp:493-501`).
            match func_decl.id {
                Some(id_node) => {
                    let ident = id_node
                        .as_identifier()
                        .expect("FunctionDeclaration.id is an Identifier");
                    push_atom(out, env.gc, ident.name.get());
                }
                None => push_str(out, "*default*"),
            }
            out.push(b'\n');
        }
    }

    /// Port of `printScopeRef` (cpp:508-512). Unlike `print_scope`, this
    /// never prints a name, so it needs no `GCLock`.
    pub fn print_scope_ref(&mut self, out: &mut Vec<u8>, s: ScopeId) {
        push_str(out, "Scope %s.");
        push_str(out, &self.scope_numbers.get_number(s).to_string());
    }

    /// Port of `printDecl` (cpp:514-554), including the CASE-macro switches
    /// for `Decl::Kind` and `Decl::Special` (cpp:519-543, 546-554).
    fn print_decl(
        &mut self,
        out: &mut Vec<u8>,
        env: Env<'_, '_, '_>,
        d: DeclId,
    ) {
        push_str(out, "Decl %d.");
        let n = self.decl_numbers.get_number(d);
        push_str(out, &n.to_string());
        push_str(out, " '");
        let decl = env.sc.decl(d);
        push_atom(out, env.gc, decl.name);
        push_str(out, "' ");
        push_str(out, decl_kind_str(decl.kind));

        if decl.special != DeclSpecial::NotSpecial {
            out.push(b' ');
            push_str(out, decl_special_str(decl.special));
        }

        if let Some(annotate) = &self.annotate_decl {
            annotate(out, d);
        }
    }

    /// Port of `printDeclRef` (cpp:556-562).
    pub fn print_decl_ref(
        &mut self,
        out: &mut Vec<u8>,
        gc: &GCLock,
        ctx: &SemContext,
        d: DeclId,
        print_name: bool,
    ) {
        push_str(out, "%d.");
        push_str(out, &self.decl_numbers.get_number(d).to_string());
        if print_name {
            let decl = ctx.decl(d);
            if decl.name != INVALID_ATOM_BYTES {
                push_str(out, " '");
                push_atom(out, gc, decl.name);
                out.push(b'\'');
            }
        }
    }
}

/// Port of `ind(level)` (cpp:15-17): `level * 4` spaces. `pub(crate)`
/// because `dump::ASTPrinter` (SemResolve.cpp:20-157) uses the exact same
/// indentation convention and reuses this helper rather than duplicating
/// it.
pub(crate) fn push_indent(out: &mut Vec<u8>, level: u32) {
    out.resize(out.len() + (level * 4) as usize, b' ');
}

/// Append ASCII/UTF-8 formatting text (literal format strings and
/// `usize::to_string()` output — never atom/identifier text, which goes
/// through `push_atom` instead) to the byte buffer. `pub(crate)`: shared
/// with `dump::ASTPrinter` (see `push_indent`'s doc for why).
pub(crate) fn push_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(s.as_bytes());
}

/// Push an atom's raw bytes into `out`, matching what the C++ dumper does
/// when it writes `d->name`/`_name->str()` straight to a `raw_ostream`:
/// the bytes, unescaped and unvalidated, verbatim (see the module doc's
/// "Output sink" deviation). Identifier text is WTF-8 — lone surrogate
/// code points from `\uD800`-style escapes are not valid UTF-8 — so this
/// deliberately does NOT decode or validate; it just copies bytes,
/// exactly like `raw_ostream::operator<<(StringRef)` does. `pub(crate)`:
/// shared with `dump::ASTPrinter` (see `push_indent`'s doc for why).
pub(crate) fn push_atom(out: &mut Vec<u8>, gc: &GCLock, atom: Atom) {
    out.extend_from_slice(gc.bytes(atom));
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
            Box::new(|out: &mut Vec<u8>, d: DeclId| {
                push_str(out, " /* annotated ");
                push_str(out, &d.index().to_string());
                push_str(out, " */");
            });
        let mut dumper = SemContextDumper::new_annotated(annotate);
        let mut out = Vec::new();
        dumper.print_sem_context(&mut out, &gc, &sc, None);

        let expected = "\
SemContext
Func loose
    Scope %s.1
        Decl %d.1 'x' Let /* annotated 0 */
";
        assert_eq!(String::from_utf8(out).unwrap(), expected);
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
        let mut out = Vec::new();
        dumper.print_decl_ref(&mut out, &gc, &sc, d, false);
        assert_eq!(out, b"%d.1");

        let mut out2 = Vec::new();
        dumper.print_decl_ref(&mut out2, &gc, &sc, d, true);
        assert_eq!(out2, b"%d.1 'x'");
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
        let mut out = Vec::new();
        dumper.print_scope_ref(&mut out, s);
        assert_eq!(out, b"Scope %s.1");
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
        let mut out = Vec::new();
        dumper.print_sem_context(&mut out, &gc, &sc, None);
        let expected = "\
SemContext
Func loose
    Scope %s.1
        Decl %d.1 'arguments' Var Arguments
";
        assert_eq!(String::from_utf8(out).unwrap(), expected);
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
        let mut out = Vec::new();
        dumper.print_sem_context(&mut out, &gc, &sc, None);
        assert_eq!(out, b"SemContext\nStaticBlock strict\n    Scope %s.1\n");
    }

    /// The finding this test locks in: a decl whose name is WTF-8 with a
    /// lone surrogate (not valid UTF-8) must round-trip byte-for-byte into
    /// the output, with no panic — matching the C++ `raw_ostream`, which
    /// just writes the bytes verbatim (see the module doc's "Output sink"
    /// deviation). `\uD800` alone (an unpaired high surrogate) encodes as
    /// WTF-8 `[0xED, 0xA0, 0x80]`, which is invalid UTF-8 proper.
    #[test]
    fn decl_name_with_wtf8_lone_surrogate_passes_through_unmodified() {
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

        // Intern the raw lone-surrogate WTF-8 bytes directly (bypassing
        // `str`, which cannot represent them) via `atom_bytes`, which only
        // requires `Into<Vec<u8>> + AsRef<[u8]>`, not valid UTF-8.
        let lone_surrogate_name: Vec<u8> = vec![0xED, 0xA0, 0x80];
        let name = gc.atom_bytes(lone_surrogate_name.clone());
        let d = sc.new_decl_in_scope(
            name,
            DeclKind::Let,
            s,
            DeclSpecial::NotSpecial,
        );

        let mut dumper = SemContextDumper::new();
        let mut out = Vec::new();
        // Must not panic.
        dumper.print_decl_ref(&mut out, &gc, &sc, d, true);

        let mut expected = b"%d.1 '".to_vec();
        expected.extend_from_slice(&lone_surrogate_name);
        expected.push(b'\'');
        assert_eq!(out, expected);

        // Also exercise the full `print_sem_context` path (goes through
        // `print_decl`'s `push_atom` call, not just `print_decl_ref`'s).
        let mut out2 = Vec::new();
        dumper.print_sem_context(&mut out2, &gc, &sc, None);
        let mut expected2 =
            b"SemContext\nFunc loose\n    Scope %s.1\n        Decl %d.1 '"
                .to_vec();
        expected2.extend_from_slice(&lone_surrogate_name);
        expected2.extend_from_slice(b"' Let\n");
        assert_eq!(out2, expected2);
    }
}
