# JS Parser — the Pre/Lazy passes (design)

> **Date:** 2026-06-28. **Branch:** `rust` (base `static_h`). **Component:** JS Parser, final phase.
> **Status:** design, pre-plan. After this phase the Parser component is COMPLETE and Sema is next.

## 1. Goal & scope

Faithfully port the three parser passes the eager Full-pass port currently no-ops, completing the
JS Parser component:

- **`FullParse`** — eager, parses everything (what the Rust port does today; unchanged).
- **`PreParse`** — a full eager walk whose *only* surviving side-effect is a per-function metadata
  side-table (`PreParsedBufferInfo`); the AST is discarded.
- **`LazyParse`** — re-parses, but skips (seeks past) function bodies at or above a byte-length
  threshold, emitting a stub `BlockStatement` with `is_lazy_function_body = true`; sub-threshold
  bodies parse normally.
- **On-demand `parse_lazy_function(kind, param_yield, param_await, start)`** — re-parses a single
  previously-deferred function body when a consumer demands it.

**Scope is the full port of all three passes plus the demand entry point** (per the
`implement-components-completely` convention), even though the Rust port has no VM/IRGen consumer
yet — in C++ the demand entry is called by the runtime on first invocation of a deferred function.
Here its only current consumer is the validation harness, which is by design (see §4, Oracle A).

**No AST changes.** The `BlockStatement` node already carries every lazy decoration field
(`buffer_id`, `is_lazy_function_body`, `param_yield`, `param_await`, `contains_arrow_functions`,
`may_contain_arrow_functions_using_arguments` — `rust/crates/ast/src/node.rs:913-918`), so
`generated_idempotent` stays the guardrail and no node is added.

**C++ source of truth:** `lib/Parser/JSParserImpl.{h,cpp}`, `include/hermes/Parser/PreParser.h`,
`include/hermes/AST/Context.h` (the threshold). Key line ranges are cited inline below.

## 2. Components & data structures

One-for-one with the C++. New/changed pieces:

| C++ | Rust |
|---|---|
| `enum ParserPass { FullParse, PreParse, LazyParse }` + `pass_{FullParse}` (`JSParserImpl.h:179`) | `ParserPass` enum + `pass: ParserPass` field on the parser, default `FullParse` |
| `PreParsedFunctionInfo { SMLoc end; bool strictMode; SmallVector<SmallString<24>> directives; bool containsArrowFunctions; bool mayContainArrowFunctionsUsingArguments; }` (`PreParser.h:38-58`) | same struct. `directives` is **owned** bytes (`Vec<Vec<u8>>`, WTF-8), since the C++ note says atoms are arena-reclaimed between passes and must be copied |
| `PreParsedBufferInfo { DenseMap<SMLoc, PreParsedFunctionInfo> functionInfo; }` + `PreParsedData` on `Context` (`PreParser.h:60-81`) | `PreParsedBufferInfo { HashMap<u32 /*start offset*/, PreParsedFunctionInfo> }`, stored on the AST `Context` like `PreParsedData`. Our `SMLoc` is `(SourceId, u32)`; we key by the `u32` offset within the buffer |
| `Context::get/setPreemptiveFunctionCompilationThreshold()` (default `0`, `Context.h:236,516-521`) | `preemptive_function_compilation_threshold: u32` + getter/setter on the AST `Context` |
| `SaveFunctionState` RAII (`JSParserImpl.h:1699-1740`): saves/restores strict-mode, `seenDirectives_` size, `isArrowFunction_`, `containsArrowFunctions_`, `mayContainArrowFunctionsUsingArguments_`; on enter sets `isArrowFunction_`, and `containsArrowFunctions_`/`mayContain…` per arrow-vs-not | a Drop-guard struct (same pattern as the existing `ParamFlagGuard` / strict save-restore), restoring all five on every `?` early-return. **Subsumes** the parser's current ad-hoc strict save/restore |
| `seenDirectives_` (`JSParserImpl.h:220`) + `copySeenDirectives()` (`JSParserImpl.cpp`) | `seen_directives: Vec<...>` + a copy-to-owned helper |
| `isArrowFunction_` / `containsArrowFunctions_` / `mayContainArrowFunctionsUsingArguments_` (`JSParserImpl.h:225,236,246`) | three parser fields; updated only by the `SaveFunctionState` guard, except `may_contain…` is also set at the `arguments`-identifier site |
| `preParseBuffer(ctx, bufferId, strict)` (`JSParserImpl.cpp:7534`) | `pre_parse_buffer(ctx, buffer_id, strict)` |
| `parseLazyFunction(kind, paramYield, paramAwait, SMLoc start)` (`JSParserImpl.cpp:7548`) | `parse_lazy_function(kind, param_yield, param_await, start)` |
| `seek(SMLoc)` (`JSParserImpl.h:128`) | `seek(offset)` |

