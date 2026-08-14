# JS Parser — the Pre/Lazy passes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Faithfully port the JS parser's three-pass machinery (`FullParse` / `PreParse` / `LazyParse`) and the on-demand `parse_lazy_function` entry point, completing the JS Parser component.

**Architecture:** Mirror `lib/Parser/JSParserImpl.{h,cpp}` one-for-one. A `ParserPass` enum + `pass` field selects behavior, localized to `parse_function_body` plus the two entry points. `PreParse` does a full eager walk that populates a per-function side-table and discards the AST; `LazyParse` re-parses but seeks past function bodies ≥ a byte threshold, emitting stub `BlockStatement`s; `parse_lazy_function` re-parses one deferred body on demand. Two complementary oracles gate the phase: a Rust-only reparse-equivalence test, and a C++ `preparse-dump` tool + byte-for-byte differential of the side-table.

**Tech Stack:** Rust (workspace `rust/`, toolchain 1.96.0), the `ast`/`parser`/`support` crates; C++ oracle tool via `add_hermes_tool`; the existing `parser_differential`/`json_differential` harness pattern.

## Global Constraints

- **Branch `rust`; commit directly; never open a PR or merge.** Commit messages end with `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.
- **Never `cd`** out of the project root; use `--manifest-path rust/Cargo.toml` and absolute/relative paths.
- **C++ default arguments are spec** — read the header (`parse_function_body` defaults, `lookahead1` `RequireNoNewLine=true`, grammar-context per call site).
- **C++ `template`s stay Rust generics; C++ RAII guards become Drop-guards** (or explicit save/restore wrappers) that restore on every `?` early-return.
- **No AST nodes added** — `generated_idempotent` must stay green. The `BlockStatement` lazy decorations already exist (`rust/crates/ast/src/node.rs:913-918`).
- **Zero `cargo build` warnings; no new clippy lints** (scoped `#[allow]` + comment only for faithful C-idioms).
- **Faithful-port deviation (documented):** the C++ `PreParsedData` lives on `Context`; the Rust port threads the table on the parser (PreParse populates it via `&mut self`; it is moved into the LazyParse parser) because the `GCLock` borrows `Context` immutably during parsing. The threshold stays on `Context` (read-only during parse). Observable behavior (table contents + skip decisions) is identical.
- **Faithful-port deviation (documented):** the C++ PreParse path uses a nested `AllocationScope` to reclaim the discarded body AST (a bump-allocator memory optimization). Rust has no bump allocator (the documented `getAllocator` gap) — PreParse simply parses and drops the AST; the GC arena reclaims it. The kept output is the side-table only.
- **C++ source of truth:** `lib/Parser/JSParserImpl.cpp` (line refs inline), `JSParserImpl.h`, `include/hermes/Parser/{JSParser,PreParser}.h`, `include/hermes/AST/Context.h`.
- **Spec:** `doc/superpowers/specs/2026-06-28-pre-lazy-passes-design.md`.
- **Workflow per task:** TDD; each task ends with a commit. Each task is independently reviewed (spec-compliance + structural-fidelity + quality) before the next.

### Validation commands (used throughout)

```bash
# Workspace build/test (zero warnings expected):
cargo build  --manifest-path rust/Cargo.toml
cargo test   --manifest-path rust/Cargo.toml -p parser
cargo test   --manifest-path rust/Cargo.toml -p ast
cargo clippy --manifest-path rust/Cargo.toml -p parser

# The full pre-existing AST differential MUST stay byte-for-byte green
# (FullParse unchanged). Build the oracle once:
cmake --build cmake-build-asan --target hermesc ast-dump 2>/dev/null || \
  cmake --build cmake-build-asan --target hermesc
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser \
  --test parser_differential -- --nocapture

# AST node-set guard:
REQUIRE_GEN=1 cargo test --manifest-path rust/Cargo.toml -p ast --test generated_idempotent
```

---

## File structure

| File | Responsibility | Task |
|---|---|---|
| `rust/crates/parser/src/js/pre_lazy.rs` (new) | `ParserPass` enum, `PreParsedFunctionInfo`/`PreParsedBufferInfo`, `SaveFunctionState` guard, `pre_parse_buffer`, `parse_lazy_function` | L0.1, L0.3, L1.1, L2.2 |
| `rust/crates/parser/src/js/mod.rs` | parser fields (`pass`, `pre_parsed`, arrow-bookkeeping, `seen_directives`) + `new_with_pass` | L0.1, L0.2, L0.3 |
| `rust/crates/ast/src/context.rs` | `preemptive_function_compilation_threshold` field + getter/setter | L0.2 |
| `rust/crates/parser/src/js/functions.rs` | `SaveFunctionState` wiring; PreParse store; LazyParse skip-and-stub | L0.3, L1.1, L2.1 |
| `rust/crates/parser/src/js/expressions.rs` | arrow PreParse store; arrow `SaveFunctionState`; `arguments` site | L0.3, L1.1 |
| `rust/crates/parser/src/js/statements.rs` | `seen_directives.push` in `process_directive` | L0.3 |
| `tools/preparse-dump/{preparse-dump.cpp,CMakeLists.txt}` (new) | C++ oracle: print the PreParse side-table | L1.2 |
| `tools/CMakeLists.txt` | register `add_subdirectory(preparse-dump)` | L1.2 |
| `rust/crates/parser/src/bin/preparse_dump.rs` (new) | Rust mirror of the side-table dump | L1.2 |
| `rust/crates/parser/tests/preparse_differential.rs` (new) | Oracle B byte-for-byte differential | L1.2 |
| `rust/crates/parser/tests/parser_corpus_lazy/` (new) | corpus for both oracles | L1.2 |
| `rust/crates/parser/tests/lazy_reparse.rs` (new) | Oracle A reparse-equivalence | L2.3 |

