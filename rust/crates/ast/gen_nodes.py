#!/usr/bin/env python3
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.
#
# Generate rust/crates/ast/src/node.rs from include/hermes/AST/ESTree.def plus a
# hand-transcribed decoration table (mirrors include/hermes/AST/ESTree.h). The
# output is the full ESTree node set: the `NodeKind` enum (with interleaved
# `_Name_First`/`_Last` range sentinels), one `#[repr(C)]` struct per leaf node
# (with composed decoration fields), the `Node` enum + accessors, minimal `new`
# constructors, and the `visit_children` / `mark_lists` GC/read walks.
#
# All parse families (FLOW/JSX/TS/Cover) are treated as ON. Re-run when
# ESTree.def or the decoration table changes:
#   python3 rust/crates/ast/gen_nodes.py            # writes src/node.rs
#   python3 rust/crates/ast/gen_nodes.py --stdout   # prints to stdout
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
DEF = ROOT / "include/hermes/AST/ESTree.def"
OUT = Path(__file__).resolve().parent / "src/node.rs"

# Total number of leaf nodes in ESTree.def (anti-drift guard). Set to the value
# the generator computes on a complete run (locked in Task 2).
EXPECTED_NODES = 271


# --------------------------------------------------------------------------
# Step 2: snake_case + Rust-keyword escaping helpers.
# --------------------------------------------------------------------------
RUST_KEYWORDS = {
    "as", "break", "const", "continue", "crate", "dyn", "else", "enum",
    "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match",
    "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static",
    "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while", "async", "await", "abstract", "become", "box", "do", "final",
    "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
}


def camel_to_snake(name):
    # CamelCase/lowerCamelCase -> snake_case, acronym-aware so JSX/TS/SH runs stay
    # together: JSXElement->jsx_element, TSTypeAliasDeclaration->ts_type_alias_declaration,
    # SHBuiltin->sh_builtin, typeAnnotation->type_annotation.
    s = re.sub(r"(.)([A-Z][a-z]+)", r"\1_\2", name)   # split before Capitalized words
    s = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", s)      # split lower/digit -> Upper
    return s.lower()


def rust_field(json_name):  # snake_case, then raw-escape reserved words
    s = camel_to_snake(json_name)
    return f"r#{s}" if s in RUST_KEYWORDS else s


# --------------------------------------------------------------------------
# Step 1: .def tokenizer/parser.
# --------------------------------------------------------------------------
def strip_comments(src):
    # Strip /* */ block comments and // line comments. The .def has no string
    # literals, so this is safe.
    src = re.sub(r"/\*.*?\*/", "", src, flags=re.S)
    src = re.sub(r"//[^\n]*", "", src)
    return src


