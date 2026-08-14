# AST Phase 4 — `ESTreeJSONDumper` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port `lib/AST/ESTreeJSONDumper.cpp` to the Rust `ast` crate — the JSON serializer that becomes the byte-for-byte differential-oracle surface at Parser time — and validate it with golden tests over hand-built trees.

**Architecture:** The dumper splits into a **generated** half (the per-kind field walk + the `type` name string, emitted by extending `gen_nodes.py` into the committed `src/node.rs`) and a **hand-written** half (`src/dump.rs`: the `ESTreeJSONDumper` struct, mode/location/raw logic, the value-emit helpers, and the public `dump_estree_json` entry points). The generated `Node::dump_children` calls typed `field_*` helper methods on `ESTreeJSONDumper`, baking each field's JSON key (the retained camelCase `.def` name) and its `ESTREE_IGNORE_IF_EMPTY` flag at generation time — so the C++ runtime `StringMap` of ignored fields disappears.

**Tech Stack:** Rust (ast crate), Python generator (`gen_nodes.py`), the ported `support::json_emitter::JSONEmitter`, `support` source manager + locations, `atom_table` for label resolution.

---

## Background the implementer must know

Read these before starting:
- **C++ source of truth:** `lib/AST/ESTreeJSONDumper.cpp` (621 lines) + `include/hermes/AST/ESTreeJSONDumper.h`.
- **The generator:** `rust/crates/ast/gen_nodes.py` — already parses `ESTree.def`, composes per-leaf fields, and emits `src/node.rs`. It already **parses `ESTREE_IGNORE_IF_EMPTY` into `ignore_if_empty` but does not use it yet** (`parse_def`, line ~126; `generate()` line ~769 discards it as `_ignore_if_empty`). Field descriptors are dicts with keys `json_name`, `rust_field`, `rust_type`, `child_kind`, `cell`, `new_arg_type`, `default_expr` (see `def_field_descriptor` ~159, `decoration_descriptor` ~303, and the `metadata` descriptor in `compose_fields` ~380).
- **The node model:** `rust/crates/ast/src/node.rs` (`@generated`). Each `Node` enum arm wraps a `#[repr(C)]` struct; the variant name **is** the C++ node name and the JSON `"type"` value. `src/node_child.rs` defines `NodeList`, `NodeLabel`/`NodeString` (both = `atom_table::AtomBytes`), `NodeMetadata`.
- **The emitter:** `rust/crates/support/src/json_emitter.rs` — `JSONEmitter::new(&mut String, pretty)`, `open_dict/close_dict/open_array/close_array`, `emit_key(&str)`, `emit_str(&str)`, `emit_bool`, `emit_f64`, `emit_i64/emit_u64`, `emit_null_value`, `emit_u16(&[u16])`, `end_jsonl`. Its `primitive_emit_string` (used by `emit_str`/`emit_key`) iterates `chars()` and `encodeUTF16`s each non-ASCII code point, **byte-identical to C++ `primitiveEmitString` for valid UTF-8**; `emit_u16` escapes each code unit independently, matching the WTF-8 (surrogate) path.
- **Atom resolution:** `gc.bytes(atom) -> &[u8]` or `ctx.atom_table().bytes(atom)`. The bytes are **WTF-8** (lone surrogates encoded as 3-byte sequences, astral as surrogate-pair 3+3 or 4-byte). The C++ `dumpNode(NodeLabel)` does `json_.emitValue(label->str())` → `primitiveEmitString` → `decodeUTF8<true>` (WTF-8-aware) then `encodeUTF16` per cp. We reproduce this by decoding WTF-8 → UTF-16 (`support::utf8::convert_utf8_with_surrogates_to_utf16`) and calling `emit_u16` — exactly as the JSONParser port did (`rust/crates/parser/src/json/mod.rs:149,163`).
- **Locations:** `support::location::{SMLoc{offset:u32}, SMRange{start,end}, SourceId}`; `support::manager::SourceErrorManager` provides `find_coords(loc) -> SourceCoords{buf,line,col}` (1-based; mirrors `findBufferLineAndLoc`) and `find_buffer_for_loc(loc) -> Rc<SourceBuffer>` with `.bytes() -> &[u8]`.

### C++ dump algorithm (what we reproduce, exactly)

For a node: `openDict` → `emitKeyValue("type", <NodeName>)` → `dumpChildren` (each `.def` arg field, in declared order, via `DUMP_KEY_VALUE_PAIR`) → **NumericLiteral only:** `"raw"` from the source range when valid and `rawProp==Include` → `printSourceLocation` (`loc` then `range`) → `closeDict`.

`DUMP_KEY_VALUE_PAIR(PARENT, KEY, NODE)`: if `isEmpty(NODE)` then — `Compact` ⇒ skip; `HideEmpty` ⇒ skip iff `(PARENT,KEY)` is in the `ESTREE_IGNORE_IF_EMPTY` set; `DumpAll` ⇒ never skip. Otherwise `emitKey(KEY); dumpNode(NODE)`.

`isEmpty`: `NodeList` ⇒ `.empty()`; `NodePtr` ⇒ `== null`; `NodeBoolean` ⇒ `!val`; `NodeLabel` ⇒ **always false**; `NodeNumber` ⇒ **always false**.

`dumpNode`: `NodeList` ⇒ array of children; `NodePtr` ⇒ recurse or `null`; `NodeLabel` ⇒ string (or `null` if the label pointer is null); `NodeBoolean` ⇒ bool; `NodeNumber` ⇒ number.

**Decorations are NOT dumped** — the C++ `visitChildren` macros iterate only `ARG` (`.def`) fields, never decoration members. The generator must therefore walk only `.def`-arg fields.

### Two deliberate, model-driven deviations (document, don't fight)

