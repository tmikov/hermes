# Intl as a separate ICU4X-based plugin

Date: 2026-10-22
Status: draft, revised during Phase 0 planning and implementation

## Context

Hermes's current `Intl` maps ECMA-402 onto platform APIs: Foundation on Apple
(`lib/Platform/Intl/PlatformIntlApple.mm`) and Java through JNI on Android
(`lib/Platform/Intl/PlatformIntlAndroid.cpp`), with the JS-facing layer in
`lib/VM/JSLib/Intl.cpp`. It exists only on those two platforms, is limited, and
is impossible to maintain: the platform APIs do not map onto ECMA-402, and every
mismatch becomes a special case. On test262.fyi (2026-10-19) Hermes passes 358
of 3382 `intl402` tests; SpiderMonkey passes 3376 and V8 3343.

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

One shared library per platform: `libhermesintl.so` on Android and Linux, a
`.dylib` on macOS, a framework on iOS, a `.dll` on Windows. It contains the
Rust core, the ICU4X data, the JSI layer and the bytecode.

### Binary contract with the host

- The plugin uses JSI and nothing else from Hermes. JSI is not header-only:
  `jsi::JSError`'s constructors, `NativeState::~NativeState` and many helpers
  are compiled code in `API/jsi/jsi/jsi.cpp`. The plugin never contains its
  own copy of `jsi.cpp`, so there is one copy of every JSI type's type
  information in the process, exceptions thrown by the plugin are caught by
  the host as `jsi::JSError`, and `hasNativeState<T>`'s `dynamic_pointer_cast`
  works across the boundary.
- On Linux and macOS the plugin links no JSI provider. The `hermes` CLI links
  the VM statically, so there is no `libhermesvm` to link against. The
  plugin's JSI references stay undefined (`-undefined dynamic_lookup` on
  macOS) and bind, when the library is loaded, to the JSI code the host
  already exports:
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

  This is the pattern the Node-API test addons already use. Linking
  explicitly against a provider (the iOS framework, `libjsi.so` on Android, a
  Windows import library) comes in Phase 6.
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
  `IntlPluginPath`. On the library search path (not used on Apple
  platforms) the loader tries `dlopen` first; if that fails for a reason
  other than "not found", it promotes and retries once. A process without a
  plugin never pays for the promotion.
- The plugin exports only `hermes_intl_get_entry`: through a version script
  on Linux and `-exported_symbol` on Apple. Nothing from the Rust core and no
  C++ inline function or template instance is exported. The JSI types' type
  information stays undefined in the plugin and binds to the host's copy.
- The plugin and the host use the same C++ standard library (`c++_shared` on
  Android), with RTTI and exceptions enabled in the plugin, as they are in
  `API/hermes`. The plugin is compiled with the same `JSI_UNSTABLE` setting
  as Hermes, because that setting changes the layout of `jsi::Runtime`; the
  entry point records it, and the loader rejects a mismatch.
- The plugin is only supported with the Hermes it was built with. It is
  built in the same build and shipped next to Hermes, so both use the same
  JSI headers. The entry point (see Loading) carries the bytecode version
  and a plugin-interface version, and the loader rejects a mismatch before
  calling into the plugin, which usually catches a plugin from another
  release. The plugin-interface version is bumped whenever the entry
  point, the config struct or `IHermesBuiltinBytecode` changes. A plugin
  built against other JSI headers with the same versions is not detected;
  mixing builds that way is unsupported, as for any other JSI library.
- Phase 0 verifies this contract on Linux, macOS, and the Apple framework
  layout (which library exports JSI there), not only in the packaging phase.
  The results are under Risks.

### Build

- CMake option `HERMES_BUILD_INTL`. When it is on and no Rust toolchain
  (1.88 or newer, the MSRV of `icu` 2.3) is found, configure fails with a
  message naming the option. So the default configuration requires Rust;
  `-DHERMES_BUILD_INTL=OFF` gives a Rust-free build. This is the first Rust
  in the Hermes CMake build.
- Phase 0 builds for the host only: Linux and single-architecture macOS.
  `HERMES_BUILD_INTL` defaults to `ON` there, and to `OFF` for cross builds
  (Android, iOS and the other Apple targets, Emscripten), Windows,
  multi-architecture macOS, and macOS with a single
  `CMAKE_OSX_ARCHITECTURES` other than cargo's host architecture, which
  cargo, building for the host, cannot produce a core for. Setting it `ON`
  for those is a configure error until Phase 6, which adds cross builds
  through cargo. Multi-configuration generators are not supported yet
  either.