def parse_def(src):
    """Parse ESTree.def into an ordered list of items.

    Items:
      ('first', name, base)
      ('last', name)
      ('node', name, base, [(type, name, opt_bool), ...])
    Also returns the IGNORE_IF_EMPTY dict (recorded for phase 4; unused now).
    """
    src = strip_comments(src)

    # All #if/#ifdef/#ifndef/#else/#endif/#define/#undef/#include lines are
    # treated as inactive directives: every family is ON and there are no #else
    # branches inside an ESTREE_ region. Assert that, then drop every
    # preprocessor line (the `#define ESTREE_FIRST(NAME, BASE)` stubs would
    # otherwise tokenize as bogus macro calls).
    assert "#else" not in src, "unexpected #else inside ESTree.def"
    src = "\n".join(
        line for line in src.splitlines() if not line.lstrip().startswith("#")
    )

    items = []
    ignore_if_empty = {}

    # Find every ESTREE_ macro invocation, scanning to the balanced close paren.
    i = 0
    n = len(src)
    macro_re = re.compile(r"ESTREE_(\w+)\s*\(")
    while True:
        m = macro_re.search(src, i)
        if not m:
            break
        macro = "ESTREE_" + m.group(1)
        # Scan from the '(' to its balanced close.
        depth = 0
        j = m.end() - 1  # position of '('
        start_args = m.end()
        while j < n:
            if src[j] == "(":
                depth += 1
            elif src[j] == ")":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        assert depth == 0, f"unbalanced parens for {macro}"
        arg_str = src[start_args:j]
        args = [a.strip() for a in arg_str.split(",")]
        args = [a for a in args if a != ""]
        i = j + 1

        if macro == "ESTREE_FIRST":
            items.append(("first", args[0], args[1]))
        elif macro == "ESTREE_LAST":
            items.append(("last", args[0]))
        elif macro == "ESTREE_IGNORE_IF_EMPTY":
            ignore_if_empty.setdefault(args[0], []).append(args[1])
        else:
            mn = re.match(r"NODE_(\d+)_ARGS$", m.group(1))
            assert mn, f"unrecognized macro {macro}"
            count = int(mn.group(1))
            name = args[0]
            base = args[1]
            rest = args[2:]
            assert len(rest) == count * 3, (
                f"{name}: expected {count*3} field tokens, got {len(rest)}"
            )
            fields = []
            for k in range(count):
                ftype = rest[k * 3]
                fname = rest[k * 3 + 1]
                fopt = rest[k * 3 + 2]
                assert fopt in ("true", "false"), (
                    f"{name}.{fname}: bad optional flag {fopt!r}"
                )
                fields.append((ftype, fname, fopt == "true"))
            items.append(("node", name, base, fields))

    return items, ignore_if_empty


# --------------------------------------------------------------------------
# Reference A: .def field-type -> Rust mapping.
# --------------------------------------------------------------------------
def def_field_descriptor(ftype, fname, opt):
    """Map a .def (type, name, opt) triple to a field descriptor dict.

    Keys: json_name, rust_field, rust_type, child_kind
    ('single'|'opt'|'list'|'none'), new_arg_type, default_expr (None — .def
    fields are always `new` args, never defaulted).
    """
    rf = rust_field(fname)
    if ftype == "NodePtr":
        if opt:
            rtype = "Option<&'gc Node<'gc>>"
            return dict(json_name=fname, rust_field=rf, rust_type=rtype,
                        child_kind="opt", new_arg_type=rtype,
                        cell=False, default_expr=None)
        rtype = "&'gc Node<'gc>"
        return dict(json_name=fname, rust_field=rf, rust_type=rtype,
                    child_kind="single", new_arg_type=rtype,
                    cell=False, default_expr=None)
    if ftype == "NodeList":
        return dict(json_name=fname, rust_field=rf, rust_type="NodeList<'gc>",
                    child_kind="list", new_arg_type="NodeList<'gc>",
                    cell=False, default_expr=None)
    # Value types -> Cell<...>.
    value_map = {
        "NodeBoolean": "bool",
        "NodeNumber": "f64",
        "NodeLabel": "NodeLabel",
        "NodeString": "NodeString",
    }
    inner = value_map.get(ftype)
    assert inner is not None, f"unknown .def field type {ftype!r}"
    return dict(json_name=fname, rust_field=rf,
                rust_type=f"Cell<{inner}>", child_kind="none",
                new_arg_type=inner, cell=True, default_expr=None)


