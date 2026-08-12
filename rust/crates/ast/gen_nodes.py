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
import textwrap
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
DEF = ROOT / "include/hermes/AST/ESTree.def"
OUT = Path(__file__).resolve().parent / "src/node.rs"

# Total leaf nodes in ESTree.def — anti-drift guard. Update this when a node
# is intentionally added or removed.
EXPECTED_NODES = 271


# --------------------------------------------------------------------------
# -- Helpers: snake_case + keyword escaping --
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
# -- .def tokenizer/parser --
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
    if "#else" in src:
        sys.exit("error: unexpected #else inside ESTree.def")
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
        if depth != 0:
            sys.exit(f"error: unbalanced parens for {macro}")
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
            if not mn:
                sys.exit(f"error: unrecognized macro {macro}")
            count = int(mn.group(1))
            name = args[0]
            base = args[1]
            rest = args[2:]
            if len(rest) != count * 3:
                sys.exit(
                    f"error: {name}: expected {count*3} field tokens, "
                    f"got {len(rest)}"
                )
            fields = []
            for k in range(count):
                ftype = rest[k * 3]
                fname = rest[k * 3 + 1]
                fopt = rest[k * 3 + 2]
                if fopt not in ("true", "false"):
                    sys.exit(
                        f"error: {name}.{fname}: bad optional flag {fopt!r}"
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
                        cell=False, default_expr=None,
                        is_def_arg=True, dump_kind="node_opt")
        rtype = "&'gc Node<'gc>"
        return dict(json_name=fname, rust_field=rf, rust_type=rtype,
                    child_kind="single", new_arg_type=rtype,
                    cell=False, default_expr=None,
                    is_def_arg=True, dump_kind="node_single")
    if ftype == "NodeList":
        return dict(json_name=fname, rust_field=rf, rust_type="NodeList<'gc>",
                    child_kind="list", new_arg_type="NodeList<'gc>",
                    cell=False, default_expr=None,
                    is_def_arg=True, dump_kind="list")
    # Value types -> Cell<...>.
    value_map = {
        "NodeBoolean": "bool",
        "NodeNumber": "f64",
        "NodeLabel": "NodeLabel",
        "NodeString": "NodeString",
    }
    inner = value_map.get(ftype)
    if inner is None:
        sys.exit(f"error: unknown .def field type {ftype!r}")
    return dict(json_name=fname, rust_field=rf,
                rust_type=f"Cell<{inner}>", child_kind="none",
                new_arg_type=inner, cell=True, default_expr=None,
                is_def_arg=True,
                dump_kind={"NodeBoolean": "bool", "NodeNumber": "number",
                           "NodeLabel": "label", "NodeString": "label"}[ftype])


# --------------------------------------------------------------------------
# -- Decoration tables (B/C/D) and field composition --
# --------------------------------------------------------------------------
# Reference B: decoration classes, hand-transcribed from ESTree.h:264-501.
# Each own-field is (source_name, rust_type, default_expr). The emitted
# field name is rust_field(source_name).
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

# Reference C: every ESTREE_FIRST range NAME implicitly carries NAMEDecoration
# (applied in compose_fields). The validation below enforces this at generation time.

# Reference D: leaf DecoratorTrait map, hand-transcribed from ESTree.h:530-581.
# (only leaves with a non-Empty trait)
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