The dormant `_param_yield`/`_param_await` parameters already threaded into `parse_function_body`
(`rust/crates/parser/src/js/functions.rs:347-348`) lose their `_` prefix and become live. The
`arguments`-identifier site (`JSParserImpl.cpp:2508-2511`) sets
`may_contain_arrow_functions_using_arguments` when `is_arrow_function` is set.

## 3. Pass control flow

All three behaviors are localized to `parse_function_body` plus the two entry points — the C++ shape.

### FullParse (unchanged)
Parse the block eagerly; no side-table, no skipping. Stays byte-for-byte under
`hermesc -dump-ast`. L0 must not regress this.

### PreParse (`JSParserImpl.cpp:803-810`, second site `5896-5908`)
Parse everything eagerly, but after each `parse_function_body` block completes, record:

```
pre_parsed[body.start_offset] = PreParsedFunctionInfo {
    end: body.end,
    strict_mode: is_strict_mode(),
    directives: copy_seen_directives(),
    contains_arrow_functions,
    may_contain_arrow_functions_using_arguments,
}
```

The AST is discarded; only the table survives. Entry point `pre_parse_buffer(ctx, buffer_id, strict)`
sets strict mode, runs `parse()` in `PreParse` mode, and returns the parser carrying the populated
table (+ `use_static_builtin` and friends, as C++ does).

### LazyParse (`JSParserImpl.cpp:747-796`)
Re-parse. In `parse_function_body`, when `pass == LazyParse && !eagerly` and the recorded
`end - start >= threshold`:

1. `seek(end)`, then `advance(grammar_ctx)`.
2. `set_prev_token_end_loc(end)` — necessary so lazily-parsed arrows get the correct source range
   for "show source" (cpp:758-763).
3. `set_strict_mode(info.strict_mode)` — emulates re-parsing the `"use strict"` directive.
4. Fabricate a `BlockStatement { is_lazy_function_body = true }` whose statement list is the
   synthesized directive nodes (`ExpressionStatement(StringLiteral)` per recorded directive), and set
   `param_yield`, `param_await`, `buffer_id`, `contains_arrow_functions`,
   `may_contain_arrow_functions_using_arguments` from the params / table.
5. Return that stub with location `start..end`.

Functions below the threshold parse normally (fall through to the eager block parse).

### Demand parse (`JSParserImpl.cpp:7548-7600`)
`parse_lazy_function(kind, param_yield, param_await, start)`: `seek(start)`, set the two param
flags, then dispatch on the five deferrable node kinds, re-running the relevant `parse_*` with
`eagerly = true` and extracting the body/value:

- `FunctionExpression` → `parse_function_expression(eagerly=true)`
- `FunctionDeclaration` → `parse_function_declaration(ParamReturn, eagerly=true)`
- `ArrowFunctionExpression` → `parse_assignment_expression(ParamIn, eagerly=true)`
- `Property` (getter/setter) → `parse_property_assignment(true)`, extract `.value`
- `MethodDefinition` → `parse_class_body_impl(eagerly=true)`, extract the single member's `.value`

### SaveFunctionState placement
The guard wraps every function-scope entry — function declaration/expression, arrow, object
method/getter/setter, and class body (classes additionally force strict mode) — so arrow-bookkeeping
+ strict-mode + `seen_directives` nest and restore correctly across `?` early-returns. It replaces
the current ad-hoc strict save/restore the eager port already performs at these sites.

## 4. Validation

The eager AST is identical whether or not lazy parsing ran, so `hermesc -dump-ast` byte-for-byte
**cannot see this phase**. Two complementary oracles close that gap; neither replaces the other.

### Oracle B — PreParse-metadata differential (C++, byte-for-byte)
A dedicated C++ tool `tools/preparse-dump/preparse-dump.cpp` (mirroring the existing
`tools/js-lexer-dump` and `tools/json-parse-dump` precedent — a *separate tool registered via
`add_hermes_tool`*, NOT a `hermesc`/`CompilerDriver` flag) that:

1. Builds a `Context`, calls `JSParser::preParseBuffer(ctx, bufferId, strict)`.
2. Reads `ctx.getPreParsedBufferInfo(bufferId)->functionInfo`.
3. Sorts entries by start offset (the `DenseMap` is unordered; sorting makes output deterministic
   and identical across both sides).
