# hermes-parser

> A Rust port of the Hermes front-end by Tzvetan Mikov, the architect of Hermes.
> Not an official Meta project and not supported by Meta.

**Version:** 0.1.0 — the initial release of the `hermes-parser` crate family on
[crates.io](https://crates.io/crates/hermes-parser).

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

This project does not publish performance comparisons at this time. The
headline is correctness: byte-for-byte agreement with the production C++
engine.

## Quickstart

Add it to your project:

```toml
[dependencies]
hermes-parser = "0.1"
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

### Resolve the names: parse + semantic analysis

Parsing gives you a tree; semantic analysis tells you what the names in it
mean. That is the second crate, and the second dependency:

```toml
[dependencies]
hermes-parser = "0.1"
hermes-sema = "0.1"
```

Those two are enough for a complete front end — `hermes-sema` takes
`hermes-parser`'s `ParsedJS` directly, and both re-export the AST crate.

```rust
use hermes_parser::{parse, ParseFlags};

fn main() {
    let src = r#"let greeting = "hi"; print(greeting);"#;
    let parsed = parse(src, ParseFlags::default()).expect("parse error");
    let mut resolved = hermes_sema::resolve(parsed).expect("resolve error");

    // What each name in the global scope turned out to be.
    resolved.with_program(|gc, _root, sem| {
        for &id in &sem.scope(sem.get_global_scope()).decls {
            let decl = sem.decl(id);
            let name = String::from_utf8_lossy(gc.bytes(decl.name));
            println!("{name}\t{:?}", decl.kind);
        }
    });
    // greeting  Let
    // print     UndeclaredGlobalProperty
}
```

`resolve` **consumes** the `ParsedJS`: the resolver is a transforming visitor,
so the root that comes out is a different node than the one that went in, and
the returned `ResolvedJS` owns the arena, the rewritten tree and the
`SemContext`. `ResolvedJS::into_parsed` hands the `ParsedJS` back when you want
the ESTree JSON dumper afterwards, and `ResolvedJS::to_sema_dump` prints the
`hermesc -dump-sema` text. `resolve_for_parser` and `resolve_for_compile` pick
the two C++ entry points explicitly; see
[`crates/sema/README.md`](crates/sema/README.md) for which to use.

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
| `hermes-sema` | Semantic analysis: declaration collection, scope/binding resolution, validation — stable *core* surface, plus advanced modules that may change |
| `hermes-support` | `SourceErrorManager`, diagnostics, JSON emitter — support crate |
| `hermes-atom-table` | String interner — support crate |
| `hermes-unicode` | Unicode property tables — support crate |
| `hermes-command-line` | LLVM-`cl`-style CLI option parser — support crate |

The support crates are published only because the dependency closure requires
it — except `hermes-command-line`, which nothing in that closure needs and
which is published because the project's own CLI drivers are built on it.
Depend on any of them directly at your own risk; only `hermes-parser`,
`hermes-ast` and `hermes-sema` carry stable public API guarantees.

`hermes-sema` is the one crate whose guarantee is partial. Its stable surface
is the `resolve` façade (`resolve` / `resolve_for_parser` / `resolve_for_compile`
/ `ResolvedJS` / `ResolveError`), the two low-level entry points in its
`resolve` module, and the result model (`sem_context`, `ids`). Its other seven
modules — `resolver`, `decl_collector`, `ast_eval`, `dump`, `dump_context`,
`libhermes`, `keywords` — are public because the project's own tools and tests
drive them directly; they are advanced / port-internal and may change or be
made private in a 0.x bump. Each says so in its own module documentation.

## Support

Issues and PRs are welcome and addressed as time permits. There is no SLA.

## License

MIT — see [LICENSE](LICENSE). The Hermes C++ engine and the juno crates
(`atom_table`, `unicode`, `command_line`) are credited in [NOTICE](NOTICE).