# Reference B': one doc comment per distinct decoration field, condensed from
# the comments on the C++ members in ESTree.h:264-501. Each entry is the list
# of `///` lines emitted above the field (no leading `/// `).
DECORATION_DOCS = {
    "scope": ["Sema: the lexical scope created by this node, if any."],
    "labelIndex": ["Sema: label index; `INVALID_LABEL` until set."],
    "semInfo": ["Sema: `FunctionInfo` for this function."],
    "strictness": ["Sema: strict-mode state of this function."],
    "isMethodDefinition": [
        "Whether this function is a method definition (getters and setters",
        "included) rather than a `function`. Used for lazy reparsing.",
    ],
    "decorations": [
        "Decorations attached to this function by `Hermes.decorate(...)`",
        "calls in typed mode; each entry wraps a decoration expression.",
    ],
    "dummyParamList": [
        "An always-empty parameter list, for uniformity with functions.",
    ],
    "bufferId": ["The source buffer id in which this block was found."],
    "isLazyFunctionBody": [
        "True if this is a function body pruned while pre-parsing.",
    ],
    "paramYield": [
        "For a lazy block, the `Yield` param to restore when parsed eagerly.",
    ],
    "paramAwait": [
        "For a lazy block, the `Await` param to restore when parsed eagerly.",
    ],
    "containsArrowFunctions": [
        "Whether this function contains an arrow function. Read by lazy",
        "compilation to populate the `FunctionInfo`.",
    ],
    "mayContainArrowFunctionsUsingArguments": [
        "Conservative estimate of whether an arrow function here may use",
        "`arguments`, so a non-arrow function must eagerly capture it.",
    ],
    "functionInfo": ["Sema: `FunctionInfo` for this static block."],
    "implicitCtorFunctionInfo": [
        "Sema: `FunctionInfo` of the synthetic implicit constructor, if the",
        "class has one.",
    ],
    "instanceElementsInitFunctionInfo": [
        "Sema: `FunctionInfo` of the synthetic function that initializes the",
        "instance elements, if the class needs one.",
    ],
    "staticElementsInitFunctionInfo": [
        "Sema: `FunctionInfo` of the synthetic function that runs the static",
        "field initializers, if the class has any.",
    ],
    "unresolvable": [
        "Sema: unresolvable because of an enclosing `eval` or `with`.",
    ],
    "declState": [
        "Sema: how to read `decl` — the `BitHave*` bits of `ESTree.h`'s",
        "`IdentifierDecoration`.",
    ],
    "decl": [
        "Sema: the declaration this identifier resolves to; `None` until a",
        "resolution is recorded.",
    ],
}


def decoration_descriptor(source_name, rust_type, default_expr):
    """Build a uniform field descriptor for a decoration field."""
    rf = rust_field(source_name)
    if rust_type == "Cell<NodeList<'gc>>":
        child_kind = "declist"
    else:
        child_kind = "none"
    if source_name not in DECORATION_DOCS:
        sys.exit(f"error: no DECORATION_DOCS entry for {source_name!r}")
    return dict(json_name=source_name, rust_field=rf, rust_type=rust_type,
                child_kind=child_kind, new_arg_type=None, cell=True,
                default_expr=default_expr, is_def_arg=False, dump_kind=None,
                doc=DECORATION_DOCS[source_name])


def flatten_decoration(name, seen=None):
    """Ordered, deduped list of decoration field descriptors.

    Walks bases base-first (recursively), then own fields, deduping by
    snake_case field name (first occurrence wins).
    """
    if seen is None:
        seen = set()
    if name not in DECORATIONS:
        sys.exit(f"error: unknown decoration {name!r}")
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
        if cur not in range_parents:
            sys.exit(f"error: range {cur!r} has no ESTREE_FIRST")
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
             new_arg_type="NodeMetadata<'gc>", cell=False, default_expr=None,
             is_def_arg=False, dump_kind=None,
             doc=["Source range, debug location, paren count, and node id."]))

    # 2. .def arg fields.
    for (ftype, fname, opt) in def_fields:
        fd = def_field_descriptor(ftype, fname, opt)
        fd["doc"] = [f"ESTree `{fname}` property."]
        add(fd)

    # 3+4. range-chain decorations (outermost-first), flattened base-first.
    # `decos_seen` dedups across decoration classes only (a field introduced by
    # an inner decoration class is not repeated by an outer one).  The outer
    # `seen`/`add()` additionally dedups decoration fields against .def fields
    # (first occurrence wins); no .def/decoration name collision exists today.
    decos_seen = set()
    for rng in range_chain(base, range_parents):
        deco = rng + "Decoration"
        if deco not in DECORATIONS:
            sys.exit(f"error: missing decoration for range {rng!r}")
        for fd in flatten_decoration(deco, decos_seen):
            add(fd)

    # 5. leaf DecoratorTrait decoration.
    leaf_deco = LEAF_DECORATOR.get(node_name, "EmptyDecoration")
    for fd in flatten_decoration(leaf_deco, decos_seen):
        add(fd)

    return fields


