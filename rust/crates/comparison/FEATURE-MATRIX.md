# Parser Feature Matrix

**Date:** 2026-06-19  
**Subjects:** `hermes-parser` (this port), `swc_ecma_parser`, `oxc_parser`, `biome_js_parser`, `boa_parser`

---

## What Makes This Port Different

Three differentiators separate the Hermes Rust port from every other parser in this comparison:

1. **Faithful 1:1 port of a production C++ engine.**  
   The Hermes JavaScript engine powers React Native on hundreds of millions of devices.  
   This port translates the production C++ front-end (`lib/Parser/JSParserImpl-*.cpp`)
   directly into Rust, preserving structure, comments, and logic at the function level.
   It is not a from-scratch Rust parser inspired by Hermes; it is Hermes in Rust.

2. **Byte-for-byte differential testing vs `hermesc -dump-ast`.**  
   Every corpus file is parsed by both this port and the real `hermesc` binary.
   The AST JSON dumps are compared byte-for-byte. Any divergence is a bug, not a
   warning. No other parser in this comparison makes an equivalent claim — their
   conformance is based on test suites or fuzzing, not a live production oracle.

3. **Full Flow type-grammar support.**  
   The complete Flow type grammar — annotation hierarchy, function/object/tuple types,
   type parameters and arguments, predicates, `type`/`opaque type`/`interface`
   declarations, `enum`, `component`/`hook`, `record`, `match`, the `declare` family,
   `import type`/`export type` — is implemented and differential-tested. Among
   Rust parsers, only `swc_ecma_parser` has any Flow support at all (as an opt-in
   feature), and its coverage is more limited. OXC and Biome have no Flow support.

---

## Feature Matrix

Versions verified: `swc_ecma_parser 41.1.1`, `oxc_parser 0.137.0`,
`biome_js_parser 0.5.7` (docs/source; not benchmarked — build failure in
published crates, see perf section), `boa_parser 0.21.1`.
Research date: 2026-06-19.

| Feature | hermes-parser (this port) | SWC `swc_ecma_parser` 41.1.1 | OXC `oxc_parser` 0.137.0 | Biome `biome_js_parser` 0.5.7 | Boa `boa_parser` 0.21.1 |
|---|---|---|---|---|---|
| **ECMAScript coverage** | ✅ ES2025+; standard JS grammar complete | ✅ ES2025+; passes nearly all tc39/test262 | ✅ ES2025+; full latest stable ECMAScript | ✅ ES2025+; JS + TS + JSX | ✅ ES2025; ~94% test262 conformance (v0.21 release notes) |
| **Stage 3+ proposals** | ✅ Decorators (Hermes set), `using`/`await using` | ✅ Decorators, `import attributes`, more | ✅ Stage 3 Decorators, import attributes | ✅ Decorators, experimental parameter decorators opt-in | ⚠️ Engine-focused; subset of proposals |
| **JSX** | 🚧 Lexer complete; parser not yet wired | ✅ Full JSX + TSX | ✅ Full JSX + TSX | ✅ Full JSX + TSX | ❌ Not supported |
| **TypeScript** | 🚧 In progress (P7); type-annotation core landed; object types + interface + class members + enums remain | ✅ Full TS 5.x including decorators, `satisfies`, `const` type params | ✅ Full TS 5.x including all modern syntax | ✅ Full TS + TSX | ❌ Not supported |
| **Flow** | ✅ **Complete** — full type grammar, `declare` family, `enum`, `component`/`hook`, `record`/`tuple`, `match`; byte-for-byte differential-tested | ✅ Partial — `Syntax::Flow` opt-in strips type-only constructs; `components`, `enums`, `patternMatching` options present; coverage shallower than this port | ❌ Not supported | ❌ Not supported | ❌ Not supported |
| **AST model** | Own AST; ESTree node shapes (271 nodes from `ESTree.def`), GC-arena-allocated, JSON-dumpable | Own `swc_ecma_ast` (ESTree-inspired but not compatible); heap-allocated (`Box`/`Vec`) | Own AST; **ESTree-compatible output** (100% compatible with acorn); bump-arena allocated | **Lossless CST** (fork of rowan/rust-analyzer green/red tree); not ESTree | Own AST; ESTree-inspired; heap-allocated |
| **Error recovery / tolerant parsing** | ❌ Fail-fast (mirrors C++ Hermes); errors terminate parse | ⚠️ Partial — can return `Ok(Module)` with errors emitted to handler; some panics on malformed input documented | ✅ Advanced error recovery; recoverable vs. unrecoverable errors distinguished | ✅ **Fully tolerant** — any input produces a CST; errors wrapped in `ERROR` nodes; designed for IDE use | ⚠️ Limited; engine-focused error handling |
| **Comments preserved** | ✅ Comments stored in lexer; accessible via `StoredComment` | ✅ Stored in `SourceMap`; some JSX edge cases drop comments (known issue) | ✅ Attached to AST nodes | ✅ Part of lossless CST; every token including trivia is preserved | ✅ Comments stored |
| **Source locations** | ✅ Every node carries byte-offset spans (`SMLoc`/`SMRange`) | ✅ `BytePos`-based `Span` on every node | ✅ `u32` spans on every node (memory-efficient) | ✅ Full token + trivia offsets in CST | ✅ Source positions on nodes |
| **Allocator model** | GC arena (bumpalo-backed, custom `Context`/`GC`); entire AST freed in one drop | Global heap (`Box`/`Vec` per node); no arena in 41.1.1 (arena PR not yet in this version) | Bump arena (`oxc_allocator`); all AST nodes share one lifetime; free in O(1) | Rowan green tree (immutable, ref-counted, interned); not arena | Global heap |
| **Conformance methodology** | **Byte-for-byte differential vs live `hermesc` binary** (production C++ oracle); any divergence is a CI failure | tc39/test262 parser tests; no external oracle | tc39/test262 + own test suite; no external oracle | Own test suite + test262 subset; no external oracle | test262; ~94% pass rate reported |
| **Maturity / ecosystem** | Initial release (0.1.0 on crates.io); React Native lineage via Hermes | Production — powers Next.js compiler, Parcel, Deno transform layer, Prettier (SWC plugin); widely deployed since 2020 | Production — Oxlint 1.0 stable; used in Vite/Rolldown, Prettier 3.6 (experimental plugin); very active development | Production — Biome 2.x; formatter + linter toolchain; used as ESLint/Prettier alternative | Experimental — embeddable JS engine; parser is internal component; not intended for standalone parse use |
| **Rust crate** | `hermes-parser` (0.1.0) | `swc_ecma_parser` | `oxc_parser` | `biome_js_parser` (crates.io publish bug in 0.5.7; unusable as standalone dep) | `boa_parser` |

