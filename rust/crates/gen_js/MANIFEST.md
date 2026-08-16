# `hermes-gen-js` coverage manifest

What this crate's correctness rests on, what was actually **run** to establish
it, and — the part a manifest exists for — what was **not** covered and why.

This file records measurements, not claims. Six false universal statements
("every …", "all …", "structurally impossible") have already been deleted from
this crate's doc comments after review found each of them wrong; nothing below
asserts completeness. Every number here is the output of a command that is
written down next to it, and every command was re-run against the tree as
committed.

Written for Task 17 of `doc/superpowers/plans/2026-08-15-gen-js-port.md`.
Style follows `rust/crates/sema/tests/sema_corpus/MANIFEST.md`.

---

## 1. The four gates, and what each one can and cannot see

| gate | file | what it runs | size | catches |
|---|---|---|---|---|
| unit + ported juno cases | `tests/roundtrip.rs` | hand-written sources | **239 tests** | what somebody thought to write down |
| Tier 1 corpus | `tests/corpus.rs` | the 421 checked-in parser/sema corpus files | **786 round trips** | real code, written by people who never heard of this generator |
| **adversarial paren matrix** | `tests/paren_matrix.rs` | a generated (parent × child × position) cross-product | **13 012 round trips** | parenthesization, which real code almost never writes |
| Tier 2 wide sweep | `crates/tools/src/bin/gen_js_sweep.rs` | all 1935 `.js` under `test/` | **3568 round trips** | node kinds and shapes no corpus has |
| kind exhaustiveness | `tests/exhaustive.rs` | source-text guard on `dispatch.rs` | 1 test | the catch-all coming back |
| sweep regressions | `tests/sweep_regressions.rs` | the 7 sweep-found defects | 8 tests | those defects returning |

`cargo test --manifest-path rust/Cargo.toml -p hermes-gen-js` runs everything
except the Tier 2 sweep: **299 tests** (36 lib + 11 corpus + 1 exhaustive +
1 paren matrix + 239 roundtrip + 8 sweep regressions + 3 doctests), 0 failures.

The Tier 2 sweep is **not** a standing test. It reads the C++ lit tree
(`test/`), which a published crate cannot assume is present, so it is a
development-time binary in the unpublished `tools` crate.

---

## 2. The adversarial parenthesization matrix

### Why it exists

Task 15's review measured the Tier 1 corpus and found the hole this gate
fills. The 421 files reach 262 of 271 node kinds but contain only **87
parenthesized nodes**, across 23 kinds and **40 distinct (parent → child)
edges**. `FunctionTypeAnnotation` and `ConditionalTypeAnnotation` never appear
parenthesized in them at all; `OptionalMemberExpression`,
`OptionalCallExpression`, `ConditionalExpression`, `YieldExpression`,
`AwaitExpression`, `UnaryExpression`, `NewExpression`,
`TaggedTemplateExpression`, `SpreadElement`, `PrivateName`, `TemplateLiteral`
and `BigIntLiteral` never appear parenthesized anywhere.

All 27 defects found in this port before Task 17 live in the "must **add**
parens" direction of `need_parens`. Real source almost never writes a
redundant parenthesis, so more real files add node *kinds* and essentially no
new parenthesized *shapes*. The 1935-file sweep did not close this, and could
not have.

### Command

```
cargo test --manifest-path rust/Cargo.toml -p hermes-gen-js --test paren_matrix -- --nocapture
```

### What it generates

A cross-product of three tables:

- **99 edges** — derived from the 78 `print_child` / `print_comma_expression`
  call sites in `src/`, written as source templates with a `%s` hole, plus a
  second entry where a list field's first and last position differ, or where
  an operator's associativity is itself the hazard (`**`, `in`, `??`, `==`).
- **132 payloads** — one per node kind that can occupy such a hole, in some
  cases several spellings of the same kind, across ES, Flow, Flow `match`,
  Flow component syntax, TypeScript and JSX.
- **2 frames** — script top level, and the body of
  `class C extends B { async *m() { … } }`, so `yield`, `await`, `super.x` and
  `new.target` are in scope for half the expression probes and out of scope
  for the other half.

Each triple is instantiated with the payload **wrapped in parentheses**, so
the parsed tree holds the raw parent → child edge with no paren node of any
kind (this parser, like every ESTree producer, records grouping parens
nowhere). The generator must then decide, unaided, whether to put them back.
Each probe is parsed, generated in both `Pretty` modes, and reparsed, with the
two ESTree dumps (`ESTreeRawProp::Exclude`, no locations — the same oracle
`tests/corpus.rs` uses) required to be identical.

### Result, as recorded by the test's own output

```
paren matrix: 6788 probes (6506 live, 278 did not parse, 4 cover-grammar,
              20 dialect conflicts), 13012 round trips,
              1985 distinct parent->child pairs, 856 predicted-skip but parsed
```

**1985 distinct (parent kind → child kind) pairs put into a tree**, every one
of them from a child the source parenthesized — against the Tier 1 corpus's
40.

### Skips are classified by rule, and pinned

A probe whose *source* does not parse is classified by [`predict_skip`], which
reads **tags** on the frame, the edge and the payload — never the text of the
probe, and never a list of names. A probe that fails to parse and matches no
rule is an **unclassified skip and fails the test**. Every count below is
asserted:

| reason | count | rule |
|---|---|---|
| `NotAnAssignmentTarget` | 6 | hole is a target position, payload is not a target |
| `SpreadOutsideList` | 82 | `...a` outside an argument/element list |
| `PrivateNameOutsideIn` | 92 | bare `#x` outside `in`'s left operand |
| `NoYieldInScope` | 47 | `yield` with no generator, or behind a nested function boundary |
| `NoAwaitInScope` | 45 | `await` likewise |
| `AngleBracketAssertionUnderJsx` | 4 | `<T>expr` cannot coexist with JSX |
| `UnparenthesizableInThisHole` | 2 | last-resort rule; see PD-2 |

