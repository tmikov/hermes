# Intl as a separate ICU4X-based plugin

Date: 2026-10-22
Status: draft, revised during Phase 0 planning and implementation

## Context

Before this plugin, Hermes's `Intl` mapped ECMA-402 onto platform APIs:
Foundation on Apple (`lib/Platform/Intl/PlatformIntlApple.mm`) and Java
through JNI on Android (`lib/Platform/Intl/PlatformIntlAndroid.cpp`), with
the JS-facing layer in `lib/VM/JSLib/Intl.cpp`. It existed only on those two
platforms, was limited, and was impossible to maintain: the platform APIs do
not map onto ECMA-402, and every mismatch became a special case. On
test262.fyi (2026-10-19) Hermes passed 358 of 3382 `intl402` tests;
SpiderMonkey passed 3376 and V8 3343. Phase 7 deleted that implementation
(see "Without the plugin" and "Phasing").

The ICU-backed implementation was deleted in `5cf80e4e9`. GitHub discussion
#1211 records the history: ICU4C is too large to ship, platform APIs are
inadequate, and Meta will not fund the work. This design replaces all of it
with an implementation built on ICU4X, packaged as a separate library that
talks to the engine only through JSI.

### Prior art examined

- **magrinj/hermes-intl** (MIT, Oct 2026). A JSI TurboModule with a Rust
  ICU4X core and a JS spec layer. Covers `PluralRules`, `RelativeTimeFormat`,
  `ListFormat` and `Locale` only, and sits on top of the engine's own
  NumberFormat, DateTimeFormat and Collator. Not a base to build on, but these
  are worth copying (keeping its copyright notice): the locale
  canonicalization and lookup fixes in `rust/src/locale.rs`, the digit-option
  to `fixed_decimal` mapping in `rust/src/plural.rs`, the lazy per-unit
  formatters in `rust/src/relativetime.rs`, and the expected-failures test262
  runner pattern in `host/`.
- **SpiderMonkey.** Still mostly ICU4C. Only Collator, Segmenter and the
  Temporal calendars use ICU4X. Its spec layer is about 20K lines of C++
  written against SpiderMonkey internals, under MPL 2.0, which is per-file
  copyleft. We use it as a reference for spec interpretation only and copy
  nothing. Two patterns are borrowed: binding ICU4X through a small Rust crate
  with its own C interface (`collator_glue`, `locale_glue`) where the official
  bindings fall short, and its data trimming choices when size work begins.

## Goals

- A complete, spec-conforming ECMA-402 implementation: all `Intl`
  constructors, and the locale-sensitive built-ins of ECMA-262 that ECMA-402
  redefines.
- One implementation on every platform: Android, iOS, macOS, Linux, Windows.
- Shipped as a separate shared library. Hermes picks it up automatically when
  it is present; no Hermes rebuild or reconfiguration is needed to add or
  remove it.
- Test262 `intl402` as the acceptance suite.

## Non-goals

- Binary size, for now. Correctness and simplicity come first. Size work
  (locale presets, data trimming) starts only after the constructors are
  correct, and is driven by measurements taken as each one lands.
- Speed, for now, beyond avoiding obvious waste. In particular, strings are
  copied across the native boundary rather than read in place.
- Supporting engines other than Hermes.
- Locale or time zone changes during a runtime's lifetime. Both are fixed when
  the runtime is created.

## Guiding rules

1. **As much as possible in JS.** Native code does only what needs CLDR data or
   ICU4X algorithms. Option processing, locale-list handling, Unicode extension
   handling, `resolvedOptions`, part assembly and the shape of every object are
   JS.
2. **Correctness and simplicity before size and speed.**
3. **Separable.** Nothing in the plugin includes Hermes internals. It builds
   from its own directory against JSI and ICU4X, so moving it to its own
   repository later is a move, not a rewrite.

## Architecture

### Layout

A new top-level directory `intl/`:

- `intl/js/` — the ECMA-402 implementation in JS, compiled to Hermes bytecode
  at build time and embedded in the library, the same way
  `API/hermes/extensions/` works.
- `intl/core/` — a Rust crate on ICU4X exposing an `extern "C"` interface. Built
  as a static library with the ICU4X data compiled in.
- `intl/jsi/` — the C++ JSI layer: turns the C interface into JSI host
  functions, owns native objects through `jsi::NativeState`, loads the embedded
  bytecode, and exports the plugin's entry point.
- `intl/test/` — lit tests for Hermes-specific behavior, the gtests for
  default detection, and the test262 expectations file.

### Output

One shared library per platform: `libhermesintl.so` on Linux and Android,
`libhermesintl.dylib` in a macOS host build, `hermesintl.framework` in an
Apple framework build (one per Apple platform slice, collected in
`hermesintl.xcframework`), and `hermesintl.dll` on Windows. It contains the
Rust core, the ICU4X data, the JSI layer and the bytecode.

### Binary contract with the host

- The plugin uses JSI and nothing else from Hermes. JSI is not header-only:
  `jsi::JSError`'s constructors, `NativeState::~NativeState` and many helpers
  are compiled code in `API/jsi/jsi/jsi.cpp`. Except on Windows (below), the
  plugin never contains its own copy of `jsi.cpp`, so there is one copy of
  every JSI type's type information in the process, exceptions thrown by the
  plugin are caught by the host as `jsi::JSError`, and
  `hasNativeState<T>`'s `dynamic_pointer_cast` works across the boundary.
