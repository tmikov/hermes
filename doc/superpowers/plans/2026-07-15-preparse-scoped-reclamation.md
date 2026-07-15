# PreParse Scoped Reclamation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore the C++ PreParse memory profile (peak ≈ skeleton + open function nest, not O(file)) by porting the `AllocationScope` discipline as a bump-semantics truncate scope on the GC arena.

**Architecture:** `support::Deque` gains `truncate`/`iter_from`; the `ast` crate gains an `unsafe` `AllocationScope` guard on `GCLock` (save both deque lengths, debug-validate, truncate on drop); the parser gains the two C++ scope sites — the keeper-with-blank-body branch in `parse_function_helper_inner` (cpp:516-560) and the whole-pass scope in `pre_parse_buffer` (cpp:7523). Gated by new memory-shape tests + the unchanged Oracle A/B semantic differentials. Bundles two oracle-hardening items (Oracle B over Flow/TS; Oracle A located dumps).

**Tech Stack:** Rust workspace `rust/` (crates `support`, `ast`, `parser`), C++ `tools/preparse-dump` oracle, the existing differential harnesses.

**Spec:** `doc/superpowers/specs/2026-07-15-preparse-scoped-reclamation-design.md` (read it first — the soundness argument in §2 governs every task here).

## Global Constraints

- **Branch `rust`; commit directly; never open a PR or merge.** Commit messages end with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- **Never `cd`** out of the project root; use `--manifest-path rust/Cargo.toml`; subshell `(cd ...; ...)` only if a tool truly requires it.
- **The C++ is the spec.** Cited sites: `lib/Parser/JSParserImpl.cpp:516-560` (keeper + per-function scope), `:548` (scope), `:7522-7546` (`PreParser` wrapper + whole-pass scope), `include/hermes/Support/Allocator.h:500-521` (`AllocationScope` = bump push/pop).
- **Zero `cargo build` warnings; no new clippy lints.**
- **The semantic gates must not move:** Oracle B (`preparse_differential`) byte-identical vs hermesc; FullParse `parser_differential` 8/8; Oracle A (`lazy_reparse`) green. The side-table contents, stub shape, and `parse_lazy_function` are pinned — this phase must not change them.
- **Soundness invariants (spec §2), relied on by every task:** `Context::gc()` takes `&mut self` (impossible under a `GCLock`); free lists are populated only by `gc()` (allocation append-only during a pass); `JSParserImpl` has no `Node`-typed fields (escapes only via return values).
- Oracle builds: `cmake --build cmake-build-asan --target hermesc preparse-dump` (the `ast-dump` oracle for `parser_differential` is `hermesc` itself; the Rust `ast-dump` bin builds via cargo).

### Validation commands (used throughout)

```bash
cargo build  --manifest-path rust/Cargo.toml            # zero warnings
cargo test   --manifest-path rust/Cargo.toml -p support
cargo test   --manifest-path rust/Cargo.toml -p ast
cargo test   --manifest-path rust/Cargo.toml -p parser
REQUIRE_GEN=1 cargo test --manifest-path rust/Cargo.toml -p ast --test generated_idempotent
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test parser_differential
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test preparse_differential
cargo test   --manifest-path rust/Cargo.toml -p parser --test lazy_reparse
```

---

## File structure

| File | Responsibility | Task |
|---|---|---|
| `rust/crates/support/src/deque.rs` | `truncate(len)` + `iter_from(index)` | 1 |
| `rust/crates/ast/src/context.rs` | `num_list_elements()`, `AllocationScope` guard + `GCLock::alloc_scope` | 2 |
| `rust/crates/parser/src/js/functions.rs` | keeper branch in `parse_function_helper_inner` (Site 1) | 3 |
| `rust/crates/parser/src/js/pre_lazy.rs` | whole-pass scope in `pre_parse_buffer` (Site 2); Site-1 targeted test | 3, 4 |
| `rust/crates/parser/tests/preparse_memory.rs` (new) | the memory-shape oracle | 4 |
| `tools/preparse-dump/preparse-dump.cpp` + `rust/crates/parser/src/bin/preparse_dump.rs` + `rust/crates/parser/tests/preparse_differential.rs` | dialect flags + Flow/TS corpus runs | 5 |
| `rust/crates/parser/tests/lazy_reparse.rs` | located dumps | 6 |
| `doc/superpowers/{RustPortRoadmap,SESSION-HANDOFF}.md` | docs amendment | 7 |

---

### Task 1: `Deque::truncate` + `Deque::iter_from` (support crate)

**Files:**
- Modify: `rust/crates/support/src/deque.rs` (methods after `iter_mut`, ~line 82; tests in the existing `#[cfg(test)]` module at the bottom)

**Interfaces:**
- Consumes: existing `Deque<T>` internals — `storage: Vec<Vec<T>>` of doubling-capacity chunks (`deque.rs:14-27`), `push` never lets a chunk grow past capacity (`deque.rs:47-59`), `new()` always leaves ≥ 1 chunk.
- Produces: `pub fn truncate(&mut self, len: usize)` and `pub fn iter_from(&self, index: usize) -> impl Iterator<Item = &T>`. Task 2 calls both.