Plus, counted separately and also pinned: **4 cover-grammar probes** (the tree
is not a program — detected structurally by `CoverFinder`, exactly as
`tests/corpus.rs` does) and **20 dialect conflicts** (a Flow payload in a
TypeScript hole; never generated at all).

Two further assertions make the tables honest rather than decorative: **every
edge and every payload must take part in at least one live probe**. That is
what caught two entries in an earlier draft that were not real: a
`TSNonNullExpression` payload (`x!`) and a `TSTypeOperator` payload
(`keyof A`) — **neither node kind exists in this AST at all**, and grepping
`ESTree.def`, `crates/ast/src/node.rs` and the whole parser for either name
finds nothing.

### Parenthesizability is measured, not declared

The matrix's premise — wrap the payload in parens and the raw edge appears —
is false for 20 payloads, for reasons that belong to the parser. Hand-writing
which ones would be exactly the string list this file avoids, and would go
stale silently. So `measure_paren_behavior` probes each payload once in a
canonical context (`type Q = X;` for types, `t = X;` inside the method frame
for expressions) and wraps only what it measures as transparent. The two other
buckets are reported and pinned as evidence about the parser — see PD-1 and
PD-2 below.

---

## 3. Mutation evidence — proving the gates can fail

A green suite proves only that it ran. Two mutations, each applied to `src/`,
run, and reverted (`git diff` empty afterwards; no `git checkout --`, no
`git clean`).

### M1 — remove the `AwaitExpression` precedence entry (defect 28)

Change `Node::AwaitExpression(_) => (UNARY, Assoc::Rtl)` back to the
`ALWAYS_PAREN` catch-all it fell into before Task 17.

| gate | result |
|---|---|
| `tests/corpus.rs` (Tier 1, 784 round trips) | **11 passed, 0 failed — GREEN** |
| `tests/paren_matrix.rs` | **FAILED**: `28 of 13012 round trips failed`, at `paren_matrix.rs:1356` |

### M2 — remove `TaggedTemplateExpression` from the optional-chain rule (defect 29)

| gate | result |
|---|---|
| `tests/corpus.rs` | **11 passed — GREEN** |
| `tests/roundtrip.rs` | **239 passed — GREEN** |
| `tests/sweep_regressions.rs` | **8 passed — GREEN** |
| `tests/paren_matrix.rs` | **FAILED**: `8 of 13012 round trips failed` |

M2 is the sharper one: a rule that **no other gate in this crate notices at
all**.

### M3 — Task 15's evidence, carried forward

From `task-15-report.md`, reproduced by that task's reviewer:

- Breaking the `ExpressionStatement` parenthesization rule fails
  `corpus_parser` (12 of 152 round trips) and `corpus_sema` (16 of 398).
- In `flow_arrow_return_type_shapes_all_round_trip` (3645 shapes): injecting
  the wrapper `"%s ###"` panics at `roundtrip.rs:7699` with "unclassified
  skips …"; reverting `gen_type_predicate` to a bare `gen_node` fails
  `flow_type_predicate_operand_keeps_its_parens` **and 96 probe entries**;
  making the spine helper descend into `TypePredicate` fails two tests and
  **384** entries; removing the `ConditionalTypeAnnotation` terminal fails
  `flow_arrow_return_conditional_type_keeps_its_parens` and **24** entries.
  The whole pre-fix `src` (`dc764513b`) fails **120 of 3645**.

### M4 — Task 15's own gate, carried forward

Injecting a name into `corpus.rs`'s `Expected::unparseable` list fails
`corpus_sema` with a full set diff; `flags_from_source` hard-errors on a
second `// FLAGS:` line (a `#[should_panic]` test on the real message).

---

## 4. The Tier 2 wide sweep

### Command

```
cargo run --manifest-path rust/Cargo.toml -p tools --bin gen-js-sweep --release \
  -- /home/tmikov/work/hermes-rust/test
```

The test root is positional (defaults to `<repo>/test`; may also name a single
file). Flags: `--kinds-only`, `--failures-only`, `--show-generated` (echoes
the inferred `ParseFlags` and both generated texts — the single-file
investigation mode used for each fix below).

### Flag inference

Per file, from the lit `RUN:` lines: `-parse-flow`, `-parse-ts`, `-parse-jsx`,
`-Xparse-flow-match`, `-Xparse-component-syntax`, `-Xparse-flow-records`, plus
a `// FLAGS:` line if present. **`-typed` is also honored**, because the
driver turns it into both Flow parsing (`CompilerDriver.cpp:1289-1296`,
`shermes.cpp:707`) and unconditional strict mode (`CompilerDriver.cpp:1235`,
`shermes.cpp:662`). That one rule is worth naming: without it the sweep
reported 460 unparseable / 2944 round trips; with it, 151 / 3564 — it pulled
**309 files** (`Sema/flow`, `hermes/flow`, `IRGen/flow`) back in.

### Result

| | |
|---|---|
| files | **1935** |
| round-tripped files | **1784** |
| round trips (both `Pretty` modes) | **3568** |
| skipped: does not parse | **151** |
| skipped: cover-grammar tree | **1** (`AST/cover-initializer.js`, `CoverInitializer`) |
| not UTF-8 | 0 |
| panicked | 0 |
| **round-trip failures** | **0** |

Panic safety is in the tool (`catch_unwind` per file, 64 MiB stack) and never
fired: the known `match(x){_=>1};` parser panic
(`crates/parser/src/js/statements.rs:1196`) is **not reachable from `test/`**.

### The 150 files that do not parse

