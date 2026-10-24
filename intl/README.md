# The Hermes Intl plugin

`libhermesintl` is an implementation of ECMA-402 (`Intl` and the
locale-sensitive built-ins) on [ICU4X](https://github.com/unicode-org/icu4x),
built as a separate shared library. Hermes loads it automatically when it
creates a runtime and the library is present; adding or removing it needs no
Hermes rebuild. The plugin talks to the engine only through JSI.

The design, and the reasons behind it, are in
[the spec](../doc/superpowers/specs/2026-10-22-intl-plugin-design.md). The
Phase 0 plan is
[doc/superpowers/plans/2026-10-22-intl-plugin-phase0.md](../doc/superpowers/plans/2026-10-22-intl-plugin-phase0.md).

Phase 0 was infrastructure, and Phase 1 added `Intl.Locale`. `Intl` has
`getCanonicalLocales`, `supportedValuesOf` and `Locale`. The other
constructors arrive in later phases (see "Phasing" in the spec).

## Layout

| Path | What it is |
|---|---|
| `core/` | Rust crate `hermes_intl_core` on ICU4X, with an `extern "C"` interface. cbindgen generates `hermes_intl_core.h` from it at build time. |
| `core/src/lib.rs` | The C interface. |
| `core/src/locale.rs` | Well-formedness, canonicalization, `[[AvailableLocales]]`, lookup, the default-locale reduction. |
| `core/src/info.rs` | The lookups behind the `Intl.Locale` information methods: calendars, hour cycles, time zones, week data, text direction, numbering system and collations. |
| `core/src/host.rs` | Turning host settings (POSIX locale names, `TZ`, `/etc/localtime`) into ECMA-402 values. |
| `core/src/supported.rs` | The lists behind `Intl.supportedValuesOf`. |
| `core/data/` | `locales.txt`, `currencies.txt`, `numbering-systems.txt`, `calendar-preferences.txt`, `hour-cycles.txt`, `time-zones-by-region.txt`, `extension-value-aliases.txt`, `update-cldr-lists.py` and its tests. |
| `js/` | The ECMA-402 code in JS, concatenated in file-name order and compiled to bytecode by this build's `hermesc`. `00-header.js` and `99-footer.js` wrap the rest in a function. |
| `js/05-tags.js` | The Unicode locale grammar checks and the ECMA-402 abstract operations for locale objects. |
| `js/20-Locale.js` | `Intl.Locale`: the constructor, the getters, the information methods, `maximize` and `minimize`. |
| `jsi/` | C++: the entry point (`Plugin.cpp`), the embedded bytecode, the native helpers JS calls, and default detection (`Defaults.cpp`). |
| `test/lit/` | Lit tests of the real plugin; `test/lit/js/` tests the JS shared operations in isolation. |
| `test/unit/` | `HermesIntlUnitTests`: gtests for detection, `resolveDefaults`, and the objects `Plugin.cpp` passes to setup (in a real runtime). On Apple platforms also `IntlDefaultsProbe`, which prints what detection finds, for `test/lit/apple-defaults.js`. |
| `test/test262-expectations.txt` | The test262 tests expected to fail, each with a reason. |

The plugin never includes Hermes internals. Code here includes only
`<jsi/...>`, `<hermes/Public/IntlPlugin.h>`, its own headers and the
generated `hermes_intl_core.h`; `test/unit/` is the one exception.

## Building

The CMake option `HERMES_BUILD_INTL` controls the plugin. It is `ON` by
default for host builds on Linux and single-architecture macOS, and needs a
Rust toolchain, 1.88 or newer (the MSRV of `icu` 2.3). Without one,
configure fails and names the option; `-DHERMES_BUILD_INTL=OFF` gives a
Rust-free build.

For cross builds (Android, iOS and the other Apple targets, Emscripten),
Windows, multi-architecture macOS, and macOS with a single
`CMAKE_OSX_ARCHITECTURES` other than the one cargo builds for, the option
defaults to `OFF`, and turning it on is a configure error until Phase 6 of
the spec. Configure then prints why the option is off, unless it was set
explicitly. Multi-config generators (Xcode, Visual Studio) are not supported
yet either.

CMake runs `cargo build --locked`; `core/Cargo.lock` is committed. The
output is `<build>/lib/libhermesintl.so` on Linux and
`<build>/lib/libhermesintl.dylib` on macOS, next to `libhermesvm`.

The library links no JSI provider. Its JSI references stay undefined and
bind, when it is loaded, to the JSI code the host already exports: every
Hermes tool that creates runtimes exports its dynamic symbols
(`hermes_export_dynamic_symbols` in `cmake/modules/Hermes.cmake`), and
`libhermesvm` exports JSI. The plugin itself exports only
`hermes_intl_get_entry`. It must be built with the same `JSI_UNSTABLE`
setting as Hermes, and it works only with the Hermes it was built with. The
loader checks the bytecode and interface versions and the `JSI_UNSTABLE`
setting.

On macOS, a host signed with the hardened runtime rejects a plugin that is
not signed with the host's Team ID, and Hermes reports that as a fatal
error. Sign the plugin with the host's Team ID, or give the host the
`com.apple.security.cs.disable-library-validation` entitlement.

## How Hermes finds the plugin

For each new runtime, unless `EnableIntlPlugin` is false:

1. If `IntlPluginPath` is set, Hermes loads that library.
2. Otherwise it uses the default plugin, resolved once per process:
   1. a plugin the embedder registered with `hermes::registerIntlPlugin`
      before creating any runtime (the way to use a statically linked
      plugin);
   2. `libhermesintl.{so,dylib}` in the directory of the Hermes library or
      executable;
   3. if that directory is named `bin`, the same name in `../lib`;
   4. except on Apple platforms, the default library search path.

Apple platforms stop after step 3. dyld's error for a library name without
a path cannot reliably tell a missing library from a broken one: older dyld
says only `image not found`, and hardened or sandboxed processes get policy
reasons such as `relative path not allowed in hardened program`. Treating
those as "broken" would abort every runtime in such an app. Such a name also
makes dyld search the current directory, where anyone who controls it could
plant a library. On Apple platforms, keep the plugin next to the Hermes code,
register it, or set `IntlPluginPath`.

`IntlPluginPath` goes to `dlopen` as given. On Linux and Android, a name
without a slash is looked up on the library search path, not in the current
directory. dyld also searches the current directory for such a name, so on
Apple platforms give a path.

Errors:

| Situation | Result |
|---|---|
| `EnableIntlPlugin` is false | no plugin |
| Default resolution finds nothing (on Apple platforms the search path is not tried) | no plugin, silently |
| A library exists but cannot be loaded, lacks `hermes_intl_get_entry`, or has a mismatched version or `JSI_UNSTABLE` setting | fatal, naming the library and the reason |
| A registered entry has a mismatched version or `JSI_UNSTABLE` setting | fatal |
| `IntlPluginPath` is set and anything goes wrong | fatal |
| An explicit default locale or time zone is invalid | fatal |
| `registerIntlPlugin` is called after the default plugin was resolved | fatal |
| Setup throws | fatal |

On Windows and Emscripten only `registerIntlPlugin` works until Phase 6;
setting `IntlPluginPath` there is fatal.

A Worker gets the same plugin as its parent, with the parent's resolved
default locales and time zone.

## Configuration

| `RuntimeConfig` field | CLI flag | Meaning |
|---|---|---|
| `EnableIntlPlugin` (default `true`) | `-Xintl-plugin` | Load the plugin when one is available. |
| `IntlPluginPath` | `-Xintl-plugin-path=<path>` | Load the plugin from this library. |
| `IntlDefaultLocales` | `-Xintl-default-locale=<tag>`, repeated for a list | Default locales, most preferred first. Empty: detect. |
| `IntlDefaultTimeZone` | `-Xintl-default-time-zone=<zone>` | Default IANA time zone. Empty: detect. |

`hermes`, `hvm` and `test-runner` take these flags, and so do programs
compiled by `shermes`: on their own command line, or through
`shermes -exec <file> -Wx,<flag>`. The old `Intl` field (`-Xintl`) controls
only the old, built-in implementation.

When a default is not given, the plugin detects it from the host:
`LC_ALL`/`LC_MESSAGES`/`LANG` and `TZ`/`/etc/localtime` on Linux, and
CoreFoundation on macOS. The spec's "Default locale and time zone" section
lists every source and how odd values are normalized.

## Testing

```
cmake --build <build> --target check-intl
```

`check-intl` runs:

- `check-intl-rust`: `cargo test` on the Rust core;
- `check-intl-lit`: the lit tests in `test/lit/` and the gtests in
  `test/unit/`;
- `check-intl-runner`: unit tests of the test262 runner's plugin mode;
- `check-intl-data`: unit tests of `core/data/update-cldr-lists.py` on
  fixture CLDR XML;
- `check-intl-test262`: test262 `intl402` and the `built-ins` tests of the
  methods ECMA-402 overrides, through `utils/test_runner.py --intl-plugin`.

test262 needs a checkout at the commit named on the `test262-commit:` line
of `test/test262-expectations.txt`
(`c9aa6126e8f6fe2c6f668c65bfba771ffc8a6096`, the one CI uses). Point the
build at it:

```
git clone https://github.com/tc39/test262
git -C test262 checkout c9aa6126e8f6fe2c6f668c65bfba771ffc8a6096
cmake ... -DHERMES_INTL_TEST262_DIR=$PWD/test262
```

Without `HERMES_INTL_TEST262_DIR`, `check-intl` fails and says so. The runner
also refuses a checkout at another commit unless given
`--intl-allow-commit-mismatch`.

Without `--intl-plugin`, `test_runner.py` passes `-Xintl-plugin=false` to
every run, so the plugin in `<build>/lib` never changes the results of a
normal test262 run.

In plugin mode the runner runs each test's strict and non-strict variants
separately, and judges each against the expectations file. An expected
failure must be a JS exception or a test262 assertion; a crash, a timeout,
a loader error, or an internal error of the plugin is always a failure. A
listed test that passes is also a failure, so the list only shrinks. So is
an entry for a test that the run covers but that got no result in that
mode, because it was skipped or runs only in the other mode.

### Updating the expectations

After a change that makes tests pass (or a new test262 commit), rewrite the
file from a run:

```
python3 utils/test_runner.py --intl-plugin <build>/lib/libhermesintl.so \
  --intl-expectations intl/test/test262-expectations.txt \
  --intl-update-expectations -b <build>/bin \
  <test262>/test/intl402 <the built-ins paths from check-intl-test262>
```

The paths are the ones `check-intl-test262` passes in `intl/CMakeLists.txt`.
Existing reasons are kept. A new failure gets a reason from its path ("Phase
3: Intl.NumberFormat"), or `UNEXPLAINED` when no phase covers it. Every
`UNEXPLAINED` entry must be investigated and given a real reason by hand
before `check-intl` passes again. Entries for tests the run covers but did
not run in that mode are dropped. The file's header comment is written
afresh and no other comment is kept, so every explanation belongs in a
reason. The update refuses to run if anything crashed or hit a plugin
internal error.

### The JS isolation tests

The tests in `test/lit/js/` concatenate the files of `js/` that lie between
the header and the footer with a test of their own, and run the result with
the plugin disabled (`-Xintl-plugin=false`) and stub native helpers. Each
runs both from source and from bytecode. This is why code in `js/` is
written at column 0, outside the header's function.

## Writing plugin JS

User code must not be able to change what `Intl` does by tampering with
built-ins (the spec's "Resistance to tampering"); `test/lit/tamper.js`
checks this. Every file in `js/` follows these rules, and reviewers check
them:

1. Call only intrinsics captured in `01-intrinsics.js`. Never call a property
   of a user-reachable object except where the spec does a `Get`/`Call` on
   user values.
2. No syntax that consults user-modifiable state: no spread, no `for-of`, no
   array destructuring or array patterns, no tagged templates, no
   `Array.prototype` methods, no `String.prototype.split` (it looks up
   `@@split`).
3. Append to arrays with `listPush`, never `a[i] = v`, which runs setters on
   `Array.prototype`. Build every property descriptor and internal record
   with `__proto__: null`.
4. `ToString(v)` is `` `${v}` ``, `ToNumber(v)` is `+v`, and `ToObject`
   throws on `null`/`undefined` and then calls the captured `Object`.
5. Every function cites the spec section and numbers its steps in comments.
   The file header names the ECMA-402 draft it follows: the
   https://tc39.es/ecma402/ draft dated on the day the file is written. If
   the draft's step text differs from the code, follow the draft.
6. Code between `00-header.js` and `99-footer.js` is written at column 0, so
   lit tests can concatenate the files outside the IIFE.

The `defaults` and `native` objects that the C++ passes to setup have a null
prototype (`jsi/NativeHelpers.cpp`), so a property they lack reads as
`undefined`.

## Adding a native helper

1. Add one line to `jsi/NativeHelpers.def`:
   `HERMES_INTL_HELPER(name, Result, Arg...)` (no `Arg` for a helper that
   takes none), with the conversion tags described at the top of that file,
   and a comment saying what it does.
2. Define `hermes_intl::helpers::name` in a `jsi/*Helpers.cpp` file (for
   example `LocaleHelpers.cpp`). `NativeHelpers.h` declares it from the
   table: it takes the `jsi::Runtime &` and a `const` reference to each
   argument's C++ type. The host function and the argument and result
   conversions are generated from the table too. A new conversion tag needs
   a `TagType` and a `Convert` in `NativeHelpers.h`.
3. JS calls it as `native.name(...)`. JS validates every argument first.

Internal errors are C++ exceptions; Hermes turns them into an `Error`.

## Updating the CLDR lists

`core/data/currencies.txt`, `core/data/numbering-systems.txt`,
`core/data/calendar-preferences.txt`, `core/data/hour-cycles.txt`,
`core/data/time-zones-by-region.txt` and
`core/data/extension-value-aliases.txt` come from CLDR. When ICU4X is upgraded,
regenerate them from the CLDR release that ICU4X version uses, and commit the
result:

```
python3 intl/core/data/update-cldr-lists.py release-48-2
```

`calendar-preferences.txt`, `hour-cycles.txt` and `time-zones-by-region.txt`
feed the `Intl.Locale` information methods and are read by
`core/src/info.rs`. `time-zones-by-region.txt` lists zones as CLDR names
them; `info.rs` turns each into its primary identifier when it is used.

`extension-value-aliases.txt` holds the value aliases that
`core/src/locale.rs` replaces when it canonicalizes a tag (UTS 35 Annex C),
from every file in CLDR's `common/bcp47` (the generator lists that directory
through the GitHub API): a deprecated type maps to its
`preferred` value, and each alias of a type to the type's name, as
`<extension> <key> <alias> <value>` lines with the extension `u` or `t`.
Aliases that are not well-formed values (IANA names such as
`America/Havana`, or `gregorian`, which has 9 letters) cannot occur in a tag
and are left out. `core/data/test_update_cldr_lists.py` tests these rules
(`check-intl-data`); a Rust test checks that every line of the file
canonicalizes to its value.

`core/data/locales.txt` is the set of locales ICU4X's compiled data covers
(ICU4X's "recommended" set) and has no generator; replace it with the new
release's set by hand. The calendar and collation lists are in
`core/src/supported.rs`, and the unit list is in `js/10-Intl.js`.