- CMake drives `cargo` for the host target. The library goes to
  `<build>/lib`, next to `libhermesvm`, where default resolution finds it.
- The plugin's JS is compiled by the `hermesc` from the same build. CMake
  reads the engine's bytecode version and passes it to the plugin's C++,
  which reports it in the entry point.
- `Cargo.lock` is committed, and every cargo invocation from CMake uses
  `--locked`. The ICU4X crates stay on 2.3.x (CLDR 48.2): `Cargo.toml` uses
  `~` requirements for them, and the lock file must not mix ICU4X minor
  versions. Shared dependencies such as `zerovec`, `yoke` and `tinystr` are
  not pinned exactly in `Cargo.toml`; the lock file holds them. Crates are
  fetched by default; offline builds through `cargo vendor` are supported
  but not required.

## Loading and installation

### The plugin's entry point

One exported `extern "C"` function, `hermes_intl_get_entry()`, returning a
pointer to a static struct, declared in `public/hermes/Public/IntlPlugin.h`:

- `struct_size`;
- `plugin_interface_version` and `bytecode_version`;
- `flags`, whose bit 0 is `JSI_UNSTABLE`. The loader rejects a mismatch,
  because that setting changes the layout of `jsi::Runtime`;
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
| Windows | `GetUserDefaultLocaleName` | the Windows zone ID, mapped to IANA through CLDR's `windowsZones` table |
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
documented, not reconciled.

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
   2. the standard library name (`libhermesintl.so`, `libhermesintl.dylib`)
      in the directory containing the Hermes code; and, when that directory
      is named `bin`, in `../lib`, which is the build and install layout
      (`bin/hermes`, `lib/libhermesintl.so`). `dladdr` tells whether the
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
   3. except on Apple platforms, the default library search path. A
      library tried there counts as absent only when the loader's error
      message says the library itself was not found. A missing dependency, a
      wrong architecture or unresolved symbols mean it was found but is
      broken. The loader knows the wording of glibc, musl and bionic; glibc's
      and musl's messages must match exactly. With any other C library the
      search path fails open: every error counts as not found, so a broken
      library there is ignored rather than aborting every runtime creation.

Apple platforms have no search-path step; default resolution there ends
after the `../lib` candidate. Trying a leaf name with dyld cannot reliably
tell "absent" from "broken". Older dyld reports only `image not found`, with
no list of the paths it tried. In a process with the hardened runtime or in a
sandbox, dyld gives policy reasons such as `relative path not allowed in
hardened program` or `file system sandbox blocked open()`. Counting those as
"broken" would make every runtime creation fatal in such a process, which
includes React Native apps. A leaf name also makes dyld search the current
directory, so whoever controls that directory could plant a library. An
Apple embedder that does not keep the plugin next to the Hermes code
registers it with `registerIntlPlugin` or sets `IntlPluginPath`.

`IntlPluginPath` is passed to `dlopen` as given. A name without a slash is
therefore looked up the way `dlopen` looks up such names: on Linux and
Android, the library search path, never the current directory. dyld also
searches the current directory for such a name, so on Apple platforms
`IntlPluginPath` should be a path.

On macOS, `proc_pidpath` rebuilds the path from the kernel's name cache, and
for a hard-linked executable it may report another link's path. Hard-linked
layouts are not supported for default resolution: such an install may load
the plugin from another link's directory.

Until Phase 6, the loader loads no libraries on Windows and Emscripten: only
`registerIntlPlugin` works there, and `IntlPluginPath` is fatal.

Loaded libraries are cached by path and never unloaded. The cache and the
default resolution are protected by a mutex, since runtimes can be created on
several threads.

**Errors.**

| Situation | Result |
|---|---|
| Disabled | no plugin |
| Default resolution finds nothing (on Apple platforms the search path is not tried) | no plugin, silently |
| A library exists but cannot be loaded, lacks `hermes_intl_get_entry`, or has a mismatched version or `JSI_UNSTABLE` flag | fatal, with a message naming the library and the reason |
| A registered entry has a mismatched version or `JSI_UNSTABLE` flag | fatal |
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
- during coexistence it also copies the old `Intl` field, so a parent with the
  old implementation disabled does not get a worker with it enabled.

### `RuntimeConfig`

New fields, appended after the last existing field so that
`HERMES_RUNTIME_CONFIG_VERSION` does not change
(`public/hermes/Public/CtorConfig.h`):

- `EnableIntlPlugin` (default `true`);
- `IntlPluginPath` (optional);
- `IntlDefaultLocales` (optional list of BCP 47 tags);
- `IntlDefaultTimeZone` (optional IANA name).

