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

Phase 0 was infrastructure, Phase 1 added `Intl.Locale`, Phase 2 added
`Intl.Collator` and the ECMA-402 versions of `localeCompare`,
`toLocaleLowerCase` and `toLocaleUpperCase`, Phase 3 added
`Intl.NumberFormat`, `Intl.PluralRules` and the ECMA-402 `toLocaleString`
of Number, BigInt, Array and TypedArray, and Phase 4 added
`Intl.DateTimeFormat`, with the time zone rules of IANA tzdb 2026e bundled
in the library, and the ECMA-402 `toLocaleString`, `toLocaleDateString`
and `toLocaleTimeString` of Date. Phase 5 added the five remaining
constructors: `Intl.ListFormat`, `Intl.RelativeTimeFormat`,
`Intl.DisplayNames`, `Intl.DurationFormat` and `Intl.Segmenter`. `Intl`
has `getCanonicalLocales`, `supportedValuesOf`, `Locale`, `Collator`,
`NumberFormat`, `PluralRules`, `DateTimeFormat`, `ListFormat`,
`RelativeTimeFormat`, `DisplayNames`, `DurationFormat` and `Segmenter`.
Later phases package it for the other platforms and delete Hermes's old
`Intl` (see "Phasing" in the spec).

## Layout

| Path | What it is |
|---|---|
| `core/` | Rust crate `hermes_intl_core` on ICU4X, with an `extern "C"` interface. cbindgen generates `hermes_intl_core.h` from it at build time. |
| `core/src/lib.rs` | The C interface. |
| `core/src/locale.rs` | Well-formedness, canonicalization, `[[AvailableLocales]]`, lookup, the default-locale reduction. |
| `core/src/info.rs` | The lookups behind the `Intl.Locale` information methods: calendars, hour cycles, time zones, week data, text direction, numbering system and collations. |
| `core/src/host.rs` | Turning host settings (POSIX locale names, `TZ`, `/etc/localtime`) into ECMA-402 values. |
| `core/src/supported.rs` | The lists behind `Intl.supportedValuesOf`. |
| `core/src/collator.rs` | The ICU4X collator for a set of resolved Collator options, and the locale defaults. |
| `core/src/casemap.rs` | Upper and lower case mapping of UTF-16 text, split at unpaired surrogates. |
| `core/src/plurals.rs` | ICU4X's plural rules and ranges for a locale, and selection from exact operands. |
| `core/src/number.rs` | The number data NumberFormat formats with: symbols, decimal, percent and range patterns, currency patterns and digits, compact patterns and compact currency patterns, as lists of strings, with CLDR's resolution of numbering systems. |
| `core/src/units.rs` | Unit patterns, including the `X-per-Y` compounds. |
| `core/src/cldr.rs` | The parsers and lookups of the generated CLDR number files. |
| `core/src/tz.rs` | The time zone rules: a reader of the IANA tzdb's TZif data that the `jiff-tzdb` crate embeds, giving the UTC offset and daylight saving flag of a zone at any ECMAScript instant. |
| `core/src/date_data.rs` | The parsers and lookups of the generated CLDR date files: a locale's date and time patterns for a calendar, and its flexible day periods. |
| `core/src/datetime.rs` | The native half of `Intl.DateTimeFormat`: a formatter's ICU4X calendar, time zone and names, and for an instant the local time (ToLocalTime's record) and the text of the names its patterns use. |
| `core/src/list.rs` | The native half of `Intl.ListFormat`: ICU4X's list formatter for a locale, type and style, and the literal text it puts around the elements of a list, chosen by the list's last element. |
| `core/src/relativetime.rs` | The relative time patterns and names of a locale and style, from `icu_experimental`'s data. |
| `core/src/displaynames.rs` | The display names of `Intl.DisplayNames`: single language, dialect, region, script, variant, currency, calendar and date-time field names, from `icu_experimental`'s data and `display-names.txt`, and the locale display patterns. |
| `core/src/segmenter.rs` | The native half of `Intl.Segmenter`: ICU4X's grapheme, word (dictionary) and sentence segmenters, every boundary of a string with whether each word segment is word-like, FindBoundary, and the workarounds for three ICU4X 2.3 word segmentation bugs. |
| `core/data/` | `locales.txt`, `currencies.txt`, `numbering-systems.txt`, `calendar-preferences.txt`, `hour-cycles.txt`, `time-zones-by-region.txt`, `extension-value-aliases.txt`, the number data files `number-symbols.txt`, `units.txt`, `currency-symbols.txt`, `explicit-number-systems.txt` and `compact-currency.txt`, the date data files `date-patterns.txt` and `day-periods.txt`, `display-names.txt` (the DisplayNames data ICU4X lacks), `LICENSE` (the Unicode License of the CLDR-derived files), `update-cldr-lists.py` and its tests. |
| `js/` | The ECMA-402 code in JS, concatenated in file-name order and compiled to bytecode by this build's `hermesc`. `00-header.js` and `99-footer.js` wrap the rest in a function. |
| `js/05-tags.js` | The Unicode locale grammar checks and the ECMA-402 abstract operations for locale objects. |
| `js/06-iterators.js` | ECMA-262's iterator protocol (GetIterator, IteratorStepValue, and IteratorClose for a throw), which ListFormat's StringListFromIterable uses in place of `for-of`. |
| `js/07-number.js` | Intl mathematical values, ToIntlMathematicalValue, and the exact rounding (FormatNumericToString, ToRawPrecision, ToRawFixed, ComputeExponent) that NumberFormat, PluralRules and later constructors share. |
| `js/20-Locale.js` | `Intl.Locale`: the constructor, the getters, the information methods, `maximize` and `minimize`. |
| `js/30-Collator.js` | `Intl.Collator`: the constructor, `supportedLocalesOf`, `resolvedOptions` and the `compare` getter. |
| `js/40-NumberFormat.js` | `Intl.NumberFormat`: the constructor and its options, the legacy constructor behavior, `supportedLocalesOf`, `resolvedOptions`, the `format` getter and the other prototype methods. |
| `js/41-number-patterns.js` | Formatting: PartitionNumberPattern and the operations it uses, the parts, the ranges, grouping, the digits of the numbering systems, and the caches of the native data. |
| `js/45-PluralRules.js` | `Intl.PluralRules`: the constructor, `supportedLocalesOf`, `resolvedOptions`, `select` and `selectRange`. |
| `js/50-DateTimeFormat.js` | `Intl.DateTimeFormat`: the constructor with CreateDateTimeFormat, the legacy constructor behavior, the time zone identifier operations, `[[LocaleData]]`, `supportedLocalesOf`, `resolvedOptions`, the `format` getter and the other prototype methods. |
| `js/51-datetime-patterns.js` | The DateTime Format Records made from CLDR's patterns, `[[formats]]` and `[[styles]]`, BasicFormatMatcher, the best-fit matcher, DateTimeStyleFormat, and the caches of the native date data. |
| `js/52-datetime-format.js` | Formatting: ToLocalTime, FormatDateTimePattern, PartitionDateTimePattern, PartitionDateTimeRangePattern, and FormatDateTime, FormatDateTimeToParts, FormatDateTimeRange and FormatDateTimeRangeToParts. |
| `js/53-datetime-ranges.js` | The range patterns of each DateTime Format Record, from CLDR's `intervalFormats` and `intervalFormatFallback`. |
| `js/54-datetime-names.js` | The CLDR name symbols a formatter asks native code for, and the flexible day periods. |
| `js/60-ListFormat.js` | `Intl.ListFormat`: the constructor, `supportedLocalesOf`, `resolvedOptions`, `format` and `formatToParts`, with CreateListFormat and CreatePartsFromList, which DurationFormat also uses, and StringListFromIterable. |
| `js/62-RelativeTimeFormat.js` | `Intl.RelativeTimeFormat`: the constructor, `supportedLocalesOf`, `resolvedOptions`, `format` and `formatToParts`, and the cache of the native relative time data. |
| `js/64-DisplayNames.js` | `Intl.DisplayNames`: the constructor, `supportedLocalesOf`, `resolvedOptions` and `of`, with the composition of language names by UTS 35's algorithm. |
| `js/66-DurationFormat.js` | `Intl.DurationFormat`: the constructor, `supportedLocalesOf`, `resolvedOptions`, `format` and `formatToParts`, and the operations from ToIntegerIfIntegral to PartitionDurationFormatPattern, with exact BigInt arithmetic. |
| `js/68-Segmenter.js` | `Intl.Segmenter`: the constructor, `supportedLocalesOf`, `resolvedOptions` and `segment`, the Segments objects with `containing` and `[Symbol.iterator]`, and the Segment Iterators. |
| `js/80-builtins.js` | The overrides of `localeCompare`, `toLocaleLowerCase`, `toLocaleUpperCase`, the `toLocaleString` of Number, BigInt, Array and TypedArray, and the `toLocaleString`, `toLocaleDateString` and `toLocaleTimeString` of Date, with their default formatters, and TransformCase. |
| `jsi/` | C++: the entry point (`Plugin.cpp`), the embedded bytecode, the native helpers JS calls, and default detection (`Defaults.cpp`). |
| `jsi/CollatorHelpers.cpp` | The collator helpers and `CollatorState`, the native state that owns the Rust collator. |
| `jsi/PluralRulesHelpers.cpp` | The plural rules helpers and `PluralRulesState`, the native state that owns the Rust plural rules. |
| `jsi/NumberHelpers.cpp` | The number data helpers (`numberSymbols`, `currencyPatterns`, `compactCurrencyPatterns`, `compactPatterns`, `unitPatterns`, `currencyDigits`). |
| `jsi/DateTimeFormatHelpers.cpp` | The date helpers (`dateTimePatterns`, `dayPeriods`, `canonicalizeTimeZone`, `dateTimeFormatInit`, `dateTimeFormatLocalTime`) and `DateTimeFormatState`, the native state that owns a formatter's Rust calendar, time zone and names. |
| `jsi/ListFormatHelpers.cpp` | The list helpers (`listFormatInit`, `listFormatLiterals`) and `ListFormatState`, the native state that owns a formatter's Rust list formatter. |
| `jsi/RelativeTimeFormatHelpers.cpp` | The `relativeTimePatterns` helper. |
| `jsi/DisplayNamesHelpers.cpp` | The display name helpers (`displayName`, `localeDisplayPatterns`). |
| `jsi/SegmenterHelpers.cpp` | The segmentation helpers (`segmenterInit`, `segmentsInit`, `segmentsFindBoundary`, `segmentsIsWordLike`), `SegmenterState`, which owns a Segmenter's Rust segmenter, and `SegmentsState`, which owns the boundaries of a Segments object's string. |
| `jsi/StringHelpers.cpp` | The `caseMap` helper. |
| `test/lit/` | Lit tests of the real plugin; `test/lit/js/` tests the JS shared operations in isolation, and `test/lit/Inputs/` holds files the tests use, such as `datetime-locales.js`: the 40 locales and the `formatMatcher: "basic"` check that `datetime-format-locales.js` and its `-2`, `-3` and `-4` parts share, a quarter of the locales each, so that lit runs them in parallel, and `datetime-fields.js`: the check that best fit keeps every requested field, which `datetime-format-fields-1.js` to `-10.js` run, one locale each. |
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
before `check-intl` passes again; while there is one, the update still
writes the file but lists them all and exits with status 1. Entries for tests the run covers but did
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
3. Append to arrays with `listPush` and replace an element with `listSet`,
   never `a[i] = v`, which runs setters on `Array.prototype`. Build every
   property descriptor and internal record with `__proto__: null`.
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
   a `TagType` and a `Convert` in `NativeHelpers.h`. The tags are `Bool`
   (an argument or a result), `Object` (an argument), `Utf8`, `Utf16`
   (UTF-16 text, lossless for unpaired surrogates, for text to compare or
   transform), `OptUtf8`, `Int` (an integer in `int` range, as an argument
   or a result; an argument that is anything else is an internal error),
   `Double` (an argument: a finite number, such as a time value; anything
   else is an internal error), `Utf8List` (a result: a new array of
   strings, built from JSON so that no setter on `Array.prototype` runs) and
   `Undefined` (a result).
