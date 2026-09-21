<!--
Copyright (c) Meta Platforms, Inc. and affiliates.

This source code is licensed under the MIT license found in the
LICENSE file in the root directory of this source tree.
-->

# Wasm test fixtures

## rust-imports.wasm

`rust-imports.wasm` is a prebuilt, checked-in artifact compiled from
`rust-imports.rs`, a `no_std` Rust module targeting
`wasm32-unknown-unknown`. It imports its linear memory and two host
functions (`env.host_add`, `env.host_log`), so it exercises the import
path against output that a real compiler actually emits, rather than
hand-written WAT.

The `.wasm` is checked in **deliberately** so that `test/wasm/e2e-rust-imports.js`
does not depend on a Rust toolchain, or on the `wasm32-unknown-unknown`
target, being available in the environment that runs the test suite.

### Regenerating

Built with:

```
rustc 1.96.0 (ac68faa20 2026-05-25)
```

```bash
rustup target add wasm32-unknown-unknown
rustc --target wasm32-unknown-unknown --crate-type cdylib \
  -C opt-level=z -C panic=abort \
  -C link-arg=--import-memory -C link-arg=--strip-all \
  -o test/wasm/fixtures/rust-imports.wasm test/wasm/fixtures/rust-imports.rs
```

This produces a 212-byte module that:

- imports `env.memory`, a memory of at least 17 pages (the linker places
  the `GREETING` data segment at byte offset 1048576, i.e. the start of
  page 16, so pages 0-16 — 17 pages — must exist);
- imports `env.host_add(i32, i32) -> i32` and `env.host_log(ptr: i32, len: i32)`;
- exports `run(i32) -> i32`, which calls `host_add(x, 1)`;
- exports `greet()`, which calls `host_log` with the address and length of
  the string `"hello from rust"`, round-tripping it through the imported
  memory.

Inspect the module's data segment offset with `wasm2wat` or `wasm-objdump`
if the fixture is ever regenerated with a different Rust/LLVM version,
since a toolchain change could shift the offset (and therefore the
minimum memory size) without changing this file.
