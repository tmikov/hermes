---
id: building-and-running
title: Building and Running
---

This document describes how to build and run Hermes as a standalone compiler and VM. To use Hermes in the context of a React Native app, see the [React Native](https://reactnative.dev/docs/getting-started) documentation.

## Dependencies

Hermes is a C++17 project. clang, gcc, and Visual C++ are supported. Hermes also requires cmake, git, Python. It builds with [CMake](https://cmake.org) and [ninja](https://ninja-build.org).

The Hermes REPL will also use libreadline, if available.

Hermes does not use ICU. Unicode case conversion, normalization, collation
and date formatting are all self-contained.

`Intl` comes from the Intl plugin (`libhermesintl`), a separate library
written in Rust on [ICU4X](https://github.com/unicode-org/icu4x), which
Hermes loads when it is present. The CMake option `HERMES_BUILD_INTL`
builds it. It is on by default on Linux and macOS (and for Android and
the Apple platforms), so a default build there needs a Rust toolchain,
1.88 or newer; [rustup](https://rustup.rs) installs one. Without Rust,
configure fails and names the option. Configure with
`-DHERMES_BUILD_INTL=OFF` for a build without Rust; Hermes then has no
`Intl`, and `toLocaleString` and the other locale-sensitive built-ins give
Hermes's own results. On Windows the option is off by default. See
[the plugin's README](../intl/README.md) for details. The option
`HERMES_ENABLE_INTL`, which enabled Hermes's former built-in `Intl`, is
deprecated and ignored.

To install dependencies on Ubuntu:

    apt install build-essential cmake git ninja-build python3 tzdata libreadline-dev

On Arch Linux:

    pacman -S cmake git ninja python zip readline

On Mac via Homebrew:

    brew install cmake git ninja

Then install Rust with rustup, unless you configure with
`-DHERMES_BUILD_INTL=OFF`:

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

## Building on Linux and macOS

Hermes will place its build files in the current directory by default.
You can also give explicit source and build directories, use `--help` on the build scripts to see how.

Create a base directory to work in, e.g. `~/workspace`, and cd into it.
(Tip: avoid naming it `hermes`, as `hermes` will be one of several subdirectories in the workspace).
After `cd`ing, follow the steps below to generate the Hermes build system:

    git clone https://github.com/facebook/hermes.git
    cmake -S hermes -B build -G Ninja -DCMAKE_BUILD_TYPE=Debug

The build system has now been generated in the `build` directory. To perform the build:

    cmake --build ./build


## Release Build

The above instructions create an unoptimized debug build. The `-DCMAKE_BUILD_TYPE=Release` flag will create a release build:

    cmake -S hermes -B build_release -G Ninja -DCMAKE_BUILD_TYPE=Release
    cmake --build ./build_release

## Building on Windows

To build on Windows using Visual Studio with a checkout in the `hermes` directory:

    cmake -S hermes -B build -G 'Visual Studio 16 2019'
    cmake --build ./build

## Running Hermes

The primary binary is the `hermes` tool, which will be found at `build/bin/hermes`. This tool compiles JavaScript to Hermes bytecode. It can also execute JavaScript, from source or bytecode or be used as a REPL.

### Executing JavaScript with Hermes

    hermes test.js

## Compiling and Executing JavaScript with Bytecode

    hermes -emit-binary -out test.hbc test.js
    hermes test.hbc

## Running Tests

To run the Hermes test suite:

    cmake --build ./build --target check-hermes

To run Hermes against the test262 suite, you need to have a Hermes binary built
already and a clone of the [test262 repo](https://github.com/tc39/test262/):

    python3 hermes/utils/test_runner.py -b <hermes_build>/bin <test262>/test

E.g. if we configured at `~/hermes_build` (i.e. `~/hermes_build/bin/hermes` is
an executable) and cloned test262 at `~/test262`, then perform:

    python3 hermes/utils/test_runner.py -b ~/hermes_build/bin ~/test262/test

You can also give one or more subdirectories or files of the suite in place
of `~/test262/test`. This normal mode runs every test with the Intl plugin
disabled, so it tests Hermes without `Intl`, and it skips every test under
`test/intl402`.

The `intl402` tests run against the Intl plugin in plugin mode, which judges
each result against the plugin's list of expected failures. The list names
the test262 commit it was made for (on its `test262-commit:` line), and the
runner refuses a checkout at another commit. For example, on Linux:

    python3 hermes/utils/test_runner.py -b ~/hermes_build/bin \
      --intl-plugin ~/hermes_build/lib/libhermesintl.so \
      --intl-expectations hermes/intl/test/test262-expectations.txt \
      ~/test262/test/intl402

The plugin is `lib/libhermesintl.dylib` on macOS. `check-intl` runs the
plugin's tests and this test262 run, with the `built-ins` tests of the
methods the plugin replaces. It needs the build configured with
`-DHERMES_INTL_TEST262_DIR=<test262>`, and fails without it:

    cmake -S hermes -B ~/hermes_build -DHERMES_INTL_TEST262_DIR=$HOME/test262
    cmake --build ~/hermes_build --target check-intl

## Formatting Code

To automatically format all your changes, you will need `clang-format`, then
simply run:

    hermes/utils/format.sh

## AddressSanitizer (ASan) Build

 The `-HERMES_ENABLE_ADDRESS_SANITIZER=ON` flag will create a ASan build:

    git clone https://github.com/facebook/hermes.git
    cmake -S hermes -B asan_build -G Ninja -D HERMES_ENABLE_ADDRESS_SANITIZER=ON -DCMAKE_BUILD_TYPE=Debug
    cmake --build ./asan_build

You can verify the build by looking for `asan` symbols in the `hermes` binary:

    nm asan_build/bin/hermes | grep asan

### Other Tools

In addition to `hermes`, the following tools will be built:

- `hdb`: JavaScript command line debugger
- `hbcdump`: Hermes bytecode disassembler
- `hermesc`: Standalone Hermes compiler. This can compile JavaScript to Hermes bytecode, but does not support executing it.
- `hvm`: Standalone Hermes VM. This can execute Hermes bytecode, but does not support compiling it.
