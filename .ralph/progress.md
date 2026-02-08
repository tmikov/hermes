# Wasm Implementation Progress

Tracks progress on `doc/WasmImplementationPlan.md` (Phase 1 - Correct MVP).

## Status

| Step | Description | Status | Date |
|------|-------------|--------|------|
| A.1 | Create directory structure | done | 2026-02-08 |
| A.2 | Integrate wabt as external dependency | done | 2026-02-08 |
| A.3 | CMakeLists.txt for lib/WasmFrontend | done | 2026-02-08 |
| A.4 | CMakeLists.txt for lib/WasmIRGen | done | 2026-02-08 |
| A.5 | Unit test CMakeLists.txt files | done | 2026-02-09 |
| A.6 | Wire hermesc to accept .wasm files | done | 2026-02-09 |
| A.7 | Build wabt test tools (wat2wasm, wast2json) | done | 2026-02-09 |
| B.1 | Define WasmValType and basic types | pending | |
| B.2 | Define WasmImport, WasmExport, WasmFunction | pending | |
| B.3 | Define WasmElemSegment, WasmDataSegment, WasmNameSection | pending | |
| B.4 | Define WasmModuleInfo | pending | |
| C.1 | BinaryReaderHermesIRGen — module-level callbacks | pending | |
| C.2 | compileWasmModule entry point (skeleton) | pending | |
| D.1 | WasmIRGen class skeleton | pending | |
| D.2 | Constants and locals | pending | |
| D.3 | Simple i32 arithmetic | pending | |
| D.4 | i32 comparison operations | pending | |
| D.5 | Return instruction | pending | |
| D.6 | Block and br/br_if | pending | |
| D.7 | Loop | pending | |
| D.8 | If/else | pending | |
| D.9 | br_table (switch) | pending | |
| D.10 | select | pending | |
| D.11 | unreachable and nop | pending | |
| D.12 | Function calls (call) | pending | |
| D.13 | Wire BinaryReaderHermesIRGen to WasmIRGen | pending | |
| D.14 | Integration — compile .wasm to .hbc and run | pending | |
| E.1 | f64 arithmetic | pending | |
| E.2 | f32 arithmetic | pending | |
| E.3 | f64/f32 comparison returning i32 | pending | |
| F.1 | Create WasmHelpers infrastructure | pending | |
| F.2 | i32 trapping division helpers | pending | |
| F.3 | i32 bit manipulation helpers | pending | |
| F.4 | Conversion helpers | pending | |
| F.5 | f64/f32 copysign, min, max helpers | pending | |
| G.1 | Define i64 representation | pending | |
| G.2 | i64 constants | pending | |
| G.3 | i64 arithmetic helpers | pending | |
| G.4 | i64 conversion helpers | pending | |
| G.5 | i64 locals and control flow | pending | |
| H.1 | Memory access helpers (load/store) | pending | |
| H.2 | memory.size and memory.grow | pending | |
| H.3 | Unaligned access handling | pending | |
| I.1 | Export wrapper functions | pending | |
| I.2 | Import trampoline functions | pending | |
| J.1 | Table representation | pending | |
| J.2 | call_indirect | pending | |
| K.1 | Wasm globals | pending | |
| L.1 | Wasm try/catch/throw | pending | |
| M.1 | WebAssembly error types | pending | |
| M.2 | WebAssembly.validate | pending | |
| M.3 | WebAssembly.Module | pending | |
| M.4 | WebAssembly.Instance | pending | |
| M.5 | WebAssembly.Memory | pending | |
| M.6 | WebAssembly.Table | pending | |
| M.7 | WebAssembly.Global | pending | |
| M.8 | WebAssembly.compile and WebAssembly.instantiate | pending | |
| M.9 | WebAssembly.Exception and WebAssembly.Tag | pending | |
| N.1 | memory.fill, memory.copy, memory.init, data.drop | pending | |
| N.2 | table.fill, table.copy, table.init, elem.drop | pending | |
| O.1 | Set up Wasm spec test runner | pending | |
| O.2 | Progressively pass spec tests | pending | |

## Context Notes

Notes are added here as steps are completed. Each entry records decisions
made, problems encountered, files created/modified, and anything the next
step needs to know.

