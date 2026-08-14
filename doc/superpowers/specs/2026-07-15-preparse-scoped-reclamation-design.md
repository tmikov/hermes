# PreParse scoped reclamation (design)

> **Date:** 2026-07-15. **Branch:** `rust` (base `static_h`). **Component:** JS Parser — completing
> the Pre/Lazy passes.
> **Status:** design, pre-plan. Follow-up to `specs/2026-06-28-pre-lazy-passes-design.md`.

## 1. Problem

The Pre/Lazy phase shipped with the C++ `AllocationScope` discipline classified as an unportable
memory shortcut. That classification was wrong in the sense that matters: **PreParse's entire
memory rationale is that the pass which must touch every byte never holds more than the skeleton
spine plus the currently-open function nest.** In C++, `parseFunctionHelper` creates keeper nodes
with blank bodies *before* an `AllocationScope` (JSParserImpl.cpp:517-546), parses the real body
*inside* it (cpp:548), and reclaims the body subtree on scope exit; a whole-pass scope in the
`PreParser` wrapper (cpp:7523) reclaims the rest. Peak ≈ O(largest open function nest).

The Rust PreParse builds the full AST and reclaims it only when the `GCLock` drops:
peak = O(whole file). Since peak memory is the binding constraint on the devices lazy compilation
exists for (mobile OOM kills happen at peak), a setup pass that transiently allocates everything
has already spent the budget the scheme protects. Lazy parsing that retains the entire AST during
its indexing pass is not lazy in any meaningful sense. This phase restores the C++ memory profile.

The LazyParse skeleton and the demand path are unaffected — they are already genuinely lazy
(deferred bodies are never built).

## 2. Mechanism decision: bump-semantics truncate scope

Chosen over a sweep-style free-list scope and over a no-AST PreParse mode:

- **A. Truncate scope (chosen).** Exact analog of the C++ `BumpPtrAllocator::pushScope`/`popScope`
  (`include/hermes/Support/Allocator.h:500-521`): record both arena deque lengths at scope open;
  on drop, truncate both deques back. O(suffix), no free-list interaction, nesting is pure bump
  save/restore.
- **B. Sweep-style scope (rejected).** Pushing suffix slots onto the free lists reuses `gc()`'s
  machinery, but slots reused from the free list *inside* a later scope sit at pre-watermark deque
  positions and escape that scope's reclamation — reclamation degrades as the pass runs — plus
  double-free bookkeeping for nested scopes.
- **C. No-AST PreParse (rejected).** The parser consumes its own nodes mid-parse (cover-expression
  reparse, export-kind detection), so a no-build mode forks every production — the fork the C++
  deliberately avoided by choosing keeper + scope.

### Soundness argument (the load-bearing facts, all verified against the code)

1. `Context::gc()` takes `&mut self` (`rust/crates/ast/src/context.rs:459`), so collection is
   statically impossible while a `GCLock` (and hence any scope) lives — never mid-parse.
2. The free lists (`free_nodes`, `free_list_elements`) are populated **only** by `gc()`'s sweep
   (context.rs:543-544, 557-559). During a pass on a Context that has not been `gc()`'d,
   allocation (`context.rs:265-292`) is therefore append-only and a deque-length watermark is a
   complete description of "allocated since scope open".
3. The no-escape discipline is the C++ one, enforceable at one point: `JSParserImpl` holds no
   `Node`-typed fields (verified), so nodes leave a body parse only through return values, and the
   PreParse branch controls the single return value (it discards the body and returns the
   pre-scope keeper).
4. The one *counted* escape channel — `NodeRc` — is checked: scope drop `debug_assert`s
   `count == 0` on every suffix entry before truncating (a `NodeRc` into the suffix would dangle).
5. `StorageEntry` and `NodeListElement` own no heap (children are refs, strings are atom handles,
   attributes are `Cell`s), so truncation is a plain drop. **The plan must re-verify this claim
   against the generated `node.rs` before implementing truncate** (any node field that owns heap
   would need `Drop` to run, which truncate does anyway — the check is that nothing outside the
   entry is kept alive by it).

### Documented benign caveat

If the Context ran `gc()` *before* the pass, its free lists are non-empty and in-scope allocations
may pop pre-watermark slots; those slots escape truncation as unreferenced garbage until the next
`gc()`. Degraded reclamation, never unsoundness. Documented on the API; no mitigation.

## 3. The arena scope API (`ast` crate)

```rust
// On GCLock:
/// SAFETY: no reference into nodes or list elements allocated after this scope
/// opens may survive its drop, and no NodeRc may point into them (debug-asserted).
/// Mirrors the C++ AllocationScope discipline (Support/Allocator.h:500).
pub unsafe fn alloc_scope(&self) -> AllocationScope<'_, 'ast, 'ctx>;

pub struct AllocationScope<...> { /* &GCLock + saved nodes.len() + list_elements.len() */ }
impl Drop for AllocationScope<...> {
    // debug_assert count==0 over both suffixes (via Deque::iter_from), then
    // truncate both deques.
}
```

Named `AllocationScope` for C++ affinity. Construction is `unsafe fn` — the contract is the
discipline above; it cannot be checked by the compiler, exactly like the C++.

`support::Deque` grows two methods:
- `truncate(len)` — drop entries ≥ `len`, freeing fully-vacated trailing chunks. Compatible with
  the never-move invariant (`deque.rs:47` `push` returns `&T`): survivors are untouched.
- `iter_from(index)` — positions via chunk-boundary arithmetic (chunks have doubling capacities,
  `deque.rs:14-27`; a handful of boundary comparisons, no entry walking) so scope drop is
  O(suffix), not O(deque).