These are not generator failures — there is no tree to generate from. They are
a long tail: **83 distinct diagnostics, 58 of which occur exactly once**
("Rest parameter must be last formal parameter", "duplicate constructors in
class", "No digits after 0x", "Closing tag must match opening", …). 104 of the
150 have `error` in their file name. By top directory: Parser 128, IRGen 12,
AST 3, dependency-extractor 2, SourceMap 2, repl 1, Sema 1, Optimizer 1.

Only four groups have more than four members:

| count | diagnostic | reason |
|---|---|---|
| 15 | `';' expected` | 12 are error fixtures by name; 1 is the match-statement fixture added with the C++ fix; 2 other. The 2 that were PD-3 now parse — see PD-3 below |
| 15 | `'return' not in a function` | driver modes `ParseFlags` cannot express: `-typed` wraps the program in an IIFE (`shouldWrapInIIFE`, `CompilerDriver.cpp:846`) and `-commonjs` in a module function, so top-level `return` is legal there. `AST/global-return.js`, `IRGen/array-typed.js`, `IRGen/flow/{array-for-of,class-field-idz,class-method,exact-object,object-indexer-spread,object-indexer,template-literal,tuple-access,tuple-destr,typecast}.js`, `Optimizer/flow/string-concat.js`, `Sema/flow/assign-ops-3.js`, `Parser/class-static-block-return-error.js` |
| 6 | `Unexpected variance sigil` | error fixtures |
| 5 | `invalid arrow function parameter list` | all `*-error.js` |

Two groups were deliberately **not** accommodated, because doing so would
invent a convention the C++ side does not have:

- `SourceMap/translator/prog1.js` and `prog1/mod1.js` are Flow source marked
  only by an `@flow` docblock, which no Hermes driver honors.
- `dependency-extractor/jsx.js` and `modules.js` need the flow+jsx that
  `tools/dependency-extractor/dependency-extractor.cpp:48-51` hardcodes in C++
  rather than passing on a command line.

Raw output, including every one of the 150 with its diagnostic and the full
271-row kind table: regenerate with the command above.

---

## 5. Defects found and fixed by Task 17

Numbering continues the port's running count, which stood at 27 after Task 15.
All 8 are **generator** defects; each is documented at its fix site with the
measured before/after, and each has a named regression test.

| # | where | input | before | after | inherited from juno? | found by |
|---|---|---|---|---|---|---|
| 28 | `precedence.rs` `get_precedence` | `await (a + b)` | `await a + b` (different tree) | `await (a + b)` | **yes** — juno has no `AwaitExpression` arm, so it fell into `_ => ALWAYS_PAREN` | paren matrix, 28 rows |
| 29 | `precedence.rs` `need_parens` | `new (a?.b)()`, `` (a?.b)`q` `` | `new a?.b()`, `` a?.b`q` `` — **do not parse** | parens kept | **yes** — juno's branch names only `MemberExpression`/`CallExpression` | paren matrix, 16 rows |
| 30 | `precedence.rs` `need_parens` | `type Q = (A?.[K])[K];` (Flow) | `A?.[K][K]` (different tree) | parens kept | yes | paren matrix, 2 rows |
| 31 | `precedence.rs` `need_parens` | `(-a) ** b` | `-a ** b` — **does not parse** | parens kept | **yes** | paren matrix (20 rows) **and** the Tier 2 sweep (`test/hermes/bigint-binary-exponentiate.js`) independently |
| 32 | `precedence.rs` `need_parens` | `(a \|\| b) as T` | `a \|\| b as T`, i.e. `a \|\| (b as T)` | parens kept | n/a — juno predates `as` | paren matrix, 12 rows |
| 33 | `precedence.rs` `need_parens` | `([a, b]) = t;`, `(a ? b : c) = y;`, `(++a)++` | `[a,b]=t` (becomes `ArrayPattern`), `a ? b : c = y` (different tree), `++a++` (roles swap) | parens kept | yes | sweep (`test/Parser/es6/reparse-array-destr.js`) + paren matrix |
| 34 | `precedence.rs` `need_parens` | `("s");` | `'s';` — becomes a **directive**, and for `"use strict"` flips strictness | `('s');` | yes | paren matrix, 4 rows |
| 35 | `gen.rs` `space_before_equals` | `var v: * = 1;`, `t = <a/> == b;` | `var v:*=1;` (`*=`), `t=<a />==b;` (`>=`) — neither parses | a space is inserted | yes | paren matrix, 3 rows |

Defect 35 is the only one that needed new machinery: `GenJS` now tracks the
last byte written, and the 21 sites that emit a token starting with `=` ask
`space_before_equals` whether a separator is needed. It is deliberately **not**
hooked into `write_ascii`: the string-literal escaper writes literal characters
through that same path, and a central hook turned `"YQ=="` into `"YQ= ="` and
`"===="` into `"= = = ="` across 12 sweep files. Maximal munch is a question
about tokens, and only the arms know which of their output is a token.

Three of `roundtrip.rs`'s pinned compact strings changed as a result
(`type T<X,Y>=X;` → `type T<X,Y> =X;` and two siblings); each carries a
comment naming defect 35 and explaining why `>` is in the byte set even though
the parser happens to split `>=` in `GrammarContext::Type` — the same byte ends
a self-closing JSX tag, where it is not split, and a byte-level guard cannot
tell the two apart.

### Defects 36-41, found by the Tier 2 sweep

Recorded here for completeness; each has its own named regression test in
`tests/sweep_regressions.rs` (which asserts both a clean round trip **and**
the exact generated text) and its rationale at the fix site.

| # | input | before | after | test |
|---|---|---|---|---|
| 36 | `55e5555…5;` | `Infinity;` — an `Identifier` | `1e999;` | `numeric_literal_that_overflowed_to_infinity_prints_as_a_literal` |
| 37 | `if (x) function f(){} else function f(){}` | braces printed, `implicit` flipped (and `elsefunction` in `Pretty::No`) | printed without braces, with a forced space | `implicit_block_around_an_if_function_declaration_prints_without_braces` |
| 38 | `x as (const);` | `x as const;` — becomes `AsConstExpression` | parens kept | `as_expression_whose_type_is_const_keeps_its_parens` |
| 39 | `declare class B { proto +x: T }` | `+proto x:T` — **does not parse** | `proto +x:T` | `object_type_property_prints_variance_after_proto_and_static` |
| 40 | `let bar = ([,,]) => {}` | `([,])` — one hole, not two | `([,,])` | `array_pattern_keeps_a_trailing_elision` |
| 41 | `[(a = 1)] = t;` | `[a=1]=t` — element becomes `AssignmentPattern` | parens kept | `parenthesized_pattern_element_keeps_its_parens` |

Defect 31 (`**`) and defect 33 (a literal as an assignment target) were found
**twice, independently** — by the paren matrix and by the sweep — and have
tests in both `tests/paren_matrix.rs` (as matrix rows) and
`tests/sweep_regressions.rs` (as named minimal reproducers).

**Running total for the port: 41 defects.**

The juno-inherited / port-introduced split is deliberately not given as a
number. An earlier draft said "20 juno / 21 ours"; the final review found that
contradicts both §5's own table (which marks 7 of defects 28-35 as
juno-inherited) and the task ledger's running 14/13, which together already
exceed 20 before defects 36-41 are attributed at all. The per-defect
attribution in §5 and in each defect's own entry is the authoritative record;
a single summary figure was wrong every way it was computed, so it is omitted
rather than guessed.