1. **`raw` requires the buffer.** C++ reads `NumericLiteral` raw text straight from the location's raw pointers (`sr.Start.getPointer()`), so its no-`sm` overload still emits `"raw"`. Our offset-based `SMLoc` has no pointer, so raw text needs the buffer via `sm`. **When `sm` is `None`, `"raw"` is omitted.** The Parser-time differential uses the **with-`sm`** overload (it has the buffer + `locMode`), so byte-fidelity holds where it is actually gated. This is the same offset-vs-pointer substitution made everywhere else in the port.
2. **`StackOverflowGuard` → a plain depth counter.** C++ uses `depthCounterGuard(128)` (non-native-stack builds). We port a `depth: usize` counter with the same limit (128): on overflow emit `null` (and, if `sm` is present, `sm.error(...)` like C++). The native-stack variant is not ported (no analog; consistent with the rest of the port).

---

## File structure

- **Create** `rust/crates/support/src/utf8.rs` — a faithful copy of the decode/encode helpers needed by `convert_utf8_with_surrogates_to_utf16` (mirrors `rust/crates/parser/src/utf8.rs` and `include/hermes/Support/UTF8.h`). Lives in `support` because it is a shared, zero-`unsafe` Unicode codec that feeds `JSONEmitter` (also in `support`) and is needed by both the JSONParser port and this dumper. *(Decision for review: `support` gains a `unicode` dep; the existing `parser::utf8` copy is left untouched this phase — a later cleanup can re-export from `support`.)*
- **Modify** `rust/crates/support/src/lib.rs` — add `pub mod utf8;`.
- **Modify** `rust/crates/support/Cargo.toml` — add `unicode = { path = "../unicode" }`.
- **Modify** `rust/crates/ast/gen_nodes.py` — tag `.def`-arg fields; thread the parsed `ignore_if_empty`; derive `Hash` on `NodeKind`; emit `Node::node_type_str` + `Node::dump_children`; add `ESTREE_IGNORE_IF_EMPTY` drift validation.
- **Regenerate** `rust/crates/ast/src/node.rs` (committed `@generated`).
- **Create** `rust/crates/ast/src/dump.rs` — `ESTreeJSONDumper`, the `ESTreeDumpMode`/`LocationDumpMode`/`ESTreeRawProp` enums, the `field_*`/`dump_*` helpers, `print_source_location`, `dump_sm_range_json`, and the public `dump_estree_json` overloads.
- **Modify** `rust/crates/ast/src/lib.rs` — add `pub mod dump;`.
- **Create** `rust/crates/ast/tests/dump_golden.rs` — golden tests over hand-built trees.
- **Modify** `rust/crates/ast/tests/generated_idempotent.rs` — (no code change expected; it force-runs the generator and diffs — it will now also cover the dumper emission).

---

## Task 1: `support::utf8` WTF-8 → UTF-16 codec

**Files:**
- Create: `rust/crates/support/src/utf8.rs`
- Modify: `rust/crates/support/src/lib.rs`
- Modify: `rust/crates/support/Cargo.toml`

- [ ] **Step 1: Add the `unicode` dependency.**

In `rust/crates/support/Cargo.toml`, under `[dependencies]`, add:

```toml
unicode = { path = "../unicode" }
```

- [ ] **Step 2: Create `rust/crates/support/src/utf8.rs`.**

Copy the decode/encode helpers faithfully from `rust/crates/parser/src/utf8.rs` (which mirrors `include/hermes/Support/UTF8.h`). Only the subset `convert_utf8_with_surrogates_to_utf16` needs is required: the `at` helper, `is_utf8_start`, `decode_utf8_slow_path`, `decode_utf8`, `encode_utf16`, and `convert_utf8_with_surrogates_to_utf16`. Keep the comments.

```rust
/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! UTF-8/WTF-8 ↔ UTF-16 codec, ported from include/hermes/Support/UTF8.h
//! (decode side). A faithful copy of the subset in `parser::utf8` needed to
//! emit interned (WTF-8) AST string/label bytes as JSON — kept here in
//! `support` because it feeds `JSONEmitter` (also in `support`) and is shared
//! by the JSONParser and AST-dumper ports. Zero `unsafe` (support `forbid`s it).

use unicode::{
    UNICODE_MAX_VALUE, UNICODE_REPLACEMENT_CHARACTER, UNICODE_SURROGATE_FIRST,
    UNICODE_SURROGATE_LAST, UTF16_HIGH_SURROGATE, UTF16_LOW_SURROGATE,
};

/// Check whether a byte is a regular ASCII or a UTF8 starting byte.
#[inline]
pub fn is_utf8_start(ch: u8) -> bool {
    (ch & 0x80) != 0
}

/// Read `bytes[i]`, or `0` if `i` is out of range.
#[inline]
fn at(bytes: &[u8], i: usize) -> u32 {
    bytes.get(i).copied().unwrap_or(0) as u32
}

/// Port of `_decodeUTF8SlowPath` (UTF8.h:77-162).
pub fn decode_utf8_slow_path<const ALLOW_SURROGATES: bool>(
    bytes: &[u8],
    i: &mut usize,
    mut error: impl FnMut(&str),
) -> u32 {
    // ... copy the body verbatim from rust/crates/parser/src/utf8.rs:68-156 ...
}

/// Port of `decodeUTF8` (UTF8.h:187-193), ASCII fast path.
#[inline]
pub fn decode_utf8<const ALLOW_SURROGATES: bool>(
    bytes: &[u8],
    i: &mut usize,
    error: impl FnMut(&str),
) -> u32 {
    if *i < bytes.len() && (bytes[*i] & 0x80) == 0 {
        let c = bytes[*i] as u32;
        *i += 1;
        return c;
    }
    decode_utf8_slow_path::<ALLOW_SURROGATES>(bytes, i, error)
}

/// Port of `encodeUTF16` (UTF8.h:197-210). Surrogate-range values pass through.
#[inline]
pub fn encode_utf16(out: &mut Vec<u16>, cp: u32) {
    if cp < 0x10000 {
        out.push(cp as u16);
    } else {
        debug_assert!(cp <= UNICODE_MAX_VALUE, "invalid Unicode value");
        let cp = cp - 0x10000;
        out.push((UTF16_HIGH_SURROGATE + ((cp >> 10) & 0x3FF)) as u16);
        out.push((UTF16_LOW_SURROGATE + (cp & 0x3FF)) as u16);
    }
}

/// Decode assumed-valid (W)UTF-8 — which may contain explicitly encoded
/// surrogates — into UTF-16. Port of `convertUTF8WithSurrogatesToUTF16`
/// (UTF8.h:216-225).
pub fn convert_utf8_with_surrogates_to_utf16(bytes: &[u8]) -> Vec<u16> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        let cp = decode_utf8::<true>(bytes, &mut i, |_| {});
        encode_utf16(&mut out, cp);
    }
    out
}
```