# --------------------------------------------------------------------------
# Step 3: Decoration tables B/C/D + composition.
# --------------------------------------------------------------------------
# Reference B: decoration classes. Each own-field is (source_name, rust_type,
# default_expr). The emitted field name is rust_field(source_name).
DECORATIONS = {
    "ScopeDecorationBase": (
        [],
        [("scope", "Cell<Option<SemaId>>", "Cell::new(None)")],
    ),
    "LabelDecorationBase": (
        [],
        [("labelIndex", "Cell<u32>", "Cell::new(INVALID_LABEL)")],
    ),
    "GotoDecorationBase": (["LabelDecorationBase"], []),
    "FunctionLikeDecoration": (
        ["ScopeDecorationBase"],
        [
            ("semInfo", "Cell<Option<SemaId>>", "Cell::new(None)"),
            ("strictness", "Cell<Strictness>", "Cell::new(Strictness::NotSet)"),
            ("isMethodDefinition", "Cell<bool>", "Cell::new(false)"),
            ("decorations", "Cell<NodeList<'gc>>", "Cell::new(NodeList::empty())"),
        ],
    ),
    "ProgramDecoration": (
        [],
        [("dummyParamList", "Cell<NodeList<'gc>>", "Cell::new(NodeList::empty())")],
    ),
    "StatementDecoration": ([], []),
    "LoopStatementDecoration": (["LabelDecorationBase"], []),
    "SwitchStatementDecoration": (
        ["LabelDecorationBase", "ScopeDecorationBase"], []
    ),
    "BreakStatementDecoration": (["GotoDecorationBase"], []),
    "ContinueStatementDecoration": (["GotoDecorationBase"], []),
    "LabeledStatementDecoration": (["LabelDecorationBase"], []),
    "BlockStatementDecoration": (
        ["ScopeDecorationBase"],
        [
            ("bufferId", "Cell<u32>", "Cell::new(0)"),
            ("isLazyFunctionBody", "Cell<bool>", "Cell::new(false)"),
            ("paramYield", "Cell<bool>", "Cell::new(false)"),
            ("paramAwait", "Cell<bool>", "Cell::new(false)"),
            ("containsArrowFunctions", "Cell<bool>", "Cell::new(false)"),
            ("mayContainArrowFunctionsUsingArguments", "Cell<bool>",
             "Cell::new(false)"),
        ],
    ),
    "StaticBlockDecoration": (
        ["ScopeDecorationBase"],
        [("functionInfo", "Cell<Option<SemaId>>", "Cell::new(None)")],
    ),
    "ForStatementDecoration": (["ScopeDecorationBase"], []),
    "ForInStatementDecoration": (["ScopeDecorationBase"], []),
    "ForOfStatementDecoration": (["ScopeDecorationBase"], []),
    "CatchClauseDecoration": (["ScopeDecorationBase"], []),
    "ClassLikeDecoration": (
        ["ScopeDecorationBase"],
        [
            ("implicitCtorFunctionInfo", "Cell<Option<SemaId>>", "Cell::new(None)"),
            ("instanceElementsInitFunctionInfo", "Cell<Option<SemaId>>",
             "Cell::new(None)"),
            ("staticElementsInitFunctionInfo", "Cell<Option<SemaId>>",
             "Cell::new(None)"),
        ],
    ),
    "IdentifierDecoration": (
        [],
        [
            ("unresolvable", "Cell<bool>", "Cell::new(false)"),
            ("declState", "Cell<u8>", "Cell::new(0)"),
            ("decl", "Cell<Option<SemaId>>", "Cell::new(None)"),
        ],
    ),
    "JSXDecoration": ([], []),
    "FlowDecoration": ([], []),
    "TSDecoration": ([], []),
    "PatternDecoration": ([], []),
    "MatchPatternDecoration": ([], []),
    "CoverDecoration": ([], []),
    "CallExpressionLikeDecoration": ([], []),
    "MemberExpressionLikeDecoration": ([], []),
    "EmptyDecoration": ([], []),
}

# Reference D: leaf DecoratorTrait map (only leaves with a non-Empty trait).
LEAF_DECORATOR = {
    "BlockStatement": "BlockStatementDecoration",
    "StaticBlock": "StaticBlockDecoration",
    "BreakStatement": "BreakStatementDecoration",
    "ContinueStatement": "ContinueStatementDecoration",
    "ForStatement": "ForStatementDecoration",
    "ForInStatement": "ForInStatementDecoration",
    "ForOfStatement": "ForOfStatementDecoration",
    "CatchClause": "CatchClauseDecoration",
    "SwitchStatement": "SwitchStatementDecoration",
    "LabeledStatement": "LabeledStatementDecoration",
    "Identifier": "IdentifierDecoration",
    "Program": "ProgramDecoration",
}


