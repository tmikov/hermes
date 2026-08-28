# game-of-life — Wasm Game of Life Example

Conway's Game of Life running on a 128×128 toroidal grid, seeded with an
[R-pentomino](https://conwaylife.com/wiki/R-pentomino) at the center.

The R-pentomino is a classic "methuselah" — just 5 cells that evolve for
over 1000 generations before stabilizing. After 200 iterations, the live
cell count is **120**.

Both a C implementation (compiled to Wasm) and a structurally equivalent
pure JavaScript implementation are provided. This exercises Wasm linear
memory access (loads and stores), nested loops, and function calls.

## Files

| File | Description |
|------|-------------|
| `life.c` | C implementation |
| `life.js` | Pure JavaScript implementation (same algorithm) |
| `life.wasm` | Compiled WebAssembly binary (checked in for convenience) |
| `life.wat` | Disassembled WebAssembly text format (for reference) |
| `node-run.js` | Node.js driver — loads `.wasm` |
| `run.js` | Hermes driver — loads `.wasm` or pre-compiled `.hbc` via the WebAssembly JS API |

## Prerequisites

- **Hermes** built with `-DHERMES_ENABLE_WASM=ON`
- **Clang with wasm32 target** (only needed to recompile `life.c`)

On macOS, Apple's system clang does not support the `wasm32` target. Install
LLVM and LLD from Homebrew:

```bash
brew install llvm lld
```

## Step 1: Compile C to WebAssembly

Skip this step if using the pre-built `life.wasm`.

```bash
PATH="/opt/homebrew/opt/lld/bin:$PATH" \
/opt/homebrew/opt/llvm/bin/clang \
  --target=wasm32-unknown-unknown -nostdlib -O2 \
  -Wl,--no-entry -Wl,--export-all \
  -o life.wasm life.c
```

## Step 2: Run

### Wasm via Node.js

```bash
node node-run.js life.wasm
```

### Wasm via Hermes (runtime compilation)

```bash
hermes -Xhermes-internal-test-methods run.js -- life.wasm
```

### Wasm via Hermes (ahead-of-time compilation)

```bash
hermesc --wasm -emit-binary -out life.hbc life.wasm
hermes -Xhermes-internal-test-methods run.js -- life.hbc
```

### Pure JavaScript

```bash
node life.js
hermes life.js
```

All paths should print the same output:

```
120
```

## Adjusting the workload

The default is 200 iterations on a 128×128 grid. To change:

- **C version:** edit `WIDTH`, `HEIGHT` (compile-time constants) and the
  iteration count in `main_entry()`, then recompile.
- **JS version:** edit `WIDTH`, `HEIGHT`, and the argument to `run()` at the
  bottom of `life.js`.

## Disassembling

To regenerate `life.wat` from `life.wasm`:

```bash
wasm2wat life.wasm -o life.wat
```