- Where the plugin's JSI references bind depends on the build. In Linux and
  macOS host builds, and in Apple builds that are not framework builds (such
  as a simulator build of the CLI), the plugin links no JSI provider. The
  `hermes` CLI links the VM statically, so there is no `libhermesvm` to link
  against. The plugin's JSI references stay undefined (`-undefined
  dynamic_lookup` on Apple platforms) and bind, when the library is loaded,
  to the JSI code the host already exports:
  - in an executable, to the executable's own copy. Every Hermes tool that
    links the VM statically and creates runtimes (`hermes`, `hvm`, `hdb`,
    `hcdp`, `synth`, `test-runner`, `napi-runner`, `hermes-jsi`,
    `hermes-jsi-lean` and the fuzzers) is linked with `-rdynamic`
    (`-Wl,-export_dynamic` on macOS) through
    `hermes_export_dynamic_symbols` in `cmake/modules/Hermes.cmake`. The
    other tools, such as `hermesc`, are not, so their unused code can still
    be dropped. A program that `shermes -static-link` builds on macOS
    links the VM and JSI in too, and needs no flag: ld64 exports an
    executable's external symbols by default, and `-export_dynamic` only
    keeps those that `-dead_strip` or LTO would remove, which that link
    does not use (`intl/test/lit/shermes-static-link.js`);
  - in a shared-library host such as `hermes-jsi-shared`, to `libhermesvm`'s
    copy.

  This is the pattern the Node-API test addons already use. The packagings
  link a provider instead:
  - **Android:** the plugin links `libjsi.so` (the CMake `jsi` target,
    `NEEDED libjsi.so`) with `-Wl,--no-undefined`, so every JSI reference
    is bound at link time to the same library `libhermesvm.so` uses.
    Configure fails on Android when `HERMES_BUILD_SHARED_JSI` is off,
    since a static JSI would be a second copy. The condition is CMake's
    `ANDROID`, which the NDK toolchain sets.
  - **Apple framework builds** (`HERMES_BUILD_APPLE_FRAMEWORK`, which
    `utils/build-apple-framework.sh` uses for every slice): the plugin
    links `hermesvm.framework`, which exports JSI, with the two-level
    namespace. Its `LC_RPATH` is `@loader_path/..` in the shallow bundles
    (iOS, tvOS, visionOS and their simulators) and `@loader_path/../../..`
    in the deep ones (macOS and Mac Catalyst), so it finds the
    `hermesvm.framework` next to its own framework. On a Mac, every slice
    has no dynamically looked-up symbol, 10 symbols imported from
    hermesvm, and one export.
  - **Windows:** no Hermes module exports JSI, so the plugin links its own
    static copy (the `jsi` target; configure fails when
    `HERMES_BUILD_SHARED_JSI` is on). This is the one exception to the
    single copy: each module has its own copy of JSI's compiled code, and
    the Microsoft C++ runtime matches C++ types across modules by their
    decorated names, so a `jsi::JSError` thrown in the plugin is still
    caught by the host as `jsi::JSError` and `hasNativeState<T>` still
    works for state the plugin attached. Under Wine, with clang-cl builds,
    both hold (the fixture pair `fake_intl_needs_dep.dll` and
    `fake_intl_dep.dll`). With the MSVC compiler and Microsoft's runtime
    this is unverified until the Windows checks under Phasing run.
- Before it loads a plugin library, the loader makes the Hermes image it runs
  in global: it finds that image with `dladdr` and re-opens it with
  `RTLD_NOW | RTLD_NOLOAD | RTLD_GLOBAL`, once per process. It skips this
  when the image is the main executable, whose exported symbols are already
  global. Without it, a host that loads `libhermesvm` `RTLD_LOCAL`, directly
  or as a dependency, would give the plugin no JSI to bind to;
  `shermes --exec` does this with the program it compiles. The cost is that
  `libhermesvm`'s exported symbols become global in those processes, so
  libraries loaded later can bind to them too. With two `RTLD_LOCAL` copies
  of `libhermesvm` in one process, the plugin binds to the first copy that
  loads it.

  The promotion happens only when a plugin will actually be loaded: before
  loading a default-resolution candidate file that exists, or an explicit
  `IntlPluginPath`. On the library search path (used only with glibc, musl
  and bionic) the loader tries `dlopen` first; if that fails for a reason
  other than "not found", it promotes and retries once. A process without a
  plugin never pays for the promotion. Windows has no promotion: the plugin
  carries its own JSI there.
- The plugin exports only `hermes_intl_get_entry`: through a version script
  on Linux and Android, `-exported_symbol` on Apple platforms, and
  `__declspec(dllexport)` on Windows (`HERMES_INTL_EXPORT` in `Plugin.cpp`).
  Nothing from the Rust core and no C++ inline function or template
  instance is exported. Except on Windows, the JSI types' type information
  stays undefined in the plugin and binds to the host's copy.
- The plugin and the host use the same C++ standard library (`c++_shared` on
  Android), with RTTI and exceptions enabled in the plugin, as they are in
  `API/hermes`. The plugin is compiled with the same `JSI_UNSTABLE` setting
  as Hermes, because that setting changes the layout of `jsi::Runtime`; the
  entry point records it, and the loader rejects a mismatch.
- On Windows, `std::string` and `std::vector` cross the interface, and their
  layout depends on `_ITERATOR_DEBUG_LEVEL` and on the debug or release C
  runtime. Both sides must use the C runtime DLL (`/MD` or `/MDd`):
  configure fails with a static runtime. The entry point records, under
  MSVC, the exact `_ITERATOR_DEBUG_LEVEL` and whether the debug runtime is
  used, and the loader rejects any difference before calling into the
  plugin, with a message naming both values, for example `built with
  _ITERATOR_DEBUG_LEVEL 2, Hermes with 0; built with the debug C runtime
  (/MDd), Hermes with the release C runtime (/MD) (flags 0xd, Hermes
  expects 0x1)`.
- The plugin is only supported with the Hermes it was built with. It is
  built in the same build and shipped next to Hermes, so both use the same
  JSI headers. The entry point (see Loading) carries the bytecode version
  and a plugin-interface version, and the loader rejects a mismatch before
  calling into the plugin, which usually catches a plugin from another
  release. The plugin-interface version is bumped whenever the entry
  point, the config struct or `IHermesBuiltinBytecode` changes; it is 2
  since Phase 6 added the Windows ABI bits to the flags. A plugin
  built against other JSI headers with the same versions is not detected;
  mixing builds that way is unsupported, as for any other JSI library.
- Phase 0 verified this contract on Linux, macOS, and the Apple framework
  layout (which library exports JSI there), and Phase 6 for each packaging
  (Android, every Apple slice, and Windows under Wine). The results are
  under Risks and Phasing.

### Build

- CMake option `HERMES_BUILD_INTL`. When it is on and no Rust toolchain
  (1.88 or newer, the MSRV of `icu` 2.3; 1.95 or newer when a tvOS or
  visionOS triple is selected) is found, configure fails with a message
  naming the option. So the default configuration requires Rust;
  `-DHERMES_BUILD_INTL=OFF` gives a Rust-free build. This is the first Rust
  in the Hermes CMake build.
- Defaults: `ON` for Linux and macOS hosts (universal macOS included),
  Android and every Apple target platform. `OFF` on Windows until a native
  Windows build is verified (see Phasing); turning it on there works and is
  what the cross build does. `OFF` where the build has no Rust target
  mapping (below), and configure prints why unless the option was set
  explicitly; turning it on there is a configure error naming
  `HERMES_INTL_RUST_TARGET`. Emscripten is refused permanently, even with
  `HERMES_INTL_RUST_TARGET`: the plugin is not built for Emscripten, and
  embedders can register a plugin with `registerIntlPlugin` (see Loader).
  An existing build directory whose cached value is still the default it
  recorded follows a new default (`hermes_intl_build_option`), and
  configure says so; a value given on the command line or edited in the
  cache is kept. The host-`hermesc` stages of the Android and Apple builds
  pass `-DHERMES_BUILD_INTL=OFF`.
- The option of the deleted built-in implementation, `HERMES_ENABLE_INTL`,
  is deprecated and ignored since Phase 7. React Native's own builds still
  pass `-DHERMES_ENABLE_INTL=True`, so a configure that sets it ON prints a
  CMake warning that names `HERMES_BUILD_INTL` and goes on; the option has
  no other effect, and no build in this repository passes it.
- CMake drives `cargo` for the target triples of the build.
  `hermes_intl_rust_targets` (`cmake/modules/HermesIntl.cmake`), a pure
  function tested with `cmake -P` (`intl/test/lit/rust-targets.js`), maps
  the build to triples:

  | Build | Rust triples |
  |---|---|
  | `HERMES_INTL_RUST_TARGET` set (a list) | that list; it wins over every rule but Emscripten |
  | Linux or other native host | the host triple from `cargo -vV` |
  | macOS host | `aarch64-apple-darwin` and/or `x86_64-apple-darwin`, one per entry of `CMAKE_OSX_ARCHITECTURES` (empty: the host's) |
  | Android `arm64-v8a`, `armeabi-v7a`, `x86`, `x86_64` | `aarch64-linux-android`, `armv7-linux-androideabi`, `i686-linux-android`, `x86_64-linux-android` |
  | Apple `macosx`, `catalyst` | `{aarch64,x86_64}-apple-darwin`, `{aarch64,x86_64}-apple-ios-macabi`, per architecture |
  | Apple `iphoneos`, `iphonesimulator` | `aarch64-apple-ios`; `aarch64-apple-ios-sim`, `x86_64-apple-ios` |
  | Apple `appletvos`, `appletvsimulator` | `aarch64-apple-tvos`; `aarch64-apple-tvos-sim` only |
  | Apple `xros`, `xrsimulator` | `aarch64-apple-visionos`; `aarch64-apple-visionos-sim` only |
  | Windows with the MSVC ABI, x64 or ARM64 | `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc` |
  | Emscripten | none, permanently |
  | anything else (another Android ABI, Apple platform or architecture, Win32, MinGW, another cross build) | none: set `HERMES_INTL_RUST_TARGET` |

  Rust has no supported x86_64 target for the tvOS and visionOS
  simulators, so those slices of the plugin are arm64 only (see
  Packaging). The Windows architecture comes from the first of
  `CMAKE_CXX_COMPILER_ARCHITECTURE_ID`, `CMAKE_VS_PLATFORM_NAME` and
  `CMAKE_SYSTEM_PROCESSOR` that is set. For Apple triples the deployment
  variable cargo needs (`MACOSX_DEPLOYMENT_TARGET`,
  `IPHONEOS_DEPLOYMENT_TARGET` for iOS, its simulator and Catalyst,
  `TVOS_DEPLOYMENT_TARGET`, `XROS_DEPLOYMENT_TARGET`) is set from
  `CMAKE_OSX_DEPLOYMENT_TARGET` when that is set.
- Configure uses the `cargo` on `PATH`, else the one in `$CARGO_HOME/bin`
  or `~/.cargo/bin`, where rustup installs it: a Gradle daemon or an Xcode
  build phase launched from the GUI does not get the shell's `PATH`. It
  uses the `rustc` next to that `cargo`. Without one, configure fails and
  names the places it searched (`intl/test/lit/find-cargo.js`).
- `intl/CMakeLists.txt` runs `cargo build --locked --lib [--release]
  --target <t>` once per triple; only the first writes the cbindgen
  header. Several triples must be Apple triples of one platform and of
  different architectures, and their archives are combined with `lipo
  -create` into `<cargo>/universal/<profile>/`. On Android each cargo
  command also gets `CC_<t>`, `CFLAGS_<t>` and `AR_<t>` from the NDK
  toolchain. Configure fails, naming the fix, when a triple's Rust standard
  library is not installed (`rustup target add <t>`), when Rust is too old,
  or when two Apple archives need `lipo` and it is not found. The native
  libraries the Rust standard library needs are per target: linux-gnu
  `Threads dl m`, Android `dl log m`, Apple none beyond the system,
  windows-msvc `kernel32 ntdll userenv ws2_32 dbghelp`.
- Multi-configuration generators are supported: the cargo profile and
  output path come from `$<CONFIG>` (Debug uses cargo's dev profile, every
  other configuration `--release`), with `IMPORTED_LOCATION_<CONFIG>`;
  `check-intl-rust` does the same, always for the host triple. This was
  run with Ninja Multi-Config on Linux. The Visual Studio generator, which
  Windows CI uses, waits for the Windows checks under Phasing; the Apple
  scripts and the podspec use single-configuration generators.
- The plugin's JS is compiled by the `hermesc` from the same build
  (`$<TARGET_FILE:hermesc>`). CMake reads the engine's bytecode version and
  passes it to the plugin's C++, which reports it in the entry point. When
  the build's own `hermesc` cannot run on the build machine, the build must
  import the host's (`IMPORT_HOST_COMPILERS=<host
  build>/ImportHostCompilers.cmake`), and configure fails without it: in
  any cross build (`CMAKE_CROSSCOMPILING`, which includes the Windows CI
  job's `CMAKE_SYSTEM_NAME=Windows`), for every Apple target platform other
  than `macosx` (Catalyst included), and for a macOS build whose
  `CMAKE_OSX_ARCHITECTURES` leaves out the host processor (also under
  Rosetta, where it is conservative).
- Where the library goes: `<build>/lib`, next to `libhermesvm`, in a
  single-configuration build; next to `hermesvm.framework` in an Apple
  framework build (`<build>/lib/hermesintl.framework`, or
  `<build>/lib/<Config>/` under a multi-configuration generator, wherever
  `hermesvm.framework` goes: the plugin takes hermesvm's output directory
  settings); otherwise the tools' directory (`bin`, or `bin/<Config>`)
  under a multi-configuration generator and on Windows, with a copy next
  to a shared `libhermesvm` or `hermesvm.dll`, since a shared-library host
  resolves the plugin from there. Default resolution finds it in each of
  these places.
- `Cargo.lock` is committed, and every cargo invocation from CMake uses
  `--locked` and runs from `intl/core`, at configure and at build time, so
  that a rustup toolchain override there applies to both. The ICU4X
  crates stay on 2.3.x (CLDR 48.2): `Cargo.toml` uses `~` requirements
  for them, and the lock file must not mix ICU4X minor versions. Shared
  dependencies such as `zerovec`, `yoke` and `tinystr` are not pinned
  exactly in `Cargo.toml`; the lock file holds them. Crates are
  fetched by default; offline builds through `cargo vendor` are supported
  but not required.

### Packaging

**Android.**

- `android/build.gradle.kts` passes `-DHERMES_BUILD_INTL=ON` and builds
  `hermesintl` next to `hermesvm`. The AAR carries
  `jni/<abi>/libhermesintl.so` for arm64-v8a, armeabi-v7a, x86 and x86_64,
  beside `libhermesvm.so`; nothing else was needed. Like `libhermesvm.so`,
  the plugin is 16 KB aligned on the 64-bit ABIs and 4 KB aligned on the
  32-bit ones. The AAR still leaves out `libjsi.so`, which React Native
  supplies.
- Since Phase 7 the AAR has no Java code for `Intl`: its `classes.jar`
  holds only `com/facebook/hermes/unicode/AndroidUnicodeUtils`, where it
  held 76 classes under `com/facebook/hermes/intl` before, and
  `libhermesvm.so` names none of them. Without `libhermesintl.so` a
  runtime has no `Intl` (see "Without the plugin"). Phase 7 also deleted
  `android/intltest`, the instrumented tests of the built-in
  implementation, with its Gradle project, the build-logic properties
  only it read, and the `androidx.annotation` library and
  `de.undercouch.download` plugin, which nothing else used. The Java
  sources of the AAR are now only `lib/Platform/Unicode/java`.
- The CI workflows that build the AAR (`build-android.yml`, and the
  publish job of `rn-build-hermes.yml`) install rustup and the four
  Android targets.
- The loader needs nothing Android-specific: default resolution finds the
  plugin next to `libhermesvm.so` when libraries are extracted, and through
  the library search path when they are mapped from the APK
  (`extractNativeLibs=false`), which is how a React Native app loads them.
- The command-line tools build in mobile configurations
  (`HERMES_IS_MOBILE_BUILD`) and set a finalizer thread runner with no JNI
  attachment (`cli::toolFinalizerThreadRunner`), so they no longer abort at
  exit in a process without a JVM; see Workers. `test-runner` still does
  not build in mobile configurations, as before the plugin.

**Apple platforms.**

- With `HERMES_BUILD_APPLE_FRAMEWORK` on, `hermesintl` is a framework, made
  by the same helper as `hermesvm` (`hermes_apple_framework` in
  `cmake/modules/Hermes.cmake`), with the bundle identifier
  `dev.hermesengine.intl.<platform>` and the install name
  `@rpath/hermesintl.framework/hermesintl` (`Versions/1/hermesintl` in the
  deep bundles).
- `utils/build-apple-framework.sh` builds `hermesvm` and `hermesintl` for
  each of the eight slices (macOS, iOS, iOS simulator, Mac Catalyst, tvOS,
  tvOS simulator, visionOS, visionOS simulator), makes a dSYM for each, and
  creates `universal/hermesvm.xcframework` and
  `universal/hermesintl.xcframework`. Before building a slice it checks
  that cargo and the slice's Rust targets are installed and prints the
  `rustup target add` command otherwise; `build-apple-framework.sh
  rust_targets <platform>` prints them from the CMake mapping
  (`utils/intl-rust-targets.cmake`). Its host `hermesc` is built with the
  other platforms' deployment-target variables unset: clang picks its
  default target from `XROS_DEPLOYMENT_TARGET` and the like, so with
  `XROS_DEPLOYMENT_TARGET` in the environment the host `hermesc` was built
  for visionOS and its configure failed (a fix outside the plugin).
- The tvOS and visionOS simulator slices of the plugin are arm64 only
  (`OSX_ARCHITECTURES arm64` on `hermesintl` there), while `hermesvm`'s
  have arm64 and x86_64. In a tvOS simulator build for x86_64, CocoaPods'
  copy phase selected hermesvm's slice name,
  `tvos-arm64_x86_64-simulator`, and looked for that same name in
  `hermesintl.xcframework`, which has no such slice, so the build failed
  (an arm64 tvOS simulator build selected `tvos-arm64_x86_64-simulator`
  for hermesvm and `tvos-arm64-simulator` for the plugin, and worked).
  After `xcodebuild -create-xcframework`,
  `utils/intl-xcframework-sim-slices.py` therefore makes those two plugin
  slices list x86_64 as well and renames them to hermesvm's names
  (`tvos-arm64_x86_64-simulator`, `xros-arm64_x86_64-simulator`). The
  intent: an x86_64 simulator build links (the linker ignores the arm64
  binary for x86_64) and embeds the plugin, and an x86_64 process gets
  dyld's incompatible-architecture error for it, which the loader treats
  as no plugin. The second Mac run (Phasing item 6) showed the build,
  the embedding and the missing x86_64 load command; an x86_64 process
  at run time has not run, since only arm64 simulator runtimes were
  available. The cost is xcframework metadata that names an architecture
  its binary lacks.
- `hermes-engine.podspec` vendors both xcframeworks on every platform and
  adds `-weak_framework hermesintl` to the app's `OTHER_LDFLAGS`. CocoaPods
  links every vendored framework, but nothing in the app refers to the
  plugin, so the load command is `LC_LOAD_WEAK_DYLIB` and the app still
  launches when the plugin is missing. On a Mac, the iOS consumer app has
  the weak load command and runs with the plugin; with
  `hermesintl.framework` deleted from the app it runs without it; and a
  control without the flag fails at launch (`Library not loaded:
  @rpath/hermesintl.framework/hermesintl`). An app whose plugin lacks the
  process's architecture (the x86_64 tvOS and visionOS simulators) is
  expected to launch the same way. The second Mac run (Phasing item 6)
  verified the pieces on tvOS: the x86_64 build embeds the plugin and has
  no load command for it, the "arm64 x86_64" build runs with the plugin,
  the app without the plugin runs, and the non-weak control fails at
  launch. The x86_64 process itself has not run.
- Since Phase 7, `utils/build-apple-framework.sh` no longer passes the
  deprecated option (see Build), and an app without the plugin has no
  `Intl`; the podspec's comment says so. The link of `hermesvm` no longer
  names Foundation, which only the built-in implementation used;
  CoreFoundation stays, for Hermes's own locale fallbacks
  (`PlatformUnicodeCF.cpp`). The built `hermesvm.framework` of every
  slice no longer loads Foundation or libobjc, and the smoke tests and
  CocoaPods app without the plugin pass (Phasing, item 7).
- The `hermes-ios` tarball carries both xcframeworks, and
  `validateHermesFrameworksExist` (`utils/scripts/hermes/hermes-utils.js`)
  requires both. The CI workflow that builds the slices
  (`build-apple-slices-hermes.yml`) installs Rust and each slice's targets
  and checks both frameworks and their dSYMs; `build-hermes-macos.yml`
  archives the plugin's dSYM next to hermesvm's. The CI steps have not run
  yet; they run on GitHub's macOS runners.
- The `macos` job of `build.yml` builds the command-line tools universal
  (x86_64 and arm64), so it builds the plugin too: it installs Rust and
  both `-apple-darwin` targets. The tools have `Intl` only when default
  resolution finds the plugin, so the CLI tarball
  (`hermes-cli-darwin.tar.gz`) carries `libhermesintl.dylib` next to them,
  the first place they look. This job has not run yet either.
- **React Native.** The `react-native` repository needs changes that are
  not part of this repository:
  - Its own podspec for `hermes-engine` must vendor
    `hermesintl.xcframework` next to `hermesvm.xcframework` and link it
    weakly (`-weak_framework hermesintl`). Without the first, React Native
    apps on Apple platforms have no plugin, and since Phase 7 deleted the
    built-in implementation they have no `Intl` at all. Without the
    second, an app whose plugin is missing or lacks the running
    architecture fails to launch.
  - On Android a React Native app has `Intl` only when
    `libhermesintl.so` is packaged with it; the AAR carries it. React
    Native's ProGuard keep rules for `com.facebook.hermes.intl` are stale,
    since those classes no longer exist.
  - Its builds still pass `-DHERMES_ENABLE_INTL=True`, which now only
    prints the deprecation warning (see Build); they can drop it.
  - **Release gate.** These changes are in another repository and cannot
    be made or tested here, so they do not block Phase 7 from landing on
    the integration branch. A Hermes release that contains Phase 7 must
    not ship to React Native until React Native's podspec vendors
    `hermesintl.xcframework` and links it weakly, and its builds of
    Hermes from source have Rust or pass `-DHERMES_BUILD_INTL=OFF` (next
    item). The `-DHERMES_BUILD_INTL=OFF` branch leaves those apps with
    no `Intl` at all, where today they have the built-in one. The first
    compatible React Native revision: not yet known; to be recorded here
    when React Native's change lands.
  - Its builds of Hermes from source (the Android Gradle build and the iOS
    build from source) configure Hermes with CMake, where
    `HERMES_BUILD_INTL` now defaults to `ON` for Android and every Apple
    platform. They need Rust and the targets of their ABIs or slices at
    configure time, or must pass `-DHERMES_BUILD_INTL=OFF`, which leaves
    the plugin out. In this repository, without Rust, the Android Gradle
    build (which passes `-DHERMES_BUILD_INTL=ON`) stops at configure with
    "HERMES_BUILD_INTL is ON, which requires a Rust toolchain, but cargo
    was not found on PATH or in <the directories searched>", and
    `utils/build-apple-framework.sh` stops before building a slice with
    "the Intl plugin (hermesintl.framework) needs Rust, but cargo was not
    found on PATH or in <the directories searched>", each naming the fix.
    Both look in `$CARGO_HOME/bin` and `~/.cargo/bin` after `PATH`.

**Windows.**

- The plugin is `hermesintl.dll`, built with a shared C runtime and its own
  static JSI (see the binary contract), linked with `/OPT:REF` even in
  Debug, and exporting only `hermes_intl_get_entry`. It imports the C
  runtime DLLs, `kernel32`, and the system DLLs Rust's standard library
  uses (`ntdll`, `bcryptprimitives`, `api-ms-win-core-synch-l1-2-0`), and
  no Hermes module.
- The build was cross-compiled on Linux only, with clang-cl, lld-link and
  the MSVC C runtime, C++ library and Windows SDK that xwin downloads,
  through the committed toolchain file
  `utils/windows-cross/clang-cl-xwin.cmake` (x64 and arm64, Release and
  Debug). The toolchain turns Boost.Context off, because llvm-ml64
  assembles a `lea` in Boost.Context's MASM source as an absolute address
  and `hermes.exe` then crashes; a native MSVC build keeps the fibers. The
  cross build also needs `-DHERMES_ENABLE_NAPI=OFF`: two Node-API test
  addons produce the same import library name, which Ninja rejects. Three
  fixes to Hermes's own clang-cl build were needed: `Hermes.cmake` tests
  `MSVC` before `GCC_COMPATIBLE`, JSI gets `/EHsc` under clang-cl, and
  `/WHOLEARCHIVE` names the archive by path.
- `HERMES_BUILD_INTL` stays `OFF` by default on Windows and the Windows CI
  workflow is unchanged until a native build is verified (see Phasing).

**Sizes.** Measured in Phase 6, in bytes. Size remains a non-goal; nothing
was done to reduce it.

| Platform | Plugin | Build it comes from |
|---|---|---|
| Linux x86_64 | 19,931,056 stripped (24,241,992 unstripped) | Ninja Multi-Config Release, Clang, Linux host |
| Android arm64-v8a | 19,766,600 | MinSizeRel, NDK r28c, configured as AGP does; stripped |
| Android armeabi-v7a | 18,455,688 | same |
| Android x86 | 19,143,460 | same |
| Android x86_64 | 19,925,832 | same |
| iOS (arm64) | 19,743,736 | `build-apple-framework.sh`, MinSizeRel, Xcode 26.4, the framework binary as built |
| iOS simulator (arm64 + x86_64) | 39,843,880 | same |
| macOS (arm64 + x86_64) | 39,860,312 | same |
| Mac Catalyst (arm64 + x86_64) | 39,755,560 | same |
| tvOS (arm64) | 19,733,928 | same |
| tvOS simulator (arm64) | 19,937,560 | same |
| visionOS (arm64) | 19,760,160 | same |
| visionOS simulator (arm64) | 19,904,712 | same |
| Windows x64 | 19,026,944 | clang-cl cross build, Release |
| Windows arm64 | 18,877,440 | same |

The Apple rows are from Xcode 26.4. Builds with Xcode 27.1 are slightly
smaller (macOS, for example: 39,487,160 bytes for the plugin, against
39,860,312).

The AAR's copies, stripped by AGP, are 300 to 504 bytes larger than the
Android figures. For comparison, the stripped `libhermesvm.so` of the same
Android arm64 build is 2,883,928 bytes. That is the Phase 6 measurement
build; the "before" figure below (2,884,968) is from a later build, made
just before Phase 7.

Deleting the built-in implementation in Phase 7 made `libhermesvm.so`
smaller. Bytes, stripped with `llvm-strip`, from the same MinSizeRel
builds configured as AGP does (NDK r28c, IPO, `HEAP_HV_PREFER32`), before
and after the deletion:

| Android ABI | Before | After | Change |
|---|---|---|---|
| arm64-v8a | 2,884,968 | 2,807,792 | -77,176 (-2.7%) |
| armeabi-v7a | 1,981,592 | 1,931,472 | -50,120 (-2.5%) |
| x86 | 3,147,300 | 3,068,648 | -78,652 (-2.5%) |
| x86_64 | 3,015,168 | 2,937,736 | -77,432 (-2.6%) |

The AAR's `classes.jar` went from 131,299 to 2,227 bytes, and the whole
AAR from 118,552,587 to 116,262,557 bytes; `libhermesintl.so` stayed
about the same (x86_64: 19,926,152, then 19,928,152 bytes). The
`hermesvm.framework` sizes per slice are under Phasing, item 7.

## Loading and installation

### The plugin's entry point

One exported `extern "C"` function, `hermes_intl_get_entry()`, returning a
pointer to a static struct, declared in `public/hermes/Public/IntlPlugin.h`:

- `struct_size`;
- `plugin_interface_version` and `bytecode_version`;
- `flags`, whose bit 0 is `JSI_UNSTABLE`; under MSVC bits 1 and 2 hold
  `_ITERATOR_DEBUG_LEVEL` and bit 3 says whether the debug C runtime is
  used (elsewhere they are 0). `HERMES_INTL_BUILD_FLAGS` in `IntlPlugin.h`
  computes the word, so each side evaluates it in its own build. The
  loader rejects any difference, because these settings change the layout
  of `jsi::Runtime` and of the standard library types the interface uses;
- `resolve_defaults`: a function pointer taking a `hermes_intl_config *`
  that holds the locales and time zone `RuntimeConfig` requested, possibly
  none. In place, it canonicalizes the given values and detects the missing
  ones (see "Default locale and time zone"). It returns `std::string`: empty
  on success, or a message naming an invalid explicit value, which the
  loader makes fatal. This is how the loader learns the effective defaults
  it records for Workers;
- `install`: a function pointer taking `jsi::Runtime &` and the
  `const hermes_intl_config *` that `resolve_defaults` filled.

Fields are only ever appended, and `struct_size` lets the loader reject an
entry that is too small to hold them all.

`hermes_intl_config` holds a `std::vector<std::string>` of locales and a
`std::string` time zone. The same-build contract already requires a shared
C++ standard library, so the config does not need to be plain C.

Returning the version fields and the functions together guarantees they come
from the same implementation. `extern "C"` only removes name mangling; the
function signatures are still a C++ interface, covered by the binary contract
above.

`install` reports failure by throwing. The loader turns any exception from
`resolve_defaults` or `install` into a fatal error.

### `IHermesBuiltinBytecode`

A new Hermes JSI interface in `API/jsi/jsi/hermes-interfaces.h`, found
through the runtime's `castInterface(uuid)` like the existing ones. It has one
method that runs a bytecode buffer with the flags `loadAndInstallExtensions`
(`API/hermes/hermes.cpp`) uses for the extensions: `persistent`,
`hidesEpilogue` and `funcsAreBuiltins`. The plugin requires it.

`funcsAreBuiltins` makes `Function.prototype.toString` print `[native code]`
for every plugin function (`lib/VM/JSLib/Function.cpp`). It does not change
constructibility, `prototype` properties or descriptors; those come from the
function-shape rules below.

### Default locale and time zone

Taken from `RuntimeConfig` when set there. Otherwise the plugin detects them.
Both happen in the plugin's `resolve_defaults`, called by the loader before
`install`:

| Platform | Locale | Time zone |
|---|---|---|
| Linux | `LC_ALL`, then `LC_MESSAGES`, then `LANG` | `TZ`, then `/etc/localtime` |
| macOS, iOS | CoreFoundation preferred languages | CoreFoundation system time zone |
| Windows | `GetUserDefaultLocaleName`, without a sort suffix (`de-DE_phoneb` becomes `de-DE`) | `GetDynamicTimeZoneInformation().TimeZoneKeyName`, mapped to IANA through CLDR's `windowsZones` table (ICU4X's `WindowsParser`, `host::time_zone_from_windows`): the row of the user's territory (`GetUserDefaultGeoName`, looked up at run time since it exists only from Windows 10 1709) when it has one for that zone, else the `001` row; `TZ` is not read |
| Android | from `RuntimeConfig`; if unset, the system locale property (`persist.sys.locale`, then `ro.product.locale`) | `TZ`, then the `persist.sys.timezone` system property — the same order bionic's `tzset` uses (bionic `libc/tzcode/bionic.cpp`) |

Normalization:

- POSIX locale names drop the encoding, turn `_` into `-`, and map `C` /
  `POSIX` to `en-US`. Modifiers that name a script become a script subtag
  unless the name already has one (`sr_RS@latin` → `sr-Latn-RS`, also
  `@cyrillic`, `@devanagari`, and `@iqtelif`, which is Latin); `@valencia`
  becomes the `valencia` variant; `@euro` and any modifier not in the table
  are dropped.
- Every default locale, detected or explicit, is reduced to what ECMA-402's
  DefaultLocale must be: an available locale with no extensions. The tag is
  canonicalized, its extensions (`-u-`, `-t-`, `-x-` and any other) are
  dropped, and it is truncated to its longest available prefix, dropping
  trailing subtags. The truncation never changes the script: if the prefix
  found is written in a different script than the tag (by its script subtag
  or its likely script), the tag reduces to nothing. So `tt-Latn-RU` (glibc
  `tt_RU@iqtelif`), which is not available, never becomes Cyrillic `tt`.
  The prefix found must also have data in that script: a tag that is
  available only as a less narrow form of entries in another script (the
  second closure rule of `[[AvailableLocales]]` below) reduces to nothing
  too. So `ff`, `ff-BF` and `bal`, available because of `ff-Adlm-BF` and
  `bal-Latn` but likely Latin and Arabic, are never a default;
  `ff-Adlm-BF` and `bal-Latn` are.
  - A detected value that is not well-formed, or reduces to nothing, is
    ignored, and the next source is tried.
  - In an explicit `IntlDefaultLocales`, an ill-formed element is fatal.
    Each well-formed element is reduced; elements that reduce to nothing,
    and repeats, are dropped. If nothing is left, that is fatal too. An
    embedder's locale can therefore be narrowed silently
    (`en-US-u-ca-buddhist` becomes `en-US`).
- `TZ` is used only if, after dropping a leading `:` and any path prefix up
  to `zoneinfo/` (and glibc's `posix/` and `right/` trees below it), it names
  an IANA zone the plugin knows. Names match case-insensitively and become
  their primary identifier: `Asia/Calcutta` becomes `Asia/Kolkata`,
  `Etc/UTC`, `Etc/GMT` and `GMT` become `UTC`, and `CST6CDT`, `EST5EDT`,
  `MST7MDT` and `PST8PDT`, which CLDR still lists as zones but tzdb 2024b
  made links, become `America/Chicago`, `America/New_York`,
  `America/Denver` and `America/Los_Angeles`. The same primary identifiers
  are what `Intl.supportedValuesOf("timeZone")` lists. POSIX rule strings are
  ignored. On Linux and Android, a `TZ` that is set but empty means `UTC`, as
  it does to glibc and tzcode; only an unset `TZ` falls through to the next
  source. `/etc/localtime` is used only if it is a symlink into a `zoneinfo`
  directory.
- An explicit `IntlDefaultTimeZone` is matched the same way; one the plugin
  does not know is fatal.
- Anything that cannot be determined falls back to `en-US` and `UTC`.

**Agreement with `Date`.** Hermes `Date` computes local time through the OS
(`lib/VM/JSLib/DateUtil.cpp`). The detection order above follows the OS's own
order on each platform, so `Date` and `Intl` normally agree on the **zone
identifier**. They can disagree when the OS accepts a source the plugin
ignores (a POSIX rule string in `TZ`, a copied `/etc/localtime`); the plugin
then tries its remaining sources, ultimately falling back to `UTC`, while
`Date` uses the OS's rules. They do not necessarily
agree on **offsets**: `Date` uses the OS's zone rules and the plugin uses its
bundled TZDB, so they can differ right after a rule change. Setting
`IntlDefaultTimeZone` changes only `Intl`, never `Date`. All of this is
documented, not reconciled. On Windows the plugin never reads `TZ`,
because the C runtime's own `TZ` format is not IANA, so when a user sets
`TZ` there, `Date` follows it and `Intl` does not. Since Phase 4 the Date
`toLocaleString`, `toLocaleDateString` and `toLocaleTimeString` methods
are the plugin's, so they show the plugin's default time zone and offsets,
while `getHours` and `toString` show the OS's.

### What setup does

Setup runs before any user code. It:

1. captures every intrinsic the JS layer uses;
2. defines `globalThis.Intl`;
3. replaces the locale-sensitive built-ins with their ECMA-402 versions (each
   in the phase that implements it):
   `String.prototype.localeCompare`, `toLocaleUpperCase`, `toLocaleLowerCase`,
   and `toLocaleString` on Number, BigInt, Array and TypedArray, plus
   `toLocaleString`, `toLocaleDateString` and `toLocaleTimeString` on Date.

The setup JS is part of the same build as the plugin; if it throws, that is a
plugin bug and is fatal, as it is for the extensions
(`loadAndInstallExtensions`). Everything that can legitimately fail (finding
the library, versions, the interface, invalid explicit defaults) is checked by
the loader before setup starts, so there is never a half-installed `Intl`.

### Loader

In `API/hermes`, shared by `makeHermesRuntime` and
`makeThreadSafeHermesRuntime` (`API/hermes/hermes.cpp`), which both
install the extensions and must both install the plugin, right after the
extensions. ConsoleHost (the `hermes` CLI), the REPL and React Native go
through one of the two.

For each new runtime:

1. If `EnableIntlPlugin` is false, do nothing.
2. If `IntlPluginPath` is set, load that library.
3. Otherwise use the default plugin, resolved once per process in this order:
   1. a plugin registered by the embedder through
      `hermes::registerIntlPlugin(entry)` before creating runtimes — the way
      to use a statically linked plugin, since `dlsym` cannot find code the
      static linker never pulled in;
   2. the standard library name (`libhermesintl.so`, `libhermesintl.dylib`,
      `hermesintl.dll`) in the directory containing the Hermes code; and,
      when that directory is named `bin`, in `../lib`, which is the build
      and install layout (`bin/hermes`, `lib/libhermesintl.so`). Windows
      finds the directory differently (see below). `dladdr` tells whether the
      Hermes code is in the main executable or in a library. For the main
      executable `dladdr` may return a bare `argv[0]`, so the executable's
      path comes from the OS instead: `/proc/self/exe` on Linux,
      `proc_pidpath` on macOS. For a library, `dladdr` returns the path
      given to `dlopen`, which may be relative to a directory the host has
      since left. So on Linux and Android the library's file is the one
      `/proc/self/maps` shows mapped at the address of a loader function.
      The kernel escapes only a newline there (as `\012`) and appends
      ` (deleted)` to a removed file. A pathname with neither is used as
      written if it exists; its device and inode are not compared, because
      on overlayfs and btrfs subvolumes they differ from what `stat`
      reports. A pathname with a `\012` or a ` (deleted)` suffix is
      ambiguous, and is used only through a reading (each `\012` decoded,
      or all as written; with the suffix kept or stripped) whose `stat`
      device and inode are the mapping's. Otherwise, as for a mapping
      without an inode or an unreadable `/proc`, the loader falls back to
      `dladdr`'s path, but only if it is absolute, `realpath` resolves it,
      and, when the maps line gave the mapping's device and inode, `stat`
      of the resolved path has them. A relative `dladdr` path may be
      relative to a directory the host has left, and `realpath` shows only
      that some file is there now; otherwise there is no directory. A removed libhermesvm
      therefore usually finds no plugin, which is the safer result: it
      usually means an upgrade, and the plugin beside it would be from
      another build. On Apple platforms `dladdr`'s path is used if
      `realpath` resolves it: dyld records an absolute path for a library
      loaded from disk (it asks the kernel for the path of the file it
      opened, with `F_GETPATH`). The directory is taken after `realpath`.
      Failing to find it is never fatal: resolution goes on to the next
      step. A candidate is loaded unless `stat` says it does not exist;
   3. on Apple platforms only, the sibling framework: when the directory
      containing the Hermes code is `<p>/<name>.framework` (a shallow
      bundle, as on iOS) or `<p>/<name>.framework/Versions/<v>` (a deep
      one, as on macOS), `<p>/hermesintl.framework/hermesintl`. That is
      where an app that embeds `hermesvm.framework` and
      `hermesintl.framework` has the plugin (`appleFrameworkCandidate`, a
      pure function tested on every host). `<name>` need not be
      `hermesvm`;
   4. only with glibc, musl and bionic, the default library search path. A
      library tried there counts as absent only when the loader's error
      message says the library itself was not found. A missing dependency, a
      wrong architecture or unresolved symbols mean it was found but is
      broken. glibc's and musl's messages must match exactly; any Linux C
      library other than glibc and bionic is taken to word them as musl
      does. With any other C library a broken library could not be told
      from an absent one, so the step is compiled out
      (`HERMES_INTL_SEARCH_PATH`), as on Apple platforms and Windows: an
      install there that relies on the search path gets no plugin, and can
      keep the plugin next to Hermes, register it, or set
      `IntlPluginPath`.

Apple platforms have no search-path step; default resolution there ends
after the sibling framework candidate. Trying a leaf name with dyld cannot
reliably tell "absent" from "broken". Older dyld reports only `image not
found`, with no list of the paths it tried. In a process with the hardened
runtime or in a sandbox, dyld gives policy reasons such as `relative path
not allowed in hardened program` or `file system sandbox blocked
open()`. Counting those as "broken" would make every runtime creation
fatal in such a process, which includes React Native apps. A leaf name
also makes dyld search the current directory, so whoever controls that
directory could plant a library. An Apple embedder that does not keep the
plugin next to the Hermes code registers it with `registerIntlPlugin` or
sets `IntlPluginPath`.

On Apple platforms a default-resolution candidate whose `dlopen` fails
because the file has no code for the running architecture counts as
absent: resolution goes on to the next candidate, and after the last one
there is no plugin, silently. dyld4 lists the files it tried, as
`tried: '<file>' (<reason>), '<file>' (<reason>), ...`, and says `mach-o
file, but is an incompatible architecture` for a single-architecture file
and `fat file, but missing compatible architecture` for a universal one
(both texts seen on macOS with Xcode 26.4). The loader
(`isIncompatibleArchitecture`) parses the whole message, which must be
`dlopen(<candidate>, 0x<flags>): tried: ` followed by the entries, each
reason with balanced parentheses, and nothing after the last one. It
counts the candidate as absent only when that parse succeeds and every
entry whose file is exactly the candidate's path (there is at least one)
has a reason that begins with one of those two phrases. Everything else
stays fatal: another reason for the candidate (such as `slice is not
valid mach-o file`), a dependency's failure (`dlopen(...): Library not
loaded: ...`, so a plugin present for the running architecture whose
`hermesvm.framework` is not still fails loudly), an unterminated reason,
text after the entries, and any other message. Text in a directory name
or a dependency's path never matches, since only an entry of the
top-level list counts. The rule is stated for dyld4 (macOS 12, iOS and
tvOS 15, visionOS, and later), which covers every simulator runtime the
tvOS and visionOS slices run in; older loaders' wordings (`mach-o, but
wrong architecture`, `missing compatible arch in`, `is not a compatible
arch`) are deliberately not recognised, so on such a loader a missing
slice is fatal, the safe direction. macOS shows the error, and no plugin,
for an x86_64 process with an arm64-only plugin. An x86_64 process in the
tvOS or visionOS simulator, whose plugins are arm64 only (see Packaging),
is expected to behave the same; that is unverified, since the simulator
runtimes used were arm64 only. On Xcode 27.1 the message texts were
checked verbatim (Phasing item 6). The promotion has already happened
by then, which is harmless. Through `IntlPluginPath` the same failure is
fatal, and every other failure of a candidate is fatal.

`IntlPluginPath` is passed to `dlopen` as given. A name without a slash is
therefore looked up the way `dlopen` looks up such names: on Linux and
Android, the library search path, never the current directory. dyld also
searches the current directory for such a name, so on Apple platforms
`IntlPluginPath` should be a path.

On macOS, `proc_pidpath` rebuilds the path from the kernel's name cache, and
for a hard-linked executable it may report another link's path. Hard-linked
layouts are not supported for default resolution: such an install may load
the plugin from another link's directory.

**Windows.** The Hermes code's module (`hermes.exe`, or `hermesvm.dll` in a
shared-library host such as `hermes-jsi-shared.exe`) comes from
`GetModuleHandleExW(FROM_ADDRESS | UNCHANGED_REFCOUNT)` on a loader
function, and its path from `GetModuleFileNameW` with a growing buffer.
The candidates are `hermesintl.dll` in that directory, then
`<parent>\lib\hermesintl.dll` when the directory's name is `bin` in any
ASCII case (the parent is made by removing the last component, not with
`..`). A candidate is absent when `GetFileAttributesW` fails with
`ERROR_FILE_NOT_FOUND` or `ERROR_PATH_NOT_FOUND`; anything else counts as
present, and a present candidate that fails to load is fatal. A candidate
of 260 characters or more is used in its `\\?\` form. Libraries load with
`LoadLibraryExW(path, nullptr, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR |
LOAD_LIBRARY_SEARCH_DEFAULT_DIRS)`, so the plugin's own directory is
searched for its imports, with the thread error mode set so that Windows
shows no error dialog. Failures read `error <n>: <FormatMessageW text>`,
with `%1` replaced by the path. `IntlPluginPath` is UTF-8 and is converted
to UTF-16; invalid UTF-8 is fatal. A value containing `\`, `/` or `:` is
made absolute with `GetFullPathNameW`; a bare name is loaded with
`LOAD_LIBRARY_SEARCH_DEFAULT_DIRS` only (the application directory,
System32 and directories added with `AddDllDirectory`), never from the
current directory or `PATH`. There is no search-path step and no
promotion. The path and text handling are pure functions tested on every
host (`IntlWindowsPathTest`, `IntlWindowsTextTest`); the Win32 calls have
run only under Wine.

**Emscripten.** The loader loads no libraries there: only
`registerIntlPlugin` works, and `IntlPluginPath` is fatal ("Hermes does not
load plugin libraries on this platform; register the plugin with
registerIntlPlugin instead"). The Hermes build does not build the plugin
for Emscripten either (configure refuses `HERMES_BUILD_INTL=ON`). This is
permanent: loading a library there would need Emscripten's dynamic
linking (`MAIN_MODULE`/`SIDE_MODULE`) for the whole Hermes wasm build,
which no Hermes wasm product uses. An embedder that builds a plugin for
Emscripten itself can link it statically and register it.

Loaded libraries are cached by path and never unloaded. The cache and the
default resolution are protected by a mutex, since runtimes can be created on
several threads.

**Errors.**

| Situation | Result |
|---|---|
| Disabled | no plugin |
| Default resolution finds nothing (the search path is tried only with glibc, musl and bionic) | no plugin, silently |
| On Apple platforms, a default-resolution candidate has no code for the running architecture | treated as absent: resolution goes on, and with no other candidate there is no plugin, silently |
| A library exists but cannot be loaded, lacks `hermes_intl_get_entry`, or has a mismatched version or flag (`JSI_UNSTABLE`; on Windows `_ITERATOR_DEBUG_LEVEL` and the debug C runtime) | fatal, with a message naming the library and the reason |
| A registered entry has a mismatched version or flag | fatal |
| `IntlPluginPath` set and anything at all goes wrong | fatal |
| `resolve_defaults` rejects an explicit `IntlDefaultLocales` or `IntlDefaultTimeZone` | fatal |
| `registerIntlPlugin` called after the default plugin was resolved | fatal |
| Setup throws | fatal |

A library that exists but is broken is a build or packaging error, so it is
reported loudly instead of silently running without `Intl`.

**Workers.** `startWorker` (`API/hermes/extensions/Worker.cpp`) used to
create the worker with a default `RuntimeConfig`. A worker must get the same
`Intl` as its parent, so it inherits the parent's **effective** state, not its
configuration:

- the loader records, per runtime, which plugin it installed (or none) and the
  default locale list and time zone actually passed to `install` after
  detection. The record lives in JSI runtime data (`setRuntimeData` under a
  private UUID);
- `startWorker` builds the worker's config from that record through
  `makeWorkerRuntimeConfig(parent)`, which needs no Hermes internals: the
  same plugin (the same `IntlPluginPath`, or default resolution, which
  returns the plugin already resolved), and the resolved locale list and
  time zone set explicitly, so the worker never re-runs detection;
- it does not copy the `Intl` field, which is deprecated and ignored (see
  `RuntimeConfig`): a worker's `Intl` comes from the plugin it inherits.
  Before Phase 7 deleted the built-in implementation, the field was copied
  too (`IntlRecord::oldIntl`), so that a parent with that implementation
  disabled did not get a worker with it enabled;
- it also passes on the finalizer thread runner the parent was created
  with. The command-line tools set one with no JNI attachment
  (`cli::toolFinalizerThreadRunner`), since on Android the default
  attaches the finalizer thread through fbjni, which aborts in a process
  without a JVM; a Worker a tool creates keeps that runner. A parent with
  no runner gives its Workers none.

### `RuntimeConfig`

New fields, appended after the last existing field so that
`HERMES_RUNTIME_CONFIG_VERSION` does not change
(`public/hermes/Public/CtorConfig.h`):

- `EnableIntlPlugin` (default `true`);
- `IntlPluginPath` (optional);
- `IntlDefaultLocales` (optional list of BCP 47 tags);
- `IntlDefaultTimeZone` (optional IANA name).

The existing `Intl` field (`public/hermes/Public/RuntimeConfig.h`)
controlled the built-in implementation that Phase 7 deleted. It is
deprecated and ignored, and stays where it is, because removing a field
from the middle of `RuntimeConfig` would change its layout.
`withIntl(true)` and `withIntl(false)` are accepted and change nothing,
with or without the plugin, for a runtime and for the Workers it creates;
`SynthTrace` still writes and parses the field. The command-line flag
`-Xintl` stays accepted, described as deprecated and ignored, because
scripts pass it. `IntlLoaderTest.IntlFieldIsIgnored` checks the field
both ways, with the plugin enabled and disabled, for a parent runtime
and a Worker made from it; `intl/test/lit/xintl-ignored.js` and
`test/hermes/no-intl-fallbacks.js` check `-Xintl` and `-Xintl=false`.

### Without the plugin

Hermes has no other `Intl`. Phase 7 deleted the built-in implementation
(`lib/VM/JSLib/Intl.cpp` and its platform backends), so:

- With no plugin, a runtime has no `Intl` global (`typeof Intl` is
  `"undefined"`), and the locale-sensitive built-ins keep Hermes's own
  fallbacks, none of which throws for an ordinary value:
  - `Number.prototype.toLocaleString` and `BigInt.prototype.toLocaleString`
    give `toString`'s result (`"1234.5"`, `"Infinity"`, `"NaN"`, `"12"`);
  - `Array.prototype.toLocaleString` and
    `%TypedArray%.prototype.toLocaleString` are ECMA-262's: each element's
    `toLocaleString` is called with no arguments;
  - `Date.prototype.toLocaleString`, `toLocaleDateString` and
    `toLocaleTimeString` format with Hermes's own platform code
    (`platform_unicode`), and give `"Invalid Date"` for a time value that
    is not finite;
  - `String.prototype.toLocaleLowerCase` and `toLocaleUpperCase` map case
    for the default locale (`convertCase`): with a Turkish default locale
    `"I".toLocaleLowerCase()` is U+0131 and `"i".toLocaleUpperCase()` is
    U+0130;
  - `String.prototype.localeCompare` uses `platform_unicode::localeCompare`.

  `test/hermes/no-intl-fallbacks.js` checks each of these with
  `-Xintl-plugin=false`, and `no-intl-fallbacks-tr.js` the Turkish case
  mappings (it runs with Hermes's own and the CoreFoundation Unicode
  backends).
  Embedders that need `Intl` must ship the plugin.
- With the plugin, the plugin replaces `globalThis.Intl` and all of these
  built-ins, whatever the `Intl` field says.
- The extensions run before the plugin. None of them captures `Intl` today
  (`API/hermes/extensions/Intrinsics.cpp`); any future extension that needs
  `Intl` must look it up when it runs, not at install.

## The JS layer

### Files

Concatenated and compiled to bytecode, as the extensions are:

- shared abstract operations: `CanonicalizeLocaleList`, `GetOption`,
  `GetNumberOption`, `GetBooleanOrStringNumberFormatOption`,
  `SetNumberFormatDigitOptions`, `ResolveLocale`, `LookupMatcher`,
  `FilterLocales` (ECMA-402 9.2.9; the draft renamed `SupportedLocales` and
  `LookupSupportedLocales` to it), Unicode extension parsing and insertion;
- `ResolveOptions` (9.2.8), in `03-locales.js` so that every constructor
  shares it. It takes the service constructor as a JS record,
  `{__proto__: null, availableLocales, relevantExtensionKeys,
  resolutionOptionDescriptors}`, and returns the resolved options, the
  resolved locale and the resolution options;
- `06-iterators.js`: ECMA-262's iterator protocol, which ListFormat's
  StringListFromIterable uses, because `for-of` consults user-modifiable
  state. GetIterator (sync) gets the method through the captured
  `Symbol.iterator`, requires it to be callable and its result to be an
  Object, and reads `next` once. IteratorStepValue calls `next`, requires
  an Object, and reads `done` and then `value`; none of these errors
  closes the iterator. IteratorCloseOnThrow reads `return` and calls it
  unless it is undefined or null, ignores any error from either step and
  the result, and throws the original error. `01-intrinsics.js` captures
  `Symbol.iterator` and %Iterator.prototype% (the prototype of the
  prototype of `[][Symbol.iterator]()`), which
  %IntlSegmentIteratorPrototype% inherits from;
- `07-number.js`: Intl mathematical values, ToIntlMathematicalValue,
  FormatNumericToString with ToRawPrecision and ToRawFixed, and
  ComputeExponent, which NumberFormat, PluralRules and the later phases
  share (see "Number formatting");
- one file per constructor (`20-Locale.js`, `30-Collator.js`,
  `40-NumberFormat.js`, `45-PluralRules.js`, `50-DateTimeFormat.js`,
  `60-ListFormat.js`, `62-RelativeTimeFormat.js`, `64-DisplayNames.js`,
  `66-DurationFormat.js`, `68-Segmenter.js`).
  `41-number-patterns.js` holds the rest of NumberFormat:
  PartitionNumberPattern and the operations it uses, the range operations,
  the digits of the numbering systems, grouping, and the caches of the
  native number data;
- DateTimeFormat takes five files. `50-DateTimeFormat.js` has the
  constructor with CreateDateTimeFormat and ChainDateTimeFormat, the time
  zone identifier operations (IsTimeZoneOffsetString,
  ParseTimeZoneOffsetString, FormatOffsetTimeZoneIdentifier,
  GetAvailableNamedTimeZoneIdentifier), `[[LocaleData]]`,
  `supportedLocalesOf`, `resolvedOptions` and the prototype methods.
  `51-datetime-patterns.js` turns CLDR patterns into DateTime Format
  Records and holds `[[formats]]`, `[[styles]]`, BasicFormatMatcher, the
  best-fit matcher and DateTimeStyleFormat. `52-datetime-format.js` has
  ToLocalTime and the formatting operations, from FormatDateTimePattern to
  FormatDateTimeRangeToParts. `53-datetime-ranges.js` builds each
  record's range patterns, and `54-datetime-names.js` chooses the CLDR
  name symbols native code is asked for and the flexible day period of a
  time (see "Date and time formatting"). `20-Locale.js` splits
  CalendarsOfLocale and HourCyclesOfLocale into functions of a tag, which
  DateTimeFormat's `[[LocaleData]]` shares;
- one file for the overridden built-ins, `80-builtins.js`. Each phase adds
  its overrides to it. Phase 2 put `localeCompare`, `toLocaleLowerCase`,
  `toLocaleUpperCase` and TransformCase there, Phase 3 the
  `toLocaleString` methods of Number, BigInt, Array and TypedArray, and
  Phase 4 the `toLocaleString`, `toLocaleDateString` and
  `toLocaleTimeString` methods of Date (see "Built-in overrides").

Code is commented step by step against the spec. Setup receives an object of
native helper functions plus the config; the native helpers are never
reachable from user code.

The draft's String overrides begin with "Let thisValue be ?
RequireObjectCoercible(this value)". ECMA-262's RequireObjectCoercible now
returns unused instead of its argument, so the code checks the this value
and then uses it.

### Constructors

- Every public constructor is a plain `function`, so it can be called with or
  without `new` as its spec requires. Constructors that require `new`
  (`Locale`, `PluralRules`, and so on) throw a `TypeError` when `new.target` is
  `undefined`. `Collator`, `NumberFormat` and `DateTimeFormat` are callable
  without `new`.
- Called with `new`, the function uses the `this` Hermes created from
  `new.target`. Called without `new`, it creates the object itself from the
  intrinsic prototype.
- Internal slots are private fields. They are attached to the object with a
  per-constructor helper class that extends a base class whose constructor
  returns the object passed to it, so the subclass's private fields land on
  that object. Phase 0 includes a test that Hermes supports this.
- Brand checks are `#field in obj`.