Debug builds may additionally poison-before-drop; decided in the plan if cheap.

## 4. Parser integration (PreParse-only, two sites — mirroring the two C++ sites)

### Site 1 — keeper discipline in `parse_function_helper_inner` (cpp:516-560)

A branch taken only when `pass == ParserPass::PreParse`, placed where the C++ has it — after
params / return type / the `l_brace` check and the `SaveFunctionState` guard
(`functions.rs:203`), before the body parse:

1. **Pre-scope:** build the blank body (`BlockStatement` with an empty list) and the keeper node
   (`FunctionDeclaration`/`FunctionExpression` with the already-parsed id, params, typeParams,
   returnType, predicate, and the blank body) — cpp:517-546. Everything the keeper references is
   pre-watermark: params were parsed earlier and the keeper's `NodeList::from_iter` runs pre-scope.
2. **In scope** (cpp:548): call the existing `parse_function_body(...)`. Its PreParse side-table
   store (cpp:803-810) records only offsets/bools/owned bytes — safe across truncation.
3. **Post-scope:** extract the real body's end `SMLoc` (plain data), drop the scope (the body
   subtree is reclaimed), return `set_location(start_loc, end, keeper)` (cpp:559; writes only the
   keeper's metadata `Cell`). A `?`/None from the body parse drops the scope guard too — the C++
   dtor path.

The PreParse skeleton's function bodies are therefore **blank**, as in C++. Nothing reads the
PreParse AST (the pass output is the side-table + parser flags; `pre_parse_buffer` discards the
`Program`), so this is unobservable outside memory accounting.

### Site 2 — whole-pass scope in `pre_parse_buffer` (cpp:7523)

Open a scope before `parse()`, drop it before returning the parser. Reclaims the skeleton spine +
keepers. Needed for the real driver flow where PreParse and LazyParse share one `Context` (C++
does this via the `PreParser` wrapper's `scope_` member held by the returned `shared_ptr`; the
Rust version is tighter-scoped and sound because the returned `JSParserImpl` holds no node
references and the `Program` result is discarded internally).

### Deliberately not scoped (matching C++)

- Arrows: no `AllocationScope` at the arrow site (cpp:5849 has none) — arrow bodies are reclaimed
  by the enclosing function's scope; top-level arrows wait for the pass scope.
- Nested functions' keepers: allocated inside the enclosing body's scope and reclaimed with it
  (not needed afterwards) — same as C++.

Net profile: peak ≈ skeleton spine + the currently-open function nest.

## 5. Validation

### The memory oracle (new; the phase's measurable gate)

`Context::num_nodes()` (context.rs:567) counts allocated slots (live + free) and truncation
shrinks it — a direct high-water probe. New test in the parser crate:

- Synthesize N (~200) copies of a moderate (~40-node) function.
- Assert `num_nodes()` after `pre_parse_buffer` ≤ (a small multiple of ONE function's node count
  + spine) — i.e. not O(N).
- Ratio guard: preparse `num_nodes()` < eager-parse `num_nodes()` / 10 on the same source.
- Analogous assertions for list elements via a new `Context::num_list_elements()` counter
  (mirroring `num_nodes()`, context.rs:567; does not exist yet — this phase adds it).

Generous constants: the test checks the *shape* (O(largest open nest) vs O(file)), not exact
counts.

### Semantic gates (all unchanged, all now exercising the scopes)

- **Oracle B** (`preparse_differential`, 13 lazy + 76 plain files) must stay byte-identical vs
  hermesc — every corpus run now crosses keeper creation + truncation.
- **FullParse differential** 8/8 unchanged (the branch is PreParse-only).
- **Oracle A** (`lazy_reparse`) unchanged (consumes the table).
- Optional, honestly framed: try `cargo miri test` on the preparse tests as a use-after-free
  canary for the discipline; if the pre-existing juno-derived unsafe already trips miri, note it
  and skip rather than chase it.

## 6. Bundled hardening (from the 2026-07-14 review)

1. **Oracle B over Flow/TS.** Add the dialect flags to *both* `preparse-dump` binaries (mirroring
   `ast-dump`'s flag plumbing: `-parse-flow`/`-parse-ts` on the C++ tool, `--parse-flow`/
   `--parse-ts` on the Rust bin) and run the existing `parser_corpus_flow` + `parser_corpus_ts`
   dirs through `preparse_differential` with matched flags. This also settles empirically the
   SavePoint-speculation × arrow-store interaction under typed arrows: any double-store divergence
   shows up as a byte diff.
2. **Oracle A located dumps.** Switch `lazy_reparse`'s `dump_node` to the with-sm overload with
   `LocationDumpMode::LocAndRange` — eager and demand-parsed bodies come from the same buffer, so
   located dumps must match byte-for-byte. Starts covering the `set_prev_token_end_loc` /
   show-source machinery (cpp:758-763) that nothing observes today.
3. **Docs.** Amend the roadmap + handoff: the Pre/Lazy block's "AllocationScope not replicated"
   deviation becomes "ported via the arena `AllocationScope` (truncate semantics)"; the Parser
   row's deviation list shrinks to one (side-table threaded on the parser). Update the stale
   code comments that describe the shortcut as not replicated (`functions.rs:212-218`,
   `pre_lazy.rs:109-115`).

## 7. Out of scope

- Reclamation for LazyParse or FullParse (C++ has none there either).
- Returning chunk memory to the OS beyond freed trailing chunks (C++ bump slabs are retained for
  reuse too — same profile).
- Any change to the side-table contents, the stub shape, or `parse_lazy_function` (Oracle B pins
  them).