- [ ] **Step 1: Write the failing tests** (in the existing test module):

```rust
#[test]
fn truncate_within_and_across_chunks() {
    // 2500 elements spans chunk 0 (1024) and chunk 1 (2048 capacity).
    let mut d = Deque::new();
    for i in 0..2500usize {
        d.push(i);
    }
    assert_eq!(d.len(), 2500);
    // Truncate within chunk 1.
    d.truncate(1500);
    assert_eq!(d.len(), 1500);
    assert_eq!(d.iter().copied().last(), Some(1499));
    // Survivors intact and re-push works.
    assert_eq!(d.iter().nth(1023).copied(), Some(1023));
    d.push(9999);
    assert_eq!(d.len(), 1501);
    assert_eq!(d.iter().copied().last(), Some(9999));
    // Truncate dropping the whole trailing chunk.
    d.truncate(500);
    assert_eq!(d.len(), 500);
    // Truncate to zero leaves a usable deque.
    d.truncate(0);
    assert_eq!(d.len(), 0);
    d.push(1);
    assert_eq!(d.len(), 1);
    // Truncate to exactly the current length is a no-op.
    d.truncate(1);
    assert_eq!(d.len(), 1);
}

#[test]
fn iter_from_positions_correctly() {
    let mut d = Deque::new();
    for i in 0..2500usize {
        d.push(i);
    }
    // Mid-chunk-1 start.
    let v: Vec<usize> = d.iter_from(1030).copied().take(3).collect();
    assert_eq!(v, vec![1030, 1031, 1032]);
    // Exactly at a chunk boundary.
    assert_eq!(d.iter_from(1024).copied().next(), Some(1024));
    // From zero == full iteration.
    assert_eq!(d.iter_from(0).count(), 2500);
    // From len() and beyond: empty.
    assert_eq!(d.iter_from(2500).count(), 0);
    assert_eq!(d.iter_from(9999).count(), 0);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --manifest-path rust/Cargo.toml -p support truncate_within`
Expected: FAIL — `truncate`/`iter_from` not found.

- [ ] **Step 3: Implement**

```rust
    /// Truncate the deque to `len` elements, dropping every element at
    /// index >= `len` and freeing fully-vacated trailing chunks. Surviving
    /// elements never move (only trailing elements/chunks are dropped), so
    /// references to them remain valid. Used by the AST arena's
    /// `AllocationScope` (bump-allocator save/restore semantics, mirroring
    /// the C++ `BumpPtrAllocator::pushScope`/`popScope`,
    /// hermes/Support/Allocator.h:500).
    pub fn truncate(&mut self, len: usize) {
        debug_assert!(len <= self.len(), "truncate beyond deque length");
        let mut remaining = len;
        let mut keep = 0usize; // number of chunks to keep
        for chunk in &mut self.storage {
            keep += 1;
            if remaining < chunk.len() {
                chunk.truncate(remaining);
                break;
            }
            remaining -= chunk.len();
            if remaining == 0 {
                break;
            }
        }
        // Always keep at least one chunk: `push` assumes storage is
        // non-empty (deque.rs `new()` pre-creates chunk 0).
        self.storage.truncate(keep.max(1));
    }

    /// Iterate over the elements starting at `index`. Positions by chunk
    /// arithmetic (a handful of chunk-boundary comparisons; skipped
    /// elements are not walked), so iterating a suffix is O(suffix).
    /// An `index` at or past `len()` yields an empty iterator.
    pub fn iter_from(&self, index: usize) -> impl Iterator<Item = &T> {
        let mut skip = index;
        let mut start_chunk = self.storage.len();
        for (i, chunk) in self.storage.iter().enumerate() {
            if skip < chunk.len() {
                start_chunk = i;
                break;
            }
            skip -= chunk.len();
        }
        self.storage[start_chunk..]
            .iter()
            .enumerate()
            .flat_map(move |(i, chunk)| {
                let s = if i == 0 { skip } else { 0 };
                chunk[s..].iter()
            })
    }
```

Note: do NOT touch `next_chunk_capacity` in `truncate` — re-growth simply reuses the doubled size (bounded by `MAX_CHUNK_CAPACITY`), and resetting it would complicate the never-move reasoning for zero benefit. Do NOT "fix" the pre-existing `is_empty()` oddity (it checks `storage.is_empty()`, not `len()==0`) — out of scope.

- [ ] **Step 4: Run tests** — `cargo test --manifest-path rust/Cargo.toml -p support` → PASS, and `cargo build --manifest-path rust/Cargo.toml` → zero warnings.
- [ ] **Step 5: Commit** — `rust(support): Deque::truncate + iter_from for arena allocation scopes`

---

### Task 2: `AllocationScope` on the GC arena (ast crate)

**Files:**
- Modify: `rust/crates/ast/src/context.rs` — `num_list_elements()` next to `num_nodes()` (~line 570); the `AllocationScope` struct + `GCLock::alloc_scope` after the `GCLock` impl (~line 711); tests in the existing `#[cfg(test)]` module.

