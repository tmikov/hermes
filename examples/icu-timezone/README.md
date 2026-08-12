# ICU4X Timezone Demo (Wasm frontend)

Runs a real-world Wasm module — ICU4X's `icu_capi` (6.4 MB) — through the
Hermes Wasm frontend: the module is AOT-compiled to bytecode with
`hermesc --wasm`, and a ported version of the demo's JS bundle instantiates
it and performs timezone conversions and formatting.

> **Status on this branch: the demo compiles but does not yet run to
> completion.** `run.sh` reaches the first string conversion and stops with
> `RangeError: byteOffset + length * elementSize must be less than
> buffer.byteLength`. The cause is that an exported `WebAssembly.Memory` here
> is a separate ArrayBuffer rather than a view onto the module's own linear
> memory: the exported object stays at the declared 80 pages while the
> module's real memory grows past it, so `diplomat_alloc` returns 5,242,888
> against a 5,242,880-byte exported buffer. Aliasing the two is scheduled
> work; until it lands, treat this example as the acceptance test for it
> rather than as a passing demo. The compile half — the slow, memory-hungry
> half — does work and is worth running on its own.

This lives in `examples/` rather than `test/` deliberately: compiling the
module takes ~10 s at ~0.46 GB RSS with a **Release** `hermesc`, producing a
13.3 MB `.hbc`. Under the default ASan build it is far slower — too slow for
the regular test suite. It is a manually-run, end-to-end smoke test for the Wasm frontend on
a large real module: 348 exports, a 5 MB data segment, a 5-entry function
table, and a linear memory that grows during the run. It exercises memory
exports and `memory.grow` with a live exported reference — which is exactly
what it currently catches, see the note above — plus the
`FinalizationRegistry` and `TextDecoder`/`TextEncoder` host APIs.

Note the module declares **no imports** — it has no import section at all —
so it gives no coverage of import trampolines, and the import object the
ported loader installs goes unused.

## Files

- `timezone-demo.bundle.mjs` — the original demo: ICU4X JS bindings
  (diplomat-generated) plus a small driver that converts a few fixed dates
  between timezones. A prebuilt Node ESM bundle, checked in as-is.
- `icu_capi.wasm` — ICU4X 2.1.1's C API compiled to Wasm, which the bundle
  drives. Checked in as-is; it is not built from this repo.
- `LICENSE-ICU4X`, `LICENSE-diplomat`, `THIRD-PARTY-NOTICES.md` — upstream
  licenses and provenance for the two prebuilt files above. Read
  `THIRD-PARTY-NOTICES.md` before updating or replacing them.
- `port.py` — mechanically ports the bundle to run under the Hermes CLI:
  strips the Node imports, and replaces the `readFileSync` +
  `WebAssembly.Instance` loader with the AOT path
  (`hermescli.loadHBC(hermescli.loadFile(<hbc path>))`, imports passed via
  the `__wasm_imports__` global). Everything else in the bundle runs
  unmodified.
- `expected.txt` — the demo's expected stdout (deterministic: fixed input
  dates, fixed timezone list).
- `run.sh` — ports, compiles (cached), runs, diffs against `expected.txt`.

Generated (git-ignored): `timezone-demo.js`, `icu_capi.hbc` (~104 MB),
`out.txt`.

## Running

Build Release `hermesc` and `hermes` first — use Release unless you
specifically want to soak the compiler under ASan (see timings above):

```shell
cmake -B cmake-build-release -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build cmake-build-release --target hermesc hermes
```

Then:

```shell
./run.sh                                  # or: BUILD=../../cmake-build-asan ./run.sh
```

`run.sh` exits 0 and prints `OK` when the output matches `expected.txt`.
A stderr warning about `console` not being declared is expected and
harmless (the check only diffs stdout).

Every run recompiles the module. That is intentional: the `.wasm` never
changes, so the only input that moves is `hermesc`, and skipping the compile
would skip the one step this example exists to exercise. If you are
iterating on the JS side and want to reuse an existing `icu_capi.hbc`, run
the manual steps below instead.

## Third-party content

`icu_capi.wasm` and `timezone-demo.bundle.mjs` are prebuilt third-party
artifacts, not Hermes code. ICU4X is under the Unicode License V3 and the
bundled diplomat runtime is `Apache-2.0 OR MIT`. See
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

## Manual steps (what run.sh does)

```shell
python3 port.py timezone-demo.bundle.mjs timezone-demo.js
hermesc --wasm -emit-binary -out icu_capi.hbc icu_capi.wasm     # slow
hermes -Xhermes-internal-test-methods timezone-demo.js -- icu_capi.hbc
```

`-Xhermes-internal-test-methods` gates the `hermescli.*` helpers the ported
loader uses; the `.hbc` path is passed as the script argument.
