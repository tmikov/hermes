/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Golden tests for ESTreeJSONDumper (ast phase 4). Trees are hand-built in a
//! Context/GCLock; output is asserted byte-for-byte.
use hermes_ast::context::{Context, GCLock};
use hermes_ast::dump::{
    dump_estree_json, dump_estree_json_with_sm, ESTreeDumpMode, ESTreeRawProp, LocationDumpMode,
};
use hermes_ast::node::{
    ExpressionStatement, Identifier, Node, NumericLiteral, Program, StringLiteral,
};
use hermes_ast::node_child::{NodeList, NodeMetadata};
use hermes_atom_table::INVALID_ATOM_BYTES;
use hermes_support::location::{SMLoc, SMRange, SourceId};
use hermes_support::manager::SourceErrorManager;

/// Build an `SMRange` over `[a, b)` in a placeholder buffer. Without a source
/// manager the buffer is never consulted, so any `SourceId` works.
fn rng(a: u32, b: u32) -> SMRange {
    let src = SourceId::from_index(0);
    SMRange {
        start: SMLoc {
            source: src,
            offset: a,
        },
        end: SMLoc {
            source: src,
            offset: b,
        },
    }
}

/// Build an `SMRange` over `[a, b)` in the given source buffer `id`.
fn rng_id(id: SourceId, a: u32, b: u32) -> SMRange {
    SMRange {
        start: SMLoc {
            source: id,
            offset: a,
        },
        end: SMLoc {
            source: id,
            offset: b,
        },
    }
}

/// Dump `root` with no source manager, compact JSON.
fn dump<'a>(gc: &GCLock, root: &'a Node<'a>, mode: ESTreeDumpMode) -> String {
    let mut out = String::new();
    dump_estree_json(&mut out, root, /*pretty=*/ false, mode, gc.ctx().atom_table());
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
    assert_eq!(
        dump(&gc, num, ESTreeDumpMode::Compact),
        "{\"type\":\"NumericLiteral\",\"value\":1.5}\n"
    );
}

/// Step 1: empty/absent fields across the three modes. `Identifier` has `.def`
/// args `name` (NodeLabel), `typeAnnotation` (NodePtr, opt), `optional`
/// (NodeBoolean); both `typeAnnotation` and `optional` are IGNORE_IF_EMPTY.
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
    assert_eq!(
        dump(&gc, id, ESTreeDumpMode::Compact),
        "{\"type\":\"Identifier\",\"name\":\"x\"}\n"
    );
    // HideEmpty: both are in IGNORE_IF_EMPTY -> also hidden.
    assert_eq!(
        dump(&gc, id, ESTreeDumpMode::HideEmpty),
        "{\"type\":\"Identifier\",\"name\":\"x\"}\n"
    );
    // DumpAll: both shown (null / false), in .def order.
    assert_eq!(
        dump(&gc, id, ESTreeDumpMode::DumpAll),
        "{\"type\":\"Identifier\",\"name\":\"x\",\"typeAnnotation\":null,\"optional\":false}\n"
    );
}

/// Step 2: a non-IGNORE_IF_EMPTY empty field differs between Compact and
/// HideEmpty. `Program.body` is a NodeList that is NOT in IGNORE_IF_EMPTY.
#[test]
fn program_empty_body_modes() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let prog = gc.alloc(Node::Program(Program::new(
        NodeMetadata::new(rng(0, 0)),
        NodeList::empty(),
    )));

    // Compact hides the empty list; HideEmpty keeps it (body not IGNORE_IF_EMPTY).
    assert_eq!(
        dump(&gc, prog, ESTreeDumpMode::Compact),
        "{\"type\":\"Program\"}\n"
    );
    assert_eq!(
        dump(&gc, prog, ESTreeDumpMode::HideEmpty),
        "{\"type\":\"Program\",\"body\":[]}\n"
    );
}

/// Build a `Program` whose body is `[ExpressionStatement(NumericLiteral(1))]`.
/// `ExpressionStatement` has `.def` args `expression` (NodePtr) and `directive`
/// (NodeString); `directive` is never IGNORE_IF_EMPTY-skipped (labels are never
/// "empty"), so passing the INVALID sentinel emits `null`.
fn build_nested_program<'s, 'ast, 'ctx>(gc: &'s GCLock<'ast, 'ctx>) -> &'s Node<'s> {
    let num = gc.alloc(Node::NumericLiteral(NumericLiteral::new(
        NodeMetadata::new(rng(0, 1)),
        1.0,
    )));
    let stmt = gc.alloc(Node::ExpressionStatement(ExpressionStatement::new(
        NodeMetadata::new(rng(0, 1)),
        num,
        /*directive=*/ INVALID_ATOM_BYTES,
    )));
    let body = NodeList::from_iter(gc, [stmt]);
    gc.alloc(Node::Program(Program::new(NodeMetadata::new(rng(0, 1)), body)))
}

/// Step 3: nested children + a non-empty list.
#[test]
fn program_nested_children() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let prog = build_nested_program(&gc);

    // Program.body -> [ExpressionStatement{expression: NumericLiteral{value:1},
    //   directive: null}]. NumericLiteral is the leaf; directive is null because
    // the INVALID sentinel resolves to JSON null in dump_label.
    assert_eq!(
        dump(&gc, prog, ESTreeDumpMode::Compact),
        "{\"type\":\"Program\",\"body\":[{\"type\":\"ExpressionStatement\",\
         \"expression\":{\"type\":\"NumericLiteral\",\"value\":1},\
         \"directive\":null}]}\n"
    );
}

