# JS Parser — Phase P0 (Foundations + Gate) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the parser scaffold and the live `hermesc -dump-ast` byte-for-byte differential gate, parsing the minimal program (empty / whitespace / comments only) end-to-end into an AST and dumping it.

**Architecture:** The JS parser lives in the existing `parser` crate (which gains an `ast` dependency). `JSParserImpl<'gc>` drives the completed `JSLexer` under one `GCLock`, allocating ESTree nodes in the `ast` GC arena and returning `Option<&'gc Node<'gc>>` (faithful to C++ `llvh::Optional`). The lexer and the AST share **one** `AtomTable` — the lexer borrows it through the `GCLock` (`&gc.ctx().atom_table`) while node allocation mutates *different* `UnsafeCell`s, so it is sound under a single lock. Validation is a Rust `ast-dump` bin diffed byte-for-byte against `hermesc -dump-ast`, which (verified in `CompilerDriver.cpp:867`) dumps the **raw parse AST** pre-Sema.

**Tech Stack:** Rust 1.96.0 workspace (`rust/`), the `ast` + `parser` + `support` crates, the C++ `hermesc` binary in `cmake-build-asan` as the differential oracle.

**Design spec:** `doc/superpowers/specs/2026-06-06-js-parser-design.md`.

**Conventions (from the spec — do not relitigate):** keep C++ templates as Rust generics (never flatten to runtime params); C++ RAII guards → explicit set/restore; `Option<T>` with `None` = "error already reported"; gate on **zero `cargo build` warnings**; commit directly to `rust` (never PR/merge); commit-message trailer `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.

**Validation commands (run from repo root; never `cd`):**
```bash
cargo build  --manifest-path rust/Cargo.toml                       # expect ZERO warnings
cargo test   --manifest-path rust/Cargo.toml -p ast                # AST tests (golden unchanged)
cargo test   --manifest-path rust/Cargo.toml -p parser             # parser tests
cmake --build cmake-build-asan --target hermesc                    # build the oracle once
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test parser_differential -- --nocapture
```
If `cmake-build-asan/` is missing, configure it (git-ignored):
`cmake -B cmake-build-asan -G Ninja -DCMAKE_BUILD_TYPE=Debug -DHERMES_ENABLE_ADDRESS_SANITIZER=ON -DCMAKE_CXX_FLAGS="-O1" -DCMAKE_C_FLAGS="-O1"`.

---

## File structure (what P0 creates/modifies)

- **Modify** `rust/crates/ast/src/node_child.rs` — add `debug_loc` to `NodeMetadata`.
- **Modify** `rust/crates/parser/Cargo.toml` — add the `ast` dependency.
- **Modify** `rust/crates/parser/src/lib.rs` — register `pub mod js;`.
- **Create** `rust/crates/parser/src/js/mod.rs` — `JSParserImpl<'gc>` struct, driver helpers, `Param`, `set_location`, `parse`/`parseProgram`.
- **Create** `rust/crates/parser/src/bin/ast_dump.rs` — the Rust dump oracle bin.
- **Create** `rust/crates/parser/tests/parser_differential.rs` — the byte-for-byte differential test.
- **Create** `rust/crates/parser/tests/parser_corpus/{empty.js,whitespace.js,line_comment.js,block_comment.js}` — the P0 corpus.

---

## Task 0.1: Add `debug_loc` to AST `NodeMetadata`

**Files:**
- Modify: `rust/crates/ast/src/node_child.rs:33-58`
- Test: `rust/crates/ast/tests/node_model.rs` (append a test)

- [ ] **Step 1: Write the failing test**

Append to `rust/crates/ast/tests/node_model.rs`:

```rust
#[test]
fn node_metadata_debug_loc_defaults_to_start() {
    use ast::node_child::NodeMetadata;
    use support::location::{SMLoc, SMRange, SourceId};

    let start = SMLoc { source: SourceId(1), offset: 10 };
    let end = SMLoc { source: SourceId(1), offset: 20 };
    let md = NodeMetadata::new(SMRange { start, end });
    assert_eq!(md.debug_loc.get(), start, "debug_loc must default to range start");

    let dbg = SMLoc { source: SourceId(1), offset: 15 };
    let md2 = NodeMetadata::new_with_debug(SMRange { start, end }, dbg);
    assert_eq!(md2.debug_loc.get(), dbg);

    // duplicate() must carry debug_loc.
    let dup = md2.duplicate_pub_for_test();
    assert_eq!(dup.debug_loc.get(), dbg);
}
```

