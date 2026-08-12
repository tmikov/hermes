# hermes-parser

> A Rust port of the Hermes front-end by Tzvetan Mikov, the architect of Hermes.
> Not an official Meta project and not supported by Meta.

**Status:** pre-release, not yet on crates.io. The crates already carry their
published names (`hermes-parser` and friends), so the quickstart below is the
real thing — only the upload is pending.

---

## What is this?

`hermes-parser` is a faithful 1:1 Rust port of the production C++ JavaScript
front-end from the [Hermes engine](https://github.com/facebook/hermes) — the
JavaScript engine that powers React Native on hundreds of millions of devices.
It provides a complete lexer, parser, and ESTree-compatible AST with JSON
output, matching the Hermes `hermesc` binary's behavior byte-for-byte.

## Language support

| Language | Status |
|---|---|
| JavaScript / ECMAScript | ✅ Complete |
| Flow type grammar | ✅ Complete |
| TypeScript | ✅ Complete |
| JSX | ✅ Complete |

## Why this parser?

### 1. Faithful 1:1 port of a production engine

This is not a from-scratch Rust parser inspired by Hermes. It translates the
production C++ front-end (`lib/Parser/JSParserImpl-*.cpp`, ~16,900 lines)
directly into Rust, preserving structure, comments, and logic at the function
level. Where the C++ uses templates, the Rust uses generics. Where the C++ uses
RAII guards, the Rust uses explicit guard types. The port is disciplined and
traceable.

### 2. Byte-for-byte differential testing vs `hermesc -dump-ast`

Every corpus file is parsed by both this port and the real `hermesc` binary.
The AST JSON dumps are compared byte-for-byte. Any divergence is a bug, not a
known difference. This gate runs continuously and is the project's primary
conformance claim.

### 3. The only complete Flow parser in Rust

SWC has only partial, opt-in Flow support (type-stripping focus, shallower than
this port); OXC and Biome have no Flow support. This port implements the **full**
Flow type
grammar: type annotations, conditional/union/intersection types, function/object
types, generics, predicates, `opaque type`, `interface` declarations, typed
arrows, `as`/`as const` casts, `enum`, `component`/`hook`, `record`, `match`,
the `declare` family, and `import type`/`export type` — the entire
`JSParserImpl-flow.cpp` surface, behind a `parse_flow` flag.

### 4. ESTree-compatible AST

The parser produces the same AST as `hermesc -dump-ast`, with full source
location information and ESTree JSON output. Downstream tooling that already
speaks ESTree works unchanged.

## Comparison with other Rust parsers

See [`crates/comparison/FEATURE-MATRIX.md`](crates/comparison/FEATURE-MATRIX.md)
for a detailed feature and conformance matrix comparing `hermes-parser`, SWC,
OXC, Biome, and Boa.

### Directional performance note

This is a fidelity-first port. The AST uses a GC arena rather than a bump
allocator, which does different amounts of work than OXC's bump AST or Biome's
lossless CST. These are apples-to-oranges comparisons.

Verified directional numbers (Criterion + Release C++ `parse-bench`; same machine;
median; FullParse/eager; MiB/s):

- The Rust port **tracks C++ Hermes** — faster than C++ Hermes on small files
  (react: 97.8 vs 78.9 MiB/s) and within ~11% on medium files. On the 8.7 MB
  typescript fixture the port is ~32% slower (63.0 vs 92.4 MiB/s); root cause is
  AST node footprint at scale (128-byte uniform `Node` enum, ~14× source size live),
  not GC collection.
- **OXC's ~2.4–2.8× lead is inherent to Hermes design** (atom interning + GC-arena
  AST vs OXC's bump allocator + zero-copy atoms) — any faithful port of Hermes
  inherits this gap, as does C++ Hermes itself.
- The Rust port beats SWC on every fixture.

See [`crates/comparison/BENCH-RESULTS.md`](crates/comparison/BENCH-RESULTS.md)
for the full table, methodology, and large-file decomposition.

Performance is a secondary concern. The headline is correctness: byte-for-byte
agreement with the production C++ engine.

## Quickstart

The crate is not yet published. Once on crates.io, add it to your project:

```toml
[dependencies]
hermes-parser = "0.1"    # version TBD at launch
```

That one dependency is enough: `hermes-parser` re-exports the AST crate as
`hermes_parser::ast`, and the diagnostic type its API returns
(`ResolvedDiagnostic`) from its own root.

### Parse a JavaScript source and dump ESTree JSON

```rust
use hermes_parser::{parse, ParseFlags};

fn main() {
    let src = r#"function greet(name) { return "Hello, " + name; }"#;
    let mut parsed = parse(src, ParseFlags::default()).expect("parse error");
    println!("{}", parsed.to_estree_json(true));
}
```

### Parse with Flow types enabled, and walk the AST

The AST lives in an arena owned by the returned `ParsedJS`, and is read under a
lock, so traversal happens inside a closure:

```rust
use hermes_parser::ast::node::Node;
use hermes_parser::{parse, ParseFlags};

fn main() {
    let src = "type Point = { x: number, y: number };";
    let flags = ParseFlags { parse_flow: true, ..Default::default() };
    let mut parsed = parse(src, flags).expect("parse error");

    let statements = parsed.with_program(|_gc, program| match program {
        Node::Program(p) => p.body.iter().count(),
        _ => unreachable!("the root of a parse is always a Program"),
    });
    assert_eq!(statements, 1);
}
```

`ParseFlags` also carries `parse_ts`, `parse_jsx`, `strict_mode`, and the three
Flow extension flags (`component`/`hook`, `record`, `match`). On failure,
`parse` returns a `ParseError` carrying the diagnostics.

`parse()` is a convenience façade over the low-level API — `Context`,
`SourceErrorManager`, `JSLexer`, `JSParserImpl` — which stays public for
callers that need lazy parsing, a custom diagnostic handler, or one arena
shared across files. `crates/tools/src/bin/ast_dump.rs` is the reference for
that path.

Expanded versions of both paths are in
[`crates/parser/examples/`](crates/parser/examples): `parse_to_estree_json.rs`
(reads a path from `argv`, prints the JSON, renders diagnostics on failure) and
`walk_ast.rs` (walks a snippet with `hermes_parser::ast::visitor::Visitor` and
prints a node-kind histogram). Run either with
`cargo run -p hermes-parser --example <name>`.

## Crate family

| Published crate | Role |
|---|---|
| `hermes-parser` | Lexer + parser + JSON parser — stable public surface |
| `hermes-ast` | ESTree node set + JSON dumper — stable public surface, re-exported as `hermes_parser::ast` |
| `hermes-support` | `SourceErrorManager`, diagnostics, JSON emitter — support crate |
| `hermes-atom-table` | String interner — support crate |
| `hermes-unicode` | Unicode property tables — support crate |

The support crates are published only because the dependency closure requires
it. Depend on them directly at your own risk; only `hermes-parser` and
`hermes-ast` carry stable public API guarantees.

## Support

Issues and PRs are welcome and addressed as time permits. There is no SLA.

## License

MIT — see [LICENSE](LICENSE). The Hermes C++ engine and the juno crates
(`atom_table`, `unicode`) are credited in [NOTICE](NOTICE).
