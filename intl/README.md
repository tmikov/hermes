# The Hermes Intl plugin

`libhermesintl` is an implementation of ECMA-402 (`Intl` and the
locale-sensitive built-ins) on [ICU4X](https://github.com/unicode-org/icu4x),
built as a separate shared library. Hermes loads it automatically when it
creates a runtime and the library is present; adding or removing it needs no
Hermes rebuild. The plugin talks to the engine only through JSI.

It provides:

- the `Intl` object with `getCanonicalLocales`, `supportedValuesOf` and the
  constructors `Locale`, `Collator`, `NumberFormat`, `PluralRules`,
  `DateTimeFormat`, `ListFormat`, `RelativeTimeFormat`, `DisplayNames`,
  `DurationFormat` and `Segmenter`;
- the ECMA-402 versions of `localeCompare`, `toLocaleLowerCase` and
  `toLocaleUpperCase` of String, the `toLocaleString` of Number, BigInt,
  Array and TypedArray, and the `toLocaleString`, `toLocaleDateString` and
  `toLocaleTimeString` of Date.

Its data comes from ICU4X 2.4 and CLDR 49, and the library bundles the
time zone rules of IANA tzdb 2026e.

Hermes has no other `Intl`: without the plugin a runtime has no `Intl`
global, and the locale-sensitive built-ins give Hermes's own results,
without throwing.

## Platforms

| Platform | Default | How it ships | Status |
|---|---|---|---|
| Linux and macOS hosts (universal macOS included) | on | `libhermesintl.so` or `libhermesintl.dylib` next to `libhermesvm` | supported |
| Android | on | `jni/<abi>/libhermesintl.so` in the AAR, for arm64-v8a, armeabi-v7a, x86 and x86_64 | supported |
| Apple platforms | on | `hermesintl.xcframework`, which `hermes-engine.podspec` vendors and links weakly | supported |
| Windows | off | `hermesintl.dll` next to the tools and `hermesvm.dll` | cross-built on Linux and tested under Wine, not yet on Windows itself |
| Emscripten | not supported | none: only `registerIntlPlugin` works | configure refuses the option |

## Building

The CMake option `HERMES_BUILD_INTL` controls the plugin. It needs a Rust
toolchain, 1.88 or newer, or 1.95 or newer for tvOS and visionOS, with the
standard library of each Rust target the build uses (configure says which
`rustup target add` is missing). Without Rust, configure fails and names
the option; `-DHERMES_BUILD_INTL=OFF` gives a Rust-free build without the
plugin.

A host build puts the plugin at `<build>/lib/libhermesintl.so` on Linux and
`<build>/lib/libhermesintl.dylib` on macOS, next to `libhermesvm`. Cross
builds compile the plugin's JavaScript with the host's `hermesc`, so they
need `-DIMPORT_HOST_COMPILERS=<host build>/ImportHostCompilers.cmake`.

[doc/Building.md](doc/Building.md) has the details: the output locations of
every build, Android, the Apple frameworks, and building for Windows on
Linux and running it under Wine.

## Using it

For each new runtime, Hermes uses, in this order: the library named by
`IntlPluginPath`; a plugin the embedder registered with
`hermes::registerIntlPlugin` before creating any runtime (the way to use a
statically linked plugin); the plugin in the directory of the Hermes library
or executable, or in `../lib` when that directory is `bin`; on Apple
platforms, `hermesintl.framework` next to `hermesvm.framework`; with glibc,
musl and bionic, the default library search path. A missing plugin means
no `Intl`; a plugin that is present but cannot be loaded is, with one
exception on Apple platforms, a fatal error, and so is any failure with
`IntlPluginPath` set. On Emscripten only `registerIntlPlugin` works. The
spec's
[Loader](../doc/superpowers/specs/2026-10-22-intl-plugin-design.md#loader)
section has the full rules and the errors.

| `RuntimeConfig` field | CLI flag | Meaning |
|---|---|---|
| `EnableIntlPlugin` (default `true`) | `-Xintl-plugin` | Load the plugin when one is available. |
| `IntlPluginPath` | `-Xintl-plugin-path=<path>` | Load the plugin from this library. |
| `IntlDefaultLocales` | `-Xintl-default-locale=<tag>`, repeated for a list | Default locales, most preferred first. Empty: detect. |
| `IntlDefaultTimeZone` | `-Xintl-default-time-zone=<zone>` | Default IANA time zone. Empty: detect. |

`hermes`, `hvm` and `test-runner` take these flags, and so do programs
compiled by `shermes`: on their own command line, or through
`shermes -exec <file> -Wx,<flag>`. The `Intl` field (`-Xintl`) is
deprecated and ignored.

When a default is not given, the plugin detects it from the host: the
locale environment variables and `TZ` or `/etc/localtime` on Linux,
CoreFoundation on Apple platforms, system properties and `TZ` on Android,
and the user's locale and time zone settings on Windows. The spec's
[Default locale and time zone](../doc/superpowers/specs/2026-10-22-intl-plugin-design.md#default-locale-and-time-zone)
section lists every source.

## Testing

```
cmake --build <build> --target check-intl
```

`check-intl` runs the Rust tests, the lit tests and gtests of the plugin,
the tests of the CLDR data generator and of the test262 runner's plugin
mode, and test262 `intl402` with the `built-ins` tests of the methods
ECMA-402 overrides. test262 needs a checkout at the commit named in
`test/test262-expectations.txt`, given to configure as
`-DHERMES_INTL_TEST262_DIR=<checkout>`.

[doc/Testing.md](doc/Testing.md) covers test262 in plugin mode, updating
the expected failures, and testing on Android, Apple platforms and
Windows.

## Working on the plugin

- `core/`: the Rust crate `hermes_intl_core`, on ICU4X, with a C interface,
  and the CLDR-derived data files in `core/data/`.
- `js/`: the ECMA-402 code, in JavaScript, compiled to bytecode and
  embedded in the library.
- `jsi/`: the C++ entry point, the native helpers the JavaScript calls, and
  default detection.
- `test/`: lit tests, gtests, the Android and Apple smoke tests and apps,
  and the test262 expectations.

The plugin never includes Hermes internals, only JSI and
`<hermes/Public/IntlPlugin.h>`. [doc/Development.md](doc/Development.md)
says where to start reading and gives the rules for the plugin's
JavaScript, adding a native helper, and updating the CLDR data.

## Known limitations

- [01a18652-9cdc](../dz/issues/01a18652-9cdc-7461-b19b-d1b6ddf96208.md):
  the plugin is about 19 MB per platform.
- Windows: no build has run on Windows itself. The spec's
  [Phasing](../doc/superpowers/specs/2026-10-22-intl-plugin-design.md#phasing)
  lists what has to be checked there before the plugin is on by default.
- React Native: its own podspec, in the `react-native` repository, needs the
  same changes as `hermes-engine.podspec` before React Native apps get the
  plugin (the spec's
  [Packaging](../doc/superpowers/specs/2026-10-22-intl-plugin-design.md#packaging)).
- The tvOS and visionOS simulator slices of the plugin are arm64 only, so
  an x86_64 simulator process has no plugin.

## Design

The design, and the reasons behind it, are in
[the spec](../doc/superpowers/specs/2026-10-22-intl-plugin-design.md).