The existing `Intl` field (`public/hermes/Public/RuntimeConfig.h`) keeps
controlling only the old built-in implementation.

### Coexistence with the old implementation

The old implementation stays until the plugin reaches parity on Android and
iOS (see Phasing). Until then:

- With no plugin, behavior is exactly today's: the old `Intl` where it is
  compiled in and enabled, and the existing locale-sensitive fallbacks
  elsewhere.
- With the plugin, the plugin replaces `globalThis.Intl` and the built-ins,
  whatever the `Intl` field says. Each built-in is replaced in the phase
  that implements it; until then it keeps the behavior of the build without
  the plugin. So the presence of `Intl` does not imply ECMA-402
  `toLocaleString`: in Phase 0, `Infinity.toLocaleString()` is still
  `"Infinity"`, not en-US `"∞"`, in a build without the old `Intl`, and the
  old `Intl`'s `"+∞"` in a build with it, whether or not the `Intl` field
  enables it.
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
- `07-number.js`: Intl mathematical values, ToIntlMathematicalValue,
  FormatNumericToString with ToRawPrecision and ToRawFixed, and
  ComputeExponent, which NumberFormat, PluralRules and the later phases
  share (see "Number formatting");
- one file per constructor (`20-Locale.js`, `30-Collator.js`,
  `40-NumberFormat.js`, `45-PluralRules.js`). `41-number-patterns.js` holds
  the rest of NumberFormat: PartitionNumberPattern and the operations it
  uses, the range operations, the digits of the numbering systems,
  grouping, and the caches of the native number data;
- one file for the overridden built-ins, `80-builtins.js`. Each phase adds
  its overrides to it. Phase 2 put `localeCompare`, `toLocaleLowerCase`,
  `toLocaleUpperCase` and TransformCase there, and Phase 3 the
  `toLocaleString` methods of Number, BigInt, Array and TypedArray (see
  "Built-in overrides").

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

**Documented deviation.** For a plain function called with `new`, Hermes
creates `this` before the constructor runs (`createThisForConstruct_RJS`,
`lib/VM/Callable.cpp`). When `new.target.prototype` is not an object, it
falls back to `Object.prototype`; ECMA-402 requires the constructor's own
intrinsic prototype (for example `%Intl.NumberFormat.prototype%`). Only
subclassing with a non-object `prototype` can observe this. It can be fixed
later with a Hermes primitive that lets a JS function create its own `this`.
This applies to `Intl.Collator` as well, and `intl/test/lit/collator-objects.js`
pins it.

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
each. Every other method does a direct brand check and rejects the wrapper.
Phase 3 implemented the NumberFormat half and tests both paths
(`intl/test/lit/number-format-objects.js`); Phase 4 adds DateTimeFormat's.

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

## The native interface

### Rust core (`intl/core`)

- One crate depending on `icu` 2.x, `icu_plurals` and `icu_decimal` with
  their `unstable` feature (plural ranges, exact plural operands, compact
  decimal data), `icu_experimental` (currency data now; relative time and
  duration later), `fixed_decimal`, and a TZDB offset provider (see Data).
- It fills the ICU4X gaps found during research:
  - DateTimeFormat option-bag matching, until ICU4X's field-bag support
    (unstable, on ICU4X main for 2.4) can be used;
  - the `h24` hour cycle;
  - UTC offsets at an instant.

  NumberFormat's gaps (scientific and engineering notation, ranges, NaN and
  infinity, the parts) are filled in JS instead, from ICU4X's data (see
  "Number formatting").
