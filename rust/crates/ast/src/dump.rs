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

use atom_table::{AtomBytes, AtomTable, INVALID_ATOM_BYTES};
use support::json_emitter::JSONEmitter;
use support::location::SMRange;
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
    /// Dump no locations.
    None,
    /// Only output locations: line and column.
    Loc,
    /// Only output byte ranges.
    Range,
    /// Output both locations and byte ranges.
    LocAndRange,
}

/// Whether to include the `"raw"` property where available. Mirrors `ESTreeRawProp`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ESTreeRawProp {
    /// Omit `"raw"`.
    Exclude,
    /// Emit `"raw"` where available — today `NumericLiteral`, and only when
    /// the dumper has a `SourceErrorManager` to read the source text from.
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

    pub(crate) fn field_node<'n>(&mut self, key: &str, node: Option<&'n Node<'n>>, ignore: bool) {
        if self.skip_empty(node.is_none(), ignore) {
            return;
        }
        self.json.emit_key(key);
        self.dump_node_ptr(node);
    }

    pub(crate) fn field_list<'n>(&mut self, key: &str, list: NodeList<'n>, ignore: bool) {
        if self.skip_empty(list.is_empty(), ignore) {
            return;
        }
        self.json.emit_key(key);
        self.dump_node_list(list);
    }

    pub(crate) fn field_bool(&mut self, key: &str, val: bool, ignore: bool) {
        // isEmpty(NodeBoolean) == !val
        if self.skip_empty(!val, ignore) {
            return;
        }
        self.json.emit_key(key);
        self.json.emit_bool(val);
    }

    pub(crate) fn field_number(&mut self, key: &str, val: f64, ignore: bool) {
        // isEmpty(NodeNumber) == false (never empty).
        if self.skip_empty(false, ignore) {
            return;
        }
        self.json.emit_key(key);
        self.json.emit_f64(val);
    }

    pub(crate) fn field_label(&mut self, key: &str, label: NodeLabel, ignore: bool) {
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
            // Port of the StackOverflowGuard overflow path: emit `null` and
            // stop recursing. C++ also calls `sm_->error(...)` here, but our
            // dumper holds a shared `&SourceErrorManager` (it resolves coords),
            // so we drop the diagnostic — the 128-depth guard is a safety net,
            // not a tested surface (see the module doc / plan deviation #2).
            self.json.emit_null_value();
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
    /// has no location pointer); omitted when `sm` is None (documented
    /// deviation #1).
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
        // Skip `raw` rather than panic if the range is out of the buffer's
        // bounds (only reachable with synthetic/malformed ranges; parser output
        // is always in-buffer).
        let bytes = match buf.bytes().get(r.start.offset as usize..r.end.offset as usize) {
            Some(b) => b,
            None => return,
        };
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
        // Mirror C++ `printSourceLocation`: if either endpoint fails to resolve
        // (`findBufferLineAndLoc` returns false), skip the whole loc+range block.
        // Our offset model can't fail to resolve an in-buffer offset, so the
        // analog is an offset past the buffer's content length. Both endpoints
        // share a buffer (guaranteed by `range_is_valid`).
        let buf = sm.find_buffer_for_loc(r.start);
        let buf_len = buf.bytes().len();
        if r.start.offset as usize > buf_len || r.end.offset as usize > buf_len {
            return;
        }
        let start = sm.find_coords(r.start);
        let end = sm.find_coords(r.end);

        if matches!(
            self.loc_mode,
            LocationDumpMode::Loc | LocationDumpMode::LocAndRange
        ) {
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

        if matches!(
            self.loc_mode,
            LocationDumpMode::Range | LocationDumpMode::LocAndRange
        ) {
            self.json.emit_key("range");
            self.json.open_array();
            dump_sm_range_json(self.json, r);
            self.json.close_array();
        }
    }
}

/// Whether a range is set (mirrors C++ `SMRange::isValid()`). In the offset
/// model an `SMLoc` always carries a buffer, so we treat a range as valid when
/// its endpoints are in the same buffer and ordered.
fn range_is_valid(r: SMRange) -> bool {
    r.start.source == r.end.source && r.start.offset <= r.end.offset
}

/// Emit a range as the two buffer-relative offsets. Port of `dumpSMRangeJSON`
/// (the caller wraps these in an array). In the offset model the offsets are the
/// values directly. Kept `pub` to mirror the C++ public `dumpSMRangeJSON`
/// (declared in `ESTreeJSONDumper.h`).
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