> Note: `duplicate()` is `pub(crate)`. Add a `#[cfg(test)]`-only public shim in `node_child.rs` next to `duplicate`:
> ```rust
> #[cfg(test)]
> pub fn duplicate_pub_for_test(&self) -> NodeMetadata<'gc> { self.duplicate() }
> ```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml -p ast node_metadata_debug_loc_defaults_to_start`
Expected: FAIL to compile — `no field debug_loc`, `no function new_with_debug`.

- [ ] **Step 3: Implement the field + constructors**

In `rust/crates/ast/src/node_child.rs`, ensure `SMLoc` is imported (it shares the module with `SMRange`; add `use support::location::SMLoc;` if absent). Change the struct and impl:

```rust
pub struct NodeMetadata<'gc> {
    pub(crate) phantom: PhantomData<&'gc Node<'gc>>,
    pub range: Cell<SMRange>,
    /// Debug location, mirroring ESTree.h Node debug loc set by
    /// JSParserImpl::setLocation. Defaults to range start.
    pub debug_loc: Cell<SMLoc>,
    /// 0, 1, or 2 (meaning "2 or more"), mirroring ESTree.h Node::parens_.
    pub parens: Cell<u8>,
}

impl<'gc> NodeMetadata<'gc> {
    pub fn new(range: SMRange) -> Self {
        NodeMetadata {
            phantom: PhantomData,
            range: Cell::new(range),
            debug_loc: Cell::new(range.start),
            parens: Cell::new(0),
        }
    }

    /// Like `new`, but with an explicit debug location (C++ 4-arg setLocation).
    pub fn new_with_debug(range: SMRange, debug_loc: SMLoc) -> Self {
        NodeMetadata {
            phantom: PhantomData,
            range: Cell::new(range),
            debug_loc: Cell::new(debug_loc),
            parens: Cell::new(0),
        }
    }

    pub(crate) fn duplicate(&self) -> NodeMetadata<'gc> {
        NodeMetadata {
            phantom: self.phantom,
            range: Cell::new(self.range.get()),
            debug_loc: Cell::new(self.debug_loc.get()),
            parens: Cell::new(self.parens.get()),
        }
    }

    #[cfg(test)]
    pub fn duplicate_pub_for_test(&self) -> NodeMetadata<'gc> {
        self.duplicate()
    }
}
```

- [ ] **Step 4: Run the test + the whole AST suite (golden must be unchanged)**

Run:
```bash
cargo test --manifest-path rust/Cargo.toml -p ast
```
Expected: PASS — the new test passes; **all `dump_golden` and `generated_idempotent` tests still pass byte-for-byte** (the dumper never reads `debug_loc`, so output is unchanged).

- [ ] **Step 5: Commit**

```bash
git add rust/crates/ast/src/node_child.rs rust/crates/ast/tests/node_model.rs
git commit -m "$(cat <<'EOF'
rust(ast): add debug_loc to NodeMetadata (parser will set it)

The C++ JSParserImpl::setLocation sets start, end, AND a debug location;
our ported NodeMetadata carried only range+parens. Add debug_loc: Cell<SMLoc>
(defaults to range.start; new_with_debug for the 4-arg overload). The dumper
does not emit it (neither does C++ -dump-ast), so all golden output is
byte-unchanged.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 0.2: `parser` crate depends on `ast`

**Files:**
- Modify: `rust/crates/parser/Cargo.toml`

- [ ] **Step 1: Add the dependency**

In `rust/crates/parser/Cargo.toml`, under `[dependencies]`, add (matching the existing path style used for `support`/`atom_table`):

```toml
ast = { path = "../ast" }
```

- [ ] **Step 2: Verify it builds**

Run: `cargo build --manifest-path rust/Cargo.toml -p parser`
Expected: builds, zero warnings (no code uses `ast` yet — this just wires the dep).

- [ ] **Step 3: Commit**

```bash
git add rust/crates/parser/Cargo.toml
git commit -m "$(cat <<'EOF'
rust(parser): depend on the ast crate (for the JS parser)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 0.3: `JSParserImpl<'gc>` scaffold + driver helpers

**Files:**
- Create: `rust/crates/parser/src/js/mod.rs`
- Modify: `rust/crates/parser/src/lib.rs` (add `pub mod js;`)

This task ports the always-needed driver surface from `JSParserImpl.h`: the `Param` flag struct, the struct fields P0 needs, `new` (constructs the lexer-backed parser and advances to the first token), the token helpers (`advance`/`check`/`check_n`/`eat`/`check_and_eat`/`need`/`error`/`error_expected`), the recursion guard, and `set_location`. Later phases extend this module and add sibling modules (`expressions.rs`, …) as `impl<'gc> JSParserImpl<'gc>` blocks.

- [ ] **Step 1: Write the failing test**

Create the module with only a `parse`-less skeleton is not testable on its own; instead drive it from a unit test that constructs the parser and checks the first token. Append to the new `rust/crates/parser/src/js/mod.rs` a `#[cfg(test)]` module (shown in Step 3). First, register the module — in `rust/crates/parser/src/lib.rs` add:

```rust
pub mod js;
```

Then the test (in `js/mod.rs`, inside `#[cfg(test)] mod tests`):

```rust
#[test]
fn parser_constructs_and_sees_first_token() {
    use ast::context::Context;
    use support::manager::SourceErrorManager;

    let mut sm = SourceErrorManager::new();
    let buf_id = sm.add_buffer_bytes("input", b"  /* hi */  ");
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let atoms = &gc.ctx().atom_table;
    let lexer = crate::lexer::JSLexer::new(
        buf_id, &mut sm, atoms, crate::lexer::GrammarContext::AllowRegExp,
    );
    let parser = JSParserImpl::new(&gc, lexer);
    // After construction the current token is EOF (only trivia in the source).
    assert_eq!(parser.cur_kind(), crate::token_kinds::TokenKind::eof);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml -p parser parser_constructs_and_sees_first_token`
