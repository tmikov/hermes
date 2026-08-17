# hermes-gen-js

Turns a Hermes AST back into JavaScript, Flow, or TypeScript source text, by
Tzvetan Mikov, the architect of Hermes. Not an official Meta project and not
supported by Meta. Part of the `hermes-parser` crate family.

This crate is the Rust port of juno's AST -> JS generator
(`unsupported/juno/crates/juno/src/gen_js.rs`, 4174 lines, frozen): given a
parsed `hermes-parser::ParsedJS`, it prints source text that reparses to the
same tree. See "Correctness: there is no C++ oracle" below for exactly what
that promise covers and what it does not.

## The façade

One call, taking `hermes-parser`'s `ParsedJS`:

```rust
use hermes_gen_js::{to_js, Opt};
use hermes_parser::{parse, ParseFlags};

let mut parsed = parse("let x = 1;", ParseFlags::default())?;
let js = to_js(&mut parsed, Opt::default())?;
assert_eq!(js, "let x = 1;\n");
```

`to_js` is a **free function**, not an inherent `ParsedJS::to_js`. The
natural place to hang "regenerate JS from a parse" is on `hermes-parser`'s
own type, but that is impossible here: `to_js` names
`hermes_parser::ParsedJS` in its own public signature, so `hermes-parser` is
a direct, non-optional dependency of this crate, and it cannot depend back on
this crate to host an inherent method without a dependency cycle. The call
site is unaffected either way — still one line for a caller already holding a
`ParsedJS`.

`examples/print_js.rs` runs that call twice, once per `Pretty` mode, over a
short snippet, and prints both results — the quickest way to see what
`Pretty::Yes` actually changes.

## `Opt`

Every generation call takes an `Opt`, mirroring juno's option set field for
field:

- `pretty` — `Pretty::No` (most compact valid source) or `Pretty::Yes`
  (indentation and a few readability spaces). See the next section for what
  `Yes` does *not* do.
- `annotation` — `Annotation::No` for plain source, or
  `Annotation::Sem(&SemContext)` to print each identifier's resolved binding
  inline, given a completed `hermes-sema` analysis. **`Annotation::Sem`
  requires the non-default `annotate` feature** — see below.
- `force_async_arrow_space` — whether an async arrow's `async` keyword is
  followed by a space before a parenthesized parameter list.
- `doc_block` — an optional literal block printed verbatim before anything
  else.
- `quote` — `QuoteChar::Single` or `QuoteChar::Double` for string literals.

## Features

`annotate` (off by default) enables `Annotation::Sem`, and is the only thing
that makes this crate depend on `hermes-sema`:

```toml
hermes-gen-js = { version = "0.1", features = ["annotate"] }
```

It is off by default because it is a debugging aid — nothing on the ordinary
generation path touches sema — and leaving it on would make every consumer
compile `hermes-sema` to get a feature most of them never enable. By default
the dependency graph is `hermes-ast`, `hermes-parser`, `hermes-support` (and
their shared `hermes-atom-table` / `hermes-unicode` / `bumpalo`); `cargo tree
-p hermes-gen-js -e normal` shows no `hermes-sema` edge at all.

`Annotation<'s>` and `Opt<'s>` keep the same arity in both states, so a
signature written against one compiles under the other; only the `Sem` variant
appears or disappears.

## Correctness: there is no C++ oracle

Every other crate in this port (the lexer, the parser, semantic analysis) was
validated by byte-comparing its output against a C++ binary built from the
same source tree. This one cannot be. A second AST-to-JS implementation does
exist in the C++ tree, `lib/AST2JS/AST2JS.cpp` (1239 lines, ES-only, no
type-annotation sites at all), but it is not used in Hermes's compile or
execution pipeline — its only caller is the `hermesc -dump-js` debug flag — was
not extensively tested, and was **not the source this crate was ported from** —
this crate ports juno's `gen_js.rs`, not `AST2JS.cpp`. `AST2JS.cpp`'s
behavior is not a specification, and byte-matching it would buy nothing.