/// Step 4: pretty-printing of the Step-3 tree.
#[test]
fn program_nested_pretty() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let prog = build_nested_program(&gc);

    let mut out = String::new();
    dump_estree_json(
        &mut out,
        prog,
        /*pretty=*/ true,
        ESTreeDumpMode::Compact,
        gc.ctx().atom_table(),
    );

    assert_eq!(
        out,
        "{\n  \"type\": \"Program\",\n  \"body\": [\n    {\n      \"type\": \"ExpressionStatement\",\n      \"expression\": {\n        \"type\": \"NumericLiteral\",\n        \"value\": 1\n      },\n      \"directive\": null\n    }\n  ]\n}\n"
    );
}

/// Step 5: WTF-8 / astral label. A StringLiteral value with an astral char and a
/// lone high surrogate becomes `\uXXXX` escapes.
#[test]
fn wtf8_string_value() {
    let mut ctx = Context::new();
    let gc = ctx.lock();
    // "a" + U+1F44B (astral, 4-byte UTF-8) + lone high surrogate U+D800 (WTF-8).
    let bytes: &[u8] = &[b'a', 0xF0, 0x9F, 0x91, 0x8B, 0xED, 0xA0, 0x80];
    let s = gc.atom_bytes(bytes);
    let lit = gc.alloc(Node::StringLiteral(StringLiteral::new(
        NodeMetadata::new(rng(0, 1)),
        s,
    )));

    // a -> a; U+1F44B -> 👋 (surrogate pair); lone surrogate -> \ud800.
    assert_eq!(
        dump(&gc, lit, ESTreeDumpMode::Compact),
        "{\"type\":\"StringLiteral\",\"value\":\"a\\ud83d\\udc4b\\ud800\"}\n"
    );
}

/// Step 6: locations + raw with a source manager. NumericLiteral over buffer
/// text "1.5", LocAndRange, raw Include.
#[test]
fn numeric_literal_loc_range_raw() {
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("test.js", "1.5");
    let mut ctx = Context::new();
    let gc = ctx.lock();
    // Range over offsets [0,3) in buffer `id`.
    let num = gc.alloc(Node::NumericLiteral(NumericLiteral::new(
        NodeMetadata::new(rng_id(id, 0, 3)),
        1.5,
    )));
    let mut out = String::new();
    dump_estree_json_with_sm(
        &mut out,
        num,
        false,
        ESTreeDumpMode::Compact,
        &sm,
        LocationDumpMode::LocAndRange,
        ESTreeRawProp::Include,
        gc.ctx().atom_table(),
    );

    // Emission order: type -> children(value) -> raw -> loc -> range.
    // Column is 1-based: "1.5" at [0,3) -> start line 1 col 1,
    // end line 1 col 4 (find_coords is called on the exclusive end offset 3).
    assert_eq!(
        out,
        "{\"type\":\"NumericLiteral\",\"value\":1.5,\"raw\":\"1.5\",\
         \"loc\":{\"start\":{\"line\":1,\"column\":1},\"end\":{\"line\":1,\"column\":4}},\
         \"range\":[0,3]}\n"
    );
}

/// Step 7: ESTreeRawProp::Exclude (with sm) omits "raw"; the no-sm
/// dump_estree_json also omits "raw" (documented deviation).
#[test]
fn raw_excluded_and_no_sm() {
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("test.js", "1.5");
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let num = gc.alloc(Node::NumericLiteral(NumericLiteral::new(
        NodeMetadata::new(rng_id(id, 0, 3)),
        1.5,
    )));

    // With sm but raw Exclude, and no locations: no "raw" key.
    let mut out = String::new();
    dump_estree_json_with_sm(
        &mut out,
        num,
        false,
        ESTreeDumpMode::Compact,
        &sm,
        LocationDumpMode::None,
        ESTreeRawProp::Exclude,
        gc.ctx().atom_table(),
    );
    assert_eq!(out, "{\"type\":\"NumericLiteral\",\"value\":1.5}\n");

    // No-sm overload: "raw" omitted because the buffer is unavailable.
    assert_eq!(
        dump(&gc, num, ESTreeDumpMode::Compact),
        "{\"type\":\"NumericLiteral\",\"value\":1.5}\n"
    );
}

/// A node whose range runs past the end of its buffer: mirror C++
/// `printSourceLocation` skipping the whole loc+range block when an endpoint
/// fails to resolve, and skip `raw` rather than panic on the out-of-bounds
/// slice. Buffer "1.5" is 3 bytes; the range [0,10) is out of bounds.
#[test]
fn out_of_range_skips_loc_range_and_raw() {
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer("test.js", "1.5");
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let num = gc.alloc(Node::NumericLiteral(NumericLiteral::new(
        NodeMetadata::new(rng_id(id, 0, 10)),
        1.5,
    )));
    let mut out = String::new();
    dump_estree_json_with_sm(
        &mut out,
        num,
        false,
        ESTreeDumpMode::Compact,
        &sm,
        LocationDumpMode::LocAndRange,
        ESTreeRawProp::Include,
        gc.ctx().atom_table(),
    );
    // No "raw", no "loc", no "range" — only the in-bounds fields.
    assert_eq!(out, "{\"type\":\"NumericLiteral\",\"value\":1.5}\n");
}