> Copy `decode_utf8_slow_path`'s body **verbatim** from `parser/src/utf8.rs:68-156` (it references `at`, `UNICODE_REPLACEMENT_CHARACTER`, `UNICODE_SURROGATE_FIRST`, `UNICODE_SURROGATE_LAST`, `UNICODE_MAX_VALUE`, all imported above). The `debug_assert!(is_utf8_start(...))` in the slow path uses `is_utf8_start`.

- [ ] **Step 3: Register the module.**

In `rust/crates/support/src/lib.rs`, add (with the other `pub mod` lines):

```rust
pub mod utf8;
```

- [ ] **Step 4: Add a unit test inside `utf8.rs`.**

```rust
#[cfg(test)]
mod tests {
    use super::convert_utf8_with_surrogates_to_utf16;

    #[test]
    fn ascii_passthrough() {
        assert_eq!(convert_utf8_with_surrogates_to_utf16(b"abc"), vec![0x61, 0x62, 0x63]);
    }

    #[test]
    fn bmp_non_ascii() {
        // U+54C8 哈 = e5 93 88
        assert_eq!(convert_utf8_with_surrogates_to_utf16(&[0xE5, 0x93, 0x88]), vec![0x54C8]);
    }

    #[test]
    fn astral_4byte() {
        // U+1F44B 👋 = f0 9f 91 8b -> surrogate pair D83D DC4B
        assert_eq!(
            convert_utf8_with_surrogates_to_utf16(&[0xF0, 0x9F, 0x91, 0x8B]),
            vec![0xD83D, 0xDC4B]
        );
    }

    #[test]
    fn wtf8_lone_surrogate() {
        // Lone high surrogate U+D800 as WTF-8 = ed a0 80 -> single unit 0xD800
        assert_eq!(convert_utf8_with_surrogates_to_utf16(&[0xED, 0xA0, 0x80]), vec![0xD800]);
    }
}
```

- [ ] **Step 5: Build and test.**

Run: `cargo test --manifest-path rust/Cargo.toml -p support utf8 -- --nocapture`
Expected: PASS (4 new tests); `cargo build --manifest-path rust/Cargo.toml -p support` → zero warnings.

- [ ] **Step 6: Commit.**

```bash
git add rust/crates/support/src/utf8.rs rust/crates/support/src/lib.rs rust/crates/support/Cargo.toml
git commit -m "$(cat <<'EOF'
rust(support): WTF-8 -> UTF-16 codec (utf8.rs) for JSON string emission

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Dumper core — generator emission + `dump.rs`

This task introduces both halves of the dumper (the generated `Node::dump_children`/`node_type_str` and the hand-written `ESTreeJSONDumper`). They are mutually dependent, so they land in one commit, ending green with a smoke golden test.

**Files:**
- Modify: `rust/crates/ast/gen_nodes.py`
- Regenerate: `rust/crates/ast/src/node.rs`
- Create: `rust/crates/ast/src/dump.rs`
- Modify: `rust/crates/ast/src/lib.rs`
- Test: `rust/crates/ast/tests/dump_golden.rs` (smoke test only in this task)

- [ ] **Step 1: Write the failing smoke test.**

Create `rust/crates/ast/tests/dump_golden.rs`:

```rust
//! Golden tests for ESTreeJSONDumper (ast phase 4). Trees are hand-built in a
//! Context/GCLock; output is asserted byte-for-byte.
use ast::context::{Context, GCLock};
use ast::dump::{dump_estree_json, ESTreeDumpMode};
use ast::node::{Identifier, Node, NumericLiteral, Program};
use ast::node_child::{NodeList, NodeMetadata};
use support::location::{SMLoc, SMRange};

fn rng(a: u32, b: u32) -> SMRange {
    SMRange { start: SMLoc { offset: a }, end: SMLoc { offset: b } }
}

/// Dump `root` with no source manager, compact JSON.
fn dump(gc: &GCLock, root: &Node, mode: ESTreeDumpMode) -> String {
    let mut out = String::new();
    dump_estree_json(&mut out, root, /*pretty=*/false, mode, gc.ctx().atom_table());
    out
}

#[test]
fn smoke_numeric_literal() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let num = gc.alloc(Node::NumericLiteral(NumericLiteral::new(
        NodeMetadata::new(rng(0, 3)),
        1.5,
    )));
    // No sm -> no "raw", no locations. JSONL appends a trailing newline.
    assert_eq!(dump(&gc, num, ESTreeDumpMode::Compact), "{\"type\":\"NumericLiteral\",\"value\":1.5}\n");
}
```

> Verify the exact `Program`/`Identifier`/`NumericLiteral::new` signatures and field names against the generated `src/node.rs` before relying on them (e.g. `Identifier`'s `.def` args are `name`, `typeAnnotation?`, `optional`). Adjust the smoke test imports/usage to whatever compiles. The `NumericLiteral` arg is its single `.def` field `value: f64`.

- [ ] **Step 2: Run it — expect a compile error.**

Run: `cargo test --manifest-path rust/Cargo.toml -p ast --test dump_golden`
Expected: FAIL to compile (`ast::dump` does not exist).

- [ ] **Step 3: Generator — tag `.def`-arg fields and a dump-kind.**

In `gen_nodes.py`, edit `def_field_descriptor` (~159) so every descriptor it returns also carries `is_def_arg=True` and a `dump_kind`. Replace its bodies' `return dict(...)` calls to add these keys:

- `NodePtr` optional → `dump_kind="node_opt"`
- `NodePtr` required → `dump_kind="node_single"`
- `NodeList` → `dump_kind="list"`
- value types: `NodeBoolean` → `dump_kind="bool"`, `NodeNumber` → `dump_kind="number"`, `NodeLabel`/`NodeString` → `dump_kind="label"`

Concretely, change each `return dict(...)` to include `is_def_arg=True, dump_kind="<kind>"`. Example for the required-`NodePtr` arm:

```python
        rtype = "&'gc Node<'gc>"
        return dict(json_name=fname, rust_field=rf, rust_type=rtype,
                    child_kind="single", new_arg_type=rtype,
                    cell=False, default_expr=None,
                    is_def_arg=True, dump_kind="node_single")
