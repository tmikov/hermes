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
- one file per constructor (`20-Locale.js`, `30-Collator.js`);
- one file for the overridden built-ins, `80-builtins.js`. Each phase adds
  its overrides to it. Phase 2 put `localeCompare`, `toLocaleLowerCase`,
  `toLocaleUpperCase` and TransformCase there.

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
setup.

Methods that accept such a wrapper go through `UnwrapNumberFormat` /
`UnwrapDateTimeFormat` first: the `format` getter and `resolvedOptions` of
each. Every other method does a direct brand check and rejects the wrapper.
Phase 3 and Phase 4 test both paths.

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

When a constructor has resolved its options, it calls a native helper such as
`native.numberFormatInit(obj, ...resolvedFields)`. The helper builds the Rust
formatter and attaches it to the instance with `setNativeState`. Methods pass
the instance to native helpers, which fetch the formatter with
`hasNativeState` and `getNativeState` and check its type with
`std::dynamic_pointer_cast`. Hermes asserts that native state is present
when `getNativeState` is called, so the check comes first, and the typed
`hasNativeState<T>` would add a third lookup. Native state holds only Rust
and C++ data, never JS values.

`Intl.Collator` is the first constructor to do this. It calls
`native.collatorInit(obj, ...)`, which attaches a `CollatorState` that owns
the Rust collator. `native.collatorCompare(obj, x, y)` looks the state up and
compares.

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

### Locale matching

`"best fit"` behaves as `"lookup"`, which the spec permits. SpiderMonkey and
magrinj do the same.

## The native interface

### Rust core (`intl/core`)

- One crate depending on `icu` 2.x, `icu_experimental` (relative time,
  duration, currency / unit / compact formatting), `fixed_decimal`, and a
  TZDB offset provider (see Data).
- It fills the ICU4X gaps found during research:
  - NumberFormat scientific and engineering notation, and `formatRange`;
  - DateTimeFormat option-bag matching, until ICU4X's field-bag support
    (unstable, on ICU4X main for 2.4) can be used;
  - the `h24` hour cycle;
  - UTC offsets at an instant.
- Interface: opaque pointers (`HiNumberFormat *` and so on) with create,
  destroy and per-operation functions.
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
  compile-time assertion in the crate (`HiCollator`, the first such type,
  has one; ICU4X's `sync` feature is enabled);
- a `NativeState` destructor frees only its own Rust object and touches no
  per-runtime cache;
- per-runtime caches (such as the default formatters for the built-in
  overrides) live in JS, not in native code;
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

### C++ JSI layer (`intl/jsi`)

- An X-macro table (`NativeHelpers.def`) lists every native helper with its
  argument and return types; the host functions are generated from it.
- `Intl.Locale` needs the helpers `addLikelySubtags`, `removeLikelySubtags`,
  `calendarPreferences`, `hourCycles`, `timeZonesOfRegion`, `weekInfo`,
  `scriptDirection`, `defaultNumberingSystem` and `collationsOfLocale`. The
  last two use the plugin's one `[[AvailableLocales]]` list
  (`isAvailableLocale`) where the spec names
  `%Intl.Collator%.[[AvailableLocales]]` and
  `%Intl.NumberFormat%.[[AvailableLocales]]`. Phases 2 and 3 must keep it
  that way, or update these two methods. They must also use the same data:
  Phase 2's Collator takes `[[SortLocaleData]].[[co]]` from
  `collationsOfLocale`, and Phase 3's NumberFormat takes its default
  `[[nu]]` from `defaultNumberingSystem`, so `getCollations` and
  `getNumberingSystems` agree with the constructors.
- Each formatter type is a `jsi::NativeState` subclass that owns its Rust
  pointer and frees it in its destructor. `CollatorState`
  (`CollatorHelpers.cpp`) is the first. The destructor may run on Hermes'
  finalizer thread, which is why `HiCollator` must be `Send`.
- The conversion tags are `Bool` (results and arguments), `Object`
  (arguments), `Utf8`, `Utf16`, `OptUtf8`, `Int` (results) and `Undefined`
  (results). `Utf16` is lossless for unpaired surrogates.
- The Collator and case-mapping helpers are `collatorLocaleDefaults`,
  `collatorInit`, `collatorCompare` and `caseMap`. `collatorLocaleDefaults`
  returns ICU4X's default `caseFirst` and `ignorePunctuation` for a locale,
  which is where the Collator's locale data gets them.
- Native code reports only internal errors, as C++ exceptions. Hermes turns a
  C++ exception thrown from a host function into an `Error` made from its own
  intrinsic, with the message `Exception in HostFunction: ...`
  (`API/hermes/hermes.cpp`, the host-function trampoline); user code cannot
  intercept that. Native code keeps no JS values, so it captures no
  intrinsics. The `RangeError`s and `TypeError`s the spec requires are thrown
  by JS, which validates every argument first.

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
    digits for only 27 of them, so NumberFormat (Phase 3) must supply the
    digits of the others itself, from the table and CLDR's
    `numberingSystems.xml`.
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
  plugin until `all` (or `hermesintl`) is built.
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