### One residual gap, not fixed

The `ObjectPattern` analogue of defect 41: `({a: ([c])} = t);` keeps an
`ArrayExpression` as the `Property` value, and `gen_property` has no `Path`,
so it cannot tell an `ObjectPattern` property from an `ObjectExpression` one.
It does not appear in `test/`, in any Tier 1 corpus file, or in the paren
matrix (whose `Property` position has no `print_child` call site to key on),
so no gate currently covers it. `({a: (b = 1)} = t)` is **not** affected — the
parser rewrites that one regardless of parens.

**Its reachability is narrower than it looks, which is why it is not fixed.**
The final review reproduced the divergence (silent, both `Pretty` modes) and
then checked whether the input is valid JavaScript at all: V8 rejects
`({a: ([c])} = t);` with `SyntaxError: Invalid destructuring assignment
target`, and C++ hermesc accepts it identically to our parser. So the gap can
only be reached by source that is not valid JavaScript in the first place —
a program no conforming engine would run. Fixing it would mean giving
`gen_property` a `Path` it does not currently need, to serve inputs that are
already invalid. Recorded rather than fixed; if `gen_property` ever gains a
`Path` for another reason, close it then.

---

## 6. Parser defects found (recorded, not fixed)

Task 17 was scoped to the generator. These are parser bugs; each has a minimal
reproducer and is left alone.

### PD-1 — `reparseIdentifierAsTSTypeAnnotation` maps 5 of the 9 keyword types

`parseTSPrimaryType` maps nine names to their `TS*Keyword` node
(`lib/Parser/JSParserImpl-ts.cpp:928-990`, ported at
`crates/parser/src/js/ts/types.rs:420-467`), but the parenthesized path goes
through `reparseIdentifierAsTSTypeAnnotation`, which maps only five
(`JSParserImpl-ts.cpp:1406-1430`, ported at `types.rs:766-800`). The C++ was
read directly to confirm the port is faithful and the defect is upstream.

```
$ echo 'type Q = (unknown); type R = unknown;' | ast-dump --parse-ts -
  Q.typeAnnotation -> TSTypeReference { typeName: Identifier "unknown" }
  R.typeAnnotation -> TSUnknownKeyword
```

Affects exactly `bigint`, `never`, `undefined`, `unknown`. `any`, `boolean`,
`number`, `symbol`, `string` are in both maps and behave; `object` is in
neither and is symmetric.

### PD-2 — a parenthesized TypeScript type only parses if it is an identifier or starts with `(`

`parseTSFunctionOrParenthesizedType`'s cover reads the contents as a
function-type *parameter* unless the next token is `(` or `)`
(`crates/parser/src/js/ts/function_types.rs:117-159`, C++ 277-315), so
everything else is rejected:

```
type Q = (A | B);        ')' expected at end of function type parameters
type Q = ((A | B));      same
type Q = ("s");          identifier, '{' or '[' expected in binding pattern
type Q = (void);         same
type Q = ([A, B]);       parses — but as an ArrayPattern, not a TSTupleType
```

The matrix measured this rather than being told it: 14 payloads land in
"parenthesized spelling rejected" and 6 in "parenthesized spelling denotes a
different tree" (4 from PD-1, plus `TSTupleType`/`TSTypeLiteral`, which the
cover hands back as an `ArrayPattern`/`ObjectPattern` — the same intruders
`is_full_ts_type_field` already documents). Both lists are asserted in
`paren_matrix.rs`, so fixing either defect turns that test red and this section
gets updated rather than going stale.

Consequence for this crate: there is very little parenthesized TypeScript type
for the generator to preserve, because the parser cannot express it. That is a
real limit on what the matrix proves about `arms/ts.rs`, and it is stated here
rather than papered over.

### PD-3 — `%checks` is never recognized after `declare function` / `declare hook`

> **FIXED after this manifest was first written — `50ec2aa52`.** Left in place
> because the numbers below are what the sweep measured before the fix, and
> because the reasoning is the record of how it was found.
>
> This was a Rust-only regression, unlike PD-1 and PD-2. The final review
> confirmed against `cmake-build-asan/bin/hermesc` that C++ Hermes parses
> `declare function foo(): boolean %checks;` and our parser did not. PD-1 and
> PD-2 were verified as upstream C++ behavior that we faithfully match; PD-3
> was a divergence we introduced, so it got its own fix in `hermes-parser`
> rather than being worked around here.
>
> What changed after the fix, re-measured (`gen-js-sweep`, both `Pretty`
> modes): the sweep goes from 1934 files / 1782 round-tripped / 151
> unparseable to **1935 / 1784 / 150** — two of the three `';' expected`
> files below now parse, and the +1 file is the new C++ lit fixture from the
> unrelated match-statement fix, which is unparseable by design. Per-kind
> coverage goes from `DeclaredPredicate` 0 and `InferredPredicate` 0 to
> **1 and 10**, so both kinds now have real sweep coverage rather than only a
> hand-written test. `hook-syntax/declare-hook-predicate-error.js` now reports
> `checks predicates unsupported with hooks`, byte-identical to `hermesc` —
> that branch had been dead, so the fix repaired the hook diagnostic too.