### A.1: Create directory structure
- **Files**: Created directories: `include/hermes/WasmFrontend/`, `include/hermes/WasmIRGen/`, `lib/WasmFrontend/`, `lib/WasmIRGen/`, `lib/VM/JSLib/WebAssembly/`, `test/wasm/`, `unittests/WasmFrontend/`, `unittests/WasmIRGen/`. Created empty `CMakeLists.txt` in each `lib/` and `unittests/` subdirectory. Modified `CMakeLists.txt` (top-level), `lib/CMakeLists.txt`, `unittests/CMakeLists.txt`.
- **Decisions**: `HERMES_ENABLE_WASM` option added (default OFF), gating `add_subdirectory` calls for WasmFrontend and WasmIRGen in both lib/ and unittests/.
- **Issues**: None.
- **Notes for next step**: A.2 (wabt integration) and A.4 (WasmIRGen CMake) can proceed. Build directories exist and configure cleanly with WASM on or off.

### A.2: Integrate wabt as external dependency
- **Files**: `external/wabt/CMakeLists.txt` (wrapper), `external/wabt/wabt/` (vendored source). Modified `CMakeLists.txt` (top-level) to `add_subdirectory(external/wabt)` before `add_subdirectory(lib)`.
- **Decisions**: Vendored wabt 1.0.39 into `external/wabt/wabt/` (following asmjit pattern). Stripped `.git/`, `test/`, `docs/`, `fuzz-in/`, and unnecessary `third_party/` subdirs (gtest, simde, testsuite, ply, uvwasi, wasm-c-api). Final size ~3MB. Wrapper sets `BUILD_TESTS=OFF`, `BUILD_LIBWASM=OFF`, `USE_INTERNAL_SHA256=ON` and adds with `EXCLUDE_FROM_ALL`.
- **Issues**: None. wabt compiles cleanly with Hermes's global CMAKE_CXX_FLAGS. `-fno-exceptions`/`-fno-rtti` are only applied per-target via `hermes_update_cxx_flags`, not globally, so wabt's `WITH_EXCEPTIONS=OFF` default works fine.
- **Notes for next step**: wabt library target is `wabt`. Link via `LINK_LIBS wabt` (not LINK_OBJLIBS). Include headers with `#include "wabt/binary-reader.h"`. Tools (wat2wasm, wast2json) build via wabt's CMake `BUILD_TOOLS=ON` (default). Must build with Clang (`-DCMAKE_C_COMPILER=clang-17 -DCMAKE_CXX_COMPILER=clang++-17`).

### A.3: CMakeLists.txt for lib/WasmFrontend
- **Files**: `lib/WasmFrontend/CMakeLists.txt`, `lib/WasmFrontend/WasmCompile.cpp`, `lib/WasmFrontend/BinaryReaderHermesIRGen.cpp` (placeholders).
- **Decisions**: Used `LINK_LIBS wabt` for the external wabt library. Used `LINK_OBJLIBS hermesWasmIRGen hermesFrontend hermesSupport` for internal deps. Placeholder .cpp files include wabt headers to verify integration.
- **Issues**: None.
- **Notes for next step**: Both placeholder files compile and link against wabt. The `hermesWasmFrontend` target builds successfully.

### A.4: CMakeLists.txt for lib/WasmIRGen
- **Files**: `lib/WasmIRGen/CMakeLists.txt`, `lib/WasmIRGen/WasmIRGen.cpp`, `lib/WasmIRGen/WasmHelpers.cpp` (placeholders).
- **Decisions**: `LINK_OBJLIBS hermesFrontend hermesSupport`. No IR-specific library — IR is part of `hermesFrontend`.
- **Issues**: None.
- **Notes for next step**: The `hermesWasmIRGen` target builds successfully.

### A.5: Unit test CMakeLists.txt files
- **Files**: Created `unittests/WasmFrontend/CMakeLists.txt`, `unittests/WasmFrontend/WasmCompileTest.cpp`, `unittests/WasmIRGen/CMakeLists.txt`, `unittests/WasmIRGen/WasmIRGenTest.cpp`.
- **Decisions**: Followed the pattern from `unittests/IR/CMakeLists.txt`. `WasmFrontendTest` links `hermesWasmFrontend` and `wabt`. `WasmIRGenTest` links `hermesWasmIRGen`. Test names match the target names expected by the plan (`WasmFrontendTest`, `WasmIRGenTest`). Placeholder tests include a simple `EXPECT_TRUE(true)` assertion. WasmCompileTest.cpp includes `wabt/binary-reader.h` to verify wabt integration in the test binary.
- **Issues**: None.
- **Notes for next step**: Both test targets build and pass. Future steps (B.x, C.x, D.x) will add real tests to these files.

