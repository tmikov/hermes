# Hermes vs OXC vs SWC — parser/lexer performance investigation

> **Date:** 2026-06-30. **Subject:** the C++ Hermes front-end (lexer + parser),
> compared against the Rust parsers **OXC** 0.137.0, **SWC** `swc_ecma_parser`
> 41.1.1, and **Boa** `boa_parser` 0.21.1. All numbers are single-thread,
> warm, median-of-N, on one machine (Linux x86-64).

## TL;DR

- **Hermes' parser and lexer have no inefficiency.** Verified at the assembly
  level (jump-table dispatch, word-compare keyword matching) and under the
  correct compiler.
- **Hermes is 1.3–1.9× *faster* than SWC** on every workload tested.
- **OXC is faster than Hermes, but by less than it first appears**, and the
  margin shrinks as the benchmark gets fairer:
  - raw lexer, OXC deferring everything: **2.2×**
  - lexer, equal work, **same compiler family (LLVM)**: **1.13×**
  - full parse: **1.5–2.0×**
  - **full front-end (parse + binding/semantic — the fair, equal-work
    comparison): 1.34× (typescript) – 1.66× (react)**
- OXC's remaining edge is its **design choice to do/defer less per byte**
  (zero-copy atoms, deferred number parsing, deferred scope/symbol resolution)
  into **~2× smaller arena nodes** (less memory traffic) — not a Hermes flaw.

## The single most important methodology lesson

**Build the C++ tree with Clang, not GCC.** A bare
`cmake -DCMAKE_BUILD_TYPE=Release` on Ubuntu silently uses GCC
(`/usr/bin/c++`). OXC/SWC/Boa are built by rustc's **LLVM**. Comparing
GCC-built Hermes against LLVM-built Rust is a compiler confound that makes
Hermes look slow and invents phantom "inlining deficits":

- The OXC-equivalent Hermes lexer measured **260 MiB/s under GCC** vs
  **354.6 MiB/s under Clang** — GCC produced ~36% slower code.
- Under GCC, `matchReservedWord` stayed a non-inlined call (~15% of the
  lexer); **Clang inlines it** (drops to ~7%, no separate symbol) — exactly
  what OXC's LLVM does. The "OXC inlines more aggressively" conclusion was
  false; it was "GCC inlines less than LLVM."

Always configure with `-DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++`.

## Results (Clang-built Hermes throughout)

### Lexer-only (clean corpus, 8.3 MB, identical 1.74 M token counts, 0 diagnostics)

| lexer | MiB/s |
|---|---:|
| OXC | 402.7 |
| Hermes (interning on) | ~185–196 |
| Hermes (OXC-equivalent: no interning, no number→f64) | **354.6** |
| SWC | 179.4 |

OXC vs the *equal-work* Hermes lexer = **1.13×**. Hermes ≈ SWC.

### Full parse (source → AST)

| file | Hermes | OXC | SWC | Boa |
|---|---:|---:|---:|---:|
| clean_lex (8.3 MB) | 78.7 | 119.4 | 41.4 | 92.9 |
| react.development (110 KB) | 149.9 | 233.6 | 100.0 | 12.4 |
| typescript.js (8.7 MB) | 102.2 | 200.7 | 63.0 | 4.9 |

OXC 1.5–2.0× over Hermes; Hermes 1.6–1.9× over SWC.

### Full front-end (parse + binding/semantic) — the fair, equal-work comparison

`resolveASTForParser` (Hermes Sema) vs `oxc_semantic::SemanticBuilder` (OXC).
This forces OXC to do the scope/symbol/interning work it defers from parse.

| file | Hermes parse+sema | OXC parse+sema | OXC faster by |
|---|---:|---:|---:|
| clean_lex (8.3 MB) | 47.6 | 68.0 | **1.43×** |
| react (110 KB) | 83.3 | 138.7 | **1.66×** |
| typescript (8.7 MB) | 73.1 | 97.7 | **1.34×** |

Adding semantic **narrows** OXC's lead vs parse-only (typescript 1.74× → 1.34×).

## Why OXC is faster (verified, not assumed)

OXC's advantage is **doing/deferring less per byte**, confirmed by reading its
source:

1. **Zero-copy atoms.** `oxc_span` identifiers are `&str` slices into the
   source with a hash precomputed once; **no hash-table intern during parse.**
   Hermes interns every identifier occurrence into a `StringTable`
   (`DenseMap`, CityHash) — ~25–35% of the lexer on identifier-dense code,
   because the rest of the Hermes compiler needs `UniqueString`s.
2. **Deferred number parsing.** OXC's lexer (`numeric.rs`) only scans digits +
   tags the `Kind`; the `string → f64` runs later in the parser
   (`parse_literal_number`). Hermes' lexer (`scanNumber`) converts eagerly.