The five constructors of Phase 5, `ListFormat`, `RelativeTimeFormat`,
`DisplayNames`, `DurationFormat` and `Segmenter`, require `new` and have
no legacy behavior. Each has `length` 0, except `DisplayNames`, which has
2 (`Intl.DisplayNames ( locales, options )`). Their methods are plain
methods; none has a getter that returns a bound function. As the draft's
ResolveOptions calls say, ResolveOptions converts their options with
CoerceOptionsToObject for RelativeTimeFormat and with GetOptionsObject for
the other four; DisplayNames also passes `require-options`, so `undefined`
options throw a `TypeError` first. Only
RelativeTimeFormat and DurationFormat have a relevant extension key,
`nu`, with NumberFormat's `[[LocaleData]]`.

**Documented deviation.** For a plain function called with `new`, Hermes
creates `this` before the constructor runs (`createThisForConstruct_RJS`,
`lib/VM/Callable.cpp`). When `new.target.prototype` is not an object, it
falls back to `Object.prototype`; ECMA-402 requires the constructor's own
intrinsic prototype (for example `%Intl.NumberFormat.prototype%`). Only
subclassing with a non-object `prototype` can observe this. It can be fixed
later with a Hermes primitive that lets a JS function create its own `this`.
This applies to `Intl.Collator` as well, and `intl/test/lit/collator-objects.js`
pins it; so does the `-objects.js` test of each Phase 5 constructor.

### Legacy constructor behavior

`Intl.NumberFormat` and `Intl.DateTimeFormat`, called without `new` with a
`this` that is an instance of the constructor (`OrdinaryHasInstance`), apply
`ChainNumberFormat` / `ChainDateTimeFormat`: they define the
`%Intl%.[[FallbackSymbol]]` property on `this` (non-writable, non-enumerable,
non-configurable, value = the new object) and return `this`. The fallback
symbol is one private `Symbol("IntlLegacyConstructedSymbol")` created at
setup. `OrdinaryHasInstance` is written in JS over the captured
`Object.getPrototypeOf`, so replacing `Object.getPrototypeOf` cannot change
it. A real NumberFormat passed as `this` is an instance too, so it is
chained, as in V8.

Methods that accept such a wrapper go through `UnwrapNumberFormat` /
`UnwrapDateTimeFormat` first: the `format` getter and `resolvedOptions` of
each. Every other method does a direct brand check and rejects the wrapper:
`formatToParts`, and for DateTimeFormat `formatRange` and
`formatRangeToParts`. Phase 3 implemented the NumberFormat half and Phase 4
the DateTimeFormat half, with the same fallback symbol and
`OrdinaryHasInstance`. `intl/test/lit/number-format-objects.js` and
`datetime-format-objects.js` test both paths.

### Function shapes

| Kind | How it is made | Shape |
|---|---|---|
| Constructor | plain `function` | `prototype` redefined non-writable, non-enumerable, non-configurable; `length` set to the spec value |
| Prototype and static methods (`format...`, `resolvedOptions`, `supportedLocalesOf`, ...) | concise methods in an object literal, copied over with their descriptors | not constructible, no `prototype`, spec `name` and `length`; installed writable, non-enumerable, configurable |
| Accessors (`get format`, `get compare`, `Intl.Locale` getters) | `get` in an object literal | not constructible, getter `name` is `"get format"` etc.; installed non-enumerable, configurable |
| Bound `format` / `compare` returned by the getters | an arrow function returned from a factory, created once and cached in a private field | not constructible, `name` `""`, spec `length` |
| Built-in overrides | concise methods | as prototype methods, with the overridden method's `name` and `length` |
| `@@toStringTag` on `Intl` and each prototype | data property | non-writable, non-enumerable, configurable |

Hermes used to make object-literal getters and setters constructible, which
ECMA-262 does not allow (they are methods). The compiler now makes them
non-constructible, and the getter shape above relies on that.

Phase 0 adds a lit test that checks every installed function against this
table, so a wrong shape fails in one place instead of being found piecemeal in
test262.

### Native state

When a constructor that needs an ICU4X object has resolved its options, it
calls a native helper such as `native.collatorInit(obj, ...resolvedFields)`.
The helper builds the Rust object and attaches it to the instance with
`setNativeState`. Methods pass
the instance to native helpers, which fetch the formatter with
`hasNativeState` and `getNativeState` and check its type with
`std::dynamic_pointer_cast`. Hermes asserts that native state is present
when `getNativeState` is called, so the check comes first, and the typed
`hasNativeState<T>` would add a third lookup. Native state holds only Rust
and C++ data, never JS values.

`Intl.Collator` is the first constructor to do this. It calls
`native.collatorInit(obj, ...)`, which attaches a `CollatorState` that owns
the Rust collator. `native.collatorCompare(obj, x, y)` looks the state up
and compares. `Intl.PluralRules` calls `native.pluralRulesInit(obj, ...)`,
which attaches a `PluralRulesState` (see "Plural rules").

`Intl.NumberFormat` attaches no native state. Formatting is JS (see "Number
formatting"), so a NumberFormat keeps only its slots and the lists of
strings native code gave its constructor (see "Data"). It takes plural
categories from an internal plural-rules object: a null-prototype object
with a `PluralRulesState` for the language identifier of `[[Locale]]`,
which user code never reaches. These objects are cached per language
identifier and type (see "Resistance to tampering"); a formatter does not
hold one, so one that leaves the cache is freed by the GC with its state.

`Intl.DateTimeFormat` formats in JS too, but it needs ICU4X's calendars and
names at each instant. The last step of its constructor calls
`native.dateTimeFormatInit(obj, languageId, calendar, timeZone, symbols)`,
which attaches a `DateTimeFormatState` (`DateTimeFormatHelpers.cpp`) that
owns a `HiDateTimeFormat`: the ICU4X calendar, the time zone (its parsed
tzdb rules or a fixed offset, and the zone's ICU4X id) and the ICU4X names
of the CLDR symbols the formatter's patterns use, such as `MMMM` or `zzzz`
(see "Date and time formatting"). The slots are attached after that, so a
constructor that throws leaves neither. `native.dateTimeFormatLocalTime(obj,
epochMs)` gives the local time of an instant and the text of those names.

Phase 5 adds three native states:

- `Intl.ListFormat`'s constructor ends with `native.listFormatInit(lf,
  languageId, type, style)`, which attaches a `ListFormatState`
  (`ListFormatHelpers.cpp`) that owns a `HiListFormat`: ICU4X's
  `ListFormatter` for the type and style (see "Data", "ListFormat");
- `Intl.Segmenter`'s constructor ends with `native.segmenterInit(segmenter,
  languageId, granularity)`, which attaches a `SegmenterState` that owns a
  `HiSegmenter`, ICU4X's segmenter for the granularity;
- a Segments object gets a `SegmentsState` (`SegmenterHelpers.cpp`, owning
  a `HiSegments`) from `native.segmentsInit(segments, segmenter, string)`
  on the first call of its `containing`, or of `next` on one of its
  iterators, that gets past the draft's early returns: every boundary of
  its string, computed at once (see "Data", "Segmenter"). An iterator
  keeps its Segments object in a private field and reads that object's
  state, so the state lives as long as either.

In each, the slots are attached before the state. A helper given an
object without its state, or one that already has state, throws an
internal error. `Intl.RelativeTimeFormat`, `Intl.DisplayNames` and
`Intl.DurationFormat` attach no native state.

### Resistance to tampering

Every intrinsic the JS layer calls is captured at setup: `Object` property
functions, the array functions it needs, `String`, `Number`,
`Symbol.iterator`, `Reflect` functions, and so on. Calls go through the
captured functions.

Internal cooperation never goes through public properties. The built-in
overrides and any constructor that uses another (DurationFormat using
NumberFormat and ListFormat, for example) call internal functions directly:
`Number.prototype.toLocaleString(locales, options)` runs the internal
NumberFormat creation and formatting functions, not
`new Intl.NumberFormat(...).format(x)`, so replacing
`Intl.NumberFormat.prototype.format` cannot change it. Dynamic property
lookups remain only where the spec itself requires them, such as
`Array.prototype.toLocaleString` calling each element's `toLocaleString`.

When both `locales` and `options` are `undefined`, the overrides reuse one
cached default-locale formatter. With other arguments, `localeCompare`
still performs every observable step of the `Intl.Collator` constructor
(CanonicalizeLocaleList, each `Get` of an option and its conversion, in
the draft's order, and each error they raise). It keys a cache on the
values those steps read: the canonical requested locales and each option
value. The rest of the constructor, locale resolution included, depends
only on those values and `DefaultLocale()`, so a hit skips it and reuses
the collator made for the same values. ECMA-402 never exposes such a
collator to user code. The shared collators are kept in a JS object with a
null prototype, at most 32 of them; the object is emptied when it is full.
The same kind of object keeps the locale data of each locale
`Intl.Collator` resolves to (one entry per available locale) and the
canonical form of up to 64 language tags that CanonicalizeLocaleList has
canonicalized.

NumberFormat and PluralRules cache in the same way, and nothing they cache
is observable:

- each kind of native number data request (`numberSymbols`,
  `currencyPatterns`, `unitPatterns`, `compactPatterns`, and the
  `[[LocaleData]]` record of a locale) is cached in a null-prototype JS
  object keyed by its arguments, 64 entries per kind, emptied when full;
- the internal plural-rules objects of NumberFormat are cached per
  language identifier and type, 32 entries, emptied when full;
- each NumberFormat keeps the parts of every sign pattern, notation
  sub-pattern and plural-dependent pattern it has used (PartitionPattern
  and the split around `{0}`), parsed on first use; the patterns come from
  its own data, so there are few of them, and the parts are never changed;
- `Number.prototype.toLocaleString` and `BigInt.prototype.toLocaleString`
  share one default NumberFormat, created on first use, when `locales` and
  `options` are both `undefined`. Any other call creates a NumberFormat
  through the internal creation function, which performs every observable
  step of the constructor. Unlike `localeCompare`, they keep no cache keyed
  on the option values; the data the new formatter needs comes from the
  caches above.

DateTimeFormat caches the same way, and nothing it caches is observable
either:

- the `dateTimePatterns` lists, per language identifier and calendar, the
  `dayPeriods` lists, per language identifier, and the `[[LocaleData]]`
  record of a locale are each cached in a null-prototype object, 64
  entries per kind, emptied when full;
- the DateTime Format Records made from a locale's patterns are cached
  by language identifier, calendar and decimal separator, 32 entries. The
  separator is part of the key because a fraction of a second is written
  with [[NumberingSystem]]'s (`de` `"9,045"`). An entry holds both
  hour-cycle families. Each family's `[[styles]]` is made with the entry;
  its `[[formats]]`, its glued records and the range patterns of its
  records are made on first use;
- each such entry also caches the records the matchers return, keyed by
  the matcher, the hour-cycle family and each requested field's width, 32
  entries, emptied when full. A DateTimeFormat made for the same fields
  shares the record and its range patterns, which are never changed once
  made, so the best fit and its range patterns run once per request
  rather than once per constructor (about 4 ms each in the ASan build);
- each DateTimeFormat keeps the parsed parts of every pattern it has used,
  and the internal NumberFormats it formats numbers with, made on first
  use (see "Date and time formatting");
- `Date.prototype.toLocaleString`, `toLocaleDateString` and
  `toLocaleTimeString` each reuse one default DateTimeFormat, made on
  first use, when `locales` and `options` are both `undefined`. The three
  are different formatters, because each method passes its own required
  and default fields to CreateDateTimeFormat. Any other call creates a
  DateTimeFormat through the internal creation function, which performs
  every observable step of the constructor.

The constructors of Phase 5 cooperate through four internal functions,
which the public constructors and methods also use:
`CreateListFormat(locales, options)` and `CreatePartsFromList(listFormat,
list)` (`60-ListFormat.js`), `CreatePluralRules(locales, options)`
(`45-PluralRules.js` splits its constructor into
`initializePluralRules(pluralRules, locales, options)` and this function,
as `40-NumberFormat.js` does) and `CreateNumberFormat`. RelativeTimeFormat
makes its `[[NumberFormat]]` and `[[PluralRules]]` with them, and
DurationFormat its NumberFormats and its ListFormat, from null-prototype
option records, so neither ever reads `Intl.ListFormat`,
`Intl.NumberFormat`, `Intl.PluralRules` or their prototypes. They cache
in the same way, and nothing they cache is observable:

- RelativeTimeFormat's parsed `relativeTimePatterns` record, per language
  identifier and style, and DisplayNames's parsed `localeDisplayPatterns`,
  per language identifier, are each cached in a null-prototype object, 64
  entries per kind, emptied when full;
- each DurationFormat keeps the internal NumberFormats it makes, keyed by
  the options that made them, and its one ListFormat, each made on first
  use.

### Locale matching

`"best fit"` behaves as `"lookup"`, which the spec permits. SpiderMonkey and
magrinj do the same.

### Number formatting

ICU4X 2.3's number formatters do not fit ECMA-402. They round on their
own, produce few parts, and have no scientific notation, ranges, NaN or
infinity. So the plugin uses only ICU4X's data for numbers, and formatting
is JS, written step by step from the draft: FormatNumericToString,
ToRawPrecision, ToRawFixed, ComputeExponent, PartitionNumberPattern,
PartitionNotationSubPattern, GetNumberFormatPattern, the range operations
and ResolvePlural. Native code returns symbols and patterns as lists of
strings and selects plural categories (see "The native interface" and
"Data").

**Intl mathematical values.** An Intl mathematical value is one of the
strings `'nan'`, `'posInf'`, `'negInf'` and `'negZero'`, or a record
`{__proto__: null, neg, n, e}` for (-1)^neg × n × 10^e, with `n` a
non-negative BigInt and `e` an integer Number. The record is normalized:
`n` has no trailing decimal zero (`e` absorbs them), and zero is
`{neg: false, n: 0n, e: 0}`.

- ToIntlMathematicalValue follows the draft. Its ToPrimitive with hint
  number is written in JS from ECMA-262 (`@@toPrimitive`, then
  OrdinaryToPrimitive), because no intrinsic exposes it. A BigInt gives its
  value. A Number gives `'negZero'` for -0; otherwise its string is
  `Number::toString(x, 10)`, which is `` `${x}` ``, and no user code runs on
  a primitive.
- A string is parsed with ECMA-262's StringNumericLiteral grammar
  (StrWhiteSpace, StrDecimalLiteral with an optional sign, `Infinity`, and
  NonDecimalIntegerLiteral through the captured `BigInt`). Anything else is
  `'nan'`.
- RoundMVResult is `ToNumber` of the same string, which uses the same
  grammar and the same rounding. So a string whose value `+str` gives as
  ±Infinity is `'posInf'` or `'negInf'`, and one that `+str` gives as ±0
  is 0 or `'negZero'`, as the draft says. This includes a non-decimal
  literal beyond Number's range (300 hex digits), which V8 keeps exact.
  A non-decimal literal is checked digit by digit first, then its
  `ToNumber`: one beyond Number's range is `'posInf'` without its BigInt
  being made, since one with more digits than a BigInt holds (17,000 hex
  digits) would throw. A finite one is below 2^1024, so the captured
  `BigInt` converts it exactly whatever its leading zeros.

**Rounding** is exact. ToRawFixed rounds x to multiples of
roundingIncrement × 10^-f, and ToRawPrecision to 10^(e-p+1). A value that
is already a multiple needs no arithmetic. Otherwise the digits of x more
than one place below the unit are replaced by one sticky digit 1, which no
rounding mode can tell from them, and the low 46 digits are rounded by one
BigInt division with remainder; the digits above them take only a carry.
ApplyUnsignedRoundingMode compares twice the remainder with the divisor,
and half-even uses the parity of the quotient (the draft's cardinality).
The rounding works on decimal digits because Hermes limits a BigInt to
1024 64-bit digits, and its addition, multiplication and division reserve
one or two digits more than their operands: they fail on a BigInt near the
limit even for `x + 1n` or `x * 1n`.

A decimal string keeps its digits down to 110 places below its leading
digit or below 10^0, whichever is lower, and one sticky digit for the
rest. Every rounding FormatNumericToString can do (21 significant digits,
or 100 fraction digits after the notation's and the percent style's
scaling) is above that, so a string of any length formats exactly, where a
BigInt of all its digits would exceed the limit.

The results are the draft's records, with `[[RoundedNumber]]` an Intl
mathematical value and `[[FormattedString]]` ASCII digits with `"."`, so
`maximumFractionDigits: 100`, `2n ** 1000n` and the largest BigInt Hermes
allows format exactly, and nothing but plain decimal strings ever reaches
Rust. The one result a record cannot hold is a rounded value whose digits
without trailing zeros exceed the largest BigInt: a BigInt above about
a tenth of the largest one, rounded with a `roundingIncrement` above 1 that leaves
fraction digits or rounds up past the largest BigInt, throws Hermes's
`RangeError` ("Maximum BigInt size exceeded").

**Documented deviation: ComputeExponent.** Its trial rounding keeps the
value's sign: the magnitude comes from the absolute value, and the
rounding applies to the signed, scaled value. Read literally, the draft's
step 2 rounds -950 with `roundingMode: "ceil"` in scientific notation to
a mantissa of -0 and gives `"-0E3"`; the plugin gives `"-9E2"`, as V8 and
ICU4C do.

**Digits.** The digits of every numbering system come from ECMA-402's
table of numbering systems with simple digit mappings, in JS; ICU4X's
digits are never used. Groups are found in the ASCII digits and each group
is then transliterated, the reverse of the draft's order, which gives the
same parts and never splits a supplementary digit (`mathbold`) in two. The
exponent's digits are transliterated like the number's, where the draft
leaves them ASCII; ICU4C does the same.

### Date and time formatting

ICU4X 2.3's date formatters do not fit ECMA-402 either. They take field
sets, not option bags (ICU4X's field-bag support is unstable, on its main
branch for 2.4), and have no `h24` hour cycle, no flexible day periods and
no ranges; its data has no CLDR `availableFormats` or `intervalFormats`.
So formatting is JS, written step by step from the draft, as
NumberFormat's is: FormatDateTimePattern, PartitionDateTimePattern,
PartitionDateTimeRangePattern, FormatDateTime, FormatDateTimeToParts,
FormatDateTimeRange, FormatDateTimeRangeToParts, BasicFormatMatcher, the
best-fit matcher, which the draft leaves to the implementation, and
DateTimeStyleFormat. `h24` needs nothing of its own: it is the draft's own
step in FormatDateTimePattern, which shows hour 0 as 24. Native code
supplies only what needs data or calendar arithmetic:

- the CLDR patterns of a locale and calendar (`dateTimePatterns`) and the
  locale's flexible day periods (`dayPeriods`), as lists of strings, from
  files generated from CLDR;
- for an instant, ToLocalTime's record in the formatter's calendar and
  time zone, and the text of the names the formatter's patterns show
  (`dateTimeFormatLocalTime`): the calendar fields and names are ICU4X's,
  and the UTC offset comes from the bundled tzdb.

JS turns the CLDR patterns into the draft's DateTime Format Records,
matches the options against them, builds the range patterns and chooses
the day period. "Data" ("DateTimeFormat") describes the data and those
rules. This changes two of the research's conclusions: option-bag
matching and `h24` are JS, not gaps the Rust core fills, and DateTimeFormat
builds its parts in JS, as NumberFormat does, so it needs no native
`...ToParts` helper (see "Parts").

**Instants.** The draft passes an instant to ToLocalTime and
FormatDateTimePattern as epoch nanoseconds, a BigInt. The plugin passes
the time value TimeClip made, a Number of milliseconds, which holds the
same instant, so no BigInt is made. Native code receives it through the
`Double` conversion tag. Every time value works, from -8.64 × 10^15 to
8.64 × 10^15 (years -271,821 to 275,760), in every calendar.

**Numbers.** FormatDateTimePattern's `nf`, `nf2` and `nf3` are internal
NumberFormats made with Phase 3's internal creation function from
null-prototype option records, once per DateTimeFormat on first use, so
user code never sees them, and every numbering system NumberFormat
supports works in dates (`ja-u-ca-japanese-nu-arab`). A 2-digit value
keeps its last two code points, as the draft says, so a supplementary
digit is never split.

**Comparing fields in ranges.** PartitionDateTimeRangePattern compares two
of the draft's fields more finely, because their values repeat where the
dates differ: [[Month]] is compared by month code, so a leap month and its
base month differ (`zh-u-ca-chinese` 2020-05-01 to 2020-05-31, months M04
and M04L, is `"4-9至闰4-9"`, where ICU4C gives `"4-9"`), and the [[Year]]
of `chinese` and `dangi` is compared by [[RelatedYear]], because the
cyclic year repeats every 60 years (1984-02-02 and 2044-01-30 both begin a
甲子 year: `en-u-ca-chinese` `{year: "numeric"}` gives
`"1984(jia-zi) – 2044(jia-zi)"`, as ICU4C does).

### Built-in overrides

The `toLocaleString` methods of Number and BigInt are the draft's: they
take the this value with ThisNumberValue or ThisBigIntValue, through the
captured `valueOf` methods, and format it with a NumberFormat made by the
internal creation function, or the default one (see "Resistance to
tampering"). Array and TypedArray `toLocaleString` are ECMA-262's
JoinIndexedProperties, with each element's `toLocaleString` looked up and
called as the spec says. Their list separator is `","`, as Hermes's own
`Array.prototype.toLocaleString` uses. The TypedArray method validates its
this value and reads its length through the captured `%TypedArray%`
methods, never through a property of the object.

**Documented deviation.** `Array.prototype.toLocaleString` keeps one
behavior of Hermes's built-in that the spec lacks: an array this method
is already joining gives `""`. The spec's steps would recurse on a cyclic
array until the stack overflows. The check covers only the arrays this
method is joining: Hermes's built-in shares one set of visited objects
with `join`, which JS cannot reach, so a cycle through both methods is
cut one level later. With `a = [1]` and
`a.push({toString() { return a.toLocaleString(); }})`, `a.join()` is
`"1,1,"` here and `"1,"` in Hermes without the plugin and in V8. The
recursion still ends. The array is unmarked when the join returns or
throws.

A length above 2^32 - 1 is joined as the spec says, where Hermes's
built-in throws a `RangeError` before reading an element: the elements are
read from index 0 (an object of length 2^32 whose element 0 throws
`"visited"` throws `"visited"`). Each element adds at least the separator,
so Hermes's string length limit (2^28) throws a `RangeError` within 2^28
elements; with no throwing element that takes about 4 microseconds an
element in the ASan build, some 18 minutes.

`test/hermes/number-functions.js`,
`test/hermes/bigint-prototype-tolocalestring.js` and
`test/hermes/regress-array-tolocalestring-tolength.js` test Hermes's own
`toLocaleString`, so they run with the plugin off
(`-Xintl-plugin=false`); `intl/test/lit/to-locale-string.js` tests the
overrides.

Date's `toLocaleString`, `toLocaleDateString` and `toLocaleTimeString`
are the draft's. They read the this value's [[DateValue]] through the
captured `Date.prototype.getTime`, which is exactly RequireInternalSlot
and the read of the slot (ECMA-262 21.4.4.10), so `{}`, `Date.prototype`
and a Proxy of a Date throw a `TypeError`, which names the method called
(`"Date.prototype.toLocaleString called on non-Date object"`, in place of
getTime's own message), and a Date's own `valueOf`
and `@@toPrimitive` are never called. A NaN time value gives
`"Invalid Date"` before any argument is read. Then each calls
CreateDateTimeFormat with its required and default fields, (`any`,
`all`), (`date`, `date`) and (`time`, `time`), and formats with
FormatDateTime: `toLocaleDateString("en", {hour: "numeric"})` is
`"1/5/2026, 3 PM"`, and `toLocaleTimeString("en", {dateStyle: "short"})`
throws a `TypeError`. `test/hermes/date-format-fixed.js` and
`test/hermes/date-locale.js` test Hermes's own Date methods, so they run
with the plugin off too; `intl/test/lit/to-locale-date-string.js` tests
the overrides, and checks that they give what `Intl.DateTimeFormat` gives
for the same arguments.

## The native interface

### Rust core (`intl/core`)

- One crate depending on `icu` 2.x, `icu_plurals` and `icu_decimal` with
  their `unstable` feature (plural ranges, exact plural operands, compact
  decimal data), `icu_experimental` (currency, relative time and display
  name data), `fixed_decimal`, `writeable` (which writes ICU4X's
  date names and list literals), `potential_utf` (the key type of
  `icu_experimental`'s dialect names), and `jiff-tzdb` for the time zone
  rules (see "Time zones" under Data). `icu`'s list formatter, segmenters
  and Unicode properties serve ListFormat and Segmenter.
- It fills the ICU4X gaps found during research that need data or
  calendar arithmetic: UTC offsets at an instant, from its own reader of
  the bundled tzdb (`tz.rs`); the CLDR date patterns and flexible day
  periods ICU4X 2.3 lacks (`date_data.rs`); and, for an instant, the
  calendar fields and names DateTimeFormat shows (`datetime.rs`). Phase 5
  adds `list.rs` (the literal text of ICU4X's list formatter),
  `relativetime.rs` (relative time patterns), `displaynames.rs` (single
  display names, with the calendar, field and code-equal names ICU4X
  lacks) and `segmenter.rs` (the boundaries of a string, with workarounds
  for three ICU4X 2.3 word segmentation bugs); see "Data".

  The research also listed DateTimeFormat's option-bag matching (until
  ICU4X's field-bag support, unstable on ICU4X main for 2.4, can be used)
  and the `h24` hour cycle here. Phase 4 does both in JS instead (see
  "Date and time formatting"), as NumberFormat's gaps (scientific and
  engineering notation, ranges, NaN and infinity, the parts) are filled
  in JS from ICU4X's data (see "Number formatting").
- Interface: opaque pointers (`HiCollator *`, `HiPluralRules *`,
  `HiDateTimeFormat *`, `HiListFormat *`, `HiSegmenter *`, `HiSegments *`)
  with create, destroy and per-operation functions.
  A data lookup that gives a list of strings returns them in one
  `HiString`, separated by U+0000.
- The C header is generated by cbindgen.
- Errors from data or construction come back as status codes. Every
  profile uses `panic = "abort"`; panics are not caught at the boundary.
  Interface functions never panic on bad input: invalid UTF-8 or an
  unparsable tag is reported as a failure. JS validates every argument
  before calling native code, so a panic is a plugin bug, such as a broken
  invariant in the crate's own data files.

### Threads

A runtime runs on one thread at a time, but Hermes destroys `NativeState`
objects on a separate finalizer thread (`finalizerExecutor_` in
`API/hermes/hermes.cpp`; documented at `FinalizerThreadRunner` in
`public/hermes/Public/RuntimeConfig.h`). Therefore:

- every Rust type held in a `NativeState` must be `Send`, enforced by a
  compile-time assertion in the crate (`HiCollator`, `HiPluralRules`,
  `HiDateTimeFormat`, `HiListFormat`, `HiSegmenter` and `HiSegments` each
  have one; ICU4X's `sync` feature is enabled);
- a `NativeState` destructor frees only its own Rust object and touches no
  per-runtime cache;
- per-runtime caches (such as the default formatters for the built-in
  overrides) live in JS, not in native code;
- the CLDR files the crate compiles in (see "Data") are parsed once per
  process into immutable tables, behind `std::sync::OnceLock`, and so is
  each tzdb zone, on first use; a `HiDateTimeFormat` refers to its zone in
  that table;
- process-wide mutable native state is limited to the loader's cache, which is
  locked.

### Strings

Two kinds of string cross the boundary, handled differently:

- **Option strings and formatter output** (locale tags, option values,
  formatted numbers and dates): UTF-8 both ways (`String::utf8`,
  `createFromUtf8`). ICU4X writes UTF-8.
- **Arbitrary user text** (Collator and `localeCompare` inputs, case mapping
  input, a ListFormat list's last element, Segmenter input): copied out as
  UTF-16 with `String::utf16` and never converted to UTF-8 in a way that
  could lose unpaired surrogates from a result. Where native code reads
  such text as UTF-8 or as characters, an unpaired surrogate is U+FFFD,
  and no text comes back from that reading.
  - Collator uses ICU4X's UTF-16 comparison (`compare_utf16`), which reads
    an unpaired surrogate as U+FFFD. Two different unpaired surrogates
    therefore compare equal, and `compare(x, x)` is 0.
  - Segmenter uses ICU4X's UTF-16 segmentation (`segment_utf16`) for
    graphemes and sentences, and all indices are UTF-16 code-unit indices
    into the original string. Word segmentation runs on UTF-8 instead,
    because ICU4X 2.3's UTF-16 word path truncates characters above
    U+FFFF in Chinese and Japanese runs: the input is transcoded with
    each unpaired surrogate replaced by U+FFFD (both have Word_Break
    Other, so no boundary moves, and both are one code unit), and the
    boundaries are mapped back to UTF-16 indices (see "Data",
    "Segmenter").
  - Case mapping splits the input at unpaired surrogates, maps each
    well-formed run, and passes unpaired surrogates through unchanged. Each
    run is mapped on its own, so a context rule such as the final sigma does
    not look across an unpaired surrogate. The result comes back as a
    `HiString16` (a UTF-16 buffer that C++ copies and then frees with
    `hi_string16_free`).
  - ListFormat sends native code only the last element of a list, because
    CLDR's list patterns depend on it (Spanish `y` becomes `e` before a
    word starting with an "i" sound; Spanish disjunctions and Hebrew
    conjunctions have similar rules), and ICU4X's conditional patterns
    test nothing else. Native code reads an unpaired surrogate in it as
    U+FFFD, uses it only to choose the patterns, and returns only literal
    text. JS builds the result from the literal pieces and the original
    element strings, so the output is lossless (see "Data", "ListFormat").
  - Tests cover unpaired surrogates and supplementary characters for each of
    these.

Reading strings in place with `getStringData` is a later optimization. It
delivers ASCII and UTF-16 chunks, possibly in several callbacks, and forbids
any runtime call inside the callback, so two-string operations would still have
to copy the first string before reading the second.

### Parts

A native `...ToParts` helper returns the formatted string and a flat array of
integer records, four per part: `start`, `end`, `type`, `extra`. No helper
uses this format today: NumberFormat, DateTimeFormat and the constructors
of Phase 5 build their parts in JS, as the paragraphs below describe. It
stays the format for a later helper that needs one.

- `start` and `end` are UTF-16 code-unit offsets into the returned string.
- `type` indexes a fixed table of part type names shared by JS and native.
- `extra` carries the per-part attribute the spec defines for that service:
  - range formatting: the `source` (`shared`, `startRange`, `endRange`);
  - everything else: `0`.

The C++ layer copies the native result into JS values before returning and
frees the native buffer immediately; nothing native outlives the call. JS
builds the part objects.

NumberFormat needs none of this: JS builds its parts from the patterns and
symbols native code supplies (see "Number formatting"). Its patterns use
the draft's placeholders, except those that depend on the plural form (unit
patterns, currency name patterns and compact patterns), which keep CLDR's
`{0}` form. JS splits such a pattern around `{0}`:

- the whitespace next to `{0}` becomes a `literal` part, and the rest of
  each side one `unit`, `currency` or `compact` part (`"時速 {0} キロメートル"`
  gives unit `"時速"`, literal `" "`, the number, literal `" "`, unit
  `"キロメートル"`). Whitespace is ECMA-262 WhiteSpace and LineTerminator,
  tested by code unit;
- the bidi marks U+200E, U+200F and U+061C next to `{0}` are literal
  parts too, and so are the marks at the ends of a currency string or of a
  compact pattern's text, and those CLDR puts around a sign: the
  `minusSign`, `plusSign` and `percentSign` parts are the bare symbols, the
  exponent's minus sign included (he `-1` gives literal `"‎"`,
  minusSign `"-"`, integer `"1"`). These are ICU4C's parts;
- a percent pattern's text other than its percent sign is literal.

CLDR has patterns without `{0}`: `ar` long `meter` "one" is `"متر"`, and
`fr` long compact has the exact-1 pattern `"mille"`. The draft's
locale-data constraint wants `{number}` in every pattern. ICU4C uses these
patterns as they are, and so does the plugin, as one part of the pattern's
type and no number part, but only when the chosen sign pattern is the zero
pattern, that is, when no sign is shown. Otherwise it uses the category's
pattern without the exact value, and if that has no `{0}` either, the
"other" pattern, which always has one. ICU4C drops the sign instead (`-1`
gives `"متر"`); the plugin never hides a sign.

DateTimeFormat needs no native parts either: JS builds its parts, and
the range parts with their `source`, from the patterns, the local time
and the names native code supplies (see "Date and time formatting").

No constructor of Phase 5 has a native `...ToParts` helper either.
ListFormat's parts are the elements and the literal pieces native code
returns. RelativeTimeFormat and DurationFormat build their parts, with
their `unit`, from NumberFormat's parts, as NumberFormat builds its own
(see "Data", "RelativeTimeFormat" and "DurationFormat").

### C++ JSI layer (`intl/jsi`)

- An X-macro table (`NativeHelpers.def`) lists every native helper with its
  argument and return types; the host functions are generated from it.
- `Intl.Locale` needs the helpers `addLikelySubtags`, `removeLikelySubtags`,
  `calendarPreferences`, `hourCycles`, `timeZonesOfRegion`, `weekInfo`,
  `scriptDirection`, `defaultNumberingSystem` and `collationsOfLocale`. The
  last two use the plugin's one `[[AvailableLocales]]` list
  (`isAvailableLocale`) where the spec names
  `%Intl.Collator%.[[AvailableLocales]]` and
  `%Intl.NumberFormat%.[[AvailableLocales]]`. Collator and NumberFormat
  use that list too; a change to either must update these two methods.
  They also use the same data: Phase 2's Collator takes
  `[[SortLocaleData]].[[co]]` from `collationsOfLocale`, and Phase 3's
  NumberFormat takes its default `[[nu]]` from `defaultNumberingSystem`,
  so `getCollations` and `getNumberingSystems` agree with the
  constructors.
- Each native state type is a `jsi::NativeState` subclass that owns its
  Rust pointer and frees it in its destructor: `CollatorState`
  (`CollatorHelpers.cpp`), `PluralRulesState`
  (`PluralRulesHelpers.cpp`), `DateTimeFormatState`
  (`DateTimeFormatHelpers.cpp`), `ListFormatState`
  (`ListFormatHelpers.cpp`), and `SegmenterState` and `SegmentsState`
  (`SegmenterHelpers.cpp`). The destructor may run on Hermes'
  finalizer thread, which is why every Rust type they own must be `Send`.
- The conversion tags are `Bool` (results and arguments), `Object`
  (arguments), `Utf8`, `Utf16`, `OptUtf8`, `Int`, `Double` (arguments),
  `Utf8List` (results) and `Undefined` (results). `Utf16` is lossless for
  unpaired surrogates. `Int` is an integer in `int` range both ways; as an
  argument, anything else (a fraction, NaN, a string) is an internal error
  naming the helper, and -0 is 0. `Double` is a finite number, such as a
  time value; anything else (NaN, an infinity, a string) is an internal
  error naming the helper. `Utf8List` is a new JS array of strings. It is
  built with
  `jsi::Value::createFromJsonUtf8` from JSON the plugin escapes itself,
  because `setValueAtIndex` would run setters on `Array.prototype`. Hermes
  implements `createFromJsonUtf8` with its internal JSON parser, not the
  global `JSON.parse` that JSI's default implementation calls; the plugin
  runs only on Hermes.
- The Collator and case-mapping helpers are `collatorLocaleDefaults`,
  `collatorInit`, `collatorCompare` and `caseMap`. `collatorLocaleDefaults`
  returns ICU4X's default `caseFirst` and `ignorePunctuation` for a locale,
  which is where the Collator's locale data gets them.
- The plural rules helpers are `pluralRulesInit`, `pluralSelect`,
  `pluralSelectRange` and `pluralCategories` (`PluralRulesHelpers.cpp`; see
  "Plural rules").
- The number data helpers are `numberSymbols`, `currencyPatterns`,
  `compactPatterns`, `unitPatterns` and `currencyDigits`
  (`NumberHelpers.cpp`). Each but the last returns a `Utf8List` that JS
  reads by index; `NativeHelpers.def` gives the layout of each list. JS
  validates every argument first: the locale is the language identifier of
  `[[Locale]]`, the numbering system is passed explicitly, and the currency
  code is uppercased. Their data is described under "Data".
- The DateTimeFormat helpers are `dateTimePatterns`, `dayPeriods`,
  `canonicalizeTimeZone`, `dateTimeFormatInit` and
  `dateTimeFormatLocalTime` (`DateTimeFormatHelpers.cpp`).
  `dateTimePatterns` and `dayPeriods` return `Utf8List`s of a language
  identifier's CLDR data, which JS reads by index, as it reads the number
  data. `canonicalizeTimeZone` gives the primary identifier of an IANA
  name, matched without regard to case, or `undefined`.
  `dateTimeFormatInit` attaches a formatter's `DateTimeFormatState` (see
  "Native state"), given its language identifier, calendar, time zone (a
  primary identifier, or an offset such as `"+05:30"`) and the CLDR name
  symbols its patterns need; a symbol the calendar does not have (`U`
  outside `chinese` and `dangi`, an era in `chinese`, `dangi` and
  `iso8601`) is an internal error. `dateTimeFormatLocalTime` takes the
  formatter and a time value (`Double`) and returns a `Utf8List`: the
  fields of ToLocalTime's record, the UTC offset in seconds, and the text
  of each symbol at that instant. A value that is not an integral time
  value within ±8.64 × 10^15, an object without that state, and any error
  ICU4X reports while formatting a name are internal errors, so ICU4X's
  fallback text is never shown.
- The Phase 5 helpers:
  - `listFormatInit` and `listFormatLiterals` (`ListFormatHelpers.cpp`).
    `listFormatLiterals(lf, isPair, last)` takes the last element as
    `Utf16` and returns a `Utf8List` of 3 literal pieces for a pair
    (before, between and after the elements) and 5 otherwise (before the
    first, after the first, between two middle elements, before the last,
    after the last); a piece may be empty;
  - `relativeTimePatterns(languageId, style)`
    (`RelativeTimeFormatHelpers.cpp`): for each unit from second to year,
    the six future and six past patterns, one per plural category, then
    the relative names (`"yesterday"`) with their offsets;
  - `displayName(languageId, kind, code, style)`, an `OptUtf8` that is
    `undefined` when there is no name, and
    `localeDisplayPatterns(languageId)`, the locale pattern and separator
    (`DisplayNamesHelpers.cpp`);
  - `segmenterInit`, `segmentsInit`, `segmentsFindBoundary` (FindBoundary,
    an `Int`) and `segmentsIsWordLike` (`SegmenterHelpers.cpp`). The
    string goes to `segmentsInit` as `Utf16`;
  - `numberSymbols` gains entry 21, the time separator, which
    DurationFormat reads.

  `NativeHelpers.def` gives the layout of each list.
- Native code reports only internal errors, as C++ exceptions. Hermes turns a
  C++ exception thrown from a host function into an `Error` made from its own
  intrinsic, with the message `Exception in HostFunction: ...`
  (`API/hermes/hermes.cpp`, the host-function trampoline); user code cannot
  intercept that. Native code keeps no JS values, so it captures no
  intrinsics. The `RangeError`s and `TypeError`s the spec requires are thrown
  by JS, which validates every argument first.

### Plural rules

A `PluralRulesState` owns an ICU4X `PluralRulesWithRanges<PluralRules>`
for a locale and a type (cardinal or ordinal). Both `Intl.PluralRules` and
NumberFormat's internal plural-rules objects use it.

- **Lookup.** The rules are those of the language identifier of
  `[[Locale]]` without its script, and without its region unless ICU4X has
  rules for that language and region themselves (`pt-PT`). CLDR keys
  plural rules by language and by language-region only, and ICU4X's
  fallback for a locale whose CLDR parent is root (`sr-Latn`, `bs-Cyrl`),
  or for a region that implies another script (`ku-Latn-IQ` becomes
  `ku-IQ`, likely Arabic), reaches root's rules, which have only "other".
- **Operands.** `pluralSelect` takes a significand string (ASCII digits,
  with at most one `"."` between digits) and an integer exponent, and
  selects for the value significand × 10^exponent. The operands are built
  exactly, with ICU4X's `RawPluralOperands`, from the digits of that value,
  as UTS 35 defines `i`, `v`, `w`, `f` and `t`, so `v` and `f` keep
  trailing zeros (`"1.0"` is "other" in `en`). ICU4X's own conversions keep
  only 18 fraction digits, which changes results for longer fractions (bs
  `0.00000000000000000001` is "one", with `f = 1`). An operand of more than
  18 digits keeps its last 18 digits plus 10^18, ICU4X's convention, so
  modulo and equality tests keep their results.
- **The `c` operand** (also called `e`) is the exponent when it is
  positive, and 0 otherwise. CLDR defines it only for the compact and
  scientific exponents of large numbers.
- **`Intl.PluralRules`** follows the draft. ResolvePlural rounds the whole
  value with FormatNumericToString. PluralRuleSelect, which the draft
  leaves to the implementation, takes that string and computes the
  exponent E of the notation: the magnitude for scientific, the magnitude
  rounded down to a multiple of 3 for engineering, the compact patterns'
  exponent for compact (from the locale's default numbering system's
  compact data), and 0 for standard or a zero value. For E > 0 it sends the
  string with its decimal point moved E places left (an exact digit
  shift, no rounding) and E, so the value is unchanged and only `c` is
  added: en ordinal scientific `21` sends `"2.1"` and 1, and selects
  "one", and fr `1e6` and `1.5e6` in compact notation are "many"
  (`c = 6`), as test262 expects. For E ≤ 0 it sends the string itself with
  0, which gives the same operands, since `c` is 0 either way; moving the
  point right by an engineering exponent below the magnitude (`"0.0001"`
  and -6) would need zeros the value does not have, which would count as
  fraction digits.
- So fr `1e-6` in scientific and engineering notation is "one" (`i = 0`,
  and fr's "one" rule is `i = 0,1`), where test262's
  `intl402/PluralRules/prototype/select/notation.js` expects "many". That
  test stays in the expectations file with the reason that CLDR defines the
  `e` operand only for non-negative exponents.
- **NumberFormat** selects the plural form of unit and currency name
  patterns for what it displays: its rounded significand with the
  notation's exponent (ru long compact `1000` kilometers is `"1 тысяча
  километров"`, as in ICU4C). NumberFormat rounds the scaled significand
  and PluralRules the whole value, as the draft says, so the two can
  differ: ru `10001` in scientific notation displays `1E4`, and NumberFormat
  selects "many", while `new Intl.PluralRules("ru", {notation:
  "scientific"}).select(10001)` is "one". Two differences from ICU4C
  follow from this: ru long compact `1234` kilometers takes the form of
  1200, "many" (ICU4C takes a fractional form), and fr `0.000123` in
  scientific notation with a unit takes "one", since `c` is 0 (ICU4C takes
  "other").
- **Ranges.** `pluralSelectRange` gives CLDR's plural range for the two
  categories. A pair CLDR does not list gives the end's category, UTS 35's
  default ("If there is no value for a <start,end> pair, the default
  result is end"), which ICU4X follows. ICU4C gives "other" instead (in 464
  of 6,188 sampled pairs, mostly descending ones such as 5 to 1, and en
  ordinal 1 to 2). The draft's step that gives two ends with the same
  formatted string the start's category makes en `selectRange(1, 1)`
  "one", where ICU4C gives "other".
- `select` and `selectRange` convert with ToIntlMathematicalValue, as the
  draft says, so they accept a BigInt (`select(1n)` is "one"). The pinned
  test262 still expects ToNumber's `TypeError`, so
  `selectRange/argument-tonumber-throws.js` is an expected failure.
- **Data gaps.** ICU4X 2.3 has no plural rules for `ff`, `kab` and `bal`.
  So `ff-Adlm` (12 tags) and `bal-Latn` have only "other", in PluralRules
  and in NumberFormat's plural patterns; ICU4C has one and other. `gv` is
  not in ICU4X's compiled locale set, so it resolves to the default locale,
  and `resolvedOptions/plural-categories-order.js` is an expected failure.

## Data

- **Locale data** is compiled into the library. The first version uses
  ICU4X's published data crates (ICU4X 2.3, CLDR 48.2; the `compiled_data`
  feature, carrying ICU4X's recommended set of about 500 locales). No
  datagen step, no CLDR download during the build. The linker drops data for
  services the code never uses.
- **`[[AvailableLocales]]`** (ECMA-402 9.1) is the closure of the locales the
  compiled data covers (`intl/core/data/locales.txt`) under two rules:
  - a canonical tag that maximizes to the same tag as a listed entry is
    available, because ICU4X serves it with that entry's data (CLDR stores
    `en-US` as `en`, `zh-Hant-TW` as `zh-Hant`);
  - every less narrow form of an available tag is available: each prefix
    (dropping trailing subtags, as `LookupMatchingLocaleByPrefix` does) and
    the language-region form of a language-script-region tag, both of which
    9.1 requires.

  Without the closure `zh-TW` would resolve to Simplified Chinese. The cost
  is that some tags are reported available that ICU4X serves through its
  own fallback.