The correctness bar instead is the **round-trip property**: parse a source,
print it back out, reparse the printed text, and require the two ASTs to be
identical (modulo two normalizations that necessarily change under any
correct generator: a numeric literal's verbatim `"raw"` source text, and
source locations). `tests/corpus.rs` checks that property over 421
checked-in parser/sema corpus files; `tests/roundtrip.rs` checks it over
several hundred hand-written and generated cases targeting the places a JS
printer is most often silently wrong — operator precedence and
parenthesization.

## `Pretty::Yes` is indentation, not formatting

`Pretty::Yes` adds indentation and a handful of readability spaces so output
is not one long line. It is **not** a source formatter: it does not reflow
line lengths, wrap long argument lists, or normalize style beyond what `Opt`
already controls. Both `Pretty` modes are the same printer with whitespace
inserted at fixed points; do not expect `rustfmt`/`prettier`-shaped output.

Known cosmetic warts, all inherited from juno's printer and all verified to
round-trip to an identical AST — listed so they are not mistaken for bugs:

- Call arguments are parenthesized more than needed: `f(a + b)` prints as
  `f((a + b))`. juno's own argument rule; `new Foo` behaves the same way.
- A class body's closing brace sits at the inner indent, followed by a blank
  line.
- A value-less private field prints a stray space: `#y ;`.
- Flow object-type bodies emit leading commas and a whitespace-only line
  before `}`; `interface I  extends` gets a double space.
- `for(`, `do  {`, `}finally  {` and `;else ` are spaced inconsistently.
- `export {A}` is reprinted as `export {A as A};`.

None of these affect the round-trip property. If this crate ever grows a real
formatting mode, they are the list to start from.

## Coverage

What follows describes what is actually run, not a completeness claim — this
port had to delete six other doc comments elsewhere in the crate that
asserted some enumeration was exhaustive when a test later disproved it, and
this README does not add a seventh.

- All 271 `hermes-ast` node kinds are handled, and this is a compile-time
  property: the dispatch match has no catch-all arm, so a kind added to the
  AST without a printing arm here is a build failure. The only kinds that
  report `GenJsError::UnsupportedKind` are the 8 with no JS source syntax at
  all (7 internal cover-grammar/builtin kinds plus `TemplateElement`, which
  only its `TemplateLiteral` may print).
- 41 real defects were found and fixed during the port — every one invisible
  to reading juno's source, caught only by running generated output back
  through the parser and comparing ASTs.
- `tests/corpus.rs` regenerates and reparses all 421 files under the
  checked-in parser/sema corpora (393 parse cleanly; the other 28 are error
  fixtures or cover-grammar trees). That corpus is **not** a stand-in for
  adversarial parenthesization coverage: it contains only 87 parenthesized
  nodes, spanning 23 kinds and 40 distinct (parent kind, child kind) edges,
  because real-world source rarely writes redundant parens — and the "must
  add parens" direction of the generator's parenthesization logic is exactly
  where those defects live. `tests/paren_matrix.rs` is the answer to that: a
  generated cross-product over (parent kind × child kind × child position) in
  which every child is explicitly parenthesized in the source, 6788 probes /
  13 012 round trips, reaching **1985** distinct (parent, child) pairs. It
  found 8 of the 41 defects on its first run, and two mutation experiments
  recorded in `MANIFEST.md` show it going red while every other gate in the
  crate stays green.
- `MANIFEST.md` records what was run, with exact commands and counts,
  including a wide sweep over all 1934 `.js` files in the C++ lit tree, the
  per-kind coverage table, and — the part that matters — the list of what is
  still **not** covered.

This README and the crate's API docs make no performance claims, benchmarked
or otherwise.

## Stability

Pre-1.0. This crate has not been published; treat its public API as unstable
until it is. `Opt`, `Pretty`, `QuoteChar`, `Annotation`, `GenJsError`,
`generate`, and `to_js` are the intended stable surface once it is;
`dispatch::GenJS::gen_node` is `pub` only so the dispatch skeleton is
directly testable, and is not meant to be called from outside this crate.

Zero `unsafe` (`unsafe_code = "forbid"`).

See [the project README](https://github.com/tmikov/hermes/blob/rust/rust/README.md) for the full documentation of the
crate family.