```

And the value-types tail:

```python
    return dict(json_name=fname, rust_field=rf,
                rust_type=f"Cell<{inner}>", child_kind="none",
                new_arg_type=inner, cell=True, default_expr=None,
                is_def_arg=True,
                dump_kind={"NodeBoolean": "bool", "NodeNumber": "number",
                           "NodeLabel": "label", "NodeString": "label"}[ftype])
```

In `decoration_descriptor` (~303) add `is_def_arg=False, dump_kind=None` to its returned dict. In `compose_fields` (~380), add `is_def_arg=False, dump_kind=None` to the `metadata` descriptor dict.

- [ ] **Step 4: Generator — derive `Hash` on `NodeKind`.**

In `emit_node_kind` (~433), change the derive line so `NodeKind` can live in a `HashSet` (for `includeSourceLocs`):

```python
    out.append("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]")
```

- [ ] **Step 5: Generator — thread `ignore_if_empty` and validate it.**

In `generate()` (~765), stop discarding the parsed map and validate it. Replace:

```python
    items, _ignore_if_empty = parse_def(src)  # ignore_if_empty: used in phase 4 (ESTree JSON dumper)
```
with:
```python
    items, ignore_if_empty = parse_def(src)
```

After the node list is composed (after the `for it in items:` loop that builds `nodes`, ~779) and before/near the other validations (~789), add a drift check that every `ESTREE_IGNORE_IF_EMPTY(NODE, FIELD)` names a real node and one of its `.def`-arg fields (by camelCase json name):

```python
    # Validate ESTREE_IGNORE_IF_EMPTY against the real nodes/fields (drift guard).
    fields_by_node = {name: fields for name, fields in nodes}
    for node_name, field_list in ignore_if_empty.items():
        if node_name not in fields_by_node:
            sys.exit(f"error: IGNORE_IF_EMPTY names unknown node {node_name!r}")
        argnames = {fd["json_name"] for fd in fields_by_node[node_name]
                    if fd.get("is_def_arg")}
        for f in field_list:
            if f not in argnames:
                sys.exit(f"error: IGNORE_IF_EMPTY({node_name},{f}) is not a .def arg field")
```

- [ ] **Step 6: Generator — emit `node_type_str` and `dump_children`.**

Add two emitter functions and call them inside `emit_accessors` (after the `mark_lists` emission, before the closing `}` of the impl block — i.e. right after the `emit_mark_lists(nodes, out)` call at ~548). First, add the calls in `emit_accessors`:

```python
    # node_type_str + dump_children (phase 4 — the JSON dumper walk).
    out.append("")
    emit_node_type_str(nodes, out)
    out.append("")
    emit_dump_children(nodes, out)
```

Then define the two functions (place them near `emit_mark_lists`):

```python
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
```

> The `_ignore` flag must be stamped onto each field descriptor before emission. In `generate()`, right after the `nodes` list is composed and `ignore_if_empty` validated (Step 5), stamp it:
>
> ```python
>     for name, fields in nodes:
>         ign = set(ignore_if_empty.get(name, ()))
>         for fd in fields:
>             fd["_ignore"] = fd.get("is_def_arg") and fd["json_name"] in ign
> ```

- [ ] **Step 7: Regenerate `node.rs`.**

Run: `python3 rust/crates/ast/gen_nodes.py`
Expected stderr: `gen_nodes.py: 271 nodes, N ranges -> .../src/node.rs`. (If the `IGNORE_IF_EMPTY` validation fails, the camelCase field name in `ESTree.def` and the descriptor `json_name` should match — debug there.)

- [ ] **Step 8: Create `rust/crates/ast/src/dump.rs`.**

```rust
/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Port of `lib/AST/ESTreeJSONDumper.cpp`. Emits an AST as ESTree JSON — the
//! byte-for-byte differential-oracle surface (the gate lands at Parser time).
//! The per-kind field walk + the `"type"` name live in the generated
//! `node.rs` (`Node::dump_children` / `Node::node_type_str`); this module is the
//! driver: modes, locations, the `raw` prop, value emission, and the public
//! entry points.

use std::collections::HashSet;

use atom_table::{AtomTable, AtomBytes, INVALID_ATOM_BYTES};
use support::json_emitter::JSONEmitter;
use support::location::{SMLoc, SMRange};
use support::manager::SourceErrorManager;

use crate::node::{Node, NodeKind};
use crate::node_child::{NodeLabel, NodeList};

/// Which fields to dump. Mirrors `ESTreeDumpMode`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ESTreeDumpMode {
    /// Hide every empty field.
    Compact,
    /// Hide empty fields that are in the `ESTREE_IGNORE_IF_EMPTY` set.
    HideEmpty,
    /// Force-dump all fields.
    DumpAll,
}

/// Which location info to dump. Mirrors `LocationDumpMode`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LocationDumpMode {
    None,
    Loc,
    Range,
    LocAndRange,
}

/// Whether to include the `"raw"` property where available. Mirrors `ESTreeRawProp`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ESTreeRawProp {
    Exclude,
    Include,
}

