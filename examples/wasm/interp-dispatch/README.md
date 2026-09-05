# interp-dispatch — Wasm Benchmark Example

A simple benchmark compiled from C to WebAssembly and run by Hermes.

The benchmark computes `100!` (as a floating-point product) in a tight loop
and sums the results — a direct translation of the JavaScript benchmark in
`tools/hvm-bench/interp-dispatch.js`.

## Files

| File | Description |
|------|-------------|
| `bench.c` | C source — the benchmark function and a `main` entry point |
| `bench.wasm` | Compiled WebAssembly binary (checked in for convenience) |
| `bench.wat` | Disassembled WebAssembly text format (for reference) |
| `run.js` | Hermes driver — loads `.wasm` or pre-compiled `.hbc` via the WebAssembly JS API |
| `node-run.js` | Node.js driver — loads `.wasm` for comparison / validation |

## Prerequisites

- **Hermes** built with `-DHERMES_ENABLE_WASM=ON`
- **Clang with wasm32 target** (only needed to recompile `bench.c`)

On macOS, Apple's system clang does not support the `wasm32` target. Install
LLVM and LLD from Homebrew:

```bash
brew install llvm lld
```

## Step 1: Compile C to WebAssembly

Skip this step if using the pre-built `bench.wasm`.

```bash
PATH="/opt/homebrew/opt/lld/bin:$PATH" \
/opt/homebrew/opt/llvm/bin/clang \
  --target=wasm32-unknown-unknown -nostdlib -O2 \
  -Wl,--no-entry -Wl,--export-all \
  -o bench.wasm bench.c
```

## Step 2: Run

### Option A: Load .wasm at runtime (WebAssembly JS API)

Hermes compiles the Wasm binary to bytecode at runtime using
`WebAssembly.Module` and `WebAssembly.Instance`, then executes it.

```bash
hermes -Xhermes-internal-test-methods run.js -- bench.wasm
```

### Option B: Ahead-of-time compilation to .hbc

First compile the `.wasm` to Hermes bytecode:

```bash
hermesc --wasm -emit-binary -out bench.hbc bench.wasm
```

Then run the bytecode (same `run.js` — it accepts both formats):

```bash
hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js run.js -- bench.hbc
```

### Option C: Verify with Node.js

```bash
node node-run.js bench.wasm
```

All three options should produce the same output:

```
3.733048617757523e+161
```

## Adjusting the workload

In `bench.c`, the `main` entry point calls `bench(4000, 100)`. The original
JavaScript benchmark uses `bench(4000000, 100)` — change the first argument
to increase the iteration count for longer runs.

## Disassembling

To regenerate `bench.wat` from `bench.wasm` (requires `wasm2wat` from wabt):

```bash
wasm2wat bench.wasm -o bench.wat
```

If building Hermes with `-DHERMES_ENABLE_WASM=ON`, `wasm2wat` can be built
from the vendored wabt:

```bash
cmake --build cmake-build-debug --target wasm2wat
cmake-build-debug/external/wabt/wabt/wasm2wat bench.wasm -o bench.wat
```
