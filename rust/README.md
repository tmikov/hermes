# hermes-parser

> A Rust port of the Hermes front-end by Tzvetan Mikov, the architect of Hermes.
> Not an official Meta project and not supported by Meta.

**Status:** pre-release, not yet on crates.io. The intended published name is
`hermes-parser`. Quickstart instructions below are written against that name.

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
| TypeScript | 🚧 In progress |
| JSX | 🚧 In progress |

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

### Parse a JavaScript file and dump ESTree JSON

```rust
use hermes_parser::{parse, ParseFlags};
use hermes_ast::dump_estree_json;

fn main() {
    let src = r#"function greet(name) { return "Hello, " + name; }"#;
    let result = parse(src, ParseFlags::default()).expect("parse error");
    let json = dump_estree_json(&result);
    println!("{}", json);
}
```

### Parse with Flow types enabled

```rust
use hermes_parser::{parse, ParseFlags};

fn main() {
    let src = "type Point = { x: number, y: number };";
    let flags = ParseFlags { parse_flow: true, ..Default::default() };
    let result = parse(src, flags).expect("parse error");
    // use result.ast ...
}
```

Note: the exact API shape will be finalized during the public-API audit before
publication. The examples above reflect the intended surface.

## Crate family

| Published crate | Role |
|---|---|
| `hermes-parser` | Lexer + parser + JSON parser — stable public surface |
| `hermes-ast` | ESTree node set + JSON dumper — stable public surface |
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