**Interfaces:**
- Consumes: Task 1's `Deque::truncate`/`iter_from`; `Context` internals `nodes`/`list_elements` (`UnsafeCell<Deque<...>>`), `StorageEntry { ctx_id_markbit, count, inner }` (context.rs:40-52), `StorageEntry::is_free()`.
- Produces: `Context::num_list_elements(&self) -> usize`; `pub struct AllocationScope<'gcl, 'ast, 'ctx>`; `pub unsafe fn GCLock::alloc_scope(&self) -> AllocationScope<'_, 'ast, 'ctx>`. Tasks 3/4 call `alloc_scope`.

- [ ] **Step 1: Write the failing tests.** Build nodes with the same construction idiom as the existing `alloc_and_deep_match` test (context.rs:888) — copy its metadata/node helper shape rather than inventing one. Structure:

```rust
#[test]
fn alloc_scope_truncates_nodes_and_lists() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let base_nodes = gc.ctx().num_nodes();
    let base_elems = gc.ctx().num_list_elements();

    // Pre-scope survivor.
    let survivor = gc.alloc(/* node built per the alloc_and_deep_match idiom */);
    {
        let _scope = unsafe { gc.alloc_scope() };
        for _ in 0..100 {
            gc.alloc(/* same idiom */);
        }
        // A NodeList inside the scope allocates list elements.
        let a = gc.alloc(/* idiom */);
        let _list = NodeList::from_iter(&gc, vec![a]);
        assert_eq!(gc.ctx().num_nodes(), base_nodes + 102);
        assert!(gc.ctx().num_list_elements() > base_elems);
    }
    // Scope drop reclaimed everything allocated inside it.
    assert_eq!(gc.ctx().num_nodes(), base_nodes + 1);
    assert_eq!(gc.ctx().num_list_elements(), base_elems);
    // The pre-scope survivor is untouched (deep-check a field on it).
    let _ = survivor; // + a structural assertion per the existing test idiom
}

#[test]
fn alloc_scope_nests() {
    let mut ctx = Context::new();
    let gc = GCLock::new(&mut ctx);
    let base = gc.ctx().num_nodes();
    {
        let _outer = unsafe { gc.alloc_scope() };
        gc.alloc(/* idiom */); // 1 outer allocation
        {
            let _inner = unsafe { gc.alloc_scope() };
            for _ in 0..50 {
                gc.alloc(/* idiom */);
            }
        }
        assert_eq!(gc.ctx().num_nodes(), base + 1, "inner scope reclaimed");
        // Outer keeps allocating after the inner truncate (bump reuse).
        for _ in 0..10 {
            gc.alloc(/* idiom */);
        }
        assert_eq!(gc.ctx().num_nodes(), base + 11);
    }
    assert_eq!(gc.ctx().num_nodes(), base);
}
```

Do NOT write a test for the `count == 0` debug-assert path: triggering it requires a `NodeRc` into the truncated suffix, and that `NodeRc`'s own `Drop` would then touch freed storage inside the test — UB in the test itself. The happy-path tests + the assert are the coverage.

- [ ] **Step 2: Run to verify failure** — `cargo test --manifest-path rust/Cargo.toml -p ast alloc_scope` → FAIL (missing items).
- [ ] **Step 3: Implement**

`num_list_elements` (mirror `num_nodes`, context.rs:565-570):

```rust
    /// Returns the number of list-element slots which have been allocated.
    /// Includes elements currently in use as well as elements in the free
    /// list.
    pub fn num_list_elements(&self) -> usize {
        let list_elements = unsafe { &*self.list_elements.get() };
        list_elements.len()
    }
```