Expected: FAIL to compile — `JSParserImpl` undefined.

- [ ] **Step 3: Implement the scaffold**

Create `rust/crates/parser/src/js/mod.rs`:

```rust
/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! The JS parser (`JSParserImpl`). Port of `lib/Parser/JSParserImpl*`.
//! Recursive-descent LL(1) over `JSLexer`, building the `ast` ESTree.

use ast::context::GCLock;
use ast::node::Node;
use support::location::{SMLoc, SMRange};

use crate::lexer::{GrammarContext, JSLexer};
use crate::token_kinds::TokenKind;

/// A bitmask of grammar parameters threaded between parse functions.
/// Port of `JSParserImpl::Param`.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Param(u32);

/// `[In]` — "in" is recognized as a binary operator in RelationalExpression.
pub const PARAM_IN: Param = Param(1 << 0);
/// `[Return]`
pub const PARAM_RETURN: Param = Param(1 << 1);
/// `[Default]`
pub const PARAM_DEFAULT: Param = Param(1 << 2);
/// `[Tagged]`
pub const PARAM_TAGGED: Param = Param(1 << 3);

impl Param {
    /// Union (C++ `operator+`).
    pub fn plus(self, b: Param) -> Param { Param(self.0 | b.0) }
    /// Difference (C++ `operator-`).
    pub fn minus(self, b: Param) -> Param { Param(self.0 & !b.0) }
    /// True if any flag in `p` is set (C++ `has`).
    pub fn has(self, p: Param) -> bool { (self.0 & p.0) != 0 }
    /// `p` if any of its bits are set here, else empty (C++ `get`).
    pub fn get(self, p: Param) -> Param { Param(self.0 & p.0) }
}

/// Maximum recursion depth, mirroring the non-MSVC default in JSParserImpl.h.
const MAX_RECURSION_DEPTH: u32 = 1024;

/// The JS parser. `'gc` is the arena lifetime of the AST it builds.
pub struct JSParserImpl<'gc> {
    /// The arena lock; all nodes are allocated through this.
    gc: &'gc GCLock<'gc, 'gc>,
    /// The lexer driving the token stream. Owns `&mut SourceErrorManager`.
    lexer: JSLexer<'gc>,
    /// Current parser recursion depth (stack-overflow guard).
    recursion_depth: u32,
    /// Set when the parser is inside a generator function (`yield`).
    param_yield: bool,
    /// Set when the parser is inside an async function (`await`).
    param_await: bool,
    /// Set on the `use static builtin` directive.
    use_static_builtin: bool,
}

impl<'gc> JSParserImpl<'gc> {
    /// Construct the parser and lex the first token (C++ ctor does
    /// `tok_ = lexer_.advance()`).
    pub fn new(gc: &'gc GCLock<'gc, 'gc>, mut lexer: JSLexer<'gc>) -> Self {
        // Prime the first token.
        lexer.advance(GrammarContext::AllowRegExp);
        JSParserImpl {
            gc,
            lexer,
            recursion_depth: 0,
            param_yield: false,
            param_await: false,
            use_static_builtin: false,
        }
    }

    /// True if the parser detected `use static builtin`.
    pub fn get_use_static_builtin(&self) -> bool {
        self.use_static_builtin
    }

    /// The kind of the current token.
    #[inline]
    fn cur_kind(&self) -> TokenKind {
        self.lexer.token().kind()
    }

    /// The source range of the current token.
    #[inline]
    fn cur_range(&self) -> SMRange {
        self.lexer.token().source_range()
    }

    /// The start location of the current token.
    #[inline]
    fn cur_start(&self) -> SMLoc {
        self.lexer.token().start_loc()
    }

    /// True if the current token is `kind`. Port of `check(TokenKind)`.
    #[inline]
    fn check(&self, kind: TokenKind) -> bool {
        self.cur_kind() == kind
    }

    /// True if the current token is `k1` or `k2`. Port of `check(k1, k2)`.
    #[inline]
    fn check2(&self, k1: TokenKind, k2: TokenKind) -> bool {
        let k = self.cur_kind();
        k == k1 || k == k2
    }

    /// Consume the current token, advancing the lexer; return the consumed
    /// token's range. Port of `JSParserImpl::advance` (note: C++ returns the
    /// PREVIOUS token's range — we copy it out before advancing).
    fn advance(&mut self, grammar_context: GrammarContext) -> SMRange {
        let prev = self.cur_range();
        self.lexer.advance(grammar_context);
        prev
    }

    /// Consume the current token if it is `kind`; return whether it matched.
    /// Port of `checkAndEat(TokenKind, GrammarContext)`.
    fn check_and_eat(&mut self, kind: TokenKind, grammar_context: GrammarContext) -> bool {
        if self.check(kind) {
            self.advance(grammar_context);
            true
        } else {
            false
        }
    }