### A.6: Wire hermesc to accept .wasm files
- **Files**: Created `include/hermes/WasmFrontend/WasmCompile.h`. Modified `lib/WasmFrontend/WasmCompile.cpp` (stub implementation). Modified `CMakeLists.txt` (added `add_definitions(-DHERMES_ENABLE_WASM)`). Modified `lib/CMakeLists.txt` (conditionally link `hermesWasmFrontend_obj`, `hermesWasmIRGen_obj`, `wabt` into `hermescompiler`). Modified `lib/CompilerDriver/CompilerDriver.cpp` (added `-wasm` flag, `.wasm` extension auto-detection, `processWasmFile()` function, flag validation).
- **Decisions**: Used `#ifdef HERMES_ENABLE_WASM` guards in CompilerDriver.cpp so that wasm support is compile-time optional. Added `add_definitions(-DHERMES_ENABLE_WASM)` to top-level CMake (following `HERMES_ENABLE_INTL` pattern). Auto-detects `.wasm` extension or accepts explicit `-wasm` flag. The wasm libs are conditionally linked into the `hermescompiler` static library.
- **Issues**: Initial attempt used wrong `Context` constructor signature (`CodeGenerationSettings` must be rvalue ref). Fixed by using the default constructor `Context()`.
- **Notes for next step**: `hermesc -emit-binary foo.wasm` and `hermesc -wasm -emit-binary foo.bin` both invoke the wasm pipeline and currently output "Error: Wasm compilation not yet implemented". C.2 will replace the stub with actual parsing. The `compileWasmModule` function signature is: `bool compileWasmModule(const uint8_t *buffer, size_t size, Module &M, std::string &errorMsg)`.

### A.7: Build wabt test tools (wat2wasm, wast2json)
- **Files**: Modified `CMakeLists.txt` (added `wat2wasm`/`wast2json` to `HERMES_TEST_DEPS`, added `wasm_enabled`/`wat2wasm`/`wast2json` to `HERMES_LIT_TEST_PARAMS_BASE`). Modified `lib/CMakeLists.txt` (added wasm libs to `hermesvm_a` for the `hermes` runtime binary). Modified `test/lit.cfg` (added `wasm` feature, `.wat` suffix, `%wat2wasm`/`%wast2json` substitutions). Created `test/wasm/wat2wasm-basic.wat` (basic lit test).
- **Decisions**: wabt tools are built from wabt's own CMake (`BUILD_TOOLS=ON` default). Executables are at `${CMAKE_BINARY_DIR}/external/wabt/wabt/{tool}` — passed directly as lit params rather than copying to `HERMES_TOOLS_OUTPUT_DIR`. Added `REQUIRES: wasm` feature so wasm tests are skipped when `HERMES_ENABLE_WASM=OFF`. Added `.wat` as a lit test suffix gated on `wasm_enabled`. Also fixed a bug from A.6: the `hermes` binary (not just `hermesc`) was missing wasm lib linkage — added wasm object libs to `hermesvm_a`.
- **Issues**: `hermes` binary failed to link with undefined `compileWasmModule` — `hermesvm_a` (used by `hermes`) didn't include wasm libs. Fixed by adding the same conditional linkage that `hermescompiler` (used by `hermesc`) has.
- **Notes for next step**: `%wat2wasm` and `%wast2json` are available in lit tests with `REQUIRES: wasm`. Typical test pattern: `%wat2wasm %s -o %t.wasm && %hermesc --wasm %t.wasm -o %t.hbc && %hermes %t.hbc | %FileCheck %s`. The `wat2wasm` executable reads from stdin with `-` arg.

<!--
Template for completed steps:

### X.N: Step name
- **Files**: list of files created or modified
- **Decisions**: any design choices made during implementation
- **Issues**: problems encountered and how they were resolved
- **Notes for next step**: anything the following step needs to know
-->
