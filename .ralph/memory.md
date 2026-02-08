# Wasm Implementation Memory

## Wasm Implementation
- Plan at `doc/WasmImplementationPlan.md` (Phase 1 - Correct MVP)
- Design doc at `doc/WasmSupport.md`
- Progress tracked in `.ralph/progress.md`
- New dirs: lib/WasmFrontend/, lib/WasmIRGen/, lib/VM/JSLib/WebAssembly/
- Uses wabt BinaryReaderDelegate for parsing
- i64 via split i32 pairs (Phase 1); Phase 3 uses GC-excluded register slots
- Memory via JSArrayBuffer + typed array views (asm.js pattern)
- Phase 1 alignment: trust annotation at compile time (known spec limitation for misaligned + natural annotation; fixed in Phase 2 with raw pointer bytecodes)
- Gated behind `HERMES_ENABLE_WASM` CMake option (default OFF)
- wabt library target is `wabt`. Link via `LINK_LIBS wabt` (not LINK_OBJLIBS)
- Include wabt headers with `#include "wabt/binary-reader.h"`
- Must build with Clang (`-DCMAKE_C_COMPILER=clang-17 -DCMAKE_CXX_COMPILER=clang++-17`)

## CompilerDriver Integration
- `HERMES_ENABLE_WASM` is both a CMake variable and a C++ define (via `add_definitions`)
- CompilerDriver uses `#ifdef HERMES_ENABLE_WASM` to guard wasm-specific code
- `hermescompiler` static library conditionally includes wasm object libs + wabt
- Entry point: `compileWasmModule(buffer, size, Module&, errorMsg)` in `WasmCompile.h`
- Context default constructor: `Context()` — don't pass `CodeGenerationSettings` by value (needs rvalue ref `&&`)
- `-wasm` flag and `.wasm` extension auto-detection in CompilerDriver

## Lit Test Infrastructure
- wabt tools at `${CMAKE_BINARY_DIR}/external/wabt/wabt/{tool}` (not in `HERMES_TOOLS_OUTPUT_DIR`)
- `%wat2wasm` and `%wast2json` substitutions available in lit tests
- Wasm lit tests require `REQUIRES: wasm` and use `.wat` suffix
- `wasm` feature is set when `HERMES_ENABLE_WASM=ON`
- Test pattern: `%wat2wasm %s -o %t.wasm && %hermesc --wasm %t.wasm -o %t.hbc && %hermes %t.hbc | %FileCheck %s`
- Both `hermescompiler` (for `hermesc`) and `hermesvm_a` (for `hermes`) must link wasm libs

## Workflow
- Branch: `work`, PR target: `static_h`
- After completing a task: update `.ralph/progress.md` (status + context notes), then commit
- Always verify cmake configures cleanly after build system changes
- **Persist new findings to this file** (`.ralph/memory.md`) so future sessions can find them — e.g., gotchas, API quirks, patterns that worked or failed, build issues and their fixes
