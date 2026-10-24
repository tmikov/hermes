# Intl Plugin Phase 0 (Infrastructure) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Intl plugin's infrastructure end to end. A Rust core on ICU4X and a C++ JSI layer are built into a separate library, `libhermesintl`. Hermes finds and loads that library when it creates a runtime. The library installs an `Intl` object with `getCanonicalLocales` and `supportedValuesOf`. All of it is tested by cargo, gtest, lit and test262 through a new `check-intl` target.

**Architecture:** Hermes gains three things:
- a loader in `API/hermes`, used by both runtime factories;
- a small public interface header;
- one JSI interface, `IHermesBuiltinBytecode`.

The plugin lives in `intl/`, in three parts:
- a Rust crate (`intl/core`) with a cbindgen-generated C header;
- C++ that turns that C interface into JSI host functions (`intl/jsi`);
- the ECMA-402 code in JS (`intl/js`), compiled to bytecode by this build's `hermesc` and embedded in the library.

The plugin never includes Hermes internals. It binds to JSI's compiled code in the host process when it is loaded.

**Tech Stack:** C++17, JSI, Rust ≥ 1.88 with ICU4X 2.3.1 (`icu` crate, `compiled_data`), cbindgen 0.29 (as a build dependency), CMake 3.21 + Ninja, Hermes bytecode, Python 3 (lit, `utils/test_runner.py`), gtest (llvh).

**Spec:** `doc/superpowers/specs/2026-10-22-intl-plugin-design.md`. Read it before starting any task. Task 12 updates it with the refinements listed below.

## Global Constraints

- Work only in the worktree `/home/tmikov/work/hermes-intl`. **Never `cd`**; pass absolute paths. Use a subshell `( cd x; cmd )` only when a tool insists.
- **Build directory:** `/home/tmikov/work/hermes-intl/cmake-build-asan`. Configure it as follows (Clang, ASan, `JSI_UNSTABLE=ON` so Workers are built):
  ```
  cmake -S /home/tmikov/work/hermes-intl -B /home/tmikov/work/hermes-intl/cmake-build-asan -G Ninja \
    -DCMAKE_BUILD_TYPE=Debug -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ \
    -DHERMES_ENABLE_ADDRESS_SANITIZER=ON -DCMAKE_CXX_FLAGS=-O1 -DCMAKE_C_FLAGS=-O1 \
    -DJSI_UNSTABLE=ON
  ```
  Task 11 adds `-DHERMES_INTL_TEST262_DIR=...` to this.
- **Git:** never commit, amend or reset. At the end of each task, stage that task's files with `git -C /home/tmikov/work/hermes-intl add <files>`. Then `git diff` shows only the next task's work, and `git diff --cached` shows everything before it. The user makes the commit.
- **Copyright header:** every new C++, JS, CMake and Python file gets the Meta MIT header used across the repo (CLAUDE.md "Copyright Header"; `#` comments for CMake and Python). Files that adapt magrinj/hermes-intl code also carry the line `Portions adapted from magrinj/hermes-intl, Copyright (c) 2026 Jérémy Magrin, MIT License; see intl/THIRD_PARTY_NOTICES.md.`
- **C++ style:** CLAUDE.md conventions: 80 columns, 2-space indent, a doc comment on every declaration, `camelCase` functions, `trailing_` members. Before touching anything in `API/hermes/` or `lib/VM/`, invoke the `gc-safe-coding` skill.
- **Separability:** code under `intl/` includes only `<jsi/...>`, `<hermes/Public/IntlPlugin.h>`, its own headers and the generated `hermes_intl_core.h`. It includes nothing from `include/hermes/` or `lib/`. The one exception is `intl/test/unit/`, which may link Hermes. CMake passes the bytecode version in as `HERMES_INTL_BYTECODE_VERSION`.
- **Plugin binary contract (Phase 0):**
  - On Linux and macOS the plugin links no JSI provider. Its JSI references stay undefined (`-undefined dynamic_lookup` on macOS) and bind, when the library is loaded, to the JSI code the host already exports.
  - Every Hermes tool executable exports its dynamic symbols (Task 4).
  - The plugin is compiled with RTTI and exceptions, and with the same `JSI_UNSTABLE` setting as Hermes, because that setting changes the layout of `jsi::Runtime`.
- **Plugin JS rules** (spec "Resistance to tampering"). Every file in `intl/js` follows them, and reviewers check them:
  1. Call only intrinsics captured in `01-intrinsics.js`. Never call a property of a user-reachable object except where the spec does a `Get`/`Call` on user values.
  2. No syntax that consults user-modifiable state: no spread, no `for-of`, no array destructuring or array patterns, no tagged templates, no `Array.prototype` methods, no `String.prototype.split` (it looks up `@@split`).
  3. Append to arrays with `listPush`, never `a[i] = v`, which runs setters on `Array.prototype`. Build every property descriptor and internal record with `__proto__: null`.
  4. `ToString(v)` is `` `${v}` ``, `ToNumber(v)` is `+v`, and `ToObject` throws on `null`/`undefined` and then calls the captured `Object`.
  5. Every function cites the spec section and numbers its steps in comments. The file header names the ECMA-402 draft it follows: the https://tc39.es/ecma402/ draft dated on the day the file is written. If the draft's step text differs from this plan's code, follow the draft and say so in the task report.
  6. Code between `00-header.js` and `99-footer.js` is written at column 0, so lit tests can concatenate the files outside the IIFE.
- **Rust:**
  - Edition 2021 with `rust-version = "1.88"`, the MSRV of `icu` 2.3.1.
  - `Cargo.lock` is committed, and every cargo invocation from CMake uses `--locked`.
  - `panic = "abort"` in every profile.
  - FFI functions never panic on bad input: invalid UTF-8 or an unparsable tag returns `false`. A panic is only for a broken invariant in our own data files.
  - Rust types later held in `NativeState` must be `Send`. Phase 0 has none; Phase 2 adds the compile-time assertion.
- **Platforms (Phase 0):** the plugin builds for the host on Linux and macOS (a single architecture). For cross builds (Android, iOS, other Apple targets, Emscripten), Windows and multi-architecture macOS, `HERMES_BUILD_INTL` defaults to `OFF`, and setting it `ON` there is a configure error until Phase 6. The loader builds everywhere; on Windows and Emscripten it supports only registered plugins until Phase 6.
- **Prove checks can fail:** every new test is shown to fail. Make the stated mutation, name the test that fails, then revert the mutation. A green run only proves the suite ran.
- **Never bump `BYTECODE_VERSION`.**

## Review Focus

These are the five inputs most likely to hurt someone that the spec implies but no feature test would naturally cover. Each one has a test in the task named.

1. **Odd host locale strings:**
   - codesets and modifiers (`sr_RS.UTF-8@latin`, `ca_ES@valencia`, `de_DE@euro`, `C.UTF-8`, `en_US.ISO-8859-1`);
   - empty values and junk;
   - deprecated codes (`iw_IL`);
   - a tag that already has a script.

   Detection must normalize them as the spec says, or skip to the next source. It must never crash, and never pick the wrong script. Covered in Task 6 (`host.rs` tests) and Task 8 (`DefaultsTest`).
2. **Odd `TZ` and `/etc/localtime` forms:**
   - `:Europe/Paris`, absolute and relative zoneinfo paths, macOS's `/var/db/timezone/zoneinfo/...`;
   - POSIX rule strings, empty `TZ`;
   - `UTC`/`Etc/UTC`/`GMT`, wrong case, deprecated names (`Asia/Calcutta`);
   - an `/etc/localtime` that is a copy rather than a symlink.

   Each gives the spec's result. Covered in Task 6 and Task 8.
3. **User code that tampers with intrinsics before calling `Intl`:**
   - accessors on `Object.prototype` named `value`/`get`/`set`/`writable`;
   - setters for `"0"` on `Array.prototype`;
   - a replaced `Array.prototype[Symbol.iterator]` or `String.prototype[Symbol.split]`.

   `getCanonicalLocales` and `supportedValuesOf` return the same results regardless. Covered in Task 10 (`tamper.js`).
4. **Runtimes created at once on several threads**, when the default plugin has not been resolved yet. It is resolved exactly once, every runtime gets it, and ASan reports nothing. Covered in Task 3 (`ConcurrentRuntimesShareOneResolution`).
5. **Strange arguments to `getCanonicalLocales`:**
   - lone surrogates;
   - a 10,000-character tag;
   - array-likes with a huge or throwing `length`;
   - a symbol element, a `null` element, holes.

   They give the spec's `TypeError`/`RangeError` or skip the holes, and never crash. Covered in Task 10 (`getCanonicalLocales-edge.js`).

## Planning decisions that refine the spec

Writing this plan against the code surfaced gaps in the spec. Task 12 writes these decisions into the spec. Implement them as stated here.

1. **The `hermes` CLI links the VM statically**, so there is no `libhermesvm` to link the plugin against. The plugin therefore links no JSI provider on Linux or macOS. Its JSI symbols bind when it is loaded:
   - in executables, to the copy the executable exports. Every Hermes tool gets `-rdynamic` / `-Wl,-export_dynamic` in `add_hermes_tool`; the `hermes` binary already exports 1131 `facebook::jsi::` symbols today because of `HERMES_ENABLE_NAPI`;
   - in a shared-library host such as `hermes-jsi-shared`, to `libhermesvm`'s copy.

   This is the pattern the Node-API test addons already use. Explicit linking against a provider (iOS framework, `libjsi.so` on Android, a Windows import library) comes in Phase 6.
2. **Default resolution, step 2,** also tries `../lib/<name>` when the Hermes code lives in a directory named `bin` (the build and install layout: `bin/hermes`, `lib/libhermesintl.so`).
   - On Linux, the executable's path comes from `/proc/self/exe` when `dladdr` returns a bare name.
   - A library found by **search path** counts as "not found" only when the loader's error message says the library itself was not found. A missing dependency, a wrong architecture or unresolved symbols are all "found but broken".
3. **The entry struct gains two fields:**
   - `flags`, whose bit 0 is `JSI_UNSTABLE`. The loader rejects a mismatch, because it changes the layout of `jsi::Runtime`.
   - `resolve_defaults(hermes_intl_config *) -> std::string`. It canonicalizes the locales and time zone `RuntimeConfig` requested and detects the missing ones. It returns an error message for an invalid explicit value, which the loader makes fatal.

   This is how the loader learns the effective defaults that it records for Workers. `hermes_intl_config` holds `std::vector<std::string>` and `std::string`; the same-build contract already requires a shared C++ library.
4. **The per-runtime record lives in JSI runtime data** (`setRuntimeData` under a private UUID). `Worker.cpp` reads it through `makeWorkerRuntimeConfig(parent)` and needs no Hermes internals.
5. **Native internal errors** are C++ exceptions. Hermes turns a C++ exception thrown from a host function into an `Error` made from its own intrinsic (`API/hermes/hermes.cpp:1132`), which user code cannot intercept. This replaces "`RangeError` or `TypeError` from intrinsics captured at install", which would have meant keeping JS values in native code.
6. **Phase 0 builds for the host only.** It covers Linux and single-architecture macOS, as described under Global Constraints. Cross builds, universal macOS and Windows move to Phase 6.
7. **The test262 pin** is the commit CI already uses, `c9aa6126e8f6fe2c6f668c65bfba771ffc8a6096` (`.github/workflows/build.yml`).

## File map

Hermes side:

| File | Responsibility |
|---|---|
| `public/hermes/Public/IntlPlugin.h` (new) | The plugin interface: `hermes_intl_config`, `hermes_intl_entry`, version and flag constants |
| `API/jsi/jsi/hermes-interfaces.h` | Adds `IHermesBuiltinBytecode` |
| `API/hermes/hermes.cpp` | Implements `IHermesBuiltinBytecode`; both factories call `installIntlPlugin` |
| `API/hermes/hermes.h` | Declares `registerIntlPlugin` |
| `API/hermes/IntlPluginLoader.h`, `.cpp` (new) | Loader: registration, path cache, default resolution, version checks, error table, per-runtime record, Worker config |
| `API/hermes/extensions/Worker.cpp` | Workers get `makeWorkerRuntimeConfig(parent)` |
| `public/hermes/Public/RuntimeConfig.h` | Adds `EnableIntlPlugin`, `IntlPluginPath`, `IntlDefaultLocales`, `IntlDefaultTimeZone` |
| `include/hermes/VM/RuntimeFlags.h`, `tools/hermes/hermes.cpp`, `tools/hvm/hvm.cpp` | CLI flags `-Xintl-plugin`, `-Xintl-plugin-path`, `-Xintl-default-locale`, `-Xintl-default-time-zone` |
| `cmake/modules/Hermes.cmake` | `add_hermes_tool` exports dynamic symbols |
| `CMakeLists.txt` | `HERMES_BUILD_INTL`, `add_subdirectory(intl)`, lit params for fixtures |
| `unittests/API/BuiltinBytecodeTest.cpp` (new) | Tests for `IHermesBuiltinBytecode` |
| `unittests/IntlLoader/` (new) | `IntlLoaderTests` (registered plugins, in-process) and the fake plugin libraries used by lit |
| `test/hermes/intl-plugin/` (new) | Lit tests for the loader's paths, error table and Workers, using the fake plugins |
| `test/lit.cfg` | `%hermes_path`, `%intl_fixture_dir`, `%intl_library_name`, feature `intl_plugin_fixtures` |
| `utils/testsuite/*.py` | test262 plugin mode |

Plugin side:

| File | Responsibility |
|---|---|
| `intl/CMakeLists.txt` | Rust toolchain check, cargo build, bytecode, `hermesintl` library, `check-intl` |
| `intl/core/` | Rust crate `hermes_intl_core`: `lib.rs` (C interface), `locale.rs`, `host.rs`, `supported.rs`, `data/` |
| `intl/js/` | ECMA-402 in JS: `00-header.js`, `01-intrinsics.js`, `02-shapes.js`, `03-locales.js`, `04-options.js`, `10-Intl.js`, `99-footer.js`, and `CMakeLists.txt` for the bytecode |
| `intl/jsi/` | `Plugin.cpp` (entry point, install), `IntlBytecode.cpp`, `NativeHelpers.def/.h/.cpp` (X-macro table), `LocaleHelpers.cpp`, `RustCore.h`, `Defaults.h/.cpp` (detection, `resolveDefaults`) |
| `intl/test/lit/` | Lit tests of the real plugin and of the JS shared operations in isolation |
| `intl/test/unit/` | `HermesIntlUnitTests`: gtests for detection and `resolveDefaults` |
| `intl/test/test262-expectations.txt` | Expected test262 failures, with reasons |
| `intl/README.md`, `intl/THIRD_PARTY_NOTICES.md` | Docs and the magrinj MIT notice |

---
### Task 1: Rust core crate and `HERMES_BUILD_INTL`

**Files:**
- Create: `intl/CMakeLists.txt`, `intl/core/Cargo.toml`, `intl/core/Cargo.lock` (generated), `intl/core/build.rs`, `intl/core/cbindgen.toml`, `intl/core/src/lib.rs`
- Modify: `CMakeLists.txt` (option near `HERMES_ENABLE_CORE_EXTENSIONS`, about line 248; `add_subdirectory(intl)` after the JSI `add_subdirectory` block, about line 720)

**Interfaces:**
- Produces:
  - CMake imported target `hermesIntlCore`: the static library, with the include directory holding `hermes_intl_core.h`.
  - CMake variables `HERMES_INTL_CARGO`, `HERMES_INTL_CORE_DIR`, `HERMES_INTL_CARGO_ENV` (a `cmake -E env ...` command prefix) and `HERMES_INTL_CARGO_PROFILE_ARGS`.
  - C: `struct HiString { uint8_t *data; size_t len; size_t capacity; }`, `void hi_string_free(HiString)`, `const char *hi_core_version(void)`.

- [ ] **Step 1: Write the crate with its first test**

`intl/core/Cargo.toml`:
```toml
# (Meta MIT header as # comments)
[package]
name = "hermes_intl_core"
version = "0.1.0"
edition = "2021"
rust-version = "1.88"
license = "MIT"
publish = false

[lib]
crate-type = ["staticlib", "rlib"]

[dependencies]
icu = { version = "2.3.1", features = ["sync"] }
icu_provider = "2.3"

[build-dependencies]
cbindgen = { version = "0.29", default-features = false }

[profile.dev]
opt-level = 1
panic = "abort"

# ICU4X is unusably slow unoptimized; optimize dependencies even in Debug.
[profile.dev.package."*"]
opt-level = 2

[profile.release]
panic = "abort"
```

`intl/core/cbindgen.toml`:
```toml
# (Meta MIT header as # comments)
language = "C"
include_guard = "HERMES_INTL_CORE_H"
autogen_warning = "/* Generated by cbindgen from intl/core. Do not edit. */"
cpp_compat = true
usize_is_size_t = true

[enum]
prefix_with_name = true
```

`intl/core/build.rs`:
```rust
// (Meta MIT header)

//! Generates the C header for the crate's `extern "C"` interface when CMake
//! asks for it through HERMES_INTL_CORE_HEADER. Plain `cargo test` skips it.

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=cbindgen.toml");
    println!("cargo:rerun-if-env-changed=HERMES_INTL_CORE_HEADER");
    let Ok(header) = std::env::var("HERMES_INTL_CORE_HEADER") else {
        return;
    };
    // A missing header counts as changed, so deleting it regenerates it.
    println!("cargo:rerun-if-changed={header}");
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    cbindgen::generate(&crate_dir)
        .expect("cbindgen failed to parse intl/core")
        .write_to_file(&header);
}
```

`intl/core/src/lib.rs`:
```rust
// (Meta MIT header)

//! The C interface of the Hermes Intl plugin's ICU4X core. Every function
//! here is a thin wrapper; the logic lives in the modules. C++ calls these
//! through intl/jsi/RustCore.h.

use std::os::raw::c_char;

/// A UTF-8 string owned by Rust and handed to C++, which must free it with
/// hi_string_free.
#[repr(C)]
pub struct HiString {
    pub data: *mut u8,
    pub len: usize,
    pub capacity: usize,
}

impl HiString {
    /// The value written to an out-parameter when there is no result.
    const EMPTY: HiString = HiString {
        data: std::ptr::null_mut(),
        len: 0,
        capacity: 0,
    };

    fn from_string(s: String) -> HiString {
        let mut bytes = std::mem::ManuallyDrop::new(s.into_bytes());
        HiString {
            data: bytes.as_mut_ptr(),
            len: bytes.len(),
            capacity: bytes.capacity(),
        }
    }
}

/// Free a string returned by this library. Accepts the empty value.
///
/// # Safety
/// `s` must come from this library and must not be freed twice.
#[no_mangle]
pub unsafe extern "C" fn hi_string_free(s: HiString) {
    if !s.data.is_null() {
        drop(Vec::from_raw_parts(s.data, s.len, s.capacity));
    }
}

/// The crate version, NUL-terminated, with static lifetime.
#[no_mangle]
pub extern "C" fn hi_core_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn version_is_the_crate_version() {
        let v = unsafe { CStr::from_ptr(hi_core_version()) };
        assert_eq!(v.to_str().unwrap(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn strings_round_trip() {
        let s = HiString::from_string("en-US".to_owned());
        let view = unsafe { std::slice::from_raw_parts(s.data, s.len) };
        assert_eq!(view, b"en-US");
        unsafe { hi_string_free(s) };
        unsafe { hi_string_free(HiString::EMPTY) };
    }
}
```

- [ ] **Step 2: Generate the lock file and run the tests**

```
cargo generate-lockfile --manifest-path /home/tmikov/work/hermes-intl/intl/core/Cargo.toml
cargo test --locked --manifest-path /home/tmikov/work/hermes-intl/intl/core/Cargo.toml
```
Expected: `test result: ok. 2 passed`. Confirm that `Cargo.lock` pins `icu 2.3.1`: `grep -A1 '^name = "icu"$' /home/tmikov/work/hermes-intl/intl/core/Cargo.lock`. Cargo puts its `target/` directory inside `intl/core`; delete it (`rm -rf /home/tmikov/work/hermes-intl/intl/core/target`) and add `intl/core/target/` to the repository `.gitignore`.

- [ ] **Step 3: Show the test can fail**

Change `"\0"` in `hi_core_version` to `"-x\0"`. Run `cargo test` again: `version_is_the_crate_version` fails. Revert the change.

- [ ] **Step 4: Add the CMake option and `intl/CMakeLists.txt`**

In the top-level `CMakeLists.txt`, after the `HERMES_ENABLE_CORE_EXTENSIONS` option:
```cmake
# The Intl plugin (intl/) builds for the host only until Phase 6 of
# doc/superpowers/specs/2026-10-22-intl-plugin-design.md adds cross builds.
set(_hermes_intl_default ON)
if(CMAKE_CROSSCOMPILING OR HERMES_IS_ANDROID OR HERMES_APPLE_TARGET_PLATFORM
   OR WIN32 OR EMSCRIPTEN OR CMAKE_OSX_ARCHITECTURES MATCHES ";")
  set(_hermes_intl_default OFF)
endif()
set(HERMES_BUILD_INTL ${_hermes_intl_default} CACHE BOOL
  "Build the ICU4X-based Intl plugin (libhermesintl); requires Rust")
```

After the block that ends with `add_subdirectory(${HERMES_JSI_DIR}/jsi ...)` and restores `CMAKE_POSITION_INDEPENDENT_CODE`:
```cmake
if(HERMES_BUILD_INTL)
  add_subdirectory(intl)
endif()
```

`intl/CMakeLists.txt`:
```cmake
# (Meta MIT header)

# The ICU4X-based Intl plugin: a separate shared library, libhermesintl, that
# Hermes loads when it creates a runtime. See
# doc/superpowers/specs/2026-10-22-intl-plugin-design.md.

if(CMAKE_CROSSCOMPILING OR HERMES_IS_ANDROID OR HERMES_APPLE_TARGET_PLATFORM
   OR WIN32 OR EMSCRIPTEN OR CMAKE_OSX_ARCHITECTURES MATCHES ";")
  message(FATAL_ERROR
    "HERMES_BUILD_INTL: the Intl plugin builds only for the host on Linux "
    "and single-architecture macOS for now. Configure with "
    "-DHERMES_BUILD_INTL=OFF.")
endif()
if(CMAKE_CONFIGURATION_TYPES)
  message(FATAL_ERROR
    "HERMES_BUILD_INTL: multi-configuration generators are not supported "
    "yet. Use Ninja or Makefiles, or configure with -DHERMES_BUILD_INTL=OFF.")
endif()

# The MSRV of icu 2.3.
set(HERMES_INTL_RUST_MIN_VERSION 1.88)
find_program(HERMES_INTL_CARGO cargo)
if(NOT HERMES_INTL_CARGO)
  message(FATAL_ERROR
    "HERMES_BUILD_INTL is ON, which requires a Rust toolchain, but cargo was "
    "not found on PATH. Install Rust (https://rustup.rs) or configure with "
    "-DHERMES_BUILD_INTL=OFF.")
endif()
execute_process(
  COMMAND ${HERMES_INTL_CARGO} --version
  OUTPUT_VARIABLE _cargo_version_out
  OUTPUT_STRIP_TRAILING_WHITESPACE
  RESULT_VARIABLE _cargo_rc)
string(REGEX MATCH "[0-9]+\\.[0-9]+(\\.[0-9]+)?" _cargo_version
  "${_cargo_version_out}")
if(NOT _cargo_rc EQUAL 0 OR
   _cargo_version VERSION_LESS HERMES_INTL_RUST_MIN_VERSION)
  message(FATAL_ERROR
    "HERMES_BUILD_INTL needs Rust ${HERMES_INTL_RUST_MIN_VERSION} or newer; "
    "${HERMES_INTL_CARGO} --version printed '${_cargo_version_out}'. Run "
    "'rustup update' or configure with -DHERMES_BUILD_INTL=OFF.")
endif()

# --- The Rust core -----------------------------------------------------------

set(HERMES_INTL_CORE_DIR ${CMAKE_CURRENT_SOURCE_DIR}/core)
set(_cargo_target_dir ${CMAKE_CURRENT_BINARY_DIR}/cargo)
if(CMAKE_BUILD_TYPE STREQUAL "Debug")
  set(HERMES_INTL_CARGO_PROFILE_ARGS "")
  set(_cargo_profile_dir debug)
else()
  set(HERMES_INTL_CARGO_PROFILE_ARGS --release)
  set(_cargo_profile_dir release)
endif()
set(_core_lib
  ${_cargo_target_dir}/${_cargo_profile_dir}/${CMAKE_STATIC_LIBRARY_PREFIX}hermes_intl_core${CMAKE_STATIC_LIBRARY_SUFFIX})
set(_core_include_dir ${CMAKE_CURRENT_BINARY_DIR}/include)
set(_core_header ${_core_include_dir}/hermes_intl_core.h)
# Imported targets require their include directories to exist at configure
# time.
file(MAKE_DIRECTORY ${_core_include_dir})
file(GLOB_RECURSE _core_sources CONFIGURE_DEPENDS
  ${HERMES_INTL_CORE_DIR}/src/*.rs ${HERMES_INTL_CORE_DIR}/data/*)
set(HERMES_INTL_CARGO_ENV
  ${CMAKE_COMMAND} -E env
  CARGO_TARGET_DIR=${_cargo_target_dir}
  HERMES_INTL_CORE_HEADER=${_core_header})

add_custom_command(
  OUTPUT ${_core_lib} ${_core_header}
  COMMAND ${HERMES_INTL_CARGO_ENV} ${HERMES_INTL_CARGO} build --locked --lib
    ${HERMES_INTL_CARGO_PROFILE_ARGS}
    --manifest-path ${HERMES_INTL_CORE_DIR}/Cargo.toml
  DEPENDS ${_core_sources}
    ${HERMES_INTL_CORE_DIR}/Cargo.toml ${HERMES_INTL_CORE_DIR}/Cargo.lock
    ${HERMES_INTL_CORE_DIR}/build.rs ${HERMES_INTL_CORE_DIR}/cbindgen.toml
  COMMENT "Building the Intl plugin's Rust core"
  USES_TERMINAL
  VERBATIM)
add_custom_target(hermesIntlCoreBuild DEPENDS ${_core_lib} ${_core_header})

find_package(Threads REQUIRED)
add_library(hermesIntlCore STATIC IMPORTED GLOBAL)
set_target_properties(hermesIntlCore PROPERTIES
  IMPORTED_LOCATION ${_core_lib}
  INTERFACE_INCLUDE_DIRECTORIES ${_core_include_dir})
# What the Rust standard library needs from the system (from
# `cargo rustc -- --print native-static-libs`).
target_link_libraries(hermesIntlCore INTERFACE Threads::Threads
  ${CMAKE_DL_LIBS})
if(NOT APPLE)
  target_link_libraries(hermesIntlCore INTERFACE m)
endif()
add_dependencies(hermesIntlCore hermesIntlCoreBuild)
```

- [ ] **Step 5: Verify the system libraries Rust needs**

```
cargo rustc --locked --lib --crate-type staticlib --manifest-path /home/tmikov/work/hermes-intl/intl/core/Cargo.toml -- --print native-static-libs 2>&1 | grep native-static-libs
rm -rf /home/tmikov/work/hermes-intl/intl/core/target
```
Expected on Linux: `-lgcc_s -lutil -lrt -lpthread -lm -ldl -lc`. The C++ driver links `gcc_s`, `c` and `rt` itself. If the list contains anything else that is not already covered by `Threads::Threads`, `${CMAKE_DL_LIBS}` or `m`, add it to the `INTERFACE` libraries and record it in the task report.

- [ ] **Step 6: Configure the ASan build and build the core**

Run the configure command from Global Constraints, then:
```
cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target hermesIntlCoreBuild
grep -c "hi_core_version\|hi_string_free" /home/tmikov/work/hermes-intl/cmake-build-asan/intl/include/hermes_intl_core.h
```
Expected: the build succeeds and `grep` prints `2`.

- [ ] **Step 7: Check that configure fails without Rust and succeeds with the option off**