/// Depth limit mirroring C++ `depthCounterGuard(128)`.
const MAX_DEPTH: usize = 128;

/// The dumper. `'a` borrows the emitter/atoms/sm/filter; node refs are passed
/// per-call (generic over their own lifetime).
pub struct ESTreeJSONDumper<'a, 'w> {
    json: &'a mut JSONEmitter<'w>,
    atoms: &'a AtomTable,
    sm: Option<&'a SourceErrorManager>,
    mode: ESTreeDumpMode,
    loc_mode: LocationDumpMode,
    raw_prop: ESTreeRawProp,
    include_source_locs: Option<&'a HashSet<NodeKind>>,
    depth: usize,
}

impl<'a, 'w> ESTreeJSONDumper<'a, 'w> {
    /// Whether `DUMP_KEY_VALUE_PAIR` would skip an empty field.
    fn skip_empty(&self, is_empty: bool, ignore_if_empty: bool) -> bool {
        if !is_empty {
            return false;
        }
        match self.mode {
            ESTreeDumpMode::Compact => true,
            ESTreeDumpMode::HideEmpty => ignore_if_empty,
            ESTreeDumpMode::DumpAll => false,
        }
    }

    // --- field_* helpers, called from the generated Node::dump_children. ---

    pub fn field_node<'n>(&mut self, key: &str, node: Option<&'n Node<'n>>, ignore: bool) {
        if self.skip_empty(node.is_none(), ignore) {
            return;
        }
        self.json.emit_key(key);
        self.dump_node_ptr(node);
    }

    pub fn field_list<'n>(&mut self, key: &str, list: NodeList<'n>, ignore: bool) {
        if self.skip_empty(list.is_empty(), ignore) {
            return;
        }
        self.json.emit_key(key);
        self.dump_node_list(list);
    }

    pub fn field_bool(&mut self, key: &str, val: bool, ignore: bool) {
        // isEmpty(NodeBoolean) == !val
        if self.skip_empty(!val, ignore) {
            return;
        }
        self.json.emit_key(key);
        self.json.emit_bool(val);
    }

    pub fn field_number(&mut self, key: &str, val: f64, ignore: bool) {
        // isEmpty(NodeNumber) == false (never empty).
        if self.skip_empty(false, ignore) {
            return;
        }
        self.json.emit_key(key);
        self.json.emit_f64(val);
    }

    pub fn field_label(&mut self, key: &str, label: NodeLabel, ignore: bool) {
        // isEmpty(NodeLabel) == false (never empty).
        if self.skip_empty(false, ignore) {
            return;
        }
        self.json.emit_key(key);
        self.dump_label(label);
    }

    // --- dumpNode overloads. ---

    fn dump_node_ptr<'n>(&mut self, node: Option<&'n Node<'n>>) {
        let node = match node {
            Some(n) => n,
            None => {
                self.json.emit_null_value();
                return;
            }
        };
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            self.json.emit_null_value();
            if let Some(sm) = self.sm {
                // C++ errors at node->getEndLoc(); we have the range's end.
                let _ = sm; // sm.error requires &mut; see note below.
            }
            self.depth -= 1;
            return;
        }
        self.visit(node);
        self.depth -= 1;
    }

    fn dump_node_list<'n>(&mut self, list: NodeList<'n>) {
        self.json.open_array();
        for n in list.iter() {
            self.dump_node_ptr(Some(n));
        }
        self.json.close_array();
    }

    fn dump_label(&mut self, label: AtomBytes) {
        if label == INVALID_ATOM_BYTES {
            self.json.emit_null_value();
            return;
        }
        let bytes = self.atoms.bytes(label);
        let units = support::utf8::convert_utf8_with_surrogates_to_utf16(bytes);
        self.json.emit_u16(&units);
    }

    // --- visit + locations + raw. ---

    fn visit<'n>(&mut self, node: &'n Node<'n>) {
        self.json.open_dict();
        self.json.emit_key("type");
        self.json.emit_str(node.node_type_str());
        node.dump_children(self);
        if node.kind() == NodeKind::NumericLiteral && self.raw_prop == ESTreeRawProp::Include {
            self.dump_raw(node);
        }
        self.print_source_location(node);
        self.json.close_dict();
    }

    /// NumericLiteral `"raw"` — the source text. Requires `sm` (offset model
    /// has no location pointer); omitted when `sm` is None (documented deviation).
    fn dump_raw<'n>(&mut self, node: &'n Node<'n>) {
        let sm = match self.sm {
            Some(sm) => sm,
            None => return,
        };
        let r = node.range();
        if !range_is_valid(r) {
            return;
        }
        let buf = sm.find_buffer_for_loc(r.start);
        let bytes = &buf.bytes()[r.start.offset as usize..r.end.offset as usize];
        self.json.emit_key("raw");
        // Numeric-literal source text is ASCII; route through the WTF-8 codec
        // for uniformity with C++ primitiveEmitString.
        let units = support::utf8::convert_utf8_with_surrogates_to_utf16(bytes);
        self.json.emit_u16(&units);
    }

    fn print_source_location<'n>(&mut self, node: &'n Node<'n>) {
        if self.loc_mode == LocationDumpMode::None {
            return;
        }
        if let Some(set) = self.include_source_locs {
            if !set.contains(&node.kind()) {
                return;
            }
        }
        let sm = match self.sm {
            Some(sm) => sm,
            None => return,
        };
        let r = node.range();
        if !range_is_valid(r) {
            return;
        }
        let start = sm.find_coords(r.start);
        let end = sm.find_coords(r.end);

        if matches!(self.loc_mode, LocationDumpMode::Loc | LocationDumpMode::LocAndRange) {
            self.json.emit_key("loc");
            self.json.open_dict();
            self.json.emit_key("start");
            self.json.open_dict();
            self.json.emit_key("line");
            self.json.emit_u64(start.line as u64);
            self.json.emit_key("column");
            self.json.emit_u64(start.col as u64);
            self.json.close_dict();
            self.json.emit_key("end");
            self.json.open_dict();
            self.json.emit_key("line");
            self.json.emit_u64(end.line as u64);
            self.json.emit_key("column");
            self.json.emit_u64(end.col as u64);
            self.json.close_dict();
            self.json.close_dict();
        }

        if matches!(self.loc_mode, LocationDumpMode::Range | LocationDumpMode::LocAndRange) {
            self.json.emit_key("range");
            self.json.open_array();
            dump_sm_range_json(self.json, r);
            self.json.close_array();
        }
    }
}