    /// Report an error at `range`. Routed through the lexer's SourceErrorManager.
    fn error_at(&mut self, range: SMRange, msg: &str) {
        self.lexer
            .get_source_mgr_mut()
            .error(range, msg, support::diag::Subsystem::Parser);
    }

    /// Report an error at the current token. Port of `error(Twine)`.
    fn error_cur(&mut self, msg: &str) {
        let range = self.cur_range();
        self.error_at(range, msg);
    }

    /// Check the current token is `kind`; if not, report an error and return
    /// false. Port of `need(kind, where, what, whatLoc)` (P0 form: the simple
    /// "expected X" message; richer `where`/`what` plumbing arrives with the
    /// statement/expression phases that need it).
    fn need(&mut self, kind: TokenKind, where_: &str) -> bool {
        if self.check(kind) {
            return true;
        }
        let msg = format!(
            "'{}' expected{}",
            crate::token_kinds::token_kind_str(kind),
            where_
        );
        self.error_cur(&msg);
        false
    }

    /// Check the current token is `kind`; if so consume it and return true,
    /// else report an error and return false. Port of `eat`.
    fn eat(&mut self, kind: TokenKind, grammar_context: GrammarContext, where_: &str) -> bool {
        if self.need(kind, where_) {
            self.advance(grammar_context);
            true
        } else {
            false
        }
    }

    /// Return true (and report an error) if the recursion limit is exceeded.
    /// Port of `recursionDepthCheck`.
    #[inline]
    fn recursion_depth_check(&mut self) -> bool {
        if self.recursion_depth < MAX_RECURSION_DEPTH {
            return false;
        }
        let range = self.cur_range();
        self.error_at(range, "Too many nested expressions/statements/declarations");
        true
    }