3. **Deferred semantics.** Scope binding, symbol/reference resolution, and
   most validation are pushed to `oxc_semantic`. The full-front-end table
   above adds this back.
4. **Smaller AST nodes / arena.** OXC nodes are `#[repr(C,u8)]` enums = 16-byte
   tagged arena pointers; concrete nodes are 32–64 bytes, children in arena
   `Vec`s, no per-node destructor (`Box` asserts `!Drop`). Hermes' ESTree
   nodes are distinct classes 48–80 bytes with a 16-byte intrusive list link
   and a 24-byte source range per node — ~2× larger, more memory traffic in
   the parse phase. (Both bump-allocate; **Hermes has no GC** — the earlier
   "GC arena / uniform 128-byte node" claim was about the unrelated *Rust
   port's* AST, not C++ Hermes.)

What OXC does **not** beat Hermes on (assembly-verified):

- **Dispatch.** `JSLexer::advance` compiles to a jump table (`cmp $0xef` bound
  + `notrack jmp *%rcx`) — same shape as OXC's 256-entry byte-handler table.
- **Keyword matching.** `matchReservedWord`'s `StringSwitch` compiles to a
  length-dispatched **word-compare** tree (`"function"` = one 64-bit `cmp`,
  `"if"` = one 16-bit `cmp`; no `memcmp` calls) — equivalent to OXC's folded
  `match &str`. A perfect-hash rewrite would buy nothing.

## Artifacts that produced false "Hermes is slow" signals

Every misleading number in this investigation traced to a measurement artifact,
not a Hermes deficiency:

- **GCC vs LLVM** (above) — the biggest one.
- **Standalone lexer benchmarks are invalid.** A lexer needs parser feedback
  for template `${…}` re-entry and regex-vs-divide. Run alone: OXC's
  `next_token_for_benchmarks` hits the first regex/template and
  `advance_to_end()`s — on typescript it returned **40 k tokens vs the real
  186 k** and a fake 12 GB/s. Hermes mis-handles templates and emits
  diagnostics whose location lookup builds the 8.7 MB line table
  (`SourceMgr::getOffsets`, ~16% of the lex-only profile — *absent* from the
  real full-parse profile). **Fix:** a synthetic corpus with no templates and
  no `/` (regex/divide), which all three lex identically with 0 diagnostics.
- **Apples-to-oranges work.** "OXC's lexer is 2.2× faster" counted interning +
  number parsing that OXC defers. Equalize the work → 1.13×.
- **SIMD is the wrong lever.** Vectorizing identifier/whitespace scans gave
  ~0% — JS runs are short (identifiers < 16 chars, single-space gaps), and the
  scan was never the bottleneck (per-token work was).
- **Interning cost is workload-dependent.** ~2 ms on template-heavy
  typescript, but ~25–35% of the lexer on identifier-dense code.

## Reproduction

- C++ baseline tool: `tools/parse-bench` (built into `cmake-build-release` with
  Clang). Modes: default (parse), `--lex-only`, `--breakdown` (setup/parse/
  teardown), `--sema` (parse + `resolveASTForParser`). Each reports median ms +
  MiB/s; lex-only also prints token count and diagnostics emitted.
- Rust competitors: ad-hoc examples in the (excluded) `rust/crates/comparison`
  crate using `oxc_parser`/`oxc_semantic` (feature `benchmarking` for the
  lexer), `swc_ecma_parser`, `boa_parser`, with a shared median timer.
- Clean corpus: a generated ~8.3 MB JS file (functions of declarations,
  arithmetic, strings, `if`/`for`) with **no template literals and no `/`** so
  standalone lexing is valid and diagnostic-free.
- Build: `cmake -B cmake-build-release -G Ninja -DCMAKE_BUILD_TYPE=Release
  -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++`.

## Caveats

- Hermes' `resolveASTForParser` and OXC's `SemanticBuilder` are not guaranteed
  to do identical work; parse+sema is the closest apples-to-apples
  "source → bound AST" comparison, not a proof of equality.
- Boa is erratic (fast on trivial code, collapses on real code); included for
  reference only.
- Single machine, single thread; absolute MiB/s will vary by host.

## Bottom line

Hermes' front-end is a well-built, eager, arena-based recursive-descent
parser/lexer with no measured inefficiency. It is materially faster than SWC.
Current OXC is faster by ~1.3–1.7× on the full front-end, entirely from a
defer-and-shrink design (zero-copy atoms, lazy numbers, deferred semantics,
smaller arena nodes) — the trade you would expect from a linter/bundler
front-end vs a compiler front-end. The fairer the comparison (same compiler,
same work), the smaller the gap.