def decoration_descriptor(source_name, rust_type, default_expr):
    """Build a uniform field descriptor for a decoration field."""
    rf = rust_field(source_name)
    if rust_type == "Cell<NodeList<'gc>>":
        child_kind = "declist"
    else:
        child_kind = "none"
    return dict(json_name=source_name, rust_field=rf, rust_type=rust_type,
                child_kind=child_kind, new_arg_type=None, cell=True,
                default_expr=default_expr)


def flatten_decoration(name, seen=None):
    """Ordered, deduped list of decoration field descriptors.

    Walks bases base-first (recursively), then own fields, deduping by
    snake_case field name (first occurrence wins).
    """
    if seen is None:
        seen = set()
    assert name in DECORATIONS, f"unknown decoration {name!r}"
    bases, own = DECORATIONS[name]
    out = []

    def take(fd):
        if fd["rust_field"] not in seen:
            seen.add(fd["rust_field"])
            out.append(fd)

    for b in bases:
        # The recursive call dedups against the shared `seen` and only returns
        # fields it actually emitted, so re-append them directly.
        for fd in flatten_decoration(b, seen):
            out.append(fd)
    for (src, rtype, default) in own:
        take(decoration_descriptor(src, rtype, default))
    return out


def build_range_parents(items):
    """Map each range NAME -> its parent BASE (from ESTREE_FIRST)."""
    parents = {}
    for it in items:
        if it[0] == "first":
            parents[it[1]] = it[2]
    return parents


def range_chain(base, range_parents):
    """Range chain for a node whose immediate base is `base`.

    Follow ESTREE_FIRST links upward until 'Base'. Return outermost-first
    (closest to Base first).
    """
    chain = []
    cur = base
    while cur != "Base":
        chain.append(cur)
        assert cur in range_parents, f"range {cur!r} has no ESTREE_FIRST"
        cur = range_parents[cur]
    chain.reverse()  # outermost-first
    return chain


def compose_fields(node_name, base, def_fields, range_parents):
    """Algorithm E: per-leaf field composition."""
    fields = []
    seen = set()

    def add(fd):
        if fd["rust_field"] not in seen:
            seen.add(fd["rust_field"])
            fields.append(fd)

    # 1. metadata.
    add(dict(json_name=None, rust_field="metadata",
             rust_type="NodeMetadata<'gc>", child_kind="meta",
             new_arg_type="NodeMetadata<'gc>", cell=False, default_expr=None))

    # 2. .def arg fields.
    for (ftype, fname, opt) in def_fields:
        add(def_field_descriptor(ftype, fname, opt))

    # 3+4. range-chain decorations (outermost-first), flattened base-first.
    decos_seen = set()
    for rng in range_chain(base, range_parents):
        deco = rng + "Decoration"
        assert deco in DECORATIONS, f"missing decoration for range {rng!r}"
        for fd in flatten_decoration(deco, decos_seen):
            add(fd)

    # 5. leaf DecoratorTrait decoration.
    leaf_deco = LEAF_DECORATOR.get(node_name, "EmptyDecoration")
    for fd in flatten_decoration(leaf_deco, decos_seen):
        add(fd)

    return fields


# --------------------------------------------------------------------------
# Emitters.
# --------------------------------------------------------------------------
HEADER = """\
/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! @generated by gen_nodes.py from include/hermes/AST/ESTree.def — DO NOT EDIT.
//! Run `python3 rust/crates/ast/gen_nodes.py` to regenerate.
#![allow(non_camel_case_types)] // NodeKind range sentinels (_Name_First/_Last) mirror C++
#![allow(clippy::too_many_arguments)] // node `new` arity follows ESTree.def (up to 10 args)
#![allow(clippy::large_enum_variant)] // one enum over all nodes — boxing would defeat deep-match

use std::cell::Cell;
use crate::node_child::{NodeLabel, NodeList, NodeMetadata, NodeString, Strictness, INVALID_LABEL};
use crate::visitor::Visitor;
use crate::SemaId;
"""