```
S=/tmp/claude-1001/-home-tmikov-work-hermes/a918a33f-abe5-47f3-80d3-351885f648de/scratchpad
env PATH=/usr/bin:/bin cmake -S /home/tmikov/work/hermes-intl -B $S/intl-nocargo -G Ninja -DCMAKE_BUILD_TYPE=Debug -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ 2>&1 | grep -A2 "cargo was"
env PATH=/usr/bin:/bin cmake -S /home/tmikov/work/hermes-intl -B $S/intl-off -G Ninja -DCMAKE_BUILD_TYPE=Debug -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ -DHERMES_BUILD_INTL=OFF 2>&1 | tail -2
rm -rf $S/intl-nocargo $S/intl-off
```
(The scratch path is this session's; any empty scratch directory will do.)

Expected:
- The first command prints the "cargo was not found on PATH ... -DHERMES_BUILD_INTL=OFF" message.
- The second ends with `-- Build files have been written to`.

If `/usr/bin` contains a `cargo` on the machine, use a `PATH` without it.

- [ ] **Step 8: Stage**

`git -C /home/tmikov/work/hermes-intl add CMakeLists.txt .gitignore intl/CMakeLists.txt intl/core`

### Task 2: `IHermesBuiltinBytecode`

**Files:**
- Modify: `API/jsi/jsi/hermes-interfaces.h` (before the closing `} // namespace facebook::hermes`)
- Modify: `API/hermes/hermes.cpp`:
  - the base list of `HermesRuntimeImpl` (line 292);
  - its method declarations (next to `evaluateJavaScriptWithSourceMap`, about line 1382);
  - `HermesRuntimeImpl::castInterface` (line 1716);
  - the new method's definition, after `evaluateJavaScriptWithSourceMap` (about line 2100).
- Create: `unittests/API/BuiltinBytecodeTest.cpp`
- Modify: `unittests/API/CMakeLists.txt` (add the file to `APITestsSources`)

**Interfaces:**
- Produces:
  ```cpp
  namespace facebook::hermes {
  class IHermesBuiltinBytecode : public jsi::ICast {
   public:
    static constexpr jsi::UUID uuid{0xf76a71a1, 0x013c, 0x4c83, 0x9ec7, 0x21ae4ba70c6b};
    virtual jsi::Value runBuiltinBytecode(
        const std::shared_ptr<const jsi::Buffer> &bytecode,
        const std::string &sourceURL) = 0;
  };
  }
  ```

- [ ] **Step 1: Invoke the `gc-safe-coding` skill** (this task edits `API/hermes/hermes.cpp`).

- [ ] **Step 2: Write the failing test**

`unittests/API/BuiltinBytecodeTest.cpp`:
```cpp
// (Meta MIT header)

#include "hermes/CompileJS.h"
#include "hermes/hermes.h"

#include <gtest/gtest.h>

using namespace facebook;
using namespace facebook::hermes;

namespace {

/// Compile \p source to Hermes bytecode.
std::shared_ptr<const jsi::Buffer> compile(const std::string &source) {
  std::string bytecode;
  EXPECT_TRUE(::hermes::compileJS(source, bytecode));
  return std::make_shared<jsi::StringBuffer>(std::move(bytecode));
}

/// Function.prototype.toString applied to \p fn.
std::string functionSource(jsi::Runtime &rt, const jsi::Value &fn) {
  auto toString = rt.global()
                      .getPropertyAsObject(rt, "Function")
                      .getPropertyAsObject(rt, "prototype")
                      .getPropertyAsFunction(rt, "toString");
  return toString.callWithThis(rt, fn.asObject(rt)).getString(rt).utf8(rt);
}

TEST(BuiltinBytecodeTest, ReturnsTheCompletionValue) {
  auto rt = makeHermesRuntime();
  auto *builtin = jsi::castInterface<IHermesBuiltinBytecode>(rt.get());
  ASSERT_NE(builtin, nullptr);
  jsi::Value fn = builtin->runBuiltinBytecode(
      compile("(function () { return 41 + 1; })"), "test.js");
  EXPECT_EQ(fn.asObject(*rt).asFunction(*rt).call(*rt).getNumber(), 42);
}

TEST(BuiltinBytecodeTest, FunctionsPrintAsNativeCode) {
  auto rt = makeHermesRuntime();
  auto *builtin = jsi::castInterface<IHermesBuiltinBytecode>(rt.get());
  ASSERT_NE(builtin, nullptr);
  const char *src = "(function setup(a) { return a; })";
  jsi::Value viaBuiltin = builtin->runBuiltinBytecode(compile(src), "b.js");
  EXPECT_NE(
      functionSource(*rt, viaBuiltin).find("[native code]"),
      std::string::npos);
  // The same bytecode through evaluateJavaScript is ordinary user code.
  jsi::Value viaEval = rt->evaluateJavaScript(compile(src), "e.js");
  EXPECT_EQ(
      functionSource(*rt, viaEval).find("[native code]"), std::string::npos);
}

TEST(BuiltinBytecodeTest, ThrowsJSErrorWhenTheCodeThrows) {
  auto rt = makeHermesRuntime();
  auto *builtin = jsi::castInterface<IHermesBuiltinBytecode>(rt.get());
  ASSERT_NE(builtin, nullptr);
  EXPECT_THROW(
      builtin->runBuiltinBytecode(
          compile("throw new TypeError('boom')"), "t.js"),
      jsi::JSError);
}

TEST(BuiltinBytecodeTest, RejectsSourceText) {
  auto rt = makeHermesRuntime();
  auto *builtin = jsi::castInterface<IHermesBuiltinBytecode>(rt.get());
  ASSERT_NE(builtin, nullptr);
  EXPECT_THROW(
      builtin->runBuiltinBytecode(
          std::make_shared<jsi::StringBuffer>("1 + 1"), "s.js"),
      jsi::JSINativeException);
}

TEST(BuiltinBytecodeTest, ThreadSafeRuntimeForwardsTheInterface) {
  auto rt = makeThreadSafeHermesRuntime();
  EXPECT_NE(jsi::castInterface<IHermesBuiltinBytecode>(rt.get()), nullptr);
}

} // namespace
```
Add `BuiltinBytecodeTest.cpp` to `APITestsSources` in `unittests/API/CMakeLists.txt`, after `APITestFactory.cpp`.

- [ ] **Step 3: Run it and watch it fail to compile**

`cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target APITests`
Expected: compile error, `IHermesBuiltinBytecode` is not declared.

- [ ] **Step 4: Declare the interface**

Add to `API/jsi/jsi/hermes-interfaces.h`, before `} // namespace facebook::hermes`:
```cpp
/// Runs Hermes bytecode that ships with the engine build, such as the Intl
/// plugin's JS, the way the built-in extensions run: the module is
/// persistent, its epilogue is hidden, and Function.prototype.toString prints
/// its functions as [native code].
class IHermesBuiltinBytecode : public jsi::ICast {
 public:
  static constexpr jsi::UUID uuid{
      0xf76a71a1,
      0x013c,
      0x4c83,
      0x9ec7,
      0x21ae4ba70c6b};

  /// Run \p bytecode, which must be Hermes bytecode of this engine's bytecode
  /// version, and return its completion value. The runtime keeps \p bytecode
  /// alive for as long as it needs it. Throws jsi::JSINativeException if the
  /// buffer is not loadable bytecode, and jsi::JSError if the code throws.
  virtual jsi::Value runBuiltinBytecode(
      const std::shared_ptr<const jsi::Buffer> &bytecode,
      const std::string &sourceURL) = 0;

 protected:
  ~IHermesBuiltinBytecode() = default;
};
```

- [ ] **Step 5: Implement it in `HermesRuntimeImpl`**

Add `public IHermesBuiltinBytecode` after `public ISetEventLoopControl` in the base list:
```cpp
class HermesRuntimeImpl final : public HermesRuntime,
                                private IHermesTestHelpers,
                                private InstallHermesFatalErrorHandler,
                                private jsi::Instrumentation,
                                public ISetEventLoopControl,
                                public IHermesBuiltinBytecode
#ifdef JSI_UNSTABLE
```
Declare the override next to `evaluateJavaScriptWithSourceMap`:
```cpp
  jsi::Value runBuiltinBytecode(
      const std::shared_ptr<const jsi::Buffer> &bytecode,
      const std::string &sourceURL) override;
```
In `HermesRuntimeImpl::castInterface`, before the `ISetEventLoopControl` branch:
```cpp
  else if (interfaceUUID == IHermesBuiltinBytecode::uuid) {
    return static_cast<IHermesBuiltinBytecode *>(this);
  }
```
Define it after `evaluateJavaScriptWithSourceMap`:
```cpp
jsi::Value HermesRuntimeImpl::runBuiltinBytecode(
    const std::shared_ptr<const jsi::Buffer> &bytecode,
    const std::string &sourceURL) {
  ExecutionScopeRAII scopeRAII(mutatorScope);
  auto bcErr = hbc::BCProviderFromBuffer::createBCProviderFromBuffer(
      std::make_unique<BufferAdapter>(bytecode));
  if (!bcErr.first) {
    throw jsi::JSINativeException(
        "Loading builtin bytecode failed: " + std::move(bcErr.second) +
        ", sourceURL: " + sourceURL);
  }
  // The flags loadAndInstallExtensions uses.
  vm::RuntimeModuleFlags flags;
  flags.persistent = true;
  flags.hidesEpilogue = true;
  flags.funcsAreBuiltins = true;
  vm::GCScope gcScope(runtime_);
  auto res = runtime_.runBytecode(
      std::move(bcErr.first),
      flags,
      sourceURL,
      vm::Runtime::makeNullHandle<vm::Environment>());
  checkStatus(res.getStatus());
  return valueFromHermesValue(*res);
}
```

- [ ] **Step 6: Run the tests**

```
cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target APITests
/home/tmikov/work/hermes-intl/cmake-build-asan/unittests/API/APITests --gtest_filter='BuiltinBytecodeTest.*'
```
Expected: 5 tests pass. If `APITests` lands in a different directory, find it with `find /home/tmikov/work/hermes-intl/cmake-build-asan -name APITests -type f`.

- [ ] **Step 7: Show the tests can fail**

Remove `flags.funcsAreBuiltins = true;` from `runBuiltinBytecode`, rebuild and rerun: `FunctionsPrintAsNativeCode` fails. Restore the line.

- [ ] **Step 8: Stage**

`git -C /home/tmikov/work/hermes-intl add API/jsi/jsi/hermes-interfaces.h API/hermes/hermes.cpp unittests/API/BuiltinBytecodeTest.cpp unittests/API/CMakeLists.txt`

### Task 3: The loader, with registered plugins, in both factories

This task writes the whole loader, including the code that loads libraries. Its tests here cover registered (in-process) plugins, the version checks, the error rows that do not involve a library file, concurrency and both factories. Task 4 tests the library paths.

**Files:**
- Create: `public/hermes/Public/IntlPlugin.h`, `API/hermes/IntlPluginLoader.h`, `API/hermes/IntlPluginLoader.cpp`
- Modify:
  - `public/hermes/Public/RuntimeConfig.h`: four fields at the end of `RUNTIME_FIELDS`, `#include <string>` and `<vector>`, and the comment on `Intl`.
  - `include/hermes/VM/RuntimeFlags.h`: four flags after `Intl`.
  - `tools/hermes/hermes.cpp`: both `RuntimeConfig` builders, around lines 128 and 230.
  - `tools/hvm/hvm.cpp`: around line 141.
  - `API/hermes/hermes.h`: `registerIntlPlugin`.
  - `API/hermes/hermes.cpp`: both factories call `installIntlPlugin` after the extensions.
  - `API/hermes/CMakeLists.txt`: add `IntlPluginLoader.cpp` to `api_sources` and `${CMAKE_DL_LIBS}` to `hermesapi`'s `LINK_LIBS`.
  - `unittests/CMakeLists.txt`: `add_subdirectory(IntlLoader)`.
- Create: `unittests/IntlLoader/CMakeLists.txt`, `unittests/IntlLoader/IntlLoaderTest.cpp`

**Interfaces:**
- Produces, in `public/hermes/Public/IntlPlugin.h`: `hermes_intl_config`, `hermes_intl_entry`, `hermes_intl_get_entry_fn`, `HERMES_INTL_PLUGIN_INTERFACE_VERSION` (1), `HERMES_INTL_ENTRY_SYMBOL`, `HERMES_INTL_FLAG_JSI_UNSTABLE`, all exactly as written in Step 3.
- Produces, in `API/hermes/IntlPluginLoader.h` (internal to Hermes):
  - `void facebook::hermes::installIntlPlugin(jsi::Runtime &, const ::hermes::vm::RuntimeConfig &)`;
  - `::hermes::vm::RuntimeConfig facebook::hermes::makeWorkerRuntimeConfig(jsi::Runtime &parent)`.
- Produces, in `API/hermes/hermes.h`: `HERMES_EXPORT void facebook::hermes::registerIntlPlugin(const hermes_intl_entry *)`.
- Produces `RuntimeConfig` getters and builders, `getEnableIntlPlugin()` and so on, and CLI flags `-Xintl-plugin`, `-Xintl-plugin-path=<path>`, `-Xintl-default-locale=<tag>` (repeatable) and `-Xintl-default-time-zone=<zone>`.
- Fatal messages have the form `Intl plugin <where>: <why>`. `<where>` is the library path, or `registered with registerIntlPlugin`. Tests match these strings.

- [ ] **Step 1: Invoke the `gc-safe-coding` skill.**

- [ ] **Step 2: Write the failing tests**

`unittests/IntlLoader/CMakeLists.txt`:
```cmake
# (Meta MIT header)

# Tests for the Intl plugin loader (API/hermes/IntlPluginLoader.cpp) with
# in-process plugins. The fake plugin libraries for the lit tests in
# test/hermes/intl-plugin are added by Task 4.
set(HERMES_ENABLE_EH ON)
set(HERMES_ENABLE_RTTI ON)
set(LLVM_ENABLE_RTTI ON)

add_hermes_unittest(IntlLoaderTests IntlLoaderTest.cpp)
target_link_libraries(IntlLoaderTests hermesvm_a)
```
Add `add_subdirectory(IntlLoader)` to `unittests/CMakeLists.txt`, next to the other `add_subdirectory` lines.

`unittests/IntlLoader/IntlLoaderTest.cpp`:
```cpp
// (Meta MIT header)

// Every test that registers a plugin or resolves the default plugin runs in a
// child process (EXPECT_EXIT / EXPECT_DEATH), because both are once per
// process.

#include "hermes/BCGen/HBC/BytecodeVersion.h"
#include "hermes/IntlPluginLoader.h"
#include "hermes/Public/IntlPlugin.h"
#include "hermes/hermes.h"

#include <gtest/gtest.h>

#include <atomic>
#include <cstdlib>
#include <stdexcept>
#include <thread>
#include <vector>

using namespace facebook;
using namespace facebook::hermes;
using ::hermes::vm::RuntimeConfig;

namespace {

std::atomic<int> resolveCalls{0};
std::atomic<int> installCalls{0};

/// Fill in missing defaults the way a real plugin's detection would.
std::string fakeResolve(hermes_intl_config *config) {
  ++resolveCalls;
  if (config->default_locales.empty())
    config->default_locales.push_back("xx-detected");
  if (config->default_time_zone.empty())
    config->default_time_zone = "Fake/Detected";
  return "";
}

/// Record the first default locale where JS can see it.
void fakeInstall(jsi::Runtime &rt, const hermes_intl_config *config) {
  ++installCalls;
  rt.global().setProperty(
      rt,
      "__fakeIntlLocale",
      jsi::String::createFromUtf8(rt, config->default_locales.at(0)));
}

std::string rejectingResolve(hermes_intl_config *) {
  return "fake rejection";
}

std::string throwingResolve(hermes_intl_config *) {
  throw std::runtime_error("fake resolver failure");
}

void throwingInstall(jsi::Runtime &, const hermes_intl_config *) {
  throw jsi::JSINativeException("fake setup failure");
}

/// The flags of a plugin built like this Hermes.
uint32_t hostFlags() {
#ifdef JSI_UNSTABLE
  return HERMES_INTL_FLAG_JSI_UNSTABLE;
#else
  return 0;
#endif
}

/// A valid entry; tests break one field at a time.
hermes_intl_entry goodEntry() {
  return hermes_intl_entry{
      sizeof(hermes_intl_entry),
      HERMES_INTL_PLUGIN_INTERFACE_VERSION,
      ::hermes::hbc::BYTECODE_VERSION,
      hostFlags(),
      "fake",
      fakeResolve,
      fakeInstall};
}

/// The entry registered by the current child process.
hermes_intl_entry registered;

/// Register \p entry for the rest of this process.
void registerEntry(const hermes_intl_entry &entry) {
  registered = entry;
  registerIntlPlugin(&registered);
}

/// The value of __fakeIntlLocale, or "" if the plugin was not installed.
std::string fakeLocale(jsi::Runtime &rt) {
  auto v = rt.global().getProperty(rt, "__fakeIntlLocale");
  return v.isString() ? v.getString(rt).utf8(rt) : "";
}

class IntlLoaderTest : public ::testing::Test {
 protected:
  void SetUp() override {
    ::testing::FLAGS_gtest_death_test_style = "threadsafe";
  }
};

void registeredPluginIsInstalled() {
  registerEntry(goodEntry());
  auto rt = makeHermesRuntime();
  std::exit(fakeLocale(*rt) == "xx-detected" ? 0 : 1);
}
TEST_F(IntlLoaderTest, RegisteredPluginIsInstalled) {
  EXPECT_EXIT(registeredPluginIsInstalled(), ::testing::ExitedWithCode(0), "");
}

void explicitDefaultsReachThePlugin() {
  registerEntry(goodEntry());
  auto rt = makeHermesRuntime(RuntimeConfig::Builder()
                                  .withIntlDefaultLocales({"fr-FR", "de"})
                                  .withIntlDefaultTimeZone("Europe/Paris")
                                  .build());
  std::exit(fakeLocale(*rt) == "fr-FR" ? 0 : 1);
}
TEST_F(IntlLoaderTest, ExplicitDefaultsReachThePlugin) {
  EXPECT_EXIT(
      explicitDefaultsReachThePlugin(), ::testing::ExitedWithCode(0), "");
}

void disabledSkipsTheRegisteredPlugin() {
  registerEntry(goodEntry());
  auto rt = makeHermesRuntime(
      RuntimeConfig::Builder().withEnableIntlPlugin(false).build());
  std::exit(fakeLocale(*rt).empty() && installCalls == 0 ? 0 : 1);
}
TEST_F(IntlLoaderTest, DisabledSkipsTheRegisteredPlugin) {
  EXPECT_EXIT(
      disabledSkipsTheRegisteredPlugin(), ::testing::ExitedWithCode(0), "");
}

void threadSafeFactoryInstallsThePlugin() {
  registerEntry(goodEntry());
  auto rt = makeThreadSafeHermesRuntime();
  std::exit(fakeLocale(*rt) == "xx-detected" ? 0 : 1);
}
TEST_F(IntlLoaderTest, ThreadSafeFactoryInstallsThePlugin) {
  EXPECT_EXIT(
      threadSafeFactoryInstallsThePlugin(), ::testing::ExitedWithCode(0), "");
}

void concurrentRuntimesShareOneResolution() {
  registerEntry(goodEntry());
  constexpr int kThreads = 8;
  std::atomic<int> installed{0};
  std::atomic<int> ready{0};
  std::vector<std::thread> threads;
  for (int i = 0; i < kThreads; ++i) {
    threads.emplace_back([&installed, &ready] {
      // Start creating runtimes together, so resolution races if it can.
      ++ready;
      while (ready < kThreads) {
      }
      auto rt = makeHermesRuntime();
      if (fakeLocale(*rt) == "xx-detected")
        ++installed;
    });
  }
  for (auto &t : threads)
    t.join();
  std::exit(
      installed == kThreads && installCalls == kThreads &&
              intlDefaultResolutionsForTests() == 1
          ? 0
          : 1);
}
TEST_F(IntlLoaderTest, ConcurrentRuntimesShareOneResolution) {
  EXPECT_EXIT(
      concurrentRuntimesShareOneResolution(),
      ::testing::ExitedWithCode(0),
      "");
}

void wrongInterfaceVersion() {
  auto entry = goodEntry();
  entry.plugin_interface_version = HERMES_INTL_PLUGIN_INTERFACE_VERSION + 1;
  registerEntry(entry);
  makeHermesRuntime();
}
TEST_F(IntlLoaderTest, WrongInterfaceVersionIsFatal) {
  EXPECT_DEATH(
      wrongInterfaceVersion(),
      "Intl plugin registered with registerIntlPlugin: plugin interface "
      "version [0-9]+, Hermes expects [0-9]+");
}

void wrongBytecodeVersion() {
  auto entry = goodEntry();
  entry.bytecode_version = ::hermes::hbc::BYTECODE_VERSION + 1;
  registerEntry(entry);
  makeHermesRuntime();
}
TEST_F(IntlLoaderTest, WrongBytecodeVersionIsFatal) {
  EXPECT_DEATH(
      wrongBytecodeVersion(),
      "Intl plugin registered with registerIntlPlugin: bytecode version");
}

void wrongFlags() {
  auto entry = goodEntry();
  entry.flags ^= HERMES_INTL_FLAG_JSI_UNSTABLE;
  registerEntry(entry);
  makeHermesRuntime();
}
TEST_F(IntlLoaderTest, WrongFlagsAreFatal) {
  EXPECT_DEATH(
      wrongFlags(),
      "Intl plugin registered with registerIntlPlugin: built with "
      "JSI_UNSTABLE");
}

void smallStruct() {
  auto entry = goodEntry();
  entry.struct_size = 4;
  registerEntry(entry);
  makeHermesRuntime();
}
TEST_F(IntlLoaderTest, SmallEntryStructIsFatal) {
  EXPECT_DEATH(smallStruct(), "entry struct is too small");
}

void rejectedDefaults() {
  auto entry = goodEntry();
  entry.resolve_defaults = rejectingResolve;
  registerEntry(entry);
  makeHermesRuntime();
}
TEST_F(IntlLoaderTest, RejectedDefaultsAreFatal) {
  EXPECT_DEATH(
      rejectedDefaults(),
      "rejected the default locale or time zone: fake rejection");
}

void throwingResolver() {
  auto entry = goodEntry();
  entry.resolve_defaults = throwingResolve;
  registerEntry(entry);
  makeHermesRuntime();
}
TEST_F(IntlLoaderTest, ThrowingResolverIsFatal) {
  EXPECT_DEATH(
      throwingResolver(),
      "Intl plugin registered with registerIntlPlugin: default resolution "
      "failed: fake resolver failure");
}

void throwingSetup() {
  auto entry = goodEntry();
  entry.install = throwingInstall;
  registerEntry(entry);
  makeHermesRuntime();
}
TEST_F(IntlLoaderTest, ThrowingSetupIsFatal) {
  EXPECT_DEATH(throwingSetup(), "setup failed: fake setup failure");
}

void registerAfterResolution() {
  makeHermesRuntime();
  registerEntry(goodEntry());
}
TEST_F(IntlLoaderTest, RegisterAfterResolutionIsFatal) {
  EXPECT_DEATH(
      registerAfterResolution(),
      "registerIntlPlugin was called after the default plugin was resolved");
}

} // namespace
```

- [ ] **Step 3: Write the interface header**

`public/hermes/Public/IntlPlugin.h`:
```cpp
// (Meta MIT header)

#pragma once

/// The interface between Hermes and the Intl plugin library (libhermesintl).
/// See doc/superpowers/specs/2026-10-22-intl-plugin-design.md, "Loading and
/// installation". A plugin is supported only with the Hermes it was built
/// with: this is a C++ interface, and the loader checks the versions and
/// flags below before calling into the plugin.

#include <cstdint>
#include <string>
#include <vector>

namespace facebook::jsi {
class Runtime;
} // namespace facebook::jsi

/// Version of this interface: this header, IHermesBuiltinBytecode, and the
/// meaning of every field below. Bump it on any change to them.
#define HERMES_INTL_PLUGIN_INTERFACE_VERSION 1

/// The name of the function every plugin library exports, of type
/// hermes_intl_get_entry_fn.
#define HERMES_INTL_ENTRY_SYMBOL "hermes_intl_get_entry"

/// Set in hermes_intl_entry::flags when the plugin was compiled with
/// JSI_UNSTABLE, which changes the layout of jsi::Runtime.
#define HERMES_INTL_FLAG_JSI_UNSTABLE 1u

/// The defaults a runtime's Intl starts with. All strings are UTF-8.
struct hermes_intl_config {
  /// Preferred locales, most preferred first, as BCP 47 tags.
  std::vector<std::string> default_locales;
  /// An IANA time zone name.
  std::string default_time_zone;
};

/// What hermes_intl_get_entry returns: a static description of the plugin.
struct hermes_intl_entry {
  /// sizeof(hermes_intl_entry) in the plugin.
  uint32_t struct_size;
  /// HERMES_INTL_PLUGIN_INTERFACE_VERSION in the plugin.
  uint32_t plugin_interface_version;
  /// The Hermes bytecode version of the plugin's embedded JS.
  uint32_t bytecode_version;
  /// HERMES_INTL_FLAG_* bits.
  uint32_t flags;
  /// The plugin's own version, for messages.
  const char *plugin_version;
  /// Turn the defaults requested by RuntimeConfig (possibly empty) into the
  /// effective ones, in place: canonicalize the given values and detect the
  /// missing ones. Returns "" on success, or a message naming the invalid
  /// value.
  std::string (*resolve_defaults)(hermes_intl_config *config);
  /// Install Intl into \p rt using \p config as resolved by
  /// resolve_defaults. Throws on failure.
  void (*install)(facebook::jsi::Runtime &rt, const hermes_intl_config *config);
};

/// The type of HERMES_INTL_ENTRY_SYMBOL.
typedef const hermes_intl_entry *(*hermes_intl_get_entry_fn)();
```

- [ ] **Step 4: Add the `RuntimeConfig` fields and the CLI flags**

In `public/hermes/Public/RuntimeConfig.h`, add `#include <string>` and `#include <vector>` next to the other standard headers.

Change the comment above `F(constexpr, bool, Intl, true)` to:
```
  /* Support for the built-in, platform-based ECMA-402 implementation. */ \
  /* Does not affect the Intl plugin (EnableIntlPlugin). */             \
```

Add these fields after the `FinalizerThreadRunner` field, before `/* RUNTIME_FIELDS END */`. Fields may only be appended (`CtorConfig.h:21`). Keep the backslashes aligned with the rest of the macro:
```
  /* Load the Intl plugin (libhermesintl) when one is available. */     \
  F(constexpr, bool, EnableIntlPlugin, true)                            \
                                                                        \
  /* Load the Intl plugin from this library instead of the default. */  \
  F(HERMES_NON_CONSTEXPR, std::string, IntlPluginPath, "")              \
                                                                        \
  /* Default locales for Intl, most preferred first; empty: detect. */  \
  F(HERMES_NON_CONSTEXPR, std::vector<std::string>, IntlDefaultLocales) \
                                                                        \
  /* Default IANA time zone for Intl; empty: detect. */                 \
  F(HERMES_NON_CONSTEXPR, std::string, IntlDefaultTimeZone, "")         \
```

In `include/hermes/VM/RuntimeFlags.h`, after the `Intl` option:
```cpp
  llvh::cl::opt<bool> IntlPlugin{
      "Xintl-plugin",
      llvh::cl::desc("Load the Intl plugin when one is available"),
      llvh::cl::init(vm::RuntimeConfig::getDefaultEnableIntlPlugin()),
      llvh::cl::cat(RuntimeCategory)};

  llvh::cl::opt<std::string> IntlPluginPath{
      "Xintl-plugin-path",
      llvh::cl::desc("Load the Intl plugin from this library"),
      llvh::cl::value_desc("path"),
      llvh::cl::cat(RuntimeCategory)};

  llvh::cl::list<std::string> IntlDefaultLocales{
      "Xintl-default-locale",
      llvh::cl::desc(
          "Default locale for Intl; repeat to give a preference list"),
      llvh::cl::value_desc("tag"),
      llvh::cl::cat(RuntimeCategory)};

  llvh::cl::opt<std::string> IntlDefaultTimeZone{
      "Xintl-default-time-zone",
      llvh::cl::desc("Default IANA time zone for Intl"),
      llvh::cl::value_desc("zone"),
      llvh::cl::cat(RuntimeCategory)};
```
In both builders in `tools/hermes/hermes.cpp`, and in the one in `tools/hvm/hvm.cpp`, after `.withIntl(flags.Intl)`:
```cpp
          .withEnableIntlPlugin(flags.IntlPlugin)
          .withIntlPluginPath(flags.IntlPluginPath)
          .withIntlDefaultLocales(std::vector<std::string>(
              flags.IntlDefaultLocales.begin(),
              flags.IntlDefaultLocales.end()))
          .withIntlDefaultTimeZone(flags.IntlDefaultTimeZone)
```

- [ ] **Step 5: Write the loader**

`API/hermes/IntlPluginLoader.h`:
```cpp
// (Meta MIT header)

#pragma once

#include "hermes/Public/RuntimeConfig.h"

#include <jsi/jsi.h>

namespace facebook::hermes {

/// Install the Intl plugin into \p rt as \p config asks, and record the
/// outcome on \p rt for makeWorkerRuntimeConfig. Follows the "Loader" section
/// of doc/superpowers/specs/2026-10-22-intl-plugin-design.md: every row of
/// its error table marked fatal calls hermes_fatal.
void installIntlPlugin(
    jsi::Runtime &rt,
    const ::hermes::vm::RuntimeConfig &config);

/// The RuntimeConfig for a Worker started by \p parent: the parent's plugin
/// (or none), the default locales and time zone the parent's plugin actually
/// received, and the parent's Intl flag. Everything else is the default.
::hermes::vm::RuntimeConfig makeWorkerRuntimeConfig(jsi::Runtime &parent);

/// How many times this process has resolved the default plugin (0 or 1).
/// For tests.
unsigned intlDefaultResolutionsForTests();

} // namespace facebook::hermes
```

`API/hermes/IntlPluginLoader.cpp`:
```cpp
// (Meta MIT header)

#include "hermes/IntlPluginLoader.h"

#include "hermes/BCGen/HBC/BytecodeVersion.h"
#include "hermes/Public/IntlPlugin.h"
#include "hermes/Support/ErrorHandling.h"
#include "hermes/hermes.h"

#include <climits>
#include <cstdlib>
#include <cstring>
#include <map>
#include <memory>
#include <mutex>
#include <optional>
#include <string>
#include <vector>

#if defined(_WIN32) || defined(__EMSCRIPTEN__)
/// Loading the plugin from a library arrives on these platforms in Phase 6;
/// until then only registerIntlPlugin works.
#define HERMES_INTL_DYNAMIC_LOADING 0
#else
#define HERMES_INTL_DYNAMIC_LOADING 1
#include <cerrno>
#include <dlfcn.h>
#include <sys/stat.h>
#include <unistd.h>
#if defined(__linux__)
#include <elf.h>
#include <link.h>
#include <sys/auxv.h>
#elif defined(__APPLE__)
#include <libproc.h>
#include <mach-o/dyld.h>
#endif
#endif

namespace facebook::hermes {
namespace {

/// The file name of the default plugin library.
constexpr const char *kLibraryName =
#if defined(__APPLE__)
    "libhermesintl.dylib";
#elif defined(_WIN32)
    "hermesintl.dll";
#else
    "libhermesintl.so";
#endif

/// The runtime-data key of the IntlRecord installIntlPlugin stores.
constexpr jsi::UUID kIntlRecordUUID{
    0xdd08ef7e,
    0x9a6f,
    0x4841,
    0xb4f0,
    0x45f899fff1b0};

/// What installIntlPlugin did for one runtime; makeWorkerRuntimeConfig copies
/// it into a Worker's config.
struct IntlRecord {
  /// The runtime's Intl field (the old, built-in implementation).
  bool oldIntl = true;
  /// Whether a plugin was installed.
  bool installed = false;
  /// The plugin's library path from RuntimeConfig; empty for the default.
  std::string pluginPath;
  /// The defaults the plugin's install received.
  hermes_intl_config defaults;
};

/// A plugin entry that passed checkEntry.
struct LoadedPlugin {
  /// The entry.
  const hermes_intl_entry *entry;
  /// The library path, or "registered with registerIntlPlugin", for
  /// messages.
  std::string where;
};

/// Guards everything in LoaderState.
std::mutex &loaderMutex() {
  static std::mutex mutex;
  return mutex;
}

/// Process-wide loader state. Libraries are loaded once and never unloaded.
struct LoaderState {
  /// Set by registerIntlPlugin.
  const hermes_intl_entry *registered = nullptr;
  /// Whether the default plugin has been resolved.
  bool defaultResolved = false;
  /// The default plugin, if one was found.
  std::optional<LoadedPlugin> defaultPlugin;
  /// Plugins loaded from an explicit IntlPluginPath, by path.
  std::map<std::string, LoadedPlugin> byPath;
  /// How many times the default plugin was resolved; must stay <= 1.
  unsigned defaultResolutions = 0;
};

LoaderState &loaderState() {
  static LoaderState state;
  return state;
}

[[noreturn]] void fail(const std::string &where, const std::string &why) {
  ::hermes::hermes_fatal("Intl plugin " + where + ": " + why);
}

/// The flags a plugin must have to match this Hermes.
uint32_t expectedFlags() {
#ifdef JSI_UNSTABLE
  return HERMES_INTL_FLAG_JSI_UNSTABLE;
#else
  return 0;
#endif
}

/// Check that \p entry can be used by this Hermes; fatal otherwise.
LoadedPlugin checkEntry(const hermes_intl_entry *entry, std::string where) {
  if (!entry)
    fail(where, "returned no entry");
  if (entry->struct_size < sizeof(hermes_intl_entry))
    fail(where, "entry struct is too small");
  if (entry->plugin_interface_version != HERMES_INTL_PLUGIN_INTERFACE_VERSION)
    fail(
        where,
        "plugin interface version " +
            std::to_string(entry->plugin_interface_version) +
            ", Hermes expects " +
            std::to_string(HERMES_INTL_PLUGIN_INTERFACE_VERSION));
  if (entry->bytecode_version != ::hermes::hbc::BYTECODE_VERSION)
    fail(
        where,
        "bytecode version " + std::to_string(entry->bytecode_version) +
            ", Hermes expects " +
            std::to_string(::hermes::hbc::BYTECODE_VERSION));
  if (entry->flags != expectedFlags())
    fail(
        where,
        std::string("built with JSI_UNSTABLE ") +
            ((entry->flags & HERMES_INTL_FLAG_JSI_UNSTABLE) ? "on" : "off") +
            ", Hermes with it " +
            ((expectedFlags() & HERMES_INTL_FLAG_JSI_UNSTABLE) ? "on" : "off"));
  if (!entry->resolve_defaults || !entry->install)
    fail(where, "entry has a null function");
  return LoadedPlugin{entry, std::move(where)};
}

#if HERMES_INTL_DYNAMIC_LOADING

/// Load the library at \p path and check its entry; fatal on any failure.
LoadedPlugin loadLibrary(const std::string &path) {
  void *handle = ::dlopen(path.c_str(), RTLD_NOW | RTLD_LOCAL);
  if (!handle)
    fail(path, std::string("cannot be loaded: ") + ::dlerror());
  auto getEntry = reinterpret_cast<hermes_intl_get_entry_fn>(
      ::dlsym(handle, HERMES_INTL_ENTRY_SYMBOL));
  if (!getEntry)
    fail(path, "does not export " HERMES_INTL_ENTRY_SYMBOL);
  return checkEntry(getEntry(), path);
}

/// Whether \p path is absent. Any other stat failure (permissions, I/O)
/// counts as present, so loading it reports the real error.
bool fileAbsent(const std::string &path) {
  struct stat st;
  return ::stat(path.c_str(), &st) != 0 && (errno == ENOENT || errno == ENOTDIR);
}

/// The absolute path of the main executable, independent of the current
/// directory; "" if unknown.
std::string mainExecutablePath() {
#if defined(__linux__)
  char buf[PATH_MAX];
  ssize_t n = ::readlink("/proc/self/exe", buf, sizeof(buf) - 1);
  return n > 0 ? std::string(buf, n) : std::string();
#elif defined(__APPLE__)
  char buf[PROC_PIDPATHINFO_MAXSIZE];
  int n = ::proc_pidpath(::getpid(), buf, sizeof(buf));
  return n > 0 ? std::string(buf, n) : std::string();
#else
  return "";
#endif
}

/// Whether \p base, a dladdr dli_fbase, is the main executable's.
bool isMainExecutable(const void *base) {
#if defined(__linux__)
  // AT_PHDR is the address of the main executable's program headers, which
  // sit e_phoff bytes after its ELF header (= dli_fbase).
  auto phdr = reinterpret_cast<const char *>(::getauxval(AT_PHDR));
  auto ehdr = reinterpret_cast<const ElfW(Ehdr) *>(base);
  return phdr && phdr - ehdr->e_phoff == reinterpret_cast<const char *>(base);
#elif defined(__APPLE__)
  return base == ::_dyld_get_image_header(0);
#else
  (void)base;
  return false;
#endif
}

/// The directory holding the code of the Hermes library or executable this
/// function is part of; empty if unknown.
std::string hermesDirectory() {
  Dl_info info;
  std::string path;
  if (!::dladdr(reinterpret_cast<void *>(&installIntlPlugin), &info))
    return "";
  // For the main executable dladdr reports argv[0], which may be relative
  // to a directory the process has since left; ask the OS instead.
  if (isMainExecutable(info.dli_fbase))
    path = mainExecutablePath();
  else if (info.dli_fname)
    path = info.dli_fname;
  if (path.empty())
    return "";
  char resolved[PATH_MAX];
  if (::realpath(path.c_str(), resolved))
    path = resolved;
  auto slash = path.rfind('/');
  return slash == std::string::npos ? "" : path.substr(0, slash);
}

/// Whether \p error, from dlopen of the bare kLibraryName, says that the
/// library itself was not found. A missing dependency, a wrong architecture
/// or unresolved symbols mean it was found but is broken.
bool isLibraryNotFound(const std::string &error) {
#if defined(__ANDROID__)
  // bionic: dlopen failed: library "libhermesintl.so" not found
  return error.find(std::string("library \"") + kLibraryName +
                    "\" not found") != std::string::npos;
#elif defined(__APPLE__)
  // dyld lists every path it tried with a reason; absent means the message
  // is about kLibraryName itself (not "Library not loaded: <dependency>")
  // and every reason is "no such file".
  if (error.compare(0, 7 + strlen(kLibraryName), std::string("dlopen(") + kLibraryName) != 0 ||
      error.find("Library not loaded") != std::string::npos)
    return false;
  size_t tried = 0, absent = 0;
  for (size_t pos = 0; (pos = error.find("' (", pos)) != std::string::npos;
       pos += 3) {
    ++tried;
    if (error.compare(pos + 3, 12, "no such file") == 0)
      ++absent;
  }
  return tried > 0 && tried == absent;
#else
  // glibc: "libhermesintl.so: cannot open shared object file: No such file
  // or directory". A missing dependency names the dependency instead, and a
  // permission error ends differently; both mean found but broken.
  std::string expected = std::string(kLibraryName) +
      ": cannot open shared object file: No such file or directory";
  return error == expected;
#endif
}

/// Find the default plugin: next to the Hermes code, then in ../lib when
/// that code is in a bin directory, then on the library search path.
std::optional<LoadedPlugin> findDefaultLibrary() {
  std::string dir = hermesDirectory();
  if (!dir.empty()) {
    std::vector<std::string> candidates{dir + "/" + kLibraryName};
    auto slash = dir.rfind('/');
    if (slash != std::string::npos && dir.substr(slash + 1) == "bin")
      candidates.push_back(dir.substr(0, slash) + "/lib/" + kLibraryName);
    for (const auto &candidate : candidates) {
      if (!fileAbsent(candidate))
        return loadLibrary(candidate);
    }
  }
  void *handle = ::dlopen(kLibraryName, RTLD_NOW | RTLD_LOCAL);
  if (!handle) {
    std::string error = ::dlerror();
    if (isLibraryNotFound(error))
      return std::nullopt;
    fail(kLibraryName, "cannot be loaded: " + error);
  }
  auto getEntry = reinterpret_cast<hermes_intl_get_entry_fn>(
      ::dlsym(handle, HERMES_INTL_ENTRY_SYMBOL));
  if (!getEntry)
    fail(kLibraryName, "does not export " HERMES_INTL_ENTRY_SYMBOL);
  return checkEntry(getEntry(), kLibraryName);
}

#endif // HERMES_INTL_DYNAMIC_LOADING

/// The plugin at \p path, loading it the first time. Caller holds the mutex.
LoadedPlugin pluginAtPath(LoaderState &state, const std::string &path) {
  auto it = state.byPath.find(path);
  if (it != state.byPath.end())
    return it->second;
#if HERMES_INTL_DYNAMIC_LOADING
  LoadedPlugin plugin = loadLibrary(path);
  state.byPath.emplace(path, plugin);
  return plugin;
#else
  fail(path, "loading the plugin from a library is not supported here yet");
#endif
}

/// The default plugin, resolving it the first time. Caller holds the mutex.
std::optional<LoadedPlugin> defaultPlugin(LoaderState &state) {
  if (!state.defaultResolved) {
    ++state.defaultResolutions;
    if (state.registered) {
      state.defaultPlugin =
          checkEntry(state.registered, "registered with registerIntlPlugin");
    } else {
#if HERMES_INTL_DYNAMIC_LOADING
      state.defaultPlugin = findDefaultLibrary();
#endif
    }
    state.defaultResolved = true;
  }
  return state.defaultPlugin;
}

} // namespace

void registerIntlPlugin(const hermes_intl_entry *entry) {
  std::lock_guard<std::mutex> lock(loaderMutex());
  LoaderState &state = loaderState();
  if (state.defaultResolved)
    fail(
        "registration",
        "registerIntlPlugin was called after the default plugin was resolved");
  state.registered = entry;
}

void installIntlPlugin(
    jsi::Runtime &rt,
    const ::hermes::vm::RuntimeConfig &config) {
  auto record = std::make_shared<IntlRecord>();
  record->oldIntl = config.getIntl();
  std::optional<LoadedPlugin> plugin;
  if (config.getEnableIntlPlugin()) {
    std::lock_guard<std::mutex> lock(loaderMutex());
    LoaderState &state = loaderState();
    if (!config.getIntlPluginPath().empty())
      plugin = pluginAtPath(state, config.getIntlPluginPath());
    else
      plugin = defaultPlugin(state);
  }
  if (plugin) {
    record->installed = true;
    record->pluginPath = config.getIntlPluginPath();
    record->defaults.default_locales = config.getIntlDefaultLocales();
    record->defaults.default_time_zone = config.getIntlDefaultTimeZone();
    std::string error;
    try {
      error = plugin->entry->resolve_defaults(&record->defaults);
    } catch (const std::exception &e) {
      fail(plugin->where, std::string("default resolution failed: ") + e.what());
    } catch (...) {
      fail(plugin->where, "default resolution failed with an unknown exception");
    }
    if (!error.empty())
      fail(plugin->where, "rejected the default locale or time zone: " + error);
    try {
      plugin->entry->install(rt, &record->defaults);
    } catch (const std::exception &e) {
      fail(plugin->where, std::string("setup failed: ") + e.what());
    } catch (...) {
      fail(plugin->where, "setup failed with an unknown exception");
    }
  }
  rt.setRuntimeData(kIntlRecordUUID, record);
}

unsigned intlDefaultResolutionsForTests() {
  std::lock_guard<std::mutex> lock(loaderMutex());
  return loaderState().defaultResolutions;
}

::hermes::vm::RuntimeConfig makeWorkerRuntimeConfig(jsi::Runtime &parent) {
  auto record = std::static_pointer_cast<IntlRecord>(
      parent.getRuntimeData(kIntlRecordUUID));
  auto builder = ::hermes::vm::RuntimeConfig::Builder();
  if (!record)
    return builder.build();
  builder.withIntl(record->oldIntl).withEnableIntlPlugin(record->installed);
  if (record->installed) {
    builder.withIntlPluginPath(record->pluginPath)
        .withIntlDefaultLocales(record->defaults.default_locales)
        .withIntlDefaultTimeZone(record->defaults.default_time_zone);
  }
  return builder.build();
}

} // namespace facebook::hermes
```

In `API/hermes/hermes.h`, before `namespace facebook {`:
```cpp
struct hermes_intl_entry;
```
and in `namespace facebook::hermes`, after `makeThreadSafeHermesRuntime`:
```cpp
/// Make \p entry the default Intl plugin of this process: the way to use a
/// plugin linked into the application statically. It must be called before
/// the first runtime that uses the default plugin is created; a later call
/// is fatal. \p entry must outlive every runtime.
HERMES_EXPORT void registerIntlPlugin(const hermes_intl_entry *entry);
```

In `API/hermes/hermes.cpp`, add `#include <hermes/IntlPluginLoader.h>` beside `#include <hermes/hermes.h>`. In `HermesRootAPI::makeHermesRuntime`, after the `#if HERMES_ENABLE_CORE_EXTENSIONS` block that calls `loadAndInstallExtensions(*ret)`:
```cpp
  installIntlPlugin(*ret, runtimeConfig);
```
In `makeThreadSafeHermesRuntime`, after its `loadAndInstallExtensions(hermesRt)` block:
```cpp
  installIntlPlugin(hermesRt, runtimeConfig);
```

In `API/hermes/CMakeLists.txt`, add `IntlPluginLoader.cpp` to `api_sources`, and add `${CMAKE_DL_LIBS}` to `LINK_LIBS` of `add_hermes_library(hermesapi ...)`.

- [ ] **Step 6: Build and run the tests**

```
cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target IntlLoaderTests hermes
/home/tmikov/work/hermes-intl/cmake-build-asan/unittests/IntlLoader/IntlLoaderTests
/home/tmikov/work/hermes-intl/cmake-build-asan/bin/hermes -help | grep -c Xintl-
```
Expected:
- 13 tests pass.
- `grep` prints at least `5` (`-Xintl` and the four new flags).

The ASan build must report no errors. `ConcurrentRuntimesShareOneResolution` is the one most likely to expose a race.

- [ ] **Step 7: Show the tests can fail**

Make each change below, run, see the named test fail, then revert it:
- In `checkEntry`, delete the `bytecode_version` check: `WrongBytecodeVersionIsFatal` fails.
- In `defaultPlugin`, delete `state.defaultResolved = true;`: `RegisterAfterResolutionIsFatal` fails.
- In `installIntlPlugin`, delete the `std::lock_guard` line, and in `defaultPlugin` insert `std::this_thread::sleep_for(std::chrono::milliseconds(10));` before `state.defaultResolved = true;`. `ConcurrentRuntimesShareOneResolution` fails: `intlDefaultResolutionsForTests()` is above 1. (ASan does not detect races; this assertion does.) Optionally, run the test once in a `-DHERMES_ENABLE_THREAD_SANITIZER=ON` build and record the result.

- [ ] **Step 8: Run `check-hermes`**

`cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-hermes`
Expected: same pass/fail set as before this task. No plugin library exists yet, so default resolution finds nothing. Record any changed test.

- [ ] **Step 9: Stage**

`git -C /home/tmikov/work/hermes-intl add public/hermes/Public/IntlPlugin.h public/hermes/Public/RuntimeConfig.h include/hermes/VM/RuntimeFlags.h tools/hermes/hermes.cpp tools/hvm/hvm.cpp API/hermes unittests/CMakeLists.txt unittests/IntlLoader`

### Task 4: The loader with libraries, by path and by default resolution

**Files:**
- Modify: `cmake/modules/Hermes.cmake` (`add_hermes_tool`)
- Create: `unittests/IntlLoader/fixtures/FakeIntlPlugin.cpp`
- Modify: `unittests/IntlLoader/CMakeLists.txt` (fixture libraries)
- Modify: `CMakeLists.txt` (fixtures in `HERMES_TEST_DEPS`; lit param `intl_fixture_dir`)
- Modify: `test/lit.cfg` (`%hermes_path`, `%intl_fixture_dir`, `%intl_library_name`, feature `intl_plugin_fixtures`)
- Create: `test/hermes/intl-plugin/lit.local.cfg`, `test/hermes/intl-plugin/explicit-path.js`, `test/hermes/intl-plugin/explicit-path-errors.js`, `test/hermes/intl-plugin/default-resolution.js`

**Interfaces:**
- Consumes: the loader from Task 3.
- Produces:
  - Fake plugin libraries in `<build>/unittests/IntlLoader/`, all with the suffix `.so` on every platform (`dlopen` does not care): `libfake_intl_good.so`, `libfake_intl_no_entry.so`, `libfake_intl_bad_interface.so`, `libfake_intl_bad_bytecode.so`, `libfake_intl_bad_flags.so`, `libfake_intl_throwing.so`, `libfake_intl_rejects_defaults.so`.
  - The good fake sets `globalThis.__fakeIntlPlugin = {locales, timeZone, detections}`. Its `resolve_defaults` fills empty locales with `x-detected-<n>`, where `n` counts its detections in this process, and an empty time zone with `Fake/Detected`.
  - Lit substitutions `%hermes_path`, `%intl_fixture_dir` and `%intl_library_name`, and the feature `intl_plugin_fixtures`.

- [ ] **Step 1: Export dynamic symbols from every tool**

In `cmake/modules/Hermes.cmake`, at the end of `add_hermes_tool`, before `endfunction(add_hermes_tool)`:
```cmake
  # Export the tool's dynamic symbols so libraries it loads at runtime (the
  # Intl plugin, Node-API addons) can bind to the JSI code compiled into it.
  if(APPLE)
    target_link_options(${name} PRIVATE -Wl,-export_dynamic)
  elseif(UNIX)
    target_link_options(${name} PRIVATE -rdynamic)
  endif()
```

- [ ] **Step 2: Write the fake plugin and its library variants**

`unittests/IntlLoader/fixtures/FakeIntlPlugin.cpp`:
```cpp
// (Meta MIT header)

// A stand-in for libhermesintl, used by test/hermes/intl-plugin. Compile
// definitions select a broken variant:
//   FAKE_NO_ENTRY           do not export hermes_intl_get_entry
//   FAKE_INTERFACE_DELTA    added to the interface version
//   FAKE_BYTECODE_DELTA     added to the bytecode version
//   FAKE_FLIP_FLAGS         flip the JSI_UNSTABLE flag
//   FAKE_THROWING_INSTALL   install throws
//   FAKE_REJECT_DEFAULTS    resolve_defaults reports an error

#include "hermes/BCGen/HBC/BytecodeVersion.h"
#include "hermes/Public/IntlPlugin.h"

#include <jsi/jsi.h>

#include <atomic>
#include <string>

#ifndef FAKE_INTERFACE_DELTA
#define FAKE_INTERFACE_DELTA 0
#endif
#ifndef FAKE_BYTECODE_DELTA
#define FAKE_BYTECODE_DELTA 0
#endif

using namespace facebook;

namespace {

/// How many times resolve_defaults detected the locales in this process.
std::atomic<int> detections{0};

std::string resolveDefaults(hermes_intl_config *config) {
#ifdef FAKE_REJECT_DEFAULTS
  (void)config;
  return "fake plugin rejects every default";
#else
  if (config->default_locales.empty())
    config->default_locales.push_back(
        "x-detected-" + std::to_string(++detections));
  if (config->default_time_zone.empty())
    config->default_time_zone = "Fake/Detected";
  return "";
#endif
}

/// Native state owned by the plugin, to check NativeState across the
/// boundary.
class FakeState final : public jsi::NativeState {
 public:
  int value = 42;
};

/// Install the host functions that exercise the binary contract:
///   __fakeIntlThrow()       throws jsi::JSError from plugin code; the host
///                           must catch it as JSError (the JS sees the
///                           message, not "Exception in HostFunction").
///   __fakeIntlAttach(o)     attaches a FakeState to o.
///   __fakeIntlRead(o)       FakeState::value via getNativeState<FakeState>,
///                           or -1 if hasNativeState<FakeState> is false.
void installContractChecks(jsi::Runtime &rt) {
  auto define = [&rt](const char *name, unsigned params, jsi::HostFunctionType fn) {
    rt.global().setProperty(
        rt,
        name,
        jsi::Function::createFromHostFunction(
            rt, jsi::PropNameID::forAscii(rt, name), params, std::move(fn)));
  };
  define(
      "__fakeIntlThrow",
      0,
      [](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *, size_t)
          -> jsi::Value { throw jsi::JSError(rt, "fake JSError"); });
  define(
      "__fakeIntlAttach",
      1,
      [](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *args, size_t)
          -> jsi::Value {
        args[0].asObject(rt).setNativeState(rt, std::make_shared<FakeState>());
        return jsi::Value::undefined();
      });
  define(
      "__fakeIntlRead",
      1,
      [](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *args, size_t)
          -> jsi::Value {
        jsi::Object o = args[0].asObject(rt);
        if (!o.hasNativeState<FakeState>(rt))
          return jsi::Value(-1);
        return jsi::Value(o.getNativeState<FakeState>(rt)->value);
      });
}

void install(jsi::Runtime &rt, const hermes_intl_config *config) {
#ifdef FAKE_THROWING_INSTALL
  throw jsi::JSINativeException("fake plugin setup failure");
#endif
  installContractChecks(rt);
  jsi::Array locales(rt, config->default_locales.size());
  for (size_t i = 0; i < config->default_locales.size(); ++i)
    locales.setValueAtIndex(
        rt, i, jsi::String::createFromUtf8(rt, config->default_locales[i]));
  jsi::Object info(rt);
  info.setProperty(rt, "locales", locales);
  info.setProperty(
      rt,
      "timeZone",
      jsi::String::createFromUtf8(rt, config->default_time_zone));
  info.setProperty(rt, "detections", detections.load());
  rt.global().setProperty(rt, "__fakeIntlPlugin", info);
}

uint32_t flags() {
#ifdef JSI_UNSTABLE
  uint32_t f = HERMES_INTL_FLAG_JSI_UNSTABLE;
#else
  uint32_t f = 0;
#endif
#ifdef FAKE_FLIP_FLAGS
  f ^= HERMES_INTL_FLAG_JSI_UNSTABLE;
#endif
  return f;
}

const hermes_intl_entry kEntry{
    sizeof(hermes_intl_entry),
    HERMES_INTL_PLUGIN_INTERFACE_VERSION + FAKE_INTERFACE_DELTA,
    ::hermes::hbc::BYTECODE_VERSION + FAKE_BYTECODE_DELTA,
    flags(),
    "fake",
    resolveDefaults,
    install};

} // namespace

#ifndef FAKE_NO_ENTRY
extern "C" __attribute__((visibility("default"))) const hermes_intl_entry *
hermes_intl_get_entry() {
  return &kEntry;
}
#endif
```

Append to `unittests/IntlLoader/CMakeLists.txt`:
```cmake
# Fake plugin libraries for test/hermes/intl-plugin. Like the Node-API test
# addons they leave JSI undefined and bind to the host's exported copy, which
# Windows linkers do not allow.
if(NOT WIN32)
  function(add_fake_intl_plugin name)
    add_hermes_library(${name} SHARED fixtures/FakeIntlPlugin.cpp)
    target_compile_definitions(${name} PRIVATE ${ARGN})
    target_include_directories(${name} PRIVATE
      ${HERMES_JSI_DIR} ${PROJECT_SOURCE_DIR}/public
      ${PROJECT_SOURCE_DIR}/include)
    set_target_properties(${name} PROPERTIES SUFFIX ".so")
    if(APPLE)
      target_link_options(${name} PRIVATE -undefined dynamic_lookup)
    endif()
    set_property(GLOBAL APPEND PROPERTY HERMES_INTL_FIXTURE_TARGETS ${name})
  endfunction()

  add_fake_intl_plugin(fake_intl_good)
  add_fake_intl_plugin(fake_intl_no_entry FAKE_NO_ENTRY=1)
  add_fake_intl_plugin(fake_intl_bad_interface FAKE_INTERFACE_DELTA=1)
  add_fake_intl_plugin(fake_intl_bad_bytecode FAKE_BYTECODE_DELTA=1)
  add_fake_intl_plugin(fake_intl_bad_flags FAKE_FLIP_FLAGS=1)
  add_fake_intl_plugin(fake_intl_throwing FAKE_THROWING_INSTALL=1)
  add_fake_intl_plugin(fake_intl_rejects_defaults FAKE_REJECT_DEFAULTS=1)

  # A plugin whose dependency cannot be found: found but broken, never
  # "not found". It has no RPATH, so the dependency is missing wherever the
  # plugin is copied.
  add_hermes_library(fake_intl_dep SHARED fixtures/FakeIntlDep.cpp)
  set_target_properties(fake_intl_dep PROPERTIES SUFFIX ".so")
  add_fake_intl_plugin(fake_intl_needs_dep)
  target_link_libraries(fake_intl_needs_dep PRIVATE fake_intl_dep)
  set_target_properties(fake_intl_needs_dep PROPERTIES SKIP_BUILD_RPATH ON)

  # An embedder that changes directory before creating its first runtime,
  # for default resolution of a relatively launched executable.
  add_hermes_executable(intl_chdir_host fixtures/ChdirHost.cpp
    LINK_LIBS hermesvm_a)
  if(APPLE)
    target_link_options(intl_chdir_host PRIVATE -Wl,-export_dynamic)
  else()
    target_link_options(intl_chdir_host PRIVATE -rdynamic)
  endif()
  # A host that gets Hermes and JSI from libhermesvm (shared), exporting no
  # JSI of its own, so the fake plugin binds to libhermesvm's copy.
  add_hermes_executable(intl_shared_host fixtures/SharedHost.cpp)
  target_link_libraries(intl_shared_host PRIVATE hermesvm)

  set_property(GLOBAL APPEND PROPERTY HERMES_INTL_FIXTURE_TARGETS
    fake_intl_dep intl_chdir_host intl_shared_host)
endif()
```

In the top-level `CMakeLists.txt`, after `add_subdirectory(unittests)`:
```cmake
  get_property(_intl_fixture_targets GLOBAL PROPERTY HERMES_INTL_FIXTURE_TARGETS)
  list(APPEND HERMES_TEST_DEPS ${_intl_fixture_targets})
```
In `HERMES_LIT_TEST_PARAMS_BASE`, add:
```cmake
    intl_fixture_dir=${CMAKE_CURRENT_BINARY_DIR}/unittests/IntlLoader
```

`unittests/IntlLoader/fixtures/FakeIntlDep.cpp`:
```cpp
// (Meta MIT header)

// A library fake_intl_needs_dep depends on; the tests never make it findable.
extern "C" __attribute__((visibility("default"))) int fake_intl_dep() {
  return 1;
}
```
In `FakeIntlPlugin.cpp`, give the `fake_intl_needs_dep` variant a real use of it, so the linker keeps the dependency. Add this at the end of the file:
```cpp
#ifdef FAKE_NEEDS_DEP
extern "C" int fake_intl_dep();
extern "C" __attribute__((visibility("default"))) int fake_intl_use_dep() {
  return fake_intl_dep();
}
#endif
```
and change `add_fake_intl_plugin(fake_intl_needs_dep)` to `add_fake_intl_plugin(fake_intl_needs_dep FAKE_NEEDS_DEP=1)`.

`unittests/IntlLoader/fixtures/ChdirHost.cpp`:
```cpp
// (Meta MIT header)

// Changes to the directory in argv[1], then creates a runtime and reports
// whether the (fake) Intl plugin was installed by default resolution.

#include "hermes/hermes.h"

#include <cstdio>
#include <unistd.h>

int main(int argc, char **argv) {
  if (argc > 1 && ::chdir(argv[1]) != 0)
    return 2;
  auto rt = facebook::hermes::makeHermesRuntime();
  bool installed = rt->global().hasProperty(*rt, "__fakeIntlPlugin");
  std::printf("%s\n", installed ? "plugin" : "no plugin");
  return 0;
}
```
Set `HERMES_ENABLE_EH` and `HERMES_ENABLE_RTTI` (already ON in this CMakeLists).

`unittests/IntlLoader/fixtures/SharedHost.cpp`:
```cpp
// (Meta MIT header)

// Usage: intl_shared_host <plugin> <script>. Creates a runtime from
// libhermesvm with IntlPluginPath = <plugin>, evaluates <script>, and prints
// its completion value. Unlike the hermes CLI it does not export JSI, so the
// plugin binds to libhermesvm's copy.

#include "hermes/hermes.h"

#include <cstdio>
#include <fstream>
#include <iostream>
#include <sstream>

using namespace facebook;

int main(int argc, char **argv) {
  if (argc != 3)
    return 2;
  std::ifstream in(argv[2]);
  std::stringstream source;
  source << in.rdbuf();
  auto rt = facebook::hermes::makeHermesRuntime(
      ::hermes::vm::RuntimeConfig::Builder()
          .withIntlPluginPath(argv[1])
          .build());
  try {
    jsi::Value result = rt->evaluateJavaScript(
        std::make_shared<jsi::StringBuffer>(source.str()), argv[2]);
    std::cout << result.toString(*rt).utf8(*rt) << std::endl;
  } catch (const jsi::JSIException &e) {
    std::cout << "exception: " << e.what() << std::endl;
    return 1;
  }
  return 0;
}
```
Do not link `timerStats` or anything else that brings a static copy of JSI into this executable. Check afterwards with `nm -DC <build>/unittests/IntlLoader/intl_shared_host | grep -c "T facebook::jsi"`, which must print `0`.

`test/hermes/intl-plugin/shared-host.js`:
```js
/**
 * (Meta MIT header)
 */

// The binary contract with a shared-library host: the plugin binds to
// libhermesvm's JSI, a jsi::JSError thrown in the plugin reaches JS as that
// error, and NativeState works across the boundary.
// RUN: %intl_shared_host %intl_fixture_dir/libfake_intl_good.so %s | %FileCheck --match-full-lines %s

var out = [];
try {
  __fakeIntlThrow();
} catch (e) {
  out.push(e.message);
}
var o = {};
out.push(String(__fakeIntlRead(o)));
__fakeIntlAttach(o);
out.push(String(__fakeIntlRead(o)));
out.push(JSON.stringify(__fakeIntlPlugin.locales));
out.join(' ');
// CHECK: fake JSError -1 42 ["x-detected-1"]
```

Add the lit substitutions `%intl_chdir_host` → `<intl_fixture_dir>/intl_chdir_host` and `%intl_shared_host` → `<intl_fixture_dir>/intl_shared_host` in Step 3.

- [ ] **Step 3: Add the lit substitutions**

In `test/lit.cfg`, before the line `if lit_config.params.get("hermes"):` (a substitution must come before any substitution whose name is a prefix of it):
```python
# The hermes binary's bare path, for tests that copy it.
if lit_config.params.get("hermes"):
  config.substitutions.append(("%hermes_path", lit_config.params["hermes"].replace('\\', '/')))
```
After the `%napi_addon_dir` block:
```python
# Fake Intl plugin libraries (unittests/IntlLoader); not built on Windows.
if lit_config.params.get("intl_fixture_dir") and sys.platform != "win32":
  config.available_features.add("intl_plugin_fixtures")
  config.substitutions.append(("%intl_fixture_dir", lit_config.params["intl_fixture_dir"].replace('\\', '/')))
  config.substitutions.append(("%intl_library_name", "libhermesintl.dylib" if sys.platform == "darwin" else "libhermesintl.so"))
  config.substitutions.append(("%intl_chdir_host", lit_config.params["intl_fixture_dir"].replace('\\', '/') + "/intl_chdir_host"))
  config.substitutions.append(("%intl_shared_host", lit_config.params["intl_fixture_dir"].replace('\\', '/') + "/intl_shared_host"))
```
(`sys` is already imported in `test/lit.cfg`; check.)

`test/hermes/intl-plugin/lit.local.cfg`:
```python
# (Meta MIT header as # comments)
if "intl_plugin_fixtures" not in config.available_features:
    config.unsupported = True
```

- [ ] **Step 4: Write the lit tests**

`test/hermes/intl-plugin/explicit-path.js`:
```js
/**
 * (Meta MIT header)
 */

// RUN: %hermes -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_good.so %s | %FileCheck --match-full-lines %s
// RUN: %hermes -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_good.so -Xintl-default-locale=fr-FR -Xintl-default-locale=de -Xintl-default-time-zone=Europe/Paris %s | %FileCheck --match-full-lines --check-prefix=EXPLICIT %s
// RUN: %hermes -Xintl-plugin=false -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_good.so %s | %FileCheck --match-full-lines --check-prefix=DISABLED %s

// An explicit IntlPluginPath loads that library; disabling the plugin wins
// over the path. The fake also checks the binary contract: a jsi::JSError
// thrown in the plugin reaches JS as that error, and NativeState works
// across the boundary.

print(
  typeof __fakeIntlPlugin === 'object'
    ? JSON.stringify(__fakeIntlPlugin)
    : 'no plugin',
);
// CHECK: {"locales":["x-detected-1"],"timeZone":"Fake/Detected","detections":1}
// EXPLICIT: {"locales":["fr-FR","de"],"timeZone":"Europe/Paris","detections":0}
// DISABLED: no plugin
if (typeof __fakeIntlThrow === 'function') {
  try {
    __fakeIntlThrow();
  } catch (e) {
    print(e.message);
  }
  var o = {};
  print(__fakeIntlRead(o));
  __fakeIntlAttach(o);
  print(__fakeIntlRead(o));
}
// CHECK-NEXT: fake JSError
// CHECK-NEXT: -1
// CHECK-NEXT: 42
```

`test/hermes/intl-plugin/explicit-path-errors.js`:
```js
/**
 * (Meta MIT header)
 */

// Every failure with an explicit IntlPluginPath is fatal and names the
// library and the reason.

// RUN: not --crash %hermes -Xintl-plugin-path=%t.missing.so %s 2>&1 | %FileCheck --check-prefix=MISSING %s
// MISSING: LLVM ERROR: Intl plugin {{.*}}.missing.so: cannot be loaded:

// RUN: echo "not a library" > %t.text.so
// RUN: not --crash %hermes -Xintl-plugin-path=%t.text.so %s 2>&1 | %FileCheck --check-prefix=TEXT %s
// TEXT: LLVM ERROR: Intl plugin {{.*}}.text.so: cannot be loaded:

// RUN: not --crash %hermes -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_no_entry.so %s 2>&1 | %FileCheck --check-prefix=NOENTRY %s
// NOENTRY: LLVM ERROR: Intl plugin {{.*}}libfake_intl_no_entry.so: does not export hermes_intl_get_entry

// RUN: not --crash %hermes -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_bad_interface.so %s 2>&1 | %FileCheck --check-prefix=INTERFACE %s
// INTERFACE: LLVM ERROR: Intl plugin {{.*}}libfake_intl_bad_interface.so: plugin interface version {{[0-9]+}}, Hermes expects {{[0-9]+}}

// RUN: not --crash %hermes -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_bad_bytecode.so %s 2>&1 | %FileCheck --check-prefix=BYTECODE %s
// BYTECODE: LLVM ERROR: Intl plugin {{.*}}libfake_intl_bad_bytecode.so: bytecode version {{[0-9]+}}, Hermes expects {{[0-9]+}}

// RUN: not --crash %hermes -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_bad_flags.so %s 2>&1 | %FileCheck --check-prefix=FLAGS %s
// FLAGS: LLVM ERROR: Intl plugin {{.*}}libfake_intl_bad_flags.so: built with JSI_UNSTABLE

// RUN: not --crash %hermes -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_throwing.so %s 2>&1 | %FileCheck --check-prefix=THROWS %s
// THROWS: LLVM ERROR: Intl plugin {{.*}}libfake_intl_throwing.so: setup failed: fake plugin setup failure

// RUN: not --crash %hermes -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_rejects_defaults.so %s 2>&1 | %FileCheck --check-prefix=REJECTS %s
// REJECTS: LLVM ERROR: Intl plugin {{.*}}libfake_intl_rejects_defaults.so: rejected the default locale or time zone: fake plugin rejects every default

print('unreachable');
```

`test/hermes/intl-plugin/default-resolution.js`:
```js
/**
 * (Meta MIT header)
 */

// Without IntlPluginPath, hermes looks for the plugin next to itself, then in
// ../lib because it lives in a bin directory, then on the library search
// path. A copy of the binary in a scratch tree controls what it finds.

// RUN: rm -rf %t && mkdir -p %t/bin %t/lib %t/search
// RUN: ln -f %hermes_path %t/bin/hermes || cp %hermes_path %t/bin/hermes

// Nothing anywhere: no plugin, silently.
// RUN: env -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH %t/bin/hermes %s | %FileCheck --match-full-lines --check-prefix=NONE %s
// NONE: no plugin

// In ../lib.
// RUN: cp %intl_fixture_dir/libfake_intl_good.so %t/lib/%intl_library_name
// RUN: env -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH %t/bin/hermes %s | %FileCheck --check-prefix=FOUND %s
// FOUND: "detections":1

// Next to the binary comes first: a broken library in ../lib is not reached.
// RUN: cp %intl_fixture_dir/libfake_intl_good.so %t/bin/%intl_library_name
// RUN: echo "not a library" > %t/lib/%intl_library_name
// RUN: env -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH %t/bin/hermes %s | %FileCheck --check-prefix=FOUND %s

// A broken library that is found is fatal.
// RUN: rm %t/bin/%intl_library_name
// RUN: env -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH not --crash %t/bin/hermes %s 2>&1 | %FileCheck --check-prefix=BROKEN %s
// BROKEN: LLVM ERROR: Intl plugin {{.*}}lib/libhermesintl.{{so|dylib}}: cannot be loaded:

// On the search path.
// RUN: rm %t/lib/%intl_library_name
// RUN: cp %intl_fixture_dir/libfake_intl_good.so %t/search/%intl_library_name
// RUN: env LD_LIBRARY_PATH=%t/search DYLD_LIBRARY_PATH=%t/search %t/bin/hermes %s | %FileCheck --check-prefix=FOUND %s

// Broken on the search path is fatal too.
// RUN: echo "not a library" > %t/search/%intl_library_name
// RUN: env LD_LIBRARY_PATH=%t/search DYLD_LIBRARY_PATH=%t/search not --crash %t/bin/hermes %s 2>&1 | %FileCheck --check-prefix=SEARCHBROKEN %s
// SEARCHBROKEN: LLVM ERROR: Intl plugin libhermesintl.{{so|dylib}}: cannot be loaded:

// A plugin whose own dependency is missing is broken, not absent, both on
// the search path and in ../lib.
// RUN: cp %intl_fixture_dir/libfake_intl_needs_dep.so %t/search/%intl_library_name
// RUN: env LD_LIBRARY_PATH=%t/search DYLD_LIBRARY_PATH=%t/search not --crash %t/bin/hermes %s 2>&1 | %FileCheck --check-prefix=NEEDSDEP %s
// RUN: rm %t/search/%intl_library_name && cp %intl_fixture_dir/libfake_intl_needs_dep.so %t/lib/%intl_library_name
// RUN: env -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH not --crash %t/bin/hermes %s 2>&1 | %FileCheck --check-prefix=NEEDSDEP %s
// NEEDSDEP: LLVM ERROR: Intl plugin {{.*}}libhermesintl.{{so|dylib}}: cannot be loaded: {{.*}}fake_intl_dep
// RUN: rm %t/lib/%intl_library_name

// An embedder launched by a relative path that changes directory before
// creating its first runtime still finds ../lib next to its real location.
// RUN: cp %intl_chdir_host %t/bin/host
// RUN: cp %intl_fixture_dir/libfake_intl_good.so %t/lib/%intl_library_name
// RUN: cd %t && env -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH ./bin/host / | %FileCheck --check-prefix=CHDIR %s
// CHDIR: plugin

print(
  typeof __fakeIntlPlugin === 'object'
    ? JSON.stringify(__fakeIntlPlugin)
    : 'no plugin',
);
```

- [ ] **Step 5: Build and run them**

```
cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target hermes FileCheck not fake_intl_good fake_intl_no_entry fake_intl_bad_interface fake_intl_bad_bytecode fake_intl_bad_flags fake_intl_throwing fake_intl_rejects_defaults fake_intl_dep fake_intl_needs_dep intl_chdir_host intl_shared_host
LIT_FILTER=intl-plugin cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-hermes
```
Expected: the four `intl-plugin` tests pass, including the `CHDIR`, `NEEDSDEP` and binary-contract (`fake JSError`, `42`) checks in both the static host (`explicit-path.js`) and the shared-library host (`shared-host.js`). If the ASan runtime prints anything to stderr during a `not --crash` run, the `CHECK` lines still match because they only anchor on the `LLVM ERROR` line.

- [ ] **Step 6: Show the tests can fail**

Make each change below, run, see the named test fail, then revert it:
- In `findDefaultLibrary`, delete the `candidates.push_back(...)` line for `../lib`: `default-resolution.js` fails at the first `FOUND`.
- In `isLibraryNotFound` (the glibc branch), return `true` always: `default-resolution.js` fails at `SEARCHBROKEN`.
- Remove the export flags from the `hermes` binary: the new block in `add_hermes_tool` **and** the `-rdynamic` / `-Wl,-export_dynamic` that `tools/hermes/CMakeLists.txt` adds when `HERMES_ENABLE_NAPI` is on (the default). Check with `ninja -C /home/tmikov/work/hermes-intl/cmake-build-asan -t commands hermes | tail -1` that the link line has no `-rdynamic`, rebuild `hermes`: `explicit-path.js` fails because the fake cannot bind the out-of-line JSI symbols. Confirm the message says `undefined symbol`. Restore both.
- In `isMainExecutable`, return `false` always: `default-resolution.js` fails at `CHDIR`.
- In `isLibraryNotFound` (the glibc branch), compare only the prefix up to `: cannot open shared object file`: `default-resolution.js` still passes `NEEDSDEP` (the dependency's name comes first). Instead, make it `return error.find(": cannot open shared object file") != std::string::npos;`: `NEEDSDEP` fails on the search path.

- [ ] **Step 7: Check that the CLI exports JSI**

`nm -DC /home/tmikov/work/hermes-intl/cmake-build-asan/bin/hermes | grep -c "facebook::jsi::"`
Expected: a number above 1000.

- [ ] **Step 8: Run all of `check-hermes`**

`cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-hermes`
Expected: the same results as at the end of Task 3, plus the three new tests.

- [ ] **Step 9: Stage**

`git -C /home/tmikov/work/hermes-intl add cmake/modules/Hermes.cmake CMakeLists.txt test/lit.cfg test/hermes/intl-plugin unittests/IntlLoader`

### Task 5: Workers inherit the parent's Intl state

**Files:**
- Modify: `API/hermes/extensions/Worker.cpp:563` (`startWorker`)
- Modify: `unittests/IntlLoader/IntlLoaderTest.cpp` (append tests)
- Create: `test/hermes/intl-plugin/worker-inherits.js`, `test/hermes/intl-plugin/worker-inherits-disabled.js`

**Interfaces:**
- Consumes: `makeWorkerRuntimeConfig(jsi::Runtime &)` from Task 3; the fake plugins from Task 4.

- [ ] **Step 1: Write the failing tests**

Append to `unittests/IntlLoader/IntlLoaderTest.cpp`, inside the anonymous namespace:
```cpp
void workerConfigCopiesEffectiveDefaults() {
  registerEntry(goodEntry());
  auto parent = makeHermesRuntime();
  RuntimeConfig config = makeWorkerRuntimeConfig(*parent);
  bool ok = config.getEnableIntlPlugin() && config.getIntl() &&
      config.getIntlPluginPath().empty() &&
      config.getIntlDefaultLocales() ==
          std::vector<std::string>{"xx-detected"} &&
      config.getIntlDefaultTimeZone() == "Fake/Detected";
  // A worker's worker gets the same.
  auto worker = makeHermesRuntime(config);
  RuntimeConfig nested = makeWorkerRuntimeConfig(*worker);
  ok = ok && fakeLocale(*worker) == "xx-detected" &&
      nested.getIntlDefaultLocales() == config.getIntlDefaultLocales() &&
      nested.getIntlDefaultTimeZone() == config.getIntlDefaultTimeZone();
  std::exit(ok ? 0 : 1);
}
TEST_F(IntlLoaderTest, WorkerConfigCopiesEffectiveDefaults) {
  EXPECT_EXIT(
      workerConfigCopiesEffectiveDefaults(),
      ::testing::ExitedWithCode(0),
      "");
}

void workerConfigCopiesDisabledState() {
  registerEntry(goodEntry());
  auto parent = makeHermesRuntime(RuntimeConfig::Builder()
                                      .withEnableIntlPlugin(false)
                                      .withIntl(false)
                                      .build());
  RuntimeConfig config = makeWorkerRuntimeConfig(*parent);
  std::exit(!config.getEnableIntlPlugin() && !config.getIntl() ? 0 : 1);
}
TEST_F(IntlLoaderTest, WorkerConfigCopiesDisabledState) {
  EXPECT_EXIT(
      workerConfigCopiesDisabledState(), ::testing::ExitedWithCode(0), "");
}
```

`test/hermes/intl-plugin/worker-inherits.js`:
```js
/**
 * (Meta MIT header)
 */

// REQUIRES: jsi_unstable
// RUN: %hermes -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_good.so %s | %FileCheck --match-full-lines %s

// A Worker gets its parent's plugin and the defaults the parent's plugin
// resolved, without detecting again: the fake counts detections per process,
// and the worker still sees 1.

// Hermes does not keep function source, so the shared helper is a string.
var DESCRIBE =
  "function describe() { return typeof __fakeIntlPlugin === 'object' ? " +
  "JSON.stringify(__fakeIntlPlugin) : 'no plugin'; }";
(0, eval)(DESCRIBE);
print('parent ' + describe());
var worker = new Worker(DESCRIBE + "\npostMessage('worker ' + describe());");
worker.onmessage = function (msg) {
  print(msg);
};
// CHECK: parent {"locales":["x-detected-1"],"timeZone":"Fake/Detected","detections":1}
// CHECK-NEXT: worker {"locales":["x-detected-1"],"timeZone":"Fake/Detected","detections":1}
```

`test/hermes/intl-plugin/worker-inherits-disabled.js`:
```js
/**
 * (Meta MIT header)
 */

// REQUIRES: jsi_unstable
// The old built-in Intl would make `typeof Intl` differ for other reasons.
// UNSUPPORTED: intl
// RUN: %hermes -Xintl-plugin=false -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_good.so %s | %FileCheck --match-full-lines %s

// A parent without a plugin has workers without one, even when the default
// plugin (the real libhermesintl in the build tree) would be found.

// Hermes does not keep function source, so the shared helper is a string.
var DESCRIBE =
  "function describe() { return typeof __fakeIntlPlugin + ' ' + typeof Intl; }";
(0, eval)(DESCRIBE);
print('parent ' + describe());
var worker = new Worker(DESCRIBE + "\npostMessage('worker ' + describe());");
worker.onmessage = function (msg) {
  print(msg);
};
// CHECK: parent undefined undefined
// CHECK-NEXT: worker undefined undefined
```

- [ ] **Step 2: Run them and watch them fail**

```
cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target IntlLoaderTests hermes
/home/tmikov/work/hermes-intl/cmake-build-asan/unittests/IntlLoader/IntlLoaderTests --gtest_filter='*Worker*'
LIT_FILTER=worker-inherits cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-hermes
```
Expected:
- The gtests pass, because `makeWorkerRuntimeConfig` already exists.
- `worker-inherits.js` fails: the worker prints `worker no plugin`, since it is created with a default `RuntimeConfig` and the fake is not on any default path.
- `worker-inherits-disabled.js` passes for now: no real plugin exists yet. It becomes meaningful after Task 7, and Task 7 reruns it.

- [ ] **Step 3: Make Workers use the parent's state**

In `API/hermes/extensions/Worker.cpp`, add `#include "hermes/IntlPluginLoader.h"` after `#include "hermes/hermes.h"`, and in `startWorker` replace
```cpp
  auto workerRuntime = api->makeHermesRuntime(::hermes::vm::RuntimeConfig());
```
with
```cpp
  // The worker gets the parent's Intl: the same plugin and the defaults the
  // parent's plugin resolved, so it never detects them again.
  auto workerRuntime = api->makeHermesRuntime(makeWorkerRuntimeConfig(rt));
```

- [ ] **Step 4: Run the tests**

`LIT_FILTER='intl-plugin|worker' cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-hermes`
Expected: all `intl-plugin` and `worker` tests pass.

- [ ] **Step 5: Show the tests can fail**

Make each change below, run, see the named test fail, then revert it:
- In `makeWorkerRuntimeConfig`, drop `.withIntlDefaultLocales(...)`: `WorkerConfigCopiesEffectiveDefaults` fails, and `worker-inherits.js` fails with `x-detected-2`.
- Drop `.withEnableIntlPlugin(record->installed)`: `WorkerConfigCopiesDisabledState` fails.

- [ ] **Step 6: Stage**

`git -C /home/tmikov/work/hermes-intl add API/hermes/extensions/Worker.cpp unittests/IntlLoader/IntlLoaderTest.cpp test/hermes/intl-plugin`

### Task 6: Locale data and host normalization in the Rust core

**Files:**
- Create: `intl/core/src/locale.rs` (adapted from magrinj), `intl/core/src/host.rs`, `intl/core/src/supported.rs`
- Create: `intl/core/data/locales.txt`, `intl/core/data/currencies.txt`, `intl/core/data/numbering-systems.txt`, `intl/core/data/update-cldr-lists.py`
- Create: `intl/THIRD_PARTY_NOTICES.md`
- Modify: `intl/core/src/lib.rs` (the C functions)

**Interfaces:**
- Produces C functions. All take UTF-8 as `(const uint8_t *, size_t)`. The ones with an out-parameter write a `HiString` that the caller frees, or the empty value when they return `false`.
  - `bool hi_locale_canonicalize(const uint8_t *tag, size_t len, HiString *out)`: `IsStructurallyValidLanguageTag` followed by `CanonicalizeUnicodeLocaleId`.
  - `bool hi_locale_is_available(const uint8_t *tag, size_t len)`: the tag is canonical and has no extensions.
  - `bool hi_locale_from_host(const uint8_t *raw, size_t len, HiString *out)`: a POSIX or Apple locale name to a canonical BCP 47 tag.
  - `bool hi_time_zone_from_host(const uint8_t *raw, size_t len, HiString *out)`: a `TZ` value or `/etc/localtime` link target to a primary IANA name.
  - `bool hi_time_zone_canonicalize(const uint8_t *name, size_t len, HiString *out)`: a case-insensitive IANA name to its primary identifier.
  - `void hi_supported_values(HiValueKind kind, HiString *out)`: sorted, unique values, separated by single spaces.
  - `enum HiValueKind { HiValueKind_Calendar, HiValueKind_Collation, HiValueKind_Currency, HiValueKind_NumberingSystem, HiValueKind_TimeZone }`.

- [ ] **Step 1: Copy the locale list and write the notices**

```
S=/tmp/claude-1001/-home-tmikov-work-hermes/a918a33f-abe5-47f3-80d3-351885f648de/scratchpad/magrinj-hermes-intl
mkdir -p /home/tmikov/work/hermes-intl/intl/core/data
{ printf '# The locales ICU4X 2.3 compiled data covers (its "recommended" set),\n# one per line. From magrinj/hermes-intl rust/locales/recommended.txt;\n# see intl/THIRD_PARTY_NOTICES.md.\n'; cat $S/rust/locales/recommended.txt; } > /home/tmikov/work/hermes-intl/intl/core/data/locales.txt
```
If the scratch clone is gone, clone it again: `git clone https://github.com/magrinj/hermes-intl <dir>`.

`intl/THIRD_PARTY_NOTICES.md` names the files that adapt magrinj code (`intl/core/src/locale.rs` and `intl/core/data/locales.txt`), followed by the complete text of magrinj's `LICENSE`, copied verbatim.

- [ ] **Step 2: Write `update-cldr-lists.py` and generate the lists**

First find the CLDR version of ICU4X 2.3's data. The `## icu4x 2.3` section of ICU4X's `CHANGELOG.md` names it; if it does not, the newest earlier section that says "Updated to CLDR" does. Read it from the crate sources (`ls ~/.cargo/registry/src/*/ | grep '^icu_provider-2.3'`), or the clone at `$S/../icu4x/icu4x/CHANGELOG.md`. Expected: CLDR 48.2, whose tag is `release-48-2`.

`intl/core/data/update-cldr-lists.py`:
```python
#!/usr/bin/env python3
# (Meta MIT header)

"""Regenerate currencies.txt and numbering-systems.txt from CLDR.

Run it when the ICU4X version (and so its CLDR version) changes, and commit
the output. Usage: update-cldr-lists.py <cldr-release-tag>, e.g. release-48-2.
"""

import os
import sys
import urllib.request
import xml.etree.ElementTree as ET

BASE = "https://raw.githubusercontent.com/unicode-org/cldr/{tag}/common/{path}"
HERE = os.path.dirname(os.path.abspath(__file__))


def fetch(tag, path):
    with urllib.request.urlopen(BASE.format(tag=tag, path=path)) as r:
        return ET.fromstring(r.read())


def write(name, tag, comment, values):
    with open(os.path.join(HERE, name), "w") as f:
        f.write(f"# {comment}\n# Generated by update-cldr-lists.py from CLDR {tag}.\n")
        for v in sorted(set(values)):
            f.write(v + "\n")


def main():
    tag = sys.argv[1]
    currency = fetch(tag, "bcp47/currency.xml")
    write(
        "currencies.txt",
        tag,
        "ISO 4217 codes CLDR knows (bcp47/currency.xml, key cu).",
        [t.get("name").upper() for t in currency.iter("type")],
    )
    numbering = fetch(tag, "supplemental/numberingSystems.xml")
    write(
        "numbering-systems.txt",
        tag,
        "Numbering systems with decimal digits (numberingSystems.xml, type numeric).",
        [n.get("id") for n in numbering.iter("numberingSystem") if n.get("type") == "numeric"],
    )


if __name__ == "__main__":
    main()
```
Run it: `python3 /home/tmikov/work/hermes-intl/intl/core/data/update-cldr-lists.py release-48-2`
Expected: `currencies.txt` has about 300 codes and `numbering-systems.txt` about 70 ids.

- [ ] **Step 3: Write `locale.rs` with its tests**

`intl/core/src/locale.rs`: header lines as in Global Constraints, including the magrinj line.
```rust
//! Locale identifiers: well-formedness, canonicalization (ECMA-402
//! IsStructurallyValidLanguageTag and CanonicalizeUnicodeLocaleId) and the
//! available-locale set.

use icu::locale::{LanguageIdentifier, Locale, LocaleCanonicalizer, LocaleExpander};
use std::sync::OnceLock;
```
Then copy from `$S/rust/src/locale.rs`, keeping the code and adjusting only the `use` lines:
- `canonicalize`;
- `canonicalize_keyword_values`;
- `expander`.

Leave out `lookup`, `default_locale`, `maximize`, `minimize`, `direction`, `week_info`, `default_numbering_system`, `numbering_system_supported`, `time_zones` and the `baked` module; later phases take them as needed. Then add:
```rust
/// The locales the compiled data covers, canonical and sorted.
pub fn available() -> &'static [String] {
    static LIST: OnceLock<Vec<String>> = OnceLock::new();
    LIST.get_or_init(|| {
        let mut list: Vec<String> = include_str!("../data/locales.txt")
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| {
                canonicalize(l)
                    .unwrap_or_else(|| panic!("data/locales.txt: {l:?} is not well-formed"))
            })
            .collect();
        list.sort();
        list.dedup();
        list
    })
}

/// Whether `tag` (canonical, without extensions) is an available locale.
/// CLDR stores a language's likely-subtag form as the bare language ("en"
/// holds en-US data), so that form is available whenever the language is,
/// as in ICU's available-locale lists.
pub fn is_available(tag: &str) -> bool {
    let in_list = |t: &str| available().binary_search_by(|a| a.as_str().cmp(t)).is_ok();
    if in_list(tag) {
        return true;
    }
    let Ok(loc) = tag.parse::<Locale>() else {
        return false;
    };
    if !loc.extensions.is_empty() || !loc.id.variants.is_empty() || loc.id.region.is_none() {
        return false;
    }
    if !in_list(loc.id.language.as_str()) {
        return false;
    }
    let mut max = LanguageIdentifier::from(loc.id.language);
    expander().maximize(&mut max);
    max.region == loc.id.region && loc.id.script.is_none_or(|s| Some(s) == max.script)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes() {
        assert_eq!(canonicalize("EN-us").as_deref(), Some("en-US"));
        assert_eq!(canonicalize("iw").as_deref(), Some("he"));
        assert_eq!(canonicalize("en-u-ca-islamicc").as_deref(), Some("en-u-ca-islamic-civil"));
        assert_eq!(canonicalize("en-u-kn-true").as_deref(), Some("en-u-kn"));
        assert_eq!(canonicalize("de-u-co-phonebk-ka-shifted").as_deref(), Some("de-u-co-phonebk-ka-shifted"));
        assert_eq!(canonicalize("posix").as_deref(), Some("posix"));
    }

    #[test]
    fn rejects_ill_formed_tags() {
        for bad in ["", "en_US", "en-", "-en", "x-private", "en-US-u", "a", "en--US", "toolonglanguage"] {
            assert_eq!(canonicalize(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn availability() {
        assert!(is_available("en"));
        assert!(is_available("en-US"));
        assert!(is_available("sr-Cyrl-RS"));
        assert!(!is_available("en-AQ"));
        assert!(!is_available("tlh"));
        assert!(!is_available("en-u-ca-gregory"));
    }

    #[test]
    fn available_list_is_canonical() {
        assert!(available().len() > 400);
        for tag in available() {
            assert_eq!(canonicalize(tag).as_deref(), Some(tag.as_str()));
        }
    }
}
```
If ICU4X accepts any tag in `rejects_ill_formed_tags`, check it against test262 (`intl402/Intl/getCanonicalLocales/invalid-tags.js` and `test/intl402/language-tags-invalid.js` in the pinned test262). If test262 also treats it as invalid, fix `canonicalize`. If the tag is actually valid, remove it from the list and say so in the report.

- [ ] **Step 4: Write `host.rs` with its tests**

`intl/core/src/host.rs`:
```rust
// (Meta MIT header)

//! Turns the host's locale and time zone settings into ECMA-402 values. See
//! "Default locale and time zone" in the spec.

use crate::locale::canonicalize;
use icu::time::zone::iana::IanaParserExtended;
use icu::time::TimeZone;

/// A POSIX locale name (lang[_TERRITORY][.codeset][@modifier]) or an Apple
/// preferred-language string to a canonical BCP 47 tag; None if it is not
/// usable, so the caller tries its next source.
pub fn locale_from_host(raw: &str) -> Option<String> {
    let (base, modifier) = match raw.split_once('@') {
        Some((base, modifier)) => (base, Some(modifier)),
        None => (raw, None),
    };
    let base = base.split('.').next().unwrap_or("");
    if base == "C" || base == "POSIX" {
        return Some("en-US".to_owned());
    }
    if base.is_empty() {
        return None;
    }
    let mut subtags: Vec<String> = base.split(['_', '-']).map(str::to_owned).collect();
    match modifier.map(str::to_ascii_lowercase).as_deref() {
        // The script modifiers glibc's locales use (/usr/share/i18n/SUPPORTED
        // lists latin, cyrillic, devanagari and iqtelif; its other modifiers
        // are @euro, @valencia and @abegede, a Ge'ez collation order).
        Some("latin") => insert_script(&mut subtags, "Latn"),
        Some("cyrillic") => insert_script(&mut subtags, "Cyrl"),
        Some("devanagari") => insert_script(&mut subtags, "Deva"),
        // Tatar in the Latin IQTElif alphabet (glibc tt_RU@iqtelif).
        Some("iqtelif") => insert_script(&mut subtags, "Latn"),
        Some("valencia") => subtags.push("valencia".to_owned()),
        // @euro and unknown modifiers carry no locale information.
        _ => {}
    }
    canonicalize(&subtags.join("-"))
}

/// Insert `script` after the language unless the tag already has a script.
fn insert_script(subtags: &mut Vec<String>, script: &str) {
    let has_script = subtags
        .get(1)
        .is_some_and(|s| s.len() == 4 && s.bytes().all(|b| b.is_ascii_alphabetic()));
    if !has_script {
        subtags.insert(1, script.to_owned());
    }
}

/// A `TZ` value or the target of the /etc/localtime link to a primary IANA
/// time zone name; None for POSIX rule strings, unknown zones, and paths
/// outside a zoneinfo directory.
pub fn time_zone_from_host(raw: &str) -> Option<String> {
    let name = raw.strip_prefix(':').unwrap_or(raw);
    let name = match name.rfind("zoneinfo/") {
        Some(i) => &name[i + "zoneinfo/".len()..],
        None => name,
    };
    if name.is_empty() || name.starts_with('/') || name.starts_with('.') {
        return None;
    }
    time_zone_canonicalize(name)
}

/// A case-insensitive IANA time zone name to its primary identifier; None if
/// the data does not know it.
pub fn time_zone_canonicalize(name: &str) -> Option<String> {
    let parsed = IanaParserExtended::new().parse(name);
    if parsed.time_zone == TimeZone::UNKNOWN {
        return None;
    }
    Some(primary_time_zone(parsed.canonical).to_owned())
}

/// ECMA-402 names UTC "UTC", although IANA's canonical name is Etc/UTC
/// (AvailableNamedTimeZoneIdentifiers).
pub fn primary_time_zone(canonical: &str) -> &str {
    match canonical {
        "Etc/UTC" | "Etc/GMT" | "GMT" => "UTC",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_locales() {
        let cases = [
            ("en_US.UTF-8", Some("en-US")),
            ("en_US.ISO-8859-1", Some("en-US")),
            ("sr_RS.UTF-8@latin", Some("sr-Latn-RS")),
            ("sr_RS@latin", Some("sr-Latn-RS")),
            ("sr_Latn_RS@latin", Some("sr-Latn-RS")),
            ("uz_UZ@cyrillic", Some("uz-Cyrl-UZ")),
            ("ks_IN@devanagari", Some("ks-Deva-IN")),
            ("tt_RU.UTF-8@iqtelif", Some("tt-Latn-RU")),
            ("gez_ET@abegede", Some("gez-ET")),
            ("ca_ES.UTF-8@valencia", Some("ca-ES-valencia")),
            ("de_DE@euro", Some("de-DE")),
            ("C", Some("en-US")),
            ("C.UTF-8", Some("en-US")),
            ("POSIX", Some("en-US")),
            ("iw_IL", Some("he-IL")),
            ("zh-Hans-CN", Some("zh-Hans-CN")),
            ("", None),
            ("garbage!!", None),
            (".UTF-8", None),
        ];
        for (raw, want) in cases {
            assert_eq!(locale_from_host(raw).as_deref(), want, "{raw:?}");
        }
    }

    #[test]
    fn host_time_zones() {
        let cases = [
            ("Europe/Paris", Some("Europe/Paris")),
            (":Europe/Paris", Some("Europe/Paris")),
            ("europe/paris", Some("Europe/Paris")),
            ("/usr/share/zoneinfo/Asia/Calcutta", Some("Asia/Kolkata")),
            ("../usr/share/zoneinfo/America/New_York", Some("America/New_York")),
            ("/var/db/timezone/zoneinfo/Europe/Berlin", Some("Europe/Berlin")),
            ("UTC", Some("UTC")),
            ("Etc/UTC", Some("UTC")),
            ("GMT", Some("UTC")),
            ("EST5EDT,M3.2.0,M11.1.0", None),
            ("", None),
            ("/etc/foo", None),
            ("Mars/Olympus_Mons", None),
        ];
        for (raw, want) in cases {
            assert_eq!(time_zone_from_host(raw).as_deref(), want, "{raw:?}");
        }
    }
}
```

- [ ] **Step 5: Write `supported.rs` with its tests**

`intl/core/src/supported.rs`:
```rust
// (Meta MIT header)

//! The lists behind Intl.supportedValuesOf. Each value must also be accepted
//! by the constructor that uses it once that constructor exists (test262
//! checks this): calendars by DateTimeFormat, collations by Collator, and so
//! on. The "unit" list is the ECMA-402 table and lives in JS.

use crate::host::primary_time_zone;
use icu::time::zone::iana::IanaParserExtended;
use icu::time::TimeZone;
use icu_provider::prelude::*;
use std::sync::OnceLock;

/// Which list; matches HiValueKind in lib.rs.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Calendar,
    Collation,
    Currency,
    NumberingSystem,
    TimeZone,
}

/// The calendars ICU4X implements, as CLDR calendar ids. "islamic" and
/// "islamic-rgsa" are left out: ICU4X has no algorithm for them.
const CALENDARS: &[&str] = &[
    "buddhist", "chinese", "coptic", "dangi", "ethioaa", "ethiopic", "gregory", "hebrew",
    "indian", "islamic-civil", "islamic-tbla", "islamic-umalqura", "iso8601", "japanese",
    "persian", "roc",
];

/// The collation types ICU4X knows, without "standard" and "search", which
/// ECMA-402 excludes, and "ducet", which no locale's data provides.
const COLLATIONS: &[&str] = &[
    "compat", "dict", "emoji", "eor", "phonebk", "phonetic", "pinyin", "searchjl", "stroke",
    "trad", "unihan", "zhuyin",
];

/// The sorted, unique values of `kind`.
pub fn values(kind: Kind) -> Vec<String> {
    let mut list: Vec<String> = match kind {
        Kind::Calendar => CALENDARS.iter().map(|s| s.to_string()).collect(),
        Kind::Collation => COLLATIONS.iter().map(|s| s.to_string()).collect(),
        Kind::Currency => data_lines(include_str!("../data/currencies.txt")),
        Kind::NumberingSystem => numbering_systems().clone(),
        Kind::TimeZone => IanaParserExtended::new()
            .iter()
            .filter(|z| z.time_zone != TimeZone::UNKNOWN)
            .map(|z| primary_time_zone(z.canonical).to_owned())
            .collect(),
    };
    list.sort();
    list.dedup();
    list
}

/// The non-comment lines of a data file.
fn data_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

/// The CLDR numeric numbering systems that ICU4X has digits for.
fn numbering_systems() -> &'static Vec<String> {
    static LIST: OnceLock<Vec<String>> = OnceLock::new();
    LIST.get_or_init(|| {
        data_lines(include_str!("../data/numbering-systems.txt"))
            .into_iter()
            .filter(|nu| has_digits(nu))
            .collect()
    })
}

/// Whether ICU4X's data has decimal digits for numbering system `nu`.
/// Adapted from magrinj/hermes-intl (numbering_system_supported).
fn has_digits(nu: &str) -> bool {
    use icu::decimal::provider::{Baked, DecimalDigitsV1};
    let Ok(attrs) = DataMarkerAttributes::try_from_str(nu) else {
        return false;
    };
    let mut metadata = DataRequestMetadata::default();
    metadata.silent = true;
    DataProvider::<DecimalDigitsV1>::load(
        &Baked,
        DataRequest { id: DataIdentifierBorrowed::for_marker_attributes(attrs), metadata },
    )
    .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendars_are_icu4x_calendars() {
        use icu::calendar::preferences::CalendarAlgorithm;
        use icu::calendar::AnyCalendarKind;
        use icu::locale::extensions::unicode::Value;
        for id in CALENDARS {
            let value = Value::try_from_str(id).unwrap();
            let algorithm = CalendarAlgorithm::try_from(&value).unwrap();
            assert!(AnyCalendarKind::try_from(algorithm).is_ok(), "{id}");
        }
        // The check can fail: ICU4X has no algorithm for plain "islamic".
        let islamic = CalendarAlgorithm::try_from(&Value::try_from_str("islamic").unwrap()).unwrap();
        assert!(AnyCalendarKind::try_from(islamic).is_err());
    }

    /// Load the collation tailoring for (`co`, `locale`) without fallback
    /// warnings.
    fn tailoring_loads(co: &str, locale: &str) -> bool {
        use icu::collator::provider::{Baked, CollationTailoringV1};
        let loc: icu::locale::Locale = locale.parse().unwrap();
        let data_locale = DataLocale::from(&loc);
        let mut metadata = DataRequestMetadata::default();
        metadata.silent = true;
        DataProvider::<CollationTailoringV1>::load(
            &Baked,
            DataRequest {
                id: DataIdentifierBorrowed::for_marker_attributes_and_locale(
                    DataMarkerAttributes::try_from_str(co).unwrap(),
                    &data_locale,
                ),
                metadata,
            },
        )
        .is_ok()
    }

    #[test]
    fn collations_have_data() {
        let cases = [
            ("compat", "ar"), ("dict", "si"), ("emoji", "und"), ("eor", "und"),
            ("phonebk", "de"), ("phonetic", "ln"), ("pinyin", "zh"), ("searchjl", "ko"),
            ("stroke", "zh"), ("trad", "es"), ("unihan", "zh"), ("zhuyin", "zh"),
        ];
        assert_eq!(cases.len(), COLLATIONS.len());
        for (co, locale) in cases {
            assert!(tailoring_loads(co, locale), "{co} for {locale}");
        }
        // The check can fail: there is no such collation.
        assert!(!tailoring_loads("zzzz", "de"));
    }

    #[test]
    fn currencies_are_well_formed() {
        let list = values(Kind::Currency);
        assert!(list.len() > 150);
        for c in &list {
            assert!(c.len() == 3 && c.bytes().all(|b| b.is_ascii_uppercase()), "{c}");
        }
        for c in ["EUR", "JPY", "USD"] {
            assert!(list.iter().any(|x| x == c), "{c}");
        }
    }

    #[test]
    fn numbering_systems_have_digits() {
        let list = values(Kind::NumberingSystem);
        // ECMA-402's table of numbering systems with simple digit mappings.
        for nu in [
            "adlm", "ahom", "arab", "arabext", "bali", "beng", "bhks", "brah", "cakm", "cham",
            "deva", "diak", "fullwide", "gong", "gonm", "gujr", "guru", "hanidec", "hmng",
            "hmnp", "java", "kali", "kawi", "khmr", "knda", "lana", "lanatham", "laoo", "latn",
            "lepc", "limb", "mathbold", "mathdbl", "mathmono", "mathsanb", "mathsans", "mlym",
            "modi", "mong", "mroo", "mtei", "mymr", "mymrshan", "mymrtlng", "nagm", "newa",
            "nkoo", "olck", "orya", "osma", "rohg", "saur", "segment", "shrd", "sind", "sinh",
            "sora", "sund", "takr", "talu", "tamldec", "telu", "thai", "tibt", "tirh", "tnsa",
            "vaii", "wara", "wcho",
        ] {
            assert!(list.iter().any(|x| x == nu), "{nu}");
        }
        assert!(!has_digits("wxyz"));
    }

    #[test]
    fn time_zones_use_primary_identifiers() {
        let list = values(Kind::TimeZone);
        for z in ["UTC", "Asia/Kolkata", "Europe/Kyiv", "America/New_York"] {
            assert!(list.iter().any(|x| x == z), "{z}");
        }
        for z in ["Etc/UTC", "Etc/Unknown", "Asia/Calcutta"] {
            assert!(!list.iter().any(|x| x == z), "{z}");
        }
    }
}
```
Check the ECMA-402 table of numbering systems with simple digit mappings, in the draft you are following, against the list in `numbering_systems_have_digits`. Use the draft's list.

If `collations_have_data` fails for a pair, use ICU4X's CLDR data to find out which locale carries that tailoring. If none does, remove the collation from `COLLATIONS` and from the test, and record it in the report.

If `zzzz` loads (the baked provider falls back silently), the test proves nothing. Stop and report it; do not delete the assertion.

- [ ] **Step 6: Export the C functions**

Add to `intl/core/src/lib.rs`, after `hi_core_version`:
```rust
mod host;
mod locale;
mod supported;

/// View (data, len) as UTF-8; None if it is not.
///
/// # Safety
/// `data` must point to `len` readable bytes, or `len` must be 0.
unsafe fn as_str<'a>(data: *const u8, len: usize) -> Option<&'a str> {
    if len == 0 {
        return Some("");
    }
    std::str::from_utf8(std::slice::from_raw_parts(data, len)).ok()
}

/// Write `result` to `out` and say whether there was one.
///
/// # Safety
/// `out` must be valid for writes.
unsafe fn write_out(out: *mut HiString, result: Option<String>) -> bool {
    match result {
        Some(s) => {
            *out = HiString::from_string(s);
            true
        }
        None => {
            *out = HiString::EMPTY;
            false
        }
    }
}

/// IsStructurallyValidLanguageTag and CanonicalizeUnicodeLocaleId.
///
/// # Safety
/// `tag` must point to `len` readable bytes; `out` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn hi_locale_canonicalize(tag: *const u8, len: usize, out: *mut HiString) -> bool {
    write_out(out, as_str(tag, len).and_then(locale::canonicalize))
}

/// Whether `tag` (canonical, without extensions) is an available locale.
///
/// # Safety
/// `tag` must point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn hi_locale_is_available(tag: *const u8, len: usize) -> bool {
    as_str(tag, len).is_some_and(locale::is_available)
}

/// A host locale name to a canonical BCP 47 tag.
///
/// # Safety
/// As hi_locale_canonicalize.
#[no_mangle]
pub unsafe extern "C" fn hi_locale_from_host(raw: *const u8, len: usize, out: *mut HiString) -> bool {
    write_out(out, as_str(raw, len).and_then(host::locale_from_host))
}

/// A TZ value or /etc/localtime link target to a primary IANA name.
///
/// # Safety
/// As hi_locale_canonicalize.
#[no_mangle]
pub unsafe extern "C" fn hi_time_zone_from_host(raw: *const u8, len: usize, out: *mut HiString) -> bool {
    write_out(out, as_str(raw, len).and_then(host::time_zone_from_host))
}

/// A case-insensitive IANA name to its primary identifier.
///
/// # Safety
/// As hi_locale_canonicalize.
#[no_mangle]
pub unsafe extern "C" fn hi_time_zone_canonicalize(name: *const u8, len: usize, out: *mut HiString) -> bool {
    write_out(out, as_str(name, len).and_then(host::time_zone_canonicalize))
}

/// Which list hi_supported_values returns.
#[repr(C)]
pub enum HiValueKind {
    Calendar,
    Collation,
    Currency,
    NumberingSystem,
    TimeZone,
}

/// The values for Intl.supportedValuesOf, sorted, separated by single spaces.
///
/// # Safety
/// `out` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn hi_supported_values(kind: HiValueKind, out: *mut HiString) {
    let kind = match kind {
        HiValueKind::Calendar => supported::Kind::Calendar,
        HiValueKind::Collation => supported::Kind::Collation,
        HiValueKind::Currency => supported::Kind::Currency,
        HiValueKind::NumberingSystem => supported::Kind::NumberingSystem,
        HiValueKind::TimeZone => supported::Kind::TimeZone,
    };
    *out = HiString::from_string(supported::values(kind).join(" "));
}
```
Add to the `tests` module in `lib.rs`:
```rust
    #[test]
    fn ffi_rejects_invalid_utf8() {
        let bad = [0xffu8, 0xfe];
        let mut out = HiString::EMPTY;
        assert!(!unsafe { hi_locale_canonicalize(bad.as_ptr(), bad.len(), &mut out) });
        assert!(out.data.is_null());
    }
```
The ICU4X paths in this step and in Step 5 (`icu::calendar::preferences::CalendarAlgorithm`, `icu::collator::provider::Baked`, `DataIdentifierBorrowed::for_marker_attributes_and_locale`, and so on) were read from ICU4X main. If 2.3.1 names something differently, find the 2.3.1 name in `~/.cargo/registry/src/*/icu_*-2.3*` and keep the behavior.

- [ ] **Step 7: Run the tests**

`cargo test --locked --manifest-path /home/tmikov/work/hermes-intl/intl/core/Cargo.toml`, then `rm -rf /home/tmikov/work/hermes-intl/intl/core/target`.
Expected: every test passes.

- [ ] **Step 8: Show the tests can fail**

Make each change below, run, see the named test fail, then revert it:
- In `insert_script`, insert unconditionally: `host_locales` fails on `sr_Latn_RS@latin`.
- In `primary_time_zone`, return `canonical` always: `host_time_zones` and `time_zones_use_primary_identifiers` fail.
- In `is_available`, `return in_list(tag)`: `availability` fails on `en-US`.

- [ ] **Step 9: Rebuild through CMake and check the header**

```
cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target hermesIntlCoreBuild
grep -c "hi_locale_canonicalize\|hi_locale_is_available\|hi_locale_from_host\|hi_time_zone_from_host\|hi_time_zone_canonicalize\|hi_supported_values\|HiValueKind_TimeZone" /home/tmikov/work/hermes-intl/cmake-build-asan/intl/include/hermes_intl_core.h
```
Expected: `7`.

- [ ] **Step 10: Stage**

`git -C /home/tmikov/work/hermes-intl add intl/core intl/THIRD_PARTY_NOTICES.md`

### Task 7: The plugin library, with an `Intl` object, and `check-intl`

**Files:**
- Create: `intl/js/CMakeLists.txt`, `intl/js/00-header.js`, `intl/js/01-intrinsics.js`, `intl/js/02-shapes.js`, `intl/js/10-Intl.js`, `intl/js/99-footer.js`
- Create: `intl/jsi/Plugin.cpp`, `intl/jsi/IntlBytecode.cpp`, `intl/jsi/NativeHelpers.def`, `intl/jsi/NativeHelpers.h`, `intl/jsi/NativeHelpers.cpp`, `intl/jsi/LocaleHelpers.cpp`, `intl/jsi/RustCore.h`, `intl/jsi/Defaults.h`, `intl/jsi/Defaults.cpp`
- Create: `intl/test/lit/lit.cfg`, `intl/test/lit/plugin-loads.js`, `intl/test/lit/hosts.js`, `intl/test/lit/exports.js`
- Modify: `intl/CMakeLists.txt` (append)

**Interfaces:**
- Consumes:
  - `IHermesBuiltinBytecode` (Task 2);
  - `hermes_intl_entry` / `hermes_intl_config` (Task 3);
  - the C functions of `hermes_intl_core.h` (Tasks 1 and 6).
- Produces:
  - `<build>/lib/libhermesintl.so`, exporting only `hermes_intl_get_entry`.
  - JS globals inside the IIFE, used by Tasks 9 and 10: `native`, `hostDefaults`, `global`, `ObjectDefineProperty`, `SymbolToStringTag`, `setToStringTag(obj, tag)`, `installIntl()`, `IntlObject`.
  - `NativeHelpers.def`, which later phases extend one line per helper. The tags are `Bool`, `Utf8`, `OptUtf8`, and the C++ signature is `HelperSignature<Result, Args...>`.
  - `hermes_intl::resolveDefaults(hermes_intl_config *)`, which Task 8 extends.
  - The CMake targets `hermesintl`, `check-intl-lit`, `check-intl-rust` and `check-intl`.
  - Lit substitutions for `intl/test/lit`:
    - `%hermes`: the CLI with `-Xintl-plugin-path=<the plugin>`;
    - `%hermes_default`: the CLI with no Intl flags;
    - `%hermes_jsi_shared`, `%hermesc`, `%FileCheck`, `%intl_plugin` and `%intl_js` (the `intl/js` source directory).

- [ ] **Step 1: Write the lit tests**

`intl/test/lit/lit.cfg`:
```python
# (Meta MIT header as # comments)

# Lit tests for the Intl plugin: the real libhermesintl, and the JS layer's
# shared operations in isolation. Run by check-intl, not check-hermes.

import os
import platform

import lit.formats

config.name = "Hermes-Intl"
config.test_format = lit.formats.ShTest(execute_external=False)
config.suffixes = [".js"]
config.excludes = ["Inputs"]
config.test_source_root = os.path.dirname(__file__)

params = lit_config.params
config.test_exec_root = params["intl_test_exec_root"]
plugin = params["intl_plugin"]

# A substitution must come before any whose name is its prefix.
config.substitutions.append(("%hermes_jsi_shared", params["hermes_jsi_shared"]))
config.substitutions.append(("%hermes_default", params["hermes"]))
config.substitutions.append(("%hermesc", params["hermesc"]))
config.substitutions.append(("%hermes", params["hermes"] + " -Xintl-plugin-path=" + plugin))
config.substitutions.append(("%FileCheck", params["FileCheck"]))
config.substitutions.append(("%intl_plugin", plugin))
config.substitutions.append(("%intl_js", params["intl_js_dir"]))

if params.get("jsi_unstable", "").upper() in ("1", "ON", "TRUE", "YES"):
    config.available_features.add("jsi_unstable")
if platform.system() == "Linux":
    config.available_features.add("linux")

# `not` and the other lit helpers live next to the Hermes tools.
config.environment["PATH"] = os.pathsep.join(
    [params["tools_dir"], config.environment.get("PATH", os.environ["PATH"])]
)
for var in ("ASAN_OPTIONS", "UBSAN_OPTIONS", "LSAN_OPTIONS", "HOME", "TMPDIR"):
    if var in os.environ:
        config.environment[var] = os.environ[var]
```
Compare it with `test/lit.cfg`'s environment handling (PATH and sanitizer variables) and match what that file does, if it does more.

`intl/test/lit/plugin-loads.js`:
```js
/**
 * (Meta MIT header)
 */

// RUN: %hermes %s | %FileCheck --match-full-lines %s
// RUN: %hermes_default %s | %FileCheck --match-full-lines %s

// The plugin installs Intl, with the property attributes ECMA-262 gives the
// global object's value properties, and Intl's @@toStringTag.

var d = Object.getOwnPropertyDescriptor(globalThis, 'Intl');
print(typeof d.value, d.writable, d.enumerable, d.configurable);
// CHECK: object true false true
print(Object.getPrototypeOf(Intl) === Object.prototype);
// CHECK-NEXT: true
print(Object.prototype.toString.call(Intl));
// CHECK-NEXT: [object Intl]
var t = Object.getOwnPropertyDescriptor(Intl, Symbol.toStringTag);
print(t.value, t.writable, t.enumerable, t.configurable);
// CHECK-NEXT: Intl false false true
```

`intl/test/lit/hosts.js`:
```js
/**
 * (Meta MIT header)
 */

// The plugin binds to JSI in a static host (the hermes CLI, which exports its
// JSI) and in a shared-library host (hermes-jsi-shared, which gets JSI from
// libhermesvm), each finding it by default resolution in the build tree.

// RUN: %hermes_default %s | %FileCheck --match-full-lines %s
// RUN: %hermes_jsi_shared --eval "Object.prototype.toString.call(Intl)" --print | %FileCheck --match-full-lines %s
print(Object.prototype.toString.call(Intl));
// CHECK: [object Intl]
```

`intl/test/lit/exports.js`:
```js
/**
 * (Meta MIT header)
 */

// REQUIRES: linux
// The plugin exports its entry point and nothing from the Rust core. nm
// sorts by name, so the NOT lines cover everything before and after it.
// RUN: nm -D --defined-only %intl_plugin > %t.syms
// RUN: %FileCheck %s < %t.syms
// CHECK-NOT: {{ hi_}}
// CHECK: hermes_intl_get_entry
// CHECK-NOT: {{ hi_}}
```

- [ ] **Step 2: Write the JS**

`intl/js/00-header.js`:
```js
/**
 * (Meta MIT header)
 */

// The Hermes Intl plugin's ECMA-402 implementation. 00-header.js and
// 99-footer.js wrap the other files (concatenated in name order) in one
// function scope, so nothing defined here is reachable from user code. The
// completion value of the compiled bytecode is the setup function in
// 99-footer.js, which intl/jsi/Plugin.cpp calls once per runtime.
//
// Follow the "Plugin JS rules" in
// doc/superpowers/plans/2026-10-22-intl-plugin-phase0.md: captured
// intrinsics only, no spread or for-of, listPush instead of indexed stores,
// null-prototype descriptors.
(function () {
'use strict';

/// The native helpers (intl/jsi/NativeHelpers.def); set by setup.
var native;
/// {locales, timeZone}: the defaults the loader resolved; set by setup.
var hostDefaults;
```

`intl/js/01-intrinsics.js`:
```js
/**
 * (Meta MIT header)
 */

// Every intrinsic the implementation uses, captured when the plugin's
// bytecode runs, before any user code.

var global = globalThis;
var ObjectDefineProperty = Object.defineProperty;
var SymbolToStringTag = Symbol.toStringTag;
```

`intl/js/02-shapes.js`:
```js
/**
 * (Meta MIT header)
 */

// The function-shape rules (spec "Function shapes"). Everything the plugin
// installs goes through these helpers.

/// Define obj[@@toStringTag] = tag: non-writable, non-enumerable,
/// configurable.
function setToStringTag(obj, tag) {
  ObjectDefineProperty(obj, SymbolToStringTag, {
    __proto__: null,
    value: tag,
    writable: false,
    enumerable: false,
    configurable: true,
  });
}
```

`intl/js/10-Intl.js`:
```js
/**
 * (Meta MIT header)
 */

// ECMA-402 8: The Intl Object. Created when the bytecode runs, so its
// [[Prototype]] is the intrinsic %Object.prototype%.
var IntlObject = {};

/// Install Intl on the global object; called once by setup.
function installIntl() {
  // 8.1.1 Intl[@@toStringTag]
  setToStringTag(IntlObject, 'Intl');
  // ECMA-262 19.1: the global object's value properties are writable,
  // non-enumerable, configurable.
  ObjectDefineProperty(global, 'Intl', {
    __proto__: null,
    value: IntlObject,
    writable: true,
    enumerable: false,
    configurable: true,
  });
}
```

`intl/js/99-footer.js`:
```js
/**
 * (Meta MIT header)
 */

// See 00-header.js. Called by intl/jsi/Plugin.cpp with the native helpers
// and the resolved defaults.
return function setup(nativeHelpers, defaults) {
  native = nativeHelpers;
  hostDefaults = defaults;
  installIntl();
};
})();
```

`intl/js/CMakeLists.txt`:
```cmake
# (Meta MIT header)

# Compiles intl/js to Hermes bytecode and dumps it as IntlBytecode.inc for
# intl/jsi/IntlBytecode.cpp. Same scheme as API/hermes/extensions.

if(IMPORT_HOST_COMPILERS)
  file(TO_CMAKE_PATH "${IMPORT_HOST_COMPILERS}" IMPORT_HOST_COMPILERS_CMAKE)
  include(${IMPORT_HOST_COMPILERS_CMAKE})
  set(hermesc_EXE imported-hermesc)
else()
  set(hermesc_EXE ${HERMES_TOOLS_OUTPUT_DIR}/hermesc)
endif()

file(GLOB intl_js_sources CONFIGURE_DEPENDS
  ${CMAKE_CURRENT_SOURCE_DIR}/[0-9][0-9]-*.js)
list(SORT intl_js_sources)

add_custom_command(
  OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.js
  COMMAND ${CMAKE_COMMAND} -E cat ${intl_js_sources}
    > ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.js
  DEPENDS ${intl_js_sources}
  VERBATIM)

add_custom_command(
  OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.hbc
  COMMAND ${hermesc_EXE} -O -Wno-undefined-variable -g0 -emit-binary
    -out=${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.hbc
    ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.js
  DEPENDS ${hermesc_EXE} ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.js
  VERBATIM)

add_custom_command(
  OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.inc
  COMMAND ${Python_EXECUTABLE} ${PROJECT_SOURCE_DIR}/lib/InternalJavaScript/xxd.py
    ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.hbc
    > ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.inc
  DEPENDS ${PROJECT_SOURCE_DIR}/lib/InternalJavaScript/xxd.py
    ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.hbc
  VERBATIM)

add_custom_target(hermesIntlBytecode
  DEPENDS ${CMAKE_CURRENT_BINARY_DIR}/IntlBytecode.inc)
```
`cmake -E cat` exists since CMake 3.18. The redirection relies on the generator running the command through a shell, as the extensions' `cat` does.

- [ ] **Step 3: Write the C++ layer**

`intl/jsi/RustCore.h`:
```cpp
// (Meta MIT header)

#pragma once

/// C++ wrappers over the Rust core's C interface (hermes_intl_core.h,
/// generated from intl/core/src/lib.rs).

#include "hermes_intl_core.h"

#include <optional>
#include <string>
#include <string_view>

namespace hermes_intl::core {

/// Take ownership of a string the Rust core returned.
inline std::string takeString(HiString s) {
  std::string result(reinterpret_cast<const char *>(s.data), s.len);
  hi_string_free(s);
  return result;
}

/// Call a Rust function of the shape (data, len, out) -> bool.
template <class F>
std::optional<std::string> callOptional(F function, std::string_view in) {
  HiString out;
  if (!function(
          reinterpret_cast<const uint8_t *>(in.data()), in.size(), &out))
    return std::nullopt;
  return takeString(out);
}

/// IsStructurallyValidLanguageTag + CanonicalizeUnicodeLocaleId.
inline std::optional<std::string> canonicalizeLocale(std::string_view tag) {
  return callOptional(hi_locale_canonicalize, tag);
}

/// Whether \p tag (canonical, no extensions) is an available locale.
inline bool isAvailableLocale(std::string_view tag) {
  return hi_locale_is_available(
      reinterpret_cast<const uint8_t *>(tag.data()), tag.size());
}

/// A host locale name to a canonical BCP 47 tag.
inline std::optional<std::string> localeFromHost(std::string_view raw) {
  return callOptional(hi_locale_from_host, raw);
}

/// A TZ value or /etc/localtime link target to a primary IANA name.
inline std::optional<std::string> timeZoneFromHost(std::string_view raw) {
  return callOptional(hi_time_zone_from_host, raw);
}

/// A case-insensitive IANA name to its primary identifier.
inline std::optional<std::string> canonicalizeTimeZone(
    std::string_view name) {
  return callOptional(hi_time_zone_canonicalize, name);
}

/// The values of \p kind for Intl.supportedValuesOf, space-separated.
inline std::string supportedValues(HiValueKind kind) {
  HiString out;
  hi_supported_values(kind, &out);
  return takeString(out);
}

} // namespace hermes_intl::core
```

`intl/jsi/NativeHelpers.def`:
```cpp
// (Meta MIT header)

// The native helpers the JS layer calls, one line each:
//   HERMES_INTL_HELPER(name, Result, Arg...)
// Result and every Arg is a conversion tag from NativeHelpers.h:
//   Bool     a JS boolean
//   Utf8     a JS string as UTF-8: option strings and formatter output
//   OptUtf8  a JS string or undefined (results only)
// JS validates every argument first; a wrong count or type is a plugin bug.
// No include guard: this file is included once per expansion.

// (tag) The canonicalized tag, or undefined if tag is not well-formed.
HERMES_INTL_HELPER(canonicalizeLocale, OptUtf8, Utf8)
// (tag) Whether tag (canonical, without extensions) is an available locale.
HERMES_INTL_HELPER(isAvailableLocale, Bool, Utf8)
// (key) The values for Intl.supportedValuesOf(key), sorted, space-separated.
// key is "calendar", "collation", "currency", "numberingSystem" or
// "timeZone".
HERMES_INTL_HELPER(supportedValues, Utf8, Utf8)
```

`intl/jsi/NativeHelpers.h`:
```cpp
// (Meta MIT header)

#pragma once

#include <jsi/jsi.h>

#include <optional>
#include <string>

namespace hermes_intl {

/// Conversion tags for NativeHelpers.def.
struct Bool {};
struct Utf8 {};
struct OptUtf8 {};

/// The C++ type a conversion tag stands for.
template <class Tag>
struct TagType;
template <>
struct TagType<Bool> {
  using type = bool;
};
template <>
struct TagType<Utf8> {
  using type = std::string;
};
template <>
struct TagType<OptUtf8> {
  using type = std::optional<std::string>;
};

/// The signature of the C++ function implementing a helper.
template <class Result, class... Args>
using HelperSignature = typename TagType<Result>::type(
    facebook::jsi::Runtime &,
    const typename TagType<Args>::type &...);

/// One declaration per helper, e.g.
///   std::optional<std::string> canonicalizeLocale(
///       jsi::Runtime &, const std::string &);
namespace helpers {
#define HERMES_INTL_HELPER(name, Result, ...) \
  HelperSignature<Result, __VA_ARGS__> name;
#include "NativeHelpers.def"
#undef HERMES_INTL_HELPER
} // namespace helpers

/// An object with one host function per line of NativeHelpers.def, passed to
/// the JS setup function and never reachable from user code.
facebook::jsi::Object makeNativeHelpers(facebook::jsi::Runtime &rt);

} // namespace hermes_intl
```

`intl/jsi/NativeHelpers.cpp`:
```cpp
// (Meta MIT header)

#include "NativeHelpers.h"

#include <stdexcept>
#include <utility>

using namespace facebook;

namespace hermes_intl {
namespace {

/// Report a plugin bug. Hermes turns a C++ exception from a host function
/// into an Error made from its own intrinsics, which user code cannot
/// intercept.
[[noreturn]] void internalError(const std::string &what) {
  throw std::logic_error("Intl internal error: " + what);
}

/// Conversions between JS values and the tags' C++ types.
template <class Tag>
struct Convert;

template <>
struct Convert<Utf8> {
  static std::string
  fromJS(jsi::Runtime &rt, const jsi::Value &v, const char *helper) {
    if (!v.isString())
      internalError(std::string(helper) + ": expected a string");
    return v.getString(rt).utf8(rt);
  }
  static jsi::Value toJS(jsi::Runtime &rt, const std::string &s) {
    return jsi::String::createFromUtf8(rt, s);
  }
};

template <>
struct Convert<Bool> {
  static jsi::Value toJS(jsi::Runtime &, bool b) {
    return jsi::Value(b);
  }
};

template <>
struct Convert<OptUtf8> {
  static jsi::Value toJS(
      jsi::Runtime &rt,
      const std::optional<std::string> &s) {
    if (!s)
      return jsi::Value::undefined();
    return jsi::String::createFromUtf8(rt, *s);
  }
};

/// Convert the arguments, call \p fn, convert the result.
template <class Result, class... Args, size_t... I>
jsi::Value callHelper(
    HelperSignature<Result, Args...> *fn,
    jsi::Runtime &rt,
    const jsi::Value *args,
    const char *name,
    std::index_sequence<I...>) {
  return Convert<Result>::toJS(
      rt, fn(rt, Convert<Args>::fromJS(rt, args[I], name)...));
}

/// A host function named \p name that calls \p fn. It captures no JS values,
/// so it is safe to destroy on the finalizer thread.
template <class Result, class... Args>
jsi::Function makeHelper(
    jsi::Runtime &rt,
    const char *name,
    HelperSignature<Result, Args...> *fn) {
  return jsi::Function::createFromHostFunction(
      rt,
      jsi::PropNameID::forAscii(rt, name),
      sizeof...(Args),
      [fn, name](
          jsi::Runtime &rt,
          const jsi::Value &,
          const jsi::Value *args,
          size_t count) -> jsi::Value {
        if (count != sizeof...(Args))
          internalError(std::string(name) + ": wrong argument count");
        return callHelper<Result, Args...>(
            fn, rt, args, name, std::index_sequence_for<Args...>{});
      });
}

} // namespace

jsi::Object makeNativeHelpers(jsi::Runtime &rt) {
  jsi::Object helpersObject(rt);
#define HERMES_INTL_HELPER(name, Result, ...) \
  helpersObject.setProperty(                  \
      rt, #name, makeHelper<Result, __VA_ARGS__>(rt, #name, &helpers::name));
#include "NativeHelpers.def"
#undef HERMES_INTL_HELPER
  return helpersObject;
}

} // namespace hermes_intl
```

`intl/jsi/LocaleHelpers.cpp`:
```cpp
// (Meta MIT header)

// The locale helpers of NativeHelpers.def.

#include "NativeHelpers.h"
#include "RustCore.h"

#include <stdexcept>

using namespace facebook;

namespace hermes_intl::helpers {

std::optional<std::string> canonicalizeLocale(
    jsi::Runtime &,
    const std::string &tag) {
  return core::canonicalizeLocale(tag);
}

bool isAvailableLocale(jsi::Runtime &, const std::string &tag) {
  return core::isAvailableLocale(tag);
}

std::string supportedValues(jsi::Runtime &, const std::string &key) {
  HiValueKind kind;
  if (key == "calendar")
    kind = HiValueKind_Calendar;
  else if (key == "collation")
    kind = HiValueKind_Collation;
  else if (key == "currency")
    kind = HiValueKind_Currency;
  else if (key == "numberingSystem")
    kind = HiValueKind_NumberingSystem;
  else if (key == "timeZone")
    kind = HiValueKind_TimeZone;
  else
    throw std::logic_error("Intl internal error: supportedValues: " + key);
  return core::supportedValues(kind);
}

} // namespace hermes_intl::helpers
```

`intl/jsi/Defaults.h`:
```cpp
// (Meta MIT header)

#pragma once

#include <hermes/Public/IntlPlugin.h>

#include <string>

namespace hermes_intl {

/// hermes_intl_entry::resolve_defaults: canonicalize the locales and time
/// zone RuntimeConfig gave, and fill in the missing ones. Returns "" or a
/// message naming the invalid value.
std::string resolveDefaults(hermes_intl_config *config);

} // namespace hermes_intl
```

`intl/jsi/Defaults.cpp`:
```cpp
// (Meta MIT header)

#include "Defaults.h"

#include "RustCore.h"

namespace hermes_intl {

std::string resolveDefaults(hermes_intl_config *config) {
  for (std::string &tag : config->default_locales) {
    auto canonical = core::canonicalizeLocale(tag);
    if (!canonical)
      return "IntlDefaultLocales: \"" + tag +
          "\" is not a well-formed BCP 47 language tag";
    tag = std::move(*canonical);
  }
  // Task 8 replaces this fallback with host detection.
  if (config->default_locales.empty())
    config->default_locales.push_back("en-US");

  if (!config->default_time_zone.empty()) {
    auto zone = core::canonicalizeTimeZone(config->default_time_zone);
    if (!zone)
      return "IntlDefaultTimeZone: \"" + config->default_time_zone +
          "\" is not an IANA time zone the Intl plugin knows";
    config->default_time_zone = std::move(*zone);
  } else {
    // Task 8 replaces this fallback with host detection.
    config->default_time_zone = "UTC";
  }
  return "";
}

} // namespace hermes_intl
```

`intl/jsi/IntlBytecode.cpp`:
```cpp
// (Meta MIT header)

#include <cstddef>
#include <cstdint>
#include <utility>

namespace hermes_intl {

/// The plugin's JS (intl/js), compiled by this build's hermesc.
std::pair<const uint8_t *, size_t> intlBytecode() {
  // Hermes requires bytecode buffers to be 4-byte aligned.
  alignas(4) static const uint8_t kBytecode[] = {
#include "IntlBytecode.inc"
  };
  return {kBytecode, sizeof(kBytecode)};
}

} // namespace hermes_intl
```

`intl/jsi/Plugin.cpp`:
```cpp
// (Meta MIT header)

// The Intl plugin's entry point. See "Loading and installation" in
// doc/superpowers/specs/2026-10-22-intl-plugin-design.md.

#include "Defaults.h"
#include "NativeHelpers.h"

#include <hermes/Public/IntlPlugin.h>
#include <jsi/hermes-interfaces.h>
#include <jsi/jsi.h>

#include <memory>

using namespace facebook;

namespace hermes_intl {

std::pair<const uint8_t *, size_t> intlBytecode();

namespace {

/// A jsi::Buffer over the embedded bytecode, which lives as long as the
/// library, which is never unloaded.
class StaticBuffer final : public jsi::Buffer {
 public:
  StaticBuffer(const uint8_t *data, size_t size) : data_(data), size_(size) {}
  size_t size() const override {
    return size_;
  }
  const uint8_t *data() const override {
    return data_;
  }

 private:
  const uint8_t *data_;
  size_t size_;
};

/// hermes_intl_entry::install: run the JS and call its setup function with
/// the native helpers and the resolved defaults.
void install(jsi::Runtime &rt, const hermes_intl_config *config) {
  auto *builtin =
      jsi::castInterface<facebook::hermes::IHermesBuiltinBytecode>(&rt);
  if (!builtin)
    throw jsi::JSINativeException(
        "the runtime does not implement IHermesBuiltinBytecode");
  auto bytecode = intlBytecode();
  jsi::Function setup =
      builtin
          ->runBuiltinBytecode(
              std::make_shared<StaticBuffer>(bytecode.first, bytecode.second),
              "hermes-intl.js")
          .asObject(rt)
          .asFunction(rt);

  jsi::Array locales(rt, config->default_locales.size());
  for (size_t i = 0; i < config->default_locales.size(); ++i)
    locales.setValueAtIndex(
        rt, i, jsi::String::createFromUtf8(rt, config->default_locales[i]));
  jsi::Object defaults(rt);
  defaults.setProperty(rt, "locales", locales);
  defaults.setProperty(
      rt,
      "timeZone",
      jsi::String::createFromUtf8(rt, config->default_time_zone));

  setup.call(rt, makeNativeHelpers(rt), defaults);
}

/// The flags this plugin was built with.
constexpr uint32_t kFlags =
#ifdef JSI_UNSTABLE
    HERMES_INTL_FLAG_JSI_UNSTABLE;
#else
    0;
#endif

const hermes_intl_entry kEntry{
    sizeof(hermes_intl_entry),
    HERMES_INTL_PLUGIN_INTERFACE_VERSION,
    HERMES_INTL_BYTECODE_VERSION,
    kFlags,
    HERMES_INTL_VERSION,
    resolveDefaults,
    install};

} // namespace
} // namespace hermes_intl

extern "C" __attribute__((visibility("default"))) const hermes_intl_entry *
hermes_intl_get_entry() {
  return &hermes_intl::kEntry;
}
```

- [ ] **Step 4: Build the library and `check-intl` in CMake**

Append to `intl/CMakeLists.txt`:
```cmake
# --- The plugin library ------------------------------------------------------

# The bytecode version of the hermesc that compiles intl/js. The loader
# rejects a plugin whose version differs from the engine's.
file(STRINGS ${PROJECT_SOURCE_DIR}/include/hermes/BCGen/HBC/BytecodeVersion.h
  _bytecode_version_line REGEX "BYTECODE_VERSION = [0-9]+")
string(REGEX MATCH "[0-9]+" HERMES_INTL_BYTECODE_VERSION
  "${_bytecode_version_line}")
if(NOT HERMES_INTL_BYTECODE_VERSION)
  message(FATAL_ERROR "Could not read BYTECODE_VERSION from BytecodeVersion.h")
endif()
set(HERMES_INTL_VERSION 0.1.0)

add_subdirectory(js)

set(HERMES_ENABLE_EH ON)
set(HERMES_ENABLE_RTTI ON)
set(HERMES_INTL_PLUGIN_SOURCES
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/Defaults.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/IntlBytecode.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/LocaleHelpers.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/NativeHelpers.cpp
  ${CMAKE_CURRENT_SOURCE_DIR}/jsi/Plugin.cpp)
add_hermes_library(hermesintl SHARED ${HERMES_INTL_PLUGIN_SOURCES})
target_include_directories(hermesintl PRIVATE
  ${HERMES_JSI_DIR} ${PROJECT_SOURCE_DIR}/public
  ${CMAKE_CURRENT_BINARY_DIR}/js)
target_compile_definitions(hermesintl PRIVATE
  HERMES_INTL_BYTECODE_VERSION=${HERMES_INTL_BYTECODE_VERSION}
  HERMES_INTL_VERSION="${HERMES_INTL_VERSION}")
target_link_libraries(hermesintl PRIVATE hermesIntlCore)
add_dependencies(hermesintl hermesIntlBytecode)
# Next to libhermesvm, where the loader's default resolution looks.
set_target_properties(hermesintl PROPERTIES
  LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib)
if(APPLE)
  # JSI binds to the host's exported copy when the library is loaded.
  target_link_options(hermesintl PRIVATE -undefined dynamic_lookup)
  target_link_libraries(hermesintl PRIVATE "-framework CoreFoundation")
else()
  # Export only hermes_intl_get_entry, nothing from the Rust core.
  target_link_options(hermesintl PRIVATE -Wl,--exclude-libs,ALL)
endif()

# --- Tests -------------------------------------------------------------------

if(HERMES_ENABLE_TEST_SUITE)
  set(HERMES_INTL_LIT_PARAMS
    intl_test_exec_root=${CMAKE_CURRENT_BINARY_DIR}/test/lit
    intl_plugin=$<TARGET_FILE:hermesintl>
    intl_js_dir=${CMAKE_CURRENT_SOURCE_DIR}/js
    hermes=${HERMES_TOOLS_OUTPUT_DIR}/hermes
    hermesc=${HERMES_TOOLS_OUTPUT_DIR}/hermesc
    hermes_jsi_shared=${HERMES_TOOLS_OUTPUT_DIR}/hermes-jsi-shared
    FileCheck=${HERMES_TOOLS_OUTPUT_DIR}/FileCheck
    tools_dir=${HERMES_TOOLS_OUTPUT_DIR}
    jsi_unstable=${JSI_UNSTABLE})
  set(HERMES_INTL_LIT_SUITES ${CMAKE_CURRENT_SOURCE_DIR}/test/lit)
  set(HERMES_INTL_LIT_DEPENDS hermesintl hermes hermesc hermes-jsi-shared)

  add_lit_testsuite(check-intl-lit "Running the Intl plugin lit tests"
    ${HERMES_INTL_LIT_SUITES}
    PARAMS ${HERMES_INTL_LIT_PARAMS}
    DEPENDS ${HERMES_INTL_LIT_DEPENDS})

  add_custom_target(check-intl-rust
    COMMAND ${HERMES_INTL_CARGO_ENV} ${HERMES_INTL_CARGO} test --locked
      ${HERMES_INTL_CARGO_PROFILE_ARGS}
      --manifest-path ${HERMES_INTL_CORE_DIR}/Cargo.toml
    COMMENT "Running the Intl plugin's Rust tests"
    USES_TERMINAL
    VERBATIM)

  add_custom_target(check-intl)
  add_dependencies(check-intl check-intl-rust check-intl-lit)
endif()
```

- [ ] **Step 5: Build and run**

```
cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target hermesintl hermes hermes-jsi-shared
cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-intl
```
Expected:
- `cmake-build-asan/lib/libhermesintl.so` exists.
- `check-intl-rust` passes.
- The three lit tests pass.

If `hosts.js` fails for `hermes-jsi-shared` with an undefined symbol, the binding to `libhermesvm` is broken. That is the contract this test exists for; diagnose it with `LD_DEBUG=bindings` and report. Do not work around it.

- [ ] **Step 6: Show the tests can fail**

Make each change below, run, see the named test fail, then revert it:
- Delete the `setToStringTag(IntlObject, 'Intl');` line: `plugin-loads.js` fails.
- Delete `-Wl,--exclude-libs,ALL`: `exports.js` fails on a `CHECK-NOT`.
- Change `HERMES_INTL_BYTECODE_VERSION` in `Plugin.cpp` to `HERMES_INTL_BYTECODE_VERSION + 1`: every test fails with the loader's `bytecode version` message. That proves the real plugin goes through the loader's checks.

- [ ] **Step 7: Rerun the Worker test that needs a real plugin, and `check-hermes`**

```
LIT_FILTER=worker-inherits-disabled cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-hermes
```
Expected: it passes. Then, with the `makeWorkerRuntimeConfig(rt)` call in `Worker.cpp` temporarily replaced by `::hermes::vm::RuntimeConfig()`, it fails: the worker prints `undefined object`. Restore the call.

Then run all of `check-hermes`. Every `%hermes` run now loads the real plugin from `../lib`. Compare against Task 5's results. Any test that changes because `Intl` now exists (`grep -rln '\bIntl\b' test --include=*.js`) is reported, with the reason, in the task report. Fix a test only if it was asserting the absence of `Intl` on Linux; anything else is a plugin bug.

- [ ] **Step 8: Stage**

`git -C /home/tmikov/work/hermes-intl add intl`

### Task 8: Default locale and time zone detection

**Files:**
- Modify: `intl/jsi/Defaults.h`, `intl/jsi/Defaults.cpp`
- Create: `intl/test/unit/CMakeLists.txt`, `intl/test/unit/lit.cfg`, `intl/test/unit/DefaultsTest.cpp`
- Modify: `intl/CMakeLists.txt` (unit tests in `check-intl-lit`)

**Interfaces:**
- Consumes: `core::localeFromHost`, `core::timeZoneFromHost`, `core::canonicalizeLocale` and `core::canonicalizeTimeZone` (`RustCore.h`, Task 7).
- Produces, in `Defaults.h`:
  ```cpp
  struct HostEnvironment {
    std::function<const char *(const char *)> getEnv;  // default: std::getenv
    std::string localtimePath;                          // default: "/etc/localtime"
  };
  std::vector<std::string> hostLocaleCandidates(const HostEnvironment &);
  std::vector<std::string> hostTimeZoneCandidates(const HostEnvironment &);
  std::string detectDefaultLocale(const HostEnvironment &);
  std::string detectDefaultTimeZone(const HostEnvironment &);
  std::string resolveDefaults(hermes_intl_config *, const HostEnvironment &);
  std::string resolveDefaults(hermes_intl_config *);  // the entry point
  ```

The per-platform sources follow the spec's table. On Android, bionic's own order is `TZ` and then `persist.sys.timezone`; this was confirmed in `libc/tzcode/bionic.cpp` (lines 79–83 on bionic `main`, 2026-10-22), as the spec asked.

- [ ] **Step 1: Write the failing tests**

`intl/test/unit/CMakeLists.txt`:
```cmake
# (Meta MIT header)

# Unit tests of the plugin's C++ that need no runtime.
set(HERMES_ENABLE_EH ON)
set(HERMES_ENABLE_RTTI ON)
set(LLVM_ENABLE_RTTI ON)

add_unittest(HermesIntlUnitTests IntlPluginUnitTests
  DefaultsTest.cpp
  ${PROJECT_SOURCE_DIR}/intl/jsi/Defaults.cpp)
target_include_directories(IntlPluginUnitTests PRIVATE
  ${PROJECT_SOURCE_DIR}/intl/jsi ${PROJECT_SOURCE_DIR}/public)
target_link_libraries(IntlPluginUnitTests hermesIntlCore)
if(APPLE)
  # Defaults.cpp's Apple branch calls CoreFoundation.
  target_link_libraries(IntlPluginUnitTests "-framework CoreFoundation")
endif()
```

`intl/test/unit/lit.cfg`:
```python
# (Meta MIT header as # comments)
import lit.formats

config.name = "Hermes-Intl-Unit"
config.suffixes = []
config.test_exec_root = lit_config.params["intl_unittests_dir"]
config.test_source_root = config.test_exec_root
config.test_format = lit.formats.GoogleTest(".", "Tests")
```

`intl/test/unit/DefaultsTest.cpp`:
```cpp
// (Meta MIT header)

#include "Defaults.h"

#include <gtest/gtest.h>

#include <cstdio>
#include <map>
#include <string>
#include <unistd.h>

using namespace hermes_intl;

namespace {

/// A HostEnvironment with the given variables and /etc/localtime path.
HostEnvironment fakeHost(
    std::map<std::string, std::string> vars,
    std::string localtime = "/nonexistent/localtime") {
  HostEnvironment env;
  auto shared = std::make_shared<std::map<std::string, std::string>>(
      std::move(vars));
  env.getEnv = [shared](const char *name) -> const char * {
    auto it = shared->find(name);
    return it == shared->end() ? nullptr : it->second.c_str();
  };
  env.localtimePath = std::move(localtime);
  return env;
}

#if defined(__linux__) && !defined(__ANDROID__)

TEST(DefaultsTest, LocaleSourcesInOrder) {
  auto env = fakeHost(
      {{"LC_ALL", "de_DE.UTF-8"},
       {"LC_MESSAGES", "fr_FR.UTF-8"},
       {"LANG", "it_IT.UTF-8"}});
  EXPECT_EQ(detectDefaultLocale(env), "de-DE");
  env = fakeHost({{"LC_MESSAGES", "fr_FR.UTF-8"}, {"LANG", "it_IT.UTF-8"}});
  EXPECT_EQ(detectDefaultLocale(env), "fr-FR");
  env = fakeHost({{"LANG", "it_IT.UTF-8"}});
  EXPECT_EQ(detectDefaultLocale(env), "it-IT");
}

TEST(DefaultsTest, UnusableLocaleSourcesAreSkipped) {
  auto env = fakeHost(
      {{"LC_ALL", ""}, {"LC_MESSAGES", "garbage!!"}, {"LANG", "sr_RS@latin"}});
  EXPECT_EQ(detectDefaultLocale(env), "sr-Latn-RS");
}

TEST(DefaultsTest, LocaleFallsBackToEnUS) {
  EXPECT_EQ(detectDefaultLocale(fakeHost({})), "en-US");
  EXPECT_EQ(detectDefaultLocale(fakeHost({{"LANG", "C.UTF-8"}})), "en-US");
}

TEST(DefaultsTest, TimeZoneFromTZ) {
  EXPECT_EQ(
      detectDefaultTimeZone(fakeHost({{"TZ", ":Europe/Paris"}})),
      "Europe/Paris");
  EXPECT_EQ(
      detectDefaultTimeZone(
          fakeHost({{"TZ", "/usr/share/zoneinfo/Asia/Calcutta"}})),
      "Asia/Kolkata");
}

/// A temporary directory with an "etc/localtime" symlink to \p target.
std::string makeLocaltimeLink(const std::string &target) {
  char dir[] = "/tmp/intl-defaults-XXXXXX";
  EXPECT_NE(::mkdtemp(dir), nullptr);
  std::string link = std::string(dir) + "/localtime";
  EXPECT_EQ(::symlink(target.c_str(), link.c_str()), 0);
  return link;
}

TEST(DefaultsTest, TimeZoneFromLocaltimeLink) {
  std::string link =
      makeLocaltimeLink("../usr/share/zoneinfo/America/New_York");
  EXPECT_EQ(detectDefaultTimeZone(fakeHost({}, link)), "America/New_York");
  // TZ comes first.
  EXPECT_EQ(
      detectDefaultTimeZone(fakeHost({{"TZ", "Asia/Tokyo"}}, link)),
      "Asia/Tokyo");
  // An unusable TZ (a POSIX rule) falls through to the link.
  EXPECT_EQ(
      detectDefaultTimeZone(fakeHost({{"TZ", "EST5EDT,M3.2.0,M11.1.0"}}, link)),
      "America/New_York");
}

TEST(DefaultsTest, LocaltimeOutsideZoneinfoIsIgnored) {
  std::string link = makeLocaltimeLink("/etc/somewhere/Europe/Paris");
  EXPECT_EQ(detectDefaultTimeZone(fakeHost({}, link)), "UTC");
}

TEST(DefaultsTest, LocaltimeCopyIsIgnored) {
  char path[] = "/tmp/intl-localtime-XXXXXX";
  int fd = ::mkstemp(path);
  ASSERT_GE(fd, 0);
  ::close(fd);
  EXPECT_EQ(detectDefaultTimeZone(fakeHost({}, path)), "UTC");
}

#endif // __linux__ && !__ANDROID__

TEST(DefaultsTest, ExplicitValuesAreCanonicalized) {
  hermes_intl_config config{{"EN-us", "iw"}, "asia/calcutta"};
  EXPECT_EQ(resolveDefaults(&config, fakeHost({})), "");
  EXPECT_EQ(config.default_locales, (std::vector<std::string>{"en-US", "he"}));
  EXPECT_EQ(config.default_time_zone, "Asia/Kolkata");
}

TEST(DefaultsTest, ExplicitValuesWinOverTheHost) {
  hermes_intl_config config{{"ja-JP"}, "Asia/Tokyo"};
  auto env = fakeHost({{"LANG", "de_DE.UTF-8"}, {"TZ", "Europe/Berlin"}});
  EXPECT_EQ(resolveDefaults(&config, env), "");
  EXPECT_EQ(config.default_locales, (std::vector<std::string>{"ja-JP"}));
  EXPECT_EQ(config.default_time_zone, "Asia/Tokyo");
}

TEST(DefaultsTest, InvalidExplicitValuesAreErrors) {
  hermes_intl_config badLocale{{"en_US"}, ""};
  EXPECT_NE(
      resolveDefaults(&badLocale, fakeHost({})).find("IntlDefaultLocales"),
      std::string::npos);
  hermes_intl_config badZone{{}, "Mars/Olympus_Mons"};
  EXPECT_NE(
      resolveDefaults(&badZone, fakeHost({})).find("IntlDefaultTimeZone"),
      std::string::npos);
}

} // namespace
```
The tests outside the Linux block run on every platform. The Linux block covers detection; Task 13 covers macOS by hand.

In `intl/CMakeLists.txt`, inside `if(HERMES_ENABLE_TEST_SUITE)` and before `add_lit_testsuite(check-intl-lit ...)`:
```cmake
  add_custom_target(HermesIntlUnitTests)
  add_subdirectory(test/unit)
  list(APPEND HERMES_INTL_LIT_PARAMS
    intl_unittests_dir=${CMAKE_CURRENT_BINARY_DIR}/test/unit)
  list(APPEND HERMES_INTL_LIT_SUITES ${CMAKE_CURRENT_SOURCE_DIR}/test/unit)
  list(APPEND HERMES_INTL_LIT_DEPENDS HermesIntlUnitTests)
```
`add_unittest` comes from `cmake/modules/Lit.cmake`. If it is not visible in the `intl/` scope, add `include(Lit)` at the top of `intl/CMakeLists.txt`.

- [ ] **Step 2: Run them and watch them fail to compile**

`cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target IntlPluginUnitTests`
Expected: errors, because `HostEnvironment` and the `detect*` functions are not declared.

- [ ] **Step 3: Implement detection**

`intl/jsi/Defaults.h` becomes:
```cpp
// (Meta MIT header)

#pragma once

#include <hermes/Public/IntlPlugin.h>

#include <cstdlib>
#include <functional>
#include <string>
#include <vector>

namespace hermes_intl {

/// Where detection reads the host's settings; tests substitute their own.
struct HostEnvironment {
  /// Like std::getenv: nullptr when the variable is not set.
  std::function<const char *(const char *)> getEnv =
      [](const char *name) -> const char * { return std::getenv(name); };
  /// The system time zone link (Linux).
  std::string localtimePath = "/etc/localtime";
};

/// The host's locale settings, most authoritative first, unnormalized. See
/// "Default locale and time zone" in the spec.
std::vector<std::string> hostLocaleCandidates(const HostEnvironment &env);

/// The host's time zone settings, most authoritative first, unnormalized.
std::vector<std::string> hostTimeZoneCandidates(const HostEnvironment &env);

/// The first locale candidate that normalizes to a BCP 47 tag, else "en-US".
std::string detectDefaultLocale(const HostEnvironment &env);

/// The first time zone candidate that names a known IANA zone, else "UTC".
std::string detectDefaultTimeZone(const HostEnvironment &env);

/// hermes_intl_entry::resolve_defaults against \p env: canonicalize the
/// values RuntimeConfig gave, detect the missing ones. Returns "" or a
/// message naming the invalid value.
std::string resolveDefaults(
    hermes_intl_config *config,
    const HostEnvironment &env);

/// resolveDefaults against the real host.
std::string resolveDefaults(hermes_intl_config *config);

} // namespace hermes_intl
```

`intl/jsi/Defaults.cpp` becomes:
```cpp
// (Meta MIT header)

#include "Defaults.h"

#include "RustCore.h"

#include <climits>

#if defined(__APPLE__)
#include <CoreFoundation/CoreFoundation.h>
#elif defined(__ANDROID__)
#include <sys/system_properties.h>
#endif
#include <unistd.h>

namespace hermes_intl {
namespace {

/// Append the variable \p name to \p out if it is set and not empty.
void addEnv(
    std::vector<std::string> &out,
    const HostEnvironment &env,
    const char *name) {
  const char *value = env.getEnv(name);
  if (value && *value)
    out.emplace_back(value);
}

#if !defined(__APPLE__) && !defined(__ANDROID__)
/// The target of the symlink at \p path, or "" if it is not a symlink.
std::string readSymlink(const std::string &path) {
  char buf[PATH_MAX];
  ssize_t n = ::readlink(path.c_str(), buf, sizeof(buf) - 1);
  return n > 0 ? std::string(buf, n) : std::string();
}
#endif

#if defined(__ANDROID__)
/// The Android system property \p name, or "".
std::string systemProperty(const char *name) {
  char buf[PROP_VALUE_MAX];
  int n = __system_property_get(name, buf);
  return n > 0 ? std::string(buf, n) : std::string();
}
#endif

#if defined(__APPLE__)
/// \p s as UTF-8.
std::string toUtf8(CFStringRef s) {
  CFIndex length = CFStringGetLength(s);
  CFIndex size =
      CFStringGetMaximumSizeForEncoding(length, kCFStringEncodingUTF8) + 1;
  std::string result(size, '\0');
  if (!CFStringGetCString(s, result.data(), size, kCFStringEncodingUTF8))
    return "";
  result.resize(std::char_traits<char>::length(result.c_str()));
  return result;
}
#endif

} // namespace

std::vector<std::string> hostLocaleCandidates(const HostEnvironment &env) {
  std::vector<std::string> out;
#if defined(__APPLE__)
  (void)env;
  if (CFArrayRef languages = CFLocaleCopyPreferredLanguages()) {
    for (CFIndex i = 0, n = CFArrayGetCount(languages); i < n; ++i)
      out.push_back(
          toUtf8(static_cast<CFStringRef>(
              CFArrayGetValueAtIndex(languages, i))));
    CFRelease(languages);
  }
#elif defined(__ANDROID__)
  (void)env;
  for (const char *name : {"persist.sys.locale", "ro.product.locale"}) {
    std::string value = systemProperty(name);
    if (!value.empty())
      out.push_back(value);
  }
#else
  addEnv(out, env, "LC_ALL");
  addEnv(out, env, "LC_MESSAGES");
  addEnv(out, env, "LANG");
#endif
  return out;
}

std::vector<std::string> hostTimeZoneCandidates(const HostEnvironment &env) {
  std::vector<std::string> out;
#if defined(__APPLE__)
  (void)env;
  if (CFTimeZoneRef zone = CFTimeZoneCopySystem()) {
    out.push_back(toUtf8(CFTimeZoneGetName(zone)));
    CFRelease(zone);
  }
#elif defined(__ANDROID__)
  // bionic's own order (libc/tzcode/bionic.cpp): TZ, then the property.
  addEnv(out, env, "TZ");
  std::string value = systemProperty("persist.sys.timezone");
  if (!value.empty())
    out.push_back(value);
#else
  addEnv(out, env, "TZ");
  // Only a symlink into a zoneinfo directory names a zone; a copied file
  // does not.
  std::string link = readSymlink(env.localtimePath);
  if (link.find("zoneinfo/") != std::string::npos)
    out.push_back(link);
#endif
  return out;
}

std::string detectDefaultLocale(const HostEnvironment &env) {
  for (const std::string &raw : hostLocaleCandidates(env)) {
    if (auto tag = core::localeFromHost(raw))
      return *tag;
  }
  return "en-US";
}

std::string detectDefaultTimeZone(const HostEnvironment &env) {
  for (const std::string &raw : hostTimeZoneCandidates(env)) {
    if (auto zone = core::timeZoneFromHost(raw))
      return *zone;
  }
  return "UTC";
}

std::string resolveDefaults(
    hermes_intl_config *config,
    const HostEnvironment &env) {
  for (std::string &tag : config->default_locales) {
    auto canonical = core::canonicalizeLocale(tag);
    if (!canonical)
      return "IntlDefaultLocales: \"" + tag +
          "\" is not a well-formed BCP 47 language tag";
    tag = std::move(*canonical);
  }
  if (config->default_locales.empty())
    config->default_locales.push_back(detectDefaultLocale(env));

  if (!config->default_time_zone.empty()) {
    auto zone = core::canonicalizeTimeZone(config->default_time_zone);
    if (!zone)
      return "IntlDefaultTimeZone: \"" + config->default_time_zone +
          "\" is not an IANA time zone the Intl plugin knows";
    config->default_time_zone = std::move(*zone);
  } else {
    config->default_time_zone = detectDefaultTimeZone(env);
  }
  return "";
}

std::string resolveDefaults(hermes_intl_config *config) {
  return resolveDefaults(config, HostEnvironment{});
}

} // namespace hermes_intl
```

- [ ] **Step 4: Run the tests**

```
cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target IntlPluginUnitTests
/home/tmikov/work/hermes-intl/cmake-build-asan/intl/test/unit/IntlPluginUnitTests
cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-intl
```
Expected: 10 tests pass, and `check-intl` passes with the unit suite included.

- [ ] **Step 5: Show the tests can fail**

Make each change below, run, see the named test fail, then revert it:
- Swap the `LC_ALL` and `LANG` lines: `LocaleSourcesInOrder` fails.
- Drop the `zoneinfo/` check in `hostTimeZoneCandidates`: `LocaltimeOutsideZoneinfoIsIgnored` does **not** fail, because the Rust side also rejects paths without `zoneinfo/`. Both guards are deliberate. Instead, change `link.find("zoneinfo/")` to `link.find("zoneinfoX/")`: `TimeZoneFromLocaltimeLink` fails.
- In `resolveDefaults`, skip canonicalizing the locale tags: `ExplicitValuesAreCanonicalized` fails.

- [ ] **Step 6: Check end to end through the CLI**

The plugin hands its resolved defaults to JS, but in Phase 0 no API shows them. Check the loader path with a fatal case instead:
```
/home/tmikov/work/hermes-intl/cmake-build-asan/bin/hermes -Xintl-default-time-zone=Mars/Olympus_Mons -Xintl-plugin-path=/home/tmikov/work/hermes-intl/cmake-build-asan/lib/libhermesintl.so /dev/null; echo "exit $?"
```
Expected: `LLVM ERROR: Intl plugin .../libhermesintl.so: rejected the default locale or time zone: IntlDefaultTimeZone: "Mars/Olympus_Mons" is not an IANA time zone the Intl plugin knows`, and a non-zero exit. Add this as `intl/test/lit/invalid-defaults.js`:
```js
/**
 * (Meta MIT header)
 */

// RUN: not --crash %hermes -Xintl-default-time-zone=Mars/Olympus_Mons %s 2>&1 | %FileCheck --check-prefix=ZONE %s
// ZONE: rejected the default locale or time zone: IntlDefaultTimeZone: "Mars/Olympus_Mons" is not an IANA time zone the Intl plugin knows
// RUN: not --crash %hermes -Xintl-default-locale=en_US %s 2>&1 | %FileCheck --check-prefix=LOCALE %s
// LOCALE: rejected the default locale or time zone: IntlDefaultLocales: "en_US" is not a well-formed BCP 47 language tag
// RUN: %hermes -Xintl-default-locale=EN-us -Xintl-default-time-zone=asia/calcutta %s | %FileCheck --check-prefix=OK %s
// OK: ok
print('ok');
```

- [ ] **Step 7: Stage**

`git -C /home/tmikov/work/hermes-intl add intl`

### Task 9: The shared abstract operations and the shape helpers, in JS

These operations have no public consumer until later phases. They are tested in isolation: a lit test concatenates the library files with a test file that supplies stub `native` and `hostDefaults`, and runs the result with the plugin disabled.

**Files:**
- Modify: `intl/js/01-intrinsics.js`, `intl/js/02-shapes.js`
- Create: `intl/js/03-locales.js`, `intl/js/04-options.js`
- Create: `intl/test/lit/js/Inputs/strict.js`, `intl/test/lit/js/canonicalize-locale-list.js`, `intl/test/lit/js/unicode-extensions.js`, `intl/test/lit/js/resolve-locale.js`, `intl/test/lit/js/options.js`, `intl/test/lit/js/digit-options.js`, `intl/test/lit/js/shapes.js`

**Interfaces:**
- Consumes: `native.canonicalizeLocale` and `native.isAvailableLocale` (Task 7), and `hostDefaults.locales`.
- Produces JS functions, at IIFE scope, used by Task 10 and later phases. Spec names are kept; parameters follow the ECMA-402 draft.
  - **Conversions:** `ToString`, `ToObject`, `ToNumber`, `ToIntegerOrInfinity`, `ToLength`, `asciiLowercase`.
  - **String helpers:** `stringIndexOf`, `stringSlice`, `charCodeAt`.
  - **Lists and arrays:** `listPush(list, v)`, `listContains(list, v)`, `CreateArrayFromList(list)`.
  - **Shapes:** `defineMethods(target, source)`, `defineGetters(target, source)`, `defineConstructor(fn, length, proto)`, `setToStringTag(obj, tag)`, `class ReturnOverride`.
  - **Locales:**
    - `isLocaleObject(o)` (`false` until Phase 1) and `localeObjectTag(o)`;
    - `CanonicalizeLocaleList(locales)`;
    - `splitUnicodeExtension(locale) -> {base, extension}`;
    - `UnicodeExtensionComponents(extension) -> {attributes, keywords: [{key, value}]}`;
    - `InsertUnicodeExtensionAndCanonicalize(locale, attributes, keywords)`;
    - `CanonicalizeUValue(ukey, uvalue)`;
    - `LookupMatchingLocaleByPrefix(isAvailable, requestedLocales) -> {locale, extension} | undefined`;
    - `ResolveLocale(isAvailable, requestedLocales, options, relevantExtensionKeys, localeData) -> {locale, localeData, <key>...}`;
    - `LookupSupportedLocales(isAvailable, requestedLocales)`;
    - `SupportedLocales(isAvailable, requestedLocales, options)`;
    - `DefaultLocale()`;
    - `isAvailableLocale(tag)`.
  - **Options:**
    - `REQUIRED`;
    - `GetOptionsObject(options)`, `CoerceOptionsToObject(options)`;
    - `GetOption(options, property, type, values, defaultValue)`;
    - `GetBooleanOrStringNumberFormatOption(options, property, stringValues, fallback)`;
    - `DefaultNumberOption(value, minimum, maximum, fallback, property)`;
    - `GetNumberOption(options, property, minimum, maximum, fallback)`;
    - `SetNumberFormatDigitOptions(intlObj, options, mnfdDefault, mxfdDefault, notation)`. It writes `minimumIntegerDigits`, `minimumFractionDigits`, `maximumFractionDigits`, `minimumSignificantDigits`, `maximumSignificantDigits`, `roundingType`, `computedRoundingPriority`, `roundingIncrement`, `roundingMode` and `trailingZeroDisplay` into the null-prototype record `intlObj`.
  - The `localeData` argument of `ResolveLocale` is a function, or `undefined` when the constructor has no locale data. `localeData(locale)` returns the record ECMA-402 calls `localeData.[[<locale>]]`: a null-prototype object with, for each relevant extension key, the list of supported values (default first), plus whatever other per-locale fields the constructor needs. `ResolveLocale` returns that record as `result.localeData` (the spec's `[[LocaleData]]`).

- [ ] **Step 1: Write the failing tests**

`intl/test/lit/js/Inputs/strict.js`:
```js
'use strict';
```

Each test below starts with the same three RUN lines (`<this test>` is the test's own file):
```
// RUN: cat %S/Inputs/strict.js %intl_js/01-intrinsics.js %intl_js/02-shapes.js %intl_js/03-locales.js %intl_js/04-options.js %s > %t.js
// RUN: %hermes_default -Xintl-plugin=false %t.js | %FileCheck --match-full-lines %s
// RUN: %hermesc -O -emit-binary -out=%t.hbc %t.js && %hermes_default -Xintl-plugin=false -b %t.hbc | %FileCheck --match-full-lines %s
```
Each also gets the Meta MIT header and this shared stub block:
```js
// Stubs for what the plugin's C++ passes to setup.
var available = ['en', 'en-US', 'de', 'fr'];
var native = {
  canonicalizeLocale: function (tag) {
    return /^[a-z]{2,3}(-[A-Za-z0-9]{1,8})*$/.test(tag) ? tag : undefined;
  },
  isAvailableLocale: function (tag) {
    return available.indexOf(tag) !== -1;
  },
};
var hostDefaults = {locales: ['fr-CA'], timeZone: 'UTC'};
/// The class of the exception f throws, or 'none'.
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}
```

`intl/test/lit/js/canonicalize-locale-list.js` (after the RUN lines, header and stubs):
```js
print(JSON.stringify(CanonicalizeLocaleList(undefined)));
// CHECK: []
print(JSON.stringify(CanonicalizeLocaleList('en')));
// CHECK-NEXT: ["en"]
print(JSON.stringify(CanonicalizeLocaleList(['de', 'en', 'de'])));
// CHECK-NEXT: ["de","en"]
print(JSON.stringify(CanonicalizeLocaleList({length: 3, 0: 'fr', 2: 'de'})));
// CHECK-NEXT: ["fr","de"]
print(JSON.stringify(CanonicalizeLocaleList({length: 1, 0: {toString() { return 'en'; }}})));
// CHECK-NEXT: ["en"]
print(thrown(() => CanonicalizeLocaleList([5])));
// CHECK-NEXT: TypeError
print(thrown(() => CanonicalizeLocaleList([null])));
// CHECK-NEXT: TypeError
print(thrown(() => CanonicalizeLocaleList([Symbol()])));
// CHECK-NEXT: TypeError
print(thrown(() => CanonicalizeLocaleList(['en_US'])));
// CHECK-NEXT: RangeError
print(thrown(() => CanonicalizeLocaleList(null)));
// CHECK-NEXT: TypeError
print(JSON.stringify(CanonicalizeLocaleList(5)));
// CHECK-NEXT: []

// Observable operations, in spec order.
var log = [];
var p = new Proxy({length: 2, 0: 'en', 1: 'fr'}, {
  get(t, k) { log.push('get ' + String(k)); return t[k]; },
  has(t, k) { log.push('has ' + String(k)); return k in t; },
});
print(JSON.stringify(CanonicalizeLocaleList(p)));
// CHECK-NEXT: ["en","fr"]
print(log.join(', '));
// CHECK-NEXT: get length, has 0, get 0, has 1, get 1
```

`intl/test/lit/js/unicode-extensions.js`:
```js
function split(l) {
  var r = splitUnicodeExtension(l);
  return r.base + ' | ' + r.extension;
}
print(split('en'));
// CHECK: en | undefined
print(split('en-u-ca-gregory'));
// CHECK-NEXT: en | -u-ca-gregory
print(split('de-a-foo-u-co-phonebk-x-u-bar'));
// CHECK-NEXT: de-a-foo-x-u-bar | -u-co-phonebk
print(split('en-x-u-ca'));
// CHECK-NEXT: en-x-u-ca | undefined

function components(e) {
  var c = UnicodeExtensionComponents(e);
  var ks = [];
  for (var i = 0; i < c.keywords.length; i++)
    ks.push(c.keywords[i].key + '=' + c.keywords[i].value);
  return c.attributes.join(',') + ' | ' + ks.join(',');
}
print(components('-u-attr1-attr2-ca-islamic-civil-kn'));
// CHECK-NEXT: attr1,attr2 | ca=islamic-civil,kn=
print(components('-u-ca-gregory-ca-buddhist'));
// CHECK-NEXT:  | ca=gregory

print(InsertUnicodeExtensionAndCanonicalize('en', [], [{key: 'ca', value: 'gregory'}, {key: 'kn', value: ''}]));
// CHECK-NEXT: en-u-ca-gregory-kn
print(InsertUnicodeExtensionAndCanonicalize('en-x-priv', ['a1b'], [{key: 'nu', value: 'thai'}]));
// CHECK-NEXT: en-u-a1b-nu-thai-x-priv
print(InsertUnicodeExtensionAndCanonicalize('en', [], []));
// CHECK-NEXT: en

print(JSON.stringify(CanonicalizeUValue('ca', 'GREGORY')));
// CHECK-NEXT: "gregory"
```

`intl/test/lit/js/resolve-locale.js`:
```js
function localeData(locale) {
  return {
    __proto__: null,
    ca: ['gregory', 'buddhist'],
    kn: ['false', 'true'],
    tag: 'data for ' + locale,
  };
}
// Records which prefixes lookup asks about.
var asked = [];
function isAvailableLogged(tag) {
  asked.push(tag);
  return isAvailableLocale(tag);
}
function resolve(locales, options) {
  var r = ResolveLocale(isAvailableLogged, locales, options, ['ca', 'kn'], localeData);
  return r.locale + ' ca=' + r.ca + ' kn=' + r.kn + ' (' + r.localeData.tag + ')';
}
var none = {__proto__: null, ca: undefined, kn: undefined};

print(resolve(['en-GB'], none));
// CHECK: en ca=gregory kn=false (data for en)
print(resolve(['xx'], none));
// CHECK-NEXT: fr ca=gregory kn=false (data for fr)
print(resolve(['de-u-ca-buddhist'], none));
// CHECK-NEXT: de-u-ca-buddhist ca=buddhist kn=false (data for de)
print(resolve(['de-u-ca-chinese'], none));
// CHECK-NEXT: de ca=gregory kn=false (data for de)
print(resolve(['de-u-ca-buddhist'], {__proto__: null, ca: 'gregory', kn: undefined}));
// CHECK-NEXT: de ca=gregory kn=false (data for de)
print(resolve(['en-u-kn'], none));
// CHECK-NEXT: en-u-kn ca=gregory kn=true (data for en)
asked = [];
print(resolve(['en-US-x-foo'], none));
// CHECK-NEXT: en-US ca=gregory kn=false (data for en-US)
// Lookup skips the dangling singleton: it never asks about "en-US-x".
print(asked.join(' '));
// CHECK-NEXT: en-US-x-foo en-US
print(resolve(['xx', 'de-AT'], none));
// CHECK-NEXT: de ca=gregory kn=false (data for de)
var noData = ResolveLocale(isAvailableLocale, ['de'], none, [], undefined);
print(noData.locale, noData.localeData);
// CHECK-NEXT: de undefined

print(DefaultLocale());
// CHECK-NEXT: fr

print(JSON.stringify(LookupSupportedLocales(isAvailableLocale, ['de-AT', 'xx', 'en-u-ca-gregory'])));
// CHECK-NEXT: ["de-AT","en-u-ca-gregory"]
print(JSON.stringify(SupportedLocales(isAvailableLocale, ['de-AT', 'xx'], undefined)));
// CHECK-NEXT: ["de-AT"]
print(thrown(() => SupportedLocales(isAvailableLocale, [], {localeMatcher: 'nope'})));
// CHECK-NEXT: RangeError
print(thrown(() => SupportedLocales(isAvailableLocale, [], null)));
// CHECK-NEXT: TypeError
```

`intl/test/lit/js/options.js`:
```js
var opts = {
  a: 'x', c: 0, n: '2.7', bad: 'zzz',
  obj: {toString() { return 'lookup'; }},
};
print(GetOption(opts, 'a', 'string', undefined, 'd'));
// CHECK: x
print(GetOption(opts, 'missing', 'string', undefined, 'd'));
// CHECK-NEXT: d
print(GetOption(opts, 'c', 'boolean', undefined, true));
// CHECK-NEXT: false
print(GetOption(opts, 'obj', 'string', ['lookup', 'best fit'], 'best fit'));
// CHECK-NEXT: lookup
print(thrown(() => GetOption(opts, 'bad', 'string', ['lookup'], 'lookup')));
// CHECK-NEXT: RangeError
print(thrown(() => GetOption(opts, 'missing', 'string', undefined, REQUIRED)));
// CHECK-NEXT: TypeError
print(thrown(() => GetOption({s: Symbol()}, 's', 'string', undefined, 'd')));
// CHECK-NEXT: TypeError

print(GetNumberOption(opts, 'n', 1, 5, 3));
// CHECK-NEXT: 2
print(GetNumberOption(opts, 'missing', 1, 5, 3));
// CHECK-NEXT: 3
print(thrown(() => GetNumberOption({v: 'abc'}, 'v', 0, 5, 1)));
// CHECK-NEXT: RangeError
print(thrown(() => GetNumberOption({v: 6}, 'v', 0, 5, 1)));
// CHECK-NEXT: RangeError
print(thrown(() => GetNumberOption({v: 1n}, 'v', 0, 5, 1)));
// CHECK-NEXT: TypeError

var values = ['min2', 'auto', 'always'];
function bs(v) {
  return String(GetBooleanOrStringNumberFormatOption({g: v}, 'g', values, 'auto'));
}
print([bs(true), bs(false), bs('min2'), bs(0), bs(''), bs(undefined)].join(' '));
// CHECK-NEXT: true false min2 false false auto
// The strings "true" and "false" are ordinary strings here; NumberFormat's
// useGrouping maps them to its fallback at its own call site (Phase 3).
print(thrown(() => bs('false')));
// CHECK-NEXT: RangeError
print(thrown(() => bs('x')));
// CHECK-NEXT: RangeError

print(Object.getPrototypeOf(GetOptionsObject(undefined)));
// CHECK-NEXT: null
print(thrown(() => GetOptionsObject('x')));
// CHECK-NEXT: TypeError
print(typeof CoerceOptionsToObject('x'));
// CHECK-NEXT: object
```

`intl/test/lit/js/digit-options.js`:
```js
function digits(options, notation) {
  var r = {__proto__: null};
  SetNumberFormatDigitOptions(r, options, 0, 3, notation || 'standard');
  return [
    r.minimumIntegerDigits, r.minimumFractionDigits, r.maximumFractionDigits,
    r.minimumSignificantDigits, r.maximumSignificantDigits, r.roundingType,
    r.computedRoundingPriority, r.roundingIncrement, r.roundingMode,
    r.trailingZeroDisplay,
  ].join('|');
}
print(digits({}));
// CHECK: 1|0|3|||fractionDigits|auto|1|halfExpand|auto
print(digits({maximumSignificantDigits: 3}));
// CHECK-NEXT: 1|||1|3|significantDigits|auto|1|halfExpand|auto
print(digits({minimumFractionDigits: 2}));
// CHECK-NEXT: 1|2|3|||fractionDigits|auto|1|halfExpand|auto
print(digits({maximumFractionDigits: 1}));
// CHECK-NEXT: 1|0|1|||fractionDigits|auto|1|halfExpand|auto
print(digits({roundingPriority: 'lessPrecision', maximumSignificantDigits: 2}));
// CHECK-NEXT: 1|0|3|1|2|lessPrecision|lessPrecision|1|halfExpand|auto
print(digits({}, 'compact'));
// CHECK-NEXT: 1|0|0|1|2|morePrecision|morePrecision|1|halfExpand|auto
print(digits({roundingIncrement: 5, minimumFractionDigits: 2, maximumFractionDigits: 2}));
// CHECK-NEXT: 1|2|2|||fractionDigits|auto|5|halfExpand|auto
print(digits({minimumIntegerDigits: 4.9, roundingMode: 'trunc', trailingZeroDisplay: 'stripIfInteger'}));
// CHECK-NEXT: 4|0|3|||fractionDigits|auto|1|trunc|stripIfInteger
print(thrown(() => digits({minimumFractionDigits: 3, maximumFractionDigits: 1})));
// CHECK-NEXT: RangeError
print(thrown(() => digits({roundingIncrement: 5, maximumFractionDigits: 2})));
// CHECK-NEXT: RangeError
print(thrown(() => digits({roundingIncrement: 3})));
// CHECK-NEXT: RangeError
print(thrown(() => digits({roundingIncrement: 5, maximumSignificantDigits: 2})));
// CHECK-NEXT: TypeError
print(thrown(() => digits({roundingMode: 'bogus'})));
// CHECK-NEXT: RangeError

// Every option is read once, in spec order, before anything is validated.
var reads = [];
digits(new Proxy({}, {get(t, k) { reads.push(String(k)); return undefined; }}));
print(reads.join(','));
// CHECK-NEXT: minimumIntegerDigits,minimumFractionDigits,maximumFractionDigits,minimumSignificantDigits,maximumSignificantDigits,roundingIncrement,roundingMode,roundingPriority,trailingZeroDisplay
```

`intl/test/lit/js/shapes.js`:
```js
/// Whether f can be called with new, without calling it.
function isConstructor(f) {
  try {
    Reflect.construct(String, [], f);
    return true;
  } catch (e) {
    return false;
  }
}

var target = {};
defineMethods(target, {foo(a, b) { return a + b; }});
var d = Object.getOwnPropertyDescriptor(target, 'foo');
print(d.writable, d.enumerable, d.configurable, d.value.name, d.value.length, 'prototype' in d.value, isConstructor(d.value));
// CHECK: true false true foo 2 false false

defineGetters(target, {get bar() { return 1; }});
var g = Object.getOwnPropertyDescriptor(target, 'bar');
print(typeof g.get, g.set, g.enumerable, g.configurable, g.get.name);
// CHECK-NEXT: function undefined false true get bar

function Ctor(a) {}
var proto = {};
defineConstructor(Ctor, 0, proto);
var pd = Object.getOwnPropertyDescriptor(Ctor, 'prototype');
print(pd.value === proto, pd.writable, pd.enumerable, pd.configurable);
// CHECK-NEXT: true false false false
var ld = Object.getOwnPropertyDescriptor(Ctor, 'length');
print(ld.value, ld.writable, ld.enumerable, ld.configurable);
// CHECK-NEXT: 0 false false true
var cd = Object.getOwnPropertyDescriptor(proto, 'constructor');
print(cd.value === Ctor, cd.writable, cd.enumerable, cd.configurable);
// CHECK-NEXT: true true false true

// The functions the format/compare getters return: arrows from a factory.
function makeBound() {
  return (x) => x;
}
var bound = makeBound();
print(JSON.stringify(bound.name), bound.length, 'prototype' in bound, isConstructor(bound));
// CHECK-NEXT: "" 1 false false

setToStringTag(target, 'T');
var td = Object.getOwnPropertyDescriptor(target, Symbol.toStringTag);
print(td.value, td.writable, td.enumerable, td.configurable);
// CHECK-NEXT: T false false true

// The private-field pattern (spec "Constructors"): a subclass of
// ReturnOverride puts its private fields on the object passed to it.
class FooSlots extends ReturnOverride {
  #v;
  constructor(o, v) {
    super(o);
    this.#v = v;
  }
  static has(o) {
    return #v in o;
  }
  static get(o) {
    return o.#v;
  }
}
function Foo(v) {
  var o = Object.create(Foo.prototype);
  new FooSlots(o, v);
  return o;
}
var f = Foo(7);
print(FooSlots.has(f), FooSlots.get(f), FooSlots.has({}), Reflect.ownKeys(f).length, Object.getPrototypeOf(f) === Foo.prototype);
// CHECK-NEXT: true 7 false 0 true
print(thrown(() => new FooSlots(f, 8)));
// CHECK-NEXT: TypeError
var px = new Proxy({}, {});
new FooSlots(px, 9);
print(FooSlots.has(px), FooSlots.get(px));
// CHECK-NEXT: true 9
```

Then add a lit config for the subdirectory that excludes `Inputs` (the top-level `lit.cfg` already lists `Inputs` in `config.excludes`, which applies to subdirectories).

- [ ] **Step 2: Run them and watch them fail**

`LIT_FILTER=intl/test/lit/js cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-intl-lit`
(If `LIT_FILTER` is not honored by `add_lit_testsuite` here, run lit directly: `python3 /home/tmikov/work/hermes-intl/cmake-build-asan/bin/hermes-lit -sv <the params check-intl-lit uses> /home/tmikov/work/hermes-intl/intl/test/lit/js`. Get the params from `ninja -C /home/tmikov/work/hermes-intl/cmake-build-asan -t commands check-intl-lit`.)
Expected: all six fail with `ReferenceError`s for the missing functions.

- [ ] **Step 3: Write `01-intrinsics.js` in full**

Replace `intl/js/01-intrinsics.js` with:
```js
/**
 * (Meta MIT header)
 */

// Every intrinsic the implementation uses, captured when the plugin's
// bytecode runs, before any user code; and the ECMA-262 conversions built on
// them. Plugin code calls only these.

var global = globalThis;
var _Object = Object;
var ObjectCreate = Object.create;
var ObjectDefineProperty = Object.defineProperty;
var ObjectGetOwnPropertyDescriptor = Object.getOwnPropertyDescriptor;
var ReflectApply = Reflect.apply;
var ReflectOwnKeys = Reflect.ownKeys;
var _TypeError = TypeError;
var _RangeError = RangeError;
var SymbolToStringTag = Symbol.toStringTag;
var MathFloor = Math.floor;
var MathMax = Math.max;
var MathMin = Math.min;
var MathTrunc = Math.trunc;
var NumberIsFinite = Number.isFinite;
var StringFromCharCode = String.fromCharCode;
var StringPrototypeCharCodeAt = String.prototype.charCodeAt;
var StringPrototypeIndexOf = String.prototype.indexOf;
var StringPrototypeSlice = String.prototype.slice;

/// s.indexOf(search, from) through the captured method.
function stringIndexOf(s, search, from) {
  return ReflectApply(StringPrototypeIndexOf, s, [search, from]);
}

/// s.slice(start, end) through the captured method.
function stringSlice(s, start, end) {
  return ReflectApply(StringPrototypeSlice, s, [start, end]);
}

/// s.charCodeAt(i) through the captured method.
function charCodeAt(s, i) {
  return ReflectApply(StringPrototypeCharCodeAt, s, [i]);
}

// ECMA-262 7.1.17 ToString: a template literal performs exactly ToString,
// including the TypeError for symbols.
function ToString(v) {
  return `${v}`;
}

// ECMA-262 7.1.18 ToObject
function ToObject(v) {
  if (v === undefined || v === null)
    throw new _TypeError('Cannot convert undefined or null to object');
  return _Object(v);
}

// ECMA-262 7.1.4 ToNumber: unary plus, including the TypeError for BigInt and
// symbols.
function ToNumber(v) {
  return +v;
}

// ECMA-262 7.1.5 ToIntegerOrInfinity
function ToIntegerOrInfinity(v) {
  var n = ToNumber(v);
  if (n !== n || n === 0) return 0;
  if (!NumberIsFinite(n)) return n;
  return MathTrunc(n);
}

// ECMA-262 7.1.20 ToLength
function ToLength(v) {
  var len = ToIntegerOrInfinity(v);
  if (len <= 0) return 0;
  return MathMin(len, 9007199254740991);
}

/// The ASCII-lowercase of s (Infra): only A-Z change.
function asciiLowercase(s) {
  var out = '';
  for (var i = 0; i < s.length; i++) {
    var c = charCodeAt(s, i);
    out += StringFromCharCode(c >= 0x41 && c <= 0x5a ? c + 0x20 : c);
  }
  return out;
}
```

- [ ] **Step 4: Write `02-shapes.js` in full**

Replace `intl/js/02-shapes.js` with:
```js
/**
 * (Meta MIT header)
 */

// Lists, and the function-shape rules (spec "Function shapes"). Everything the
// plugin installs goes through these helpers.

/// Append v to list without consulting Array.prototype.
function listPush(list, v) {
  ObjectDefineProperty(list, list.length, {
    __proto__: null,
    value: v,
    writable: true,
    enumerable: true,
    configurable: true,
  });
}

/// Whether list contains v (SameValueZero is not needed: lists hold strings).
function listContains(list, v) {
  for (var i = 0; i < list.length; i++) {
    if (list[i] === v) return true;
  }
  return false;
}

// ECMA-262 7.3.16 CreateArrayFromList
function CreateArrayFromList(list) {
  var array = [];
  for (var i = 0; i < list.length; i++) listPush(array, list[i]);
  return array;
}

/// Install the concise methods of source on target: writable,
/// non-enumerable, configurable. Concise methods are not constructors and
/// have no prototype property; their name and length come from the source.
function defineMethods(target, source) {
  var keys = ReflectOwnKeys(source);
  for (var i = 0; i < keys.length; i++) {
    var d = ObjectGetOwnPropertyDescriptor(source, keys[i]);
    ObjectDefineProperty(target, keys[i], {
      __proto__: null,
      value: d.value,
      writable: true,
      enumerable: false,
      configurable: true,
    });
  }
}

/// Install the getters of source on target: non-enumerable, configurable.
function defineGetters(target, source) {
  var keys = ReflectOwnKeys(source);
  for (var i = 0; i < keys.length; i++) {
    var d = ObjectGetOwnPropertyDescriptor(source, keys[i]);
    ObjectDefineProperty(target, keys[i], {
      __proto__: null,
      get: d.get,
      enumerable: false,
      configurable: true,
    });
  }
}

/// Shape the plain function fn as a constructor: its length, its prototype
/// (non-writable, non-enumerable, non-configurable), and
/// proto.constructor.
function defineConstructor(fn, length, proto) {
  ObjectDefineProperty(fn, 'length', {
    __proto__: null,
    value: length,
    writable: false,
    enumerable: false,
    configurable: true,
  });
  ObjectDefineProperty(fn, 'prototype', {
    __proto__: null,
    value: proto,
    writable: false,
    enumerable: false,
    configurable: false,
  });
  ObjectDefineProperty(proto, 'constructor', {
    __proto__: null,
    value: fn,
    writable: true,
    enumerable: false,
    configurable: true,
  });
}

/// Define obj[@@toStringTag] = tag: non-writable, non-enumerable,
/// configurable.
function setToStringTag(obj, tag) {
  ObjectDefineProperty(obj, SymbolToStringTag, {
    __proto__: null,
    value: tag,
    writable: false,
    enumerable: false,
    configurable: true,
  });
}

/// The base of every internal-slots class (spec "Constructors"): its
/// constructor returns the object passed to it, so a subclass's private
/// fields are added to that object.
class ReturnOverride {
  constructor(o) {
    return o;
  }
}
```

- [ ] **Step 5: Write `03-locales.js`**

Put the ECMA-402 draft date in the header comment, per the JS rules. Number every step against that draft; where this plan's code differs from the draft, follow the draft.
```js
/**
 * (Meta MIT header)
 */

// ECMA-402 9: Locale and Parameter Negotiation, against the ECMA-402 draft
// of <date of the draft you read>. Locale tags here are canonical (they come
// from CanonicalizeLocaleList), so Unicode extensions are lowercase and
// sorted.

/// Whether o has an [[InitializedLocale]] slot. Phase 1 (Intl.Locale)
/// replaces this; until then no object has one.
function isLocaleObject(o) {
  return false;
}

/// o.[[Locale]] for an Intl.Locale object; defined by Phase 1.
function localeObjectTag(o) {
  throw new _TypeError('Intl.Locale is not implemented');
}

/// Whether the locale data covers tag (canonical, without extensions).
function isAvailableLocale(tag) {
  return native.isAvailableLocale(tag);
}

// 9.2.1 CanonicalizeLocaleList ( locales )
function CanonicalizeLocaleList(locales) {
  // 1. If locales is undefined, return a new empty List.
  if (locales === undefined) return [];
  // 2. Let seen be a new empty List.
  var seen = [];
  var O;
  // 3. If locales is a String or has an [[InitializedLocale]] internal slot,
  //    let O be CreateArrayFromList(« locales »).
  if (typeof locales === 'string' || isLocaleObject(locales)) {
    O = CreateArrayFromList([locales]);
  } else {
    // 4. Else, let O be ? ToObject(locales).
    O = ToObject(locales);
  }
  // 5. Let len be ? LengthOfArrayLike(O).
  var len = ToLength(O.length);
  // 6. Let k be 0. 7. Repeat, while k < len,
  for (var k = 0; k < len; k++) {
    // a. Let Pk be ! ToString(𝔽(k)).
    var Pk = ToString(k);
    // b. Let kPresent be ? HasProperty(O, Pk).
    // c. If kPresent is true, then
    if (Pk in O) {
      // i. Let kValue be ? Get(O, Pk).
      var kValue = O[Pk];
      // ii. If kValue is not a String and not an Object, throw a TypeError.
      var type = typeof kValue;
      if (
        type !== 'string' &&
        (kValue === null || (type !== 'object' && type !== 'function'))
      ) {
        throw new _TypeError('Language ID should be string or object');
      }
      // iii-iv. tag is kValue.[[Locale]] for an Intl.Locale, else
      //         ? ToString(kValue).
      var tag = isLocaleObject(kValue) ? localeObjectTag(kValue) : ToString(kValue);
      // v. If IsWellFormedLanguageTag(tag) is false, throw a RangeError.
      // vi. Let canonicalizedTag be CanonicalizeUnicodeLocaleId(tag).
      var canonicalizedTag = native.canonicalizeLocale(tag);
      if (canonicalizedTag === undefined)
        throw new _RangeError('Incorrect locale information provided');
      // vii. If seen does not contain canonicalizedTag, append it to seen.
      if (!listContains(seen, canonicalizedTag)) listPush(seen, canonicalizedTag);
    }
  }
  // 8. Return seen.
  return seen;
}

/// Split the Unicode locale extension sequence ("-u-..." up to the next
/// singleton) off a canonical locale: {base, extension}, extension
/// undefined if there is none. A "-u-" inside a private use sequence is not
/// one.
function splitUnicodeExtension(locale) {
  var start = -1;
  var end = locale.length;
  var pos = 0;
  while (pos < locale.length) {
    var dash = stringIndexOf(locale, '-', pos);
    var next = dash === -1 ? locale.length : dash;
    if (next - pos === 1) {
      // A singleton ends the extension, if one has started.
      if (start !== -1) {
        end = pos - 1;
        break;
      }
      var c = charCodeAt(locale, pos);
      // Nothing after "x" (private use) is an extension.
      if (c === 0x78) break;
      if (c === 0x75) start = pos - 1;
    }
    pos = next + 1;
  }
  if (start === -1)
    return {__proto__: null, base: locale, extension: undefined};
  return {
    __proto__: null,
    base: stringSlice(locale, 0, start) + stringSlice(locale, end),
    extension: stringSlice(locale, start, end),
  };
}

/// The element of keywords whose key is key, or undefined.
function findKeyword(keywords, key) {
  for (var i = 0; i < keywords.length; i++) {
    if (keywords[i].key === key) return keywords[i];
  }
  return undefined;
}

// 9.2.5 UnicodeExtensionComponents ( extension )
function UnicodeExtensionComponents(extension) {
  // 1. Let attributes be a new empty List.
  var attributes = [];
  // 2. Let keywords be a new empty List.
  var keywords = [];
  // 3. Let keyword be undefined.
  var keyword = undefined;
  // 4. Let size be the length of extension.
  var size = extension.length;
  // 5. Let k be 3.
  var k = 3;
  // 6. Repeat, while k < size,
  while (k < size) {
    // a. Let e be StringIndexOf(extension, "-", k).
    var e = stringIndexOf(extension, '-', k);
    // b. If e = -1, let len be size - k; else let len be e - k.
    var len = e === -1 ? size - k : e - k;
    // c. Let subtag be the substring of extension from k to k + len.
    var subtag = stringSlice(extension, k, k + len);
    // d. NOTE: A keyword is a key of length 2 followed by value subtags of
    //    length 3 to 8; an attribute is a subtag of length 3 to 8 before all
    //    keywords.
    if (keyword === undefined && len !== 2) {
      // e. If keyword is undefined and length ≠ 2, then
      //    i. If subtag is not an element of attributes, append it.
      if (!listContains(attributes, subtag)) listPush(attributes, subtag);
    } else if (len === 2) {
      // f. Else if length = 2, then
      //    i. Set keyword to the Record { [[Key]]: subtag, [[Value]]: "" }.
      keyword = {__proto__: null, key: subtag, value: ''};
      //    ii. If keywords does not contain an element whose [[Key]] is
      //        keyword.[[Key]], append keyword to keywords. (A repeated key's
      //        record is filled in below but never appended.)
      if (findKeyword(keywords, keyword.key) === undefined) listPush(keywords, keyword);
    } else if (keyword.value === '') {
      // g. Else if keyword.[[Value]] is the empty String, set it to subtag.
      keyword.value = subtag;
    } else {
      // h. Else, set it to keyword.[[Value]] + "-" + subtag.
      keyword.value = keyword.value + '-' + subtag;
    }
    // i. Set k to k + length + 1.
    k = k + len + 1;
  }
  // 7. Return the Record { [[Attributes]]: attributes, [[Keywords]]: keywords }.
  return {__proto__: null, attributes: attributes, keywords: keywords};
}

// 9.2.6 InsertUnicodeExtensionAndCanonicalize ( locale, attributes, keywords )
function InsertUnicodeExtensionAndCanonicalize(locale, attributes, keywords) {
  // 1. Assert: locale does not contain a Unicode locale extension sequence.
  // 2. Let extension be "-u".
  var extension = '-u';
  // 3. For each element attribute of attributes, append "-" and attribute.
  for (var i = 0; i < attributes.length; i++)
    extension = extension + '-' + attributes[i];
  // 4. For each Record keyword of keywords: append "-" and keyword.[[Key]],
  //    and if keyword.[[Value]] is not "", "-" and keyword.[[Value]].
  for (var j = 0; j < keywords.length; j++) {
    extension = extension + '-' + keywords[j].key;
    if (keywords[j].value !== '') extension = extension + '-' + keywords[j].value;
  }
  var newLocale;
  // 5. If extension is "-u", return CanonicalizeUnicodeLocaleId(locale).
  if (extension === '-u') {
    newLocale = locale;
  } else {
    // 6. Let privateIndex be StringIndexOf(locale, "-x-", 0).
    var privateIndex = stringIndexOf(locale, '-x-', 0);
    // 7-8. Insert extension before the private use sequence, if any.
    newLocale =
      privateIndex === -1
        ? locale + extension
        : stringSlice(locale, 0, privateIndex) + extension + stringSlice(locale, privateIndex);
  }
  // 9. Assert: IsWellFormedLanguageTag(newLocale) is true.
  // 10. Return CanonicalizeUnicodeLocaleId(newLocale).
  var canonical = native.canonicalizeLocale(newLocale);
  if (canonical === undefined)
    throw new _TypeError('Intl internal error: ill-formed locale ' + newLocale);
  return canonical;
}

// 9.2.2 CanonicalizeUValue ( ukey, uvalue )
function CanonicalizeUValue(ukey, uvalue) {
  // 1. Let lowerValue be the ASCII-lowercase of uvalue.
  var lowerValue = asciiLowercase(uvalue);
  // 2. Let canonicalized be lowerValue canonicalized as a value of key ukey
  //    per UTS 35 Annex C. The canonicalizer works on whole tags, so wrap
  //    the value in one and read it back.
  var tag = native.canonicalizeLocale('und-u-' + ukey + '-' + lowerValue);
  if (tag === undefined) return lowerValue;
  var keyword = findKeyword(
    UnicodeExtensionComponents(splitUnicodeExtension(tag).extension).keywords,
    ukey,
  );
  // 3-4. Return canonicalized.
  return keyword === undefined ? lowerValue : keyword.value;
}

// 9.2.3 LookupMatchingLocaleByPrefix ( availableLocales, requestedLocales )
// availableLocales is given as a membership predicate.
function LookupMatchingLocaleByPrefix(isAvailable, requestedLocales) {
  // 1. For each element locale of requestedLocales, do
  for (var i = 0; i < requestedLocales.length; i++) {
    // a. Let extension be empty.
    // b. If locale contains a Unicode locale extension sequence, set
    //    extension to it and remove it from locale.
    var parts = splitUnicodeExtension(requestedLocales[i]);
    // c. Let prefix be locale.
    var prefix = parts.base;
    // d. Repeat, while prefix is not the empty String,
    while (prefix !== '') {
      // i. If availableLocales contains prefix, return the Record
      //    { [[locale]]: prefix, [[extension]]: extension }.
      if (isAvailable(prefix))
        return {__proto__: null, locale: prefix, extension: parts.extension};
      // ii. Let pos be the index of the last "-" in prefix, or 0.
      var pos = 0;
      for (var p = prefix.length - 1; p >= 0; p--) {
        if (charCodeAt(prefix, p) === 0x2d) {
          pos = p;
          break;
        }
      }
      // iii. Repeat, while pos ≥ 2 and the substring of prefix from pos - 2
      //      to pos - 1 is "-", set pos to pos - 2.
      while (pos >= 2 && charCodeAt(prefix, pos - 2) === 0x2d) pos = pos - 2;
      // iv. Set prefix to the substring of prefix from 0 to pos.
      prefix = stringSlice(prefix, 0, pos);
    }
  }
  // 2. Return undefined.
  return undefined;
}

/// DefaultLocale(): the first host locale with available data, matched by
/// prefix, without extensions; "en" if none has any.
var defaultLocaleCache;
function DefaultLocale() {
  if (defaultLocaleCache === undefined) {
    var locales = hostDefaults.locales;
    for (var i = 0; i < locales.length && defaultLocaleCache === undefined; i++) {
      var match = LookupMatchingLocaleByPrefix(isAvailableLocale, [locales[i]]);
      if (match !== undefined) defaultLocaleCache = match.locale;
    }
    if (defaultLocaleCache === undefined) defaultLocaleCache = 'en';
  }
  return defaultLocaleCache;
}

// 9.2.7 ResolveLocale ( availableLocales, requestedLocales, options,
//                       relevantExtensionKeys [ , localeData ] )
// availableLocales is a predicate. localeData is undefined (not present) or
// a function: localeData(locale) is the spec's localeData.[[<locale>]], a
// record with a list per relevant key, default first. "best fit" behaves
// as "lookup", which the spec permits, so options.[[localeMatcher]] does
// not change the result.
function ResolveLocale(isAvailable, requestedLocales, options, relevantExtensionKeys, localeData) {
  // 1-3. Let match be LookupMatchingLocaleByPrefix(availableLocales,
  //      requestedLocales) (or ByBestFit, which is the same here).
  var r = LookupMatchingLocaleByPrefix(isAvailable, requestedLocales);
  // 4. If match is undefined, set match to { [[locale]]: DefaultLocale(),
  //    [[extension]]: empty }.
  if (r === undefined)
    r = {__proto__: null, locale: DefaultLocale(), extension: undefined};
  // 5. Let foundLocale be match.[[locale]].
  var foundLocale = r.locale;
  // 6. Let result be a new Record.
  var result = {__proto__: null};
  var foundLocaleData;
  if (localeData === undefined) {
    // 7. If localeData is not present, then
    //    a. Assert: relevantExtensionKeys is an empty List.
    //    b. Set result.[[LocaleData]] to undefined.
    result.localeData = undefined;
  } else {
    // 8. Else,
    //    a. Let foundLocaleData be localeData.[[<foundLocale>]].
    foundLocaleData = localeData(foundLocale);
    //    b. Assert: foundLocaleData is a Record.
    //    c. Set result.[[LocaleData]] to foundLocaleData.
    result.localeData = foundLocaleData;
  }
  // 9-10. Let keywords be the keywords of match.[[extension]], or empty.
  var keywords =
    r.extension !== undefined ? UnicodeExtensionComponents(r.extension).keywords : [];
  // 11. Let supportedKeywords be a new empty List.
  var supportedKeywords = [];
  // 12. For each element key of relevantExtensionKeys, do
  for (var i = 0; i < relevantExtensionKeys.length; i++) {
    var key = relevantExtensionKeys[i];
    // a. Let keyLocaleData be foundLocaleData.[[<key>]].
    var keyLocaleData = foundLocaleData[key];
    // b-c. Let value be keyLocaleData[0].
    var value = keyLocaleData[0];
    // d. Let supportedKeyword be empty.
    var supportedKeyword = undefined;
    // e. If keywords contains an element whose [[Key]] is key, then
    var entry = findKeyword(keywords, key);
    if (entry !== undefined) {
      // i. Let entry be that element. ii. Let requestedValue be
      //    entry.[[Value]].
      var requestedValue = entry.value;
      if (requestedValue !== '') {
        // iii. If requestedValue is not the empty String, then
        //   1. If keyLocaleData contains requestedValue, then
        if (listContains(keyLocaleData, requestedValue)) {
          // a. Set value to requestedValue.
          value = requestedValue;
          // b. Set supportedKeyword to { [[Key]]: key, [[Value]]: value }.
          supportedKeyword = {__proto__: null, key: key, value: value};
        }
      } else if (listContains(keyLocaleData, 'true')) {
        // iv. Else if keyLocaleData contains "true", then
        //   1. Set value to "true".
        value = 'true';
        //   2. Set supportedKeyword to { [[Key]]: key, [[Value]]: "" }.
        supportedKeyword = {__proto__: null, key: key, value: ''};
      }
    }
    // f-g. Let optionsValue be options.[[<key>]].
    var optionsValue = options[key];
    // h. Assert: optionsValue is a String, or undefined or null.
    // i. If optionsValue is a String, then
    if (typeof optionsValue === 'string') {
      // i. Let ukey be the ASCII-lowercase of key.
      // ii. Set optionsValue to CanonicalizeUValue(ukey, optionsValue).
      optionsValue = CanonicalizeUValue(asciiLowercase(key), optionsValue);
      // iii. If optionsValue is the empty String, set it to "true".
      if (optionsValue === '') optionsValue = 'true';
    }
    // j. If SameValue(optionsValue, value) is false and keyLocaleData
    //    contains optionsValue, then
    if (optionsValue !== value && listContains(keyLocaleData, optionsValue)) {
      // i. Set value to optionsValue. ii. Set supportedKeyword to empty.
      value = optionsValue;
      supportedKeyword = undefined;
    }
    // k. If supportedKeyword is not empty, append it to supportedKeywords.
    if (supportedKeyword !== undefined) listPush(supportedKeywords, supportedKeyword);
    // l. Set result.[[<key>]] to value.
    result[key] = value;
  }
  // 13. If supportedKeywords is not empty, then
  if (supportedKeywords.length > 0) {
    // a-b. Set foundLocale to InsertUnicodeExtensionAndCanonicalize(
    //      foundLocale, « », supportedKeywords).
    foundLocale = InsertUnicodeExtensionAndCanonicalize(foundLocale, [], supportedKeywords);
  }
  // 14. Set result.[[Locale]] to foundLocale.
  result.locale = foundLocale;
  // 15. Return result.
  return result;
}

// 9.2.? LookupSupportedLocales ( availableLocales, requestedLocales )
function LookupSupportedLocales(isAvailable, requestedLocales) {
  // 1. Let subset be a new empty List.
  var subset = [];
  // 2. For each element locale of requestedLocales, do
  for (var i = 0; i < requestedLocales.length; i++) {
    // a. Let match be LookupMatchingLocaleByPrefix(availableLocales,
    //    « locale »).
    // b. If match is not undefined, append locale to subset.
    if (LookupMatchingLocaleByPrefix(isAvailable, [requestedLocales[i]]) !== undefined)
      listPush(subset, requestedLocales[i]);
  }
  // 3. Return subset.
  return subset;
}

// 9.2.? SupportedLocales ( availableLocales, requestedLocales, options )
function SupportedLocales(isAvailable, requestedLocales, options) {
  // 1. Set options to ? CoerceOptionsToObject(options).
  options = CoerceOptionsToObject(options);
  // 2. Let matcher be ? GetOption(options, "localeMatcher", string,
  //    « "lookup", "best fit" », "best fit").
  GetOption(options, 'localeMatcher', 'string', ['lookup', 'best fit'], 'best fit');
  // 3-4. "best fit" behaves as "lookup".
  var supportedLocales = LookupSupportedLocales(isAvailable, requestedLocales);
  // 5. Return CreateArrayFromList(supportedLocales).
  return CreateArrayFromList(supportedLocales);
}
```
Replace each `9.2.?` / `6.2.?` with the section number in the draft you follow.

- [ ] **Step 6: Write `04-options.js`**

```js
/**
 * (Meta MIT header)
 */

// ECMA-402 options processing, against the ECMA-402 draft of <date>.

/// The `default` argument of GetOption that makes the option required.
var REQUIRED = {__proto__: null};

// 9.2.? GetOptionsObject ( options )
function GetOptionsObject(options) {
  // 1. If options is undefined, return OrdinaryObjectCreate(null).
  if (options === undefined) return ObjectCreate(null);
  // 2. If options is an Object, return options.
  if ((typeof options === 'object' && options !== null) || typeof options === 'function')
    return options;
  // 3. Throw a TypeError exception.
  throw new _TypeError('Options must be an object');
}

// 9.2.? CoerceOptionsToObject ( options )
function CoerceOptionsToObject(options) {
  // 1. If options is undefined, return OrdinaryObjectCreate(null).
  if (options === undefined) return ObjectCreate(null);
  // 2. Return ? ToObject(options).
  return ToObject(options);
}

// 9.2.11 GetOption ( options, propertyKey, type, values, default )
// values is a list, or undefined for "empty"; default may be REQUIRED.
function GetOption(options, property, type, values, defaultValue) {
  // 1. Let value be ? Get(options, property).
  var value = options[property];
  // 2. If value is undefined, then
  if (value === undefined) {
    // a. If default is required, throw a TypeError exception.
    if (defaultValue === REQUIRED)
      throw new _TypeError(property + ' is required');
    // b. Return default.
    return defaultValue;
  }
  if (type === 'boolean') {
    // 3. If type is boolean, set value to ToBoolean(value).
    value = !!value;
  } else {
    // 4. Else, set value to ? ToString(value).
    value = ToString(value);
  }
  // 5. If values is not empty and does not contain value, throw a RangeError.
  if (values !== undefined && !listContains(values, value))
    throw new _RangeError('Value ' + ToString(value) + ' out of range for ' + property);
  // 6. Return value.
  return value;
}

// 9.2.12 GetBooleanOrStringNumberFormatOption ( options, propertyKey,
//                                              stringValues, fallback )
function GetBooleanOrStringNumberFormatOption(options, property, stringValues, fallback) {
  // 1. Let value be ? Get(options, property).
  var value = options[property];
  // 2. If value is undefined, return fallback.
  if (value === undefined) return fallback;
  // 3. If value is true, return true.
  if (value === true) return true;
  // 4. If ToBoolean(value) is false, return false.
  if (!value) return false;
  // 5. Set value to ? ToString(value).
  value = ToString(value);
  // 6. If stringValues does not contain value, throw a RangeError.
  if (!listContains(stringValues, value))
    throw new _RangeError('Value ' + value + ' out of range for ' + property);
  // 7. Return value.
  return value;
}

// 9.2.13 DefaultNumberOption ( value, minimum, maximum, fallback )
// property is only for the error message.
function DefaultNumberOption(value, minimum, maximum, fallback, property) {
  // 1. If value is undefined, return fallback.
  if (value === undefined) return fallback;
  // 2. Set value to ? ToNumber(value).
  value = ToNumber(value);
  // 3. If value is not finite or ℝ(value) < minimum or ℝ(value) > maximum,
  //    throw a RangeError exception.
  if (!NumberIsFinite(value) || value < minimum || value > maximum)
    throw new _RangeError((property === undefined ? 'Value' : property) + ' value is out of range');
  // 4. Return floor(ℝ(value)).
  return MathFloor(value);
}

// 9.2.14 GetNumberOption ( options, propertyKey, minimum, maximum, fallback )
function GetNumberOption(options, property, minimum, maximum, fallback) {
  // 1. Let value be ? Get(options, property).
  // 2. Return ? DefaultNumberOption(value, minimum, maximum, fallback).
  return DefaultNumberOption(options[property], minimum, maximum, fallback, property);
}

/// The roundingIncrement values ECMA-402 allows.
var ROUNDING_INCREMENTS = [1, 2, 5, 10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 2500, 5000];

// 15.1.? SetNumberFormatDigitOptions ( intlObj, options, mnfdDefault,
//                                      mxfdDefault, notation )
// intlObj is a null-prototype record; the slots are lowerCamelCase fields.
function SetNumberFormatDigitOptions(intlObj, options, mnfdDefault, mxfdDefault, notation) {
  // 1. Let mnid be ? GetNumberOption(options, "minimumIntegerDigits", 1, 21, 1).
  var mnid = GetNumberOption(options, 'minimumIntegerDigits', 1, 21, 1);
  // 2. Let mnfd be ? Get(options, "minimumFractionDigits").
  var mnfd = options.minimumFractionDigits;
  // 3. Let mxfd be ? Get(options, "maximumFractionDigits").
  var mxfd = options.maximumFractionDigits;
  // 4. Let mnsd be ? Get(options, "minimumSignificantDigits").
  var mnsd = options.minimumSignificantDigits;
  // 5. Let mxsd be ? Get(options, "maximumSignificantDigits").
  var mxsd = options.maximumSignificantDigits;
  // 6. Set intlObj.[[MinimumIntegerDigits]] to mnid.
  intlObj.minimumIntegerDigits = mnid;
  // 7. Let roundingIncrement be ? GetNumberOption(options,
  //    "roundingIncrement", 1, 5000, 1).
  var roundingIncrement = GetNumberOption(options, 'roundingIncrement', 1, 5000, 1);
  // 8. If roundingIncrement is not in « 1, 2, 5, 10, 20, 25, 50, 100, 200,
  //    250, 500, 1000, 2000, 2500, 5000 », throw a RangeError exception.
  if (!listContains(ROUNDING_INCREMENTS, roundingIncrement))
    throw new _RangeError('roundingIncrement value is out of range');
  // 9. Let roundingMode be ? GetOption(options, "roundingMode", string,
  //    « ... », "halfExpand").
  var roundingMode = GetOption(
    options, 'roundingMode', 'string',
    ['ceil', 'floor', 'expand', 'trunc', 'halfCeil', 'halfFloor', 'halfExpand', 'halfTrunc', 'halfEven'],
    'halfExpand');
  // 10. Let roundingPriority be ? GetOption(options, "roundingPriority",
  //     string, « "auto", "morePrecision", "lessPrecision" », "auto").
  var roundingPriority = GetOption(
    options, 'roundingPriority', 'string', ['auto', 'morePrecision', 'lessPrecision'], 'auto');
  // 11. Let trailingZeroDisplay be ? GetOption(options,
  //     "trailingZeroDisplay", string, « "auto", "stripIfInteger" », "auto").
  var trailingZeroDisplay = GetOption(
    options, 'trailingZeroDisplay', 'string', ['auto', 'stripIfInteger'], 'auto');
  // 12. NOTE: All fields required by SetNumberFormatDigitOptions have now
  //     been read from options.
  // 13. If roundingIncrement is not 1, set mxfdDefault to mnfdDefault.
  if (roundingIncrement !== 1) mxfdDefault = mnfdDefault;
  // 14. Set intlObj.[[RoundingIncrement]] to roundingIncrement.
  intlObj.roundingIncrement = roundingIncrement;
  // 15. Set intlObj.[[RoundingMode]] to roundingMode.
  intlObj.roundingMode = roundingMode;
  // 16. Set intlObj.[[TrailingZeroDisplay]] to trailingZeroDisplay.
  intlObj.trailingZeroDisplay = trailingZeroDisplay;
  // 17. hasSd is false iff mnsd and mxsd are both undefined.
  var hasSd = !(mnsd === undefined && mxsd === undefined);
  // 18. hasFd is false iff mnfd and mxfd are both undefined.
  var hasFd = !(mnfd === undefined && mxfd === undefined);
  // 19. Let needSd be true. 20. Let needFd be true.
  var needSd = true;
  var needFd = true;
  // 21. If roundingPriority is "auto", then
  if (roundingPriority === 'auto') {
    // a. Set needSd to hasSd.
    needSd = hasSd;
    // b. If needSd is true, or hasFd is false and notation is "compact",
    //    set needFd to false.
    if (needSd || (!hasFd && notation === 'compact')) needFd = false;
  }
  // 22. If needSd is true, then
  if (needSd) {
    if (hasSd) {
      // a. If hasSd is true, then
      //    i. Set [[MinimumSignificantDigits]] to ? DefaultNumberOption(mnsd, 1, 21, 1).
      intlObj.minimumSignificantDigits = DefaultNumberOption(mnsd, 1, 21, 1, 'minimumSignificantDigits');
      //    ii. Set [[MaximumSignificantDigits]] to ? DefaultNumberOption(mxsd,
      //        [[MinimumSignificantDigits]], 21, 21).
      intlObj.maximumSignificantDigits = DefaultNumberOption(
        mxsd, intlObj.minimumSignificantDigits, 21, 21, 'maximumSignificantDigits');
    } else {
      // b. Else, set them to 1 and 21.
      intlObj.minimumSignificantDigits = 1;
      intlObj.maximumSignificantDigits = 21;
    }
  }
  // 23. If needFd is true, then
  if (needFd) {
    if (hasFd) {
      // a. If hasFd is true, then
      //    i. Set mnfd to ? DefaultNumberOption(mnfd, 0, 100, undefined).
      mnfd = DefaultNumberOption(mnfd, 0, 100, undefined, 'minimumFractionDigits');
      //    ii. Set mxfd to ? DefaultNumberOption(mxfd, 0, 100, undefined).
      mxfd = DefaultNumberOption(mxfd, 0, 100, undefined, 'maximumFractionDigits');
      if (mnfd === undefined) {
        //  iii. If mnfd is undefined, set mnfd to min(mnfdDefault, mxfd).
        mnfd = MathMin(mnfdDefault, mxfd);
      } else if (mxfd === undefined) {
        //  iv. Else if mxfd is undefined, set mxfd to max(mxfdDefault, mnfd).
        mxfd = MathMax(mxfdDefault, mnfd);
      } else if (mnfd > mxfd) {
        //  v. Else if mnfd is greater than mxfd, throw a RangeError.
        throw new _RangeError('minimumFractionDigits is greater than maximumFractionDigits');
      }
      //    vi-vii.
      intlObj.minimumFractionDigits = mnfd;
      intlObj.maximumFractionDigits = mxfd;
    } else {
      // b. Else, use the defaults.
      intlObj.minimumFractionDigits = mnfdDefault;
      intlObj.maximumFractionDigits = mxfdDefault;
    }
  }
  if (!needSd && !needFd) {
    // 24. If needSd and needFd are both false, then
    intlObj.minimumFractionDigits = 0;
    intlObj.maximumFractionDigits = 0;
    intlObj.minimumSignificantDigits = 1;
    intlObj.maximumSignificantDigits = 2;
    intlObj.roundingType = 'morePrecision';
    intlObj.computedRoundingPriority = 'morePrecision';
  } else if (roundingPriority === 'morePrecision' || roundingPriority === 'lessPrecision') {
    // 25. Else if roundingPriority is "morePrecision" or "lessPrecision".
    intlObj.roundingType = roundingPriority;
    intlObj.computedRoundingPriority = roundingPriority;
  } else if (hasSd) {
    // 26. Else if hasSd is true.
    intlObj.roundingType = 'significantDigits';
    intlObj.computedRoundingPriority = 'auto';
  } else {
    // 27. Else.
    intlObj.roundingType = 'fractionDigits';
    intlObj.computedRoundingPriority = 'auto';
  }
  // 28. If roundingIncrement is not 1, then
  if (roundingIncrement !== 1) {
    // a. If [[RoundingType]] is not fractionDigits, throw a TypeError.
    if (intlObj.roundingType !== 'fractionDigits')
      throw new _TypeError('roundingIncrement requires fraction-digit rounding');
    // b. If [[MaximumFractionDigits]] is not [[MinimumFractionDigits]],
    //    throw a RangeError.
    if (intlObj.maximumFractionDigits !== intlObj.minimumFractionDigits)
      throw new _RangeError('roundingIncrement requires equal minimum and maximum fraction digits');
  }
}
```
These were checked against the ECMA-402 draft of 2026-10-12 (GetOption, GetBooleanOrStringNumberFormatOption, DefaultNumberOption, GetNumberOption, CoerceOptionsToObject, SetNumberFormatDigitOptions through step 23, CanonicalizeLocaleList, UnicodeExtensionComponents, InsertUnicodeExtensionAndCanonicalize, LookupMatchingLocaleByPrefix, ResolveLocale and Intl.supportedValuesOf). Check the remaining steps and anything newer against the draft you read, and record each difference in the report.

- [ ] **Step 7: Run the tests**

Run the isolation suite as in Step 2, then all of `check-intl`.
Expected: the six isolation tests pass, in both source and `-O` bytecode form. `check-intl` passes: the plugin's bytecode now includes the new files, and nothing calls them yet.

- [ ] **Step 8: Show the tests can fail**

Make each change below, run, see the named test fail, then revert it:
- In `LookupMatchingLocaleByPrefix`, delete the `while (pos >= 2 ...)` loop: `resolve-locale.js` fails on the `asked` line (`en-US-x` is queried).
- In `SetNumberFormatDigitOptions`, move the `roundingIncrement` read after `roundingMode`: `digit-options.js` fails on the read order.
- In `UnicodeExtensionComponents`, drop the duplicate-key check: `unicode-extensions.js` fails on `ca=gregory`.
- In `ToString`, use `'' + v`: `options.js` still passes. Then add this case to `options.js`, which tells hint `string` from hint `default`, and see it fail:
  ```js
  print(GetOption({o: {toString() { return 's'; }, valueOf() { return 'v'; }}}, 'o', 'string', undefined, 'd'));
  // CHECK-NEXT: s
  ```
  Keep the new case.

- [ ] **Step 9: Stage**

`git -C /home/tmikov/work/hermes-intl add intl`

### Task 10: `Intl.getCanonicalLocales`, `Intl.supportedValuesOf`, and the shape table

**Files:**
- Modify: `intl/js/10-Intl.js`
- Create: `intl/test/lit/getCanonicalLocales.js`, `intl/test/lit/getCanonicalLocales-edge.js`, `intl/test/lit/supportedValuesOf.js`, `intl/test/lit/shapes-table.js`, `intl/test/lit/tamper.js`

**Interfaces:**
- Consumes: `CanonicalizeLocaleList`, `CreateArrayFromList`, `defineMethods`, `setToStringTag`, `listPush`, `ToString`, `stringIndexOf` and `stringSlice` (Task 9), and `native.supportedValues` (Task 7).
- Produces:
  - `Intl.getCanonicalLocales` and `Intl.supportedValuesOf`;
  - `intl/test/lit/shapes-table.js`, the table every later phase adds its functions to.

- [ ] **Step 1: Write the failing tests**

`intl/test/lit/getCanonicalLocales.js`:
```js
/**
 * (Meta MIT header)
 */

// RUN: %hermes %s | %FileCheck --match-full-lines %s

print(JSON.stringify(Intl.getCanonicalLocales()));
// CHECK: []
print(JSON.stringify(Intl.getCanonicalLocales('EN-us')));
// CHECK-NEXT: ["en-US"]
print(JSON.stringify(Intl.getCanonicalLocales(['iw', 'he', 'de-DE'])));
// CHECK-NEXT: ["he","de-DE"]
print(JSON.stringify(Intl.getCanonicalLocales('EN-latn-us-u-CA-gregory-KN-true')));
// CHECK-NEXT: ["en-Latn-US-u-ca-gregory-kn"]
print(JSON.stringify(Intl.getCanonicalLocales('en-u-ca-islamicc')));
// CHECK-NEXT: ["en-u-ca-islamic-civil"]
var a = Intl.getCanonicalLocales('en');
var b = Intl.getCanonicalLocales('en');
print(Array.isArray(a), a !== b);
// CHECK-NEXT: true true
```

`intl/test/lit/getCanonicalLocales-edge.js`:
```js
/**
 * (Meta MIT header)
 */

// RUN: %hermes %s | %FileCheck --match-full-lines %s

// Inputs a person might pass by mistake: each gives the spec's error, or is
// skipped, and none crashes.

function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}
print(thrown(() => Intl.getCanonicalLocales('en\uD800')));
// CHECK: RangeError
print(thrown(() => Intl.getCanonicalLocales('a'.repeat(10000))));
// CHECK-NEXT: RangeError
print(thrown(() => Intl.getCanonicalLocales({get length() { throw new SyntaxError('length'); }})));
// CHECK-NEXT: SyntaxError
print(JSON.stringify(Intl.getCanonicalLocales({length: -5, 0: 'en'})));
// CHECK-NEXT: []
print(thrown(() => Intl.getCanonicalLocales({length: 2 ** 53, 0: 'en', get 1() { throw new EvalError('stop'); }})));
// CHECK-NEXT: EvalError
print(thrown(() => Intl.getCanonicalLocales([Symbol('x')])));
// CHECK-NEXT: TypeError
print(thrown(() => Intl.getCanonicalLocales([null])));
// CHECK-NEXT: TypeError
print(thrown(() => Intl.getCanonicalLocales(null)));
// CHECK-NEXT: TypeError
print(JSON.stringify(Intl.getCanonicalLocales([, 'de', , 'EN'])));
// CHECK-NEXT: ["de","en"]
print(thrown(() => Intl.getCanonicalLocales(['en_US'])));
// CHECK-NEXT: RangeError
print(thrown(() => Intl.getCanonicalLocales([''])));
// CHECK-NEXT: RangeError
```

`intl/test/lit/supportedValuesOf.js`:
```js
/**
 * (Meta MIT header)
 */

// RUN: %hermes %s | %FileCheck --match-full-lines %s

function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}
/// "ok" if list is a non-empty array sorted by code units without duplicates
/// and containing every element of mustHave.
function check(key, mustHave) {
  var list = Intl.supportedValuesOf(key);
  if (!Array.isArray(list) || list.length === 0) return key + ': empty';
  for (var i = 1; i < list.length; i++) {
    if (!(list[i - 1] < list[i])) return key + ': not sorted at ' + list[i];
  }
  for (var j = 0; j < mustHave.length; j++) {
    if (list.indexOf(mustHave[j]) === -1) return key + ': missing ' + mustHave[j];
  }
  return key + ': ok';
}
print(check('calendar', ['gregory', 'iso8601', 'japanese']));
// CHECK: calendar: ok
print(check('collation', ['phonebk', 'pinyin']));
// CHECK-NEXT: collation: ok
print(check('currency', ['EUR', 'USD']));
// CHECK-NEXT: currency: ok
print(check('numberingSystem', ['arab', 'latn', 'thai']));
// CHECK-NEXT: numberingSystem: ok
print(check('timeZone', ['America/New_York', 'Asia/Kolkata', 'UTC']));
// CHECK-NEXT: timeZone: ok
print(check('unit', ['meter', 'mile-scandinavian', 'percent']));
// CHECK-NEXT: unit: ok
print(Intl.supportedValuesOf('unit').length);
// CHECK-NEXT: 45
print(['standard', 'search'].some((c) => Intl.supportedValuesOf('collation').indexOf(c) !== -1));
// CHECK-NEXT: false
print(Intl.supportedValuesOf('timeZone').indexOf('Etc/UTC'));
// CHECK-NEXT: -1
print(Intl.supportedValuesOf({toString() { return 'calendar'; }}).length > 0);
// CHECK-NEXT: true
print(Intl.supportedValuesOf('unit') !== Intl.supportedValuesOf('unit'));
// CHECK-NEXT: true
print(thrown(() => Intl.supportedValuesOf('Calendar')));
// CHECK-NEXT: RangeError
print(thrown(() => Intl.supportedValuesOf(Symbol())));
// CHECK-NEXT: TypeError
```

`intl/test/lit/shapes-table.js`:
```js
/**
 * (Meta MIT header)
 */

// RUN: %hermes %s | %FileCheck --match-full-lines %s

// Every function the plugin installs, checked against the spec's
// function-shape table. Each phase adds its rows here, and a new kind of row
// gets its check below.

var TABLE = [
  // [holder, property, kind, name, length]
  ['Intl', 'getCanonicalLocales', 'method', 'getCanonicalLocales', 1],
  ['Intl', 'supportedValuesOf', 'method', 'supportedValuesOf', 1],
];

function isConstructor(f) {
  try {
    Reflect.construct(String, [], f);
    return true;
  } catch (e) {
    return false;
  }
}

/// A list of what is wrong with property d of a function, given the
/// expected value.
function checkFunctionProperty(f, key, expected, problems) {
  var d = Object.getOwnPropertyDescriptor(f, key);
  if (!d || d.value !== expected || d.writable || d.enumerable || !d.configurable)
    problems.push(key);
}

function check(row) {
  var holder = (0, eval)(row[0]);
  var d = Object.getOwnPropertyDescriptor(holder, row[1]);
  var problems = [];
  if (row[2] === 'method') {
    if (!d || !d.writable || d.enumerable || !d.configurable) problems.push('holder attributes');
    var f = d.value;
    if (isConstructor(f)) problems.push('constructible');
    if (Object.prototype.hasOwnProperty.call(f, 'prototype')) problems.push('prototype');
  } else {
    problems.push('unknown kind ' + row[2]);
  }
  checkFunctionProperty(f, 'name', row[3], problems);
  checkFunctionProperty(f, 'length', row[4], problems);
  if (Function.prototype.toString.call(f).indexOf('[native code]') === -1)
    problems.push('toString');
  return row[0] + '.' + row[1] + ': ' + (problems.length ? problems.join(', ') : 'ok');
}

for (var i = 0; i < TABLE.length; i++) print(check(TABLE[i]));
// CHECK: Intl.getCanonicalLocales: ok
// CHECK-NEXT: Intl.supportedValuesOf: ok

// Intl has exactly the table's properties and @@toStringTag.
print(Reflect.ownKeys(Intl).map(String).join(','));
// CHECK-NEXT: getCanonicalLocales,supportedValuesOf,Symbol(Symbol.toStringTag)
```

`intl/test/lit/tamper.js`:
```js
/**
 * (Meta MIT header)
 */

// RUN: %hermes %s | %FileCheck --match-full-lines %s

// User code that replaces intrinsics before calling Intl does not change what
// Intl returns.

function results() {
  return JSON.stringify([
    Intl.getCanonicalLocales(['EN-us', 'de', 'EN-us']),
    Intl.supportedValuesOf('calendar').length,
    Intl.supportedValuesOf('unit').slice(0, 2),
  ]);
}
var before = results();

// Read every captured value used below first, then tamper.
var define = Object.defineProperty;
var boom = function () {
  throw new Error('tampered intrinsic was used');
};
Array.prototype[Symbol.iterator] = boom;
Array.prototype.push = boom;
Array.prototype.indexOf = boom;
String.prototype[Symbol.split] = boom;
String.prototype.slice = boom;
String.prototype.indexOf = boom;
Function.prototype.call = boom;
Function.prototype.apply = boom;
Object.defineProperty = boom;
Object.getOwnPropertyDescriptor = boom;
define(Array.prototype, '0', {__proto__: null, set: boom, get: boom, configurable: true});
define(Array.prototype, '1', {__proto__: null, set: boom, get: boom, configurable: true});
// Last, because it breaks ordinary descriptor objects: accessors for the
// descriptor fields on Object.prototype.
var fields = ['value', 'writable', 'enumerable', 'configurable', 'get', 'set'];
for (var i = 0; i < fields.length; i++)
  define(Object.prototype, fields[i], {__proto__: null, get: boom, configurable: true});

var after = results();
print(after === before ? 'same' : 'different: ' + before + ' vs ' + after);
// CHECK: same
```
`results()` calls `.slice` on the array that `supportedValuesOf` returns. After the tampering, `Array.prototype.slice` still works, because it is not replaced, but `Array.prototype['0']` has a getter. Indexing a returned array still reads its own elements, since the plugin defines them as own properties. If `slice` itself fails in Hermes because of the `'0'` accessor on the prototype, replace it with a loop that reads indices 0 and 1. Do not remove the accessors.

- [ ] **Step 2: Run them and watch them fail**

`cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-intl-lit`
Expected:
- The five new tests fail with `TypeError: undefined is not a function` (`Intl.getCanonicalLocales` does not exist).
- `shapes-table.js` fails on the key list.
- The older tests still pass.

- [ ] **Step 3: Implement**

Replace `intl/js/10-Intl.js` with:
```js
/**
 * (Meta MIT header)
 */

// ECMA-402 8: The Intl Object, against the ECMA-402 draft of <date>. Created
// when the bytecode runs, so its [[Prototype]] is the intrinsic
// %Object.prototype%.
var IntlObject = {};

/// The sanctioned single unit identifiers (ECMA-402 table "Single units
/// sanctioned for use in ECMAScript"), sorted by code unit.
var SANCTIONED_UNITS = [
  'acre', 'bit', 'byte', 'celsius', 'centimeter', 'day', 'degree',
  'fahrenheit', 'fluid-ounce', 'foot', 'gallon', 'gigabit', 'gigabyte',
  'gram', 'hectare', 'hour', 'inch', 'kilobit', 'kilobyte', 'kilogram',
  'kilometer', 'liter', 'megabit', 'megabyte', 'meter', 'microsecond',
  'mile', 'mile-scandinavian', 'milliliter', 'millimeter', 'millisecond',
  'minute', 'month', 'nanosecond', 'ounce', 'percent', 'petabyte', 'pound',
  'second', 'stone', 'terabit', 'terabyte', 'week', 'yard', 'year',
];

/// Split a space-separated list from native code into a List.
function splitSpaces(s) {
  var list = [];
  var start = 0;
  while (start < s.length) {
    var end = stringIndexOf(s, ' ', start);
    if (end === -1) end = s.length;
    if (end > start) listPush(list, stringSlice(s, start, end));
    start = end + 1;
  }
  return list;
}

/// Install Intl on the global object; called once by setup.
function installIntl() {
  defineMethods(IntlObject, {
    // 8.3.1 Intl.getCanonicalLocales ( locales )
    getCanonicalLocales(locales) {
      // 1. Let ll be ? CanonicalizeLocaleList(locales).
      var ll = CanonicalizeLocaleList(locales);
      // 2. Return CreateArrayFromList(ll).
      return CreateArrayFromList(ll);
    },
    // 8.3.2 Intl.supportedValuesOf ( key )
    supportedValuesOf(key) {
      // 1. Let key be ? ToString(key).
      key = ToString(key);
      var list;
      if (key === 'calendar') {
        // 2. If key is "calendar", then
        //    a. Let list be a new empty List.
        list = [];
        //    b. For each element identifier of AvailableCalendars(), do
        var calendars = splitSpaces(native.supportedValues(key));
        for (var i = 0; i < calendars.length; i++) {
          //  i. Let canonical be CanonicalizeUValue("ca", identifier).
          //  ii. If identifier is canonical, append identifier to list.
          if (CanonicalizeUValue('ca', calendars[i]) === calendars[i])
            listPush(list, calendars[i]);
        }
      } else if (
        key === 'collation' ||
        key === 'currency' ||
        key === 'numberingSystem' ||
        key === 'timeZone'
      ) {
        // 3-6. AvailableCanonicalCollations, AvailableCanonicalCurrencies,
        //      AvailableCanonicalNumberingSystems,
        //      AvailablePrimaryTimeZoneIdentifiers.
        list = splitSpaces(native.supportedValues(key));
      } else if (key === 'unit') {
        // 7. AvailableCanonicalUnits.
        list = SANCTIONED_UNITS;
      } else {
        // 8. Else, throw a RangeError exception.
        throw new _RangeError('Invalid key : ' + key);
      }
      // 9. Return CreateArrayFromList(list).
      return CreateArrayFromList(list);
    },
  });
  // 8.1.1 Intl[@@toStringTag]
  setToStringTag(IntlObject, 'Intl');
  // ECMA-262 19.1: the global object's value properties are writable,
  // non-enumerable, configurable.
  ObjectDefineProperty(global, 'Intl', {
    __proto__: null,
    value: IntlObject,
    writable: true,
    enumerable: false,
    configurable: true,
  });
}
```
Check the unit table against the draft you follow, and use the draft's list.

- [ ] **Step 4: Run the tests**

`cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-intl`
Expected: everything passes.

- [ ] **Step 5: Show the tests can fail**

Make each change below, run, see the named test fail, then revert it:
- Replace `defineMethods(IntlObject, {...})` with direct assignments (`IntlObject.getCanonicalLocales = function getCanonicalLocales(locales) {...}`): `shapes-table.js` fails with `holder attributes, constructible, prototype`.
- In `listPush`, use `list[list.length] = v`: `tamper.js` fails.
- In `splitSpaces`, use `s.split(' ')`: `tamper.js` fails.

- [ ] **Step 6: Stage**

`git -C /home/tmikov/work/hermes-intl add intl`

### Task 11: test262 plugin mode and `check-intl-test262`

**Files:**
- Create: `utils/testsuite/intl_plugin.py`, `utils/testsuite/test_intl_plugin.py`
- Modify: `utils/testsuite/utils.py` (`TestCaseResult`), `utils/testsuite/hermes.py` (`run`), `utils/testsuite/test_run_defs.py` (`TestRunArgs`, `Test262Suite.run_test`), `utils/testsuite/cli.py` (flags, `run`, `main`)
- Create: `intl/test/test262-expectations.txt` (generated, then explained by hand)
- Modify: `intl/CMakeLists.txt` (`HERMES_INTL_TEST262_DIR`, `check-intl-test262`, `check-intl-runner`)

**Interfaces:**
- Consumes: the `hermes` CLI flags from Task 3, and the plugin.
- Produces:
  - `test_runner.py` flags:
    - `--intl-plugin <library>`;
    - `--intl-expectations <file>`;
    - `--intl-update-expectations`;
    - `--intl-allow-commit-mismatch`.
  - The expectations file format:
    ```
    # comments
    test262-commit: <sha>
    <path relative to the test262 checkout> <both|strict|non-strict> <reason>
    ```
  - The CMake cache variable `HERMES_INTL_TEST262_DIR`.

**Plugin mode, as the spec's Testing section requires:**
- **Skips:**
  - The skiplist's entries under `test/intl402/` (every category), its `intl_tests` gate, and the "No support for multiple Intl constructors" (`testIntl.js`) skip are ignored.
  - Features whose name starts with `intl` (case-insensitive) are not skipped.
  - Everything else (for example `Temporal`) is skipped as usual.
- **How tests run:**
  - Every run gets `-Xintl-plugin-path=<library> -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC`, so results do not depend on the machine.
  - Each strictness mode runs separately and is judged separately.
- **Classifying outcomes:**
  - "js-fail": the process exited normally, with no sanitizer report and no `LLVM ERROR`, and either threw an uncaught exception (exit 1 with `Uncaught ` on stderr) or failed a test262 check while exiting 0 (an async failure, or a negative test that did not throw).
  - "hard-fail": anything else that is not a pass, including crashes, timeouts and compile failures. A hard-fail never matches an expectation.
- **Judging the run:**
  - An expected test that passes fails the run.
  - An expectation whose file does not exist fails the run.
  - An expectation whose reason is `UNEXPLAINED` fails the run.
- **Before running:**
  - A probe checks that the plugin loads.
  - The checkout's commit must equal the file's `test262-commit` unless `--intl-allow-commit-mismatch` is given.

- [ ] **Step 1: Write the failing unit tests for the judgment logic**

`utils/testsuite/test_intl_plugin.py`:
```python
# (Meta MIT header as # comments)

"""Unit tests for testsuite/intl_plugin.py. Run with
PYTHONPATH=<repo>/utils python3 -m unittest testsuite.test_intl_plugin"""

import os
import tempfile
import unittest

from testsuite.intl_plugin import (
    Expectations,
    HARD_FAIL,
    JS_FAIL,
    PASS,
    classify,
    compile_is_hard_failure,
    derive_reason,
    evaluate,
    parse_expectations,
    render_expectations,
)


class Result:
    """The parts of TestCaseResult evaluate() reads."""

    def __init__(self, path, modes):
        self.test_name = "test262 :: " + path
        self.modes = modes


class IntlPluginTest(unittest.TestCase):
    def setUp(self):
        self.root = tempfile.mkdtemp()
        for p in ["test/intl402/a.js", "test/intl402/b.js", "test/intl402/c.js"]:
            os.makedirs(os.path.dirname(os.path.join(self.root, p)), exist_ok=True)
            open(os.path.join(self.root, p), "w").close()

    def expectations(self, text):
        return parse_expectations("test262-commit: abc\n" + text)

    def test_parse_expands_both(self):
        e = self.expectations("test/intl402/a.js both Phase 2: Intl.Collator\n")
        self.assertEqual(e.commit, "abc")
        self.assertEqual(
            e.entries["test/intl402/a.js"],
            {"strict": "Phase 2: Intl.Collator", "non-strict": "Phase 2: Intl.Collator"},
        )

    def test_expected_js_failure_is_ok(self):
        e = self.expectations("test/intl402/a.js strict why\n")
        problems = evaluate([Result("test/intl402/a.js", {"strict": JS_FAIL})], e, self.root)
        self.assertEqual(problems, [])

    def test_hard_failure_never_matches(self):
        e = self.expectations("test/intl402/a.js strict why\n")
        problems = evaluate([Result("test/intl402/a.js", {"strict": HARD_FAIL})], e, self.root)
        self.assertEqual(len(problems), 1)
        self.assertIn("crash", problems[0])

    def test_unexpected_pass_fails(self):
        e = self.expectations("test/intl402/a.js both why\n")
        problems = evaluate(
            [Result("test/intl402/a.js", {"strict": PASS, "non-strict": JS_FAIL})], e, self.root
        )
        self.assertEqual(len(problems), 1)
        self.assertIn("unexpected pass", problems[0])

    def test_unexpected_failure_fails(self):
        e = self.expectations("")
        problems = evaluate([Result("test/intl402/b.js", {"non-strict": JS_FAIL})], e, self.root)
        self.assertEqual(len(problems), 1)
        self.assertIn("unexpected failure", problems[0])

    def test_stale_and_unexplained_entries_fail(self):
        e = self.expectations(
            "test/intl402/gone.js both why\ntest/intl402/c.js both UNEXPLAINED\n"
        )
        problems = evaluate([], e, self.root)
        self.assertEqual(len(problems), 2)

    def test_render_round_trips_and_merges_modes(self):
        e = Expectations(
            "abc",
            {
                "test/intl402/b.js": {"strict": "r1", "non-strict": "r1"},
                "test/intl402/a.js": {"strict": "r2"},
            },
        )
        text = render_expectations(e)
        self.assertIn("test/intl402/b.js both r1", text)
        self.assertLess(text.index("a.js"), text.index("b.js"))
        self.assertEqual(parse_expectations(text).entries, e.entries)

    def test_classify(self):
        ok, uncaught = "", "Uncaught Test262Error: boom\n"
        asan = "==1==ERROR: AddressSanitizer: heap-use-after-free\n"
        self.assertEqual(classify(0, "", ok, False, False), PASS)
        self.assertEqual(classify(1, "", uncaught, False, False), JS_FAIL)
        self.assertEqual(classify(1, "", uncaught, True, False), PASS)
        self.assertEqual(classify(0, "", ok, True, False), JS_FAIL)
        # A sanitizer report exits 1 too; it never passes, even as an
        # expected runtime error, and is never a JS failure.
        self.assertEqual(classify(1, "", asan, True, False), HARD_FAIL)
        self.assertEqual(classify(1, "", asan + uncaught, False, False), HARD_FAIL)
        self.assertEqual(classify(0, "", asan, False, False), HARD_FAIL)
        self.assertEqual(classify(-6, "", "LLVM ERROR: x\n", True, False), HARD_FAIL)
        self.assertEqual(classify(1, "", "some other error\n", True, False), HARD_FAIL)
        self.assertEqual(classify(2, "", ok, False, False), HARD_FAIL)
        self.assertEqual(classify(0, "Test262:AsyncTestComplete\n", ok, False, True), PASS)
        self.assertEqual(classify(0, "Test262:AsyncTestFailure:x\n", ok, False, True), JS_FAIL)
        self.assertEqual(classify(0, "", ok, False, True), JS_FAIL)

    def test_compile_outcome(self):
        asan = "==1==ERROR: AddressSanitizer: stack-overflow\n"
        self.assertTrue(compile_is_hard_failure(1, asan))
        self.assertTrue(compile_is_hard_failure(0, asan))
        self.assertTrue(compile_is_hard_failure(-11, ""))
        self.assertFalse(compile_is_hard_failure(1, "error: unexpected token\n"))
        self.assertFalse(compile_is_hard_failure(0, ""))

    def test_probe_rejects_hangs_and_diagnostics(self):
        import stat

        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        d = tmp.name
        hang = os.path.join(d, "hang")
        with open(hang, "w") as f:
            # exec, so the timeout kills sleep itself, not just the shell.
            f.write("#!/bin/sh\nexec sleep 1000\n")
        noisy = os.path.join(d, "noisy")
        with open(noisy, "w") as f:
            f.write("#!/bin/sh\necho function\necho '==1==ERROR: AddressSanitizer: x' >&2\n")
        good = os.path.join(d, "good")
        with open(good, "w") as f:
            f.write("#!/bin/sh\necho function\n")
        for x in (hang, noisy, good):
            os.chmod(x, stat.S_IRWXU)
        import testsuite.intl_plugin as ip

        saved = ip.PROBE_TIMEOUT_SECONDS
        ip.PROBE_TIMEOUT_SECONDS = 1
        try:
            self.assertIn("timed out", ip.probe(hang, "p.so", d))
        finally:
            ip.PROBE_TIMEOUT_SECONDS = saved
        self.assertIsNotNone(ip.probe(noisy, "p.so", d))
        self.assertIsNone(ip.probe(good, "p.so", d))

    def test_derive_reason(self):
        self.assertEqual(
            derive_reason("test/intl402/Collator/x.js"), "Phase 2: Intl.Collator"
        )
        self.assertEqual(derive_reason("test/intl402/Intl/x.js"), "UNEXPLAINED")


if __name__ == "__main__":
    unittest.main()
```
Run: `PYTHONPATH=/home/tmikov/work/hermes-intl/utils python3 -m unittest testsuite.test_intl_plugin`
Expected: an `ImportError` for `testsuite.intl_plugin`.

- [ ] **Step 2: Write `intl_plugin.py`**

`utils/testsuite/intl_plugin.py`:
```python
# (Meta MIT header as # comments)

"""test262 plugin mode for the Intl plugin: expectations, per-mode results,
and the run's verdict. See "Testing" in
doc/superpowers/specs/2026-10-22-intl-plugin-design.md."""

import os
import subprocess
from dataclasses import dataclass, field
from typing import Dict, List, Optional

PASS = "pass"
JS_FAIL = "js-fail"
HARD_FAIL = "hard-fail"

MODES = ("strict", "non-strict")

# Reasons for new expected failures, by path prefix. Anything else is
# UNEXPLAINED and must be explained by hand before the run passes.
PHASE_REASONS = [
    ("test/intl402/Locale/", "Phase 1: Intl.Locale"),
    ("test/intl402/Collator/", "Phase 2: Intl.Collator"),
    ("test/intl402/String/", "Phase 2: locale-sensitive String methods"),
    ("test/built-ins/String/prototype/localeCompare/", "Phase 2: String.prototype.localeCompare"),
    ("test/built-ins/String/prototype/toLocale", "Phase 2: locale-sensitive case mapping"),
    ("test/intl402/NumberFormat/", "Phase 3: Intl.NumberFormat"),
    ("test/intl402/PluralRules/", "Phase 3: Intl.PluralRules"),
    ("test/intl402/Number/", "Phase 3: Number.prototype.toLocaleString"),
    ("test/intl402/BigInt/", "Phase 3: BigInt.prototype.toLocaleString"),
    ("test/intl402/Array/", "Phase 3: Array.prototype.toLocaleString"),
    ("test/intl402/TypedArray/", "Phase 3: TypedArray.prototype.toLocaleString"),
    ("test/built-ins/Number/prototype/toLocaleString/", "Phase 3: Number.prototype.toLocaleString"),
    ("test/built-ins/BigInt/prototype/toLocaleString/", "Phase 3: BigInt.prototype.toLocaleString"),
    ("test/built-ins/Array/prototype/toLocaleString/", "Phase 3: Array.prototype.toLocaleString"),
    ("test/built-ins/TypedArray/prototype/toLocaleString/", "Phase 3: TypedArray.prototype.toLocaleString"),
    ("test/intl402/DateTimeFormat/", "Phase 4: Intl.DateTimeFormat"),
    ("test/intl402/Date/", "Phase 4: Date.prototype.toLocale*String"),
    ("test/built-ins/Date/prototype/toLocale", "Phase 4: Date.prototype.toLocale*String"),
    ("test/intl402/ListFormat/", "Phase 5: Intl.ListFormat"),
    ("test/intl402/RelativeTimeFormat/", "Phase 5: Intl.RelativeTimeFormat"),
    ("test/intl402/DisplayNames/", "Phase 5: Intl.DisplayNames"),
    ("test/intl402/DurationFormat/", "Phase 5: Intl.DurationFormat"),
    ("test/intl402/Segmenter/", "Phase 5: Intl.Segmenter"),
]

UNEXPLAINED = "UNEXPLAINED"


@dataclass
class Expectations:
    """An expectations file: the test262 commit, and path -> mode -> reason."""

    commit: str
    entries: Dict[str, Dict[str, str]] = field(default_factory=dict)


def parse_expectations(text: str) -> Expectations:
    commit = ""
    entries: Dict[str, Dict[str, str]] = {}
    for number, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("test262-commit:"):
            commit = line.split(":", 1)[1].strip()
            continue
        parts = line.split(None, 2)
        if len(parts) != 3 or parts[1] not in ("both",) + MODES:
            raise ValueError(f"expectations line {number}: cannot parse {raw!r}")
        path, mode, reason = parts
        modes = MODES if mode == "both" else (mode,)
        for m in modes:
            entries.setdefault(path, {})[m] = reason
    if not commit:
        raise ValueError("expectations: missing 'test262-commit:' line")
    return Expectations(commit, entries)


HEADER = """\
# Expected test262 failures for the Hermes Intl plugin (check-intl).
# Format: <path> <both|strict|non-strict> <reason>
# Each line is a test that must fail, in that mode, with a JS exception or a
# test262 assertion. A crash, timeout or loader error never matches. A listed
# test that passes fails the run, so this list only shrinks. Regenerate with
# test_runner.py --intl-update-expectations; reasons are kept, and new
# entries outside the known phases are UNEXPLAINED until explained by hand.
"""


def render_expectations(e: Expectations) -> str:
    lines = [HEADER, f"test262-commit: {e.commit}"]
    for path in sorted(e.entries):
        modes = e.entries[path]
        if set(modes) == set(MODES) and modes["strict"] == modes["non-strict"]:
            lines.append(f"{path} both {modes['strict']}")
        else:
            for m in MODES:
                if m in modes:
                    lines.append(f"{path} {m} {modes[m]}")
    return "\n".join(lines) + "\n"


def derive_reason(path: str) -> str:
    for prefix, reason in PHASE_REASONS:
        if path.startswith(prefix):
            return reason
    return UNEXPLAINED


def result_path(result) -> str:
    """The test's path relative to the test262 checkout."""
    return result.test_name.split(" :: ", 1)[1]


def evaluate(results, e: Expectations, test262_root: str) -> List[str]:
    """Every problem with this run against e; empty means it passed."""
    problems = []
    for result in results:
        modes = getattr(result, "modes", None)
        if not modes:
            continue
        path = result_path(result)
        expected = e.entries.get(path, {})
        for mode, outcome in sorted(modes.items()):
            reason = expected.get(mode)
            if outcome == PASS and reason is not None:
                problems.append(f"unexpected pass: {path} [{mode}] (listed: {reason})")
            elif outcome == JS_FAIL and reason is None:
                problems.append(f"unexpected failure: {path} [{mode}]")
            elif outcome == HARD_FAIL:
                problems.append(f"crash, timeout or loader error: {path} [{mode}]")
    for path, modes in sorted(e.entries.items()):
        if not os.path.exists(os.path.join(test262_root, path)):
            problems.append(f"expectation for a missing test: {path}")
        if UNEXPLAINED in modes.values():
            problems.append(f"expectation without a reason: {path}")
    return problems


def update(results, old: Expectations, test262_root: str, commit: str) -> Optional[Expectations]:
    """New expectations from this run; None if the run had hard failures,
    which can never be expected."""
    hard = []
    entries: Dict[str, Dict[str, str]] = {}
    run_paths = set()
    for result in results:
        modes = getattr(result, "modes", None)
        if not modes:
            continue
        path = result_path(result)
        run_paths.add(path)
        for mode, outcome in modes.items():
            if outcome == HARD_FAIL:
                hard.append(f"{path} [{mode}]")
            elif outcome == JS_FAIL:
                reason = old.entries.get(path, {}).get(mode) or derive_reason(path)
                entries.setdefault(path, {})[mode] = reason
    if hard:
        print("Not updating expectations; these crashed, timed out or failed to load:")
        for h in sorted(hard):
            print("  " + h)
        return None
    # Keep entries for tests this run did not cover, if they still exist.
    for path, modes in old.entries.items():
        if path not in run_paths and os.path.exists(os.path.join(test262_root, path)):
            entries[path] = dict(modes)
    return Expectations(commit, entries)


def checkout_commit(test262_root: str) -> str:
    out = subprocess.run(
        ["git", "-C", test262_root, "rev-parse", "HEAD"],
        capture_output=True,
        text=True,
    )
    return out.stdout.strip() if out.returncode == 0 else ""


PROBE_TIMEOUT_SECONDS = 60


def has_fatal_diagnostics(stderr: str) -> bool:
    """Whether stderr shows a sanitizer report or a Hermes fatal error."""
    return "Sanitizer" in stderr or "LLVM ERROR" in stderr


def probe(hermes: str, plugin: str, work_dir: str) -> Optional[str]:
    """None if the plugin loads into hermes; else what went wrong."""
    path = os.path.join(work_dir, "intl-plugin-probe.js")
    with open(path, "w") as f:
        f.write("print(typeof Intl === 'object' && typeof Intl.getCanonicalLocales);\n")
    try:
        out = subprocess.run(
            [hermes, f"-Xintl-plugin-path={plugin}", path],
            capture_output=True,
            text=True,
            timeout=PROBE_TIMEOUT_SECONDS,
        )
    except subprocess.TimeoutExpired:
        return f"timed out after {PROBE_TIMEOUT_SECONDS} seconds"
    if (
        out.returncode == 0
        and out.stdout.strip() == "function"
        and not has_fatal_diagnostics(out.stderr)
    ):
        return None
    return f"exit {out.returncode}\nstdout: {out.stdout}\nstderr: {out.stderr}"


def vm_args(plugin: str) -> List[str]:
    """The arguments plugin mode adds to every hermes run."""
    return [
        f"-Xintl-plugin-path={plugin}",
        "-Xintl-default-locale=en-US",
        "-Xintl-default-time-zone=UTC",
    ]


def compile_is_hard_failure(returncode: int, stderr: str) -> bool:
    """Plugin mode: whether a compile step crashed rather than reporting a
    compile error. Hermes reports compile errors with exit codes 1-8."""
    return has_fatal_diagnostics(stderr) or returncode < 0 or returncode > 8


def classify(
    returncode: int, stdout: str, stderr: str, expect_runtime_error: bool, is_async: bool
) -> str:
    """PASS, JS_FAIL or HARD_FAIL for one finished hermes run in plugin mode.

    Only a process that exited normally (0, or 1 for an uncaught exception)
    with no sanitizer report and no fatal error can pass or fail in JS;
    anything else is HARD_FAIL, whatever the test expected."""
    if has_fatal_diagnostics(stderr) or returncode not in (0, 1):
        return HARD_FAIL
    uncaught = "Uncaught " in stderr
    if returncode == 1 and not uncaught:
        return HARD_FAIL
    threw = returncode == 1
    if is_async and not threw:
        if "Test262:AsyncTestFailure" in stdout or "Test262:AsyncTestComplete" not in stdout:
            return JS_FAIL
    if threw != expect_runtime_error:
        return JS_FAIL
    return PASS
```

- [ ] **Step 3: Run the unit tests**

`PYTHONPATH=/home/tmikov/work/hermes-intl/utils python3 -m unittest testsuite.test_intl_plugin -v`
Expected: 11 tests pass.

- [ ] **Step 4: Wire plugin mode into the runner**

`utils/testsuite/utils.py`, in `TestCaseResult`, after `duration`:
```python
    # Whether a failure happened in JS (an uncaught exception or a test262
    # assertion) rather than a crash, timeout or compile error.
    js_failure: bool = False
    # Plugin mode: strictness mode ("strict" / "non-strict") -> outcome.
    modes: Optional[Dict[str, str]] = None
```
Add `Dict` and `Optional` to its `typing` import.

`utils/testsuite/hermes.py`:
- Add the field `intl_classify: bool = False` at the end of `CompileRunArgs`, documented as "Plugin mode: judge the run with intl_plugin.classify".
- In `run()`, import `from .intl_plugin import classify, PASS, JS_FAIL`. Right after the timeout handling and the computation of `output` and `stdout_str`, before the existing async and exit-code checks, add:
  ```python
    if compile_run_args.intl_classify:
        stderr_str = stderr.decode("utf-8") if stderr else ""
        outcome = classify(
            proc.returncode,
            stdout_str,
            stderr_str,
            expected_failure_phase == "runtime",
            compile_run_args.is_async,
        )
        return TestCaseResult(
            compile_run_args.test_name,
            TestResultCode.TEST_PASSED if outcome == PASS else TestResultCode.EXECUTE_FAILED,
            f"{outcome}: {base_file_name}",
            output,
            duration,
            js_failure=outcome == JS_FAIL,
        )
  ```
  The existing logic, which accepts any positive exit code for an expected runtime error, stays as it is for normal mode.
- `compile_with_args` gets a keyword parameter `intl_classify: bool = False`, and `compile_and_run_single` passes `intl_classify=compile_run_args.intl_classify`. In `compile_with_args`, right after `output` is built and before `if proc.returncode:`, add:
  ```python
    if intl_classify:
        from .intl_plugin import compile_is_hard_failure

        stderr_str = stderr.decode("utf-8") if stderr else ""
        if compile_is_hard_failure(proc.returncode, stderr_str):
            msg = f"FAIL: compiler crashed or reported a sanitizer error: {args}"
            return TestCaseResult(
                test_name, TestResultCode.COMPILE_FAILED, msg, output, duration
            )
  ```
  So in plugin mode an ASan report during an expected parse failure, or during a successful compile, is a hard failure, while an ordinary compile error (exit 1-8) still counts as the expected parse failure.

`utils/testsuite/test_run_defs.py`:
- Add the field `intl_plugin: Optional[str] = None` at the end of `TestRunArgs`. Import `Optional` from `typing`, and `copy.deepcopy` is already imported.
- Import `from .intl_plugin import HARD_FAIL, JS_FAIL, PASS, vm_args as intl_vm_args` and `from .hermes import compile_and_run_single`.
- In `Test262Suite.run_test`:
  - change the `testIntl.js` check to `if "testIntl.js" in test_case.includes and not args.intl_plugin:`;
  - at the top of the feature loop add:
    ```python
            # Plugin mode runs every Intl feature.
            if args.intl_plugin and f.lower().startswith("intl"):
                continue
    ```
  - build `js_sources` as a list of `(mode, path)` pairs, `("strict", ...)` and `("non-strict", ...)`;
  - and replace the final `return await compile_and_run(js_sources, compile_run_args)` with:
    ```python
            if not args.intl_plugin:
                return await compile_and_run([p for _, p in js_sources], compile_run_args)
            # Plugin mode: run each strictness mode on its own and judge it alone.
            compile_run_args.extra_compile_vm_args = deepcopy(args.extra_compile_vm_args)
            compile_run_args.extra_compile_vm_args.vm_args += intl_vm_args(args.intl_plugin)
            modes = {}
            output = ""
            duration = 0.0
            for mode, path in js_sources:
                # Each variant gets exactly its own strictness: the shared
                # strict_mode is usually ALL, which would compile both
                # variants with -strict.
                mode_args = dataclasses.replace(
                    compile_run_args,
                    strict_mode=StrictMode.STRICT if mode == "strict" else StrictMode.NO_STRICT,
                    intl_classify=True,
                )
                r = await compile_and_run_single(path, mode_args)
                duration += r.duration
                if r.code == TestResultCode.TEST_PASSED:
                    modes[mode] = PASS
                elif r.js_failure:
                    modes[mode] = JS_FAIL
                else:
                    modes[mode] = HARD_FAIL
                output += f"[{mode}] {r.msg}\n{r.output}\n"
            ok = all(o == PASS for o in modes.values())
            return TestCaseResult(
                full_test_name,
                TestResultCode.TEST_PASSED if ok else TestResultCode.EXECUTE_FAILED,
                "PASS" if ok else "FAIL in plugin mode",
                output,
                duration,
                modes=modes,
            )
    ```
  - Add `import dataclasses` at the top of `test_run_defs.py`.
  - `compile_and_run_single` returns failures from `run()` unchanged, so `js_failure` reaches here. Compile failures leave it `False`, which makes them hard failures.

`utils/testsuite/cli.py`:
- **Flags,** after `--test-intl`:
  ```python
    parser.add_argument(
        "--intl-plugin",
        dest="intl_plugin",
        default=None,
        help="Intl plugin mode: load this plugin library in every run, run "
        "intl402, and judge results against --intl-expectations",
    )
    parser.add_argument(
        "--intl-expectations",
        dest="intl_expectations",
        default=None,
        help="Expected-failures file for --intl-plugin",
    )
    parser.add_argument(
        "--intl-update-expectations",
        dest="intl_update_expectations",
        action="store_true",
        help="Rewrite --intl-expectations from this run's results",
    )
    parser.add_argument(
        "--intl-allow-commit-mismatch",
        dest="intl_allow_commit_mismatch",
        action="store_true",
        help="Run even if the test262 checkout is not at the expectations' commit",
    )
  ```
- **`validate_args`:** if `args.intl_plugin` is set and `args.intl_expectations` is not, print `Argument error: --intl-plugin requires --intl-expectations.` and `sys.exit(1)`. Also reject `--intl-plugin` together with `--lazy`, `--shermes` or `--bytecode-compat-check`; plugin mode classifies runs only for the default compile-then-run path.
- **`run()`:**
  - It gains an `intl: Optional[dict] = None` parameter holding the four flag values.
  - When `intl` is set, the skip block becomes:
    ```python
        in_intl402 = "/test/intl402/" in test_file.replace(os.sep, "/")
        if not (intl and in_intl402):
            <the existing two skip checks, unchanged>
    ```
  - `TestRunArgs` gets `intl_plugin=intl["plugin"] if intl else None`.
  - After the results loop and `pd.finish()`, when `intl` is set, return `intl_verdict(all_results, intl, tests_paths)` instead of the normal failure accounting. Collect passing results into `all_results` as the code already does.
- **Helpers** (add to `cli.py`):
  ```python
  def test262_root(tests_paths) -> str:
      """The test262 checkout containing the first test path."""
      p = os.path.abspath(tests_paths[0])
      return p[: p.rfind("/test/") ] if "/test/" in p else p


  def intl_prepare(args, binary_directory, work_dir, tests_paths) -> Optional[dict]:
      """Check the plugin loads and the checkout's commit; None on failure."""
      from . import intl_plugin

      hermes = os.path.join(binary_directory, "hermes")
      if problem := intl_plugin.probe(hermes, args.intl_plugin, work_dir):
          print(f"Intl plugin probe failed:\n{problem}")
          return None
      with open(args.intl_expectations) as f:
          expectations = intl_plugin.parse_expectations(f.read())
      root = test262_root(tests_paths)
      commit = intl_plugin.checkout_commit(root)
      if (
          commit != expectations.commit
          and not args.intl_allow_commit_mismatch
          and not args.intl_update_expectations
      ):
          print(
              f"test262 at {root} is at {commit or 'an unknown commit'}, but "
              f"{args.intl_expectations} was made against {expectations.commit}. "
              "Check out that commit, or pass --intl-allow-commit-mismatch."
          )
          return None
      return {
          "plugin": args.intl_plugin,
          "expectations": expectations,
          "file": args.intl_expectations,
          "update": args.intl_update_expectations,
          "root": root,
          "commit": commit,
      }


  def intl_verdict(results, intl, tests_paths) -> int:
      from . import intl_plugin

      if intl["update"]:
          new = intl_plugin.update(results, intl["expectations"], intl["root"], intl["commit"])
          if new is None:
              return 1
          with open(intl["file"], "w") as f:
              f.write(intl_plugin.render_expectations(new))
          print(f"Wrote {intl['file']}")
          return 0
      problems = intl_plugin.evaluate(results, intl["expectations"], intl["root"])
      for p in problems:
          print(p)
      print(f"Intl plugin mode: {len(problems)} problem(s)")
      return 1 if problems else 0
  ```
- **`main()`:** after the work directory is set up, if `args.intl_plugin`, call `intl = intl_prepare(args, args.binary_directory, work_dir, args.paths)`, return `1` if it is `None`, and pass `intl=intl` to `run()`.

- [ ] **Step 4b: Check per-mode runs end to end**

The normal runner compiles every variant of a test with `-strict` whenever the test may run in strict mode (`utils/testsuite/hermes.py:337` with `StrictMode.ALL` from `preprocess.py:66`). So the "non-strict" run of a test without `onlyStrict`/`noStrict` is actually strict. Plugin mode fixes this for itself (Step 4). Check that the fix works through the real runner with a synthetic suite:
```
S=/tmp/claude-1001/-home-tmikov-work-hermes/a918a33f-abe5-47f3-80d3-351885f648de/scratchpad/modes/test262
T262=/home/tmikov/work/hermes-intl/cmake-build-asan/test262
mkdir -p $S/test/intl402 && cp -r $T262/harness $S/
cat > $S/test/intl402/mode-check.js <<'JS'
/*---
description: fails only in strict mode
---*/
if ((function () { return this; })() === undefined) throw new Test262Error('strict');
JS
printf 'test262-commit: none
test/intl402/mode-check.js strict fails in strict mode by design
' > $S/../expect.txt
python3 /home/tmikov/work/hermes-intl/utils/test_runner.py --intl-plugin /home/tmikov/work/hermes-intl/cmake-build-asan/lib/libhermesintl.so --intl-expectations $S/../expect.txt --intl-allow-commit-mismatch -b /home/tmikov/work/hermes-intl/cmake-build-asan/bin $S/test/intl402
```
Expected: `Intl plugin mode: 0 problem(s)`. Then change `strict` to `both` in `expect.txt` and rerun. Expected: one `unexpected pass: ... [non-strict]`. Before the `dataclasses.replace` fix (try it by passing `compile_run_args` unchanged), the first run reports `unexpected failure: ... [non-strict]`, which shows the check can fail. The test262 checkout comes from Step 6, so run this after Step 6. Delete the scratch directory afterwards.

File the normal-mode bug in `dz` (CLAUDE.md "Issue Tracking"): `test_runner.py runs both strictness variants of a test262 test in strict mode`. Name the two code locations above and say that plugin mode is not affected. If `dz` is not available, ask the user how to proceed; do not install it.

- [ ] **Step 5: Add `check-intl-test262` and `check-intl-runner`**

In `intl/CMakeLists.txt`, inside `if(HERMES_ENABLE_TEST_SUITE)` before `add_custom_target(check-intl)`:
```cmake
  set(HERMES_INTL_TEST262_DIR "" CACHE PATH
    "A tc39/test262 checkout for check-intl, at the commit named in intl/test/test262-expectations.txt")
  set(_t262 ${HERMES_INTL_TEST262_DIR}/test)
  if(HERMES_INTL_TEST262_DIR)
    add_custom_target(check-intl-test262
      COMMAND ${Python_EXECUTABLE} ${PROJECT_SOURCE_DIR}/utils/test_runner.py
        --intl-plugin $<TARGET_FILE:hermesintl>
        --intl-expectations ${CMAKE_CURRENT_SOURCE_DIR}/test/test262-expectations.txt
        -b ${HERMES_TOOLS_OUTPUT_DIR}
        ${_t262}/intl402
        ${_t262}/built-ins/String/prototype/localeCompare
        ${_t262}/built-ins/String/prototype/toLocaleLowerCase
        ${_t262}/built-ins/String/prototype/toLocaleUpperCase
        ${_t262}/built-ins/Number/prototype/toLocaleString
        ${_t262}/built-ins/BigInt/prototype/toLocaleString
        ${_t262}/built-ins/Array/prototype/toLocaleString
        ${_t262}/built-ins/TypedArray/prototype/toLocaleString
        ${_t262}/built-ins/Date/prototype/toLocaleString
        ${_t262}/built-ins/Date/prototype/toLocaleDateString
        ${_t262}/built-ins/Date/prototype/toLocaleTimeString
      DEPENDS hermesintl hermes hermesc
      COMMENT "Running test262 intl402 against the Intl plugin"
      USES_TERMINAL
      VERBATIM)
  else()
    add_custom_target(check-intl-test262
      COMMAND ${CMAKE_COMMAND} -E echo
        "check-intl needs test262: configure with -DHERMES_INTL_TEST262_DIR=<a tc39/test262 checkout at the commit in intl/test/test262-expectations.txt>"
      COMMAND ${CMAKE_COMMAND} -E false
      VERBATIM)
  endif()

  add_custom_target(check-intl-runner
    COMMAND ${CMAKE_COMMAND} -E env PYTHONPATH=${PROJECT_SOURCE_DIR}/utils
      ${Python_EXECUTABLE} -m unittest testsuite.test_intl_plugin
    COMMENT "Running the test262 plugin-mode unit tests"
    VERBATIM)
```
and change the `check-intl` dependencies to `check-intl-rust check-intl-lit check-intl-runner check-intl-test262`.

If any of the `built-ins` directories does not exist at the pinned commit, remove it from the list and say so in the report.

- [ ] **Step 6: Check out test262 and generate the expectations**

```
git clone https://github.com/tc39/test262 /home/tmikov/work/hermes-intl/cmake-build-asan/test262
git -C /home/tmikov/work/hermes-intl/cmake-build-asan/test262 checkout c9aa6126e8f6fe2c6f668c65bfba771ffc8a6096
cmake -S /home/tmikov/work/hermes-intl -B /home/tmikov/work/hermes-intl/cmake-build-asan -DHERMES_INTL_TEST262_DIR=/home/tmikov/work/hermes-intl/cmake-build-asan/test262
printf 'test262-commit: c9aa6126e8f6fe2c6f668c65bfba771ffc8a6096\n' > /home/tmikov/work/hermes-intl/intl/test/test262-expectations.txt
python3 /home/tmikov/work/hermes-intl/utils/test_runner.py --intl-plugin /home/tmikov/work/hermes-intl/cmake-build-asan/lib/libhermesintl.so --intl-expectations /home/tmikov/work/hermes-intl/intl/test/test262-expectations.txt --intl-update-expectations -b /home/tmikov/work/hermes-intl/cmake-build-asan/bin /home/tmikov/work/hermes-intl/cmake-build-asan/test262/test/intl402
```
(Use the same path list as `check-intl-test262`.)

Expected: `Wrote .../test262-expectations.txt`. If it reports hard failures instead, each one is a bug: in the plugin, the runner or Hermes. Fix it and rerun.

- [ ] **Step 7: Explain every `UNEXPLAINED` entry, and fix what Phase 0 should pass**

`grep -c UNEXPLAINED /home/tmikov/work/hermes-intl/intl/test/test262-expectations.txt` lists the entries outside the per-phase directories: `test/intl402/Intl/` and the files directly in `test/intl402/`. For each one, read the test and do one of the following:
- **The test only needs something Phase 0 delivers** (`getCanonicalLocales`, `supportedValuesOf`, `Intl` itself, `@@toStringTag`): it is a Phase 0 bug. Fix it and delete the entry.
- **The test uses a later constructor:** replace `UNEXPLAINED` with what it needs, for example `uses Intl.NumberFormat (Phase 3)`.
- **The test fails because of the runner or Hermes itself, not Intl:** say exactly that.

Then run `cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-intl-test262`.
Expected: `Intl plugin mode: 0 problem(s)`.

The report lists:
- the counts by reason;
- every Phase 0 test262 failure that was fixed;
- every remaining entry under `test/intl402/Intl/getCanonicalLocales/`, `test/intl402/Intl/supportedValuesOf/` and `test/intl402/language-tags-*`, with its reason. These are the ones Phase 0 is expected to pass, so each remaining one needs a reason a reviewer will accept.

- [ ] **Step 8: Show the checks can fail**

Make each change below, run `check-intl-test262`, see it fail with the named problem, then revert it:
- Delete one `Phase 2` line from the expectations file: `unexpected failure`.
- Add `test/intl402/Intl/getCanonicalLocales/has-property.js both bogus` (or any passing test): `unexpected pass`.
- Add `test/intl402/nonexistent.js both bogus`: `expectation for a missing test`.
- Make the plugin's `install` throw (a `throw jsi::JSINativeException("x");` at the top of `install` in `Plugin.cpp`) and rebuild: the probe fails before any test runs.
- Change `test262-commit:` to another value: the commit check refuses to run.

- [ ] **Step 9: Stage**

`git -C /home/tmikov/work/hermes-intl add utils/testsuite intl/CMakeLists.txt intl/test/test262-expectations.txt`

### Task 12: CI, docs, and the spec

**Files:**
- Modify: `.github/workflows/build.yml`, `.circleci/config.yml`
- Create: `intl/README.md`
- Modify: `doc/superpowers/specs/2026-10-22-intl-plugin-design.md`

- [ ] **Step 1: CI**

`HERMES_BUILD_INTL` is already `OFF` by default in the cross, Windows and universal-macOS jobs. That covers `test-linux-armv7`, whose explicit `CMAKE_SYSTEM_NAME` makes CMake treat it as a cross build, `test-windows-test262`, and `macos`.

The jobs that change:
- **`.github/workflows/build.yml`, `test-macos-test262`:** add `-DHERMES_BUILD_INTL=OFF` to its configure line, with the comment `# This job tests the old, built-in Intl (HERMES_ENABLE_INTL); the plugin would replace it.`
- **`.github/workflows/build.yml`, `test-linux-test262`:**
  - Add a step before "Run test262":
    ```yaml
    - name: Ensure Rust (the Intl plugin needs it)
      run: |-
        command -v cargo || {
          curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
          echo "$HOME/.cargo/bin" >> "$GITHUB_PATH"
        }
    ```
  - In "Run test262", configure with `-DHERMES_INTL_TEST262_DIR=$PWD/test262`, and after `cmake --build build --target check-hermes all` add `cmake --build build --target check-intl`. The job's test262 pin is already `c9aa6126...`, the expectations' commit.
  - Replace the comment "Intl is unavailable on Linux..." with `# intl402 runs through check-intl, against the Intl plugin.`
- **`.circleci/config.yml`:** add `-DHERMES_BUILD_INTL=OFF` to both configure lines.
  - On `test-macos`, comment: `# Tests the old, built-in Intl.`
  - On `test-linux`, comment: `# The image has no Rust toolchain.`

The workflows cannot be run from here. Say so in the report, and quote each changed line.

- [ ] **Step 2: `intl/README.md`**

Write a short README covering:
- what the plugin is, linking the spec;
- the directory layout;
- the build: `HERMES_BUILD_INTL`, the Rust ≥ 1.88 requirement, which platforms build in Phase 0, and that the output is `<build>/lib/libhermesintl.{so,dylib}`;
- how Hermes finds the plugin: registered, `IntlPluginPath`, next to the Hermes code, `../lib`, the search path; and the error table;
- the `RuntimeConfig` fields and CLI flags;
- how to test:
  - `check-intl` and `HERMES_INTL_TEST262_DIR` with the pinned commit;
  - regenerating the expectations with `--intl-update-expectations`, then explaining every `UNEXPLAINED` entry;
  - the JS isolation tests;
- the JS rules, pointing to the plan's "Plugin JS rules";
- how to add a native helper: one line in `NativeHelpers.def` and a function in a `*Helpers.cpp`;
- how to regenerate the CLDR lists.

- [ ] **Step 3: Update the spec**

Edit `doc/superpowers/specs/2026-10-22-intl-plugin-design.md`:
- **Status line:** `draft, revised during Phase 0 planning`.
- **"Binary contract with the host", first bullet:** replace it with decision 1 from this plan's "Planning decisions that refine the spec". On Linux and macOS the plugin leaves JSI undefined and binds to the host's exported copy, and every Hermes tool exports its dynamic symbols. Linking against a provider explicitly (iOS framework, `libjsi.so`, a Windows import library) comes in Phase 6.
- **"Build":** Phase 0 builds for the host only. `HERMES_BUILD_INTL` defaults to `OFF` for cross builds, Windows and multi-architecture macOS, and setting it `ON` there is an error until Phase 6. Phase 6 adds cross builds through cargo.
- **"The plugin's entry point":** add `flags` and `resolve_defaults`, and note that `hermes_intl_config` uses `std::vector<std::string>`/`std::string` (decision 3).
- **"Loader", default resolution step 2:** add `../lib` when the directory is `bin`, `/proc/self/exe` on Linux, and the not-found rule for the search path (decision 2). Add that the per-runtime record lives in JSI runtime data (decision 4). On Windows and Emscripten only `registerIntlPlugin` works until Phase 6.
- **"C++ JSI layer", last bullet:** internal errors are C++ exceptions, which Hermes turns into an `Error` from its own intrinsics (decision 5).
- **"Default locale and time zone", Android row:** replace "(confirmed in the Phase 0 plan)" with "(bionic `libc/tzcode/bionic.cpp`)".
- **"Testing", runner:** plugin mode passes `-Xintl-default-locale=en-US -Xintl-default-time-zone=UTC`. The test262 pin is the CI commit `c9aa6126e8f6fe2c6f668c65bfba771ffc8a6096`.
- **Phasing, Phase 6:** add cross builds via cargo, universal macOS, Windows (plugin build and loader), and explicit JSI provider linking.

- [ ] **Step 4: Stage**

`git -C /home/tmikov/work/hermes-intl add .github/workflows/build.yml .circleci/config.yml intl/README.md doc/superpowers/specs/2026-10-22-intl-plugin-design.md`

### Task 13: Verify the binary contract on macOS and in the Apple framework layout (requires a Mac)

The spec requires Phase 0 to check this, and it cannot be done on Linux. If no Mac is available to the executor, **do not mark this task done**. Report it as not run, with the steps below, and leave it for the user.

- [ ] **Step 1: The default macOS build**

On a Mac (Apple Silicon), with Rust ≥ 1.88:
```
git clone https://github.com/tc39/test262 <build>/test262
git -C <build>/test262 checkout c9aa6126e8f6fe2c6f668c65bfba771ffc8a6096
cmake -S <worktree> -B <build> -G Ninja -DCMAKE_BUILD_TYPE=Debug -DJSI_UNSTABLE=ON -DHERMES_INTL_TEST262_DIR=<build>/test262
cmake --build <build> --target check-intl check-hermes
```
Expected: both pass. The `default-resolution.js` lit test exercises `dladdr`, `../lib` and the dyld not-found rule. `IntlPluginUnitTests` covers the Apple-independent parts of `Defaults`.

- [ ] **Step 2: Apple detection by hand**

`<build>/bin/hermes -Xintl-plugin-path=<build>/lib/libhermesintl.dylib -e ...` cannot show the defaults in Phase 0. Instead, add a temporary `fprintf(stderr, ...)` of `config->default_locales[0]` and `config->default_time_zone` at the top of `install`. Run with System Settings' language and region set to two different values. Check that the output follows `CFLocaleCopyPreferredLanguages` and `CFTimeZoneCopySystem`. Remove the `fprintf`.

- [ ] **Step 3: The framework layout**

Build the macOS framework variant:
```
cmake -S <worktree> -B <build-fw> -G Ninja -DCMAKE_BUILD_TYPE=Release -DHERMES_BUILD_APPLE_FRAMEWORK=ON -DHERMES_APPLE_TARGET_PLATFORM=macosx -DCMAKE_OSX_ARCHITECTURES=arm64 -DHERMES_BUILD_INTL=OFF
cmake --build <build-fw> --target hermesvm
nm -gU <build-fw>/lib/hermesvm.framework/hermesvm | grep -c "facebook::jsi" ; nm -gUC ... | grep "typeinfo for facebook::jsi::NativeState"
```
Expected:
- The framework exports the JSI symbols, including `typeinfo for facebook::jsi::NativeState`.
- `HERMES_BUILD_INTL=OFF` here because `HERMES_APPLE_TARGET_PLATFORM` makes `ON` an error. Then link a minimal dylib with `-undefined dynamic_lookup` that throws a `jsi::JSError` from a host function, load it into a program linked against the framework, and check that the program catches the error as `jsi::JSError`.
- Record which library exports JSI, and whether linking the plugin against the framework itself (`-framework hermesvm`) works. That decides Phase 6's iOS linking. Write the answer into the spec's "Risks" section, replacing "unverified".

### Task 14: Final verification

- [ ] **Step 1: Clean build and every suite**

```
rm -rf /home/tmikov/work/hermes-intl/cmake-build-asan/intl
cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan
cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-hermes
cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-intl
```
Expected:
- `check-intl` passes with 0 problems.
- `check-hermes` has the same results as before Task 1, plus the new loader tests. Record the before and after counts in the report.

- [ ] **Step 2: A Release build**

```
cmake -S /home/tmikov/work/hermes-intl -B /home/tmikov/work/hermes-intl/cmake-build-release -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ -DJSI_UNSTABLE=ON -DHERMES_INTL_TEST262_DIR=/home/tmikov/work/hermes-intl/cmake-build-asan/test262
cmake --build /home/tmikov/work/hermes-intl/cmake-build-release --target check-intl
ls -l /home/tmikov/work/hermes-intl/cmake-build-release/lib/libhermesintl.so
```
Expected: `check-intl` passes (`cargo build --release` with `panic = "abort"`). Record the library size; the spec asks for it as each constructor lands.

- [ ] **Step 3: A build without the plugin**

```
env PATH=/usr/bin:/bin cmake -S /home/tmikov/work/hermes-intl -B /home/tmikov/work/hermes-intl/cmake-build-nointl -G Ninja -DCMAKE_BUILD_TYPE=Debug -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ -DHERMES_BUILD_INTL=OFF -DJSI_UNSTABLE=ON
env PATH=/usr/bin:/bin cmake --build /home/tmikov/work/hermes-intl/cmake-build-nointl --target check-hermes
```
Expected: it builds and passes without Rust on `PATH`; the loader tests with fake plugins still run. Then drop the plugin built in the ASan tree next to this build's `hermes`. This is the spec's "no rebuild needed" claim, but across build trees the version and flag checks decide the outcome. The plugin must load, because both trees share the source and `JSI_UNSTABLE=ON`:
```
mkdir -p /home/tmikov/work/hermes-intl/cmake-build-nointl/lib
cp /home/tmikov/work/hermes-intl/cmake-build-asan/lib/libhermesintl.so /home/tmikov/work/hermes-intl/cmake-build-nointl/lib/
echo 'print(Object.prototype.toString.call(Intl))' > /tmp/claude-1001/-home-tmikov-work-hermes/a918a33f-abe5-47f3-80d3-351885f648de/scratchpad/intl-probe.js
/home/tmikov/work/hermes-intl/cmake-build-nointl/bin/hermes /tmp/claude-1001/-home-tmikov-work-hermes/a918a33f-abe5-47f3-80d3-351885f648de/scratchpad/intl-probe.js
```
Expected: `[object Intl]`. The ASan-built plugin needs the ASan runtime, which the non-ASan host lacks. If loading fails for that reason, copy the plugin from `cmake-build-release` instead (also `JSI_UNSTABLE=ON`), and record which was used. Delete `cmake-build-nointl` and `cmake-build-release` afterwards.

- [ ] **Step 4: Separability check**

```
grep -rn '#include' /home/tmikov/work/hermes-intl/intl/jsi | grep -v -e '<jsi/' -e '<hermes/Public/IntlPlugin.h>' -e '"' -e '<[a-z_]*>' -e '<CoreFoundation' -e '<sys/' -e '<unistd.h>'
```
Expected: no output. The plugin includes nothing from Hermes but JSI and the public interface header.

- [ ] **Step 5: Report**

Summarize for the user:
- what passed, with counts;
- the library size;
- every place the implementation departed from this plan, and why;
- every `UNEXPLAINED` entry resolved, and how;
- what Task 13 found, or that it was not run.

Leave everything staged and uncommitted.