- **`Intl.supportedValuesOf`.** Each list must also be accepted by the
  constructor that uses it, once that constructor exists (test262 checks
  this).
  - Numbering systems: all 78 rows of ECMA-402 Table 30, which in CLDR 48.2
    are exactly CLDR's numeric systems
    (`intl/core/data/numbering-systems.txt`). ICU4X's compiled data has
    digits for only 27 of them, so NumberFormat takes the digits of every
    one from the table, in JS (see "Number formatting").
  - Calendars: the calendars ICU4X implements, plus the CLDR aliases of
    those (`islamicc`, `ethiopic-amete-alem`), because 6.9.1
    AvailableCalendars must list both an alias and its canonical form or
    neither. `supportedValuesOf` drops the aliases again (8.3.2 step 2.b);
    the constructors that take a calendar must accept and canonicalize them.
  - Currencies come from CLDR (`currencies.txt`), collations from the
    tailorings in ICU4X's data, time zones from ICU4X's IANA data, and units
    from ECMA-402's table, in JS. `intl/core/data/update-cldr-lists.py`
    regenerates the CLDR lists when ICU4X is upgraded.
- **Collator** (Phase 2). How `Intl.Collator` gets its locale data and
  reaches ICU4X:
  - `[[RelevantExtensionKeys]]` is « "co", "kf", "kn" ». The
    `[[ResolutionOptionDescriptors]]` are the draft's: `co`/`collation`,
    `kn`/`numeric` (boolean) and `kf`/`caseFirst` with the values « "upper",
    "lower", "false" ».
  - For each available locale, `[[co]]` is « null » followed by the
    collations `collationsOfLocale` gives (so `getCollations` and the
    constructor agree), and `[[kn]]` is « "false", "true" ». `[[kf]]` starts
    with ICU4X's default case-first for the locale ("upper" for `da` and
    `mt`, "false" for the others) followed by the other two values.
    `[[ignorePunctuation]]` is true where ICU4X's default alternate handling
    is shifted (`th`). `[[sensitivity]]` is "variant" for search only. The
    sort and search locale data are otherwise the same. The defaults come
    from `collatorLocaleDefaults`.
  - The constructor uses the plugin's one `[[AvailableLocales]]`
    (`isAvailableLocale`), as `getCollations` does.
  - ICU4X's compiled data has no "search" tailorings (checked in
    `icu_collator_data` 2.3.0), and the design rules out a datagen step. So
    `usage: "search"` uses the sort tailoring, with its own default
    sensitivity, and `intl402/Collator/usage-de.js` keeps failing.
  - The resolved options reach ICU4X explicitly, not through `-u-` keywords
    in the tag:

    | ECMA-402 | ICU4X |
    |---|---|
    | sensitivity `"base"` | strength `Primary` |
    | sensitivity `"accent"` | strength `Secondary` |
    | sensitivity `"case"` | strength `Primary`, case level `On` |
    | sensitivity `"variant"` | strength `Tertiary` |
    | ignorePunctuation `true` / `false` | alternate handling `Shifted` / `NonIgnorable`, always set, because `th` defaults to shifted |
    | numeric | `numeric_ordering` |
    | caseFirst | `case_first` |
    | collation other than `"default"` | `collation_type` |

    Only the language identifier of `[[Locale]]` picks the tailoring.
  - The String overrides create their collator through the internal
    creation function, never through `Intl.Collator`. When `locales` and
    `options` are both `undefined`, `localeCompare` reuses one default
    collator kept in a JS variable; otherwise it shares a collator among
    the calls that read the same locales and option values (see
    "Resistance to tampering").
- **Case mapping** (Phase 2). TransformCase's available locales are « "az",
  "el", "hy", "lt", "tr" »: the languages ICU4X's `CaseMapper` tailors for
  upper and lower case. `nl` is left out because it tailors only titlecase.
  A tag matches by prefix (`tr-TR` gives `tr`), and no match gives `"und"`,
  the default mapping. The `hy` uppercase tailoring differs from engines
  built on ICU4C: here `"և".toLocaleUpperCase("hy")` is `"ԵՎ"`, there it is
  `"ԵՒ"`, the default mapping. When `locales` is `undefined`, the locale
  matched for `DefaultLocale()` is kept in a JS variable.