def emit_node_kind(items, out):
    out.append("#[repr(u32)]")
    out.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
    out.append("pub enum NodeKind {")
    for it in items:
        if it[0] == "first":
            out.append(f"    _{it[1]}_First,")
        elif it[0] == "last":
            out.append(f"    _{it[1]}_Last,")
        else:  # node
            out.append(f"    {it[1]},")
    out.append("}")
    out.append("")


def emit_struct(node_name, fields, out):
    out.append("#[derive(Debug)]")
    out.append("#[repr(C)]")
    out.append(f"pub struct {node_name}<'gc> {{")
    for fd in fields:
        out.append(f"    pub {fd['rust_field']}: {fd['rust_type']},")
    out.append("}")
    # new constructor.
    new_args = ["metadata: NodeMetadata<'gc>"]
    for fd in fields:
        if fd["rust_field"] == "metadata":
            continue
        if fd["new_arg_type"] is None:
            continue  # decoration field: defaulted
        new_args.append(f"{fd['rust_field']}: {fd['new_arg_type']}")
    out.append(f"impl<'gc> {node_name}<'gc> {{")
    out.append(f"    pub fn new({', '.join(new_args)}) -> Self {{")
    out.append(f"        {node_name} {{")
    # Build the field-init list.
    inits = []
    for fd in fields:
        rf = fd["rust_field"]
        if rf == "metadata":
            inits.append("metadata")
        elif fd["new_arg_type"] is None:
            # decoration field: default.
            inits.append(f"{rf}: {fd['default_expr']}")
        elif fd["cell"]:
            inits.append(f"{rf}: Cell::new({rf})")
        else:
            inits.append(rf)
    out.append("            " + ", ".join(inits) + ",")
    out.append("        }")
    out.append("    }")
    out.append("}")
    out.append("")


def emit_node_enum(nodes, out):
    out.append("#[derive(Debug)]")
    out.append("#[repr(C)]")
    out.append("pub enum Node<'gc> {")
    for name, _fields in nodes:
        out.append(f"    {name}({name}<'gc>),")
    out.append("}")
    out.append("")


def emit_accessors(items, nodes, out):
    out.append("impl<'gc> Node<'gc> {")
    # kind().
    out.append("    pub fn kind(&self) -> NodeKind {")
    out.append("        match self {")
    for name, _ in nodes:
        out.append(f"            Node::{name}(_) => NodeKind::{name},")
    out.append("        }")
    out.append("    }")
    out.append("")
    # metadata().
    out.append("    pub fn metadata(&self) -> &NodeMetadata<'gc> {")
    out.append("        match self {")
    for name, _ in nodes:
        out.append(f"            Node::{name}(n) => &n.metadata,")
    out.append("        }")
    out.append("    }")
    out.append("")
    # range().
    out.append("    pub fn range(&self) -> support::location::SMRange {")
    out.append("        self.metadata().range.get()")
    out.append("    }")
    out.append("")
    # range predicates.
    for it in items:
        if it[0] == "first":
            rng = it[1]
            pred = "is_" + camel_to_snake(rng)
            out.append(f"    pub fn {pred}(&self) -> bool {{")
            out.append("        let k = self.kind() as u32;")
            out.append(
                f"        (NodeKind::_{rng}_First as u32) < k "
                f"&& k < (NodeKind::_{rng}_Last as u32)"
            )
            out.append("    }")
            out.append("")
    # leaf accessors.
    for name, _ in nodes:
        acc = "as_" + camel_to_snake(name)
        out.append(f"    pub fn {acc}(&self) -> Option<&{name}<'gc>> {{")
        out.append(f"        if let Node::{name}(n) = self {{ Some(n) }} "
                   f"else {{ None }}")
        out.append("    }")
        out.append("")
    # visit_children.
    emit_visit_children(nodes, out)
    out.append("")
    # mark_lists.
    emit_mark_lists(nodes, out)
    out.append("}")
    out.append("")


def child_fields(fields):
    """Fields that participate in child walks, in declared order."""
    return [fd for fd in fields if fd["child_kind"] in
            ("single", "opt", "list", "declist")]