The scope (place after the `GCLock` impl; `Drop` may live in the same module and touch `Context`'s private fields):

```rust
/// RAII allocation scope over the arena: everything allocated (nodes AND
/// list elements) between construction and drop is reclaimed at drop, with
/// bump-allocator save/restore semantics. Port of the C++ `AllocationScope`
/// (hermes/Support/Allocator.h:500-521) as used by the parser's PreParse
/// pass (JSParserImpl.cpp:548, 7523).
///
/// See [`GCLock::alloc_scope`] for the safety contract.
pub struct AllocationScope<'gcl, 'ast, 'ctx> {
    lock: &'gcl GCLock<'ast, 'ctx>,
    nodes_watermark: usize,
    list_elements_watermark: usize,
}

impl Drop for AllocationScope<'_, '_, '_> {
    fn drop(&mut self) {
        let ctx: &Context<'_> = self.lock.ctx;
        let nodes = unsafe { &mut *ctx.nodes.get() };
        #[cfg(debug_assertions)]
        for entry in nodes.iter_from(self.nodes_watermark) {
            // A NodeRc into the suffix would dangle after truncation.
            debug_assert!(
                entry.count.get() == 0,
                "NodeRc points into a truncated AllocationScope suffix"
            );
            // gc() cannot run under a GCLock, so no suffix entry can be
            // free (free-list pops reuse only pre-watermark slots).
            debug_assert!(!entry.is_free(), "free entry in scope suffix");
        }
        nodes.truncate(self.nodes_watermark);
        let list_elements = unsafe { &mut *ctx.list_elements.get() };
        list_elements.truncate(self.list_elements_watermark);
    }
}
```

On `GCLock`:

```rust
    /// Open an allocation scope: everything allocated between this call and
    /// the returned guard's drop is reclaimed at drop (nodes and list
    /// elements). Mirrors the C++ `AllocationScope` discipline the PreParse
    /// pass uses (JSParserImpl.cpp:516-560).
    ///
    /// # Safety
    ///
    /// The caller must guarantee that when the guard drops:
    /// - no `&Node`, `NodeList`, or interior reference into an allocation
    ///   made after this call survives — the storage is freed and any such
    ///   reference dangles; and
    /// - no `NodeRc` points into those allocations (debug-asserted).
    ///
    /// If the `Context` ran `gc()` before this pass, in-scope allocations
    /// may be served from the free list at pre-watermark positions; those
    /// escape reclamation harmlessly (unreferenced until the next `gc()`).
    pub unsafe fn alloc_scope<'s>(&'s self) -> AllocationScope<'s, 'ast, 'ctx> {
        let nodes = unsafe { &*self.ctx.nodes.get() };
        let list_elements = unsafe { &*self.ctx.list_elements.get() };
        AllocationScope {
            lock: self,
            nodes_watermark: nodes.len(),
            list_elements_watermark: list_elements.len(),
        }
    }
```

Pre-implementation check (spec §2 item 5): confirm by reading the generated `node.rs` struct fields that `Node` variants own no heap (children are `&Node`/`NodeList`/`Option<&Node>`, strings are `AtomBytes` handles, attributes are `Cell`s of `Copy` types) so truncation's plain drop frees nothing that outlives it. Record the confirmation in the commit message.

- [ ] **Step 4: Run** — `cargo test --manifest-path rust/Cargo.toml -p ast` → PASS (incl. all pre-existing tests); `REQUIRE_GEN=1 cargo test ... --test generated_idempotent` → PASS; zero warnings.
- [ ] **Step 5: Commit** — `rust(ast): AllocationScope (bump-semantics truncate scope) + num_list_elements`

---

### Task 3: Site 1 — the keeper branch in `parse_function_helper_inner` (cpp:516-560)

**Files:**
- Modify: `rust/crates/parser/src/js/functions.rs` (insert after the `grammar_context` computation, ~line 210; also replaces the now-stale comment block at ~212-218)
- Test: `rust/crates/parser/src/js/pre_lazy.rs` (tests module)

**Interfaces:**
- Consumes: Task 2's `unsafe gc.alloc_scope()`; the in-scope locals of `parse_function_helper_inner` — `opt_id`, `param_list`, `type_parameters`, `return_type`, `predicate`, `is_generator`, `is_async`, `is_declaration`, `start_loc`, `grammar_context` (all already defined, functions.rs:104-210).
- Produces: PreParse now returns blank-bodied keeper function nodes and reclaims each real body subtree at function end. No signature changes.

- [ ] **Step 1: Write the failing test** (`pre_lazy.rs` tests; construction idiom identical to the existing `preparse_records_functions` test):

```rust
    // Site 1 (cpp:516-560): PreParse reclaims each function body when the
    // function completes. Measured by driving a PreParse parser manually
    // (no whole-pass scope yet) and comparing node counts against an eager
    // parse of the same source: the retained PreParse AST is the skeleton
    // spine + blank-bodied keepers, a small fraction of the full AST.
    #[test]
    fn preparse_reclaims_function_bodies() {
        use ast::context::Context;
        use support::manager::SourceErrorManager;
        use crate::lexer::{GrammarContext, JSLexer};
        use crate::js::{JSParserImpl, ParserPass};

        // One function with a fat body (~20 statements), repeated 50x.
        let mut src: Vec<u8> = Vec::new();
        for f in 0..50 {
            src.extend_from_slice(format!("function f{f}(a, b) {{\n").as_bytes());
            for i in 0..20 {
                src.extend_from_slice(
                    format!("  var x{i} = a + b * {i};\n").as_bytes(),
                );
            }
            src.extend_from_slice(b"  return a;\n}\n");
        }

        let count_nodes = |pass: ParserPass| -> usize {
            let mut sm = SourceErrorManager::new();
            let id = sm.add_buffer_bytes("t", &src);
            let mut ctx = Context::new();
            let gc = ctx.lock();
            let atoms = &gc.ctx().atom_table;
            let lexer =
                JSLexer::new(id, &mut sm, atoms, GrammarContext::AllowRegExp);
            let mut p = JSParserImpl::new_with_pass(&gc, lexer, pass);
            assert!(p.parse().is_some(), "parse failed");
            gc.ctx().num_nodes()
        };

        let eager = count_nodes(ParserPass::FullParse);
        let pre = count_nodes(ParserPass::PreParse);
        // Shape assertion, generous constant: the keepers + spine must be a
        // small fraction of the full AST (each body is ~20x its keeper).
        assert!(
            pre * 5 < eager,
            "PreParse retained O(file) AST: pre={pre} eager={eager}"
        );
    }
```

- [ ] **Step 2: Run to verify failure** — `cargo test --manifest-path rust/Cargo.toml -p parser preparse_reclaims_function_bodies` → FAIL (`pre` ≈ `eager`; no reclamation yet).
- [ ] **Step 3: Implement the keeper branch.** Insert in `parse_function_helper_inner` immediately after the `grammar_context` computation (C++ order: SaveFunctionState :510 → grammarContext :512-514 → the PreParse branch :516), replacing the stale "the C++ PreParse path (cpp:516-560) … not replicated" comment block:

```rust
        // cpp:516-560 — PreParse: create the keeper we want to keep BEFORE
        // the AllocationScope, parse the real body INSIDE the scope, and
        // reclaim the body subtree on scope exit. The keeper gets a blank
        // body; only the source extent (start..body end) is retained. The
        // side-table store inside parse_function_body (cpp:803-810) records
        // owned offsets/bools/bytes and safely survives the truncation.
        //
        // Adaptation: the C++ allocates the keeper node pre-scope and
        // returns it; we keep the keeper as a stack VALUE (its children —
        // params, blank body, id, types — are arena-allocated pre-scope)
        // and let set_location allocate it post-scope. Equivalent: either
        // way no keeper storage lies inside the truncated suffix.
        if self.pass == ParserPass::PreParse {
            // Blank body, unlocated, like the C++ `BlockStatementNode({},
            // false)` (cpp:531, 544).
            let blank_body = self.gc.alloc(Node::BlockStatement(
                BlockStatement::new(
                    NodeMetadata::new(self.dummy_range()),
                    NodeList::from_iter(self.gc, Vec::new()),
                    false,
                ),
            ));
            let params = NodeList::from_iter(self.gc, param_list);
            let keeper = if is_declaration {
                Node::FunctionDeclaration(FunctionDeclaration::new(
                    NodeMetadata::new(self.dummy_range()),
                    opt_id,
                    params,
                    blank_body,
                    type_parameters,
                    return_type,
                    predicate,
                    is_generator,
                    is_async,
                ))
            } else {
                Node::FunctionExpression(FunctionExpression::new(
                    NodeMetadata::new(self.dummy_range()),
                    opt_id,
                    params,
                    blank_body,
                    type_parameters,
                    return_type,
                    predicate,
                    is_generator,
                    is_async,
                ))
            };

            // cpp:548. SAFETY: nothing allocated inside the scope escapes —
            // the body subtree is discarded (only its end SMLoc, plain
            // data, is read out before the drop), the keeper and all its
            // children are pre-scope, and the side-table holds no node
            // references. On the error path the `?` drops the guard, which
            // is exactly the C++ dtor behavior.
            let scope = unsafe { self.gc.alloc_scope() };
            let body = self.parse_function_body(
                Param::default(),
                false,
                is_generator,
                is_async,
                grammar_context,
                /* parse_directives= */ true,
            )?;
            let body_end = body.range().end;
            // `body` must not be used past this point: the drop reclaims
            // its storage.
            drop(scope);
            return Some(self.set_location(start_loc, body_end, keeper));
        }
```

Notes for the implementer: (a) `Vec::new()` in the blank-body `from_iter` may need a type annotation (`Vec::<&'gc Node<'gc>>::new()`) — match `NodeList::from_iter`'s signature in the ast crate; (b) `param_list` is moved here, which is fine because this branch `return`s before the eager tail's own `NodeList::from_iter(self.gc, param_list)` (functions.rs:234); (c) the eager tail (functions.rs:224-261) stays byte-for-byte unchanged — FullParse/LazyParse behavior must not move.

- [ ] **Step 4: Run** — the new test PASSES; then the full gates:

```bash
cargo test --manifest-path rust/Cargo.toml -p parser              # whole crate
cmake --build cmake-build-asan --target hermesc preparse-dump
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test preparse_differential
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test parser_differential
cargo test --manifest-path rust/Cargo.toml -p parser --test lazy_reparse
```

Expected: ALL green. Oracle B byte-identical proves the keeper branch did not perturb the side-table; `parser_differential` 8/8 proves FullParse untouched. If Oracle B diverges, the bug is in this task — do not touch the table code to compensate.

- [ ] **Step 5: Commit** — `rust(parser): PreParse keeper + per-function AllocationScope (cpp:516-560)`

---

### Task 4: Site 2 — whole-pass scope in `pre_parse_buffer` (cpp:7523) + the memory oracle

**Files:**
- Modify: `rust/crates/parser/src/js/pre_lazy.rs` (`pre_parse_buffer`, ~line 116; its doc comment at ~109-115 loses the "no AllocationScope in Rust" deviation text)
- Create: `rust/crates/parser/tests/preparse_memory.rs`

**Interfaces:**
- Consumes: Task 2's `alloc_scope`; Task 3's keeper branch.
- Produces: `pre_parse_buffer` reclaims the entire pass AST (spine + keepers) before returning. Signature unchanged.

- [ ] **Step 1: Write the failing test** (`tests/preparse_memory.rs`, new file — full standard header + these contents):

```rust
//! Memory-shape oracle for the PreParse scoped reclamation
//! (specs/2026-07-15-preparse-scoped-reclamation-design.md §5).
//!
//! `Context::num_nodes()` counts allocated slots and `AllocationScope`
//! truncation shrinks it, so post-pass counts expose the reclamation shape:
//! after `pre_parse_buffer` (whole-pass scope, cpp:7523) the retained count
//! must be near zero — NOT O(file), and not even O(keepers).

use ast::context::Context;
use parser::js::JSParserImpl;
use parser::lexer::{GrammarContext, JSLexer};
use support::manager::SourceErrorManager;

fn gen_source(n: usize) -> Vec<u8> {
    let mut src = Vec::new();
    for f in 0..n {
        src.extend_from_slice(format!("function f{f}(a, b) {{\n").as_bytes());
        for i in 0..20 {
            src.extend_from_slice(format!("  var x{i} = a + b * {i};\n").as_bytes());
        }
        src.extend_from_slice(b"  return a;\n}\n");
    }
    src
}

fn eager_nodes(src: &[u8]) -> usize {
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer_bytes("t", src);
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let atoms = &gc.ctx().atom_table;
    let lexer = JSLexer::new(id, &mut sm, atoms, GrammarContext::AllowRegExp);
    let mut p = JSParserImpl::new(&gc, lexer);
    assert!(p.parse().is_some());
    gc.ctx().num_nodes()
}

fn preparse_nodes(src: &[u8]) -> (usize, usize) {
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer_bytes("t", src);
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let atoms = &gc.ctx().atom_table;
    let lexer = JSLexer::new(id, &mut sm, atoms, GrammarContext::AllowRegExp);
    let p = JSParserImpl::pre_parse_buffer(&gc, lexer, false)
        .expect("preparse failed");
    let n = (gc.ctx().num_nodes(), gc.ctx().num_list_elements());
    // The table must still be populated (reclamation must not eat it).
    let mut p = p;
    assert_eq!(p.take_pre_parsed().function_info.len(), 200 * 2 - 200);
    n
}

#[test]
fn preparse_retains_no_ast() {
    let one = gen_source(1);
    let many = gen_source(200);
    let e1 = eager_nodes(&one);
    let e200 = eager_nodes(&many);
    let (p200_nodes, p200_elems) = preparse_nodes(&many);
    // After the whole-pass scope, essentially nothing is retained: less
    // than a single function's AST, and >20x under the full AST.
    assert!(p200_nodes < e1, "retained nodes: {p200_nodes} (one-fn = {e1})");
    assert!(p200_nodes * 20 < e200, "nodes {p200_nodes} vs eager {e200}");
    assert!(p200_elems < e1, "retained list elements: {p200_elems}");
}
```

Note for the implementer: the `function_info.len()` expectation must match the generated source — 200 plain function decls with no arrows means exactly 200 body entries; fix the literal to `200` (the expression above is a reminder to COUNT, not a formula to keep — write `assert_eq!(..., 200)` after confirming against the actual output).

- [ ] **Step 2: Run to verify failure** — `cargo test --manifest-path rust/Cargo.toml -p parser --test preparse_memory` → FAIL on `p200_nodes < e1` (the spine + keepers are retained until the GCLock drops; Site 2 not implemented).
- [ ] **Step 3: Implement.** Replace the body of `pre_parse_buffer` (pre_lazy.rs:116-125), and rewrite its deviation note:

```rust
    /// Port of `JSParserImpl::preParseBuffer` (JSParserImpl.cpp:7534-7546).
    /// The C++ `PreParser` wrapper holds an `AllocationScope` (cpp:7523)
    /// that reclaims the whole pass AST when the returned shared_ptr dies;
    /// here the scope is opened around `parse()` and dropped before
    /// returning — tighter, and sound because the `Program` result is
    /// discarded and `JSParserImpl` holds no node references. The pass
    /// output is the side-table + parser flags only.
    pub fn pre_parse_buffer(
        gc: &'gc ast::context::GCLock<'ast, 'ctx>,
        lexer: JSLexer<'a>,
        strict: bool,
    ) -> Option<JSParserImpl<'gc, 'ast, 'ctx, 'a>> {
        let mut p = JSParserImpl::new_with_pass(gc, lexer, ParserPass::PreParse);
        p.lexer.set_strict_mode(strict);
        // SAFETY: the only node reference produced inside the scope is the
        // `Program` result, consumed by `.is_some()` before the drop.
        let scope = unsafe { gc.alloc_scope() };
        let ok = p.parse().is_some();
        drop(scope);
        if !ok {
            return None;
        }
        Some(p)
    }
```

- [ ] **Step 4: Run all gates** (same block as Task 3 Step 4, plus the new test):

```bash
cargo test --manifest-path rust/Cargo.toml -p parser --test preparse_memory
cargo test --manifest-path rust/Cargo.toml -p parser
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test preparse_differential
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test parser_differential
cargo test --manifest-path rust/Cargo.toml -p parser --test lazy_reparse
cargo build --manifest-path rust/Cargo.toml   # zero warnings
```

Expected: ALL green — in particular the `preparse_records_functions` and `preparse_reclaims_function_bodies` unit tests still pass (they drive the parser manually, without `pre_parse_buffer`, so Site 2 doesn't affect them) and Oracle B stays byte-identical (it uses `pre_parse_buffer` via the bin — the table survives the whole-pass truncate because it holds owned data only).

Optional (honestly framed): `cargo miri test --manifest-path rust/Cargo.toml -p parser --test preparse_memory` as a use-after-free canary. If miri trips on PRE-EXISTING arena unsafe (juno-derived transmutes), record that in the report and skip; only act if the failure is in the new scope code.

- [ ] **Step 5: Commit** — `rust(parser): whole-pass AllocationScope in pre_parse_buffer (cpp:7523) + memory oracle`

---

### Task 5: Oracle B over the Flow/TS corpora

**Files:**
- Modify: `tools/preparse-dump/preparse-dump.cpp` (flag loop, lines 38-51; Context setup, line 61)
- Modify: `rust/crates/parser/src/bin/preparse_dump.rs` (arg handling; Context setup)
- Modify: `rust/crates/parser/tests/preparse_differential.rs` (`run_differential` gains a flags param; two new tests)

**Interfaces:**
- Consumes: the existing output contract (unchanged); `Context::setParseFlow(ParseFlowSetting)` / `setParseTS(bool)` (C++ Context.h:442,473); Rust `Context::set_parse_flow/set_parse_flow_ambiguous/set_parse_ts`.
- Produces: `--parse-flow` and `--parse-ts` on BOTH binaries (identical spelling); `preparse_differential_flow_corpus` and `preparse_differential_ts_corpus` tests.

- [ ] **Step 1: Extend the differential test first (the failing test).** In `preparse_differential.rs`, change `run_differential(corpus: &str)` to `run_differential(corpus: &str, extra: &[&str])`, pass `.args(extra)` to BOTH `Command`s (before the file arg), update the two existing calls with `&[]`, and add:

```rust
#[test]
fn preparse_differential_flow_corpus() {
    // Flow ambiguous grammar ON (hermesc -parse-flow defaults to ALL);
    // both binaries get the identical flag.
    run_differential("tests/parser_corpus_flow", &["--parse-flow"]);
}

#[test]
fn preparse_differential_ts_corpus() {
    run_differential("tests/parser_corpus_ts", &["--parse-ts"]);
}
```

- [ ] **Step 2: Run to verify failure** — `REQUIRE_DIFFERENTIAL=1 cargo test ... --test preparse_differential preparse_differential_flow` → FAIL (both binaries reject/ignore the unknown flag; outputs diverge or error).
- [ ] **Step 3: C++ tool.** In the arg loop (preparse-dump.cpp:40-47) add, before the positional-arg branch:

```cpp
    if (std::strcmp(arg, "--parse-flow") == 0) {
      parseFlow = true;
      continue;
    }
    if (std::strcmp(arg, "--parse-ts") == 0) {
      parseTS = true;
      continue;
    }
```

(declare `bool parseFlow = false, parseTS = false;` above the loop; `#include <cstring>` is already present). After the Context is created (line 61):

```cpp
  if (parseFlow)
    ctx->setParseFlow(ParseFlowSetting::ALL);
  if (parseTS)
    ctx->setParseTS(true);
```

Update the usage string to `" [--parse-flow] [--parse-ts] <file|->"`. Rebuild: `cmake --build cmake-build-asan --target preparse-dump`.

- [ ] **Step 4: Rust bin.** Replace the single-positional arg handling in `preparse_dump.rs` with a small loop (mirroring the C++):

```rust
    let mut parse_flow = false;
    let mut parse_ts = false;
    let mut file_path: Option<String> = None;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--parse-flow" => parse_flow = true,
            "--parse-ts" => parse_ts = true,
            _ => {
                if file_path.is_some() {
                    eprintln!("{prog}: too many arguments");
                    std::process::exit(1);
                }
                file_path = Some(arg);
            }
        }
    }
    let file_path = file_path.unwrap_or_else(|| "-".to_string());
```

and after `let mut ctx = Context::new();`:

```rust
    // hermesc -parse-flow defaults to ParseFlowSetting::ALL → ambiguous on
    // (same plumbing as ast_dump.rs).
    ctx.set_parse_flow(parse_flow);
    ctx.set_parse_flow_ambiguous(parse_flow);
    ctx.set_parse_ts(parse_ts);
```

- [ ] **Step 5: Run** — `REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test preparse_differential` → all four tests pass (42 Flow + 20 TS files now byte-checked). **If flow/ts files mismatch, that is a REAL finding** (e.g. the SavePoint-speculation × arrow-store interaction) — do NOT weaken the test or special-case files; report BLOCKED with the differing file + both outputs.
- [ ] **Step 6: Commit** — `rust(parser): Oracle B over Flow/TS corpora (preparse-dump dialect flags)`

---

### Task 6: Oracle A located dumps

**Files:**
- Modify: `rust/crates/parser/tests/lazy_reparse.rs` (`dump_node` ~line 313; `check_file` ~line 372)

**Interfaces:**
- Consumes: `ast::dump::dump_estree_json_with_sm(out, root, pretty, mode, sm, loc_mode, raw_prop, atoms)` (dump.rs:322-331), `LocationDumpMode::LocAndRange`, `ESTreeRawProp::Exclude`.
- Produces: eager-vs-reparsed body comparisons now include `loc` + `range` — covering the `seek`/`set_prev_token_end_loc` source-range machinery (cpp:758-763).

- [ ] **Step 1: Make the change (the assertion set is pre-existing; this strengthens it).** The live parsers hold `&mut sm`, so the dumps use a SECOND `SourceErrorManager` loaded with the same buffer — same content ⇒ same id ⇒ identical coordinates (asserted):

In `check_file`, right after `let id = sm.add_buffer_bytes(label, src);` add:

```rust
    // A second SourceErrorManager for location dumps: the live parsers hold
    // `&mut sm`, so dumps resolve line/col through an identical read-only
    // copy. Same content + first buffer => same SourceId (asserted).
    let mut sm_dump = SourceErrorManager::new();
    let id_dump = sm_dump.add_buffer_bytes(label, src);
    assert_eq!(id, id_dump, "buffer id mismatch between managers");
    let sm_dump = sm_dump; // no longer mutated
```

Change `dump_node` to:

```rust
fn dump_node<'a>(
    node: &'a Node<'a>,
    atoms: &atom_table::AtomTable,
    sm: &support::manager::SourceErrorManager,
) -> String {
    let mut out = String::new();
    ast::dump::dump_estree_json_with_sm(
        &mut out,
        node,
        false,
        ESTreeDumpMode::HideEmpty,
        sm,
        ast::dump::LocationDumpMode::LocAndRange,
        ast::dump::ESTreeRawProp::Exclude,
        atoms,
    );
    out
}
```

and thread `&sm_dump` through the two call sites (`collect_eager_body_strings`'s `dump_node` — add the `sm` parameter down that helper chain — and the BFS leaf comparison at ~line 497). Update imports.

- [ ] **Step 2: Run** — `cargo test --manifest-path rust/Cargo.toml -p parser --test lazy_reparse -- --nocapture` → both tests pass with the same comparison counts as before (30 + 48). **A location mismatch is a REAL finding** in the LazyParse/demand path (ranges, `set_prev_token_end_loc`) — do NOT fall back to unlocated dumps; report BLOCKED with the diff.
- [ ] **Step 3: Run the whole crate + zero-warnings build.**
- [ ] **Step 4: Commit** — `rust(parser): Oracle A compares located dumps (covers lazy source ranges)`

---

### Task 7: Docs + final gates

**Files:**
- Modify: `doc/superpowers/RustPortRoadmap.md` — in the Parser row and the Pre/Lazy DONE block: replace the "there is no `AllocationScope` (no bump allocator — PreParse just discards the AST)" deviation with "the `AllocationScope` discipline is ported (arena truncate scope; PreParse peak ≈ skeleton + open function nest — see `specs/2026-07-15-preparse-scoped-reclamation-design.md`)", leaving the side-table-threaded-on-parser deviation as the only one; append a short "PreParse scoped reclamation DONE (2026-07-15)" note to the Pre/Lazy block naming the two sites (cpp:516-560, cpp:7523), the memory oracle, and the two Oracle hardening items.
- Modify: `doc/superpowers/SESSION-HANDOFF.md` — same deviation-text correction in the status block (it currently says the Parser is complete with the AllocationScope gap implicit; make the reclamation explicit and keep "next component: Sema").

- [ ] **Step 1: Make the doc edits above.**
- [ ] **Step 2: Verify no stale text remains:**

```bash
grep -rn "not replicated\|no bump allocator\|discards the AST" doc/superpowers/RustPortRoadmap.md doc/superpowers/SESSION-HANDOFF.md rust/crates/parser/src/js/
```

Expected: no hits describing the AllocationScope as missing (hits describing the *history* in the two new spec/plan files are fine).

- [ ] **Step 3: Run ALL gates one final time** (the full Validation commands block from Global Constraints, plus `cargo clippy --manifest-path rust/Cargo.toml -p parser -p ast -p support`).
- [ ] **Step 4: Commit** — `doc(rust): PreParse scoped reclamation complete — AllocationScope deviation retired`

---

## Self-review

- **Spec coverage:** §2 mechanism+soundness → Tasks 1-2; §3 API → Task 2; §4 Site 1 → Task 3, Site 2 → Task 4, not-scoped list → Task 3 notes (arrows untouched); §5 memory oracle → Tasks 3 (mid-pass shape) + 4 (post-pass shape), semantic gates → every task's Step 4, miri → Task 4 optional; §6.1 → Task 5; §6.2 → Task 6; §6.3 docs+comments → Tasks 3/4 (code comments) + 7 (roadmap/handoff). No gaps.
- **Placeholders:** the two "idiom" references (Task 2 node construction per `alloc_and_deep_match`/`dump_golden.rs`; Task 4's count-the-literal note) point at exact existing code to copy, with the surrounding test code fully written — acceptable; no TBD/TODO remain.
- **Type consistency:** `alloc_scope`/`AllocationScope`/`truncate`/`iter_from`/`num_list_elements`/`pre_parse_buffer` names and signatures match across Tasks 1→4; `run_differential(corpus, extra)` matches Task 5's calls; `dump_node(node, atoms, sm)` matches Task 6's call sites.
