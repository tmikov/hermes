# Building the Intl plugin

This document covers what building the Hermes Intl plugin needs beyond a
normal Hermes build: Rust, cross builds, Android, the Apple frameworks, and
building and running the Windows plugin on Linux. The
[README](../README.md) is the introduction; the spec's
[Build](../../doc/superpowers/specs/2026-10-22-intl-plugin-design.md#build)
section has the design, including the mapping from builds to Rust targets.

## Rust

The CMake option `HERMES_BUILD_INTL` needs a Rust toolchain, 1.88 or newer
(the MSRV of `icu` 2.4), or 1.95 or newer for tvOS and visionOS. Without
one, configure fails and names the option; `-DHERMES_BUILD_INTL=OFF` gives
a Rust-free build. Configure uses the `cargo` on `PATH`, else the one in
`$CARGO_HOME/bin` or `~/.cargo/bin`, where rustup installs it (a Gradle
daemon or an Xcode build launched from the GUI does not get the shell's
`PATH`), and the `rustc` next to it.

The Rust standard library of each target triple the build uses must be
installed, and configure says which `rustup target add` is missing. A build
that has no Rust target mapping (another Android ABI, Apple platform or
architecture, Win32, MinGW, another cross build) defaults to `OFF`, and
`HERMES_INTL_RUST_TARGET` can name the triples.

The option of the former built-in `Intl`, `HERMES_ENABLE_INTL`, is
deprecated and ignored: a configure that sets it ON prints a warning and
goes on.

## Cross builds

A cross build (and any Apple build whose `hermesc` cannot run on the build
machine) compiles the plugin's JavaScript with the host's `hermesc`, so it
needs `-DIMPORT_HOST_COMPILERS=<host build>/ImportHostCompilers.cmake`.

## Outputs

The output is `<build>/lib/libhermesintl.so` on Linux and Android and
`<build>/lib/libhermesintl.dylib` on macOS, next to `libhermesvm`;
`hermesintl.framework` next to `hermesvm.framework` in an Apple framework
build (`<build>/lib`, or `<build>/lib/<Config>` under a multi-configuration
generator); and otherwise, on Windows and under multi-configuration
generators, the tools' directory (`bin` or `bin/<Config>`), with a copy
next to a shared `libhermesvm` or `hermesvm.dll`.

The plugin must be built with the same `JSI_UNSTABLE` setting as Hermes,
and it works only with the Hermes it was built with. The loader checks the
bytecode and interface versions and the `JSI_UNSTABLE` setting (and, on
Windows, the C runtime settings).

On macOS, a host signed with the hardened runtime rejects a plugin that is
not signed with the host's Team ID, and Hermes reports that as a fatal
error. Sign the plugin with the host's Team ID, or give the host the
`com.apple.security.cs.disable-library-validation` entitlement.

## Android

The Hermes Android build (`android/build.gradle.kts`) builds the plugin,
and the AAR carries `jni/<abi>/libhermesintl.so` for arm64-v8a,
armeabi-v7a, x86 and x86_64, next to `libhermesvm.so`. It needs rustup and
the four targets:

```
rustup target add aarch64-linux-android armv7-linux-androideabi \
  i686-linux-android x86_64-linux-android
```

A CMake build with the NDK toolchain works the same way: give it
`-DIMPORT_HOST_COMPILERS=<host build>/ImportHostCompilers.cmake` and
`-DHERMES_BUILD_SHARED_JSI=ON` (the plugin links `libjsi.so`), and build
`hermesintl`.

## Apple platforms

`utils/build-apple-framework.sh` builds `hermesvm.framework` and
`hermesintl.framework` for each slice and makes
`destroot/Library/Frameworks/universal/hermesvm.xcframework` and
`hermesintl.xcframework`. Before building a slice it checks for cargo and
the slice's Rust targets; `utils/build-apple-framework.sh rust_targets
<platform>` prints them, for example `rustup target add
$(utils/build-apple-framework.sh rust_targets iphonesimulator)`.

The tvOS and visionOS simulator slices of the plugin are arm64 only, since
Rust has no supported x86_64 target there;
`utils/intl-xcframework-sim-slices.py` explains how their xcframework
slices are named so that x86_64 app builds still work.

Hermes cannot be generated with CMake's Xcode generator (upstream's
`InternalBytecode.js` custom command is attached to two targets, which the
Xcode new build system rejects). Use Ninja Multi-Config for a
multi-configuration Apple framework build.

`hermes-engine.podspec` vendors both xcframeworks and links the plugin
weakly (`-weak_framework hermesintl`), so an app still launches with the
plugin deleted from it. React Native's own podspec, in the `react-native`
repository, needs the same two changes before React Native apps get the
plugin, and React Native's builds of Hermes from source need Rust or
`-DHERMES_BUILD_INTL=OFF` (the spec's
[Packaging](../../doc/superpowers/specs/2026-10-22-intl-plugin-design.md#packaging)
has both).

## Cross-building for Windows on Linux

`HERMES_BUILD_INTL` is off by default on Windows: the build with
Microsoft's compiler and linker, and the shared-type matching with
Microsoft's runtime, have been checked under Wine only, and no build has
run on Windows itself. The spec's
[Phasing](../../doc/superpowers/specs/2026-10-22-intl-plugin-design.md#phasing)
lists what a Windows machine has to check before it defaults to `ON` there.

Two CMake toolchain files build Hermes and the plugin for Windows (x64 or
arm64, MSVC ABI) on Linux. Each file's header comment says what it needs,
how to install it, and its settings:

- `utils/windows-cross/clang-cl-xwin.cmake`: clang-cl and lld-link, with
  the MSVC C runtime, C++ library and Windows SDK that
  [xwin](https://github.com/Jake-Shadle/xwin) downloads. It turns
  Boost.Context fibers off.
- `utils/windows-cross/msvc-wine.cmake`: Microsoft's own tools, which
  [msvc-wine](https://github.com/mstorsjo/msvc-wine) runs under Wine. It
  needs `-DCMAKE_POLICY_DEFAULT_CMP0141=NEW`.

Both also need the Rust target (`rustup target add x86_64-pc-windows-msvc`,
or `aarch64-pc-windows-msvc`) and a host build of Hermes, whose `hermesc`
compiles the bytecode:

```bash
cmake -G Ninja -B build-win -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_TOOLCHAIN_FILE=utils/windows-cross/clang-cl-xwin.cmake \
  -DHERMES_LLVM_BIN_DIR=$HOME/opt/LLVM-23.1.4-Linux-X64/bin \
  -DIMPORT_HOST_COMPILERS=<host build>/ImportHostCompilers.cmake \
  -DHERMES_BUILD_INTL=ON -DJSI_UNSTABLE=ON -DHERMES_ENABLE_NAPI=OFF
cmake --build build-win --target hermes hermes-jsi-shared hermesvm hermesintl
```

`HERMES_ENABLE_NAPI=OFF` leaves out the Node-API test addons, whose import
libraries share names, which Ninja rejects on Windows. For arm64, add
`-DHERMES_WINDOWS_ARCH=arm64` (and, with xwin,
`-DHERMES_XWIN_DIR=$HOME/.xwin-aarch64`).

On Windows the plugin is `hermesintl.dll`. It needs the C runtime DLL
(`/MD` or `/MDd`): configure fails with a static runtime. The loader
rejects a plugin whose `_ITERATOR_DEBUG_LEVEL` or debug/release C runtime
differs from Hermes's.

## Running the Windows build under Wine

Builds of either toolchain run under Wine 9 (`apt install wine wine64`).
Three things are needed:

- **Microsoft's C and C++ runtime.** With Wine's own `msvcp140.dll`,
  Hermes executables now and then hang at exit after printing their
  output, with or without the plugin, and always after heavy allocation.
  Copy Microsoft's DLLs of the build's toolset next to `hermes.exe` (and
  next to `hermesvm.dll`): `vcruntime140.dll`, `vcruntime140_1.dll` and
  `msvcp140.dll`, which msvc-wine installs in
  `VC/Redist/MSVC/14.44.35112/x64/Microsoft.VC143.CRT/`, and
  `ucrtbase.dll`, in `Windows Kits/10/Redist/10.0.26100.0/ucrt/DLLs/x64/`.
  A copy alone is not enough: Wine prefers its builtins for these names,
  so run with
  `WINEDLLOVERRIDES=vcruntime140,vcruntime140_1,msvcp140,ucrtbase=n`
  (`WINEDEBUG=+loaddll` shows each DLL loaded as `native` or `builtin`).
  The `api-ms-win-crt-*` names need nothing: Wine maps them to
  `ucrtbase.dll` itself. Overriding only `msvcp140` also stops the hang.
- **The prefix started without the override.** Wine's own programs, such
  as `wineboot`, which Wine runs when a prefix is created or updated, need
  Wine's `ucrtbase.dll` and fail to start under `ucrtbase=n`. Run `wine
  wineboot` once without the override (and keep a `wineserver -p`
  running) before the runs.
- **stdin from `/dev/null`.** `hermes-jsi-shared.exe` waits on a terminal
  stdin under Wine.

```bash
R=build-msvc  # or build-win
C=~/opt/msvc/VC/Redist/MSVC/14.44.35112/x64/Microsoft.VC143.CRT
U="$HOME/opt/msvc/Windows Kits/10/Redist/10.0.26100.0/ucrt/DLLs/x64"
for d in $R/bin $R/lib; do
  cp $C/vcruntime140.dll $C/vcruntime140_1.dll $C/msvcp140.dll \
    "$U/ucrtbase.dll" $d/
done
wineserver -p; wine wineboot
export WINEDLLOVERRIDES=vcruntime140,vcruntime140_1,msvcp140,ucrtbase=n
wine $R/bin/hermes.exe test.js </dev/null
WINEPATH="Z:$(realpath $R/lib | tr / '\\')" \
  wine $R/bin/hermes-jsi-shared.exe test.js </dev/null
```

`hermes.exe` finds `bin/hermesintl.dll` next to it; `hermes-jsi-shared.exe`
finds the copy next to `lib/hermesvm.dll`, which `WINEPATH` makes
loadable. A fatal error exits with status 3 with Microsoft's
`ucrtbase.dll` (as on Windows) and 29 with Wine's. The defaults come
from the locale and zone Wine derives from the Unix `LC_ALL` (or `LANG`)
and `TZ`, which may not be the zone you expect: Wine picks the first
Windows zone whose rules match, so `TZ=UTC` becomes `Greenwich Standard
Time` (`Atlantic/Reykjavik`).

Debug builds need Microsoft's debug runtime instead: `vcruntime140d.dll`,
`vcruntime140_1d.dll` and `msvcp140d.dll` from
`VC/Redist/MSVC/14.44.35112/debug_nonredist/x64/Microsoft.VC143.DebugCRT/`
and `ucrtbased.dll` from `Windows Kits/10/bin/10.0.26100.0/x64/ucrt/`.
Wine has no builtins for these, so they need no override, and a process
that has only the debug runtime must run without `ucrtbase=n`, which would
also apply to Wine's own DLLs. A Debug `hermes.exe` stops at its first
runtime under Wine with `Assertion failed: info.State == MEM_RESERVE &&
"Reserved memory not found"` (`lib/Support/OSCompatWindows.cpp`): Wine
commits a thread's whole stack, where Windows leaves its lowest part
reserved. To run Hermes's Debug code under Wine, build Debug with the
assertions off, which keeps the debug runtime and `_ITERATOR_DEBUG_LEVEL`
2: add `-DCMAKE_C_FLAGS_DEBUG="/Ob0 /Od /RTC1 /DNDEBUG"`,
`-DCMAKE_CXX_FLAGS_DEBUG="/Ob0 /Od /RTC1 /DNDEBUG"` and
`-DHERMES_SLOW_DEBUG=OFF`. A Debug plugin built normally loads into it.

### test262 under Wine

Plugin-mode test262 runs under Wine through `utils/testsuite/wine-hermes`;
its header comment says what it does. From the checkout's root:

```bash
mkdir -p /tmp/wine-bin
ln -sf $PWD/utils/testsuite/wine-hermes /tmp/wine-bin/hermes
T=<test262>/test
WINE_HERMES_DIR=$PWD/build-win/bin python3 utils/test_runner.py \
  --intl-plugin $PWD/build-win/bin/hermesintl.dll \
  --intl-expectations intl/test/test262-expectations.txt \
  -b /tmp/wine-bin \
  $T/intl402 \
  $T/built-ins/String/prototype/localeCompare \
  $T/built-ins/String/prototype/toLocaleLowerCase \
  $T/built-ins/String/prototype/toLocaleUpperCase \
  $T/built-ins/Number/prototype/toLocaleString \
  $T/built-ins/BigInt/prototype/toLocaleString \
  $T/built-ins/Array/prototype/toLocaleString \
  $T/built-ins/TypedArray/prototype/toLocaleString \
  $T/built-ins/Date/prototype/toLocaleString \
  $T/built-ins/Date/prototype/toLocaleDateString \
  $T/built-ins/Date/prototype/toLocaleTimeString
```

These are the directories `check-intl-test262` runs.