    /// Allocate `node` with its source locations set. Port of the 3-arg
    /// `setLocation(start, end, node)`: debug loc defaults to start.
    ///
    /// `start`/`end` are start/end `SMLoc`s; richer overloads (accepting tokens
    /// or nodes) are added as callers need them — for now callers pass explicit
    /// `SMLoc`s.
    fn set_location(&self, start: SMLoc, end: SMLoc, node: Node<'gc>) -> &'gc Node<'gc> {
        // Stamp the metadata before allocation. Every node embeds
        // `metadata.range`/`debug_loc`; the per-node `new` already built the
        // metadata, so we overwrite it via the node's metadata Cell.
        let allocated = self.gc.alloc(node);
        let md = allocated.metadata();
        md.range.set(SMRange { start, end });
        md.debug_loc.set(start);
        allocated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl<'gc> JSParserImpl<'gc> {
        /// Test-only accessor for the current token kind.
        pub(crate) fn cur_kind_pub(&self) -> TokenKind {
            self.cur_kind()
        }
    }

    #[test]
    fn parser_constructs_and_sees_first_token() {
        use ast::context::Context;
        use support::manager::SourceErrorManager;

        let mut sm = SourceErrorManager::new();
        let buf_id = sm.add_buffer_bytes("input", b"  /* hi */  ");
        let mut ctx = Context::new();
        let gc = ctx.lock();
        let atoms = &gc.ctx().atom_table;
        let lexer = JSLexer::new(buf_id, &mut sm, atoms, GrammarContext::AllowRegExp);
        let parser = JSParserImpl::new(&gc, lexer);
        assert_eq!(parser.cur_kind_pub(), TokenKind::eof);
    }
}
```

> **Implementation notes for the worker (read before coding):**
> - **`Node::metadata()` accessor:** `set_location` calls `allocated.metadata()` to reach the shared `NodeMetadata`. The `Node` enum may not yet expose a uniform `metadata()` getter across all arms. If it does not, add a generated `Node::metadata(&self) -> &NodeMetadata<'gc>` arm in `ast/src/node.rs` via `gen_nodes.py` (every node has `metadata` as its first field), regenerate, and run the idempotency guard (`REQUIRE_GEN=1 cargo test -p ast --test generated_idempotent`). Prefer this over per-call matching — it is the faithful analog of the C++ `Node` base accessors. Treat adding `metadata()` as a small sub-step of this task and commit it with the AST change pattern from Task 0.1.
> - **Lifetimes:** the struct uses a single `'gc` for the lock, the lexer borrow, and the node lifetime. The lexer also holds `&mut SourceErrorManager`; ensure the driver (Task 0.5 / tests) declares `sm` *before* `ctx` so drop order is correct, and unifies the borrows to `'gc`. If the borrow checker rejects the single-lifetime form, introduce a second lifetime `'a` for the lexer (`JSParserImpl<'gc, 'a>`, `lexer: JSLexer<'a>`, with `'gc: 'a`); this is a mechanical adjustment, not a design change.
> - `token_kind_str` and `TokenKind::eof` already exist in `crate::token_kinds` (used by the lexer). Confirm the `Subsystem::Parser` path (`support::diag::Subsystem`) and the `SourceErrorManager::error(range, msg, subsystem)` signature against `support/src/manager.rs`; adjust the call to match exactly.

- [ ] **Step 4: Run the test**

Run: `cargo test --manifest-path rust/Cargo.toml -p parser parser_constructs_and_sees_first_token`
Expected: PASS. Also run `cargo build --manifest-path rust/Cargo.toml` — zero warnings (allow-and-comment any unavoidable "field never read" on `param_yield`/`param_await`/`use_static_builtin`/helpers that P0 doesn't exercise yet, since later phases use them; prefer `#[allow(dead_code)]` with a `// used from P1+` comment over deleting faithful state).

- [ ] **Step 5: Commit**

```bash
git add rust/crates/parser/src/js/mod.rs rust/crates/parser/src/lib.rs
git commit -m "$(cat <<'EOF'
rust(parser): JSParserImpl<'gc> scaffold + driver helpers (P0)

Port the always-needed driver surface from JSParserImpl.h: Param flags,
new (advances to first token), check/check2/advance/check_and_eat/need/eat,
error reporting via the lexer's SourceErrorManager, recursion guard, and
set_location (3-arg, debug loc = start). Lexer + AST share one AtomTable via
the GCLock. Later phases add sibling impl modules.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 0.4: Minimal `parseProgram` + `parse`

**Files:**
- Modify: `rust/crates/parser/src/js/mod.rs` (add the `parse`/`parse_program` methods + a test)

P0 parses only empty/whitespace/comment sources: `parse_program` builds a `Program` node with an empty body covering `[start .. EOF]`, requiring EOF (any real statement token is a not-yet-supported error in P0). Statement parsing arrives in P1–P4.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` in `js/mod.rs`:

```rust
#[test]
fn parses_empty_program() {
    use ast::context::Context;
    use ast::node::Node;
    use support::manager::SourceErrorManager;

    let mut sm = SourceErrorManager::new();
    let buf_id = sm.add_buffer_bytes("input", b"/* only trivia */\n");
    let mut ctx = Context::new();
    let gc = ctx.lock();
    let atoms = &gc.ctx().atom_table;
    let lexer = JSLexer::new(buf_id, &mut sm, atoms, GrammarContext::AllowRegExp);
    let mut parser = JSParserImpl::new(&gc, lexer);
    let program = parser.parse().expect("empty program parses");
    match program {
        Node::Program(p) => assert!(p.body.is_empty(), "empty source -> empty body"),
        other => panic!("expected Program, got {:?}", other.kind()),
    }
    assert_eq!(parser.error_count_pub(), 0);
}
```

> Add a test-only `pub(crate) fn error_count_pub(&self) -> u32 { self.lexer.get_source_mgr().error_count() }` next to the other test shim. Confirm `NodeList::is_empty()` exists in `ast` (the model has `NodeList::empty()`); if the predicate is named differently (e.g. comparing to `empty()`), adjust the assertion.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml -p parser parses_empty_program`
Expected: FAIL to compile — no `parse` method.

- [ ] **Step 3: Implement `parse` + `parse_program`**

Add to `impl<'gc> JSParserImpl<'gc>` in `js/mod.rs`:

```rust
    /// Parse the whole program. Port of `JSParserImpl::parse` /
    /// `parseProgram` (P0: empty body only; statement parsing is P1–P4).
    pub fn parse(&mut self) -> Option<&'gc Node<'gc>> {
        self.parse_program()
    }

    fn parse_program(&mut self) -> Option<&'gc Node<'gc>> {
        use ast::node::Program;
        use ast::node_child::{NodeList, NodeMetadata};

        let start = self.cur_start();
        // P0 supports only trivia-only sources: the first significant token
        // must be EOF. (Statement-list parsing lands in P1–P4.)
        if !self.check(TokenKind::eof) {
            self.error_cur("statement parsing not yet implemented (parser phase P0)");
            return None;
        }
        let end = self.cur_start(); // EOF: zero-width at the end of input.
        let program = Node::Program(Program::new(NodeMetadata::new(SMRange { start, end }), NodeList::empty()));
        Some(self.set_location(start, end, program))
    }
```

> Confirm the `ast::node::Program` and `ast::node_child::{NodeList, NodeMetadata}` import paths against the crate's `pub use`/module layout; adjust if the crate re-exports them elsewhere (e.g. `ast::node::NodeList`).

- [ ] **Step 4: Run the test**

Run: `cargo test --manifest-path rust/Cargo.toml -p parser parses_empty_program`
Expected: PASS. `cargo build --manifest-path rust/Cargo.toml` — zero warnings.

- [ ] **Step 5: Commit**

```bash
git add rust/crates/parser/src/js/mod.rs
git commit -m "$(cat <<'EOF'
rust(parser): minimal parseProgram (empty body) + parse (P0)

Parses trivia-only sources into an empty Program node covering [start..EOF];
non-EOF input errors (statement parsing is P1-P4). Establishes the
parse -> &'gc Node entry point the dump oracle drives.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 0.5: The Rust `ast-dump` bin

**Files:**
- Create: `rust/crates/parser/src/bin/ast_dump.rs`

Mirrors `json_parse_dump.rs`. Reads a file (or `-` for stdin) plus flags, parses, and dumps via `ast::dump::dump_estree_json_with_sm`. Flags mirror `hermesc`'s `-dump-ast` path so output can be diffed byte-for-byte.

- [ ] **Step 1: Write the bin**

Create `rust/crates/parser/src/bin/ast_dump.rs`:

```rust
/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! ast-dump: Rust mirror of `hermesc -dump-ast`. Parses a JS file and prints
//! the ESTree as JSON via the ported ESTreeJSONDumper. The parser_differential
//! test compares this byte-for-byte against `hermesc -dump-ast`.
//!
//! OUTPUT CONTRACT
//!   On success (parsed AND error_count()==0): the dumped JSON (no extra
//!     trailing newline beyond what the dumper emits).
//!   On error: exactly "ERROR <count>\n".
//!
//! Args: [--pretty] [--dump-source-location] [--include-empty-ast-nodes]
//!       [--include-raw-ast-prop] <file|->

use std::io::{self, Read, Write};

use ast::context::Context;
use ast::dump::{dump_estree_json_with_sm, ESTreeDumpMode, ESTreeRawProp, LocationDumpMode};
use parser::js::JSParserImpl;
use parser::lexer::{GrammarContext, JSLexer};
use support::manager::SourceErrorManager;

fn main() {
    let mut pretty = false;
    let mut dump_loc = false;
    let mut include_empty = false;
    let mut include_raw = false;
    let mut file_path: Option<String> = None;

    let prog = std::env::args().next().unwrap_or_else(|| "ast-dump".to_string());
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--pretty" => pretty = true,
            "--dump-source-location" => dump_loc = true,
            "--include-empty-ast-nodes" => include_empty = true,
            "--include-raw-ast-prop" => include_raw = true,
            a if a.starts_with("--") => {
                eprintln!("{prog}: unknown flag '{a}'");
                std::process::exit(1);
            }
            a => {
                if file_path.is_some() {
                    eprintln!("{prog}: multiple input files");
                    std::process::exit(1);
                }
                file_path = Some(a.to_string());
            }
        }
    }

    let bytes = match file_path.as_deref() {
        Some("-") | None => {
            let mut b = Vec::new();
            io::stdin().read_to_end(&mut b).expect("read stdin");
            b
        }
        Some(p) => std::fs::read(p).unwrap_or_else(|e| {
            eprintln!("{prog}: {p}: {e}");
            std::process::exit(1);
        }),
    };

    let mut sm = SourceErrorManager::new();
    let buf_id = sm.add_buffer_bytes("input", &bytes);
    let mut ctx = Context::new();
    let gc = ctx.lock();

    // Parse inside a scope so the parser (and its &mut sm borrow) drops before
    // we read sm for the dump.
    let result: Option<&Node> = {
        let atoms = &gc.ctx().atom_table;
        let lexer = JSLexer::new(buf_id, &mut sm, atoms, GrammarContext::AllowRegExp);
        let mut parser = JSParserImpl::new(&gc, lexer);
        parser.parse()
    };

    let out = io::stdout();
    let mut out = out.lock();
    match result {
        Some(program) if sm.error_count() == 0 => {
            let mode = if include_empty {
                ESTreeDumpMode::DumpAll
            } else {
                ESTreeDumpMode::HideEmpty
            };
            let loc_mode = if dump_loc {
                LocationDumpMode::LocAndRange
            } else {
                LocationDumpMode::None
            };
            let raw = if include_raw { ESTreeRawProp::Include } else { ESTreeRawProp::Exclude };
            let mut s = String::new();
            dump_estree_json_with_sm(
                &mut s, program, pretty, mode, &sm, loc_mode, raw, &gc.ctx().atom_table,
            );
            out.write_all(s.as_bytes()).unwrap();
        }
        _ => {
            writeln!(out, "ERROR {}", sm.error_count()).unwrap();
        }
    }
}

use ast::node::Node;
```

> **Notes:**
> - For `JSParserImpl` and `lexer` to be reachable from the bin, confirm they are `pub` in the `parser` crate (Task 0.3 added `pub mod js;`; `crate::lexer` must also be `pub mod lexer;` in `lib.rs` — check and make it `pub` if needed, with a one-line commit).
> - **`LocationDumpMode` default:** the differential (Task 0.6) will tell you whether `hermesc`'s `-dump-ast` without `--dump-source-location` corresponds to `LocationDumpMode::None` and whether `--dump-source-location` maps to `LocAndRange` vs `Loc`/`Range`. Adjust the mapping until the bytes match; the spec's risk #1 calls this out.
> - The `use ast::node::Node;` at the bottom is fine in Rust (items are order-independent); move it to the top with the other `use`s if you prefer.

- [ ] **Step 2: Build the bin**

Run: `cargo build --manifest-path rust/Cargo.toml -p parser --bin ast-dump`
Expected: builds, zero warnings.

- [ ] **Step 3: Smoke-test it**

Run:
```bash
printf '/* hi */\n' | ./rust/target/debug/ast-dump --dump-source-location -
```
Expected: a JSON object with `"type":"Program"` (exact bytes validated in Task 0.6).

- [ ] **Step 4: Commit**

```bash
git add rust/crates/parser/src/bin/ast_dump.rs rust/crates/parser/src/lib.rs
git commit -m "$(cat <<'EOF'
rust(parser): ast-dump bin (Rust mirror of hermesc -dump-ast)

Parses a file and dumps the ESTree via dump_estree_json_with_sm, with flags
mirroring the hermesc -dump-ast path. Drives the parser_differential gate.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 0.6: The `parser_differential` test + P0 corpus

**Files:**
- Create: `rust/crates/parser/tests/parser_corpus/empty.js` (0 bytes)
- Create: `rust/crates/parser/tests/parser_corpus/whitespace.js` (e.g. `"  \n\t \n"`)
- Create: `rust/crates/parser/tests/parser_corpus/line_comment.js` (e.g. `"// a line comment\n"`)
- Create: `rust/crates/parser/tests/parser_corpus/block_comment.js` (e.g. `"/* block\n   comment */\n"`)
- Create: `rust/crates/parser/tests/parser_differential.rs`

Mirrors the lexer/JSON differential pattern: resolve `hermesc` via `CARGO_MANIFEST_DIR`, run it with `-dump-ast` + `--dump-source-location` and the Rust `ast-dump` with the matching flag, compare byte-for-byte. Honor `REQUIRE_DIFFERENTIAL=1` (hard-fail if the binary is absent rather than silently skipping — the exact bug the lexer hit).

- [ ] **Step 1: Create the corpus files**

```bash
mkdir -p rust/crates/parser/tests/parser_corpus
printf ''                       > rust/crates/parser/tests/parser_corpus/empty.js
printf '  \n\t \n'              > rust/crates/parser/tests/parser_corpus/whitespace.js
printf '// a line comment\n'    > rust/crates/parser/tests/parser_corpus/line_comment.js
printf '/* block\n   comment */\n' > rust/crates/parser/tests/parser_corpus/block_comment.js
```

- [ ] **Step 2: Write the differential test**

Create `rust/crates/parser/tests/parser_differential.rs`:

```rust
/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Byte-for-byte differential: Rust `ast-dump` vs `hermesc -dump-ast`.
//! `hermesc -dump-ast` dumps the raw parse AST (pre-Sema; CompilerDriver.cpp:867),
//! so this gates the parser directly. Set REQUIRE_DIFFERENTIAL=1 to hard-fail
//! when hermesc is missing instead of skipping.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Locate the `hermesc` oracle relative to the repo root (two levels up from
/// the parser crate's CARGO_MANIFEST_DIR: rust/crates/parser -> repo root).
fn hermesc_path() -> Option<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest.ancestors().nth(3)?; // parser -> crates -> rust -> root
    let p = repo_root.join("cmake-build-asan/bin/hermesc");
    if p.exists() { Some(p) } else { None }
}

/// Locate our compiled `ast-dump` bin (same target dir as the test binary).
fn ast_dump_path() -> PathBuf {
    // tests run from the crate dir; the workspace target is rust/target.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.ancestors().nth(2).unwrap(); // parser -> crates -> rust
    // Prefer debug; fall back is unnecessary for the gate.
    root.join("target/debug/ast-dump")
}

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/parser_corpus")
}