/// Whether a range is set (mirrors C++ `SMRange::isValid()`). Adjust to the
/// actual `SMLoc`/`SMRange` validity convention in `support::location`.
fn range_is_valid(r: SMRange) -> bool {
    r.start.offset <= r.end.offset
}

/// Emit a range as the two buffer-relative offsets. Port of `dumpSMRangeJSON`
/// (the caller wraps these in an array). In the offset model the offsets are the
/// values directly.
pub fn dump_sm_range_json(json: &mut JSONEmitter, rng: SMRange) {
    json.emit_u64(rng.start.offset as u64);
    json.emit_u64(rng.end.offset as u64);
}

// --- public entry points (mirror the C++ dumpESTreeJSON overloads). ---

/// Dump `root` to `out` without locations. Mirrors the no-`sm`
/// `dumpESTreeJSON(os, root, pretty, mode)` — `"raw"` is omitted (no buffer).
pub fn dump_estree_json<'n>(
    out: &mut String,
    root: &'n Node<'n>,
    pretty: bool,
    mode: ESTreeDumpMode,
    atoms: &AtomTable,
) {
    let mut json = JSONEmitter::new(out, pretty);
    {
        let mut d = ESTreeJSONDumper {
            json: &mut json,
            atoms,
            sm: None,
            mode,
            loc_mode: LocationDumpMode::None,
            raw_prop: ESTreeRawProp::Include,
            include_source_locs: None,
            depth: 0,
        };
        d.dump_node_ptr(Some(root));
    }
    json.end_jsonl();
}

/// Dump `root` with a source manager and a location mode. Mirrors the
/// `dumpESTreeJSON(os, root, pretty, mode, sm, locMode, rawProp)` overload.
#[allow(clippy::too_many_arguments)]
pub fn dump_estree_json_with_sm<'n>(
    out: &mut String,
    root: &'n Node<'n>,
    pretty: bool,
    mode: ESTreeDumpMode,
    sm: &SourceErrorManager,
    loc_mode: LocationDumpMode,
    raw_prop: ESTreeRawProp,
    atoms: &AtomTable,
) {
    let mut json = JSONEmitter::new(out, pretty);
    {
        let mut d = ESTreeJSONDumper {
            json: &mut json,
            atoms,
            sm: Some(sm),
            mode,
            loc_mode,
            raw_prop,
            include_source_locs: None,
            depth: 0,
        };
        d.dump_node_ptr(Some(root));
    }
    json.end_jsonl();
}
```

> **Lifetime note:** the exact `'a`/`'w`/`'n` shapes may need small adjustments to satisfy the borrow checker (e.g. tying `json`'s inner lifetime). Keep `ESTreeJSONDumper`'s public name and `field_*` method names stable — the generated `node.rs` references `crate::dump::ESTreeJSONDumper` and `d.field_node/field_list/field_bool/field_number/field_label`.
>
> **`sm.error` note:** C++ calls `sm_->error(...)` on overflow, which needs `&mut`. Our dumper holds `&SourceErrorManager` (shared) to also resolve coords. Rather than thread `&mut`, leave the overflow path emitting `null` only and drop the diagnostic (the 128-depth guard is a safety net, not a tested surface — note this in a comment). If a future consumer needs the diagnostic, switch to a `&mut` sm and re-resolve coords through it.
>
> **`range_is_valid` / `find_coords` / `bytes` note:** confirm `SMRange` validity convention and that `find_coords` matches `findBufferLineAndLoc`'s line/col semantics against `support::manager` before trusting the loc output (the loc/range/raw paths are exercised by Task 3 golden tests, not yet by a differential).

- [ ] **Step 9: Register the module.**

In `rust/crates/ast/src/lib.rs`, add:

```rust
pub mod dump;
```

- [ ] **Step 10: Build, then run the smoke test.**

Run: `cargo build --manifest-path rust/Cargo.toml -p ast`
Expected: compiles, zero warnings.
Run: `cargo test --manifest-path rust/Cargo.toml -p ast --test dump_golden`
Expected: PASS (`smoke_numeric_literal`).

- [ ] **Step 11: Run the idempotency guard (the generator output must be self-consistent).**

Run: `REQUIRE_GEN=1 cargo test --manifest-path rust/Cargo.toml -p ast --test generated_idempotent`
Expected: PASS (regenerate-and-diff is clean).

- [ ] **Step 12: Commit.**