def emit_visit_children(nodes, out):
    out.append("    pub fn visit_children<V: Visitor<'gc> + ?Sized>"
               "(&'gc self, v: &mut V) {")
    out.append("        match self {")
    for name, fields in nodes:
        cf = child_fields(fields)
        if not cf:
            out.append(f"            Node::{name}(_) => {{}}")
            continue
        out.append(f"            Node::{name}(n) => {{")
        for fd in cf:
            rf = fd["rust_field"]
            kind = fd["child_kind"]
            if kind == "single":
                out.append(f"                v.visit_node(n.{rf});")
            elif kind == "opt":
                out.append(
                    f"                if let Some(c) = n.{rf} "
                    f"{{ v.visit_node(c); }}")
            elif kind == "list":
                out.append(
                    f"                for c in n.{rf}.iter() "
                    f"{{ v.visit_node(c); }}")
            elif kind == "declist":
                out.append(
                    f"                for c in n.{rf}.get().iter() "
                    f"{{ v.visit_node(c); }}")
        out.append("            }")
    out.append("        }")
    out.append("    }")


def emit_mark_lists(nodes, out):
    out.append("    pub fn mark_lists<F: FnMut(&NodeList<'gc>)>"
               "(&'gc self, cb: &mut F) {")
    out.append("        match self {")
    for name, fields in nodes:
        list_fields = [fd for fd in fields
                       if fd["child_kind"] in ("list", "declist")]
        if not list_fields:
            continue
        parts = []
        for fd in list_fields:
            rf = fd["rust_field"]
            if fd["child_kind"] == "declist":
                parts.append(f"cb(&n.{rf}.get());")
            else:
                parts.append(f"cb(&n.{rf});")
        out.append(f"            Node::{name}(n) => {{ {' '.join(parts)} }}")
    out.append("            _ => {}")
    out.append("        }")
    out.append("    }")


# --------------------------------------------------------------------------
# Step 8: CLI + validation + driver.
# --------------------------------------------------------------------------
def generate():
    if not DEF.exists():
        sys.exit(f"error: {DEF} not found — run from a full hermes checkout")
    src = DEF.read_text()
    items, ignore_if_empty = parse_def(src)
    range_parents = build_range_parents(items)

    # Build the composed node list (name, fields), in .def order.
    nodes = []
    for it in items:
        if it[0] != "node":
            continue
        _, name, base, def_fields = it
        fields = compose_fields(name, base, def_fields, range_parents)
        nodes.append((name, fields))

    # Anti-drift: node count.
    node_count = len(nodes)
    if node_count != EXPECTED_NODES:
        sys.exit(
            f"error: ESTree.def has {node_count} nodes, expected "
            f"{EXPECTED_NODES} (update EXPECTED_NODES if intentional)"
        )

    # Validate: every range has a decoration; every leaf resolved a decoration.
    for rng in range_parents:
        deco = rng + "Decoration"
        if deco not in DECORATIONS:
            sys.exit(f"error: range {rng!r} has no {deco} in the table")
    for name, _ in nodes:
        leaf_deco = LEAF_DECORATOR.get(name, "EmptyDecoration")
        if leaf_deco not in DECORATIONS:
            sys.exit(f"error: leaf {name!r} -> unknown decoration {leaf_deco}")

    out = [HEADER]
    emit_node_kind(items, out)
    for name, fields in nodes:
        emit_struct(name, fields, out)
    emit_node_enum(nodes, out)
    emit_accessors(items, nodes, out)

    text = "\n".join(out)
    if not text.endswith("\n"):
        text += "\n"
    return text, node_count, len(range_parents)


def main():
    to_stdout = "--stdout" in sys.argv[1:]
    text, node_count, range_count = generate()
    if to_stdout:
        sys.stdout.write(text)
    else:
        OUT.write_text(text)
    print(
        f"gen_nodes.py: {node_count} nodes, {range_count} ranges "
        f"{'-> stdout' if to_stdout else f'-> {OUT}'}",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