3. JS calls it as `native.name(...)`. JS validates every argument first.

Internal errors are C++ exceptions; Hermes turns them into an `Error`.

## Updating the CLDR lists

`core/data/currencies.txt`, `core/data/numbering-systems.txt`,
`core/data/calendar-preferences.txt`, `core/data/hour-cycles.txt`,
`core/data/time-zones-by-region.txt`, `core/data/extension-value-aliases.txt`,
`core/data/number-symbols.txt`, `core/data/units.txt`,
`core/data/currency-symbols.txt`, `core/data/explicit-number-systems.txt`,
`core/data/compact-currency.txt`, `core/data/date-patterns.txt`,
`core/data/day-periods.txt` and `core/data/display-names.txt` come from CLDR.
When ICU4X is upgraded, regenerate them from the CLDR release that ICU4X
version uses, and commit the result:

```
python3 intl/core/data/update-cldr-lists.py release-48-2
```

`number-symbols.txt`, `units.txt` and `currency-symbols.txt` come from the CLDR
JSON npm packages `cldr-core`, `cldr-numbers-full` and `cldr-units-full` (the
generator downloads version 48.2.0 for the tag `release-48-2` from the npm
registry), whose locale data is already resolved. `number-symbols.txt` has one
tab-separated line per locale and numbering system: `<locale> <nu>
<percentSign> <exponential> <infinity> <nan> <approximatelySign>
<percentPattern> <rangePattern> <currencyDecimal> <currencyGroup>
<timeSeparator>`; the currency separators are empty where the locale has no
separators of its own for currency formats (de-AT's `currencyGroup` ".",
fr-CH's `currencyDecimal` "."), which the currency style uses, and
`timeSeparator` is what DurationFormat's digital style puts between hours,
minutes and seconds (":"; "." in da's latn, "٫" in ug's arabext).
`units.txt` has a string table (`strings <count>`, then one pattern per line),
then for each locale a `locale <tag> <parent>` line and
the entries `<width> <unit> <key>=<index>...` that differ from its parent chain
(`<unit>` is `-` for the width's compound pattern). `currency-symbols.txt` (35
KB) is tab-separated: for each locale a `locale <tag> <parent>` line, with the
same parents as `units.txt`, then `<code> <symbol> <narrow>` for each currency
whose symbol or narrow symbol differs from what its parent chain gives, with an
empty field for the one that does not, followed by `<pattern> <decimal>
<group>` where the locale's XML has a format of its own for the currency
(CLDR's `<currency type="X">` `<pattern>`, `<decimal>` and `<group>`: en-150's
EUR `¤#,##0.00`, pt-CV's CVE decimal `$`), and `lateral <field> <nu> <type>`
lines for the locale's own currencyFormat patterns (`<type>` standard or
accounting) and symbols decimal and group (`<type>` `-`), where a parent has
per-currency formats: UTS 35's lateral inheritance ends the search for a
per-currency field at them (tr35.md, "Lateral Inheritance"). The per-currency
fields come from the XML because cldr-json resolves them vertically only. The
generator reads them back as `core/src/cldr.rs` does and compares the result
with lateral inheritance over the XML for latn, deva, arab and arabext; it
also checks that a per-currency pattern is `#,##0.00` with the currency sign
before or after it, which is all CLDR 48.2 has. CLDR's fallbacks are applied first (no
narrow symbol means the symbol; no symbol means the code), and a currency no
locale of the chain lists uses its code. ICU4X's own currency symbol data
leaves out symbols equal to the code, so its locale fallback finds a parent's
(`af` USD would be `US$`, not `USD`). For units and currency symbols the
generator checks that reading the file back along the parent chain gives each
locale's resolved data. It also checks that every locale's `currencySpacing` is
root's, the one rule `core/src/number.rs` implements. A locale with no
directory in the JSON packages (`zh-HK`, `sr-ME`) is read from the directory
with the script of `likelySubtags` inserted (`zh-Hant-HK`). The sanctioned
units come from `SANCTIONED_UNITS` in `js/10-Intl.js`. Every value is checked
to contain no tab or newline. Each file starts with `#` comment lines, which a
parser skips. `core/data/LICENSE` is the Unicode License V3 for these files.