```bash
git add rust/crates/ast/gen_nodes.py rust/crates/ast/src/node.rs rust/crates/ast/src/dump.rs rust/crates/ast/src/lib.rs rust/crates/ast/tests/dump_golden.rs
git commit -m "$(cat <<'EOF'
rust(ast): ESTreeJSONDumper core — generated dump walk + dump.rs driver

Generator emits Node::node_type_str + Node::dump_children (baking camelCase
JSON keys + per-field IGNORE_IF_EMPTY flags); dump.rs ports the modes, the
raw/location logic, value emission, and the public entry points.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Golden tests — modes, IGNORE_IF_EMPTY, locations, raw, WTF-8, pretty

**Files:**
- Modify: `rust/crates/ast/tests/dump_golden.rs`

Add the helpers and tests below. Each asserts a byte-exact string. **Before writing each `assert_eq!`, build the tree and `println!` the actual output once (run with `-- --nocapture`) to capture the exact bytes, then verify each token against the C++ algorithm above** — do not guess field order; it follows `.def` declaration order in the generated `node.rs`.

- [ ] **Step 1: Empty/absent fields across the three modes.**

Build an `Identifier` with `name="x"`, `typeAnnotation=None`, `optional=false` (a `NodeBoolean`). Per `ESTree.def`: `Identifier` has `.def` args `name` (NodeLabel), `typeAnnotation` (NodePtr, opt), `optional` (NodeBoolean); both `typeAnnotation` and `optional` are in `IGNORE_IF_EMPTY`.

```rust
#[test]
fn identifier_modes() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let name = gc.atom_bytes("x".as_bytes());
    let id = gc.alloc(Node::Identifier(Identifier::new(
        NodeMetadata::new(rng(0, 1)),
        name,
        /*type_annotation=*/ None,
        /*optional=*/ false,
    )));
    // Compact: empty typeAnnotation (null) and optional (false) both hidden.
    assert_eq!(dump(&gc, id, ESTreeDumpMode::Compact),
        "{\"type\":\"Identifier\",\"name\":\"x\"}\n");
    // HideEmpty: both are in IGNORE_IF_EMPTY -> also hidden.
    assert_eq!(dump(&gc, id, ESTreeDumpMode::HideEmpty),
        "{\"type\":\"Identifier\",\"name\":\"x\"}\n");
    // DumpAll: both shown (null / false).
    assert_eq!(dump(&gc, id, ESTreeDumpMode::DumpAll),
        "{\"type\":\"Identifier\",\"name\":\"x\",\"typeAnnotation\":null,\"optional\":false}\n");
}
```

> Confirm the generated `Identifier::new` parameter order/names from `node.rs` (snake_case: `type_annotation`). If `optional` is keyword-escaped or named differently, adjust.

- [ ] **Step 2: A non-IGNORE_IF_EMPTY empty field differs between Compact and HideEmpty.**

Pick a node with an empty `NodeList` child that is **not** in `IGNORE_IF_EMPTY` (e.g. `Program.body` — `Program` has `.def` arg `body` (NodeList), not ignored). Build an empty `Program`:

```rust
#[test]
fn program_empty_body_modes() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let prog = gc.alloc(Node::Program(Program::new(
        NodeMetadata::new(rng(0, 0)),
        NodeList::empty(),
    )));
    // Compact hides the empty list; HideEmpty keeps it (body not in IGNORE_IF_EMPTY).
    assert_eq!(dump(&gc, prog, ESTreeDumpMode::Compact), "{\"type\":\"Program\"}\n");
    assert_eq!(dump(&gc, prog, ESTreeDumpMode::HideEmpty), "{\"type\":\"Program\",\"body\":[]}\n");
}
```

- [ ] **Step 3: Nested children + a non-empty list.**

Build a `Program` whose `body` is `[ExpressionStatement(NumericLiteral(1))]` (or the simplest available statement wrapping an expression — verify node names/args in `node.rs`). Assert the nested array + object structure byte-for-byte (capture-then-verify).

- [ ] **Step 4: Pretty-printing.**

Re-dump the Step-3 tree with `pretty=true`; assert the exact indented form (2-space indent, newlines per the `JSONEmitter` pretty rules). Capture-then-verify.

- [ ] **Step 5: WTF-8 / astral label.**

Intern a label containing an astral char and a lone surrogate via the lexer's WTF-8 encoding, dump an `Identifier`/`StringLiteral` with it, and assert the `\uXXXX`-escaped output. Build the WTF-8 bytes explicitly:

```rust
#[test]
fn wtf8_string_value() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    // "a" + U+1F44B (astral) encoded as 4-byte UTF-8 + lone high surrogate WTF-8.
    let bytes: &[u8] = &[b'a', 0xF0, 0x9F, 0x91, 0x8B, 0xED, 0xA0, 0x80];
    let s = gc.atom_bytes(bytes);
    let lit = gc.alloc(Node::StringLiteral(ast::node::StringLiteral::new(
        NodeMetadata::new(rng(0, 1)),
        s,
    )));
    // a -> a; U+1F44B -> 👋; lone surrogate -> \ud800
    assert_eq!(dump(&gc, lit, ESTreeDumpMode::Compact),
        "{\"type\":\"StringLiteral\",\"value\":\"a\\ud83d\\udc4b\\ud800\"}\n");
}
```

> `StringLiteral`'s `.def` arg is `value` (NodeString). Confirm the variant/field names.

- [ ] **Step 6: Locations + raw with a source manager.**

Register a buffer in a `SourceErrorManager`, build a `NumericLiteral` whose range matches the buffer text `"1.5"`, and dump with `dump_estree_json_with_sm` in `LocAndRange` mode, `ESTreeRawProp::Include`. Assert `"raw":"1.5"`, the `loc` `{start:{line,column},end:{...}}`, and `range:[start,end]`. Use offsets that index into the registered buffer.

```rust
#[test]
fn numeric_literal_loc_range_raw() {
    use ast::dump::{dump_estree_json_with_sm, ESTreeRawProp, LocationDumpMode};
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("test.js", "1.5");
    let mut ctx = Context::new();
    let gc = ctx.lock();
    // Range over offsets [0,3) in buffer `id`.
    let r = SMRange { start: SMLoc { offset: 0 }, end: SMLoc { offset: 3 } };
    let num = gc.alloc(Node::NumericLiteral(NumericLiteral::new(
        NodeMetadata::new(r), 1.5,
    )));
    let mut out = String::new();
    dump_estree_json_with_sm(&mut out, num, false, ESTreeDumpMode::Compact,
        &sm, LocationDumpMode::LocAndRange, ESTreeRawProp::Include, gc.ctx().atom_table());
    // Capture-then-verify the exact loc/column convention before locking this in.
    assert_eq!(out,
        "{\"type\":\"NumericLiteral\",\"value\":1.5,\"raw\":\"1.5\",\"loc\":{\"start\":{\"line\":1,\"column\":1},\"end\":{\"line\":1,\"column\":4}},\"range\":[0,3]}\n");
}
```

> **Important:** the exact `column` values depend on `find_coords`' 1-based column convention; capture the real output first and confirm it matches `findBufferLineAndLoc`. If `SMLoc { offset }` needs the buffer's `SourceId` baked in (i.e. `SMLoc` is `(SourceId, offset)` rather than a bare offset), construct locations via the appropriate constructor so `find_buffer_for_loc`/`find_coords` resolve to buffer `id`. Verify the `SMLoc` shape in `support::location` and adjust `rng`/this test accordingly.

- [ ] **Step 7: `ESTreeRawProp::Exclude` and the no-sm raw omission.**

Assert that with `ESTreeRawProp::Exclude` (with sm) there is no `"raw"`, and that the no-sm `dump_estree_json` also omits `"raw"` for the same `NumericLiteral` (documented deviation).

- [ ] **Step 8: Run all golden tests.**

Run: `cargo test --manifest-path rust/Cargo.toml -p ast --test dump_golden -- --nocapture`
Expected: all PASS.

- [ ] **Step 9: Commit.**

```bash
git add rust/crates/ast/tests/dump_golden.rs
git commit -m "$(cat <<'EOF'
rust(ast): ESTreeJSONDumper golden tests — modes, IGNORE_IF_EMPTY, loc/range/raw, WTF-8, pretty

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Whole-workspace verification + docs + capstone