---

## Notes on Individual Competitors

### SWC (`swc_ecma_parser` 41.1.1)

SWC is the most directly comparable parser: production-grade, Rust, handles Flow via
`Syntax::Flow` with `components`/`enums`/`patternMatching` options. Its Flow support
predates this port's but is shallower — it targets type-stripping for compilation, not
full-fidelity preservation of Flow semantics. SWC's AST uses `Box`/`Vec` heap
allocation; an arena (`swc_allocator`) has been prototyped (PR #9805) but is not
present in 41.1.1.

SWC offers partial error recovery (returns `Ok(Module)` with errors; some inputs
trigger panics on malformed TypeScript — documented issues exist). Comments are
stored and accessible but some edge cases around JSX drop them.

Verified against: `swc_ecma_parser` crate docs, `swc.rs/docs/usage/flow`, SWC
GitHub `crates/swc_ecma_parser/` source (2026-06-19).

### OXC (`oxc_parser` 0.137.0)

OXC is the fastest parser in this comparison by a wide margin. Its bump-arena
allocation (`oxc_allocator`) and u32 spans explain much of the throughput advantage.
ESTree compatibility (100% acorn-compatible output) makes it drop-in for tooling.
Full TypeScript 5.x and JSX/TSX. No Flow support — not planned (Flow is treated as
a separate dialect handled by other tools; Prettier's Hermes plugin is recommended
for Flow).

Error recovery is advanced and distinguished from unrecoverable errors at the API level.

Verified against: `oxc.rs` parser architecture docs, `docs.rs/oxc_parser`, OXC
GitHub source (2026-06-19).

### Biome (`biome_js_parser` 0.5.7)

Biome's parser is unique in this list: it produces a **lossless CST** (concrete
syntax tree) rather than an AST. Based on an internal fork of the `rowan` library
(used in rust-analyzer), the green/red tree preserves every character including
whitespace and comments. This makes it ideal for formatter and linter use — Biome
is designed as a formatter-first toolchain. The trade-off is a heavier data
structure than a sparse AST.