`explicit-number-systems.txt` (55 KB) comes from CLDR's `common/main/*.xml`
instead (the generator fetches root's and those of every locale of
`locales.txt` and its CLDR parent chain from GitHub at the release tag): the
explicit values the XML has for numbering systems outside a locale's default
and other numbering systems, which cldr-json and ICU4X leave out. For each
locale of the chains it has a `locale <tag> <parent>` line (the CLDR parent),
then tab-separated lines `symbols <nu> <decimal> <group> <minusSign> <plusSign>
<percentSign> <exponential> <infinity> <nan> <approximatelySign>
<timeSeparator>`, `percent <nu> <pattern>`, `range <nu> <pattern>` and
`currency <nu> <standard> <standardAlpha> <accounting> <accountingAlpha>`, with
an empty field for a value the locale does not have; `=standard` is an
accounting pattern that aliases to the standard one. Values marked as inherited (`↑↑↑`) or of draft level unconfirmed or
provisional, which cldr-json and ICU leave out, are skipped. Root has such data
for arab (symbols, percent and currency patterns) and arabext (symbols) in CLDR
48.2, and so do many locales (fr's arab plus sign, cv's NaN for every numbering
system). Symbols' alt variants (en_ZA's latn `decimal alt="us"`) are not the
symbols and are skipped. The generator fails on an alias outside root, on
root's explicit decimal formats, and on currency separators (`currencyDecimal`, `currencyGroup`) of such a numbering system. The
other kinds of data for such numbering systems are not read, and a locale's
explicit ones are dropped without notice: decimal formats (the grouping and
compact patterns; az's arab and arabext ones in CLDR 48.2), scientific and
rational formats, and miscPatterns other than the range pattern.
`core/src/number.rs` takes a numbering system that is one of the locale's own
from ICU4X and `number-symbols.txt`; for any other it takes each symbol and the
percent, range and currency patterns from the nearest locale of the chain in
this file, else the locale's latn value, which root's aliases point to, and the
grouping and compact patterns from the locale's latn data. Its currency
separators are the locale's latn ones where root aliases its symbols to latn
(de-AT deva groups currencies with "."), and the plain separators where root's
symbols for it are explicit (arab, arabext), which have none.

`compact-currency.txt` (39 KB) holds CLDR's short currency patterns
(`currencyFormatLength type="short"`), which compact notation uses with the
currency style and which ICU4X does not have. It has a string table
(`strings <count>`, then one pattern per line, with the pattern's run of digits
written as one `0`), then for each locale a `locale <tag> <parent>` line, with
the same parents as `units.txt`, and tab-separated lines `<nu> <n>[-<m>]
<zeros> <count>=<index>[,<alt>[<delta>]] ...` for the magnitudes 10^n to 10^m:
the patterns of each plural count that differ, by index, with the
alphaNextToNumber alternate where it differs from the pattern, whose digit run
is zeros long at 10^n and one longer at each next magnitude (0 for the pattern
"0", which does not compact), and delta (`+2`) more for the alternate where its
run differs (kok's 10^14 `¤000LCr`). A locale's own numbering systems have
cldr-json's patterns. Another numbering system has, pattern by pattern, the
explicit one of the nearest locale of its XML chain (only ba and kok have any in
CLDR 48.2), else the locale's latn one, which root's aliases point to; root has
short currency formats for latn only, which the generator checks, so root's
arab currency formats give latn's. A numbering system other than latn lists
only the patterns that differ from what the chain gives for it, or where that
has none, from latn's. The generator checks that reading the file back as
`core/src/cldr.rs` does gives each locale's patterns, that each pattern has one
currency sign at an end with only bidi marks and white space beyond it and one
run of digits, and that the counts of a magnitude have the same digits; a
negative subpattern (sw's) is dropped.

`date-patterns.txt` (820 KB) holds the date and time patterns that ICU4X does
not have, from the JSON packages `cldr-dates-full` (`ca-gregorian.json`,
`ca-generic.json`) and `cldr-cal-<name>-full` for `buddhist`, `chinese`,
`coptic`, `dangi`, `ethiopic` (`ca-ethiopic.json`, and
`ca-ethiopic-amete-alem.json` for `ethioaa`), `hebrew`, `indian`, `islamic`
(`ca-islamic-civil.json`, `ca-islamic-tbla.json`, `ca-islamic-umalqura.json`),
`japanese`, `persian` and `roc`, all at the same npm version, and from
`common/main/*.xml` for `iso8601`, which cldr-json does not have: root's
patterns, with the values each locale of the CLDR chain has explicitly over
them, the nearest last. In CLDR 48.2 only `zh` has any, 14 intervals of its own
(`y年MMMd–d日`), which its children inherit; `bg`'s iso8601 calendar holds only an
inheritance marker, which defines nothing. The generator fails if a locale of
the chains that is not in `locales.txt` has iso8601 data, and resolves root's
`atTime` aliases to each locale's own standard patterns. It has a string table,
then for each locale a `locale <tag> <parent>` line, with the same parents as
`units.txt`, and for each calendar (BCP 47 ids, plus CLDR's `generic`, which
only appears as the target of `=`) either `<calendar> = <other>`, where the
locale's whole data for the calendar equals its data for a calendar listed
before it, or `<calendar> <key>=<index>...` with the entries that differ from
the parent's for the calendar (no line where none do). Reading a locale and
calendar: follow its `=` line if it has one, else take the parent's data for the
calendar with the locale's own entries over it; `und` has every calendar in
full. The keys are `d0`-`d3` (`dateFormats` full to short), `t0`-`t3`
(`timeFormats`), `c0`-`c3` (the standard `dateTimeFormats`), `a0`-`a3` (the
`atTime` ones), `fb` (`intervalFormatFallback`), `lp` (the numeric leap month
pattern of `chinese` and `dangi`), `tz` (the `Timezone` append item),
`f.<skeleton>` (`availableFormats`) and `i.<skeleton>.<letter>`
(`intervalFormats`), the last two in CLDR's order. Alternates (`-alt-ascii`,
`-alt-variant`) and count variants (`-count-`) are left out, and so is an entry
whose skeleton has a field ECMA-402 does not have (`Q`, `w`, `W`, `D`, `F`, `g`,
`e`, `u`, `Y`, `x`, `X`, `Z`, `V`). Outside quotes, a pattern may have only the
letters `G y U r M L d E c a b B h H K k m s S z O v`, or generation fails,
except for two misspellings CLDR 48.2 has (`gl`'s full date format, `te`'s
`yyyyMMMM`, `ak`'s and `ko`'s interval patterns): `Y` is written as `y`, and `e`
as `E`, with the same letter count (`ko`'s `e요일` is `E요일`, `월요일`). The
`dateTimeFormats`, `fb` and `tz` must have `{0}` and `{1}`, and `lp` `{0}`. A
pattern's numbering override (`_numbers`: `he` `hebrew` `hebr`, `ja` `japanese`
`y=jpanyear`, and `chinese` and `dangi` `hanidec` in `ja`, `zh-SG`,
`zh-Hans-HK`, `zh-Hans-MO` and `d=hanidays` in `zh`, `zh-Hant` and `yue`) is
left out. The generator checks that every locale and calendar read back from the
written lines gives its CLDR data, which also means a child has every key its
parent has; it reads `day-periods.txt` back the same way.

`day-periods.txt` (51 KB) holds the flexible day periods (`B`): `rules
<key>` for each of CLDR's `dayPeriodRuleSet`s (`cldr-core`
`supplemental/dayPeriods.json`; the keys as BCP 47 tags, `es-CO`, `hi-Latn`),
each followed by `<period> <from> <before>` lines in minutes of the day
(`morning1 0 720`; 24:00 is 1440; `before` is smaller than `from` for a period
that crosses midnight, bg's `night1 1320 240`; an `at` rule is `noon 720 720`),
then for each locale a `locale <tag> <parent> <rules-key>` line, with the same
parents as `units.txt`, and `<period> <abbreviated>\t<wide>\t<narrow>` lines
with the format-context names (`ca-gregorian.json`) of the periods of its
rules that differ from the parent chain's. The rules key is found as ICU finds
it, by truncating the tag until a rule set has it, else `und` (`zh-Hant` and
`zh-HK` use `zh`, though their parent is `und`); names follow the parent
chain. A period CLDR has no names for (`lij`, `scn`) has no line, and its
names read as `""`; the generator fails on a period named in some widths only,
or not named where the parent chain names it.

`display-names.txt` (252 KB) holds the DisplayNames data that ICU4X does not
have, from the JSON package `cldr-localenames-full` (at the same npm version)
and `cldr-dates-full`. For each locale it has a `locale <tag> <parent>` line,
with the same parents as `units.txt`, then `<key>\t<value>` lines for the
entries that differ from the parent chain's; a key is read from the nearest
locale of the chain that has it, and `und` has every key root defines. The
keys are `ca.<calendar>`, the calendar names
(`localeDisplayNames.json` `types.calendar`) of the 16 calendars of
`core/src/supported.rs`, by BCP 47 id (CLDR's `gregorian` is `gregory`,
`ethiopic-amete-alem` `ethioaa`; CLDR's `islamic` and `islamic-rgsa` are left
out, and root has none); `f.<code>.<style>`, the names of the date-time
fields of ECMA-402's 12 codes (`dateFields.json` `displayName` of the field,
`<field>-short` and `<field>-narrow` for the styles `long`, `short` and
`narrow`; `weekOfYear` is CLDR's `week`, `dayPeriod` `dayperiod`,
`timeZoneName` `zone`; `displayName-alt-variant` is left out); `lp` and `ls`,
`localeDisplayPattern`'s `localePattern` (`{0} ({1})`) and `localeSeparator`
(`{0}, {1}`), which must have `{0}` and `{1}`; and `eq.<kind>.<style>.<code>`
with an empty value for each name in `languages.json`, `territories.json`,
`scripts.json` and `variants.json` that equals its code (en's `US-alt-short`
`US`, `Thai`, `fon`), which ICU4X's data leaves out: `<kind>` is `language`,
`dialect` for a language key with a `-`, `region`, `script` or `variant`,
`<style>` is `long`, or `short` for an `-alt-short` entry, and `<code>` is
CLDR's key: variants are uppercase (`PINYIN`), so a reader uppercases a
variant subtag before it looks one up. An `eq` entry applies only where ICU4X has no name for the code,
so a locale may inherit one for a code it has its own name for (`nn`'s `tiv`
"tivi" under `no`'s "tiv", `hi-Latn`'s short `US` "America" under `en-IN`'s
"US"): ICU4X has that name, and the entry is never used. The generator fails
unless `und` has `lp`, `ls` and all 36 field names and `en` names every
calendar, and unless reading every locale back gives its CLDR entries, with
only that exception.

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