---

## Task L0.1: `ParserPass` enum + `pass` field (no behavior change)

**Files:**
- Create: `rust/crates/parser/src/js/pre_lazy.rs`
- Modify: `rust/crates/parser/src/js/mod.rs` (add `mod pre_lazy;`, the `pass` field, `new_with_pass`)

**Interfaces:**
- Produces: `pub enum ParserPass { PreParse, LazyParse, FullParse }` (order matches `JSParser.h:26-36`); `JSParserImpl::new_with_pass(gc, lexer, pass) -> Self`; `self.pass: ParserPass` field (private to `crate::js`).
- Consumes: nothing.

- [ ] **Step 1: Write the failing test** (in `pre_lazy.rs` `#[cfg(test)] mod tests`)

```rust
// A parser built with `new` defaults to FullParse; new_with_pass honors the arg.
#[test]
fn parser_pass_defaults_and_override() {
    use ast::context::Context;
    use support::manager::SourceErrorManager;
    use crate::lexer::{GrammarContext, JSLexer};
    use crate::js::{JSParserImpl, ParserPass};

    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer_bytes("t", b"1;");
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let atoms = &gc.ctx().atom_table;
    let lexer = JSLexer::new(id, &mut sm, atoms, GrammarContext::AllowRegExp);
    let p = JSParserImpl::new_with_pass(&gc, lexer, ParserPass::PreParse);
    assert_eq!(p.pass, ParserPass::PreParse);
}
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test --manifest-path rust/Cargo.toml -p parser parser_pass_defaults_and_override`
Expected: FAIL — `ParserPass`/`new_with_pass`/`pass` not found.

- [ ] **Step 3: Implement**

In `pre_lazy.rs`:
```rust
//! The Pre/Lazy parser passes. Port of the `ParserPass` machinery in
//! `lib/Parser/JSParserImpl.{h,cpp}` and `include/hermes/Parser/JSParser.h`.

/// The parser mode. Port of `enum ParserPass` (JSParser.h:26-36). Same order:
/// PreParse, LazyParse, FullParse.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ParserPass {
    /// Parse and index the file's functions without keeping an AST.
    PreParse,
    /// Re-parse, skipping function bodies indexed by a prior PreParse.
    LazyParse,
    /// Completely parse the full file (the default, eager mode).
    FullParse,
}
```

In `mod.rs`: add `mod pre_lazy;` and `pub use pre_lazy::ParserPass;` (mirror the existing `pub use`/`pub(super)` style). Add the field to the struct (`JSParserImpl`):
```rust
    /// The current parser mode. Port of `pass_{FullParse}` (JSParserImpl.h:179).
    pub(super) pass: ParserPass,
```
In `new`, initialize `pass: ParserPass::FullParse`. Add:
```rust
    /// Construct the parser in a specific pass. Port of the C++
    /// `JSParserImpl(Context&, bufferId, ParserPass)` ctor (JSParserImpl.cpp:39).
    pub fn new_with_pass(
        gc: &'gc GCLock<'ast, 'ctx>,
        lexer: JSLexer<'a>,
        pass: ParserPass,
    ) -> Self {
        let mut p = Self::new(gc, lexer);
        p.pass = pass;
        p
    }
```

- [ ] **Step 4: Run the test; expect PASS.** Also `cargo build --manifest-path rust/Cargo.toml` (zero warnings).
- [ ] **Step 5: Commit** — `rust(parser): L0.1 ParserPass enum + pass field (no behavior change)`

---

## Task L0.2: side-table types + Context threshold knob

**Files:**
- Modify: `rust/crates/parser/src/js/pre_lazy.rs` (the two structs + parser `pre_parsed` field accessors)
- Modify: `rust/crates/parser/src/js/mod.rs` (add `pre_parsed` field)
- Modify: `rust/crates/ast/src/context.rs` (threshold field + getter/setter)

**Interfaces:**
- Produces:
  - `pub struct PreParsedFunctionInfo { pub end: SMLoc, pub strict_mode: bool, pub directives: Vec<Vec<u8>>, pub contains_arrow_functions: bool, pub may_contain_arrow_functions_using_arguments: bool }` (port of `PreParser.h:38-58`).
  - `pub struct PreParsedBufferInfo { pub function_info: std::collections::HashMap<u32, PreParsedFunctionInfo> }` (key = function start **offset** within the buffer; port of `PreParser.h:60-63`).
  - `self.pre_parsed: PreParsedBufferInfo` field on the parser; `take_pre_parsed(&mut self) -> PreParsedBufferInfo` and `set_pre_parsed(&mut self, t: PreParsedBufferInfo)`.
  - `Context::preemptive_function_compilation_threshold(&self) -> u32`, `Context::set_preemptive_function_compilation_threshold(&mut self, u32)`, default `0` (port of `Context.h:236,516-521`).
- Consumes: L0.1.

- [ ] **Step 1: Write the failing test** (`pre_lazy.rs` tests + a `context.rs` test)

```rust
// pre_lazy.rs: the table round-trips through take/set; threshold defaults to 0.
#[test]
fn pre_parsed_table_and_threshold() {
    use ast::context::Context;
    let mut ctx = Context::new();
    assert_eq!(ctx.preemptive_function_compilation_threshold(), 0);
    ctx.set_preemptive_function_compilation_threshold(64);
    assert_eq!(ctx.preemptive_function_compilation_threshold(), 64);
}
```