Error recovery is the strongest in this list: fully tolerant parsing, any input
produces a valid CST with errors wrapped in `ERROR` nodes.

No Flow support. Supports JS, JSX, TS, TSX, JSON, CSS, GraphQL.

**Important:** `biome_js_parser 0.5.7` cannot be used as a standalone Cargo
dependency due to a publish mismatch between `biome_js_syntax 0.5.7` and
`biome_rowan 0.5.8` (FileSourceError variant API changed between published
versions). This crate was excluded from benchmarking for this reason.

Verified against: `biomejs.dev/internals/language-support/`,
`biomejs.dev/internals/architecture/`, `docs.rs/biome_js_parser` (2026-06-19).

### Boa (`boa_parser` 0.21.1)

Boa is a full JavaScript engine (lexer + parser + bytecode compiler + VM), not a
standalone parse library. Its parser is an internal component that feeds the
interpreter. It does not support TypeScript, JSX, or Flow — it targets plain
ECMAScript. Test262 conformance is ~94% as of v0.21.

The throughput is significantly lower than the other parsers (~8× slower than this
port on the react fixture) because Boa's parser performs additional work during
parsing (scope resolution, interning) that the other parsers defer or skip.

Verified against: Boa v0.21 release blog (`boajs.dev/blog/2025/10/22/boa-release-21`),
`github.com/boa-dev/boa` README (2026-06-19).

---

## Performance (Directional)

> **Important caveat:** These numbers are directional only. Each parser does
> different amounts of work: different AST shapes, different interning strategies,
> arena vs. heap allocation, presence or absence of scope resolution during parse.
> A faster number here does not mean a better parser for your use case.

Full methodology, all fixture sizes, and the trailing-error fairness guard are
documented in [`BENCH-RESULTS.md`](BENCH-RESULTS.md).

Benchmarked with Criterion.rs (`opt-level = 3`) for Rust parsers and the Release
C++ `parse-bench` tool for the C++ Hermes baseline. Per-iteration fresh `Context`;
`FullParse`/eager; median; same machine. Four fixtures including the 8.7 MB
typescript.js (plain JS — TS/JSX in this port are in progress and were not exercised).

| Parser | react 107K | jquery 278K | three.min 654K | typescript 8.7M |
|---|---|---|---|---|
| **hermes-parser (this port)** | 97.8 | 73.8 | 42.4 | 63.0 |
| **C++ Hermes (Release)** | 78.9 | 82.6 | 47.5 | 92.4 |
| `oxc_parser` 0.137.0 | 230.5 | 152.2 | 101.7 | 176.7 |
| `swc_ecma_parser` 41.1.1 | 93.9 | 66.4 | 34.0 | 60.3 |
| `boa_parser` 0.21.1 | 12.0 | 10.5 | 4.8 | 4.9 |
| `biome_js_parser` 0.5.7 | not benchmarked (build failure) | — | — | — |

Numbers are MiB/s (median). Higher is faster.

**Key directional conclusions (verified):**

- **The Rust port tracks the C++ Hermes baseline.** It is faster than C++ Hermes
  on the react fixture and within ~11% on jquery/three.min. This is the primary
  performance claim for a faithful port: parity with the original engine.
- **The Rust port is ~32% slower than C++ Hermes on the 8.7 MB typescript fixture**
  (63.0 vs 92.4 MiB/s). Root cause (verified by decomposition): each AST node is a
  uniform 128-byte `Node` enum; ~904,000 nodes for typescript.js ≈ ~123 MiB live AST
  (~14× the source), exceeding CPU cache. Boxing the large variants is a candidate
  fix (unvalidated hypothesis — boxing trades footprint for indirection; net effect
  must be measured). See BENCH-RESULTS.md for full decomposition.
- **The OXC gap (~2.4–2.8×) is inherent to Hermes design, not a port regression.**
  OXC's bump allocator and zero-copy `Atom` type are structurally different from
  Hermes's atom interning and GC-arena AST. Any faithful port of Hermes inherits
  this gap. The Rust port beats SWC on every fixture.
- Boa is ~8× slower; its parser performs scope resolution during parse.
- Biome's lossless CST does fundamentally different work; throughput comparison is
  not meaningful.

The benchmark source is in `benches/parse_throughput.rs`; run with:

```
bash rust/crates/comparison/fetch_fixtures.sh   # download fixtures once
cargo bench --manifest-path rust/crates/comparison/Cargo.toml
```