fn run(cmd: &mut Command) -> (Vec<u8>, Vec<u8>, Option<i32>) {
    let out = cmd.output().expect("spawn");
    (out.stdout, out.stderr, out.status.code())
}

#[test]
fn parser_differential_p0() {
    let require = std::env::var("REQUIRE_DIFFERENTIAL").is_ok();
    let hermesc = match hermesc_path() {
        Some(p) => p,
        None => {
            if require {
                panic!("REQUIRE_DIFFERENTIAL=1 but cmake-build-asan/bin/hermesc not found; \
                        build it: cmake --build cmake-build-asan --target hermesc");
            }
            eprintln!("skipping parser_differential: hermesc not found (set REQUIRE_DIFFERENTIAL=1 to force)");
            return;
        }
    };
    let ast_dump = ast_dump_path();
    assert!(ast_dump.exists(),
        "ast-dump bin not built at {:?}; run: cargo build -p parser --bin ast-dump", ast_dump);

    let mut files: Vec<PathBuf> = std::fs::read_dir(corpus_dir())
        .expect("corpus dir")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().map(|e| e == "js").unwrap_or(false))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "empty corpus");

    let mut checked = 0;
    for f in &files {
        // C++ oracle: dump the raw parse AST with source locations.
        let (c_out, _c_err, _c_code) = run(
            Command::new(&hermesc).args(["-dump-ast", "-dump-source-location"]).arg(f),
        );
        // Rust: matching flag.
        let (r_out, _r_err, _r_code) = run(
            Command::new(&ast_dump).args(["--dump-source-location"]).arg(f),
        );
        assert_eq!(
            String::from_utf8_lossy(&c_out),
            String::from_utf8_lossy(&r_out),
            "mismatch for {}", f.display(),
        );
        checked += 1;
    }
    eprintln!("parser differential: {checked} corpus files matched");
}
```

> **Notes:**
> - Verify the exact `hermesc` flag spellings (`-dump-ast`, `-dump-source-location`) against `lib/CompilerDriver/CompilerDriver.cpp` (the `clEnumValN` registrations) — LLVM `cl::opt` accepts both `-flag` and `--flag`. Adjust if needed.
> - This first run is where the **flag-mapping risk** (spec §10.1) gets resolved: if the bytes differ only in location formatting, fix the `LocationDumpMode`/mode mapping in `ast_dump.rs`, not the test.
> - The test depends on the `ast-dump` bin being built first. Document that the gate command is: `cargo build -p parser --bin ast-dump && REQUIRE_DIFFERENTIAL=1 cargo test -p parser --test parser_differential`.

- [ ] **Step 3: Build the bin, then run the differential**

Run:
```bash
cmake --build cmake-build-asan --target hermesc
cargo build --manifest-path rust/Cargo.toml -p parser --bin ast-dump
REQUIRE_DIFFERENTIAL=1 cargo test --manifest-path rust/Cargo.toml -p parser --test parser_differential -- --nocapture
```
Expected: `parser differential: 4 corpus files matched` — PASS. If mismatches appear, they will be in `Program` range/loc formatting; reconcile the flag/mode mapping in `ast_dump.rs` until byte-equal.

- [ ] **Step 4: Commit**

```bash
git add rust/crates/parser/tests/parser_differential.rs rust/crates/parser/tests/parser_corpus
git commit -m "$(cat <<'EOF'
rust(parser): parser_differential gate vs hermesc -dump-ast (P0)