# --------------------------------------------------------------------------
# -- Emitters --
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
use crate::node_child::{NodeChild, NodeLabel, NodeList, NodeMetadata, NodeString, Strictness, INVALID_LABEL};
use crate::visitor::{Path, TransformResult, Visitor, VisitorMut};
use crate::NodeId;
use crate::SemaId;
"""


def emit_doc(out, lines, indent=""):
    """Emit `lines` as a `///` doc comment at `indent`.

    Each input line is a paragraph of its own: hard-wrapped to 80 columns if
    needed (long node names make some of them overflow), but never joined with
    its neighbours, so the hand-written line breaks survive.
    """
    width = 80 - len(indent) - len("/// ")
    for line in lines:
        for wrapped in (textwrap.wrap(line, width) or [""]):
            out.append(f"{indent}/// {wrapped}".rstrip())


def unraw(name):
    """The plain source name of a possibly raw-escaped Rust identifier."""
    return name[2:] if name.startswith("r#") else name


def emit_node_kind(items, out):
    emit_doc(out, [
        "The kind discriminant of an AST node.",
        "",
        "Mirrors the C++ `NodeKind` enum: `#[repr(u32)]`, `ESTree.def` order,",
        "with `_Name_First`/`_Last` sentinels interleaved so the `Node::is_*`",
        "range predicates are two integer comparisons.",
    ])
    out.append("#[repr(u32)]")
    out.append("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]")
    out.append("pub enum NodeKind {")
    for it in items:
        if it[0] == "first":
            emit_doc(out, [f"Exclusive lower bound of the `{it[1]}` range."],
                     "    ")
            out.append(f"    _{it[1]}_First,")
        elif it[0] == "last":
            emit_doc(out, [f"Exclusive upper bound of the `{it[1]}` range."],
                     "    ")
            out.append(f"    _{it[1]}_Last,")
        else:  # node
            emit_doc(out, [f"The kind of [`{it[1]}`]."], "    ")
            out.append(f"    {it[1]},")
    out.append("}")
    out.append("")


def emit_struct(node_name, fields, out):
    emit_doc(out, [f"The `{node_name}` AST node."])
    out.append("#[derive(Debug)]")
    out.append("#[repr(C)]")
    out.append(f"pub struct {node_name}<'gc> {{")
    for fd in fields:
        emit_doc(out, fd["doc"], "    ")
        out.append(f"    pub {fd['rust_field']}: {fd['rust_type']},")
    out.append("}")
    # new constructor — signature args one-per-line, struct-init fields one-per-line.
    new_args = ["metadata: NodeMetadata<'gc>"]
    for fd in fields:
        if fd["rust_field"] == "metadata":
            continue
        if fd["new_arg_type"] is None:
            continue  # decoration field: defaulted
        new_args.append(f"{fd['rust_field']}: {fd['new_arg_type']}")
    doc = [f"Build `{node_name}` from its metadata and `ESTree.def` fields."]
    if any(fd["new_arg_type"] is None for fd in fields):
        doc.append("Decoration fields start at their defaults.")
    out.append(f"impl<'gc> {node_name}<'gc> {{")
    emit_doc(out, doc, "    ")
    out.append("    pub fn new(")
    for arg in new_args:
        out.append(f"        {arg},")
    out.append("    ) -> Self {")
    out.append(f"        {node_name} {{")
    # Build the field-init list, one field per line.
    for fd in fields:
        rf = fd["rust_field"]
        if rf == "metadata":
            out.append("            metadata,")
        elif fd["new_arg_type"] is None:
            # decoration field: default.
            out.append(f"            {rf}: {fd['default_expr']},")
        elif fd["cell"]:
            out.append(f"            {rf}: Cell::new({rf}),")
        else:
            out.append(f"            {rf},")
    out.append("        }")
    out.append("    }")
    out.append("}")
    out.append("")


def emit_node_enum(nodes, out):
    emit_doc(out, [
        "An AST node: one arm per `ESTree.def` node kind.",
        "",
        "`#[repr(C)]`, and each arm's payload struct is named after the arm,",
        "so a deep `match` compiles to a single dispatch. Nodes live in the",
        "[`crate::context::Context`] arena and are handed out as `&'gc Node`",
        "for as long as a [`crate::context::GCLock`] is held.",
    ])
    out.append("#[derive(Debug)]")
    out.append("#[repr(C)]")
    out.append("pub enum Node<'gc> {")
    for name, _fields in nodes:
        emit_doc(out, [f"A [`{name}`] node."], "    ")
        out.append(f"    {name}({name}<'gc>),")
    out.append("}")
    out.append("")


def emit_accessors(items, nodes, out):
    out.append("impl<'gc> Node<'gc> {")
    # kind().
    out.append("    /// This node's [`NodeKind`].")
    out.append("    pub fn kind(&self) -> NodeKind {")
    out.append("        match self {")
    for name, _ in nodes:
        out.append(f"            Node::{name}(_) => NodeKind::{name},")
    out.append("        }")
    out.append("    }")
    out.append("")
    # metadata().
    out.append("    /// This node's metadata, common to every kind.")
    out.append("    pub fn metadata(&self) -> &NodeMetadata<'gc> {")
    out.append("        match self {")
    for name, _ in nodes:
        out.append(f"            Node::{name}(n) => &n.metadata,")
    out.append("        }")
    out.append("    }")
    out.append("")
    # node_id().
    out.append("    /// This node's arena identity (see [`NodeId`]).")
    out.append("    pub fn node_id(&self) -> NodeId {")
    out.append("        self.metadata().id.get()")
    out.append("    }")
    out.append("")
    # range().
    out.append("    /// This node's source range.")
    out.append("    pub fn range(&self) -> support::location::SMRange {")
    out.append("        self.metadata().range.get()")
    out.append("    }")
    out.append("")
    # range predicates.
    for it in items:
        if it[0] == "first":
            rng = it[1]
            pred = "is_" + camel_to_snake(rng)
            emit_doc(out, [f"Whether this node's kind is in the `{rng}` range."],
                     "    ")
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
        emit_doc(out, [f"The payload if this is a [`{name}`], `None` otherwise."],
                 "    ")
        out.append(f"    pub fn {acc}(&self) -> Option<&{name}<'gc>> {{")
        out.append(f"        if let Node::{name}(n) = self {{ Some(n) }} "
                   f"else {{ None }}")
        out.append("    }")
        out.append("")
    # visit_children.
    emit_visit_children(nodes, out)
    out.append("")
    # visit_children_mut.
    emit_visit_children_mut(nodes, out)
    out.append("")
    # mark_lists.
    emit_mark_lists(nodes, out)
    # node_type_str + dump_children (phase 4 — the JSON dumper walk).
    out.append("")
    emit_node_type_str(nodes, out)
    out.append("")
    emit_dump_children(nodes, out)
    out.append("}")
    out.append("")


def child_fields(fields):
    """Fields that participate in child walks, in declared order."""
    return [fd for fd in fields if fd["child_kind"] in
            ("single", "opt", "list", "declist")]


def emit_visit_children(nodes, out):
    out.append("    /// Visit every child node in declared order: the `ESTree.def`")
    out.append("    /// fields plus the decoration lists the GC marker must trace.")
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
    out.append("    /// Call `cb` on every `NodeList` field of this node, including")
    out.append("    /// decoration lists. Used by the GC to mark list elements.")
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


def emit_node_type_str(nodes, out):
    out.append("    /// The ESTree node type name — the JSON `\"type\"` value.")
    out.append("    pub fn node_type_str(&self) -> &'static str {")
    out.append("        match self {")
    for name, _ in nodes:
        out.append(f"            Node::{name}(_) => {name!r},".replace("'", '"'))
    out.append("        }")
    out.append("    }")


def dump_arg_fields(fields):
    """The .def-arg fields, in declared order (the only fields the dumper emits)."""
    return [fd for fd in fields if fd.get("is_def_arg")]


def emit_dump_children(nodes, out):
    out.append("    /// Emit this node's `.def` child fields as JSON key/values,")
    out.append("    /// in declared order. Mirrors C++ `visitChildren`.")
    out.append("    pub fn dump_children<'a, 'w>(")
    out.append("        &'gc self,")
    out.append("        d: &mut crate::dump::ESTreeJSONDumper<'a, 'w>,")
    out.append("    ) {")
    out.append("        match self {")
    for name, fields in nodes:
        af = dump_arg_fields(fields)
        if not af:
            out.append(f"            Node::{name}(_) => {{}}")
            continue
        out.append(f"            Node::{name}(n) => {{")
        for fd in af:
            key = fd["json_name"]
            rf = fd["rust_field"]
            ign = "true" if fd.get("_ignore") else "false"
            dk = fd["dump_kind"]
            if dk == "node_single":
                out.append(f'                d.field_node("{key}", Some(n.{rf}), {ign});')
            elif dk == "node_opt":
                out.append(f'                d.field_node("{key}", n.{rf}, {ign});')
            elif dk == "list":
                out.append(f'                d.field_list("{key}", n.{rf}, {ign});')
            elif dk == "bool":
                out.append(f'                d.field_bool("{key}", n.{rf}.get(), {ign});')
            elif dk == "number":
                out.append(f'                d.field_number("{key}", n.{rf}.get(), {ign});')
            elif dk == "label":
                out.append(f'                d.field_label("{key}", n.{rf}.get(), {ign});')
            else:
                sys.exit(f"error: {name}.{rf}: bad dump_kind {dk!r}")
        out.append("            }")
    out.append("        }")
    out.append("    }")


def emit_node_field(nodes, out):
    """Emit the NodeField enum: one variant per distinct structural-child field name.

    Structural children are fields with child_kind in {single, opt, list}.
    Variants are sorted deterministically (so output is stable/idempotent).
    The rust_field strings are used verbatim as variant names (including any
    raw-identifier prefix like r#await).
    """
    seen = set()
    for _name, fields in nodes:
        for fd in fields:
            if fd["child_kind"] in ("single", "opt", "list"):
                seen.add(fd["rust_field"])
    variants = sorted(seen)

    out.append("/// The name of a structural child field of an AST node (used in `Path`).")
    out.append("#[derive(Debug, Copy, Clone, PartialEq, Eq)]")
    out.append("#[allow(non_camel_case_types)]")
    out.append("pub enum NodeField {")
    for v in variants:
        emit_doc(out, [f"The `{unraw(v)}` child field."], "    ")
        out.append(f"    {v},")
    out.append("}")
    out.append("")


def structural_fields(fields):
    """Structural-child fields (get a builder setter; threaded by
    visit_children_mut), in declared order."""
    return [fd for fd in fields if fd["child_kind"] in ("single", "opt", "list")]


def emit_visit_children_mut(nodes, out):
    """Emit Node::visit_children_mut — functional rebuild via the per-kind
    builder. Threads only structural-child fields (single/opt/list)."""
    out.append("    /// Transform this node's children with `visitor`, rebuilding it only if")
    out.append("    /// a child changed. `self` is the original parent.")
    out.append("    pub fn visit_children_mut<V: VisitorMut<'gc>>(")
    out.append("        &'gc self,")
    out.append("        ctx: &'gc crate::context::GCLock<'_, '_>,")
    out.append("        visitor: &mut V,")
    out.append("    ) -> TransformResult<&'gc Node<'gc>> {")
    out.append("        let builder = builder::Builder::from_node(self);")
    out.append("        #[allow(unused_mut)]")
    out.append("        match builder {")
    for name, fields in nodes:
        sf = structural_fields(fields)
        if not sf:
            out.append(
                f"            builder::Builder::{name}(mut b) => b.build(ctx),")
            continue
        out.append(f"            builder::Builder::{name}(mut b) => {{")
        for fd in sf:
            rf = fd["rust_field"]
            out.append("                if let TransformResult::Changed(v) =")
            out.append(
                f"                    b.inner.{rf}.visit_child_mut("
                f"ctx, visitor, Path::new(self, NodeField::{rf})) {{")
            out.append(f"                    b.{rf}(v);")
            out.append("                }")
        out.append("                b.build(ctx)")
        out.append("            }")
    out.append("        }")
    out.append("    }")


def emit_builders(nodes, out):
    """Emit the module-level `pub mod builder` — the Builder enum + one
    clone-with-one-field-changed builder struct per node."""
    out.append("/// Per-kind node builders: clone a node with (optionally) one")
    out.append("/// structural-child field changed. The rebuild primitive used by")
    out.append("/// `Node::visit_children_mut`.")
    out.append("pub mod builder {")
    out.append("    use std::cell::Cell;")
    out.append("    use super::*;")
    out.append("    use crate::node_child::NodeChild;")
    out.append("    use crate::visitor::TransformResult;")
    out.append("")
    out.append("    /// One builder per node kind; clone-with-one-field-changed.")
    out.append("    #[derive(Debug)]")
    out.append("    pub enum Builder<'gc> {")
    for name, _fields in nodes:
        emit_doc(out, [f"Builder for [`super::{name}`]."], "        ")
        out.append(f"        {name}(self::{name}<'gc>),")
    out.append("    }")
    out.append("")
    out.append("    impl<'gc> Builder<'gc> {")
    out.append("        /// Start a builder for `node`, dispatching on its kind.")
    out.append("        pub fn from_node(node: &'gc Node<'gc>) -> Self {")
    out.append("            match node {")
    for name, _fields in nodes:
        out.append(
            f"                Node::{name}(n) => "
            f"Builder::{name}(self::{name}::from_node(n)),")
    out.append("            }")
    out.append("        }")
    out.append("    }")
    out.append("")
    for name, fields in nodes:
        emit_builder_struct(name, fields, out)
    out.append("}")
    out.append("")


def emit_builder_struct(name, fields, out):
    """Emit one builder struct + impl (from_node/build/build_forced/setters)."""
    emit_doc(out, [
        f"Clone-with-changes builder for [`super::{name}`].",
    ], "    ")
    out.append("    #[derive(Debug)]")
    out.append(f"    pub struct {name}<'gc> {{")
    out.append("        is_changed: bool,")
    out.append(f"        pub(super) inner: super::{name}<'gc>,")
    out.append("    }")
    out.append(f"    impl<'gc> {name}<'gc> {{")
    out.append("        /// Start from a copy of `node`, with no field changed yet.")
    out.append(
        f"        pub fn from_node(node: &'gc super::{name}<'gc>) -> Self {{")
    out.append("            Self {")
    out.append("                is_changed: false,")
    out.append(f"                inner: super::{name} {{")
    for fd in fields:
        rf = fd["rust_field"]
        kind = fd["child_kind"]
        if kind == "meta":
            out.append("                    metadata: node.metadata.duplicate(),")
        elif kind in ("single", "opt", "list"):
            out.append(f"                    {rf}: node.{rf}.duplicate(),")
        else:  # declist or value cell
            out.append(f"                    {rf}: Cell::new(node.{rf}.get()),")
    out.append("                },")
    out.append("            }")
    out.append("        }")
    out.append("        /// Allocate the rebuilt node if a setter ran, else")
    out.append("        /// `TransformResult::Unchanged`.")
    out.append(
        "        pub fn build(self, gc: &'gc crate::context::GCLock<'_, '_>)"
        " -> TransformResult<&'gc Node<'gc>> {")
    out.append("            if self.is_changed {")
    out.append("                TransformResult::Changed(self.build_forced(gc))")
    out.append("            } else {")
    out.append("                TransformResult::Unchanged")
    out.append("            }")
    out.append("        }")
    out.append("        /// Allocate the node unconditionally, changed or not.")
    out.append(
        "        pub fn build_forced(self, gc: &'gc crate::context::GCLock<'_, '_>)"
        " -> &'gc Node<'gc> {")
    out.append(f"            gc.alloc(Node::{name}(self.inner))")
    out.append("        }")
    for fd in structural_fields(fields):
        rf = fd["rust_field"]
        ty = fd["rust_type"]
        out.append(
            f"        /// Set the `{unraw(rf)}` field and mark the builder changed.")
        out.append(
            f"        pub fn {rf}(&mut self, {rf}: {ty}) {{ "
            f"self.is_changed = true; self.inner.{rf} = {rf}; }}")
    out.append("    }")


# --------------------------------------------------------------------------
# -- CLI + validation + driver --
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

    # Validate ESTREE_IGNORE_IF_EMPTY against the real nodes/fields (drift guard).
    fields_by_node = {name: fields for name, fields in nodes}
    for node_name, field_list in ignore_if_empty.items():
        if node_name not in fields_by_node:
            sys.exit(f"error: IGNORE_IF_EMPTY names unknown node {node_name!r}")
        argnames = {fd["json_name"] for fd in fields_by_node[node_name]
                    if fd.get("is_def_arg")}
        for f in field_list:
            if f not in argnames:
                sys.exit(
                    f"error: IGNORE_IF_EMPTY({node_name},{f}) is not a .def arg field")

    # Stamp the per-field _ignore flag (used by the dump-children emission).
    for name, fields in nodes:
        ign = set(ignore_if_empty.get(name, ()))
        for fd in fields:
            fd["_ignore"] = fd.get("is_def_arg") and fd["json_name"] in ign

    # Anti-drift: node count.
    node_count = len(nodes)
    if node_count != EXPECTED_NODES:
        sys.exit(
            f"error: ESTree.def has {node_count} nodes, expected "
            f"{EXPECTED_NODES} (update EXPECTED_NODES if intentional)"
        )

    # Validate: every range has a decoration; every leaf resolved a decoration;
    # every LEAF_DECORATOR key names a real node (catches silent node renames).
    node_names = {name for name, _ in nodes}
    for rng in range_parents:
        deco = rng + "Decoration"
        if deco not in DECORATIONS:
            sys.exit(f"error: range {rng!r} has no {deco} in the table")
    for name, _ in nodes:
        leaf_deco = LEAF_DECORATOR.get(name, "EmptyDecoration")
        if leaf_deco not in DECORATIONS:
            sys.exit(f"error: leaf {name!r} -> unknown decoration {leaf_deco}")
    for leaf in LEAF_DECORATOR:
        if leaf not in node_names:
            sys.exit(f"error: LEAF_DECORATOR key {leaf!r} is not a node in ESTree.def")

    out = [HEADER]
    emit_node_kind(items, out)
    emit_node_field(nodes, out)
    for name, fields in nodes:
        emit_struct(name, fields, out)
    emit_node_enum(nodes, out)
    emit_accessors(items, nodes, out)
    emit_builders(nodes, out)

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