- Interface: opaque pointers (`HiCollator *`, `HiPluralRules *`) with
  create, destroy and per-operation functions. A data lookup that gives a
  list of strings returns them in one `HiString`, separated by U+0000.
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
  compile-time assertion in the crate (`HiCollator` and `HiPluralRules`
  each have one; ICU4X's `sync` feature is enabled);
- a `NativeState` destructor frees only its own Rust object and touches no
  per-runtime cache;
- per-runtime caches (such as the default formatters for the built-in
  overrides) live in JS, not in native code;
- the CLDR files the crate compiles in (see "Data") are parsed once per
  process into immutable tables, behind `std::sync::OnceLock`;
- process-wide mutable native state is limited to the loader's cache, which is
  locked.

### Strings

Two kinds of string cross the boundary, handled differently:

- **Option strings and formatter output** (locale tags, option values,
  formatted numbers and dates): UTF-8 both ways (`String::utf8`,
  `createFromUtf8`). ICU4X writes UTF-8.
- **Arbitrary user text** (Collator and `localeCompare` inputs, case mapping
  input, ListFormat elements, Segmenter input): copied out as UTF-16 with
  `String::utf16` and never converted to UTF-8 in a way that could lose
  unpaired surrogates.
  - Collator uses ICU4X's UTF-16 comparison (`compare_utf16`), which reads
    an unpaired surrogate as U+FFFD. Two different unpaired surrogates
    therefore compare equal, and `compare(x, x)` is 0.
  - Segmenter uses ICU4X's UTF-16 segmentation, and all indices are UTF-16
    code-unit indices into the original string.
  - Case mapping splits the input at unpaired surrogates, maps each
    well-formed run, and passes unpaired surrogates through unchanged. Each
    run is mapped on its own, so a context rule such as the final sigma does
    not look across an unpaired surrogate. The result comes back as a
    `HiString16` (a UTF-16 buffer that C++ copies and then frees with
    `hi_string16_free`).
  - ListFormat sends the element texts to native code, because CLDR list
    patterns depend on them (Spanish `y` becomes `e` before a word starting
    with an "i" sound; Spanish disjunctions and Hebrew conjunctions have
    similar rules). Native code uses the texts only to choose the patterns, and
    returns the literal pieces plus, for each element slot, the index of the
    element that goes there. JS builds the result from the literal pieces and
    the original element strings, so the output is lossless even though
    native code never needs to return user text.
  - Tests cover unpaired surrogates and supplementary characters for each of
    these.

Reading strings in place with `getStringData` is a later optimization. It
delivers ASCII and UTF-16 chunks, possibly in several callbacks, and forbids
any runtime call inside the callback, so two-string operations would still have
to copy the first string before reading the second.

### Parts

A native `...ToParts` helper returns the formatted string and a flat array of
integer records, four per part: `start`, `end`, `type`, `extra`.

- `start` and `end` are UTF-16 code-unit offsets into the returned string.
- `type` indexes a fixed table of part type names shared by JS and native.
- `extra` carries the per-part attribute the spec defines for that service:
  - range formatting: the `source` (`shared`, `startRange`, `endRange`);
  - RelativeTimeFormat and DurationFormat: the `unit`, an index into a fixed
    unit-name table, or `0` for parts that have no `unit` property. In
    RelativeTimeFormat, every part that comes from the formatted number has
    the unit, including literals inside the number pattern;
    the literals of the surrounding relative-time pattern do not;
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
  (`CollatorHelpers.cpp`) and `PluralRulesState`
  (`PluralRulesHelpers.cpp`). The destructor may run on Hermes' finalizer
  thread, which is why `HiCollator` and `HiPluralRules` must be `Send`.
- The conversion tags are `Bool` (results and arguments), `Object`
  (arguments), `Utf8`, `Utf16`, `OptUtf8`, `Int`, `Utf8List` (results) and
  `Undefined` (results). `Utf16` is lossless for unpaired surrogates. `Int`
  is an integer in `int` range both ways; as an argument, anything else (a
  fraction, NaN, a string) is an internal error naming the helper, and -0
  is 0. `Utf8List` is a new JS array of strings. It is built with
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
    | percent sign, exponent symbol, infinity, NaN, approximately sign, percent pattern, range pattern, currency decimal and group separators | `intl/core/data/number-symbols.txt` |
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
  plugin uses, and the 0.9 MB of generated CLDR files. The share of each
  ICU4X data struct has not been measured yet.
- **Time zones.** ICU4X provides zone names and IANA identifiers but not the
  offset rules. The plugin bundles TZDB rules compiled into the Rust core, so
  offsets are the same on every platform and test262 results are
  reproducible. The provider must compute the UTC offset at any instant from
  compiled-in data with no file access. The Phase 4 plan picks the crate
  (temporal_rs's `zoneinfo64` data, or TZif files compiled in at build time
  are the candidates) and pins the TZDB version.
- **Segmenter** uses ICU4X's default word segmenter, which includes the
  Chinese and Japanese dictionary. Its size is measured when Segmenter lands.

## Testing

- **Acceptance:** test262 `intl402`, plus the `built-ins` tests of the
  overridden methods, with the `hermes` binary from the same build. The
  test262 pin is the commit CI already uses,
  `c9aa6126e8f6fe2c6f668c65bfba771ffc8a6096`.
- **Runner:** `utils/test_runner.py` has a plugin mode (`--intl-plugin`,
  `utils/testsuite/intl_plugin.py`). Its normal mode skips the Intl tests
  through `skiplist.json` and only reports passes; plugin mode judges every
  result against an expectations file instead. In plugin mode:
  - every run gets `-Xintl-plugin-path=<plugin>`,
    `-Xintl-default-locale=en-US` and `-Xintl-default-time-zone=UTC`, and
    runs with `TZ=UTC`, so that the engine's local time agrees with the
    plugin's zone (test262 builds dates from local fields, `new Date(2017,
    11, 12, h)`), and results do not depend on the host;
  - the `intl_tests` gate and the platform skip list are ignored for every
    path: on the hosts plugin mode supports, their entries are all about the
    old `Intl`, and ignoring them gives the same coverage on every platform.
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
  `intl402` that use `Intl` when it exists would see a partial one. test262
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
  "Built-in overrides").
- **Lit tests and gtests** for Hermes-specific behavior: each loader path and
  each row of the error table, `EnableIntlPlugin=false`, Workers inheriting
  the parent's settings, both runtime factories, the function-shape table,
  and the private-field attachment pattern.
- **Rust unit tests** for the code that fills ICU4X gaps. Each must be shown
  able to fail: break the code under test and name the test that catches it.
- **Sanitizers:** `check-intl` runs locally in the ASan build; CI runs it
  without sanitizers. In the ASan build the Rust static library is linked in
  uninstrumented; the JSI layer and the engine are instrumented.
- **Mobile:** smoke tests on an Android emulator and an iOS simulator in the
  packaging phase, plus `intl402` on device if the runner can be made to work
  there. Conformance work happens on Linux.

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
   overrides.
5. **Remaining constructors.** ListFormat, RelativeTimeFormat, DisplayNames,
   DurationFormat (built on NumberFormat and ListFormat), Segmenter.
6. **Packaging.** Cross builds through cargo (Android ABIs, iOS and the
   other Apple targets), universal macOS, and Windows, both the plugin build
   and library loading in the loader; linking the plugin explicitly against
   the JSI provider of each packaging (the iOS framework, `libjsi.so` on
   Android, a Windows import library). The Android `.so` in the Hermes
   Android build and AAR; the Apple xcframework and podspec; on-device smoke
   tests.
7. **Delete the old implementation:** `lib/VM/JSLib/Intl.cpp`,
   `lib/Platform/Intl/`, the `HERMES_ENABLE_INTL` paths in the VM, its
   `PredefinedStrings`, `NativeFunctions` and `RuntimeHermesValueFields`
   entries, and `android/intltest`. The `Intl` `RuntimeConfig` field stays
   in place, documented as deprecated and ignored, because removing a field
   from the middle of `RuntimeConfig` would change its layout and require a
   version bump.

## Risks

- **ICU4X gaps may be larger than found.** The research (2026-10) covered
  ICU4X 2.3 and main. DateTimeFormat option-bag matching in particular could
  be substantial if ICU4X's field-bag support does not stabilize.
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
  is viable, and is the expected route for iOS, where Apple's linker
  discourages `-undefined dynamic_lookup`. Phase 6 must confirm on an iOS
  target.

  Code signing: a host signed with the hardened runtime (`codesign -o
  runtime`) has library validation on, and `dlopen` rejects a plugin that
  is not signed with the host's Team ID ("not valid for use in process").
  The loader treats that as a broken plugin, so it is fatal. Such a host
  must sign the plugin with its own Team ID, or carry the
  `com.apple.security.cs.disable-library-validation` entitlement. Checked
  on macOS with ad-hoc signatures: the rejection, and loading with the
  entitlement. Signing both with one Team ID was not checked (no signing
  identity on the test machine); Phase 6 packaging signs the plugin with
  the app.

  Also verified on macOS: the build, `check-intl` (test262 plugin mode gives
  the same results as on Linux) and `check-hermes` pass; `shermes` with
  `libhermesvm` loaded `RTLD_LOCAL` works with the promotion described under
  "Binary contract with the host"; `-exported_symbol` leaves exactly one
  exported symbol; CoreFoundation detection reports the Mac's settings, and
  `TZ` overrides the time zone. That CoreFoundation honours a per-process
  `-AppleLanguages` override is checked by `intl/test/lit/apple-defaults.js`
  through `IntlDefaultsProbe`, a plain executable, because the gtest
  binary's argument parser rejects that argument.
- **Global symbols in `RTLD_LOCAL` hosts.** The loader's promotion makes
  `libhermesvm`'s exported symbols global in any process that loads it
  `RTLD_LOCAL` and then loads a plugin. A library loaded later can bind to
  them instead of its own copy.
- **Fail-open search path.** On C libraries whose `dlopen` messages the
  loader does not know, a broken plugin found on the search path is
  silently ignored until Phase 6.