Byte-for-byte differential of the Rust ast-dump bin against hermesc -dump-ast
(raw parse AST, pre-Sema) over a trivia-only corpus. Resolves hermesc via
CARGO_MANIFEST_DIR; REQUIRE_DIFFERENTIAL=1 hard-fails when absent. This is the
gate every later parser phase extends.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 0.7: Update the roadmap

**Files:**
- Modify: `doc/superpowers/RustPortRoadmap.md`

- [ ] **Step 1: Mark P0 done**

Add a "JS Parser — P0 DONE" note under the Parser row / a new subsection: scaffold + driver helpers + minimal `parseProgram` + `ast-dump` bin + live `parser_differential` gate (4 trivia-only corpus files) + the AST `debug_loc` addition. Note next = P1 (core expressions).

- [ ] **Step 2: Commit**

```bash
git add doc/superpowers/RustPortRoadmap.md
git commit -m "$(cat <<'EOF'
doc(rust): JS Parser P0 (foundations + differential gate) complete

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Self-review

**Spec coverage (P0 slice of the spec):**
- §4.1 crate placement → Task 0.2 (`ast` dep) + 0.3 (`src/js/`). ✓
- §4.2 lifetime/arena model (one `GCLock`, `Option<&'gc Node>`) → Task 0.3/0.4. ✓
- §4.3 `set_location` + the AST `debug_loc` change → Task 0.1 + 0.3. ✓
- §4.4 `Option`-as-`Optional` error idiom → Task 0.3 (`error_*`/`need`/`eat`) + 0.4. ✓
- §4.5 `Param` value struct → Task 0.3. ✓ (generics N/A in P0; no `template` ported yet.)
- §4.6 recursion guard → Task 0.3. ✓
- §6 validation gate (`hermesc -dump-ast` oracle, `ast-dump` bin, `parser_differential`) → Tasks 0.5–0.6. ✓
- §7 P0 row → Task 0.7. ✓
- Out of P0 scope (correctly deferred to later phases): statement/expression parsing, dialects, lazy passes, the `JSParserTest.cpp` port.