- **NumberFormat and PluralRules** (Phase 3). Native code supplies locale
  data as lists of strings that JS reads by index (see "Number formatting"
  and "Parts"). This is where it comes from, and how JS uses it.
  - **Sources.** ICU4X 2.3 has no unit display names for 21 of the 45
    sanctioned units (none for `bit`, `byte` and their multiples,
    `celsius`, `fahrenheit`, `degree`, `percent`, or any `-per-` unit), and
    its long names for the units it has are wrong (`en` long `meter`
    renders `"1.5 m"`). It also has no infinity, NaN, exponent or
    approximately sign, and no range pattern; its currency symbol data
    leaves out a symbol that equals the ISO code, so its locale fallback
    finds a parent's (`af` USD would be `"US$"`, where CLDR has `"USD"`);
    and it has no explicit data for numbering systems outside a locale's
    own. So:

    | Data | Source |
    |---|---|
    | decimal and group separators, plus and minus signs, grouping sizes | ICU4X `DecimalSymbolsV1` |
    | compact patterns | ICU4X `DecimalCompactShortV1` and `DecimalCompactLongV1` |
    | currency patterns (standard, accounting and their alphaNextToNumber forms), plural names, name patterns, fraction digits | the provider structs of `icu_experimental` 0.6: `CurrencyEssentialsV1`, `CurrencyExtendedDataV1`, `CurrencyPatternsDataV1`, `CurrencyFractionsV1`. They are read directly; `CurrencyFormatter` is never used, because it ignores the digit options, grouping and the locale's name patterns |
    | currency symbols and narrow symbols, and per-currency formats (`<currency type="X">`'s `<pattern>`, `<decimal>` and `<group>`) | `intl/core/data/currency-symbols.txt` |
    | plural rules, plural ranges, plural categories | ICU4X `PluralRules` and `PluralRulesWithRanges` |
    | percent sign, exponent symbol, infinity, NaN, approximately sign, percent pattern, range pattern, currency decimal and group separators, and (since Phase 5, for DurationFormat) the time separator | `intl/core/data/number-symbols.txt` |
    | unit patterns | `intl/core/data/units.txt` |
    | the explicit symbols and patterns of numbering systems outside a locale's own | `intl/core/data/explicit-number-systems.txt` |
    | the short (compact) currency patterns, which ICU4X 2.3 and `icu_experimental` 0.6 do not have (ICU4X's compact currency formatter puts its decimal compact patterns inside the ordinary currency patterns) | `intl/core/data/compact-currency.txt` |
    | digits | ECMA-402's table, in JS |
    | the default numbering system | the `defaultNumberingSystem` helper, so that `getNumberingSystems` and NumberFormat agree |

    `update-cldr-lists.py` generates the five files from CLDR 48.2, the
    release ICU4X 2.3 uses. `number-symbols.txt`, `units.txt` and
    `currency-symbols.txt` come from the CLDR JSON packages (`cldr-core`,
    `cldr-numbers-full` and `cldr-units-full`, npm version 48.2.0 for the
    tag `release-48-2`), whose locale data is already resolved.
    `number-symbols.txt` has a line for each locale and numbering system;
    the other two hold only what differs from a locale's parent chain, and
    the generator checks that reading them back along that chain gives
    each locale's data. `explicit-number-systems.txt` comes from CLDR's
    `common/main/*.xml`, because cldr-json leaves those values out.
    `compact-currency.txt` comes from both: cldr-json for a locale's own
    numbering systems, the XML for the others (see "Numbering systems"),
    in the same form as `units.txt`, with each pattern's digits written as
    one `0` so that consecutive magnitudes share a line, and the generator
    checks it the same way. `intl/core/data/LICENSE` is the Unicode License
    V3, which covers every CLDR-derived file there. `intl/README.md`
    describes the formats. The five files are 0.94 MB, 0.78 MB of it
    `units.txt` and 39 KB `compact-currency.txt`.
  - **Lookups.** Native data is looked up by the language identifier of
    `[[Locale]]` (the `-u-` keywords are already in the slots) and the
    numbering system, passed explicitly. In the generated files the entry
    for a locale is the first one ICU4X's `LocaleFallbacker` reaches from
    it. Every locale in `locales.txt` has an entry, so this finds the entry
    ICU4X's own data would use; inside a file, a missing value is inherited
    along the parents the file records. A tag that has no directory in the
    JSON packages (`zh-HK`, `sr-ME`) is read from the directory with the
    script of its `likelySubtags` entry inserted (`zh-Hant-HK`), or, for a
    tag with no entry of its own, the script of its language's entry
    (`zh-SG` is read as `zh-Hans-SG`, `sr-BA` as `sr-Cyrl-BA`), which is
    the data ICU4X uses for it.
  - **Numbering systems.** The data for `(L, nu)` is resolved as CLDR's
    inheritance does:
    - when `nu` is one of the locale's own numbering systems (its default
      and its other numbering systems, the ones cldr-json and ICU4X have),
      the data is ICU4X's and `number-symbols.txt`'s for `(L, nu)`;
    - otherwise each symbol, the percent pattern, the range pattern and
      each currency pattern is the explicit value for `nu` of the nearest
      locale of L's chain, root included, in CLDR's XML, and where the
      chain has none, L's latn value, to which root's aliases point. CLDR
      48.2's root defines arab and arabext symbols, and arab percent and
      currency patterns, so `en-u-nu-arab` and `de-u-nu-arab` use Arabic
      separators and signs, as ICU4C does (`"؜-١٬٢٣٤٫٥"`, `"٥٠٪؜"`); deva,
      beng and the others alias to the locale's latn data. Many locales
      have such values too (`fr`'s arab plus sign, `fa`'s arab NaN);
    - the grouping and the compact patterns of such a numbering system are
      L's latn ones, because root aliases its decimal formats to latn;
    - the short currency patterns of compact notation are a kind of data
      of their own (CLDR's `currencyFormatLength type="short"`), resolved
      pattern by pattern: the explicit pattern for `nu` of the nearest
      locale of L's chain in CLDR's XML (only `ba` and `kok` have any in
      CLDR 48.2: `kok`'s 10^14 alphaNextToNumber pattern for every
      numbering system is `"¤000LCr"`), else L's latn pattern. Root's
      arab currency formats have no short ones, so `en-u-nu-arab` takes
      `en`'s latn ones, as ICU4C does; the generator checks that root has
      short currency formats for latn only. The signs and separators are
      the numbering system's.

    ICU4X's own formatter falls back to the locale's default numbering
    system instead, which is one reason the plugin reads the data itself.
    A locale's own decimal formats for such a numbering system are not
    read: in CLDR 48.2 only `az` has them, for arab and arabext, and they
    are the text `"standart onluq kəsr0.###"`, which ICU4C prints. Nor are
    scientific and rational formats, or miscellaneous patterns other than
    the range pattern.
  - **Currency digits** are CLDR's `digits` for the currency (not its cash
    digits), and 2 for a currency CLDR does not list. The draft asks only
    for "the number of fractional digits used when formatting quantities
    of the currency", and CLDR's values are the ones engines built on ICU
    use.
  - **Grouping.** With the primary size `p`, the secondary size `s` and
    the minimum grouping digits `g` of the data, and `n` integer digits:
    `false` never groups, `"always"` groups when `n > p`, `"auto"` when
    `n >= p + g`, and `"min2"` when `n >= p + max(g, 2)`. The groups are the
    last `p` digits and then groups of `s`. `"always"` ignores the locale's
    minimum grouping (`pl` 1234 is `"1 234"`), as ICU4C does and ICU4X's
    `Always` does not. The percent style, and the unit `percent` at the
    widths short and narrow, which formats with the percent patterns (see
    "Units"), take `p` and `s` from the locale's CLDR percent pattern,
    which differs from the decimal one in 21 locales (`bn` groups percents
    by threes, decimals by three and two).
    The currency style, in every display and notation, uses CLDR's
    `currencyDecimal` and `currencyGroup` where the locale has them: `de-AT`
    groups currencies with `"."` and other numbers with U+00A0, and `fr-CH`
    has the currency decimal separator `"."`. They resolve as the other
    symbols do. A numbering system whose symbols root aliases to latn has
    the locale's latn ones (`de-AT-u-nu-deva` groups currencies with
    `"."`, `fr-CH-u-nu-thai` has the currency decimal `"."`). Root's
    explicit arab and arabext symbols have no currency separators, so
    `de-AT-u-nu-arab` uses its plain ones (UTS 35: without
    `currencyGroup`, the group separator), where ICU4C takes the missing
    symbols from latn's. No locale of CLDR 48.2 has currency separators
    for a numbering system other than its own, which the generator checks.
  - **Per-currency formats.** CLDR's `<currencies><currency type="X">`
    has a pattern and decimal and grouping separators for some currencies
    (41 patterns, 29 decimal and 27 grouping separators in CLDR 48.2:
    en-150's and its children's EUR `"¤#,##0.00"`, `tr`'s TRY, `it`'s
    ITL; pt-PT's and pt-CV's PTE and CVE decimal `"$"`).

    What UTS 35 prescribes: tr35-numbers.md, "Currencies", says a
    currency may have optional grouping, decimal and pattern elements,
    "inherited from the `<symbols>` in the same locale data (if not
    present in the chain up to root)", and points to tr35.md's
    inheritance rules. tr35.md's "Lateral
    Inheritance" table lists them: `currency@pattern` falls back to
    `currencyFormat` (for the default numbering system and
    `type="standard"` unless an API specifies another, such as
    accounting), and `currency@decimal` and `currency@group` to `symbols`
    `decimal` and `group` (for the default numbering system unless
    specified), "within the same locale, before inheriting from the
    parent"; its example is pt_CV's CVE decimal `"$"`.

    The plugin's policy within that: the numbering system and the type are
    the formatter's (`numberingSystem` and `currencySign` "specify" them).
    At each locale of the chain the currency's own value comes first;
    else the locale's own currencyFormat pattern, or symbols decimal or
    group, for that context ends the search with none, so the locale's
    data applies; else the parent. The Currencies section's wording
    ("if not present in the chain up to root") can be read as searching
    the whole chain first; the plugin follows the table, which differs
    only for en-CH, en-GE, en-NL, en-PL, en-PT, en-RO, en-SI, en-SK and
    it-CH in CLDR 48.2. cldr-json resolves these fields vertically only,
    so the generator reads them, and the locales' lateral sources, from
    the XML. The table names `symbols` `decimal` and `group`; the
    plugin puts a currency's own separator before `currencyDecimal` and
    `currencyGroup` too, which no CLDR 48.2 locale has together. A
    per-currency pattern has no alphaNextToNumber form, so CLDR's
    currency spacing applies to it, and the fraction digits come from the
    currency's supplemental data, as for any currency pattern.
    - The separators come before `currencyDecimal` and `currencyGroup`,
      in every display, notation and numbering system (pt-CV `CVE`
      `-1234.5` is `"-1234$50\u00a0\u200b"` (its symbol is U+200B),
      compact `"-1$2\u00a0mil\u00a0\u200b"`, the name display
      `"-1234$50 escudos cabo-verdianos"`, with arab digits
      `"؜-١٢٣٤$٥٠\u00a0\u200b"`).
    - The pattern takes the place of the locale's standard or accounting
      currency pattern for the code, symbol and narrowSymbol displays
      (en-150 `EUR` `"-€1,234.50"`, `USD` `"-1,234.50 US$"`; en-DE `EUR`
      `"€1.234,50"` with en-DE's separators; en-150 arab
      `"؜-€١٬٢٣٤٫٥٠"`). It has no alphaNextToNumber form, so CLDR's
      currency spacing applies (tr `TRY` code `"TRY 1.234,50"`). Found
      before tr's accounting pattern, it is the accounting pattern too
      (tr `"-₺1.234,50"`).
    - A locale's own patterns or separators come before its parents'
      per-currency ones: en-CH's own currency patterns give `EUR`
      `"€-1'234.50"`, while en-CH with deva, which has none there, takes
      en-150's (`"-€१'२३४.५०"`); en-PL has only an accounting pattern of
      its own, so `EUR` is `"-€1.234,50"` and accounting
      `"(1.234,50 €)"`; it-CH's own symbols and standard pattern come
      before it's ITL ones, its accounting takes it's ITL pattern.
    - Compact notation uses the pattern for the numbers it does not
      compact (en-DE `EUR` `999` is `"€999"`) and the separators at every
      magnitude; the short currency patterns are separate data.

    ICU4C differs: it applies the locale's default currency's format to
    currencies that have none (en-DE `USD` `"US$1,234.50"`, pt-CV `USD`
    `"1234$50 US$"`); its data format stores a currency's pattern and
    both separators together, so a currency with any of them takes the
    others, in latn, from the locale whose entry it is: the separators of
    en-150 with its EUR pattern (en-DE `EUR` `"€1,234.50"`), tr's latn
    separators with arab digits (tr `TRY` arab `"₺٩٩٩,٠٠"`), and the
    locale's standard pattern for accounting where the currency has only
    separators (pt-PT `PTE` accounting `"-999$00\u00a0\u200b"`); it does not use the
    locales'
    own patterns and separators as lateral sources (en-CH `EUR`
    `-1234567.5` is `"-€1,234,567.50"`, en-PL accounting
    `"-€1,234,567.50"`, it-CH `ITL` `"-ITL 1.234.568"`); it puts the
    symbol into the name display, which UTS 35's currency name algorithm
    formats with the decimal pattern (`"₧ 999 pessetes espanyoles"`);
    and for pt-CV `PTE`, whose group pt_CV.xml inherits from pt_PT's
    `","`, it groups with U+00A0. Like the plugin, it applies a
    currency's own values in every numbering system and to accounting.
  - **Signs and accounting.** For decimal and percent, the minus and plus
    signs go where CLDR's sign goes: before the whole pattern when the
    locale's sign is a prefix. Currency patterns come from ICU4X's explicit
    positive and negative patterns. The bidi marks CLDR puts in a sign
    (U+200E, U+200F, U+061C) are literal text of the patterns, and the sign
    symbols are the bare signs (see "Parts").
    - An accounting negative pattern has no `{minusSign}` (`"($0.00)"`),
      unlike the draft's locale-data constraint; test262 and ICU4C both
      produce it.
    - Currency spacing is CLDR's one rule: U+00A0 goes between the currency
      string and a digit when the currency string's character next to the
      number is neither a symbol (S) nor a separator (Z). Every CLDR 48.2
      locale uses it, which the generator checks. Where ICU4X has no
      alphaNextToNumber form of the accounting negative, the plain
      accounting pattern is used with that spacing (`ee` code `-1` is
      `"(USD 1.00)"`, as in ICU4C).
    - NaN and the infinities, which put no digit next to the currency, use
      the patterns without currency spacing (`"USDNaN"`, as in ICU4C).
    - The currency name display has no accounting form and uses the
      standard sign patterns. A unit or currency name pattern wraps the
      signed number (`"時速 -987 キロメートル"`).
  - **Units.** `units.txt` holds every pattern of the 45 sanctioned units
    and their per-unit patterns, for the widths long, short and narrow,
    and CLDR's dedicated compound units whose two halves are sanctioned
    (`en` has `kilometer-per-hour`, `liter-per-kilometer`,
    `meter-per-second`, `mile-per-gallon` and `mile-per-hour`). An
    `X-per-Y` unit uses, in this order, ICU4C's:
    1. the dedicated compound unit, if the locale has one;
    2. Y's per-unit pattern with `{0}` replaced by X's pattern for the
       plural category;
    3. the width's compound pattern (`"{0}/{1}"`, `"{0} per {1}"`) with
       `{0}` replaced by X's pattern for the plural category, and `{1}` by
       Y's "one" pattern with `{0}` and the whitespace next to it removed.

    The unit `percent` at the widths short and narrow uses the locale's
    percent patterns, with its percent sign as the `unit` part and no
    scaling; long uses CLDR's unit pattern. ICU4C does the same (`de`
    `"-1 %"` with U+00A0, `tr` `"-%1"`, long `"-1 Prozent"`). A compound
    with `percent` keeps its denominator (`en` short `percent-per-day` is
    `"{0}%/d"`), where ICU4C drops it and prints only the percentage.
  - **Notations.**
    - Scientific and engineering use the sub-pattern
      `"{number}{scientificSeparator}{scientificExponent}"` with the
      locale's exponent symbol.
    - ICU4X stores compact patterns as entries, each with the magnitude it
      starts at (CLDR's "type"), its exponent, and its patterns.
      Consecutive magnitudes with compatible patterns are one entry, and
      magnitudes below the first entry are not compact.
      ComputeExponentForMagnitude takes the exponent of the last entry
      that starts at or below the magnitude, and 0 if there is none (`ja`
      1000 to 9999, `de` short thousands). Exponent 0 formats with
      `"{number}"`. The pattern comes from the entry of the displayed
      number's magnitude among those with that exponent: in `ar`, 3000
      takes the "few" pattern of the 10^3 entry and 30000 the 10^4 entry,
      which has the same exponent.
    - Compact pattern selection is ICU4X's (`PluralElements::get`): when
      the displayed number is exactly 0 or 1 with no fraction digits and
      the entry has an exact-0 or exact-1 pattern, that pattern; otherwise
      the pattern for the plural category of the displayed number alone,
      with exponent 0 (`ru` long `1000` is `"1 тысяча"`, `fr` long `1000`
      is `"mille"`). "Parts" says when a pattern without `{0}` may be
      used, so `it` long `-1000` is `"-1 mila"`, where ICU4C gives
      `"-mille"`.
    - Compact notation with the currency style (the code, symbol and
      narrowSymbol displays) uses CLDR's short currency patterns
      (`compact-currency.txt`), as UTS 35's "Compact Number Formats" says:
      their magnitudes and digits give the exponent (`ja` compacts by four
      digits), the pattern `"0"` means that the magnitude is not compacted
      (`de` `1234` with `EUR` is `"1234 €"`, `1234567` is `"1,2 Mio. €"`),
      the alphaNextToNumber pattern is used where the currency string
      touches the digits with a character that is neither a symbol nor a
      separator (`en` `CHF` `"CHF 1.2K"`), and CLDR's currency spacing
      applies after that. The plural category of the displayed number
      picks the pattern. Each pattern is split in two for the draft's
      levels of patterns: the currency, its spacing and the text beyond
      them (bidi marks) make the sign patterns, and the number with its
      compact text the compact pattern, so `fa` `1000` is `"\u200e$\u00a0۱\u00a0هزار"`
      with its own space after the currency, which the ordinary arabext
      pattern lacks. A number below the first compact magnitude, zero, NaN
      and the infinities use the ordinary currency patterns of `(L, nu)`
      (`en-u-nu-arab` `999` is `"٩٩٩ $"`, as in ICU4C). Where the plural
      category of the compacted number picks the pattern `"0"` while the
      magnitude's other counts are compact (`vec`'s 10^3 "one", for the
      decimal and the currency patterns, the only such data in CLDR 48.2),
      the number is not compacted, as UTS 35 says of `"0"`: it is
      formatted again unscaled with exponent 0, like a number below the
      first compact magnitude (`vec` `1000` is `"1000"` and
      `"1000 EUR"`, `1400` is `"1,4 mila EUR"`). ICU4C keeps the scaled
      number (`"1"`, `"1 EUR"`). Ranges and parts follow the number
      shown; `Intl.PluralRules` gives `vec` compact `1000` `"other"`, the
      category of `"1000"`, since its compacted string keeps the unscaled
      digits (`"1.000"` with c = 3), which is never `"one"`. These
      patterns are used whatever
      `compactDisplay` says (`"$1.2K"`, not `"$1.2 thousand"`), as ICU4C
      does, because CLDR has compact currency patterns only in short form;
      the draft's GetNotationSubPattern, read literally, takes
      `[[CompactDisplay]]`. The name display spells the number out with
      the decimal compact patterns of `compactDisplay`
      (`"1.2 thousand US dollars"`). `resolvedOptions` still reports
      `[[CompactDisplay]]`.
    - Accounting applies with compact notation (`"($1.2K)"`), as the
      draft's patterns give; ICU4C ignores it there (`"-$1.2K"`). CLDR has
      no short accounting patterns, so the accounting negative pattern is
      the ordinary one with its currency, spacing and number replaced by
      the short pattern's (`fr` `"(1,2 k €)"`), unless the ordinary one
      shows a minus sign (`de`), which then goes where the short pattern
      puts it.
    - The sign goes outside the compact pattern (`sw` `"-elfu 1.2"`, and
      with a currency `"-US$ elfu 1.2"`), the draft's structure; ICU4C
      puts it next to the number (`"elfu -1.2"`), from `sw`'s explicit
      negative patterns, which the plugin does not use.
    - The percent style in compact notation keeps the locale's percent
      pattern and its `percentSign` part. ICU4C labels the `%` a `unit`
      there and sometimes writes U+0020 where the pattern has U+00A0
      (`de`, `de-AT`).
  - **Approximately.** FormatApproximately inserts the `approximatelySign`
    part:
    - before the first `minusSign` or `plusSign` part, and before the bidi
      marks of that sign (`ar-EG` `-3` gives `"~؜-٣"`, as in ICU4C);
    - if there is none and no sign is shown (the zero pattern: a value that
      is not negative, or a negative one under `signDisplay: "never"`),
      where `{plusSign}` stands in the formatter's positive pattern, in
      place of the plus sign and its marks: `"~$3"`, `"~3 €"`, and
      `"~$3.00"` for accounting, as ICU4C gives;
    - otherwise before the first part: an accounting negative, which
      shows no sign (`"~($3.00)"`), or a pattern without a number. ICU4C
      omits the sign there, which the draft also allows; the plugin shows
      it, so an approximate range never looks exact.

    Two ends with the same FormatNumeric string format approximately, as
    the draft says: `signDisplay: "never"` from -5 to 5 gives `"~5"`, where
    ICU4C gives `"5–5"`.
  - **Ranges.** The separator is the text between `{0}` and `{1}` of the
    locale's range pattern. CollapseNumberRange follows ICU4C's default
    collapse mode in looking at each side's text in two parts. Each side
    has its numbers (the parts from its first to its last `integer`,
    `group`, `decimal`, `fraction`, `exponentSeparator`,
    `exponentMinusSign`, `exponentInteger`, `nan` or `infinity` part, with
    any literal between them); around them, the sign, currency symbol or
    code, percent sign and compact text, with the literals of their
    patterns; and around all of that, the text of a unit or currency name
    pattern. Two pieces of text match if their parts have the same types
    and, except for `unit`, `currency` and `compact` parts, whose text
    depends on the plural form, the same values.
    - The unit or currency name text is shown once when both sides have
      numbers and the two sides' unit or name texts match (`"3–5 km"`,
      `"3K – 5M km"`).
    - The sign, symbol and compact text is shown once only when the unit
      or name text is, the two exponents are equal, neither side shows an
      exponent (scientific or engineering), the two sides' texts match
      and have more than one code point, and at least one of the pieces
      left out (the start's text after its number or the end's text before
      its number) has a `currency` or `percentSign` part (`"$3–5K"`,
      `"3–5 €"`, `"($3.00–5.00)"`, `"+$2.90–3.10"`; but `"$3 – $5"`,
      whose `"$"` is one code point).
    - Text shown once keeps the start's copy before the numbers and the
      end's copy after them, as parts with the source `"shared"`.
    - A space is added at each end of the separator that has none when the
      start side has text of either kind that is not shown once
      (`"$3 – $5"`, `"-3 – -5 km"`), or either side shows an exponent
      (`"3E3 – 5E3"`). These are ICU4C's conditions: text that only the end
      side has adds none (`"5–-5"`).

    The exponent condition and the `currency` or `percentSign` condition
    for the sign, symbol and compact text are the plugin's. The draft
    requires that collapsing never make two ranges look the same that are
    different in full, and ICU4C's rule breaks that: its `"3–5 thousand"`
    is both the range from 3 to 5000 and the one from 3000 to 5000, and its
    `ar` `"‎-3E0 – 5E0"` is both -3 to -5 and -3 to 5. A currency or
    percent sign is shown for every value of the formatter, so a range
    whose sides keep all their text shows it once for each side, and one
    that leaves it out on one side shows it one time fewer; the two can
    never be equal. Two ranges that leave it out and are equal have equal
    numbers, since that text has no digits and sides with an exponent keep
    all of theirs. A sign or compact text alone gives no such count, so it
    is repeated: `"-3 – -5 km"` (as in ICU4C), `"3 thousand – 5 thousand"`
    and `"‎-3 – ‎-5"` in `ar` (where ICU4C gives `"3–5 thousand"`
    and `"‎-3–5"`). So is the percent unit's `"%"`, which is a `unit`
    part: `"-3% – -5%"` (ICU4C `"-3–5%"`).
  - **Resolution.** `[[RelevantExtensionKeys]]` of NumberFormat is « "nu"
    », the draft's. `[[LocaleData]].[[L]].[[nu]]` is the locale's default
    numbering system followed by the other entries of
    `numbering-systems.txt`, in that file's order. An unsupported
    `numberingSystem` option leaves the locale's `-u-nu` value in place,
    as the draft's ResolveLocale says (`en-u-nu-thai` with an unknown
    option stays `thai`), where V8 gives `latn`. PluralRules has no
    relevant extension keys, so `fr-u-nu-arab` resolves to `fr`. Both use
    the plugin's one `[[AvailableLocales]]`.
  - **List separator.** Array and TypedArray `toLocaleString` separate
    elements with `","`, as Hermes's own `Array.prototype.toLocaleString`
    does (see "Built-in overrides").
  - **ICU4C as a reference.** The plugin's results were compared with
    Node 24 (ICU 78.2) over samples of every style, notation and range.
    Apart from the differences stated above, the remaining ones come from
    ICU 78.2 shipping CLDR 48.0 against ICU4X's 48.2 (`en-150`, `en-BE` and
    other currency patterns, `fr-CH`'s group separator), from locales
    ICU4C has no data for (`bal-Latn`, `ht`, `za`), and from ICU4X's own
    data (`ps` long compact patterns).