**Files:**
- Modify: `doc/superpowers/RustPortRoadmap.md`
- Modify: `doc/superpowers/SESSION-HANDOFF.md`
- Modify: `doc/superpowers/specs/2026-06-03-ast-design.md` (status line: phase 4 complete)
- Update the auto-memory `rust-port-roadmap-pointer.md` resume pointer.

- [ ] **Step 1: Full workspace build + test.**

Run: `cargo build --manifest-path rust/Cargo.toml`
Expected: zero warnings.
Run: `cargo test --manifest-path rust/Cargo.toml`
Expected: all green (the prior ~229 tests + the new `support::utf8` + `dump_golden` tests).

- [ ] **Step 2: Clippy on the touched crates.**

Run: `cargo clippy --manifest-path rust/Cargo.toml -p ast -p support`
Expected: no new lints (only pre-existing faithful-C-idiom ones, if any).

- [ ] **Step 3: Re-confirm the idempotency guard.**

Run: `REQUIRE_GEN=1 cargo test --manifest-path rust/Cargo.toml -p ast --test generated_idempotent`
Expected: PASS.

- [ ] **Step 4: Capstone review (whole component).**

Per the SESSION-HANDOFF §5 workflow, run the whole-component capstone review. Specifically verify, by reading the generated `node.rs` dump arms against `ESTreeJSONDumper.cpp`:
- The `"type"` value equals the C++ `#NAME` for every kind.
- Field **order** matches `.def` declaration order; **only** `.def`-arg fields are dumped (no decorations).
- `isEmpty` semantics: list (`is_empty()`), opt-node (`is_none()`), bool (`!val`), label/number never skipped by emptiness.
- The `IGNORE_IF_EMPTY` flag is baked on exactly the `(node,field)` pairs from `ESTree.def` (spot-check `Identifier.optional/typeAnnotation`, `FunctionDeclaration.{typeParameters,returnType,predicate}`, `ClassDeclaration.*`, `BlockStatement.implicit`).
- `NumericLiteral` is the only kind with `"raw"`; emission order is type → children → raw → loc → range.
- The two documented deviations (no-sm raw omission; depth-counter guard) are the only ones.
- **Structural-fidelity:** this phase adds no C++ `template` specializations to port (the dumper's `visit`/`dumpNode` are runtime switches in C++), so there is no template-to-generic risk here — note this explicitly in the review.

- [ ] **Step 5: Update the roadmap + handoff + spec + memory.**

Mark AST phase 4 (and thus the whole AST component) complete in `RustPortRoadmap.md` and `SESSION-HANDOFF.md`; flip the spec status; point the next component at the **JS Parser** (which provides the producer and lands the byte-for-byte `-dump-ast` differential as its gate). Update `rust-port-roadmap-pointer.md` to: AST complete; next = JS Parser.

- [ ] **Step 6: Commit the docs.**

```bash
git add doc/superpowers/RustPortRoadmap.md doc/superpowers/SESSION-HANDOFF.md doc/superpowers/specs/2026-06-03-ast-design.md
git commit -m "$(cat <<'EOF'
doc(rust): AST phase 4 (ESTreeJSONDumper) complete — AST component done; Parser next

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Self-review notes (author check against the spec)

- **Spec coverage** (`specs/2026-06-03-ast-design.md` §4 + handoff §6): generator emits the dumper ✓ (Task 2); camelCase `.def` names baked as JSON keys ✓ (`json_name` used verbatim); `ESTREE_IGNORE_IF_EMPTY` honored ✓ (parsed, validated, baked per field); golden tests over hand-built trees ✓ (Task 3); the `-dump-ast` differential is correctly deferred to Parser time ✓ (noted, not built here).
- **Full public surface** (the "implement completely" rule): all three `ESTreeDumpMode`s, all four `LocationDumpMode`s, both `ESTreeRawProp`s, `includeSourceLocs` filter, the `dump_sm_range_json` helper, and the no-sm + with-sm entry points are implemented. The third C++ overload (`dumpESTreeJSON(JSONEmitter&, ...)` taking a caller-owned emitter + `includeSourceLocs`) can be added as a thin `dump_estree_json_into(json, root, ...)` if a consumer needs it — note it in the capstone if you add it; otherwise the two value-returning entry points cover the tested surface.
- **Deviations** are exactly two and model-driven (raw needs the buffer; depth counter for the stack guard) — both documented in code + capstone.
- **Type/name consistency:** `ESTreeJSONDumper`, `field_node/field_list/field_bool/field_number/field_label`, `node_type_str`, `dump_children` are referenced identically in the generator emission (Task 2 Step 6) and `dump.rs` (Task 2 Step 8).