4. Prints each function as a stable line, e.g.
   `start end strict | contains_arrow may_contain_arrow_args | dirCount dir0 dir1 …`.

The Rust `preparse-dump` bin (`rust/crates/parser/src/bin/preparse_dump.rs`) emits the identical
format. A `parser_differential`-style test (`REQUIRE_DIFFERENTIAL=1`) asserts byte-for-byte equality
over the corpus. This is the only way to validate the side-table's lazy-only metadata
(`contains_arrow_functions`, `may_contain_arrow_functions_using_arguments`, strict flag, directives,
recorded end-loc) — none of which appears in any AST.

### Oracle A — reparse-equivalence (Rust-only, no new C++)
A Rust integration test that, per corpus file:

1. Eager (`FullParse`) parse → collect `start_offset → function-subtree` map. This AST is already
   `hermesc`-verified by the existing `parser_differential` gate.
2. `pre_parse_buffer` → table; then `LazyParse` with **threshold 0** (defer every eligible function)
   → skeleton AST with stubs.
3. **Assert the skeleton's set of deferred-function start offsets equals the eager set.** This
   directly catches `seek`/`advance` resume corruption: if the post-skip restart is wrong, later
   function offsets drift.
4. For each stub, call `parse_lazy_function(kind, param_yield, param_await, start)` and assert its
   body's ESTree-JSON dump equals the corresponding eager function's body dump (transitively
   `hermesc`-faithful, since the eager body is).
5. Re-run at a **mid threshold** so some functions defer and some parse inline — exercises both
   branches of the deferral decision.

### Corpus
A dedicated `rust/crates/parser/tests/parser_corpus_lazy/` directory engineered to hit all five
deferrable kinds (function decl/expr, arrow, getter/setter, class method) × directives (`"use strict"`
and a custom directive) × nested arrows × `arguments`-inside-arrow × generators/async — used by
**both** oracles. Oracle B additionally runs over the existing 76-file plain corpus for breadth
(PreParse records every function regardless of size).

## 5. Sub-task slicing

Subagent-driven, TDD, each sub-task two-stage reviewed (spec-compliance with adversarial diffing +
structural-fidelity + code-quality), with a whole-phase capstone — the workflow used through P0–P8.

- **L0 — foundations.** `ParserPass` enum + `pass` field; `PreParsedFunctionInfo` /
  `PreParsedBufferInfo` on `Context`; threshold knob; `SaveFunctionState` Drop-guard wired into all
  function-scope entries (subsuming the existing strict save/restore); the `is_arrow_function` /
  `contains_arrow_functions` / `may_contain_arrow_functions_using_arguments` fields + the
  `arguments` site; `seen_directives` + copy helper. **No behavior change to `FullParse`** — the full
  pre-existing differential (plain/Flow/TS/JSX) must stay byte-for-byte green.
- **L1 — PreParse.** The side-table population in `parse_function_body` (both sites) +
  `pre_parse_buffer`; the C++ `preparse-dump` tool + Rust `preparse-dump` bin + Oracle B differential
  gate over the new lazy corpus + the existing plain corpus.
- **L2 — LazyParse + demand.** The skip-and-stub path + `seek` + `set_prev_token_end_loc` + the stub
  fabrication, and `parse_lazy_function` dispatch over the five kinds; Oracle A reparse-equivalence
  test (offset-set check + per-body equivalence + threshold sweep).
- **Capstone.** Map every C++ `pass_` / `SaveFunctionState` / lazy site to its Rust production;
  confirm zero deferral markers remain; structural-fidelity grep (templates→generics, RAII→Drop,
  no silent flattening); re-run all gates + the full pre-existing differential.

## 6. Faithful-port conventions (carried from prior phases)

- C++ default arguments are spec — read the header (`parse_function_body`'s defaults, `lookahead1`'s
  `RequireNoNewLine = true`, grammar-context per call site).
- C++ `template`s stay Rust generics; C++ RAII guards become Drop-guards or explicit save/restore
  wrappers that survive `?` early-returns.
- Pointer→offset adaptations are commented where a C++ method moved.
- Commit directly to `rust`; never open a PR or merge. Commit messages end with
  `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.

## 7. Out of scope / deferred

- IRGen/BCGen/VM consumers of lazy parsing — those components don't exist in the Rust port yet. The
  demand entry point is fully implemented and exercised by Oracle A, but has no production caller
  until the VM is ported.
- Magic-URL / static-builtin registration plumbing beyond what `pre_parse_buffer` already returns is
  ported only insofar as the existing parser already models it.