- [ ] **Step 2: Run it; expect FAIL** (threshold methods missing).
- [ ] **Step 3: Implement**

`context.rs`: add field `preemptive_function_compilation_threshold: u32` (init `0` in `new`) + the getter/setter shown above, with doc comments citing `Context.h:236,516-521`.

`pre_lazy.rs`: add the two structs (doc comments citing `PreParser.h`). Note in a comment that `directives` holds **owned** bytes because the C++ stores `SmallString` copies (atoms are reclaimed between passes — `PreParser.h:46-48`).

`mod.rs`: add field `pub(super) pre_parsed: PreParsedBufferInfo` (init `PreParsedBufferInfo { function_info: HashMap::new() }` in `new`); add `take_pre_parsed`/`set_pre_parsed` methods.

- [ ] **Step 4: Run tests; expect PASS.** `cargo build` (zero warnings).
- [ ] **Step 5: Commit** — `rust(parser): L0.2 PreParsed side-table types + Context threshold`

---

## Task L0.3: `SaveFunctionState` + arrow-bookkeeping + `seen_directives` wiring (FullParse behavior unchanged)

This task adds the bookkeeping state and the RAII guard, and replaces the existing ad-hoc strict save/restore at every function-scope entry. It must NOT change `FullParse` output — the full pre-existing differential is the gate.

**Files:**
- Modify: `rust/crates/parser/src/js/mod.rs` (fields: `is_arrow_function`, `contains_arrow_functions`, `may_contain_arrow_functions_using_arguments`, `seen_directives`)
- Modify: `rust/crates/parser/src/js/pre_lazy.rs` (the `SaveFunctionState` guard + `copy_seen_directives`)
- Modify: `rust/crates/parser/src/js/functions.rs`, `expressions.rs`, `classes.rs` (wire the guard at every function-scope entry, replacing ad-hoc strict save/restore)
- Modify: `rust/crates/parser/src/js/statements.rs` (`process_directive` pushes to `seen_directives`)
- Modify: `rust/crates/parser/src/js/expressions.rs` (the `arguments`-identifier site)

**Interfaces:**
- Produces: `SaveFunctionState` Drop-guard (constructed `self.save_function_state(is_arrow: bool) -> SaveFunctionState`); the three bookkeeping fields (as `Rc<Cell<bool>>` to match the existing `ParamFlagGuard` pattern so the guard owns handles without borrowing `self`); `self.seen_directives: Vec<Vec<u8>>`; `copy_seen_directives(&self) -> Vec<Vec<u8>>`.
- Consumes: L0.1, L0.2.

**Reference C++:** `SaveFunctionState` ctor/dtor `JSParserImpl.h:1699-1740`; fields `JSParserImpl.h:225,236,246`; `seenDirectives_.push_back` `JSParserImpl.cpp:341`; `copySeenDirectives` `JSParserImpl.cpp:731`; the `arguments` site `JSParserImpl.cpp:2508-2511`; guard construction sites `JSParserImpl.cpp:510` (function helper), `5849` (arrow, `/*arrow*/true`), and the class-body strict force (`SaveFunctionState saveFunctionState{this}; setStrictMode(true);`).

- [ ] **Step 1: Write the failing test** (`pre_lazy.rs` tests)

```rust
// SaveFunctionState restores strict-mode + seen_directives size + the three
// arrow-bookkeeping flags on drop (mirrors the C++ dtor). Drives the guard
// directly on a parser.
#[test]
fn save_function_state_restores_on_drop() {
    use ast::context::Context;
    use support::manager::SourceErrorManager;
    use crate::lexer::{GrammarContext, JSLexer};
    use crate::js::{JSParserImpl, ParserPass};

    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer_bytes("t", b"0");
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let atoms = &gc.ctx().atom_table;
    let lexer = JSLexer::new(id, &mut sm, atoms, GrammarContext::AllowRegExp);
    let mut p = JSParserImpl::new_with_pass(&gc, lexer, ParserPass::PreParse);

    p.lexer.set_strict_mode(false);
    p.contains_arrow_functions.set(false);
    {
        // Enter a NON-arrow function: flags reset to false, restored on drop.
        let _g = p.save_function_state(false);
        p.lexer.set_strict_mode(true);
        p.contains_arrow_functions.set(true);
    }
    assert!(!p.lexer.strict_mode(), "strict restored");
    assert!(!p.contains_arrow_functions.get(), "contains_arrow restored");
}
```

- [ ] **Step 2: Run it; expect FAIL** (`save_function_state`/fields missing).
- [ ] **Step 3: Implement the guard + fields**