- **DateTimeFormat** (Phase 4). Native code supplies CLDR's date patterns
  and day periods as lists of strings, and for each instant the calendar
  fields and the names its patterns show (see "Date and time
  formatting"). This is where they come from, and how JS uses them.
  - **Sources.** ICU4X 2.3 has no CLDR `availableFormats`,
    `intervalFormats` or flexible day periods, so `update-cldr-lists.py`
    generates two more files from CLDR 48.2:
    - `date-patterns.txt` (820 KB): for each locale of `locales.txt` (and
      `und`) and each of the 16 supported calendars, the `dateFormats`,
      `timeFormats`, `dateTimeFormats` (standard and `atTime`),
      `availableFormats`, `intervalFormats` with their
      `intervalFormatFallback`, the `Timezone` append item, and for
      `chinese` and `dangi` the numeric leap month pattern (`zh` `"闰{0}"`,
      `en` `"{0}bis"`). It comes from `cldr-dates-full` and the
      `cldr-cal-*-full` packages (npm version 48.2.0), except `iso8601`,
      which cldr-json lacks: its patterns are root's from the XML, with
      what each locale of the CLDR chain defines over them (in CLDR 48.2
      only `zh`, 14 intervals that its children inherit). It has the form
      of `units.txt`, with a line `<calendar> = <other calendar>` where a
      locale's whole data for a calendar equals its data for another.
      Alternates (`-alt-ascii`, `-alt-variant`) and count variants are left
      out, and so is an entry whose skeleton has a field ECMA-402 does not
      have (`Q`, `w`, `W`, `D`, `F`, `g`, `e`, `u`, `Y`, `x`, `X`, `Z`,
      `V`). Any such letter in a kept pattern fails generation, except two
      that CLDR 48.2 writes for the letters meant: `Y` (week-based year) is
      read as `y`, and `e` (local day of the week) as `E`, with the same
      count (`gl`'s full date format, `te`'s `yyyyMMMM`, `ak`'s and `ko`'s
      interval patterns).
    - `day-periods.txt` (51 KB): CLDR's `dayPeriodRuleSet` of each
      language, and each locale's format-context names of its periods
      (abbreviated, wide and narrow). A locale's rules are found as ICU
      finds them, by truncating its tag until a rule set has it (`zh-Hant`
      uses `zh`'s, though its CLDR parent is root, and so does its child
      `zh-HK`); its names follow its parent chain. The times are CLDR
      48.2's: in `en`, `morning1` is 0:00 to 12:00 and `night1` 21:00 to
      24:00.

    The generator checks that every locale and calendar reads back to its
    CLDR data. Lookups work as for NumberFormat: by the language
    identifier of `[[Locale]]`, whose entry is the first one ICU4X's
    `LocaleFallbacker` reaches from it. `intl/README.md` describes the
    formats.
  - **Time zone identifiers.** The draft's CreateDateTimeFormat stores
    the primary identifier (`Asia/Calcutta` is `Asia/Kolkata`, `Etc/UTC`
    is `UTC`), and so does the plugin. test262's `canonical-tz` tests come
    from Temporal's newer rule, which keeps the identifier as given, so
    four of them are expected failures (see "Phasing"). An offset time
    zone is parsed with ECMA-262's `UTCOffset` grammar (ASCII signs only;
    `+05:30`, `+0530`, `+05`); seconds are a `RangeError`, as the draft
    says, and `[[TimeZone]]` is FormatOffsetTimeZoneIdentifier's
    `"+05:30"`. Its names are ICU4X's localized GMT format (`en`
    `"GMT-3:30"`, long `"GMT-03:30"`). ICU4X's offsets stop at ±18:00 and
    offset time zones go to ±23:59; beyond ±18:00, each name is ICU4X's
    for an offset of the same sign that it can hold, with the digits
    replaced by the real ones in the same widths (`+23:00` is
    `"GMT+23"`).
  - **ToLocalTime.** Native code computes the record of an instant: the
    UTC offset (see "Time zones" below, or the fixed offset of an offset
    time zone), then the local time, epoch milliseconds plus the offset,
    split into days and milliseconds of the day by floor division, and
    the date of that day in the formatter's calendar with ICU4X's
    `Date::from_rata_die` (day `d` after 1970-01-01 is Rata Die
    `d + 719163`; ICU4X's years ±999,999 cover ECMAScript's). The fields:
    - [[Weekday]] is ECMA-262's WeekDay, 0 for Sunday;
    - [[Year]] is the era year for a calendar with eras (ICU4X's
      `YearInfo::Era`), the cyclic year 1 to 60 for `chinese` and `dangi`
      (`YearInfo::Cyclic`), and the ISO year for `iso8601`. The draft's
      step that shows a year ≤ 0 as 1 − year then applies, as written.
      [[Era]] is ICU4X's era code (`"bce"`, `"ce"`, `"reiwa"`), and `""`
      for `chinese`, `dangi` and `iso8601`;
    - [[RelatedYear]] is the ISO year of `chinese` and `dangi`, and
      undefined for the others; [[YearName]] is the cyclic year's name;
    - [[Month]] is zero-based, as in the draft's Gregorian table, because
      FormatDateTimePattern adds 1. Native code gives the number shown:
      the ordinal month, or for `chinese` and `dangi` the number of the
      month code (`"M04L"` is 4) with a leap flag. A numeric leap month
      then takes the locale's numeric leap pattern (`zh-u-ca-chinese`
      2020-05-23 is `"闰4-1"`, `en` `"4bis/1"`); only `chinese` and
      `dangi` have one, so a numeric Hebrew `M05L` (Adar I) is its
      ordinal month number. A text month comes from ICU4X with the leap
      month already named;
    - [[Day]], [[Hour]], [[Minute]], [[Second]] and [[Millisecond]] are
      the local time's, and [[InDST]] is the tzdb's (see "Time zones");
    - `iso8601` is computed as `gregory` with no era and the ISO year, so
      year 0 shows as `"1"` and year −1 as `"2"`, as in ICU4C, and its
      `{era}` gives no part.
  - **Names.** Month, weekday, era, cyclic year, AM/PM and time zone names
    are ICU4X 2.3's, through its public
    `FixedCalendarDateTimeNames<C, CompositeFieldSet>`, one for each CLDR
    symbol the formatter's patterns use (`MMMM`, `ccc`, `zzzz`...),
    loaded with `include_for_pattern` for a one-symbol pattern and
    formatted with `with_pattern_unchecked`; `C` is the ICU4X type of
    the formatter's calendar (`iso8601` uses `Gregorian`'s names). The
    field set is `CompositeFieldSet` because `CompositeDateTimeFieldSet`
    has no time zone data in 2.3, so it cannot load the names of `z`, `O`
    or `v`. ICU4X's handling of leap months (Hebrew Adar I, Chinese leap
    months), era codes and cyclic years is therefore the plugin's. Each
    name is formatted for the instant's `ZonedDateTime`, whose
    `TimeZoneInfo` has the zone's ICU4X id (`TimeZone::UNKNOWN` for an
    offset time zone), the tzdb's offset, and the instant as its zone name
    timestamp.
    - Only the symbols a calendar has are valid: `U` only for `chinese`
      and `dangi`, the era for every calendar but those two and
      `iso8601`. JS asks for no other, and native code treats one as an
      internal error, as it does any error ICU4X's formatting returns.
    - Month and weekday names take the context of the CLDR letter they
      come from: `M` and `E` format, `L` and `c` stand-alone.
    - ICU4X chooses a specific name (`z`, `zzzz`) by comparing the offset
      with CLDR's standard and daylight offsets of the zone's metazone at
      that time; it ignores the variant it is given. So the tzdb's
      [[InDST]] does not choose the name, and where the tzdb's offset
      matches neither of CLDR's (local mean time, or a rule newer than
      CLDR 48.2), the name is the localized GMT format: `America/New_York`
      in 1800 is `"GMT-4:56:02"`, as in ICU4C.
    - Time zone names are formatted with Latin digits, and JS writes the
      ASCII digits of the `timeZoneName` part in [[NumberingSystem]]'s
      digits with NumberFormat's digit table: `en-u-nu-thai`
      `"GMT+๕:๓๐"`, `ar-EG` `"غرينتش‎+٥:٣٠"`.
  - **Day periods.** The draft's `dayPeriod` is CLDR's flexible day period
    (`B`), which ICU4X does not have, so JS chooses it from
    `day-periods.txt`: the rule whose range holds the minute of the day;
    "noon" only when the hour is 12 and the formatter shows no non-zero
    minute or second (`"12 noon"`, `"12:30 in the afternoon"`); "midnight"
    never, as in ICU4C (in `en`, 0:00 is `"12 in the morning"`, which
    test262's `dayPeriod-*-en.js` accept). A period CLDR names in no width
    (`lij`, `scn`) shows the hour's AM/PM name of the same width, as ICU
    does.
  - **Records.** A CLDR pattern becomes a DateTime Format Record: a pattern
    with the draft's placeholders and a width for each field (`n`
    letters):

    | CLDR | Placeholder | Field and width |
    |---|---|---|
    | `G` (1-3), `GGGG`, `GGGGG` | `{era}` | era short, long, narrow |
    | `y` (≠ 2), `yy` | `{year}` | year numeric, 2-digit |
    | `U` | `{yearName}` | year numeric |
    | `r` | `{relatedYear}` | year numeric |
    | `M`/`L` (1, 2, 3, 4, 5) | `{month}` | numeric, 2-digit, short, long, narrow; `L` stand-alone |
    | `d` (1, 2) | `{day}` | numeric, 2-digit |
    | `E` (1-3, 6), `EEEE`, `EEEEE`; `c` (3, 6), `cccc`, `ccccc` | `{weekday}` | short, long, narrow; `c` stand-alone |
    | `a`, `b` (1-3, 4, 5) | `{ampm}` | AM/PM name abbreviated, wide, narrow (no field) |
    | `B` (1-3, 4, 5) | `{dayPeriod}` | dayPeriod short, long, narrow |
    | `h`, `H`, `K`, `k` (1, 2) | `{hour}` | numeric, 2-digit |
    | `m`, `s` (1, 2) | `{minute}`, `{second}` | numeric, 2-digit |
    | `S` (n) | `{fractionalSecondDigits}` | min(n, 3) |
    | `z` (1-3), `zzzz`, `O`, `OOOO`, `v`, `vvvv` | `{timeZoneName}` | short, long, shortOffset, longOffset, shortGeneric, longGeneric |

    Quoted text and other characters are literal. In `chinese` and
    `dangi`, `y` is `{relatedYear}`, as in ICU4X and ICU4C (`en` `"M/d/y"`
    is `"1/1/1984"` with a `relatedYear` part). In `chinese`, `dangi` and
    `iso8601`, which have no eras, `{era}` gives no part. A record also
    keeps the stand-alone flags of its month and weekday, its AM/PM and
    day period widths, and the fields and widths of the CLDR skeleton it
    was made for, which best fit scores. A `B` the request did not ask for
    (a locale whose 12-hour patterns use it, such as `zh-TW`, whose `h` is
    `"Bh時"`) adds no `dayPeriod` field, so `resolvedOptions` does not
    report it. What it shows depends on the matcher, as in ICU:
    - in best fit, a request without `dayPeriod` shows it as AM/PM:
      `{dayPeriod}` becomes `{ampm}` in the `B`'s width (abbreviated for
      `B`, wide for `BBBB`, narrow for `BBBBB`), and the formatter asks
      native code for those AM/PM names. So `zh-TW` `{hour: "numeric"}` is
      `"上午12時"`, `"上午6時"`, `"下午12時"` and `"下午8時"` at 0, 6, 12 and
      20 o'clock, as in ICU4C, not the day periods `"凌晨12時"` and
      `"晚上8時"`. Its range patterns still take CLDR's intervals written
      for the `B` (see "Ranges"), from a copy of the record with the `B`
      to which best fit's step 5 adds the same fields;
    - the style records (`timeStyle`, `dateStyle`) and `formatMatcher:
      "basic"` keep the day period: `zh-TW` `timeStyle: "short"` at 1:07
      is `"凌晨1:07"`, as in ICU4C.
  - **Hour-cycle families.** `[[HourCycle]]` is known before matching
    (CreateDateTimeFormat step 13), so matching uses the records of its
    family: `h11` and `h12` take the entries whose hour is `h` or `K` or
    has a `B` day period; `h23` and `h24` those whose hour is `H` or `k`;
    both take the entries without an hour. A record's [[pattern]] and
    [[pattern12]] are the same string. The draft keys [[formats]] by
    calendar only; the plugin keys it by calendar and family, which is how
    the 12- and 24-hour widths can differ (`en` `"9:07 AM"`, `h23`
    `"09:07"`). A request with a `dayPeriod` and no hour is matched in
    the `h12` family's records whatever [[HourCycle]] is, by either
    matcher: CLDR's records with a day period and another time field are
    all 12-hour, so the 24-hour family has none with a minute or a
    second, and matching there would drop one of the two. The record's
    hour, if it has one, shows by [[HourCycle]], so `en` `{dayPeriod:
    "short", minute: "numeric", hour12: false}` is `"13:07 in the
    afternoon"`; its range patterns are made in the same family.
  - **[[formats]]** of a locale, calendar and family, which
    `formatMatcher: "basic"` runs the draft's BasicFormatMatcher over,
    has:
    - the records of `availableFormats`;
    - a one-field record for each field that no `availableFormats`
      skeleton has alone (`{minute}` numeric, `{second}`, `{dayPeriod}`,
      `{era}`...), `{fractionalSecondDigits}` of each width 1 to 3, and
      `{timeZoneName}` of each of its six widths. Where a locale's `y`
      entry shows the era (`en` japanese `"y G"`), `{year}` uses it, so it
      shows the era and `resolvedOptions` reports it, as the draft says;
    - for every record with `{second}` and no fraction, three with
      `{fractionalSecondDigits}` of width 1, 2 and 3 after `{second}`;
    - for every time record without a `timeZoneName`, one for each
      `timeZoneName` width, with the zone added by the `Timezone` append
      item, so that `{hour, timeZoneName}` keeps its hour;
    - every date record glued to every time record (see "Gluing").

    The draft requires its 14 field sets in the calendar's list, which
    here is the union of the two families. CLDR's patterns with an hour
    and a day period are all 12-hour, so a 24-hour request with both
    drops the day period, as ICU4C does (`en` `{dayPeriod: "short", hour:
    "numeric", hourCycle: "h23"}` is `"09"`, with no `dayPeriod` in
    `resolvedOptions`); the one-field `{dayPeriod}` record is in both
    families, so `{dayPeriod: "short", hourCycle: "h23"}` is `"in the
    morning"`. BasicFormatMatcher's score is a sum over the fields, so the
    best glued record joins the first best date record to the first best
    time record: the plugin finds it without gluing every pair, and a test
    checks that the result is the same.
  - **Best fit** (the default) is the plugin's BestFitFormatMatcher:
    1. Score the `availableFormats` and one-field records with
       BasicFormatMatcher's penalties, by the fields and widths of each
       record's CLDR skeleton, not of its pattern, as ICU matches
       skeletons (`zh` and `ja` write a numeric `M` in their `MMM`
       skeletons' patterns). Three refinements:
       - a field asked for as numeric (numeric or 2-digit) and shown as
         text (narrow, short or long), or the other way, costs 13 more.
         The usual mismatch, numeric to short or long, then costs 19 in
         all: less than an extra field (20), more than three numeric
         width changes (18). So `en-GB` `{year, month, day: "numeric"}` is
         `"05/01/2026"` (`yMd`), not `"5 Jan 2026"` (`yMMMd`), and the
         default of `en-u-ca-hebrew` is `"5 Adar II 5784"` (`yMd`,
         `"d MMM y"`) rather than `yyyyMd`'s `"M/d/y G"`, with its extra
         era;
       - a `timeZoneName` of another kind (specific, offset, generic)
         costs only a small width change (3), so a locale's own layouts
         with `v` serve every zone style, as ICU treats `z`, `O` and `v`
         as one field;
       - the one-field `{timeZoneName}` and `{fractionalSecondDigits}`
         records, which step 5 can add to any record, compete only for a
         request with no other field; otherwise `{timeZoneName}` would
         beat `"HH"` for `{hour, timeZoneName}` and lose the hour;
       - before the score, a record ranks by the number of requested
         fields it lacks that step 5 cannot add (all but `timeZoneName`
         and `fractionalSecondDigits`), fewer first, so a missing field
         that can be added never costs one that cannot: `th`'s data has
         `Hv` `"HH v"` (-123) and `Hm` `"HH:mm"` (-126) but no `Hmv`, so
         `{hour, minute, timeZoneName: "short"}` takes `Hm` and adds the
         zone, `"13:07 UTC"`, rather than drop the minute.

       Ties go, in `chinese` and `dangi`, to a record with
       `{relatedYear}` (`en` has `UMd` `"M/d/U"` before `yyyyMd`
       `"M/d/r"`, and ICU4C shows `"1/1/1984"`); then to the smaller
       width distance, the sum over the requested fields of the
       difference between the requested and the skeleton's letter
       counts (numeric 1, 2-digit 2, short 3, long 4, narrow 5), not
       clamped as the draft's is, so that short is closer to numeric than
       long, as in ICU (`en` chinese's `GyMMMd` beats `GyMMMMd` for a
       numeric month, `"Mo11 17, 2025"`); then to the skeleton first in
       code point order, the order ICU loads them in; then to an
       `availableFormats` record before a one-field one.
    2. If the best record has every requested field, return it with its
       widths adjusted, unless the request has both date and time fields
       and the record has a field the request lacks: ICU then splits, so
       `en-GB` `{weekday: "long", hour: "numeric"}` is `"Monday, 15"`.
    3. Else, if the request has both date fields (`weekday`, `era`,
       `year`, `month`, `day`) and time fields (`dayPeriod`, `hour`,
       `minute`, `second`, `fractionalSecondDigits`, `timeZoneName`),
       match each half by this algorithm and glue them.
    4. Else, if the request has `fractionalSecondDigits` without an hour
       or minute and the best record has no `{second}`, return the
       one-field fraction record joined by a space, fraction first, to
       the best record for the other fields (`en` `{dayPeriod,
       fractionalSecondDigits: 2}` is `"04 in the morning"`). ICU4C
       appends the day period with its append item for a missing field,
       `"04 ├AM/PM: in the morning┤"` (see "ICU4C as a reference").
    5. Else add the missing fields: `fractionalSecondDigits` right after
       `{second}`, preceded by the locale's decimal separator for
       [[NumberingSystem]] (`de` `"9,045"`); `timeZoneName` by the
       `Timezone` append item (`"{0} {1}"`, `"3 PM UTC"`).

    A request with `fractionalSecondDigits` and an hour or minute is
    matched as if it also asked for `second: "numeric"`, as ICU4C does
    (`{minute, fractionalSecondDigits: 2}` is `"07:09.04"`).
  - **Gluing.** The date record's pattern replaces `{1}` and the time
    record's `{0}` in the `atTime` dateTimeFormat (the standard one where
    the locale has no `atTime`), chosen by UTS 35's rule from the
    requested date fields: `full` for a long month and a weekday, `long`
    for a long month, `medium` for a short month, else `short`
    (`"January at 3 PM"`, `"Monday, January 5, 2026 at 3:07 PM"`,
    `"Jan 5, 2026, 9:07 AM"`). The glued record keeps both halves for its
    range patterns.
  - **Width adjustment** (best fit only) follows ICU's, from the matched
    skeleton. A field both the request and the record have is adjusted
    only when the requested width, as a CLDR letter count, differs from
    the skeleton's count for it. The pattern's field then takes the
    requested width if the two are of the same kind (numeric or text);
    otherwise it keeps the pattern's. So a 2-digit hour makes skeleton
    `h`'s `"h"` `"hh"`, but a numeric hour keeps skeleton `H`'s `"HH"`;
    `cs`'s `yMMM` `"LLLL y"` stays long for `{month: "short"}` (as in
    ICU4C, `"červenec 2026"`); and `eu`'s `Ed` `"d, EEEE"` becomes short
    for `{weekday: "short"}`. `timeZoneName` takes the requested value,
    of any kind, and `fractionalSecondDigits` the requested number. A
    `B` the request did not ask for becomes AM/PM at this step (see
    "Records"). `resolvedOptions` reports the adjusted widths, as V8 does
    (`minute: "2-digit"`).
  - **[[styles]].** The DateTime Styles Record's [[Date]] and [[Time]]
    records come from `dateFormats` and `timeFormats`, and its
    [[Connector]] from the `atTime` dateTimeFormats (`dateStyle:
    "full"` with `timeStyle: "short"` is `"Monday, January 5, 2026 at
    3:07 PM"`). A time style whose hour is of the other family than
    [[HourCycle]] is replaced by the best-fit record for its fields and
    widths in [[HourCycle]]'s family (`en` `timeStyle: "short"` with `h23`
    is `"15:07"`, `en-GB` with `hour12: true` `"03:07 pm"`). Its range
    patterns are made by the rules below, like any record's.
  - **Hour cycles.** `[[LocaleData]].[[L]].[[hc]]` is « null, "h11",
    "h12", "h23", "h24" ». For the locale L that ResolveLocale finds, let
    `list` be its hour cycles as `Intl.Locale.prototype.getHourCycles`
    computes them: [[hourCycle]] is `list[0]`, [[hourCycle12]] the first
    `h11` or `h12` in `list` (else `"h12"`), and [[hourCycle24]] the first
    `h23` or `h24` (else `"h23"`). So `ja` with `hour12: true` is `h11`
    (CLDR's JP allows `H K h`), as test262 expects.
  - **Calendars and numbering systems.** `[[LocaleData]].[[L]].[[ca]]` is
    the first calendar of L's CalendarsOfLocale that
    `Intl.supportedValuesOf("calendar")` lists (`th` `buddhist`, `fa`
    `persian`; CLDR 48.2 puts `gregory` first for `ar-SA`, as Node shows),
    then the other supported calendars in that list's order. `"islamic"`
    and `"islamic-rgsa"`, which ICU4X does not implement, are not in it,
    so a request for them resolves to the locale's default calendar,
    which test262 accepts. `[[nu]]` is NumberFormat's list.
  - **Ranges.** For a record R with date fields D, time fields T and the
    pattern P, FB(p) is the parts « startRange p, shared separator,
    endRange p », with the separator the text between `{0}` and `{1}` of
    `intervalFormatFallback` (text around them is shared).
    - **Interval patterns.** An `intervalFormats` entry can serve a part
      of R only if its skeleton has the same fields as that part (widths
      aside; `z`, `O` and `v` count as one field) and, if it has an
      hour, an hour of R's hour-cycle family; of those, the skeleton whose
      widths match best by best fit's penalties is used, the first on
      ties. It is not used if it does not show the same year placeholders
      (`{year}`, `{relatedYear}`, `{yearName}`) as R: `en` chinese
      `y`'s interval `"U – U"` would drop the related years of R's
      `"r(U)"`, so a year difference takes FB(P),
      `"1984(jia-zi) – 2044(jia-zi)"`, as ICU4C gives. An interval for an
      era difference is not used unless both its halves show the era
      (see `ko` in "ICU4C as a reference"). A CLDR interval pattern is
      split at the first field that repeats (UTS 35), and the
      sources follow ICU4C: the fields that appear in both halves are the
      overlap; in each half, from the first to the last overlap field is
      `startRange` or `endRange`, and the rest is `shared` (`"Jan 5 – 9,
      2026"`: the month shared, `"5"` startRange, `" – "` shared, `"9"`
      endRange, `", 2026"` shared).
    - **Interval widths** follow ICU4C's. A field both R and the interval
      pattern show takes R's width only when the letter count R's pattern
      writes it with differs from the interval skeleton's, the interval
      pattern writes it with the skeleton's count, and both are of the
      same kind; otherwise it keeps the interval pattern's own width. So
      `de`'s `yMd` interval `"dd.–dd.MM.y"` stays 2-digit for `"d.M.y"`
      (`"05.–06.01.2026"`), and `en`'s `MMMd` `"MMM d – d"` becomes long
      for `"MMMM d"`. A field the interval pattern writes in two widths
      (`fi`'s `"LLL–LLLL"`) takes R's width if the kinds agree, else
      keeps the first (`fi` `{year, month: "numeric"}` is `"tammi–helmi
      2026"`). `timeZoneName` and `fractionalSecondDigits` keep R's. A
      field the interval pattern shows that R lacks keeps the interval's
      own width and context, and its name is loaded, as in ICU4C (`en`
      hebrew's `"y – y G"` for `"y"`: `"5780 – 5786 AM"`).
    - **The record**, in the draft's field order:
      - [[Default]]: FB(P) if D is not empty; else FB of the best-fit
        pattern for `year`, `month` and `day` numeric plus T, so a
        different day shows the dates (`{hour, minute}` gives `"1/5/2026,
        3:07 PM – 1/9/2026, 5:08 PM"`).
      - [[Era]], [[Year]], [[Month]], [[Day]], if T is empty: CLDR's
        interval pattern for `G`, `y`, `M` or `d`; a field R shows without
        its own pattern takes the nearest larger field's, else FB(P). A
        weekday counts as a day, so `{weekday: "long"}` an hour apart is
        `"Monday"`, not `"Monday – Monday"`. A field R does not show
        widens R where a smaller field would otherwise hide a difference:
        [[Year]] of an R with a month or a day but no year is FB of the
        best fit for R plus `year: "numeric"`, and `month: "numeric"` too
        when R has no month (`"Jan 5, 2026 – Feb 9, 2027"`); [[Month]] of
        an R with a day but no month is FB of the best fit for R plus
        `month: "numeric"` (`{day}` gives `"1/5 – 2/9"`). Other fields R
        does not show are absent, but for [[Era]] (see "Eras" below).
      - [[Era]] to [[Day]], if D and T are both non-empty: FB(P), except
        that each of [[Year]], [[Month]] and [[Day]] whose field R lacks
        widens R, with that field and each smaller one of year, month and
        day that R lacks, numeric (`en` `{weekday: "long", hour}` a day
        apart is `"5 Monday, 9 AM – 6 Tuesday, 9 AM"`, as in ICU4C), and
        [[Era]] as in "Eras" below.
      - If D is empty, [[Era]] to [[Day]] are absent, so a different day
        gives [[Default]], but for "Eras" below.
      - **Eras.** The draft's loop checks [[Era]] first. For an R whose
        pattern shows no era, in a calendar with eras (all but `chinese`,
        `dangi` and `iso8601`), an absent [[Era]] or FB(P) would show a
        range across two eras without them (`en` `{year}` from 1 BC to
        1 AD would be `"1 – 1"`, `{day}` `"1 – 1"`). So [[Era]] is FB of
        the best fit for R plus `era: "short"` and the fields below,
        numeric; R's own fields keep their widths:
        - if T is empty, the fields [[Year]]'s widening adds, if R has a
          [[Year]] (`{year}` gives `"1 BC – 1 AD"`, `{day}` `"6/1/1 BC –
          6/1/1 AD"`, as ICU4C gives). It is made after the other date
          fields, so no field takes it as the nearest larger field's and a
          range within one era is as before. An R without a [[Year]] (a
          weekday alone) has no [[Era]]: `{weekday: "long"}` stays
          `"Thursday – Friday"`, as in ICU4C.
        - if D and T are both non-empty, each of year, month and day that
          R lacks (`{weekday: "short", hour}` gives `"Thu, 6/1/1 BC,
          12 AM – Fri, 6/1/1 AD, 12 AM"`, as ICU4C gives). ICU4C asks for
          its era skeleton's `G`, which keeps a pattern's own era width,
          so `en-u-ca-japanese` there is `"Tue, 4/30/31 H, 12 AM"` where
          the plugin's `era: "short"` gives `"Tue, 4/30/31 Heisei,
          12 AM"`.
        - if D is empty and [[Default]] shows no era, year, month and day
          (`{hour}` gives `"6/1/1 BC, 12 AM – 6/1/1 AD, 12 AM"`, as ICU4C
          gives); [[Year]], [[Month]] and [[Day]] are then [[Default]]'s,
          since once [[Era]] has a pattern the loop would stop at an
          absent [[Year]] and show two days as one. A [[Default]] that
          shows an era (japanese's `"4/30/31 H, 12 AM – 5/1/1 R, 12 AM"`)
          needs no [[Era]].
      - [[AmPm]], [[DayPeriod]], [[Hour]], [[Minute]], [[Second]],
        [[FractionalSecondDigits]], if T is not empty: CLDR's patterns for
        `a`, `B`, `h` or `H`, and `m`; a field R shows without one takes
        FB of R's time part (`"9:07:03 AM – 9:07:10 AM"`); a field from
        [[AmPm]] down that R does not show, but that is larger than the
        smallest field R shows, takes the pattern of the next smaller
        field R shows (AM/PM in a 24-hour pattern takes [[Hour]]'s, so
        `"09:07 – 14:07"`; AM/PM in a pattern with `B` takes
        [[DayPeriod]]'s, `"1:07 in the afternoon – 6:07 in the
        evening"`; [[DayPeriod]] in a pattern without `B` takes
        [[Hour]]'s, `"1:07 – 6:07 PM"`); fields smaller than the smallest
        one R shows are absent, so their differences are not shown. A
        best-fit record whose `B` shows as AM/PM (see "Records") takes
        CLDR's intervals written for its `B`, as ICU4C's interval formats
        do: its [[AmPm]] takes the `B` interval, so an AM/PM difference
        shows the day periods (`zh-TW` `{hour: "numeric"}` from 9:00 to
        20:00 is `"上午9時至晚上8時"`); its [[DayPeriod]] takes [[Hour]]'s,
        whose interval keeps its `B`, so an hour difference shows the day
        period too (`"凌晨1時至3時"`); and its fallbacks, FB of its own
        pattern, show AM/PM. The record with the `B` has the fields best
        fit's step 5 added, so `zh-TW-u-ca-roc` `{hour, minute,
        timeZoneName: "short"}` from 9:07 to 20:07 is `"上午9:07至晚上8:07
        [UTC]"`, as in ICU4C. A style record's `B`, which adds no
        `dayPeriod` field, counts as one when its interval skeleton is
        chosen, so `zh-TW` `timeStyle: "short"` takes CLDR's `Bhm`
        intervals, `"上午9:07–10:07"` and `"上午9:07 – 晚上8:07"`, as ICU4C
        gives, not `hm`'s `"上午9:07至10:07"`; if no skeleton has those
        fields, the record's own fields choose. If D is not empty,
        each of these patterns is joined to the date part, shared, by
        the locale's medium standard dateTimeFormat, as ICU4C's
        interval formats join them (`"Jan 5, 2026, 9:07 – 10:07 AM"`,
        `"Monday, 9 – 10 AM"`; `it` with `dateStyle: "long"` and
        `timeStyle: "short"` is `"5 gennaio 2026, 09:07–11:07"`, though
        its long connector has no comma).

      The draft's loop needs this coverage: once a field has a pattern, a
      smaller field without one stops the search and its difference is
      not shown. A test runs the loop over every pair of hours of a day
      for the field sets `h`, `H`, `Bh`, `hm`, `Hm`, `Bhm` and `hms`, and
      checks that no two different times give the same string and that
      no two times of the same day select [[Default]]. Another
      (`datetime-format-fields-*.js`) checks that best fit keeps every
      requested field, over every supported calendar, ten locales (`th`,
      `fa`, `ja`, `he`, `zh-TW`, `zh-HK`, `de`, `en`, `en-GB`, `ar`), 17
      option bags of time fields (with day periods, fractions of a second
      and two with date fields), each without a zone name and with three,
      and `hour12` unset, true and false: each field is in
      `resolvedOptions`, is a part of the format and of a range within a
      day, and is in both halves of a range over two days. The one
      exception is the rule above: a 24-hour `dayPeriod` with an hour is
      dropped.
  - **ICU4C as a reference.** The plugin's results were compared with
    Node 24 (ICU 78.2, CLDR 48.0) over thousands of option bags, locales,
    calendars and ranges. Apart from CLDR 48.0 against 48.2 (`ko`'s AM/PM
    names are `"오전"` and `"오후"` in 48.2, where Node prints `"PM"`), the
    differences are these, each with its reason:
    - **U+202F.** CLDR 48.2 puts U+202F before `en`'s AM/PM and in other
      places (`ru` `"2026 г."`), and the plugin keeps it, as the draft
      and CLDR do. V8 replaces it with a space in `format()`, the Date
      methods and a collapsed `formatRange()`, for web compatibility, but
      not in its parts, so V8's parts do not join to its `format()`
      string. test262's `prototype/format/numbering-system.js` expects
      V8's replacement and is an expected failure.
    - **`iso8601`** uses CLDR root's patterns with Gregorian names, the
      only `iso8601` data CLDR 48.2 has (`zh`'s intervals aside):
      `dateStyle: "long"` is `"2026 January 5"`, where ICU4C prints
      `"2026  5"`, and `ar-EG` joins a full date to a time with root's
      `"{1} {0}"`, where ICU4C uses `"في"`.
    - **`japanese` before Meiji** uses ICU4X's eras, `ce` and `bce`, where
      ICU4C has every historical era: `ja-u-ca-japanese` 1850-01-01 is
      `"西暦1850年1月1日"`, ICU4C `"嘉永3年1月1日"`.
    - **`ko` intervals for an era difference.** CLDR 48.2's `ko`
      `GyMMMd` and `GyMd` intervals for `G`, `"G y년 M월 d일 ~ y년 M월
      d일"`, lack the second era, and ICU4C shows them as they are
      (`ko-u-ca-japanese` from Heisei to Reiwa is `"헤이세이 31년 4월 30일
      ~ 1년 5월 1일"`). The plugin uses an interval for [[Era]] only if
      both halves show the era, else FB(P): `"헤이세이 31년 4월 30일 ~
      레이와 1년 5월 1일"`.
    - **Years before a calendar's epoch** follow the draft's 1 − year
      step: `fa` in June of the year 2 is `"۶۲۰"`, where ICU4C gives
      `"‎−۶۱۹"`.
    - **Zone names far from the present.** ICU4X keeps a zone name
      timestamp only between about 1970 and 2448, so names before 1970
      come from the zone's metazone of 1970, and names after 2448 from its
      last one: `America/New_York` in July 1950 is `"EDT"` and in July
      275000 `"Eastern Daylight Time"`, where ICU4C gives `"GMT-4"` and
      `"GMT-04:00"`; `Asia/Kolkata` in 1950 is `"India Standard Time"`,
      ICU4C `"GMT+05:30"`. An offset that matches neither of the
      metazone's still gives the GMT format, so the generic names of
      `America/New_York` in 1800 are `"GMT-4:56:02"` and `"GMT-04:56:02"`,
      where ICU4C gives `"New York Time"`. `UTC`'s generic names are
      ICU4X's `"UTC"` and `"Coordinated Universal Time"`, ICU4C's
      `"GMT+0"` and `"GMT+00:00"`.
    - **`ar` zone names.** ICU4X writes the localized GMT format with the
      locale's plus and minus signs for `latn`, so `ar` and `ar-EG` names
      have U+200E before the sign (`"غرينتش‎+5:30"`), where ICU4C has
      none. Rewriting ICU4X's sign would mean reimplementing its offset
      format.
    - **Numbering overrides.** Some CLDR date patterns name a numbering
      system for their numbers: `he` hebrew's Hebrew letters (`hebr`),
      `ja` japanese's `元年` for year 1 (`y=jpanyear`), and in `chinese`
      and `dangi`, `hanidec` in `ja`, `zh-SG`, `zh-Hans-HK` and
      `zh-Hans-MO` and Chinese day numerals (`d=hanidays`) in `zh`,
      `zh-Hant` and `yue`. The plugin leaves them out, because the draft
      formats every number with [[NumberingSystem]], and `hebr`,
      `jpanyear` and `hanidays` are not numbering systems ECMA-402 has:
      `he-u-ca-hebrew` `dateStyle: "full"` is `"יום שישי, 5 באדר ב׳
      5784"`, where ICU4C writes `"ה׳ באדר ב׳ תשפ״ד"`; `ja-u-ca-japanese`
      2019-05-01 is `"令和1年5月1日"`, ICU4C `"令和元年5月1日"`; and
      `zh-u-ca-chinese` `dateStyle: "long"` ends in `"十一月17"`, ICU4C
      `"十一月十七"`.
    - **`Y` and `e`** in CLDR's data are read as `y` and `E` (see
      "Sources"), so week-based years near January 1 differ in those
      patterns, and `ko`'s intervals with `e요일` show the weekday name
      (`"월요일"`), where ICU4C shows a numbered day of the week.
    - **`chinese` and `dangi` date-time connectors** are CLDR's: root
      gives `chinese` its own `"{1} {0}"` with its `atTime` an alias of
      it, and `dangi` aliases `chinese`, so `he-u-ca-chinese` with
      `dateStyle: "medium"` and `timeStyle: "short"` is `"2025 M11 17
      12:00"`, ICU4C `"2025 M11 17, 12:00"`. ICU's choice depends on its
      own resource tables: the best rule derived from CLDR matched Node in
      93% of 2,537 connectors, with 170 exceptions it does not explain.
    - **`ko` dangi `y`** is the related year, as CLDR's patterns and ICU4X
      have it and as the comparison of years in ranges needs, so
      `ko-u-ca-dangi` `dateStyle: "medium"` is `"2025. 11. 17."`, where
      ICU4C shows the cyclic year, `"42. 11. 17."`.
    - **A lone 2-digit minute or second** is 2-digit: `{minute:
      "2-digit"}` is `"07"`, where V8 gives `"7"`. ICU adjusts minute and
      second widths only with options V8 does not pass; the plugin
      applies the width adjustment to every numeric field.
    - **An era with a date that has no year.** `{weekday: "short", era:
      "short"}` is `"Mon, 1/5/2026 AD"`, where ICU4C appends the era to
      the weekday (`"AD Mon"`); likewise `{month: "short", era: "short"}`
      is `"Jan 2026 AD"`, ICU4C `"AD (month: Jan)"`. ICU appends the
      missing fields to its best match; best fit here keeps a date record
      with extra fields for a date-only request. Best fit is left to the
      implementation, and appending date fields would be a mechanism of
      its own.
    - **A fraction of a second without an hour or minute** is joined
      before the other fields by a space (`en` `{dayPeriod,
      fractionalSecondDigits: 2}` is `"04 in the morning"`), where ICU4C
      appends the missing field with its append item and gives `"04
      ├AM/PM: in the morning┤"`, text that is not meant to be shown.
    - **`resolvedOptions`** reports what the matched record shows: an era
      calendar's `{year}` reports its era, and
      `{hour, fractionalSecondDigits: 3}` reports `minute` and `second`
      too, where V8 reports only the requested fields.
    - **Narrow months with a long weekday and a year.** The 13 more best
      fit charges a field shown in the other kind (see "Best fit") and
      ICU's distance rank some records differently:
      `{weekday: "long", year, month: "narrow", day}` gives a narrow text
      month in `fi` (`"sunnuntai 5. H 2026"`, ICU4C `"sunnuntaina
      5.7.2026"`) and a numeric one in `bg`, `lt` and `sk` (`sk` `"nedeľa
      5. 7. 2026"`, ICU4C `"nedeľa 5. j 2026"`).
    - **Ranges.** Five differences come from the rules above: a day
      period difference is shown (`"1:07 in the afternoon – 6:07 in the
      evening"`, ICU4C `"1:07 – 6:07 in the evening"`); 2-digit requests
      keep 2 digits (`"01/05/26 – 01/06/26"`, ICU4C `"1/5/26 – 1/6/26"`);
      a year difference is shown for an R without a year (`{month:
      "long"}` is `"January 2026 – February 2027"`, where ICU4C's
      `"January – February"` hides the year); a leap month differs from
      its base month (see "Date and time formatting"); and an interval
      pattern serves every width of its fields (`de` `{month: "long"}` is
      `"Januar–Februar"`, ICU4C `"Januar – Februar"`, and `fi` `{year,
      month: "numeric"}` `"tammi–helmi 2026"`, ICU4C `"1.2026–2.2026"`).
      Three come from the draft or from rules no single choice can match:
      `fractionalSecondDigits: 2` compares hundredths, as the draft's
      step does, so .045 and .049 collapse to `"9:07:03.04 AM"`, where
      ICU4C shows both ends; `{month: "long", hour}` a month apart is
      `"January at 9 AM – February at 9 AM"`, where ICU4C adds the day in
      `en`, `de` and `fr` but not in `ja`; and `{year, hour}` a day apart
      is `"Jan 5, 2026, 9 AM – Jan 6, 2026, 9 AM"`, where ICU4C appends
      the day to the year (`"2026 (day: 5), 9 AM – 2026 (day: 6), 9
      AM"`). `en` `{dateStyle: "full", timeStyle: "full"}` ranges keep the
      time style's long zone name, where ICU4C shows `"EDT"`.
    - **`zh-HK` and `zh-MO` hour ranges.** A best-fit `{hour}`-style
      record shows its `B` as AM/PM and takes CLDR's `h` interval, which
      writes `a`: from 9:00 to 20:00 is `"上午9時至下午8時"`, where ICU4C
      gives `"上午9時至晚上8時"`.
    - **`zh-TW` `timeStyle: "short"` with `h11`** keeps the style's `B`
      (`"凌晨0:07"`), where ICU4C rebuilds the pattern for the hour cycle
      and shows AM/PM (`"上午0:07"`): a style is replaced only across
      hour-cycle families (see "[[styles]]"). Its ranges take the `Bhm`
      intervals (`"上午9:07 – 晚上8:07"`), ICU4C the `hm` ones
      (`"上午9:07至晚上8:07"`).
    - **A minute with a time zone name.** `{minute, timeZoneName:
      "short"}` is `"1:07 PM UTC"` (`hmv`, with an hour added), where
      ICU4C gives `"7 UTC"`: ICU's distance charges a field the pattern
      adds more than one it lacks, and then appends the zone, while best
      fit keeps the draft's penalties (an added field 20, a missing one
      120). Likewise a `dayPeriod` with a minute or second and no hour
      shows an hour (see "Hour-cycle families"), where ICU4C appends the
      day period with its append item, `"7 ├AM/PM: in the afternoon┤"`.
    - **`Africa/Casablanca`** is `+00:00` all year from 2026-09-20 on, as
      tzdb 2026e has it (Morocco's end of permanent daylight saving time);
      Node's older tzdata keeps `+01:00` with a `+00:00` month around
      Ramadan.
    - **A repeated hour.** A range over the hour a daylight saving time
      transition repeats (`America/New_York` 2026-11-01 05:30Z to
      06:30Z) is one time, `"1:30 AM"`, because the draft compares local
      fields, and the zone name, which is not one of them, does not
      separate the ends (`"1:30 AM EDT"`); Node gives the same.
    - **The ends of the time range.** Node 24 prints only `","` for
      `en-u-ca-chinese` `dateStyle: "full"` at 8.64 × 10^15; the plugin
      gives `"Saturday, Seventh Month 30, 275760(geng-shen)"`.
- **ListFormat** (Phase 5). ICU4X 2.3's `ListFormatter` (`try_new_and`,
  `try_new_or` and `try_new_unit` for the types `conjunction`,
  `disjunction` and `unit`, with `ListLength::Wide`, `Short` and `Narrow`
  for the styles `long`, `short` and `narrow`) has data for every locale
  of `locales.txt`, every type and every style, and the conditional
  patterns UTS 35 describes (Spanish `y`/`e` and `o`/`u`, Hebrew
  `ו`/`ו‑`). Those are compiled into its data, not kept as CLDR text, and
  they test only the last element, through the `end` and `pair` patterns.
  ICU4X never reorders elements. So:
  - CreatePartsFromList gives « » for no element and « element » for one,
    without native code. For two or more it makes one call,
    `listFormatLiterals(lf, isPair, last)`: native code formats a list of
    empty elements (two for a pair, four otherwise) whose last element is
    `last`, keeps the text of every top-level literal part, and returns
    the 3 or 5 pieces (see "C++ JSI layer"). JS puts them between the
    original element strings, repeating the middle piece between inner
    elements. An empty piece gives no part, as the draft's PartitionPattern
    gives none; an empty element is still an `element` part, as the draft
    says (Node drops it).
  - The draft's [[Templates]], and the index n of CreatePartsFromList that
    it leaves to the implementation, are what native code chooses. The
    slots JS keeps are [[Locale]], [[Type]] and [[Style]].
  - Two differences from ICU4C follow ICU4X's data. In the `conjunction`
    type, Hebrew before a non-Hebrew element is `" ו‑"` with U+2011, where
    ICU4C and UTS 35's example have U+002D; ICU4X's rule exists for that
    type only, so the `unit` type keeps CLDR's `"ו-"` with U+002D. And
    Spanish `"11.000"` and `"11,5"` take `u` after an `o`-joined list (UTS
    35's rule), where ICU4C gives `o`.
- **RelativeTimeFormat** (Phase 5).
  - **Data.** `icu_experimental` 0.6's 24 markers
    `{Long,Short,Narrow}{Second,Minute,Hour,Day,Week,Month,Quarter,Year}RelativeV1`
    (`RelativeTimePatternData`: the relative names, and the past and
    future patterns), read with `number.rs`'s loader and plural category
    helpers. They load for every locale of `locales.txt`, and the research
    found them equal to CLDR 48.2's `dateFields.json` in every value, so no
    file is generated. `relativeTimePatterns` returns them; a category the
    locale lacks gets "other"'s pattern. The style `long` gives the draft's
    fields `"second"` to `"year"`, and `short` and `narrow` its fields
    `"second-short"` and so on.
  - `[[NumberFormat]]` is `CreateNumberFormat(locale, {__proto__: null,
    numberingSystem: nu})`, and `[[PluralRules]]`
    `CreatePluralRules(locale, undefined)` (see "Resistance to
    tampering"). The `nu` list of `[[LocaleData]]` is NumberFormat's.
  - **Documented deviation: the magnitude.** The draft's
    PartitionRelativeTimePattern formats ℝ(value) with its sign, so
    `format(-1, "day")` would be `"-1 day ago"`. test262 and every engine
    format the magnitude, and the past pattern carries the sense. The
    plugin formats the absolute value, with -0 as +0, never `'negZero'`,
    which would show `"-0"` (`format(-0, "day")` is `"0 days ago"`). The
    plural category is ResolvePlural's for the signed value, which the
    sign does not change.
  - **Documented deviation: the conversion.** Step 11 of
    PartitionRelativeTimePattern formats ℝ(value), the exact value of the
    Number. The plugin converts the Number as NumberFormat does
    (ToIntlMathematicalValue, which reads Number::toString), so
    RelativeTimeFormat shows the same digits as the plugin's own
    NumberFormat. Engines agree on large integers:
    `format(1000000000000000100, "day")` is `"in
    1,000,000,000,000,000,100 days"`, as in Node, although the Number is
    exactly 1000000000000000128. They differ on some fractions:
    `format(1.0005, "day")` is `"in 1.001 days"`, since `"1.0005"` rounds
    half-expand to 1.001, while the exact value (1.000499999…) would round
    to 1; V8 gives `"in 1 day"` there although its NumberFormat gives
    1.001. DurationFormat keeps its exact integral handling.
  - A pattern without `{0}` (ar `"قبل ثانية واحدة"`) gives one literal
    part, as MakePartsList says. Every part of the number has the `unit`,
    the literals inside the number pattern included; the relative time
    pattern's own literals do not.
  - gd's `"an-eararais"` for `format(3, "day")` with `numeric: "auto"` is
    CLDR's name for +3 days, which the draft uses; V8 prints a number.
- **DisplayNames** (Phase 5). `of` makes one native lookup per name, and
  there is no native state; the draft's [[Fields]] is what the lookups
  read.
  - **Names.** Language, dialect, region, script and variant names come
    from `icu_experimental` 0.6's data, read as data structs:
    `LanguageDisplayNamesV1` (a language subtag), `LocaleDisplayNamesV1`
    (the dialect names, keyed by `language-script` or `language-region`),
    `RegionDisplayNamesV1`, `ScriptDisplayNamesV1` and
    `VariantDisplayNamesV1`. Currency names come from
    `CurrencyDisplaynameV1` (the singular display name, with the ISO code
    as the marker attribute; Phase 3's currency data has only the plural
    names). For language, dialect, region and script names, the style
    `long` reads `names`, and `short` and `narrow` read `short_names`
    (CLDR's `alt="short"`), then `names`, as V8 does. Variants have no
    short names, and currency names have one style.
    `long_names` and `menu_names` are never used, and neither is ICU4X's
    `LocaleDisplayNamesFormatter`, whose separators are fixed English text
    and which cannot report a missing part.
  - **Names equal to their codes.** ICU4X's data leaves out every name
    that equals its code (en's short region `"US"` and `"UN"`, the script
    `"Thai"`). `display-names.txt` lists them as `eq` entries, and a lookup
    that ICU4X misses gives the code when the file lists it for the
    locale's chain, kind and style. For short and narrow the order is
    `short_names`, the listed short entry, `names`, the listed long entry;
    for long, `names`, then the listed long entry. An `eq` entry applies
    only where ICU4X has no name, so a locale may inherit one for a code
    that it names itself: `nn`'s `tiv` is "tivi" under `no`'s code-equal
    "tiv", and `hi-Latn`'s short `US` is "America" under `en-IN`'s "US".
    Variant codes in the file are CLDR's uppercase keys (`PINYIN`), so a
    variant subtag is uppercased before it is looked up there.
  - **Calendar names, date-time field names and the locale display
    pattern** come from `display-names.txt` alone; ICU4X has none of
    them. Only the 16 calendars of `supported.rs` and the aliases
    `ethiopic-amete-alem` and `islamicc` have names, an alias its
    canonical calendar's; `islamic` and `islamic-rgsa`, which CLDR names,
    have none, because test262's `calendars-accepted-by-DisplayNames.js`
    requires a name for exactly the supported calendars. Calendar names
    have one style. Field names have three (CLDR's `displayName` of
    `<field>`, `<field>-short` and `<field>-narrow`), with the draft's
    codes mapped to CLDR's fields: `weekOfYear` is `week`, `dayPeriod`
    `dayperiod`, `timeZoneName` `zone`, and the rest the same word.
  - **The file.** `update-cldr-lists.py` writes `display-names.txt`
    (252 KB) from `cldr-localenames-full` and `cldr-dates-full`, in
    `day-periods.txt`'s layout: `locale <tag> <parent>` lines, with the
    parents of `units.txt`, each followed by `<key>\t<value>` lines for
    what differs from the parent chain. The keys are `ca.<calendar>` (BCP
    47 ids; CLDR's `gregorian` is `gregory`, `ethiopic-amete-alem`
    `ethioaa`), `f.<code>.<style>` for the 12 draft codes (`displayName`,
    not `displayName-alt-variant`), `lp` and `ls` (`localeDisplayPattern`'s
    `localePattern` and `localeSeparator`, with `{0}` and `{1}`), and
    `eq.<kind>.<style>.<code>` with an empty value, for each name in
    `languages.json`, `territories.json`, `scripts.json` and
    `variants.json` that equals its code (`kind` is `language`, `dialect`
    for a language key with `-`, `region`, `script` or `variant`; `style`
    is `long`, or `short` for an `-alt-short` entry). The generator fails
    unless `und` has `lp`, `ls` and all 36 field names and `en` names
    every supported calendar, and unless reading every locale back gives
    its CLDR entries, apart from the inherited `eq` entries above. The
    entry for a locale is the first one ICU4X's `LocaleFallbacker`
    reaches from its language identifier.
  - **Language names are composed in JS** by UTS 35's algorithm (Part 2,
    "Locale Display Name Algorithm"), which matched Node in all 110,160
    cases of the research:
    1. the code is CanonicalCodeForDisplayNames's result, split into
       language, script, region and variants;
    2. with `languageDisplay: "dialect"` and exactly one of script and
       region, the dialect name of `language-script` or `language-region`
       is looked up; on a hit, that subtag is used up. With both a script
       and a region, or with `"standard"`, there is no dialect lookup;
    3. otherwise the base is the language name of the language subtag;
    4. the qualifiers are, in order, the script name, the region name and
       each variant's name, in the instance's style (short and narrow use
       the short names: `"English (Latin, UK)"`). In each, `(` and `)`
       become `[` and `]`, and `（` and `）` become `［` and `］`;
    5. the qualifiers are joined with the locale's `localeSeparator`
       (`"{0}, {1}"`), and the base and the qualifiers with its
       `localePattern` (`"{0} ({1})"`);
    6. if the base or any qualifier has no name, the code has no name:
       `of` returns the canonical code with `fallback: "code"` and
       `undefined` with `"none"`, as the draft says (V8 gives a partial
       name such as `"English (ZY)"`).
  - The draft canonicalizes region, script and calendar codes before the
    lookup, so `of("us")` with `type: "region"` is `"United States"` (V8
    gives `undefined`). `"und"` has CLDR's name, `"Unknown language"` (V8
    gives `undefined`).
  - Some locales whose CLDR parent is root have no name for common codes:
    CLDR 48.2 has none for the region `US` in 22 locales of `locales.txt`,
    the script `Latn` in 4, the calendar `gregory` in 25 and the currency
    `USD` in 28, and Node lacks the same names in the same locales. A Rust
    test pins these lists; every other locale but `und` has a name for
    `en`, `US`, `Latn`, `gregory`, `USD` and `year`.
- **DurationFormat** (Phase 5). The draft step by step, with these
  decisions:
  - `[[DigitalFormat]]` is per numbering system, and the draft's
    constructor never picks the entry of the resolved one; the plugin
    reads the entry of `[[NumberingSystem]]`. Its
    `[[HourMinuteSeparator]]` and `[[MinuteSecondSeparator]]` are both
    CLDR's `timeSeparator` for the language identifier of `[[Locale]]` and
    `[[NumberingSystem]]`, entry 21 of `numberSymbols`, as V8 shows (`da`
    `"1.02.03"`). `update-cldr-lists.py` adds it as the last column of
    `number-symbols.txt` (from `symbols-numberSystem-<nu>.timeSeparator`
    of `cldr-numbers-full`) and to the explicit symbols of
    `explicit-number-systems.txt`, and it resolves along the chain as the
    other symbols do (see "Numbering systems" under NumberFormat). In
    CLDR 48.2 a locale's own numbering systems have `":"`, except latn
    `"."` in da, da-GL, en-DK, en-FI, fi, id, ms-ID, si, su and sv-FI,
    and arabext `"٫"` in ug, ur and ur-IN. Another numbering system takes
    the explicit value of the nearest locale of the CLDR chain, root
    included: root's arabext `"٫"` (`en-u-nu-arabext` gives
    `"۱٫۰۲٫۰۳"`), `"."` for arab and arabext in `no` and its children
    (`no-u-nu-arab` `"١.٠٢.٠٣"`), and `"："` (U+FF1A) for fullwide in
    `sv` and its children (`sv-u-nu-fullwide` `"１：０２：０３"`); every other
    explicit value is `":"`. Where the chain has none, it is the locale's
    latn separator (`da-u-nu-deva` `"१.०२.०३"`).
  - `[[TwoDigitHours]]` is false for every locale. The step of
    GetDurationUnitOptions that turns hours into `"2-digit"` when it is
    true applies whatever the style, so a true value would pad the hours
    of the long, short and narrow styles too. V8 never pads hours either.
  - Every field is formatted with its exact value. ToIntegerIfIntegral
    returns +0 for -0. An integral field becomes an Intl mathematical
    value from the BigInt of its magnitude, never from its shortest
    decimal string (`milliseconds: 4503599627370497000` is exactly
    4503599627370497024 and formats so, where Node prints
    `"…497,000 ms"`). IsValidDuration and ComputeFractionalDigits are
    exact: the sum is a BigInt count of nanoseconds, compared with 2^53 ×
    10^9, built from the BigInt of each integral field and BigInt literal
    powers of ten.
  - Numbers go through `CreateNumberFormat` and PartitionNumberPattern,
    with null-prototype option records, and the list through
    `CreateListFormat` and `CreatePartsFromList` (see "Resistance to
    tampering"). The list is of type `unit`, so a Hebrew list of units
    keeps CLDR's `"ו-"` (see "ListFormat").
- **Segmenter** (Phase 5).
  - **Segmenters.** `GraphemeClusterSegmenter::new()`, and
    `SentenceSegmenter::try_new` and `WordSegmenter::try_new_dictionary`
    with the language identifier of `[[Locale]]` as content locale (ICU4X
    2.3 tailors only `el` sentences; passing the locale follows any later
    data). The word segmenter is the dictionary one: CLDR's dictionary for
    Chinese and Japanese and ICU4X's dictionaries for Thai, Lao, Khmer and
    Burmese, as ICU4C uses. ICU4X's default word segmenter
    (`try_new_auto`, with LSTM models for the Southeast Asian scripts) is
    1.46 MB smaller but about 120 times slower on Thai (2.34 s against
    19 ms for 414,000 code units of ordinary Thai text) and differs from
    ICU4C for Lao and Burmese; correctness and speed come before size
    (Guiding rule 2). The five dictionaries are 3,780,186 bytes of the
    library's read-only data (2,007,110 for Chinese and Japanese,
    1,773,076 for the other four). They were found by size: the library's
    five largest constants match the sizes ICU4X's baked data states for
    the dictionaries (2,007,134 and 1,773,172 bytes) within 24 bytes each.
    The rest of the segmenters' share was not separated; in a probe binary
    the three segmenters with their rules added 3.9 MB.
  - **Known limitation: long unbroken runs.** ICU4X 2.3's word segmenter
    takes time quadratic in the number of words of an unbroken run of
    Chinese, Japanese or Thai text. It segments the run with the
    dictionary once (`word.rs:961` for UTF-8), then after every break
    rebuilds its vector of the breaks still pending
    (`rule_segmenter_v1.rs:59` in `icu_segmenter` 2.3). So a crafted
    200,000-code-unit run takes tens of seconds. Measured in the Debug
    ASan build, through `containing`: `"我爱北京天安门".repeat(30000)` (210,000
    code units) takes 21.9 s, and `"ก"` followed by 199,999 tone marks
    U+0E48 (200,000 code units) takes 60 s. ICU4X 2.4 fixes it with a
    cache that keeps the pending breaks by distance
    (`rule_segmenter_v1.rs:21`, `:131`, `:158` in `icu_segmenter` 2.4).
    The plugin gets the fix when it moves to ICU4X 2.4 (CLDR 49): Cargo
    cannot mix ICU4X 2.3 and 2.4 crates (Cargo keeps one 2.x version of a
    crate, the 2.3 and 2.4 crates require `icu_collections` and
    `icu_locale_fallback` at their own minor version, and `icu` 2.3 itself
    requires `icu_segmenter` 2.3). Grapheme and sentence segmentation, and
    words in ordinary text, are not affected.
  - **Boundaries.** ICU4X's iterators only go forward and borrow their
    input, and the rules need unbounded left context, so native code
    computes every boundary of a Segments object's string once (see
    "Native state") and keeps the ascending boundaries from 0 to the
    length, and for word granularity whether each segment is word-like:
    4 bytes per segment, and one more per word segment, while the Segments
    object lives. FindBoundary is a binary search.
  - **ICU4X 2.3 word bugs** are worked around in native code, all found
    against Node. Each goes when an ICU4X upgrade fixes it (2.4 rewrote
    the UTF-16 path):
    - the UTF-16 word path truncates characters above U+FFFF in Chinese
      and Japanese runs (`word.rs:988`, `:991`), so word segmentation
      runs on UTF-8 (see "Strings"): `"日本語𠀀テスト"` is `日本語|𠀀|テスト`,
      as in Node;
    - **boundaries before ignored characters.** ICU4X puts word
      boundaries before characters that UAX #29's rule WB4 ignores
      (Word_Break Extend, Format or ZWJ): its Thai dictionary splits the
      tone mark U+0E48 of `"ช่"` from its base, and a soft hyphen after
      Thai (`"ก\u00AD"`) becomes a segment of its own. Native code repairs
      them: after ICU4X's word segmentation, a boundary at index i (0 < i
      < length) is dropped when the code point after it has Word_Break
      Extend, Format or ZWJ and the code point before it is not CR, LF or
      Word_Break Newline (WB3a keeps that one), and the segments on either
      side become one, word-like if either was. This repairs only
      boundaries before ignored characters; it is not the whole of WB4,
      and it uses no grapheme or Line_Break data. U+0E33, which is
      Grapheme_Cluster_Break SpacingMark but Word_Break Other, keeps its
      boundary (`" ำ"` is two segments, as in Node), and so does a
      character after a Grapheme_Cluster_Break Prepend character (U+0600,
      a space and `x` are three segments);
    - **the word-type re-check.** A segment that ICU4X reports not
      word-like and that ends in a code point with Word_Break Extend,
      Format, ZWJ or Single_Quote is segmented again without all of its
      trailing such code points, and takes the word type at the end of
      that: a word followed by more text and ending in a mark (decomposed
      `"café x"`, Hindi `"की एस"`, Arabic with harakat), a word that ends
      in ZWJ even at the end of the text, and a Hebrew word that keeps
      its apostrophe by WB7a (`"מס'"`). This set is separate from the
      repair's: an apostrophe after a word is still a segment of its own
      (`"x'"`, `"؀'"`). One difference from Node follows:
      `"מס'\u0301 x"` is word-like, where ICU4C's rule status for a
      Hebrew letter and an apostrophe does not survive the mark;
    - **segments made only of ignored characters.** Before the repair
      merges the word-like flags, a segment made only of Word_Break
      Extend, Format or ZWJ code points is word-like only if one of them
      is Ideographic: `"\u0E48"` alone, and `" \u0E48"` after the repair,
      are not word-like, as in Node, and U+16FE4 KHITAN SMALL SCRIPT
      FILLER, which is Extend and Ideographic, is.
  - **Objects.** %IntlSegmentsPrototype% has `containing` (length 1) and
    `[Symbol.iterator]` (length 0, `name` `"[Symbol.iterator]"`) and no
    `@@toStringTag`. %IntlSegmentIteratorPrototype% inherits from
    %Iterator.prototype% and has `next` (length 0) and the
    `@@toStringTag` `"Segmenter String Iterator"`. Segments objects and
    iterators are made with `ObjectCreate` of these prototypes, and their
    slots are private fields. CreateSegmentDataObject makes one of two
    object literals, with or without `isWordLike`, so the properties come
    in the order `segment`, `index`, `input`, `isWordLike`.
  - Differences from ICU4C, which the draft leaves to the
    implementation: some Thai splits where ICU4X's dictionary lacks a word
    (`intl/test/lit/segmenter-text.js` pins one), Korean next to Latin
    letters and U+00B7 follows UAX #29 (`"MB가"` is one word, where ICU4C
    splits it), `"_"` alone is word-like, and a period between a Latin
    letter and Thai splits (`"a.ำ"` is `a|.|ำ`, where ICU4C treats Thai
    letters as ALetter and keeps one word).