**Placeholder scan:** No "TBD"/"handle errors"/"similar to". The few "confirm/adjust against the crate" notes are *named API-reconciliation steps* (exact module paths, `metadata()` accessor, flag spellings) with the concrete action to take — not deferred work. The lifetime note gives the exact fallback (second lifetime) if the single-`'gc` form is rejected.

**Type consistency:** `JSParserImpl<'gc>`, `Param`/`PARAM_*`, `GrammarContext::AllowRegExp`, `TokenKind::eof`, `token_kind_str`, `Program::new(metadata, body)`, `NodeMetadata::new`/`new_with_debug`/`debug_loc`, `dump_estree_json_with_sm(out, root, pretty, mode, sm, loc_mode, raw, atoms)`, `ESTreeDumpMode::{HideEmpty,DumpAll}`, `LocationDumpMode::{None,LocAndRange}`, `ESTreeRawProp::{Exclude,Include}`, `SourceErrorManager::{new,add_buffer_bytes,error_count,error}`, `JSLexer::{new,advance,token,get_source_mgr,get_source_mgr_mut}`, `Token::{kind,start_loc,source_range}`, `SMLoc{source,offset}`/`SMRange{start,end}` — all match the signatures gathered from the crates. The one accessor not yet confirmed to exist (`Node::metadata()`) is called out in Task 0.3 with the exact way to add it via `gen_nodes.py`.