`mod.rs` fields (mirror `param_yield`'s `Rc<Cell<bool>>` rationale + doc):
```rust
    /// Whether the current function is an arrow. Only set/restored by
    /// `SaveFunctionState`. Port of `isArrowFunction_` (JSParserImpl.h:225).
    pub(super) is_arrow_function: Rc<Cell<bool>>,
    /// Whether the nearest enclosing non-arrow function contains an arrow.
    /// Port of `containsArrowFunctions_` (JSParserImpl.h:236).
    pub(super) contains_arrow_functions: Rc<Cell<bool>>,
    /// Whether that function may contain an arrow using `arguments`.
    /// Port of `mayContainArrowFunctionsUsingArguments_` (JSParserImpl.h:246).
    pub(super) may_contain_arrow_functions_using_arguments: Rc<Cell<bool>>,
    /// Directives seen in the current function scope (for lazy directive
    /// recovery). Port of `seenDirectives_` (JSParserImpl.h:220).
    pub(super) seen_directives: Vec<Vec<u8>>,
```
Init all in `new` (the `Rc<Cell<bool>>` to `false`, the Vec empty).

`pre_lazy.rs`: the guard, holding `Rc` clones of the three flag cells + the lexer's strict state and the seen-directives length. Because strict-mode and `seen_directives` live behind `&mut self` (the lexer / the Vec), the guard cannot own handles to them; instead capture old strict + old length and restore them in the method epilogue is NOT `?`-safe. Faithful approach: the guard owns the three `Rc<Cell<bool>>` (like `ParamFlagGuard`) AND owns an `Rc<Cell<bool>>` mirror of strict mode is wrong (strict lives on the lexer). Resolve by giving the guard the three flag cells plus saved `old_strict`/`old_seen_len`, and have the guard restore the flags on Drop, while strict-mode + `seen_directives` truncation are restored by the guard via a small callback is not possible without `self`.

  **Decision (documented):** model `SaveFunctionState` as a Drop-guard for the three arrow flags (which are `Rc<Cell<bool>>`, fully `?`-safe), and keep strict-mode + `seen_directives` restoration as the existing explicit save/restore wrappers the eager port already performs at these sites (those wrappers already survive `?` in the current code — see `expressions.rs:1057` and `classes.rs:223-224`). This preserves the C++ dtor's *observable* effect: on entry, set `is_arrow_function`, and (non-arrow) reset `contains_arrow_functions`/`may_contain…` to false; on exit, restore the three flags (non-arrow) or propagate (arrow), and restore strict + truncate `seen_directives`. Cite `JSParserImpl.h:1719-1738` for the enter/exit logic.

```rust
pub(super) struct SaveFunctionState {
    is_arrow: Rc<Cell<bool>>,
    contains: Rc<Cell<bool>>,
    may_contain: Rc<Cell<bool>>,
    old_is_arrow: bool,
    old_contains: bool,
    old_may_contain: bool,
}
impl Drop for SaveFunctionState {
    fn drop(&mut self) {
        // C++ dtor JSParserImpl.h:1728-1738.
        if !self.is_arrow.get() {
            self.contains.set(self.old_contains);
            self.may_contain.set(self.old_may_contain);
        }
        self.is_arrow.set(self.old_is_arrow);
    }
}
```
And on the parser:
```rust
    pub(super) fn save_function_state(&self, is_arrow: bool) -> SaveFunctionState {
        let g = SaveFunctionState {
            is_arrow: Rc::clone(&self.is_arrow_function),
            contains: Rc::clone(&self.contains_arrow_functions),
            may_contain: Rc::clone(&self.may_contain_arrow_functions_using_arguments),
            old_is_arrow: self.is_arrow_function.get(),
            old_contains: self.contains_arrow_functions.get(),
            old_may_contain: self.may_contain_arrow_functions_using_arguments.get(),
        };
        // C++ ctor JSParserImpl.h:1719-1726.
        self.is_arrow_function.set(is_arrow);
        if is_arrow {
            self.contains_arrow_functions.set(true);
        } else {
            self.contains_arrow_functions.set(false);
            self.may_contain_arrow_functions_using_arguments.set(false);
        }
        g
    }
    /// Port of `copySeenDirectives` (JSParserImpl.cpp:731-739).
    pub(super) fn copy_seen_directives(&self) -> Vec<Vec<u8>> {
        self.seen_directives.clone()
    }
```

- [ ] **Step 4: Wire `save_function_state` at every function-scope entry**, alongside the existing strict + `seen_directives`-length save/restore. Sites: `parse_function_helper` (`functions.rs`, mirrors `cpp:510`), arrow (`expressions.rs`, `is_arrow=true`, mirrors `cpp:5849`), object methods/getters/setters (`expressions.rs`/`classes.rs`), class body (force-strict path, `classes.rs:223`). In `statements.rs::process_directive`, add `self.seen_directives.push(directive.as_bytes().to_vec())` (or the byte slice from the atom) **before** the strict-mode set, mirroring `cpp:341`. In `expressions.rs`, at the `arguments` primary-expression site (mirror `cpp:2508-2511`): `if self.is_arrow_function.get() && self.check(arguments_ident) { self.may_contain_arrow_functions_using_arguments.set(true); }`.

  Each wrapped scope must save the `seen_directives` length on entry and truncate back to it on exit (mirror the C++ `getSeenDirectives().resize(oldSeenDirectiveSize_)` in the dtor) using the same `?`-safe pattern already used for strict mode at these sites.

- [ ] **Step 5: Run the guard test; expect PASS.**
- [ ] **Step 6: Verify FullParse is unchanged — the gate.**

```bash
cargo build --manifest-path rust/Cargo.toml          # zero warnings
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser \
  --test parser_differential -- --nocapture
```
Expected: every corpus (`p0`, `flow`, `flow_component`, `flow_records`, `flow_match`, `ts`, `jsx`, `jsx_flow`) matches — identical to before this task.

- [ ] **Step 7: Commit** — `rust(parser): L0.3 SaveFunctionState + arrow bookkeeping + seen_directives (FullParse unchanged)`

---

## Task L1.1: PreParse side-table population + `pre_parse_buffer`

**Files:**
- Modify: `rust/crates/parser/src/js/functions.rs` (the `parse_function_body` PreParse store, `cpp:803-810`)
- Modify: `rust/crates/parser/src/js/expressions.rs` (the arrow PreParse store, `cpp:5896-5908`)
- Modify: `rust/crates/parser/src/js/pre_lazy.rs` (`pre_parse_buffer`)

**Interfaces:**
- Produces: after a `PreParse` `parse()`, `self.pre_parsed.function_info` is populated; `JSParserImpl::pre_parse_buffer(gc, lexer, strict) -> Option<JSParserImpl>` convenience (sets strict, `pass=PreParse`, runs `parse()`, returns the parser carrying the table + `use_static_builtin`), mirroring `JSParserImpl.cpp:7534-7546`.
- Consumes: L0.2, L0.3.

**Behavior:** In `parse_function_body`, after the block parses successfully, if `self.pass == PreParse`, insert (overwrite — C++ uses `operator[]`, `cpp:804`) keyed by **body start offset** the `PreParsedFunctionInfo { end: body.end, strict_mode: self.lexer.strict_mode(), directives: self.copy_seen_directives(), contains_arrow_functions: self.contains_arrow_functions.get(), may_contain_arrow_functions_using_arguments: self.may_contain_arrow_functions_using_arguments.get() }`. In the arrow path (`parse_assignment_expression`/arrow helper), after building the arrow node, if `self.pass == PreParse`, insert (insert-if-absent — C++ `try_emplace` + `assert(inserted)`, `cpp:5897-5907`) keyed by the **arrow start offset**, `end = body.end`.

- [ ] **Step 1: Write the failing test** (`pre_lazy.rs` tests)

```rust
// PreParse over a file with two functions records both, with correct strict
// flag and directives.
#[test]
fn preparse_records_functions() {
    use ast::context::Context;
    use support::manager::SourceErrorManager;
    use crate::lexer::{GrammarContext, JSLexer};
    use crate::js::{JSParserImpl, ParserPass};

    let src = b"function a(){ 'use strict'; return 1; }\nvar b = () => 2;\n";
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer_bytes("t", src);
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let atoms = &gc.ctx().atom_table;
    let lexer = JSLexer::new(id, &mut sm, atoms, GrammarContext::AllowRegExp);
    let mut p = JSParserImpl::new_with_pass(&gc, lexer, ParserPass::PreParse);
    assert!(p.parse().is_some());
    let t = p.take_pre_parsed();
    // function a's body { ... } and the arrow are both recorded.
    assert_eq!(t.function_info.len(), 2);
    // exactly one recorded function is strict (function a, due to 'use strict').
    let strict_count = t.function_info.values().filter(|i| i.strict_mode).count();
    assert_eq!(strict_count, 1);
    let with_dir = t.function_info.values().filter(|i| !i.directives.is_empty()).count();
    assert_eq!(with_dir, 1);
}
```

- [ ] **Step 2: Run it; expect FAIL** (table empty — stores not implemented).
- [ ] **Step 3: Implement** the two store sites (guarded by `self.pass == ParserPass::PreParse`) and `pre_parse_buffer`. Key by start **offset** (`loc.offset`). Cite `cpp:803-810` and `cpp:5896-5908`. Document the deviation: no `AllocationScope` (Global Constraints); Rust just discards the AST.
- [ ] **Step 4: Run the test; expect PASS.** `cargo build` (zero warnings).
- [ ] **Step 5: Commit** — `rust(parser): L1.1 PreParse side-table population + pre_parse_buffer`

---

## Task L1.2: Oracle B — `preparse-dump` C++ tool + Rust bin + differential + corpus

**Files:**
- Create: `tools/preparse-dump/preparse-dump.cpp`, `tools/preparse-dump/CMakeLists.txt`
- Modify: `tools/CMakeLists.txt` (`add_subdirectory(preparse-dump)`)
- Create: `rust/crates/parser/src/bin/preparse_dump.rs`
- Create: `rust/crates/parser/tests/preparse_differential.rs`
- Create: `rust/crates/parser/tests/parser_corpus_lazy/*.js`

**Interfaces:**
- Produces: two binaries with an identical stdout contract; a differential test `preparse_corpus_differential`.
- Consumes: L1.1.

**Output contract (both binaries).** Read source (file arg or `-`/stdin). Run a PreParse. Collect `function_info` entries, **sort by start offset ascending** (the map is unordered). Print one line per entry, then nothing else (no trailing blank line):
```
<start> <end> <strict:0|1> <containsArrow:0|1> <mayContainArrowArgs:0|1> <dirCount> <dir0> <dir1> ...
```
where `<start>`/`<end>` are byte **offsets** within the buffer (0-based), directives are the raw directive strings separated by single spaces (UTF-8/WTF-8 bytes written verbatim; directives in this corpus are ASCII). On parse error: print exactly `ERROR <count>\n` and stop. Header line first: `PREPARSE <n>\n` where `<n>` is the entry count (gives a stable, diffable anchor and guards the empty case).

**C++ tool** (`preparse-dump.cpp`) — mirror `tools/json-parse-dump/json-parse-dump.cpp` structure (stdin via `MemoryBuffer::getFileOrSTDIN`, no-op diag handler). Key body:
```cpp
#include "hermes/AST/Context.h"
#include "hermes/Parser/JSParser.h"
#include "hermes/Parser/PreParser.h"
#include "hermes/Support/SourceErrorManager.h"
// ... read buffer into Context's source manager:
auto ctx = std::make_shared<Context>();
SourceErrorManager &sm = ctx->getSourceErrorManager();
sm.setDiagHandler([](const llvh::SMDiagnostic&, void*){}, nullptr);
uint32_t bufId = sm.addNewSourceBuffer(std::move(fileBuf)); // 1-based id
auto parser = JSParser::preParseBuffer(*ctx, bufId, /*strict*/ false);
if (!parser) { llvh::outs() << "ERROR " << sm.getErrorCount() << "\n"; return 0; }
const char *bufStart = sm.getSourceBuffer(bufId)->getBufferStart();
PreParsedBufferInfo *info = ctx->getPreParsedBufferInfo(bufId);
// Collect + sort by (entry.first.getPointer() - bufStart):
std::vector<std::pair<size_t, PreParsedFunctionInfo>> v;
for (auto &kv : info->functionInfo)
  v.push_back({(size_t)(kv.first.getPointer() - bufStart), kv.second});
std::sort(v.begin(), v.end(), [](auto&a, auto&b){ return a.first < b.first; });
llvh::outs() << "PREPARSE " << v.size() << "\n";
for (auto &e : v) {
  size_t endOff = (size_t)(e.second.end.getPointer() - bufStart);
  llvh::outs() << e.first << " " << endOff << " " << (e.second.strictMode?1:0)
    << " " << (e.second.containsArrowFunctions?1:0) << " "
    << (e.second.mayContainArrowFunctionsUsingArguments?1:0) << " "
    << e.second.directives.size();
  for (auto &d : e.second.directives) llvh::outs() << " " << d;
  llvh::outs() << "\n";
}
```
(Adjust offset basis so it matches the Rust `SMLoc.offset` convention — verify both report 0-based offsets from the buffer start. If `addNewSourceBuffer` ids/locations differ, normalize by subtracting the buffer start pointer, as above.)

`tools/preparse-dump/CMakeLists.txt` — copy `json-parse-dump`'s, swap the name:
```cmake
add_hermes_tool(preparse-dump
  preparse-dump.cpp
  LINK_OBJLIBS hermesParser hermesSupport LLVHSupport
  )
```
Register in `tools/CMakeLists.txt`.

**Rust bin** (`preparse_dump.rs`) — mirror `ast_dump.rs`'s IO + `json_parse_dump.rs`; build `Context`, add buffer, run `pre_parse_buffer`, print the identical format. Offsets come from `SMLoc.offset`.

**Differential test** (`preparse_differential.rs`) — mirror `json_differential.rs`: resolve the C++ binary at `../../../cmake-build-asan/bin/preparse-dump`, the Rust at `CARGO_BIN_EXE_preparse-dump`; run every `.js` in `tests/parser_corpus_lazy/` through both; assert byte-equal stdout; skip unless present, hard-fail under `REQUIRE_DIFFERENTIAL=1`. ALSO run it over the existing `tests/parser_corpus` for breadth (PreParse records every function regardless of size).

**Corpus `parser_corpus_lazy/`** — engineered files covering: plain function decl + expr; nested functions; arrow (block body) + arrow (concise body); getter/setter; class method + static method; `async`/generator functions; a `"use strict"` directive inside a function; a custom directive (`"use frobnicate";`); an arrow that references `arguments`; deeply nested arrows inside a non-arrow. Keep each small and ASCII.

- [ ] **Step 1: Write the corpus files** (`parser_corpus_lazy/*.js`), then the differential test (it fails first because the binaries don't exist yet — that IS the failing test).
- [ ] **Step 2: Build the C++ tool**

```bash
cmake --build cmake-build-asan --target preparse-dump
```
Expected: builds (after registering the subdirectory; re-run cmake configure if the target isn't found).

- [ ] **Step 3: Implement the Rust bin.** `cargo build --manifest-path rust/Cargo.toml -p parser --bin preparse-dump`.
- [ ] **Step 4: Run the differential**

```bash
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser \
  --test preparse_differential -- --nocapture
```
Expected: both corpora match byte-for-byte. Iterate on the offset/format until identical (the most likely mismatch is offset basis or directive escaping — fix on whichever side diverges from the other's documented contract; the contract above is canonical).

- [ ] **Step 5: Commit** — `rust(parser): L1.2 Oracle B — preparse-dump tool + bin + differential + lazy corpus`

---

## Task L2.1: LazyParse skip-and-stub

**Files:**
- Modify: `rust/crates/parser/src/js/functions.rs` (`parse_function_body` skip block, `cpp:747-796`)

**Interfaces:**
- Produces: in `LazyParse` mode with `!eagerly` and `end - start >= threshold`, `parse_function_body` returns a stub `BlockStatement{is_lazy_function_body=true}` (with synthesized directive statements + the decoration fields set) instead of parsing the body.
- Consumes: L1.1 (the table), L0.2 (threshold).

**Behavior (port `cpp:747-796`):** at entry, if `self.pass == LazyParse && !eagerly`: let `start = self.cur_start()`; look up `self.pre_parsed.function_info[start.offset]` (assert present); `end = info.end`. If `(end.offset - start.offset) >= self.gc.ctx().preemptive_function_compilation_threshold()`: `self.lexer.seek(end)`, `self.advance(grammar_context)`, `self.lexer.set_prev_token_end_loc(end)`, `self.lexer.set_strict_mode(info.strict_mode)`; build the stub `BlockStatement` whose statement list is one `ExpressionStatement(StringLiteral)` per `info.directives` entry (intern via `self.lexer.get_identifier(d)`); set `is_lazy_function_body=true`, `param_yield`, `param_await` (from the now-live params), `buffer_id = self.lexer.get_buffer_id()`, `contains_arrow_functions`/`may_contain_arrow_functions_using_arguments` from `info`; `set_location(start, end, body)` and return it. Otherwise fall through to the normal block parse. **Make the `_param_yield`/`_param_await` params live** (drop the `_` prefix) — they feed the stub.

- [ ] **Step 1: Write the failing test** (`pre_lazy.rs` tests)

```rust
// LazyParse with threshold 0 defers a function body: the BlockStatement is a
// lazy stub.
#[test]
fn lazyparse_defers_body() {
    use ast::context::Context;
    use support::manager::SourceErrorManager;
    use crate::lexer::{GrammarContext, JSLexer};
    use crate::js::{JSParserImpl, ParserPass};

    let src = b"function a(){ return 1 + 2; }\n";
    let mut sm = SourceErrorManager::new();
    let id = sm.add_buffer_bytes("t", src);
    let mut ctx = Context::new();
    ctx.set_preemptive_function_compilation_threshold(0); // defer everything
    let gc = ctx.lock();
    let atoms = &gc.ctx().atom_table;
    // First PreParse to build the table.
    let table = {
        let l = JSLexer::new(id, &mut sm, atoms, GrammarContext::AllowRegExp);
        let mut pp = JSParserImpl::new_with_pass(&gc, l, ParserPass::PreParse);
        pp.parse().unwrap();
        pp.take_pre_parsed()
    };
    let l = JSLexer::new(id, &mut sm, atoms, GrammarContext::AllowRegExp);
    let mut lp = JSParserImpl::new_with_pass(&gc, l, ParserPass::LazyParse);
    lp.set_pre_parsed(table);
    let prog = lp.parse().unwrap();
    // Walk to the function's body and assert it's a lazy stub.
    assert!(crate::js::pre_lazy::tests::has_lazy_stub(prog),
            "expected a lazy function body stub");
}
```
(Provide `has_lazy_stub(&Node) -> bool` as a small test helper that walks the AST for a `BlockStatement` with `is_lazy_function_body.get() == true`.)

- [ ] **Step 2: Run it; expect FAIL** (body is parsed normally, no stub).
- [ ] **Step 3: Implement the skip block** per `cpp:747-796`.
- [ ] **Step 4: Run the test; expect PASS.** Re-run the full `parser_differential` (FullParse path untouched — must stay green).
- [ ] **Step 5: Commit** — `rust(parser): L2.1 LazyParse skip-and-stub`

---

## Task L2.2: `parse_lazy_function` demand entry

**Files:**
- Modify: `rust/crates/parser/src/js/pre_lazy.rs` (`parse_lazy_function`)

**Interfaces:**
- Produces: `parse_lazy_function(&mut self, kind: ast::node::NodeKind, param_yield: bool, param_await: bool, start: SMLoc) -> Option<&'gc Node<'gc>>` (port of `cpp:7548-7600`).
- Consumes: L2.1, the eager `parse_*` entry points with `eagerly=true`.

**Behavior (port `cpp:7548-7600`):** `self.seek(start)` (a parser `seek` mirroring `cpp:128`: `self.lexer.seek(start); /* the lexer's seek advances to the token */`), set `self.param_yield`/`self.param_await`, then dispatch on `kind`:
- `FunctionExpression` → `parse_function_expression(eagerly=true)`
- `FunctionDeclaration` → `parse_function_declaration(PARAM_RETURN, eagerly=true)`
- `ArrowFunctionExpression` → `parse_assignment_expression(PARAM_IN, eagerly=true, ...)` (match the existing arrow entry signature; pass the eager flag)
- `Property` → `parse_property_assignment(true)`, extract `.value` from the `Property` node
- `MethodDefinition` → `parse_class_body_impl(eagerly=true)`, extract the single member's `.value`
- otherwise: `unreachable!()` (C++ `llvm_unreachable`)

(Confirm the exact eager-flag parameter names against each method's current Rust signature; the eager flag already exists as the threaded `eagerly`/`force_eagerly` arg in those methods.)

- [ ] **Step 1: Write the failing test** (`pre_lazy.rs` tests)

```rust
// Demand-parsing a deferred function reproduces a non-stub body.
#[test]
fn parse_lazy_function_reparses_body() {
    // ... same setup as lazyparse_defers_body up to the lazy skeleton `prog` ...
    // Find the FunctionDeclaration's start loc + kind from the skeleton, then:
    // let body = lp.parse_lazy_function(NodeKind::FunctionDeclaration, false, false, start).unwrap();
    // assert the returned function's body is NOT a lazy stub and contains the
    // `return 1 + 2;` statement.
}
```
(Write it concretely against the real node-walk helpers; assert the re-parsed body has `is_lazy_function_body == false` and a non-empty statement list.)

- [ ] **Step 2: Run it; expect FAIL.**
- [ ] **Step 3: Implement** `parse_lazy_function` + the parser `seek`.
- [ ] **Step 4: Run the test; expect PASS.**
- [ ] **Step 5: Commit** — `rust(parser): L2.2 parse_lazy_function demand entry`

---

## Task L2.3: Oracle A — reparse-equivalence

**Files:**
- Create: `rust/crates/parser/tests/lazy_reparse.rs`

**Interfaces:**
- Consumes: L2.1, L2.2, the `ast::dump::dump_estree_json` driver.

**Behavior.** For each `.js` in `tests/parser_corpus_lazy/` (and a few from `tests/parser_corpus`), per file and for thresholds `[0, mid]` (mid = a value that defers some functions, e.g. 20):
1. Eager (`FullParse`) parse → `eager` AST. Collect `eager_funcs: map<offset, &Node>` = every function-like node (FunctionDeclaration/Expression, ArrowFunctionExpression, the getter/setter `Property` value, the class `MethodDefinition` value) keyed by node start offset.
2. PreParse (fresh parser, same buffer) → table.
3. LazyParse (fresh parser + table + threshold) → `skeleton`. Collect `lazy_funcs` the same way.
4. **Assert `eager_funcs.keys() == lazy_funcs.keys()`** (offset-set equality — catches `seek`/`advance` resume corruption).
5. For each lazy function whose body is a lazy stub, read `(kind, param_yield, param_await, start)` from the node + its stub body decorations; call `parse_lazy_function(...)`; dump the re-parsed body via `dump_estree_json` (no source locations, HideEmpty) and assert it equals the dump of the corresponding eager function's body.

Helper functions (walk + collect + dump-body) live in the test file. Use `dump_estree_json` (the no-`sm` overload) so the comparison is location-independent.

- [ ] **Step 1: Write the test** (`lazy_reparse.rs`) with the helpers and the two-threshold loop. It is the failing test (and the gate).
- [ ] **Step 2: Run it**

```bash
cargo test --manifest-path rust/Cargo.toml -p parser --test lazy_reparse -- --nocapture
```
Expected: all corpus files pass at both thresholds. If a mismatch appears, it is a real bug in L2.1/L2.2 — debug via `superpowers:systematic-debugging` (do not weaken the test).

- [ ] **Step 3: Commit** — `rust(parser): L2.3 Oracle A — reparse-equivalence`

---

## Task L3: Capstone — completeness + structural fidelity

**Files:** none (review + roadmap/handoff doc update + final gate run).

- [ ] **Step 1: Map every C++ pass/lazy site to its Rust production.** Grep the C++ for `pass_`, `PreParse`, `LazyParse`, `SaveFunctionState`, `preParsed_`, `parseLazyFunction`, `seenDirectives_`, `containsArrowFunctions_`, `mayContainArrowFunctionsUsingArguments_`; confirm each maps to a ported Rust site. Confirm **zero** remaining `// Full-pass only` / `pass_ == PreParse` / `pass_ == LazyParse` "omitted" comments in `rust/crates/parser/src/js/` (they are now implemented).

```bash
grep -rn "Full-pass\|PreParse\|LazyParse\|SaveFunctionState\|lazy" rust/crates/parser/src/js/ | grep -i "omit\|not modeled\|not ported\|dormant\|no-op"
```
Expected: no remaining "omitted/not ported" markers for the pass machinery.

- [ ] **Step 2: Structural-fidelity grep.** For the C++ ranges ported (JSParserImpl.cpp 39-52, 300-345, 505-560, 731-813, 2508-2511, 5840-5911, 7522-7600): confirm no `template`→runtime flattening and that every RAII guard became a Drop-guard or a documented explicit save/restore.
- [ ] **Step 3: Run ALL gates.**

```bash
cargo build  --manifest-path rust/Cargo.toml          # zero warnings
cargo clippy --manifest-path rust/Cargo.toml -p parser # no new lints
REQUIRE_GEN=1 cargo test --manifest-path rust/Cargo.toml -p ast --test generated_idempotent
cargo test --manifest-path rust/Cargo.toml -p parser   # whole crate
cmake --build cmake-build-asan --target hermesc ast-dump preparse-dump
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser \
  --test parser_differential -- --nocapture            # FullParse: all corpora green
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser \
  --test preparse_differential -- --nocapture          # Oracle B green
cargo test --manifest-path rust/Cargo.toml -p parser --test lazy_reparse # Oracle A green
```
Expected: everything green; the pre-existing FullParse differential byte-for-byte unchanged.

- [ ] **Step 4: Update `doc/superpowers/RustPortRoadmap.md`** (the Parser row + a "Pre/Lazy DONE" block, marking the Parser component COMPLETE; next = Sema) **and `doc/superpowers/SESSION-HANDOFF.md`** (status line: Parser COMPLETE; next component Sema). Note the two documented deviations (table threaded on parser; no `AllocationScope`).
- [ ] **Step 5: Commit** — `doc(rust): JS Parser Pre/Lazy passes COMPLETE — Parser component done (next: Sema)`

---

## Self-review (run before execution)

- **Spec coverage:** §2 components → L0.1/L0.2/L0.3; §3 PreParse → L1.1; §3 LazyParse → L2.1; §3 demand → L2.2; §4 Oracle B → L1.2; §4 Oracle A → L2.3; §4 corpus → L1.2; §5 slicing → L0–L3 + capstone. All covered.
- **Placeholders:** test bodies in L2.2/L2.3 are described against real helpers (the node-walk/dump utilities are written in those tasks); no `TBD`/`TODO`.
- **Type consistency:** `PreParsedFunctionInfo`/`PreParsedBufferInfo`/`ParserPass`/`pre_parsed`/`take_pre_parsed`/`set_pre_parsed`/`save_function_state`/`copy_seen_directives`/`pre_parse_buffer`/`parse_lazy_function` used consistently across tasks.
- **Open verification points for the implementer (call out in review, not blockers):** (a) the exact eager-flag parameter name in each `parse_*` entry (`eagerly` vs `force_eagerly`); (b) the offset basis equivalence between C++ `addNewSourceBuffer` locations and Rust `SMLoc.offset` (normalized by subtracting the buffer start in the C++ tool); (c) whether the parser `seek` needs an explicit `advance` after `lexer.seek` (mirror `cpp:128-131`).