`crates/parser/src/js/flow/declarations.rs:741` and `:1805` test
`self.check_name(b"checks")`, but the interned token text is `%checks`
(`include/hermes/AST/Keywords.def:132` is
`HERMES_KEYWORD(Checks, "%checks")`; `lib/Parser/JSLexer.cpp:479-480` interns
the literal). Every other Rust site uses `b"%checks"`
(`functions.rs:180,187,194`, `expressions.rs:569,588,2046,2055`,
`flow/function_types.rs:750`).

```
$ echo 'declare function foo(): boolean %checks;' | ast-dump --parse-flow -
  ';' expected, at the '%'
```

Consequences: `test/Parser/flow/predicate-checks.js` and
`declare-function-location.js` do not parse,
`hook-syntax/declare-hook-predicate-error.js` reports the wrong diagnostic, and
`DeclaredPredicate`/`InferredPredicate` had **zero** coverage anywhere until
Task 17 added `flow_predicate_annotations_round_trip` (which uses the ordinary
function-declaration spelling, unaffected by the defect).

### PD-4 — the known `match` panic, not reachable from `test/`

`match(x){_=>1};` panics at `crates/parser/src/js/statements.rs:1196`
(`assertion failed: self.check(TokenKind::l_brace)`). Already open before this
task. The generator guards against emitting it (the `MatchExpression` disjunct
in `need_parens`'s `ExpressionStatement` branch), and the sweep confirms no
file in `test/` reaches it: **0 panics in 1935 files**.


---

## 7. Per-kind coverage

Counts are occurrences in the **original** trees of the 1782 files the Tier 2
sweep round-tripped, tallied by walking each tree. **211 of the 271 kinds have
count ≥ 1.** The 60 with count 0 are every one named below, with what covers
them instead — silent zero coverage is the failure mode this table exists to
prevent.

### The 60 kinds with count 0

**46 of them are the entire TypeScript grammar**, and the reason is one fact:
**the lit tree contains no TypeScript source.** The only `-parse-ts` anywhere
under `test/` is `Sema/deep-ast-err.js`, whose `RUN:` line pipes *generated
stdin* (`echo "x" "+"{1..10000}`) into shermes, so no `.js` file in the tree is
TypeScript. The port's TypeScript coverage is therefore Tier 1
(`crates/parser/tests/parser_corpus_ts`, 20 files, in `tests/corpus.rs`), the
64 hand-written `ts_*` tests in `tests/roundtrip.rs`, and the 21 TypeScript
edges of the paren matrix — not the sweep.

| kind | why 0 | covered instead by |
|---|---|---|
| `TSAnyKeyword` | no TS in `test/` | `ts_any_keyword_round_trips` |
| `TSArrayType` | no TS in `test/` | `ts_array_type_round_trips`, `ts_array_type_keeps_parens_around_a_function_type_element` |
| `TSAsExpression` | no TS in `test/` | `ts_as_expression_round_trips`, `ts_as_expression_left_operand_keeps_parens_for_looser_expressions` |
| `TSBigIntKeyword` | no TS in `test/` | `ts_bigint_keyword_round_trips` |
| `TSBooleanKeyword` | no TS in `test/` | `ts_boolean_keyword_round_trips` |
| `TSCallSignatureDeclaration` | no TS in `test/` | `ts_call_signature_declaration_round_trips` |
| `TSConditionalType` | no TS in `test/` | `ts_conditional_type_round_trips`, `ts_conditional_type_check_type_keeps_parens_around_a_function_type` |
| `TSConstructorType` | no TS in `test/` | `ts_constructor_type_round_trips` |
| `TSEnumDeclaration` | no TS in `test/` | `ts_enum_declaration_round_trips` |
| `TSEnumMember` | no TS in `test/` | `ts_enum_member_round_trips`, `ts_enum_member_sequence_initializer_keeps_parens` |
| `TSFunctionType` | no TS in `test/` | `ts_function_type_round_trips` |
| `TSIndexSignature` | no TS in `test/` | `ts_index_signature_round_trips` |
| `TSIndexedAccessType` | no TS in `test/` | `ts_indexed_access_type_round_trips` |
| `TSInterfaceBody` | no TS in `test/` | `ts_interface_body_round_trips` |
| `TSInterfaceDeclaration` | no TS in `test/` | `ts_interface_declaration_round_trips` |
| `TSInterfaceHeritage` | no TS in `test/` | `ts_interface_heritage_round_trips` |
| `TSIntersectionType` | no TS in `test/` | `ts_intersection_type_round_trips`, `ts_intersection_type_keeps_parens_around_a_function_type_member` |
| `TSLiteralType` | no TS in `test/` | `ts_literal_type_round_trips` |
| `TSMethodSignature` | no TS in `test/` | `ts_method_signature_round_trips` |
| `TSModifiers` | no TS in `test/` | `ts_modifiers_round_trip_in_accessibility_static_readonly_order`, `ts_modifiers_on_a_private_class_property_round_trip` |
| `TSModuleBlock` | no TS in `test/` | `ts_module_member_and_module_block_round_trip`, `ts_module_block_empty_with_qualified_name_round_trips` |
| `TSModuleDeclaration` | **unreachable from source** — every spelling tried (`module N {}`, `declare module N {}`, `declare namespace N {}`, `declare module "x" {}`) is a parse error under `-parse-ts`; `namespace N {}` builds a `TSModuleMember` | nothing; the arm is reachable only from a hand-built tree |
| `TSModuleMember` | no TS in `test/` | `ts_module_member_and_module_block_round_trip` |
| `TSNeverKeyword` | no TS in `test/` | `ts_never_keyword_round_trips` |
| `TSNumberKeyword` | no TS in `test/` | `ts_number_keyword_round_trips` |
| `TSParameterProperty` | no TS in `test/` | `ts_parameter_property_round_trips` |
| `TSPropertySignature` | no TS in `test/` | `ts_property_signature_round_trips` |
| `TSQualifiedName` | no TS in `test/` | `ts_qualified_name_round_trips` |
| `TSStringKeyword` | no TS in `test/` | `ts_string_keyword_round_trips` |
| `TSSymbolKeyword` | no TS in `test/` | `ts_symbol_keyword_round_trips` |
| `TSThisType` | no TS in `test/` | `ts_this_type_round_trips` |
| `TSTupleType` | no TS in `test/` | `ts_tuple_type_round_trips` |
| `TSTypeAliasDeclaration` | no TS in `test/` | `ts_type_alias_declaration_round_trips` |
| `TSTypeAnnotation` | no TS in `test/` | `ts_type_annotation_round_trips`, `ts_typed_arrow_return_type_keeps_no_parens` |
| `TSTypeAssertion` | no TS in `test/` | `ts_type_assertion_round_trips`, `ts_type_assertion_operand_parenthesizes_only_looser_expressions` |
| `TSTypeLiteral` | no TS in `test/` | `ts_type_literal_round_trips` |
| `TSTypeParameter` | no TS in `test/` | `ts_type_parameter_round_trips` |
| `TSTypeParameterDeclaration` | no TS in `test/` | `ts_type_parameter_declaration_round_trips` |
| `TSTypeParameterInstantiation` | no TS in `test/` | `ts_type_parameter_instantiation_round_trips` |
| `TSTypePredicate` | no TS in `test/` | `ts_type_predicate_round_trips` |
| `TSTypeQuery` | no TS in `test/` | `ts_type_query_round_trips` |
| `TSTypeReference` | no TS in `test/` | `ts_type_reference_round_trips` |
| `TSUndefinedKeyword` | no TS in `test/` | `ts_undefined_keyword_round_trips` |
| `TSUnionType` | no TS in `test/` | `ts_union_type_round_trips`, `ts_union_type_member_that_is_itself_a_union_keeps_its_parens` |
| `TSUnknownKeyword` | no TS in `test/` | `ts_unknown_keyword_round_trips` |
| `TSVoidKeyword` | no TS in `test/` | `ts_void_keyword_round_trips` |

The remaining 14:

| kind | why 0 | covered instead by |
|---|---|---|
| `CoverEmptyArgs` | excluded by construction — a tree holding one is not a program (`src/dispatch.rs:90`, spec §4) | `tests/corpus.rs`'s `cover` pin (`sema_corpus/error-cover-nodes.js`) asserts generation **refuses**; the paren matrix counts 4 such probes |
| `CoverInitializer` | same | `corpus_parser`'s `cover` pin (`cover_init.js`); the sweep's 1 skip (`AST/cover-initializer.js`) |
| `CoverRestElement` | same | the paren matrix's cover-grammar count (`t = (0, ...a);`) |
| `CoverTrailingComma` | same | `dispatch.rs`'s arm; no live producer found |
| `CoverTypedIdentifier` | same | `corpus_sema`'s `cover` pin (`flow-typecast-cover.js`) |
| `ImplicitCheckedCast` | compiler-internal; the parser never builds one | `dispatch.rs:95` returns `UnsupportedKind` by design |
| `SHBuiltin` | compiler-internal | `dispatch.rs:96`, same |
| `Metadata` | not a node in the tree — it is every node's first field | n/a |
| `Directive` | the alternate ESTree spelling; Hermes uses `ExpressionStatement::directive` instead | `three_statements_get_semicolons_and_separation` and the directive-fidelity tests exercise the field |
| `DirectiveLiteral` | same | `gen_directive_literal` is reachable only from a hand-built tree |
| `DeclaredPredicate` | was **PD-3** (now fixed, `50ec2aa52`); sweep count is 1 after the fix | **`flow_predicate_annotations_round_trip`**, added by Task 17 |
| `InferredPredicate` | same; sweep count is 10 after the fix | **`flow_predicate_annotations_round_trip`**, added by Task 17 |
| `ExistsTypeAnnotation` | Flow `*` simply does not occur in `test/` | `all_primitive_and_literal_flow_types_round_trip_in_one_union`; and it is a paren-matrix payload, which is how defect 35 was found |
| `BooleanLiteralTypeAnnotation` | `type T = true` does not occur in `test/` | `all_primitive_and_literal_flow_types_round_trip_in_one_union`, `literal_type_annotations_preserve_their_raw_spelling` |

### The full 271-row table

Regenerate with `--kinds-only`:

```
cargo run --manifest-path rust/Cargo.toml -p tools --bin gen-js-sweep --release \
  -- --kinds-only /home/tmikov/work/hermes-rust/test
```

<details>
<summary>counts, descending</summary>

| kind | count |
|---|---|
| `NumericLiteral` | 83152 |
| `Identifier` | 79893 |
| `CallExpression` | 23622 |
| `ExpressionStatement` | 18686 |
| `MemberExpression` | 16516 |
| `StringLiteral` | 11421 |
| `BlockStatement` | 8470 |
| `VariableDeclarator` | 5792 |
| `BinaryExpression` | 5562 |
| `VariableDeclaration` | 5391 |
| `AssignmentExpression` | 4581 |
| `Property` | 3177 |
| `ReturnStatement` | 2644 |
| `FunctionExpression` | 2369 |
| `FunctionDeclaration` | 2182 |
| `TypeAnnotation` | 2108 |
| `ArrayExpression` | 1927 |
| `ObjectExpression` | 1899 |
| `Program` | 1782 |
| `NewExpression` | 1549 |
| `UnaryExpression` | 1461 |
| `NumberTypeAnnotation` | 1405 |
| `TryStatement` | 1240 |
| `CatchClause` | 1200 |
| `BooleanLiteral` | 1131 |
| `ArrowFunctionExpression` | 1110 |
| `GenericTypeAnnotation` | 947 |
| `ThisExpression` | 868 |
| `TemplateElement` | 792 |
| `MethodDefinition` | 720 |
| `SwitchCase` | 567 |
| `ClassBody` | 558 |
| `ClassDeclaration` | 534 |
| `IfStatement` | 521 |
| `RegExpLiteral` | 505 |
| `UpdateExpression` | 497 |
| `PrivateName` | 486 |
| `YieldExpression` | 473 |
| `StringTypeAnnotation` | 462 |
| `TypeAlias` | 344 |
| `ClassPrivateProperty` | 331 |
| `ForStatement` | 330 |
| `NullLiteral` | 315 |
| `ClassProperty` | 312 |
| `TypeParameterInstantiation` | 309 |
| `BreakStatement` | 285 |
| `ThrowStatement` | 278 |
| `VoidTypeAnnotation` | 249 |
| `ArrayTypeAnnotation` | 248 |
| `LogicalExpression` | 223 |
| `ObjectTypeAnnotation` | 214 |
| `TypeParameter` | 213 |
| `Decorator` | 203 |
| `ObjectTypeProperty` | 176 |
| `TypeParameterDeclaration` | 176 |
| `Super` | 166 |
| `Empty` | 165 |
| `ForOfStatement` | 163 |
| `UnionTypeAnnotation` | 160 |
| `ArrayPattern` | 154 |
| `ConditionalExpression` | 122 |
| `SpreadElement` | 119 |
| `FunctionTypeParam` | 116 |
| `ObjectPattern` | 116 |
| `RestElement` | 113 |
| `TemplateLiteral` | 112 |
| `BigIntLiteral` | 110 |
| `TupleTypeAnnotation` | 108 |
| `JSXIdentifier` | 101 |
| `BooleanTypeAnnotation` | 99 |
| `FunctionTypeAnnotation` | 99 |
| `AssignmentPattern` | 94 |
| `DebuggerStatement` | 94 |
| `ForInStatement` | 89 |
| `EmptyStatement` | 81 |
| `Variance` | 81 |
| `OptionalMemberExpression` | 79 |
| `SwitchStatement` | 77 |
| `WhileStatement` | 73 |
| `AwaitExpression` | 63 |
| `AnyTypeAnnotation` | 60 |
| `MatchLiteralPattern` | 56 |
| `MatchExpressionCase` | 55 |
| `NullLiteralTypeAnnotation` | 47 |
| `JSXElement` | 45 |
| `JSXOpeningElement` | 45 |
| `LabeledStatement` | 45 |
| `ObjectTypeIndexer` | 42 |
| `MetaProperty` | 40 |
| `TypeCastExpression` | 40 |
| `TaggedTemplateExpression` | 39 |
| `ComponentDeclaration` | 37 |
| `OptionalCallExpression` | 36 |
| `TypePredicate` | 31 |
| `JSXClosingElement` | 30 |
| `ImportDeclaration` | 29 |
| `ContinueStatement` | 27 |
| `MixedTypeAnnotation` | 25 |
| `ClassExpression` | 24 |
| `MatchStatementCase` | 24 |
| `SequenceExpression` | 24 |
| `StaticBlock` | 23 |
| `TypeOperator` | 23 |
| `DeclareExportDeclaration` | 22 |
| `MatchBindingPattern` | 22 |
| `MatchObjectPatternProperty` | 22 |
| `NullableTypeAnnotation` | 22 |
| `DeclareClass` | 21 |
| `ComponentParameter` | 20 |
| `ConditionalTypeAnnotation` | 20 |
| `ExportNamedDeclaration` | 20 |
| `JSXAttribute` | 19 |
| `StringLiteralTypeAnnotation` | 18 |
| `MatchStatement` | 17 |
| `DeclareComponent` | 16 |
| `ExportDefaultDeclaration` | 16 |
| `HookTypeAnnotation` | 16 |
| `JSXText` | 16 |
| `EnumDeclaration` | 15 |
| `MatchObjectPattern` | 15 |
| `ComponentTypeParameter` | 14 |
| `RecordDeclaration` | 14 |
| `RecordDeclarationBody` | 14 |
| `RecordDeclarationProperty` | 14 |
| `RecordExpression` | 14 |
| `RecordExpressionProperties` | 14 |
| `HookDeclaration` | 13 |
| `ImportSpecifier` | 13 |
| `MatchIdentifierPattern` | 13 |
| `AsExpression` | 12 |
| `DeclareVariable` | 12 |
| `ImportDefaultSpecifier` | 12 |
| `KeyofTypeAnnotation` | 12 |
| `MatchMemberPattern` | 12 |
| `InterfaceDeclaration` | 11 |
| `JSXStringLiteral` | 11 |
| `MatchExpression` | 11 |
| `MatchRestPattern` | 11 |
| `NumberLiteralTypeAnnotation` | 11 |
| `DeclareModule` | 10 |
| `EnumStringBody` | 10 |
| `MatchArrayPattern` | 10 |
| `MatchWildcardPattern` | 10 |
| `ComponentTypeAnnotation` | 9 |
| `DeclareFunction` | 9 |
| `EnumDefaultedMember` | 9 |
| `IndexedAccessType` | 9 |
| `InferTypeAnnotation` | 9 |
| `JSXExpressionContainer` | 9 |
| `DoWhileStatement` | 8 |
| `ExportSpecifier` | 8 |
| `ImportAttribute` | 8 |
| `InterfaceExtends` | 8 |
| `BigIntTypeAnnotation` | 7 |
| `ClassImplements` | 7 |
| `ObjectTypeMappedTypeProperty` | 7 |
| `QualifiedTypeIdentifier` | 7 |
| `TypeofTypeAnnotation` | 7 |
| `MatchInstanceObjectPattern` | 6 |
| `MatchInstancePattern` | 6 |
| `MatchOrPattern` | 6 |
| `OpaqueType` | 6 |
| `OptionalIndexedAccessType` | 6 |
| `RecordDeclarationStaticProperty` | 6 |
| `DeclareOpaqueType` | 5 |
| `ImportExpression` | 5 |
| `JSXNamespacedName` | 5 |
| `JSXSpreadAttribute` | 5 |
| `MatchAsPattern` | 5 |
| `TupleTypeLabeledElement` | 5 |
| `DeclareExportAllDeclaration` | 4 |
| `DeclareModuleExports` | 4 |
| `EnumNumberMember` | 4 |
| `IntersectionTypeAnnotation` | 4 |
| `MatchUnaryPattern` | 4 |
| `QualifiedTypeofIdentifier` | 4 |
| `DeclareEnum` | 3 |
| `DeclareTypeAlias` | 3 |
| `EnumNumberBody` | 3 |
| `JSXClosingFragment` | 3 |
| `JSXFragment` | 3 |
| `JSXOpeningFragment` | 3 |
| `ObjectTypeCallProperty` | 3 |
| `DeclareInterface` | 2 |
| `DeclareNamespace` | 2 |
| `EmptyTypeAnnotation` | 2 |
| `EnumBigIntBody` | 2 |
| `EnumBigIntMember` | 2 |
| `EnumBooleanBody` | 2 |
| `EnumBooleanMember` | 2 |
| `EnumStringMember` | 2 |
| `ExportAllDeclaration` | 2 |
| `ImportNamespaceSpecifier` | 2 |
| `InterfaceTypeAnnotation` | 2 |
| `JSXEmptyExpression` | 2 |
| `JSXMemberExpression` | 2 |
| `JSXSpreadChild` | 2 |
| `ObjectTypeInternalSlot` | 2 |
| `TupleTypeSpreadElement` | 2 |
| `WithStatement` | 2 |
| `AsConstExpression` | 1 |
| `BigIntLiteralTypeAnnotation` | 1 |
| `DeclareHook` | 1 |
| `EnumSymbolBody` | 1 |
| `ExportNamespaceSpecifier` | 1 |
| `NeverTypeAnnotation` | 1 |
| `ObjectTypeSpreadProperty` | 1 |
| `RecordDeclarationImplements` | 1 |
| `SymbolTypeAnnotation` | 1 |
| `UndefinedTypeAnnotation` | 1 |
| `UnknownTypeAnnotation` | 1 |
| `BooleanLiteralTypeAnnotation` | 0 |
| `CoverEmptyArgs` | 0 |
| `CoverInitializer` | 0 |
| `CoverRestElement` | 0 |
| `CoverTrailingComma` | 0 |
| `CoverTypedIdentifier` | 0 |
| `DeclaredPredicate` | 0 (1 after PD-3 was fixed) |
| `Directive` | 0 |
| `DirectiveLiteral` | 0 |
| `ExistsTypeAnnotation` | 0 |
| `ImplicitCheckedCast` | 0 |
| `InferredPredicate` | 0 (10 after PD-3 was fixed) |
| `Metadata` | 0 |
| `SHBuiltin` | 0 |
| `TSAnyKeyword` | 0 |
| `TSArrayType` | 0 |
| `TSAsExpression` | 0 |
| `TSBigIntKeyword` | 0 |
| `TSBooleanKeyword` | 0 |
| `TSCallSignatureDeclaration` | 0 |
| `TSConditionalType` | 0 |
| `TSConstructorType` | 0 |
| `TSEnumDeclaration` | 0 |
| `TSEnumMember` | 0 |
| `TSFunctionType` | 0 |
| `TSIndexSignature` | 0 |
| `TSIndexedAccessType` | 0 |
| `TSInterfaceBody` | 0 |
| `TSInterfaceDeclaration` | 0 |
| `TSInterfaceHeritage` | 0 |
| `TSIntersectionType` | 0 |
| `TSLiteralType` | 0 |
| `TSMethodSignature` | 0 |
| `TSModifiers` | 0 |
| `TSModuleBlock` | 0 |
| `TSModuleDeclaration` | 0 |
| `TSModuleMember` | 0 |
| `TSNeverKeyword` | 0 |
| `TSNumberKeyword` | 0 |
| `TSParameterProperty` | 0 |
| `TSPropertySignature` | 0 |
| `TSQualifiedName` | 0 |
| `TSStringKeyword` | 0 |
| `TSSymbolKeyword` | 0 |
| `TSThisType` | 0 |
| `TSTupleType` | 0 |
| `TSTypeAliasDeclaration` | 0 |
| `TSTypeAnnotation` | 0 |
| `TSTypeAssertion` | 0 |
| `TSTypeLiteral` | 0 |
| `TSTypeParameter` | 0 |
| `TSTypeParameterDeclaration` | 0 |
| `TSTypeParameterInstantiation` | 0 |
| `TSTypePredicate` | 0 |
| `TSTypeQuery` | 0 |
| `TSTypeReference` | 0 |
| `TSUndefinedKeyword` | 0 |
| `TSUnionType` | 0 |
| `TSUnknownKeyword` | 0 |
| `TSVoidKeyword` | 0 |

</details>

---

## 8. What is still not covered

Stated plainly, because the point of a manifest is the second list.

- **Readability.** No gate judges it; a round trip cannot. Spec §7.4 asks for a
  sample read by hand, and that belongs to the final review, not to a test.
- **Parenthesized TypeScript types**, beyond an identifier or a nested `(` —
  PD-2 means the parser cannot express them, so 20 of the matrix's 30
  TypeScript type payloads run unwrapped.
- **`TSModuleDeclaration`**, and the `Directive`/`DirectiveLiteral` pair: no
  source spelling in this parser reaches those arms, so they are exercised
  only by the compiler proving the dispatch exhaustive.
- **`Annotation::Sem`** output is checked by its own unit tests
  (`src/annotate.rs`), not by any round trip — annotated output is by
  construction not reparseable.
- **Sourcemaps** are out of scope for the whole crate (spec §6).
- **The 151 unparseable and 1 cover-grammar files** of the Tier 2 sweep, and
  the 25 unparseable + 3 cover files of Tier 1: there is no tree to generate
  from. Each set is enumerated — by name in `tests/corpus.rs`'s `Expected`
  pins, by name and diagnostic in the sweep's own output.
- **Anything the parser cannot produce.** This crate's domain is the trees
  `hermes-parser` builds; a hand-built tree can hold combinations no source
  spells, and the only guarantee there is that generation returns a
  `GenJsError` rather than panicking.
