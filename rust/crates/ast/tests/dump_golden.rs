//! Golden tests for ESTreeJSONDumper (ast phase 4). Trees are hand-built in a
//! Context/GCLock; output is asserted byte-for-byte.
use ast::context::{Context, GCLock};
use ast::dump::{dump_estree_json, ESTreeDumpMode};
use ast::node::{Node, NumericLiteral};
use ast::node_child::NodeMetadata;
use support::location::{SMLoc, SMRange, SourceId};

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