- **Locale information** (Phase 1, the `Intl.Locale` information methods and
  Add / Remove Likely Subtags) comes from these sources:

  | Operation | Data |
  |---|---|
  | Add / Remove Likely Subtags | ICU4X `LocaleExpander::new_extended()`; minimizing favors the region, as UTS 35 does |
  | Calendars | CLDR `calendarPreferenceData`, with CLDR calendar names mapped to BCP 47 types (`gregorian` is `gregory`); a region with no line gets the `001` line; `calendar-preferences.txt` |
  | Hour cycles | CLDR `timeData`, by region or by language and region; `preferred` first, then `allowed`, without repeats; `h`, `hb` and `hB` give `h12`, `H` gives `h23`, `K` gives `h11`, `k` gives `h24`; a region with no line gets the `001` line, a language-region key with no line gets nothing; `hour-cycles.txt` |
  | Time zones | CLDR `bcp47/timezone.xml`: a zone belongs to its `region` attribute, or else to the first two letters of its id; `utc`, `gmt`, `unk`, `utce*` and `utcw*` belong to no region; `time-zones-by-region.txt` |
  | Week information | ICU4X `WeekInformation`, with its fallback to `001` |
  | Text direction | ICU4X `LocaleDirectionality::new_extended()` on the script |
  | Numbering systems | the numbering system of the found locale's decimal symbols data |
  | Collations | the collation types in `COLLATIONS` (`supported.rs`) that have a tailoring for the found locale or a locale on its fallback chain down to `und` |

  The three CLDR files are generated by `update-cldr-lists.py`. Each time
  zone is given as its primary identifier, through the same canonicalization
  as `Intl.supportedValuesOf("timeZone")`, so the two agree. CLDR's first
  alias is not always the primary identifier (`inccu` lists `Asia/Calcutta`
  first).

  One rule decides whether week, hour-cycle or calendar-preference data
  "are available" for a region: they always are. CLDR's `001` entry is the
  default for every region its supplemental data does not list (UTS 35
  Part 4, "Week Data", "Time Data" and "Calendar Preference Data"), so a
  region with no entry of its own has the `001` data. A language-region key
  of the hour-cycle data (`en-AQ`) is not a region: with no line of its own
  it has no data, and HourCyclesOfLocale goes on to the region. The draft's
  last-resort `"h23"` and `"gregory"` steps stay, reached only if `001`
  itself had no data (or, for calendars, if none of the calendars found is
  available).

  - Calendars and hour cycles: `calendarPreferences` and `hourCycles` give
    the `001` line for a region with no line of its own. So
    `th-TH-u-rg-aqzzzz` gets `001`'s calendars, not Thailand's, and
    `en-AQ` gets `001`'s hour cycles.
  - Week data: ICU4X drops region entries equal to `001`, so an exact load
    could not tell "no data" from "same as `001`" (`en-US-u-rg-gbzzzz`
    would get US data instead of GB's). WeekInfoOfLocale therefore takes
    the region override if there is one, else the region, and loads with
    fallback.

  `Intl.Locale.[[LocaleExtensionKeys]]` is « "ca", "co", "fw", "hc", "kf",
  "kn", "nu" ». Phase 2's Collator supports `kf` and `kn`, so the draft's
  condition for including them holds, and `caseFirst` and `numeric` exist as
  getters. The draft is newer than the pinned test262 in two normative
  changes, which the plugin follows: CollationsOfLocale has no default-locale
  fallback (tc39/ecma402#1072), and WeekInfoOfLocale goes through
  RegionPreference (#1051).
- **Keyword and field value aliases.** Canonicalization (UTS 35 Annex C)
  replaces a `-u-` keyword value or `-t-` field value that CLDR's
  `common/bcp47` lists as an alias of its key: a deprecated type maps to its
  `preferred` value, and each `alias` of a type that is a well-formed value
  maps to the type's name (`eire` is `iedub`, `yes` is `true` for `kn`).
  `update-cldr-lists.py` writes them to
  `intl/core/data/extension-value-aliases.txt`. Only a whole value is
  replaced, and a `-u-` value is removed only when the whole value is `true`
  after that (`en-u-ca-true-foo` stays).
- **Size work comes later.** Smaller presets, if measurements call for them,
  would come from a checked-in script that runs `icu4x-datagen` once and
  commits its output, picked up through `ICU4X_DATA_DIR`. The library size per
  ABI is recorded as each constructor lands.

  | After | `libhermesintl.so`, Linux x86-64, Release, Clang | stripped |
  |---|---|---|
  | Phase 2 | 6,113,960 bytes | 2,043,120 bytes |
  | Phase 3 | 9,222,880 bytes | 5,054,136 bytes |
  | Phase 4 | 14,420,328 bytes | 10,170,232 bytes |
  | Phase 5 | 24,250,680 bytes | 19,939,248 bytes |

  Each build is of the phase's commit in a temporary worktree with its own
  build directory configured with `-DCMAKE_BUILD_TYPE=Release` and Clang
  18, building the `hermesintl` target; the stripped size is that of
  `llvm-strip -o` of the built library. The library is linked with
  `HERMES_EXTRA_LINKER_FLAGS`, as `hermesvm` is (`--gc-sections` on Linux,
  `-dead_strip` on Apple platforms, and `--icf=safe` where the linker has
  it), which `add_hermes_library` does not apply to a shared library. The
  linker then keeps only what the entry point reaches, and most of ICU4X's
  compiled data is never referenced: without the flags, the Phase 3
  library is about 22.6 MB, 17.4 MB stripped, with 12.8 MB of `.rodata`,
  and on macOS (ASan, Debug) `-dead_strip` takes it from 19.9 MB to
  5.8 MB. Phase 3 adds 3.0 MB, most of it read-only data (`.rodata`
  1.3 MB to 4.1 MB, `.text` 0.56 MB to 0.76 MB, `.data.rel.ro` 29 KB to
  50 KB). The data is ICU4X's number, currency and plural data that the
  plugin uses, and the 0.9 MB of generated CLDR files. Phase 4 adds
  5.2 MB (5.1 MB stripped), again mostly read-only data (`.rodata` 4.1 MB
  to 8.4 MB, `.text` 0.76 MB to 1.07 MB, `.data.rel.ro` 50 KB to 277 KB):
  ICU4X's calendar, date name and time zone name data, the 0.87 MB of
  `date-patterns.txt` and `day-periods.txt`, the tzdb's 207 KB, and 41 KB
  more bytecode. The Phase 4 row was measured the same way, together with
  a new build of the Phase 3 commit, which reproduced the Phase 3 row to
  the byte. Phase 5 adds 9.83 MB (9.77 MB stripped), again mostly read-only
  data (`.rodata` 8.4 MB to 17.0 MB, `.text` 1.07 MB to 1.20 MB,
  `.data.rel.ro` 277 KB to 839 KB): ICU4X's display name, relative time
  and list data, the segmenters with their 3.78 MB of dictionaries (see
  "Segmenter"), the 252 KB of `display-names.txt`, and 30 KB more
  bytecode. The Phase 5 row was measured the same way, together with a
  new build of the Phase 4 commit, which reproduced the Phase 4 row to
  the byte. Apart from the dictionaries, the share of each ICU4X data
  struct has not been measured yet.
- **Time zones.** ICU4X provides zone names and IANA identifiers but not the
  offset rules. The plugin bundles the tzdb in the Rust core, so offsets are
  the same on every platform and test262 results are reproducible: the
  `jiff-tzdb` crate, pinned as `=0.1.9`, which is IANA tzdb **2026e**
  compiled by `zic` in the "rearguard" format (so Europe/Dublin's daylight
  saving time is in summer, `"Irish Standard Time"` in July) and the "slim"
  layout (explicit transitions, then a POSIX TZ string for the rules after
  them): 207 KB of TZif data, under the MIT license or the Unlicense
  (`intl/THIRD_PARTY_NOTICES.md`). The plugin reads it with its own reader,
  `intl/core/src/tz.rs`, not with `jiff`, because `jiff`'s timestamps stop
  at years ±9999 and ECMAScript's run to ±275,760:
  - a zone is looked up by its primary identifier with `jiff_tzdb::get`, and
    its TZif data (version 2 or later: the 64-bit block and the footer) is
    parsed once per process, on first use;
  - an instant before the first transition has the zone's first local time
    type, which is local mean time with its seconds (RFC 8536;
    `America/New_York` in 1800 is −4:56:02); one after the last transition
    has the offset the footer's POSIX TZ string gives for that year,
    computed with exact integer arithmetic on days, which works for any
    year (`America/New_York` in July 275000 is −4:00);
  - [[InDST]] is the `isdst` flag of the local time type, or, after the
    transitions, whether the POSIX rule is in its daylight part;
  - the offset is in seconds; the draft's GetNamedTimeZoneOffsetNanoseconds
    is that × 10^9.

  ICU4X's own zone list (CLDR 48.2) decides which identifiers exist
  (`Intl.supportedValuesOf("timeZone")`), and a Rust test checks that each
  of its primary identifiers has tzdb data. jiff-tzdb's data is tzdb's
  main data without its `backzone` file, so before 1970 a zone that the
  main data makes a link (about 100 of them) has its target's offsets:
  `Europe/Oslo` in 1880 is +0:53:28, Berlin's local mean time, where a
  build with `backzone` has Oslo's own. Node 24 gives the same as the
  plugin. ECMA-402 recommends a build with `backzone` but does not require
  one.

## Testing

- **Acceptance:** test262 `intl402`, plus the `built-ins` tests of the
  overridden methods, with the `hermes` binary from the same build. The
  test262 pin is the commit CI already uses,
  `c9aa6126e8f6fe2c6f668c65bfba771ffc8a6096`.
- **Runner:** `utils/test_runner.py` has a plugin mode (`--intl-plugin`,
  `utils/testsuite/intl_plugin.py`). Its normal mode runs without the
  plugin and only reports passes; since Phase 7 it skips every test under
  `test262/test/intl402` as a fixed rule, even with `--test-skiplist`,
  because a runtime without the plugin has no `Intl`. (`--test-intl`,
  which ran them against the built-in implementation, is gone from
  `utils/test_runner.py` and `tools/test-runner`, and so are the
  `skiplist.json` entries under `intl402`.) Plugin mode judges every
  result against an expectations file instead. In plugin mode:
  - every run gets `-Xintl-plugin-path=<plugin>`,
    `-Xintl-default-locale=en-US` and `-Xintl-default-time-zone=UTC`, and
    runs with `TZ=UTC`, so that the engine's local time agrees with the
    plugin's zone (test262 builds dates from local fields, `new Date(2017,
    11, 12, h)`), and results do not depend on the host;
  - the platform skip list is ignored for every path, which gives the same
    coverage on every platform. Its macOS entries (the built-in Apple
    Collator) and Linux entries (`intl402` paths, and Hermes's own
    `toLocaleLowerCase` and `toLocaleUpperCase`, which pass in normal mode
    without them) were removed in Phase 7, so only the Windows entries,
    slow tests under ASan, remain.
    Inside `intl402` every skip list is ignored. Outside it the other lists,
    which record non-Intl Hermes bugs, still apply. Intl feature exclusions
    (for example `Intl.NumberFormat-v3`, `Intl.DurationFormat`) are ignored
    everywhere;
  - each strictness mode runs separately, compiled with exactly its own
    strictness, and is judged on its own;
  - `intl/test/test262-expectations.txt` lists each expected failure with a
    reason and the mode it fails in (`both`, `strict` or `non-strict`);
  - an expected failure must fail with a JS exception or assertion. A crash,
    timeout, sanitizer report, fatal error or loader error is always a
    failure (a hard failure);
  - so is any run whose output contains `Exception in HostFunction:`, the
    message of an `Error` made from a C++ exception thrown by a host
    function. test262 calls no host functions but the plugin's, so this is
    always a plugin internal error, never a missing feature, even when the
    test expected an exception. A known limitation: an internal error that
    the test catches itself, for example with `assert.throws`, prints
    nothing and stays invisible;
  - a failed result with no per-mode outcomes, one that failed before the
    per-mode runs, is a hard failure, so silence never reads as success;
  - an unexpected pass is a failure, so the list only shrinks;
  - an entry for a test inside the paths the run was given that got no
    result in that mode (it was skipped, or runs only in the other mode) is
    a failure too, so no entry stays unchecked;
  - results are judged as they arrive. Expected failures show as passing, so
    output appears only for real problems, and a green run reads green;
  - before running anything, a probe checks that the plugin is loaded;
  - the expectations file records the test262 commit it was made against, and
    the runner refuses a different commit unless told otherwise;
  - `--intl-update-expectations` rewrites the file from a run. It keeps
    existing reasons, gives a new failure the phase its path belongs to, and
    marks anything else `UNEXPLAINED`, which fails the run until it is
    explained by hand. Within the paths the run was given, it keeps only
    this run's failures; outside them, the old entries for tests that still
    exist. It writes the file's header comment afresh and keeps no other
    comment, so explanations go in the reasons. It refuses to update after
    any hard failure.

  Outside plugin mode (normal mode, for test262 and mjsunit) the runner
  passes `-Xintl-plugin=false` to every run. Normal mode measures the engine
  without the plugin, as before the plugin existed, so a plugin in
  `<build>/lib` does not change its results; otherwise tests outside
  `intl402` that use `Intl` when it exists would see the plugin's. test262
  runs against the plugin only in plugin mode. An external `b_hermes`
  (`--bytecode-compat-check`) gets the flag only if its `-help` lists it; one
  that predates the plugin cannot load it anyway.

  Outside plugin mode the runner also compiles both strictness variants of a
  test with `-strict`, so tests that may run in either mode never run sloppy
  (dz issue `01a16722-587d-7a30-ab48-eef8fee593d9`). Plugin mode does not
  have this bug.
- **Target:** `check-intl`, separate from `check-hermes`. It runs the Rust
  tests, the plugin's lit tests and gtests, the unit tests of the runner's
  plugin mode, and test262 `intl402` plus the overridden built-ins. It needs
  a test262 checkout, given with `HERMES_INTL_TEST262_DIR`, and fails
  without one.
- `check-hermes` runs the loader's tests, with fake plugin libraries, but
  not the real plugin's tests. Every `hermes` it starts does load the real
  plugin from `<build>/lib` by default resolution, though, so
  `check-hermes`, `check-shermes` and the Node-API suites build the plugin
  first. The `hermes` tool target itself does not depend on the plugin, so
  building only the CLI never runs cargo or the plugin's bytecode build. The
  cost: after a change in `intl/`, a `bin/hermes` run by hand loads a stale
  plugin until `all` (or `hermesintl`) is built. Tests of Hermes's own
  behavior that the plugin replaces run with `-Xintl-plugin=false` (see
  "Built-in overrides"): `test/hermes/number-functions.js`,
  `bigint-prototype-tolocalestring.js` and
  `regress-array-tolocalestring-tolength.js` since Phase 3, and
  `date-format-fixed.js` and `date-locale.js`, which test Hermes's own
  Date `toLocaleString`, `toLocaleDateString` and `toLocaleTimeString`,
  since Phase 4. Their `shermes -exec` runs pass the flag with
  `-Wx,-Xintl-plugin=false`.
- **Lit tests and gtests** for Hermes-specific behavior: each loader path and
  each row of the error table, `EnableIntlPlugin=false`, Workers inheriting
  the parent's settings, both runtime factories, the function-shape table,
  and the private-field attachment pattern.
- **Rust unit tests** for the code that fills ICU4X gaps. Each must be shown
  able to fail: break the code under test and name the test that catches it.
- **Sanitizers:** `check-intl` runs locally in the ASan build; CI runs it
  without sanitizers. In the ASan build the Rust static library is linked in
  uninstrumented; the JSI layer and the engine are instrumented.
- **Packaging** (Phase 6). Conformance work happens on Linux; these runs
  check that each packaging builds, loads and gives the same results. None
  of them runs in CI.
  - **Android**, on this repository's x86_64 emulator image (Android 36
    `google_apis`, whose ARM translation also runs arm64 executables):
    - `hermes-intl-android-smoke` (`intl/test/android/`) with the plugin
      next to `libhermesvm.so`, only on the library search path, and in an
      APK-style zip with every library mapped from it, plus no plugin and a
      truncated plugin (fatal, naming the library), and the defaults from
      `TZ` and the system properties. The search-path and APK layouts ran
      on x86_64 only: the ARM translation ignores `LD_LIBRARY_PATH`.
    - The app in `intl/test/android/app/`, which loads Hermes from the AAR
      through SoLoader, as React Native does, with the libraries left in
      the APK; it reports `Intl.Segmenter` from the plugin. Once the
      plugin is removed from the APK it reported `undefined` for
      `Intl.Segmenter` in Phase 6, and since Phase 7 it reports
      `typeof Intl: undefined` and the results of Hermes's own fallbacks.
    - Plugin-mode test262 on the device through `utils/testsuite/adb-hermes`
      (`intl/README.md`), x86_64 natively and arm64 under translation:
      both give the host's totals (2696 pass, 18 js-fail, 0 problems).
    - armeabi-v7a and x86 are built and checked (exports, `NEEDED`,
      alignment) but not run: the image has no 32-bit ABIs. No physical
      device was used.
  - **Apple platforms**, by hand on a Mac (`intl/README.md` has the
    commands):
    - per-slice link checks of both frameworks (`lipo`, `vtool`, `otool`,
      `nm -m`);
    - the smoke host `hermes-intl-apple-smoke` (`intl/test/apple/`) in the
      iOS, tvOS and visionOS simulators (arm64), and in Mac Catalyst and
      macOS natively and under Rosetta, in the build tree and in an app
      layout, with the plugin present, absent and broken, and default
      detection in the iOS simulator (`AppleLanguages`, `TZ`);
    - a universal macOS host build (`CMAKE_OSX_ARCHITECTURES="arm64;x86_64"`)
      with `check-intl`, and plugin-mode test262 natively and under
      Rosetta, both with the host's totals;
    - the CocoaPods consumer app `intl/test/apple/podapp/`
      (`run-podapp.sh`) and `pod lib lint` against a local tarball.

    Two things are not run. test262 in the iOS simulator: the CLI does not
    build for the simulator SDK, which lacks the readline headers
    `tools/hermes/repl.cpp` uses. And x86_64 simulator processes: the
    simulator runtimes used for testing are arm64 only, so the x86_64
    simulator builds are to be the evidence (see Phasing), and the
    incompatible-architecture rule is shown on macOS.
  - **Windows**, on Linux: the clang-cl cross builds (x64 and arm64,
    Release and Debug), and the x64 Release build under Wine 9: default
    resolution for `hermes.exe` and `hermes-jsi-shared.exe` (including
    `bin\..\lib` and a non-ASCII directory), `IntlPluginPath` (absolute,
    relative, bare names), a missing dependency (error 126), a
    wrong-architecture DLL (error 193), a Release-built fixture with the
    debug ABI bits rejected before any call into it, the JSI contract
    fixture, the defaults Wine reports, `IntlPluginUnitTests.exe`'s
    `DefaultsTest.*`, and plugin-mode test262 with the host's totals
    (through `utils/testsuite/wine-hermes`, `intl/README.md`).
    Wine 9's own `msvcp140.dll` makes Hermes executables hang
    intermittently, with or without the plugin; with Microsoft's
    `msvcp140.dll` (`WINEDLLOVERRIDES=msvcp140=n`) there is no hang, so
    the Wine runs use it. Debug builds only link (Wine has no debug C
    runtime DLLs), and the arm64 build does not run. The native checks
    are under Phasing.

## Phasing

Each phase gets its own implementation plan. Each phase delivers its
components completely: the whole public surface, including `formatToParts`,
`formatRange` and `resolvedOptions`.

0. **Infrastructure.** CMake / cargo integration for the host and
   `HERMES_BUILD_INTL`; the binary contract checked on Linux, macOS and the
   Apple framework layout; the
   Rust core skeleton and its generated header; the JSI layer and its X-macro
   table; `IHermesBuiltinBytecode`; the loader, its error table, Worker
   inheritance and the `RuntimeConfig` fields; the default locale and time
   zone detection; the test runner's plugin mode and `check-intl`; the
   function-shape rules and the private-field pattern, with their lit tests;
   the shared abstract operations; the `Intl` object itself
   (`getCanonicalLocales`, `supportedValuesOf`).
1. **`Intl.Locale`.** Every other constructor resolves locales through it.
   The whole surface: the constructor, the getters, `toString`, `maximize`
   and `minimize`, and the information methods `getCalendars`,
   `getCollations`, `getHourCycles`, `getNumberingSystems`,
   `getTimeZones`, `getTextInfo` and `getWeekInfo`, with `firstDayOfWeek`.
2. **Collator and case mapping.** `Intl.Collator` with its whole surface
   (the constructor and its options, `supportedLocalesOf`, `resolvedOptions`
   and the `compare` getter with its bound function), and the
   `localeCompare`, `toLocaleUpperCase` and `toLocaleLowerCase` overrides.
   `ResolveOptions` is shared with the later phases. Phase 2 built all of
   it; `usage: "search"` uses the sort tailoring (see Data).
3. **NumberFormat and PluralRules.** All notations, styles and ranges, and
   parts; legacy constructor behavior; PluralRules shares the digit options.
   Digits for the numbering systems ICU4X has no data for (see Data). Also
   the `toLocaleString` overrides on Number, BigInt, Array and TypedArray.
   Phase 3 built all of it. Apart from the tests whose harness needs
   Intl.DateTimeFormat (`NumberFormat/this-value-ignored.js`,
   `PluralRules/undefined-newtarget-throws.js`), three test262 tests in its
   scope stay expected failures, each for a reason given under "Plural
   rules": `gv`, which ICU4X has no data for, the BigInt argument of
   `selectRange`, and fr's plural category for a negative exponent.
4. **DateTimeFormat.** Option-bag matching, the bundled TZDB, `h24`, ranges
   and parts, legacy constructor behavior, and the Date `toLocale*`
   overrides. Phase 4 built all of it: every calendar of
   `Intl.supportedValuesOf("calendar")`, every numbering system
   NumberFormat supports, every time zone and offset time zone, over the
   whole time value range. Five test262 tests in its scope stay expected
   failures. Four are `canonical-tz` tests, which expect the time zone
   identifier kept as given, where the draft of October 24, 2026 stores
   the primary identifier (see "Data", "DateTimeFormat"):
   `DateTimeFormat/timezone-not-canonicalized.js`,
   `canonicalize-utc-timezone.js`, `timezone-case-insensitive.js` and
   `canonicalize-timezone.js`. The fifth,
   `DateTimeFormat/prototype/format/numbering-system.js`, expects V8's
   replacement of U+202F by a space in `format()`, which the draft and
   CLDR do not have. The tests whose harness needed Intl.DateTimeFormat
   (Phase 3's two among them) pass.
5. **Remaining constructors.** ListFormat, RelativeTimeFormat, DisplayNames,
   DurationFormat (built on NumberFormat and ListFormat), Segmenter.
   Phase 5 built all of it: `Intl.ListFormat` over any iterable of
   strings, `Intl.RelativeTimeFormat` in every numbering system
   NumberFormat supports, `Intl.DisplayNames` for its six types,
   `Intl.DurationFormat` on the plugin's internal NumberFormat and
   ListFormat, and `Intl.Segmenter` with its Segments objects and Segment
   Iterators, each with its whole surface (see "Data"). Every test262
   test in its scope passes; none stays an expected failure.
6. **Packaging.** Cross builds through cargo for every target (the four
   Android ABIs, the eight Apple slices, universal macOS, Windows x64 and
   arm64) and multi-configuration generators; library loading on Windows;
   the plugin linked against the JSI provider of each packaging
   (`libjsi.so` on Android, `hermesvm.framework` on Apple platforms) or,
   on Windows, carrying its own JSI; the Android `.so` in the Hermes Android
   build and AAR; `hermesintl.xcframework` in the `hermes-ios` tarball and
   the podspec; and the packaging tests under Testing. Fixes to Hermes
   outside the plugin that the packaging needed:
   - the command-line tools build in mobile configurations again
     (`cli::withIntlPluginFlags`);
   - on Android the command-line tools no longer abort at exit without a
     JVM (`cli::toolFinalizerThreadRunner`, inherited by Workers);
   - Hermes builds with clang-cl (`Hermes.cmake` tests `MSVC` before
     `GCC_COMPATIBLE`, JSI gets `/EHsc`, `/WHOLEARCHIVE` names the archive
     by path);
   - `utils/build-apple-framework.sh` builds its host `hermesc` for macOS
     even when another platform's deployment target, such as
     `XROS_DEPLOYMENT_TARGET`, is in the environment;
   - `shermes -static-link` on Apple platforms links Boost.Context, which
     every non-ASan build uses, so the link no longer fails with undefined
     `hoost::context` symbols (`intl/test/lit/shermes-static-link.js`).

   Phase 6 built all of it. Three verifications are not done:
   - **Native Windows.** The Windows build was cross-compiled on Linux and
     run under Wine only. `HERMES_BUILD_INTL` stays `OFF` by default on
     Windows, and the Windows CI workflow is unchanged, until this
     checklist has been run on a Windows machine:
     1. **A host `hermesc` for CI.** The Windows CI job sets
        `CMAKE_SYSTEM_NAME=Windows`, which CMake treats as a cross build,
        and a cross build with the plugin needs `IMPORT_HOST_COMPILERS`.
        Choose one: (a) drop `CMAKE_SYSTEM_NAME` from the job, so the
        build compiles its own `hermesc`; (b) add a first stage that builds
        `hermesc` natively and passes its `ImportHostCompilers.cmake`; or
        (c) exempt from the check a Windows host building for its own
        architecture (`CMAKE_HOST_WIN32` and the same processor).
     2. **Rust in the workflow:** rustup with `x86_64-pc-windows-msvc` and
        `aarch64-pc-windows-msvc`, Rust 1.95 or later, and cargo on `PATH`
        for the Visual Studio generator's custom commands.
     3. **Flip the default** only after items 4 to 9 pass, together with
        the CI changes of items 1 and 2.
     4. **Native MSVC build** with the Visual Studio generator, x64 and
        arm64 (arm64 cross-built from an x64 host is fine), Debug and
        Release, with item 1's choice for `hermesc`:

        ```
        cmake -S . -B build-x64 -G "Visual Studio 17 2022" -A x64 ^
          -DHERMES_BUILD_INTL=ON -DJSI_UNSTABLE=ON
        cmake --build build-x64 --config Release ^
          --target hermes hermes-jsi-shared hermesvm hermesintl
        cmake --build build-x64 --config Debug ^
          --target hermes hermes-jsi-shared hermesvm hermesintl
        ```

        and the same with `-A ARM64` in `build-arm64`. Check:
        `bin\<Config>\hermesintl.dll` exports only `hermes_intl_get_entry`
        (`dumpbin /exports`, or `python
        intl\test\lit\Inputs\coff-exports.py <dll>`); the copy next to
        `hermesvm.dll` is identical; Debug and Release use cargo's dev and
        release profiles; the imports (`dumpbin /dependents`) are the C
        runtime, `kernel32`, `ntdll`, `bcryptprimitives` and
        `api-ms-win-core-synch` only; `hermes.exe` runs scripts with
        Boost.Context fibers on.
     5. **No hangs:** a Release `hermes.exe` never hangs over many runs
        (200 runs of a one-line script with `-Xintl-plugin=false`, and
        `intl402/NumberFormat/prototype/format/no-instanceof.js` in plugin
        mode).
     6. **The JSI contract** with each module's own JSI. Build the
        fixture pair by name (it is not in the default build):
        `cmake --build build-x64 --config Release --target
        fake_intl_needs_dep fake_intl_dep` (the DLLs land under
        `unittests\IntlLoader`). Copy both next to a Release `hermes.exe`,
        `fake_intl_needs_dep.dll` renamed to `hermesintl.dll` (or pass it
        with `-Xintl-plugin-path=<path>`), and run a script holding

        ```
        try { __fakeIntlThrow(); } catch (e) { print(e.message); }
        var o = {}; print(__fakeIntlRead(o));
        __fakeIntlAttach(o); print(__fakeIntlRead(o));
        ```

        with `hermes.exe`. It must print `fake JSError` (a `jsi::JSError`
        thrown in the DLL caught by the host as one, not "Exception in
        HostFunction"), then `-1` and `42` (`hasNativeState` and
        `getNativeState` work for state the DLL attached). If either
        fails, the plugin cannot keep its own JSI.
     7. **ABI bits with real builds:** a Debug `hermesintl.dll` loaded by a
        Release `hermes.exe` is fatal with `built with
        _ITERATOR_DEBUG_LEVEL 2, Hermes with 0; built with the debug C
        runtime (/MDd), Hermes with the release C runtime (/MD) (flags 0xd,
        Hermes expects 0x1)`, and the reverse with `built with
        _ITERATOR_DEBUG_LEVEL 0, Hermes with 2; ... (flags 0x1, Hermes
        expects 0xd)`; each aborts before the plugin's `resolve_defaults`
        or `install` runs.
     8. **The loader natively** (Release x64, and arm64 on an arm64 machine
        if there is one): default resolution next to `hermes.exe`, through
        `bin\..\lib` and `Bin\..\lib`, for `hermes-jsi-shared.exe` from
        `hermesvm.dll`'s directory, and in a non-ASCII directory, including
        characters outside the ANSI code page; `IntlPluginPath` absolute,
        relative (`\`, `/`, `..`), a bare name in the application
        directory, and a bare name present only in the current directory
        (fatal, error 126); a missing dependency (fatal, 126); the arm64
        plugin under an x64 `hermes.exe` (fatal, 193); the debug-bits
        fixture `fake_intl_debug_abi.dll` (fatal, with no "was called"
        line); no error dialog for any of them; and a Hermes directory long
        enough that the candidate has 260 characters or more, in a process
        without long-path awareness: the plugin loads, and with it absent
        there is no plugin and no fatal error.
     9. **Defaults:** user locale German (Germany) and zone `W. Europe
        Standard Time` give `de-DE` and `Europe/Berlin`; a sort-order
        locale such as `de-DE_phoneb` gives `de-DE`; region Canada with
        Pacific Time gives `America/Vancouver`, region United States
        `America/Los_Angeles`; the `qps-ploc` pseudo-locale gives `en-US`
        (`IntlPluginUnitTests.exe --gtest_filter=DefaultsTest.*` prints
        what it detects).
     10. **`check-intl`** natively (cargo tests, lit, whose `exports.js`
         reads the DLL with `coff-exports.py`, the gtests, and plugin-mode
         test262; `IntlRustCoreAllocTests` is not built on Windows, since
         the C library has no `aligned_alloc`), and `IntlLoaderTests`,
         which should build with MSVC (never tried; it does not build with
         clang-cl, where `-Werror=undef` trips on the vendored gtest).
         The loader lit tests in `test/hermes/intl-plugin` are skipped on
         win32; item 8 stands in for them.
     11. **Plugin-mode test262**: `python utils\test_runner.py
         --intl-plugin build-x64\bin\Release\hermesintl.dll
         --intl-expectations intl\test\test262-expectations.txt -b
         build-x64\bin\Release` followed by the directories
         `check-intl-test262` runs (`intl/CMakeLists.txt`) gives the
         host's totals: 2696 pass, 18 js-fail, 0 problems.
   - **Apple, a second run on a Mac with a newer Xcode (27.1): passed,
     with two items open.** The first run (Xcode 26.4, at the first
     Apple packaging commit) passed every Apple run under Testing except
     three, each fixed since: CocoaPods builds that include x86_64 for
     the tvOS or visionOS simulator failed in CocoaPods' copy phase (the
     slice renaming under Packaging addresses it); the host `hermesc` of
     `utils/build-apple-framework.sh` failed to configure with
     `XROS_DEPLOYMENT_TARGET` in the environment; and in the universal
     macOS `check-intl`, `shermes-static-link.js` failed because `shermes
     -static-link` did not link Boost.Context. The second run, on Xcode
     27.1 at the Phase 6 tip, passed everything in the Apple testing
     list: the host `hermesc` under a foreign deployment target,
     `shermes -static-link` with Boost.Context, the loader's dyld-message
     rule with dyld's verbatim messages (one line, `dlopen(<path>,
     0x0006): tried: ` and then entries only, nothing after the last), all
     eight slices and both xcframeworks, the link checks, the smoke tests
     on the iOS 27.1, tvOS 27.0 and visionOS 27.0 simulators and on macOS
     and Mac Catalyst, and the tarball. The CocoaPods consumer app passed
     with the committed podspec (see Packaging): the tvOS x86_64 build embeds
     the arm64-only plugin (the linker ignores it for x86_64, so the x86_64
     code has no load command for it), the tvOS "arm64 x86_64" build runs
     with the plugin, the app with the plugin removed runs (then with the
     built-in implementation, which Phase 7 deleted, so without
     `Intl.Segmenter`), and a link without the weak flag fails at launch.
     `pod lib lint` passed for iOS and tvOS together and for visionOS. Still
     unverified: an x86_64 simulator process at run time (only arm64
     simulator runtimes were available). `pod lib lint` for macOS fails on
     Xcode 27.1 only because the pod target's 10.15 minimum is below Xcode
     27.1's 12.0; raising it is upstream's choice to make. Hermes cannot be
     generated with CMake's Xcode generator (upstream: the
     `InternalBytecode.js` custom command in `lib/InternalJavaScript` is
     attached to two targets, which the Xcode new build system rejects), so
     the multi-configuration Apple framework layout is verified with Ninja
     Multi-Config.
   - **Signing with an app's Team ID** (see Risks).
7. **Delete the built-in implementation.** Phase 7 did it, so `Intl`
   comes only from the plugin:
   - Deleted: `lib/VM/JSLib/Intl.cpp`; `lib/Platform/Intl/` (the Apple
     and Android backends, the unused `PlatformIntlShared.cpp`, the BCP 47
     parser and the Java classes); `include/hermes/Platform/Intl/`;
     `android/intltest`; `unittests/PlatformIntl/`; `utils/IntlGen/`;
     `doc/IntlAPIs.md`; the VM's `Intl` global, `Runtime::hasIntl()`,
     and the conditional entries of `NativeFunctions.def`,
     `PredefinedStrings.def` and `RuntimeHermesValueFields.def`, and
     `IntlNativeType` in `InternalProperties.def` (runtime-only IDs, so no
     bytecode change); `${FOUNDATION}`, which only the Apple backend
     linked (`CORE_FOUNDATION` stays). fbjni stays, for the Unicode code
     and the finalizer thread runner.
   - Kept: each locale-sensitive built-in's fallback, now its only code
     (see "Without the plugin"). The `HERMES_ENABLE_INTL` option is
     deprecated and ignored (see Build). The `Intl` field of
     `RuntimeConfig` and `-Xintl` are deprecated and ignored (see
     `RuntimeConfig`), and the loader no longer copies the field to
     Workers.
   - Tests. The 23 lit tests of the built-in implementation
     (`test/hermes/intl/`, `test/shermes/intl/`,
     `test/hermes/regress-intl-number-format-bigint-input.js`), which
     never ran on a Linux host, and the 4 BCP 47 parser gtests are gone.
     What they covered and the plugin's tests lacked became plugin lit
     tests: `NumberFormat` `format(0n)` and `formatToParts(0n)`
     (`number-format-bigint-zero.js`), and a default locale with an
     unknown POSIX modifier (`LANG=en_GB@foo=bar`) and the default time
     zone from `TZ=US/Pacific` (`default-locale-env.js`). The test262
     `toLocaleLowerCase` cases were already covered by plugin-mode test262.
     New: `test/hermes/no-intl-fallbacks.js` and
     `no-intl-fallbacks-tr.js`, `intl/test/lit/xintl-ignored.js` and
     `IntlLoaderTest.IntlFieldIsIgnored`. `shermes-flags.js` and
     `tool-flags.js` expect `undefined`, not an object, with the plugin
     off.
   - Test runners: see "Runner" under Testing.
   - CI. The `test-macos-test262` job of `build.yml` builds once, with
     the plugin, and runs `check-hermes`, `check-intl` and a normal-mode
     test262 run of the whole `test262/test` tree, which keeps the
     `-Xjit=force` matrix entry, in place of its run of the built-in
     implementation. The `test-macos` job of `.circleci/config.yml`
     builds with `-DHERMES_BUILD_INTL=OFF`, since it has no Rust, so it
     tests Hermes without `Intl`. Neither has run yet.
   - Android and Apple: see Packaging. React Native: the release gate
     under Packaging.

   Verified on the Linux host: `check-hermes` gives 4375 passes, 6
   expected failures, 156 unsupported and 0 unexpected results (before:
   4376 passes and 179 unsupported; the 4 parser gtests and the 23 lit
   tests went, 3 tests were added); `check-shermes` 69 passes, 1 expected
   failure, 5 unsupported (before: 11 unsupported); `check-intl` 161 lit
   passes and 2 unsupported (3 more passes), 198 Rust tests, 23 runner
   tests (2 more) and 57 data tests; plugin-mode test262 2696 pass, 18
   js-fail, 0 problems, unchanged. A configure with
   `-DHERMES_ENABLE_INTL=ON` or `=True` prints the warning and succeeds.
   Verified on Android (the x86_64 emulator image under Testing): the AAR
   has no `com/facebook/hermes/intl` classes; the smoke test and the test
   app run with the plugin as before, and without it report `typeof
   Intl: undefined` and the fallback results, the app under
   `-Xcheck:jni` with no JNI error; plugin-mode test262 on the device
   gives the host's totals. As a negative case, the same app built
   against the AAR from before Phase 7, with the plugin removed, reports
   `typeof Intl: object`.

   **Apple: verified on a Mac** (Xcode 27.1; iOS 27.1, tvOS 27.0 and
   visionOS 27.0 simulators; the eight slices and both xcframeworks built
   without the deprecated option, with no deprecation warning). Foundation
   and libobjc are gone from every `hermesvm` slice: what remains is the
   `hermesvm` id, CoreFoundation, libc++ and libSystem (21 CoreFoundation
   imports in each slice). `hermesintl`'s link set is unchanged.
   `hermesvm.xcframework` went from 61M to 58M, and the `hermes-ios`
   tarball from 104,590,473 to 103,718,999 bytes. The smoke tests (iOS,
   tvOS and visionOS simulators; Mac Catalyst and macOS, each native and
   under `arch -x86_64`) print `typeof Intl: object` and
   `Intl.Segmenter: function` with the plugin; without it they print
   `typeof Intl: undefined` and the seven fallback results, following the
   device locale and time zone, with no crash. The CocoaPods app (iOS and
   tvOS) does the same with and without the plugin, and `pod lib lint`
   passes for ios,tvos and for visionos. `hermesvm` sizes, in bytes:

   | Slice | Before Phase 7 | After Phase 7 | Change |
   |---|---:|---:|---:|
   | macOS | 10,563,576 | 10,075,544 | -488,032 (-4.6%) |
   | iOS | 4,464,968 | 4,224,168 | -240,800 (-5.4%) |
   | iOS simulator | 8,957,576 | 8,485,544 | -472,032 (-5.3%) |
   | Mac Catalyst | 10,561,464 | 10,073,064 | -488,400 (-4.6%) |
   | tvOS | 4,464,712 | 4,224,472 | -240,240 (-5.4%) |
   | tvOS simulator | 8,957,304 | 8,485,848 | -471,456 (-5.3%) |
   | visionOS | 5,273,504 | 5,015,280 | -258,224 (-4.9%) |
   | visionOS simulator | 10,641,816 | 10,135,784 | -506,032 (-4.8%) |

   The macOS host paths that both macOS CI jobs now run also passed
   (arm64, Xcode 27.1): `check-hermes` with the plugin (4188 pass,
   0 failures; `no-intl-fallbacks.js`, `no-intl-fallbacks-tr.js` and
   `date-locale.js` pass with CoreFoundation, `date-format-fixed.js`
   is unsupported there as before) and in the CircleCI configuration
   with `-DHERMES_BUILD_INTL=OFF` (4220 pass, 0 failures), and a
   normal-mode test262 run of the locale built-ins (String
   `toLocale{Lower,Upper}Case` and `localeCompare`, Date, Number, Array
   and `%TypedArray%` `toLocaleString`, `staging/sm/String`) against
   the CoreFoundation fallbacks in both builds: 181 tests, 160 pass,
   0 failures, 21 skipped by the existing lists.

## Risks

- **ICU4X gaps may be larger than found.** The research (2026-10) covered
  ICU4X 2.3 and main. DateTimeFormat option-bag matching in particular could
  be substantial if ICU4X's field-bag support does not stabilize. Phase 4
  did it in JS from CLDR's patterns (see "Date and time formatting"), so
  it no longer waits on ICU4X.
- **Word segmentation of long unbroken runs is quadratic** in ICU4X 2.3:
  a crafted 200,000-code-unit run of Chinese or Thai text takes 22 to 60 s
  in the Debug ASan build. It is fixed by the move to ICU4X 2.4; see
  "Data" → Segmenter.
- **Unstable and experimental ICU4X APIs** (`icu_experimental`, unstable
  constructors) can change between releases. `Cargo.lock` pins them; upgrades
  are deliberate.
- **The binary contract on Apple.** Checked on macOS (arm64) in Phase 0,
  Task 13:
  - `hermesvm.framework` (Release, macosx, arm64, `HERMES_BUILD_INTL=OFF`)
    exports JSI: 150 `facebook::` symbols, including the typeinfo of
    `jsi::NativeState`, `HostObject`, `JSIException`, `JSINativeException`
    and `JSError`, and `makeHermesRuntime`. Its install name is
    `@rpath/hermesvm.framework/Versions/1/hermesvm`.
  - A probe library built with `-undefined dynamic_lookup`, `dlopen`ed
    `RTLD_LOCAL` by a host linked against the framework, works: a
    `jsi::JSError` it throws keeps its message and the host catches it as
    `jsi::JSError`, so type information is shared across the boundary. A
    `std::logic_error` becomes a JS `Error` with the message
    `Exception in HostFunction: ...`, and uncaught it reaches the host as
    `jsi::JSError`.
  - The same library linked directly with `-framework hermesvm` behaves
    identically, its JSI references bound to `hermesvm`.

  So both approaches work on macOS. Linking explicitly against the framework
  is viable, and is the route Phase 6 took for every Apple framework
  build, since Apple's linker discourages `-undefined dynamic_lookup`.
  Phase 6 confirmed it on all eight slices: `hermesintl.framework` links
  `hermesvm.framework`, `nm -m` shows no dynamically looked-up symbol and
  10 symbols imported from hermesvm, and the plugin runs in the iOS, tvOS
  and visionOS simulators (arm64) and in Mac Catalyst and macOS. No
  physical device was run.

  Code signing: a host signed with the hardened runtime (`codesign -o
  runtime`) has library validation on, and `dlopen` rejects a plugin that
  is not signed with the host's Team ID ("not valid for use in process").
  The loader treats that as a broken plugin, so it is fatal. Such a host
  must sign the plugin with its own Team ID, or carry the
  `com.apple.security.cs.disable-library-validation` entitlement. Checked
  on macOS with ad-hoc signatures: the rejection, and loading with the
  entitlement. Signing both with one Team ID was not checked, in Phase 0
  or Phase 6: no signing identity was available. The build leaves the
  device slices unsigned and the simulator and macOS slices ad-hoc signed,
  and an app is expected to sign the embedded `hermesintl.framework` with
  its own identity, as it does `hermesvm.framework` (CocoaPods' generated
  embed script code-signs each framework when signing is enabled; not run
  here). What would verify it: the CocoaPods consumer app built with a
  real development identity, for a device and as a hardened-runtime macOS
  app, launching with the plugin loaded.

  Also verified on macOS: the build, `check-intl` (test262 plugin mode gives
  the same results as on Linux) and `check-hermes` pass; `shermes` with
  `libhermesvm` loaded `RTLD_LOCAL` works with the promotion described under
  "Binary contract with the host"; `-exported_symbol` leaves exactly one
  exported symbol; CoreFoundation detection reports a Mac's settings, and
  `TZ` overrides the time zone. That CoreFoundation honours a per-process
  `-AppleLanguages` override is checked by `intl/test/lit/apple-defaults.js`
  through `IntlDefaultsProbe`, a plain executable, because the gtest
  binary's argument parser rejects that argument.
- **Global symbols in `RTLD_LOCAL` hosts.** The loader's promotion makes
  `libhermesvm`'s exported symbols global in any process that loads it
  `RTLD_LOCAL` and then loads a plugin. A library loaded later can bind to
  them instead of its own copy.
- **The search path only where the messages are known.** Default
  resolution tries the library search path only with glibc, musl and
  bionic, whose `dlopen` messages tell an absent library from a broken one.
  With any other C library, as on Apple platforms and Windows, there is no
  search-path step, so an install there that relies on `LD_LIBRARY_PATH`
  or the like gets no plugin. A Linux C library other than glibc and
  bionic whose messages differ from musl's would make an absent plugin
  look broken, which is fatal; none is known to be used with Hermes.
- **Windows is verified only under Wine.** The JSI contract with a copy of
  JSI in each module, the Visual Studio build, Boost.Context fibers (off
  in the cross build), the real `GetUserDefaultGeoName` territory rows,
  long paths and the ABI-bit check with real Debug and Release builds
  are unverified on Windows itself (checklist under Phasing).
- **The tvOS and visionOS simulator xcframework slices list x86_64**,
  which their binaries lack (see Packaging). Xcode 27.1 and CocoaPods
  accepted this in the second Mac run (Phasing item 6); a later
  Xcode or CocoaPods that checks the binary against the metadata could
  reject the xcframework. An x86_64 simulator process loading the
  arm64-only plugin has not run (the simulator runtimes available were
  arm64 only); macOS shows the same dyld message and the loader's rule
  for it.
- **A fatal error can end with SIGSEGV.** In the iOS simulator and on
  Android, a broken plugin gives the loader's fatal message, naming the
  library, and then the process ends with SIGSEGV instead of an abort.
  This is a Hermes issue outside the plugin (`hermes_fatal` in embedders),
  dz issue `01a1807c-83b4-7f47-8b19-d4ef66140f7a` (filed on
  `static_h_bugs`); the loader's contract, a fatal error that names the
  plugin, holds.
