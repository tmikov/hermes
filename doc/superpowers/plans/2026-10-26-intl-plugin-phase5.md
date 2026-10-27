# Intl Plugin Phase 5 (ListFormat, RelativeTimeFormat, DisplayNames, DurationFormat, Segmenter) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the five remaining constructors to the plugin, each with its whole public surface:
- `Intl.ListFormat`: the constructor, `supportedLocalesOf`, `resolvedOptions`, `format` and `formatToParts`, over any iterable of strings;
- `Intl.RelativeTimeFormat`: the constructor, `supportedLocalesOf`, `resolvedOptions`, `format` and `formatToParts`, in every numbering system NumberFormat supports;
- `Intl.DisplayNames`: the constructor, `supportedLocalesOf`, `resolvedOptions` and `of`, for the six types (`language`, `region`, `script`, `currency`, `calendar`, `dateTimeField`);
- `Intl.DurationFormat`: the constructor, `supportedLocalesOf`, `resolvedOptions`, `format` and `formatToParts`, built on the plugin's internal NumberFormat and ListFormat;
- `Intl.Segmenter`: the constructor, `supportedLocalesOf`, `resolvedOptions`, `segment`, the Segments objects with `containing` and `@@iterator`, and the Segment Iterators.

At the end, every test262 test the expectations file assigns to Phase 5 (393 lines) passes, and no new line is added.

**Architecture:**
- As in Phases 3 and 4, the operations are JS, written step by step from the draft, and native code supplies data and algorithms: ICU4X's list formatter (it chooses the literal text around the elements), ICU4X's relative time and display name data, three small CLDR tables ICU4X lacks, CLDR's time separator, and ICU4X's segmenters (they compute the boundaries).
- ListFormat and Segmenter attach native state (`ListFormatState`, `SegmenterState`, `SegmentsState`). RelativeTimeFormat, DisplayNames and DurationFormat attach none.
- DurationFormat and RelativeTimeFormat call the plugin's internal NumberFormat, PluralRules and ListFormat functions directly, never the public constructors.
- New JS files: `intl/js/06-iterators.js` (the iterator protocol), `60-ListFormat.js`, `62-RelativeTimeFormat.js`, `64-DisplayNames.js`, `66-DurationFormat.js`, `68-Segmenter.js`. New Rust modules: `list.rs`, `relativetime.rs`, `displaynames.rs`, `segmenter.rs`. New C++ files: `ListFormatHelpers.cpp`, `RelativeTimeFormatHelpers.cpp`, `DisplayNamesHelpers.cpp`, `SegmenterHelpers.cpp`.

**Tech Stack:** Rust 1.88 with ICU4X 2.3 (`icu` with `compiled_data` and `sync`: `icu::list`, `icu::segmenter`, `icu::properties`) and `icu_experimental` 0.6 (`relativetime` and `displaynames` data, `dimension::provider::currency::displayname`); C++17 and JSI; Python 3 for the data generator; the plugin's JS compiled to Hermes bytecode; test262 at `c9aa6126e8f6fe2c6f668c65bfba771ffc8a6096`.

**Spec:** `doc/superpowers/specs/2026-10-22-intl-plugin-design.md`. Read "Guiding rules", "The JS layer" (including "Resistance to tampering"), "The native interface" (including "Strings" and "Parts"), "Data", "Testing" and "Phasing" before any task. Task 9 writes this plan's decisions into it.

**ECMA-402 text:** the https://tc39.es/ecma402/ draft dated October 24, 2026, which is tc39/ecma402 commit `3691634ebc`. The files are `spec/listformat.html`, `spec/relativetimeformat.html`, `spec/displaynames.html`, `spec/durationformat.html`, `spec/segmenter.html`, and `spec/negotiation.html` (ResolveOptions, already implemented). Cite section titles and number the steps. Fetch with `gh api -H 'Accept: application/vnd.github.raw' 'repos/tc39/ecma402/contents/spec/<file>?ref=3691634ebc'`. GetIterator, IteratorStepValue, IteratorClose, CreateIteratorResultObject and ToIntegerOrInfinity are in ECMA-262; fetch `spec.html` from `tc39/ecma262` `main` the same way.

**Research:** the findings behind this plan, with evidence and probe programs, are in `/tmp/claude-1001/-home-tmikov-work-hermes/a918a33f-abe5-47f3-80d3-351885f648de/scratchpad/p5-research/` (`listformat.md`, `relativetimeformat.md`, `displaynames.md`, `durationformat.md`, `segmenter.md`). An implementer may read the report for its constructor; the plan's decisions win where they differ.

**Reference output:** `node` (Node 24, ICU 78.2, CLDR 48.0) is on PATH. The plugin uses CLDR 48.2, so a string may differ for that reason alone; every other difference from Node is either one of the decisions below or a bug. Where the plugin's output differs from Node, the implementer checks CLDR 48.2 and ICU4X, and reports the difference with its cause.

## Global Constraints

- **Worktree:** work only in `/home/tmikov/work/hermes-intl`, on branch `hermes-intl`. **Never `cd`**; pass absolute paths. Use a subshell `( cd x; cmd )` only when a tool insists.
- **Build directory:** `/home/tmikov/work/hermes-intl/cmake-build-asan`. It is already configured: Clang, ASan, Debug with `-O1`, `JSI_UNSTABLE=ON`, and `HERMES_INTL_TEST262_DIR=/home/tmikov/work/hermes-intl/cmake-build-asan/test262` (the pinned checkout). Do not reconfigure it. New `intl/js/NN-*.js` files are picked up by the glob in `intl/js/CMakeLists.txt` on the next build.
- **Commands:**
  - Rust tests: `cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-intl-rust`
  - Generator tests: `cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-intl-data`
  - Plugin lit tests and gtests: `cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-intl-lit`. Narrow a run with `LIT_FILTER=<regex>`.
  - test262 in plugin mode: `cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-intl-test262`
  - Everything: `check-intl`, then `check-hermes`.
  - Never run test262 in normal mode. The plugin-mode runs above are the only test262 runs allowed.
- **Updating the expectations:** use the command in `intl/README.md` ("Updating the expectations"), with the paths `check-intl-test262` passes in `intl/CMakeLists.txt`. After each update:
  - read the diff of `intl/test/test262-expectations.txt`;
  - every removed line must be a test this task made pass;
  - no line may be added and no reason changed. If a task finds a test it cannot make pass, or a test that newly fails (watch the `testWithIntlConstructors` tests, which start covering each new constructor), it stops and reports it with the failure output; the controller rules on it.
- **Git:** at the end of each task, commit that task's files on `hermes-intl` with a plain-English message that says what changed and why. End the message with:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01JPBWtUGtZ5QcTC9dE7h3R8
  ```
  Never push, amend or reset.
- **Copyright header:** every new source file gets the Meta MIT header used across the repo. Generated data files get the header comment `update-cldr-lists.py` writes.
- **Separability:** code under `intl/` includes only `<jsi/...>`, `<hermes/Public/IntlPlugin.h>`, its own headers and the generated `hermes_intl_core.h`. The one exception is `intl/test/unit/`.
- **Plugin JS rules:** `intl/README.md`, "Writing plugin JS", rules 1 to 6. Reviewers check every line against them. In particular:
  - no `String.prototype.split`, no regular expressions, no spread, no `for-of`, and no `Array.prototype` methods;
  - every array is built with `listPush`;
  - every record and descriptor has `__proto__: null`;
  - an object the draft creates with CreateDataPropertyOrThrow (part objects, `resolvedOptions` results, segment data objects, iterator results) is made with an object literal or with the captured `Object.defineProperty`, never by assignment, so a setter on `Object.prototype` never runs;
  - a required parameter is a plain parameter and an optional one a default parameter (`format(list)`, `supportedLocalesOf(locales, options = undefined)`), which keeps `length` at the spec value. Never read `arguments[i]` past `arguments.length`;
  - the file headers name the draft of October 24, 2026.
- **Native helpers:** `intl/README.md`, "Adding a native helper". JS validates every argument before calling a helper. A helper that gets an argument JS should never send throws `std::logic_error("Intl internal error: ...")`.
- **Rust:**
  - FFI functions never panic on bad input. Invalid UTF-8, an unknown type, style, kind or granularity, a tag that does not parse, or an index out of range returns `false` or a null pointer.
  - A panic is only for a broken invariant in our own data files.
  - Every new public function has a doc comment and a unit test.
  - Run `cargo fmt` in `intl/core`, and `clang-format` on changed C++.
- **Threads** (spec, "Threads"): a Rust type held in a `NativeState` must be `Send`, checked at compile time. A `NativeState` destructor frees only its own Rust object. Parsed data files are process-wide immutable tables built once behind `std::sync::OnceLock`.
- **Prove checks can fail:** for every new test, make a stated mutation of the code under test, name the test that fails, then revert the mutation. Put this evidence in the task report.
- **Never bump `BYTECODE_VERSION`**, and never bump the plugin-interface version. Phase 5 adds native helpers and changes no entry-point, config or `IHermesBuiltinBytecode` interface.

## Planning decisions

Task 9 writes these into the spec. Implement them as stated.

1. **Operations are JS; native code supplies data and algorithms.** Every abstract operation of the five draft files is JS, step by step. Two statements of the spec change:
   - "Strings": ListFormat sends native code only the last element of a list, not every element, and native code returns only literal text (decision 2);
   - "Parts": no constructor of this phase has a native `...ToParts` helper. JS builds the parts of RelativeTimeFormat and DurationFormat, with their `unit`, as it builds NumberFormat's. The `extra` field's RelativeTimeFormat and DurationFormat bullet goes.
2. **ListFormat.** ICU4X 2.3's `ListFormatter` (`try_new_and`, `try_new_or`, `try_new_unit`, with `ListLength::Wide`, `Short`, `Narrow` for `long`, `short`, `narrow`) has every locale of `locales.txt`, every type and every style, and the conditional patterns UTS 35 describes (Spanish `y`/`e` and `o`/`u`, Hebrew `ו`/`ו‑`). Those are compiled automata in its data, not CLDR text, and they test only the last element, through the `end` and `pair` patterns. ICU4X never reorders elements. So:
   - the constructor ends with `native.listFormatInit(lf, languageId, type, style)`, which attaches a `ListFormatState` that owns the `ListFormatter`;
   - CreatePartsFromList returns « » for no element and « element » for one, without native code. For two or more it makes one call, `native.listFormatLiterals(lf, isPair, last)`: native code formats a list of empty elements (two for a pair, four otherwise) whose last element is `last`, with a `PartsWrite` sink that keeps the text of every top-level `LITERAL` part, and returns 3 pieces for a pair (before, between, after) and 5 otherwise (before, after-first, middle, before-last, after). JS interleaves them with the original element strings, repeating `middle` between inner elements. Pieces that are empty give no part, as the draft's PartitionPattern gives none;
   - `last` crosses as UTF-16 and native code reads unpaired surrogates as U+FFFD, which only chooses a pattern; no element text ever comes back, so the result is lossless;
   - the draft's [[Templates]] and its "implementation-defined index n" are what native code chooses; the slots JS keeps are [[Locale]], [[Type]] and [[Style]]. A comment says so.
   - Known differences from ICU4C: Hebrew before a non-Hebrew element is `" ו‑"` with U+2011 (ICU4X's data; ICU4C and UTS 35's example have U+002D), and Spanish `"11.000"` and `"11,5"` take `u` after `o`-joined lists (UTS 35's rule; ICU4C gives `o`). The plugin keeps ICU4X's data in both cases.
3. **The iterator protocol** (`06-iterators.js`), from ECMA-262: `GetIterator(obj)` (sync kind, through the captured `Symbol.iterator`; the method must be callable and its result an Object; `next` is read once), `IteratorStepValue(record)` (calls `next`, requires an Object, reads `done`, then `value`; no close on any of these errors), and `IteratorCloseOnThrow(record, error)` (reads `return` and calls it if it is not undefined or null, ignores any error from either step and any result, then throws `error`). StringListFromIterable uses them: `undefined` gives an empty list; a value whose type is not String (a String object too) closes the iterator with a new `TypeError`. `01-intrinsics.js` captures `Symbol.iterator` and %Iterator.prototype% (the prototype of the prototype of `[][Symbol.iterator]()`).
4. **RelativeTimeFormat.**
   - Data: `icu_experimental` 0.6's 24 markers `{Long,Short,Narrow}{Second,Minute,Hour,Day,Week,Month,Quarter,Year}RelativeV1` (`RelativeTimePatternData`: `relatives`, `past`, `future`), read with `number.rs`'s existing `load`, `by_category` and `render_compact` (made `pub(crate)`). They load for every locale of `locales.txt` and equal CLDR 48.2's `dateFields.json` in every value, so no data file is generated. The helper `relativeTimePatterns(languageId, style)` returns them (Task 3's table); JS caches the parsed record by `languageId;style` in a `newDataCache()`.
   - [[NumberFormat]] is `CreateNumberFormat(locale, {__proto__: null, numberingSystem: nu})`. [[PluralRules]] is a PluralRules made by a new internal `CreatePluralRules(locales, options)`: `45-PluralRules.js` splits its constructor into `initializePluralRules(pluralRules, locales, options)` and that function, as `40-NumberFormat.js` does. The `nu` list of [[LocaleData]] is NumberFormat's (`numberFormatLocaleData`).
   - **The absolute value.** The draft's PartitionRelativeTimePattern formats ℝ(value) with its sign (`format(-1, "day")` would be `"-1 day ago"`); test262 and every engine format the magnitude, and the past pattern carries the sense. The plugin formats `IntlMVFromNumber(abs(value))` with abs(−0) = +0 (never `'negZero'`, which shows `"-0"`), and the plural category is `ResolvePlural(pr, IntlMVFromNumber(value)).pluralCategory` (the plugin's ResolvePlural takes an Intl mathematical value and returns a record; the sign does not change the category). The number's parts are `PartitionNumberPattern(nf, x).parts` (the plugin's returns a record). A comment names the deviation.
   - A pattern without `{0}` (ar `"قبل ثانية واحدة"`, he `"בעוד יומיים ({0})"` has one) gives one literal part, as MakePartsList says. Every part of the number has the `unit`, literals inside the number pattern included; the pattern's own literals do not.
   - gd's `"an-eararais"` for `format(3, "day")` with `numeric: "auto"` is CLDR's name for +3 days, which the draft uses (V8 prints a number).
5. **DisplayNames.**
   - **Names.** From `icu_experimental` 0.6's data, read as data structs: `LanguageDisplayNamesV1` (a language subtag), `LocaleDisplayNamesV1` (a `language-script` or `language-region` key, the dialect names), `RegionDisplayNamesV1`, `ScriptDisplayNamesV1`, `VariantDisplayNamesV1`, and `CurrencyDisplaynameV1` (the singular display name, with the ISO code as marker attribute; Phase 3's currency data has only the plural names). Style `long` reads `names`; `short` and `narrow` read `short_names`, then `names` (CLDR's `alt="short"`; V8 does the same). `long_names` and `menu_names` are never used. ICU4X's `LocaleDisplayNamesFormatter` is not used (its separators are fixed English text, and it cannot report a missing part).
   - **ICU4X's data leaves out every name that equals its code** (en region short `"US"` and `"UN"`, script `"Thai"`, `"Cham"`; 20 distinct pairs, about 2,000 locale entries). The generated file lists them (decision 6), and a lookup that ICU4X misses returns the code when the file lists it for the locale's chain, kind and style. Order for short and narrow: `short_names`, the listed short entry, `names`, the listed long entry; for long: `names`, the listed long entry.
   - **Calendar names, date-time field names and the locale display pattern** come from the generated `display-names.txt` (decision 6). Only the 16 calendars of `supported.rs` and the two aliases `ethiopic-amete-alem` and `islamicc` have names, an alias with its canonical calendar's; `islamic` and `islamic-rgsa`, which CLDR names, have none, because test262's `calendars-accepted-by-DisplayNames.js` requires a name for exactly the supported calendars. Calendar names have one style. Field names have three (CLDR's `displayName` of `<field>`, `<field>-short`, `<field>-narrow`), with the draft's codes mapped to CLDR's fields: `weekOfYear` is `week`, `dayPeriod` is `dayperiod`, `timeZoneName` is `zone`, the rest are the same word.
   - **Language names are composed in JS** by UTS 35's algorithm (Part 2, "Locale Display Name Algorithm"), which matched Node in all 110,160 cases of the research:
     1. the code is CanonicalCodeForDisplayNames's result, split into language, script, region and variants;
     2. with `languageDisplay: "dialect"` and exactly one of script and region, look up the dialect name of `language-script` or `language-region`; on a hit, that subtag is used up. With both a script and a region, or `"standard"`, there is no dialect lookup;
     3. otherwise the base is the language name of the language subtag;
     4. the qualifiers, in order: the script name, the region name, each variant's name, in the instance's style (short and narrow use the short names: `"English (UK)"`). In each, `(` and `)` become `[` and `]`, and `（` and `）` become `［` and `］`;
     5. the qualifiers are joined with the locale's `localeSeparator` (`"{0}, {1}"`), and the base and qualifiers with its `localePattern` (`"{0} ({1})"`);
     6. if the base or any qualifier has no name, the code has no name: `of` returns the canonical code with `fallback: "code"` and `undefined` with `"none"`, as the draft says (V8 gives a partial name such as `"English (ZY)"`).
   - The draft canonicalizes region, script and calendar codes before the lookup, so `of("us")` with `type: "region"` is `"United States"` (V8 gives `undefined`). `"und"` has CLDR's name, `"Unknown language"` (V8 gives `undefined`).
   - No native state; `of` makes one native lookup per name. [[Fields]] is what the lookups read.
6. **Data generation.** `update-cldr-lists.py` writes one new file and one new column:
   - `intl/core/data/display-names.txt`, in `day-periods.txt`'s layout: `locale <tag> <parent>` lines, the parent as `units.txt` computes it, each followed by `<key>\t<value>` lines for what differs from the parent chain. Keys: `ca.<calendar>` (16 BCP 47 ids; CLDR's `gregorian` is `gregory` and `ethiopic-amete-alem` is `ethioaa`) from `cldr-localenames-full` `localeDisplayNames.types.calendar`; `f.<code>.long`, `f.<code>.short`, `f.<code>.narrow` for the 12 draft codes, from `cldr-dates-full` `dateFields.json` `displayName` (not `displayName-alt-variant`); `lp` and `ls`, `localeDisplayPattern`'s `localePattern` and `localeSeparator` (with `{0}` and `{1}` in CLDR form), from `localeDisplayNames.json`; and `eq.<kind>.<style>.<code>` with an empty value, for each name in `languages.json`, `territories.json`, `scripts.json` and `variants.json` that equals its code (`kind` is `language`, `dialect` for a language key with `-`, `region`, `script` or `variant`; `style` is `long`, or `short` for an `-alt-short` entry). `und` has a complete entry for every key it defines. The generator checks that `und` has `lp`, `ls`, and all 36 field names, and that no supported calendar lacks an `und` or `en` name;
   - `number-symbols.txt` gains a last column, `timeSeparator` (from `symbols-numberSystem-<nu>.timeSeparator` of `cldr-numbers-full`), and `EXPLICIT_SYMBOLS` gains `timeSeparator`, so `explicit-number-systems.txt` carries root's explicit value where it has one. `cldr.rs` reads it, and `numberSymbols` gains entry 21, resolved by the existing chain. In CLDR 48.2 it is `":"` everywhere except latn `"."` in da, da-GL, en-DK, en-FI, fi, id, ms-ID, si, su, sv-FI, and arabext `"٫"` in ug, ur, ur-IN.
7. **DurationFormat.** The draft step by step, with these decisions:
   - [[DigitalFormat]] is per numbering system, and the draft's constructor never picks the entry of the resolved one; the plugin reads the record of [[NumberingSystem]] (the evident intent): [[HourMinuteSeparator]] and [[MinuteSecondSeparator]] are both entry 21 of `numberSymbols` for the language identifier of [[Locale]] and [[NumberingSystem]], which is what V8 shows (`da` `"1.02.03"`);
   - [[TwoDigitHours]] is false for every locale. The draft's GetDurationUnitOptions step 15 would turn hours into `"2-digit"` for any style, so a true value breaks non-digital styles (`af` `{days: 1, hours: 1}` would give `"1 dag, 01:00:00"`, and `{minutes: "long"}` would throw). V8 never pads hours either;
   - ToIntegerIfIntegral returns +0 for −0 (it returns ℝ(number)), and every field is formatted with its exact value: an integral field is `makeIntlMV(neg, BigInt(abs(field)), 0)`, never `IntlMVFromNumber`, which reads the shortest decimal string (`4503599627370497000` is exactly `4503599627370497024`); IsValidDuration and ComputeFractionalDigits are exact: the sum is a BigInt count of nanoseconds compared with 2^53 × 10^9, built from `BigInt` of each integral field and literal BigInt powers of ten (no BigInt `**`), and a fractional value is an Intl mathematical value from `makeIntlMV`;
   - numbers go through `CreateNumberFormat` and `PartitionNumberPattern(nf, x).parts`, with null-prototype option records; the list through the internal ListFormat of decision 8. A DurationFormat keeps each internal NumberFormat it creates, keyed by the options that made it, and its one ListFormat, both made on first use; none of this is observable.
8. **Internal cooperation** (spec, "Resistance to tampering"): `CreateListFormat(locales, options)` and `CreatePartsFromList(listFormat, list)` (ListFormat), `CreatePluralRules` (decision 4) and `CreateNumberFormat` are internal functions that the public constructors and methods also use. DurationFormat and RelativeTimeFormat never read `Intl.ListFormat`, `Intl.NumberFormat`, `Intl.PluralRules` or their prototypes.
9. **Segmenter.**
   - **Segmenters.** `GraphemeClusterSegmenter::new()`; `SentenceSegmenter::try_new(SentenceBreakOptions)` and `WordSegmenter::try_new_dictionary(WordBreakOptions)`, each with the language identifier of [[Locale]] as content locale (ICU4X 2.3 tailors only `el` sentences; passing the locale follows any later data). The word segmenter is the **dictionary** one: CLDR's dictionary for Chinese and Japanese and ICU4X's dictionaries for Thai, Lao, Khmer and Burmese, as ICU4C uses. The spec's "default word segmenter" (`try_new_auto`, LSTM models for the Southeast Asian scripts) is 1.46 MB smaller but about 120 times slower on Thai (2.34 s against 19 ms for 414,000 code units) and differs from ICU4C for Lao and Burmese; correctness and speed come before size (Guiding rule 2).
   - **Boundaries.** ICU4X's iterators only go forward and borrow the input, and the rules need unbounded left context, so native code computes every boundary of a Segments object's string once, on its first `containing` or `next` that reaches native code (after the draft's early returns), and keeps them in a `SegmentsState` on the Segments object: the ascending boundaries from 0 to the length, and for word granularity whether each segment is word-like. FindBoundary is a binary search. An iterator reads its Segments object's state, which an internal slot keeps alive.
   - **Two ICU4X 2.3 word bugs are worked around in native code**, both found against Node: the UTF-16 word path truncates characters above U+FFFF in Chinese and Japanese runs (`word.rs:988`, `:991`), and a word that ends in a code point with Word_Break Extend, Format or ZWJ, followed by more text, is reported `WordType::None` (decomposed `"café x"`, Hindi `"की एस"`, Arabic with harakat). Word segmentation therefore runs on UTF-8: the UTF-16 input is transcoded with each unpaired surrogate replaced by U+FFFD (both have Word_Break Other, so no boundary moves), with a map from byte offsets back to UTF-16 indices; and a segment marked `None` whose last code point has Word_Break Extend, Format or ZWJ is segmented again on its own and takes the word type at its end. Grapheme and sentence segmentation use `segment_utf16` directly. The workaround is removed when an ICU4X upgrade fixes both (2.4 rewrote the UTF-16 path).
   - **Objects.** %IntlSegmentsPrototype% has `containing` (length 1) and `[Symbol.iterator]` (length 0, `name` `"[Symbol.iterator]"`) and no `@@toStringTag`. %IntlSegmentIteratorPrototype% inherits from %Iterator.prototype% and has `next` (length 0) and `@@toStringTag` `"Segmenter String Iterator"`. The Segments objects and iterators are made with `ObjectCreate` of these prototypes, and their slots are private fields (`SegmentsSlots`, `SegmentIteratorSlots`). CreateSegmentDataObject is one of two object literals (with or without `isWordLike`), so the property order is `segment`, `index`, `input`, `isWordLike`.
   - Known differences from ICU4C (implementation-defined): some Thai and dictionary splits, Korean next to Latin (`"MB가"` is one word; ICU4C splits it), `"_"` alone is word-like.
10. **Native state.** `ListFormatState` (`ListFormatHelpers.cpp`, owns a `HiListFormat`), `SegmenterState` (owns a `HiSegmenter`) and `SegmentsState` (owns a `HiSegments`, `SegmenterHelpers.cpp`). Each Rust type is `Send`. A helper given an object without its state, or an object that already has state, throws an internal error.
11. **Constructors.** All five require `new` and have no legacy behavior. Each is a plain function, with `length` 0 (DisplayNames: 2, from `Intl.DisplayNames ( locales, options )`), that uses the `this` Hermes created from `new.target` (the spec's documented deviation for a non-object `new.target.prototype` applies). Their methods are plain methods: no bound getters. The options objects go through GetOptionsObject (ListFormat, DurationFormat, Segmenter), CoerceOptionsToObject (RelativeTimeFormat) or `require-options` (DisplayNames), as the draft's ResolveOptions call says. Only RelativeTimeFormat and DurationFormat have a relevant extension key (`nu`).
12. **Size.** The phase adds about 8.5 MB of ICU4X data and code (display names about 5.2 MB, the dictionary segmenter about 3.9 MB, relative time data about 0.7 MB, list data about 0.1 MB, before the linker drops duplicates). The spec's non-goal "Binary size, for now" applies; Task 9 measures and records the result.

## Review Focus

These are the five inputs most likely to hurt someone that the spec implies but test262 does not exercise. Each one has a test in the task named.

1. **Text that is not plain BMP text.** Unpaired surrogates (leading, trailing, reversed pairs) and supplementary characters (emoji ZWJ sequences, flags, Ext-B Han `"𠀀𠀁𠀂"`, `"日本語𠀀テスト"`) in ListFormat elements and Segmenter input: ListFormat output is the elements unchanged; Segmenter indices are UTF-16 code-unit indices, the segments tile the input, and word segments match Node (`日本語|𠀀|テスト`); decomposed `"café x"` and `"की एस"` give `isWordLike: true` for their first word. Covered in Task 2 (`list-format.js`), Task 7 (Rust) and Task 8 (`segmenter-text.js`).
2. **Tampering after the plugin loads:**
   - setters on `Object.prototype` for `type`, `value`, `unit`, `segment`, `index`, `input`, `isWordLike`, `done`, `locale` and every option name;
   - a setter for `"0"` on `Array.prototype`;
   - replaced `Intl.ListFormat`, `Intl.NumberFormat`, `Intl.PluralRules` and their prototype methods, `Symbol.iterator` getters on the internal lists' prototypes, `Array.prototype[Symbol.iterator]` (it changes what an array passed to `ListFormat.format` yields, which the draft requires, but not what DurationFormat's internal list gives), `Math.abs`, `BigInt`, `Number.prototype.toLocaleString`.

   Every method gives the same results regardless. Covered in Tasks 2, 3, 5, 6 and 8 (`tamper.js`).
3. **Objects that look like instances:** `Object.create(<C>.prototype)`, a Proxy around an instance, an instance of another Intl constructor, a Segments object passed to the iterator's `next`, an iterator passed to `containing`, `Object.create(%IntlSegmentsPrototype%)`. Each method throws `TypeError`, while an instance of a subclass (`class X extends Intl.ListFormat {}`) works with its own and borrowed methods, as test262's `subclassing.js` checks; no `Exception in HostFunction:` ever appears. Covered in each constructor's `-objects.js` test.
4. **Extreme values.** RelativeTimeFormat with `1e21`, `-0`, `0.0001`, `Number.MAX_VALUE`, `-Number.MIN_VALUE`; DurationFormat at the limits (`years: 2**32 - 1`, `seconds: 2**53 - 1`, `milliseconds: 4503599627370497000`, `nanoseconds: Number.MAX_SAFE_INTEGER`, the smallest durations that fail `IsValidDuration`, `fractionalDigits` 0 and 9); DisplayNames with a 100-variant tag and a 3-letter region code; Segmenter on a one-million-code-unit string, `containing` at `length - 1`, `NaN`, `-0.5`, `Infinity`, and on `""`. Covered in Tasks 3, 5, 6 and 8.
5. **Data for every locale.** For each locale of `locales.txt`: ListFormat initializes for every type and style and gives literals for a pair and for four elements; `relativeTimePatterns` succeeds for every style with 8 units of 12 patterns; every DisplayNames lookup succeeds, and every locale other than `und` has a name for `en`, `US`, `Latn`, `gregory` and `year` (and `USD`, except in a pinned list of locales); `numberSymbols` entry 21 is a non-empty string for the locale's numbering systems; the three segmenters initialize. Covered in Tasks 1, 2, 3, 4 and 7 (Rust tests over every locale), and in Task 6 (`duration-format-locales.js`: 40 locales format and format to parts).

---

### Task 1: The new CLDR data

**Files:**
- Modify: `intl/core/data/update-cldr-lists.py`, `intl/core/data/test_update_cldr_lists.py`, `intl/core/data/number-symbols.txt` and `intl/core/data/explicit-number-systems.txt` (both regenerated), `intl/core/src/cldr.rs`, `intl/core/src/number.rs`, `intl/jsi/NativeHelpers.def` (the `numberSymbols` comment), `intl/README.md` ("Updating the CLDR lists": the new file, the new column, the new npm package)
- Create: `intl/core/data/display-names.txt` (generated)
- Test: generator tests; unit tests in `cldr.rs` or `number.rs`

**Interfaces:**
- Produces `display-names.txt` (decision 6) and entry 21 of `numberSymbols` (decision 6). The `cldr-localenames-full` package (48.2.0) joins the generator's `PACKAGES`.
- Reading `(L, key)` in `display-names.txt`: L's own line if it has one, else its parent's, up to `und`.

- [ ] **Step 1: Write the failing generator tests** in `test_update_cldr_lists.py`, on small fixture JSON written by the test (`und` and two locales with a parent):
  - `test_display_names_calendars`: `gregorian` comes out as `ca.gregory`, `ethiopic-amete-alem` as `ca.ethioaa`; `islamic`, `islamic-rgsa` and the `iso8601-alt-variant` key are not written;
  - `test_display_names_fields`: `year`, `year-short`, `year-narrow` give `f.year.long`, `.short`, `.narrow`; `week`, `dayperiod` and `zone` give `f.weekOfYear.*`, `f.dayPeriod.*`, `f.timeZoneName.*`; `displayName-alt-variant` is ignored;
  - `test_display_names_patterns`: `localeDisplayPattern` gives `lp` and `ls`;
  - `test_display_names_code_equal`: a fixture with territory `US-alt-short` = `"US"`, script `Thai` = `"Thai"` and language `fon` = `"fon"` writes `eq.region.short.US`, `eq.script.long.Thai` and `eq.language.long.fon`; a name that differs from its code writes nothing;
  - `test_display_names_dedup`: a child entry equal to the parent's is not written, and one that differs is;
  - `test_display_names_checks`: a fixture whose `und` lacks `f.era.narrow`, or whose `en` lacks a supported calendar, fails generation naming it;
  - `test_time_separator_column`: a locale's `timeSeparator` is the last column of its `number-symbols.txt` row, and an explicit root `timeSeparator` for a numbering system reaches `explicit-number-systems.txt`.
- [ ] **Step 2: Run them to see them fail.** `check-intl-data` fails: the functions do not exist.
- [ ] **Step 3: Implement the generator.** Reuse `resolve_locale_dirs` and `file_parents`; the file's locales are those of `locales.txt` and `und`.
- [ ] **Step 4: Run the generator** with `python3 intl/core/data/update-cldr-lists.py release-48-2`. Check:
  - `git diff --stat` shows only `display-names.txt` (new), `number-symbols.txt` and `explicit-number-systems.txt` changed; in `number-symbols.txt` only the new column differs (compare the rows with the column cut off);
  - record the new file's size and line count in the report (the research estimate is about 180 KB);
  - `en` reads `ca.gregory` `"Gregorian Calendar"`, `f.year.short` `"yr."`, `f.weekday.long` `"day of the week"`; `de` `f.era.narrow` `"E"`; `und` `lp` `"{0} ({1})"` and `ls` `"{0}, {1}"`; `en` has `eq.region.short.US` and `eq.script.long.Thai`;
  - `da` latn's `timeSeparator` is `"."`, `en` latn's `":"`, `ug` arabext's `"٫"`.
- [ ] **Step 5: Read the column in Rust.** `cldr.rs` parses the new column; `numberSymbols` returns it as entry 21 through the existing resolution chain; update the helper's comment. Rust test `time_separator`: `da`/`latn` is `"."`, `en`/`latn` `":"`, `fi`/`latn` `"."`, `ug`/`arabext` `"٫"`, `ar`/`arab` a non-empty string, and for every locale of `locales.txt` and each of its numbering systems entry 21 is non-empty (Review Focus 5).
- [ ] **Step 6: Run `check-intl-data`, `check-intl-rust` and `check-intl-lit`.** Expected: pass; the lit tests are unchanged.
- [ ] **Step 7: Commit** the generator, its tests, the data files, the Rust change and the README change.

### Task 2: ListFormat and the iterator protocol

**Files:**
- Create: `intl/core/src/list.rs`, `intl/jsi/ListFormatHelpers.cpp`, `intl/js/06-iterators.js`, `intl/js/60-ListFormat.js`
- Modify: `intl/core/src/lib.rs`, `intl/jsi/NativeHelpers.def`, `intl/jsi/RustCore.h`, `intl/CMakeLists.txt`, `intl/test/unit/CMakeLists.txt`, `intl/js/01-intrinsics.js` (capture `Symbol.iterator` and %Iterator.prototype%), `intl/js/10-Intl.js` (`installListFormat(IntlObject)`), `intl/test/test262-expectations.txt`
- Test:
  - unit tests in `list.rs`; gtests in `intl/test/unit/NativeHelpersTest.cpp`
  - Create: `intl/test/lit/list-format.js`, `list-format-objects.js`, `list-format-gc.js`, `intl/test/lit/js/iterators.js` (an isolation test built like `number-core.js`, concatenating `01-intrinsics.js`, `02-shapes.js` and `06-iterators.js`)
  - Modify: `shapes-table.js`, `tamper.js`

**Interfaces:**
- Produces, by decisions 2, 3, 8 and 10:
  - `list::ListFormat` (`HiListFormat` in C, opaque, `Send` asserted): `ListFormat::new(locale: &str, kind: &str, style: &str) -> Option<Self>` with `kind` `"conjunction"`, `"disjunction"` or `"unit"` and `style` `"long"`, `"short"` or `"narrow"`; `ListFormat::literals(&self, pair: bool, last: &[u16]) -> Vec<String>` (3 or 5 pieces);
  - helper lines:
    - `HERMES_INTL_HELPER(listFormatInit, Undefined, Object, Utf8, Utf8, Utf8)`: `(obj, locale, type, style)`;
    - `HERMES_INTL_HELPER(listFormatLiterals, Utf8List, Object, Bool, Utf16)`: `(obj, isPair, last)`;
  - JS: `GetIterator`, `IteratorStepValue`, `IteratorCloseOnThrow` (decision 3); `StringListFromIterable(iterable)`; `ListFormatSlots` (`ReturnOverride` subclass, static `is` and `get`); `initializeListFormat(lf, locales, options)`; `CreateListFormat(locales, options)` (a new object from %Intl.ListFormat.prototype%, initialized); `CreatePartsFromList(lf, list) -> List of {__proto__: null, type, value}`; `FormatList`, `FormatListToParts`.

- [ ] **Step 1: Write the failing tests.**
  - Rust, in `list.rs`:
    - `english`: `en` conjunction long pair gives `["", " and ", ""]`, four gives `["", ", ", ", ", ", and ", ""]`; disjunction `", or "`; short `", & "`; unit narrow pair `" "`;
    - `conditional`: `es` conjunction pair with last `"Ibiza"` gives between `" e "`, with `"hielo"` `" y "`; disjunction with `"ocho"` `" u "`; `he` conjunction with last `"Ibiza"` gives `" ו‑"` (U+2011), with `"בננה"` `" ו"`; a last element `[0xD800]` reads as U+FFFD and gives `he`'s `" ו‑"`;
    - `suffixes`: `ti` conjunction long pair is `["", "ን ", "ን"]`; `mn` disjunction four ends with `" зэргийн аль нэг"`;
    - `every_locale` (Review Focus 5): for each locale of `locales.txt`, each kind and style, `new` succeeds and `literals` gives 3 and 5 pieces;
    - `rejects`: kind `"and"`, style `"wide"` and locale `"en_US"` give `None`.
  - gtests: `ListFormatStateFreedWithRuntime` (init, one `listFormatLiterals` call, destroy the runtime; LeakSanitizer reports nothing); `ListFormatHelpersRejectMisuse` (`listFormatLiterals` on an object without state, `listFormatInit` twice, an unknown type: each throws an Error containing `Intl internal error`).
  - `iterators.js` (isolation): an iterable whose `@@iterator` is not callable, or returns a primitive, throws `TypeError`; `next` is read once even if the iterator replaces it; a throwing `next`, `done` getter or `value` getter propagates without calling `return`; `IteratorCloseOnThrow` calls `return` once, ignores a throwing `return` and a non-object result, and throws the given error.
  - `list-format.js`, with `%hermes -Xintl-default-locale=en-US %s`, each value checked with `JSON.stringify`:
    - `format(["a", "b", "c"])` is `"a, b, and c"` and its parts are element, literal `", "`, element, literal `", and "`, element; `en-GB` `"a, b and c"`; `en` disjunction `"a, b, or c"`, short `"a, b, & c"`, narrow `"a, b, c"`, unit narrow `["5 lb", "12 oz"]` `"5 lb 12 oz"`;
    - `es` `["fresa", "Ibiza"]` `"fresa e Ibiza"`, `["agua", "hielo"]` `"agua y hielo"`, disjunction `["siete", "ocho"]` `"siete u ocho"`; `he` `["תפוח", "Ibiza"]` `"תפוח ו‑Ibiza"` (U+2011, decision 2), `["תפוח", "בננה"]` `"תפוח ובננה"`; `ti` pair parts element, literal `"ን "`, element, literal `"ን"` (no empty literal anywhere); `mn` disjunction `"a, b, c зэргийн аль нэг"`; `ja` `"a、b、c"`; `zh` disjunction `"a、b或c"`; `ar` `"a وb وc"`;
    - Review Focus 1: `["\ud800", "x\udfff"]` is `"\ud800 and x\udfff"`; `he` `["x", "\ud800"]` uses `" ו‑"`; emoji and Ext-B elements come back unchanged;
    - `[]` is `""` and `[]` parts; `["only"]` is one element part; `format()` is `""`; `format("abc")` is `"a, b, and c"` (a string iterates by code point); a `Set` works; a custom iterable yielding `"a"` then `1`, whose iterator has an instrumented `return`, makes `format` throw `TypeError` after two `next` calls and one `return` call (a plain array iterator has no `return`); `format(null)` throws `TypeError`; a `new String("x")` element throws `TypeError`;
    - `resolvedOptions()` is `{"locale":"en-US","type":"conjunction","style":"long"}`; `new Intl.ListFormat("de-u-co-phonebk").resolvedOptions().locale` is `"de"`; `{type: "foo"}` throws `RangeError`; `Intl.ListFormat()` throws `TypeError`; options `null` throws `TypeError`; options are read in the order `localeMatcher`, `type`, `style`;
    - `supportedLocalesOf(["de", "xx"])` is `["de"]`; `Object.prototype.toString.call(new Intl.ListFormat())` is `"[object Intl.ListFormat]"`.
  - `list-format-objects.js`: Review Focus 3 for ListFormat.
  - `list-format-gc.js`: 2,000 ListFormats alternating locales, types and styles, `gc()` every 100, every tenth kept and used afterwards. ASan and LeakSanitizer report nothing.
  - `shapes-table.js`: rows `Intl` `ListFormat` constructor length 0; `Intl.ListFormat` `supportedLocalesOf` method 1; `Intl.ListFormat.prototype` `resolvedOptions` method 0, `format` method 1, `formatToParts` method 1, `@@toStringTag` `Intl.ListFormat`.
  - `tamper.js`: Review Focus 2 for ListFormat.
- [ ] **Step 2: Run them to see them fail.** `check-intl-rust` fails to compile; `LIT_FILTER=list-format` fails: `Intl.ListFormat` is undefined.
- [ ] **Step 3: Implement** `list.rs`, the C interface, `ListFormatState`, the two helpers, `06-iterators.js` and `60-ListFormat.js`, step by step from `spec/listformat.html` and ECMA-262. The constructor ends with `listFormatInit` and the language identifier of [[Locale]].
- [ ] **Step 4: Run `check-intl-rust` and `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Update the expectations.** The 79 `Phase 5: Intl.ListFormat` lines go. List every removed line in the report. Then run `check-intl-test262`. Expected: `0 problem(s)`.
- [ ] **Step 6: Commit.**

### Task 3: RelativeTimeFormat

**Files:**
- Create: `intl/core/src/relativetime.rs`, `intl/jsi/RelativeTimeFormatHelpers.cpp`, `intl/js/62-RelativeTimeFormat.js`
- Modify: `intl/core/src/lib.rs`, `intl/core/src/number.rs` (`load`, `by_category`, `plural_categories`, `render_compact` become `pub(crate)`), `intl/jsi/NativeHelpers.def`, `intl/jsi/RustCore.h`, `intl/CMakeLists.txt`, `intl/js/45-PluralRules.js` (decision 4's split), `intl/js/10-Intl.js`, `intl/test/test262-expectations.txt`
- Test: unit tests in `relativetime.rs`; Create `intl/test/lit/relative-time-format.js`, `relative-time-format-objects.js`; modify `shapes-table.js`, `tamper.js`

**Interfaces:**
- Consumes: `CreateNumberFormat`, `PartitionNumberPattern(nf, x)` (returns `{parts, ...}`; use `.parts`), `PartitionPattern`, `IntlMVFromNumber`, `numberFormatLocaleData`, `PLURAL_CATEGORY_INDEX`, `ResolvePlural(pr, intlMV)` (takes an Intl mathematical value, returns `{pluralCategory, ...}`), `newDataCache`.
- Produces:
  - `relativetime::patterns(locale: &str, style: &str) -> Option<Vec<String>>`;
  - `HERMES_INTL_HELPER(relativeTimePatterns, Utf8List, Utf8, Utf8)`: `(locale, style)`, style `"long"`, `"short"` or `"narrow"`. For each unit in the order second, minute, hour, day, week, month, quarter, year: entries 0-5 the future patterns and 6-11 the past patterns for zero, one, two, few, many and other, in CLDR form (`"in {0} days"`; a pattern may lack `{0}`; a category the locale lacks gets other's), then the number N of relative names, then N pairs (the offset as a decimal string, `"-1"`, and the name, `"yesterday"`);
  - `initializePluralRules(pluralRules, locales, options)` and `CreatePluralRules(locales, options)` in `45-PluralRules.js`;
  - `RelativeTimeFormatSlots`; SingularRelativeTimeUnit, PartitionRelativeTimePattern, MakePartsList, FormatRelativeTime, FormatRelativeTimeToParts.

- [ ] **Step 1: Write the failing tests.**
  - Rust, in `relativetime.rs`:
    - `english`: `en` long day: future other `"in {0} days"`, one `"in {0} day"`, past other `"{0} days ago"`, relative names -1 `"yesterday"`, 0 `"today"`, 1 `"tomorrow"`; `en` short week past other `"{0} wk. ago"`; `en` narrow month future other `"in {0}mo"`;
    - `plurals`: `pl` long year future few `"za {0} lata"` and many `"za {0} lat"`; `ar` second past one `"قبل ثانية واحدة"` (no `{0}`);
    - `names`: `de` day -2 `"vorgestern"`; `gd` day 3 `"an-eararais"`; `ja` day 2 `"明後日"`;
    - `every_locale` (Review Focus 5): every locale of `locales.txt` and style gives 8 units of 12 patterns, each a string, and no locale has an explicit `=0` or `=1` pattern (so a CLDR change that adds one is noticed);
    - `rejects`: style `"wide"` and locale `"en_US"` give `None`.
  - `relative-time-format.js`, each value with `JSON.stringify`:
    - `en`: `format(-1, "day")` `"1 day ago"`; `numeric: "auto"`: `format(-1, "day")` `"yesterday"`, `format(-0, "second")` `"now"`, `formatToParts(1, "days")` one literal `"tomorrow"` with no `unit`, `formatToParts(1, "quarter")` `"next quarter"`;
    - `formatToParts(-1234.5, "hours")` is integer `"1"`, group `","`, integer `"234"`, decimal `"."`, fraction `"5"` (each with `unit: "hour"`), literal `" hours ago"` (no `unit`); `style: "narrow"` `formatToParts(3, "month")` literal `"in "`, integer `"3"` with unit, literal `"mo"`; `style: "short"` `formatToParts(-0, "week")` integer `"0"`, literal `" wk. ago"`;
    - `de` `numeric: "auto"` `format(-2, "day")` `"vorgestern"`; `de` `formatToParts(1234567.891, "year")` with `"."` groups and `","` decimal; `fr` `format(-1.5, "minute")` `"il y a 1,5 minute"`; `ar` `formatToParts(-1, "second")` one literal `"قبل ثانية واحدة"`; `ar` `format(-3, "hour")` `"قبل 3 ساعات"`; `he` `formatToParts(2, "day")` literal `"בעוד יומיים ("`, integer `"2"`, literal `")"`; `pl` short `format(-2, "year")` `"2 lata temu"`; `ru` narrow `format(21, "second")` `"+21 с"`; `ja` auto `format(2, "day")` `"明後日"`;
    - numbering systems: `en-u-nu-arab` `formatToParts(1000, "second")` integer `"١"`, group `"٬"`, integer `"٠٠٠"`; `{numberingSystem: "hanidec"}` `format(12, "day")` `"in 一二 days"`;
    - Review Focus 4: `format(1e21, "day")` `"in 1,000,000,000,000,000,000,000 days"`; `format(0.0001, "day")` `"in 0 days"`; `format(-Number.MIN_VALUE, "second")` `"0 seconds ago"`; `format(Number.MAX_VALUE, "year")` starts with `"in 179,769,313"`; `format(NaN, "day")`, `format(Infinity, "day")`, `format(1, "days ")`, `format(1, "Day")` and `format(1, "millisecond")` throw `RangeError`; the value is converted before the unit (a throwing `valueOf` runs before a throwing unit `toString`);
    - `resolvedOptions()`: `en-u-nu-thai` with `{style: "short"}` is `{"locale":"en-u-nu-thai","style":"short","numeric":"always","numberingSystem":"thai"}`; `de-u-nu-native` resolves `"de"` and `latn`; `en-u-ca-japanese-nu-latn` resolves `"en-u-nu-latn"`; options `null` throws `TypeError`; `Intl.RelativeTimeFormat()` throws `TypeError`; options are read in the order `localeMatcher`, `numberingSystem`, `style`, `numeric`.
  - `relative-time-format-objects.js`: Review Focus 3 for RelativeTimeFormat.
  - `shapes-table.js`: the constructor (length 0), `supportedLocalesOf` (1), `resolvedOptions` (0), `format` (2), `formatToParts` (2), `@@toStringTag`.
  - `tamper.js`: Review Focus 2, including replaced `Intl.NumberFormat`, `Intl.PluralRules` and their prototype methods, and `Math.abs`.
- [ ] **Step 2: Run them to see them fail.**
- [ ] **Step 3: Implement** `relativetime.rs`, the helper, the PluralRules split (its lit tests must still pass unchanged) and `62-RelativeTimeFormat.js` step by step from `spec/relativetimeformat.html`, with decision 4's absolute value.
- [ ] **Step 4: Run `check-intl-rust` and `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Update the expectations.** The 79 `Phase 5: Intl.RelativeTimeFormat` lines and `numberingSystems-accepted-by-RelativeTimeFormat.js` go. Then run `check-intl-test262`. Expected: `0 problem(s)`.
- [ ] **Step 6: Commit.**

### Task 4: Display names in the Rust core and the JSI layer

**Files:**
- Create: `intl/core/src/displaynames.rs` (the ICU4X lookups and the reader of `display-names.txt`), `intl/jsi/DisplayNamesHelpers.cpp`
- Modify: `intl/core/src/lib.rs`, `intl/core/Cargo.toml` (`potential_utf` or `tinystr` only if a key type needs a direct dependency, at the version the lock file already has), `intl/jsi/NativeHelpers.def`, `intl/jsi/RustCore.h`, `intl/CMakeLists.txt`
- Test: unit tests in `displaynames.rs`

**Interfaces:**
- Consumes: Task 1's `display-names.txt`.
- Produces, by decision 5:
  - `displaynames::name(locale: &str, kind: &str, code: &str, style: &str) -> Option<Option<String>>`: the outer `None` for bad input, the inner `None` for no name. `kind` is `"language"` (a language subtag), `"dialect"` (`language-script` or `language-region`), `"region"`, `"script"`, `"variant"` (lowercase), `"currency"` (uppercase), `"calendar"` or `"dateTimeField"`;
  - `displaynames::locale_patterns(locale: &str) -> Option<[String; 2]>`: `localePattern` and `localeSeparator`;
  - helper lines:
    - `HERMES_INTL_HELPER(displayName, OptUtf8, Utf8, Utf8, Utf8, Utf8)`: `(locale, kind, code, style)`; undefined when there is no name; throws for an unknown kind or style or a locale that does not parse;
    - `HERMES_INTL_HELPER(localeDisplayPatterns, Utf8List, Utf8)`: `(locale)`, entries 0 and 1 in CLDR form.

- [ ] **Step 1: Write the failing tests** in `displaynames.rs`, values from CLDR 48.2 (Node agrees unless noted):
  - `regions`: `en` `US` long `"United States"`, short `"US"` (from the code-equal list; ICU4X alone would give `"United States"`), narrow `"US"`; `GB` short `"UK"`; `BA` short `"Bosnia"`; `de` `UN` short `"UN"`;
  - `scripts`: `en` `Hant` long `"Traditional"`; `Thai` `"Thai"` (code-equal list); `Latn` `"Latin"`;
  - `languages`: `en` `en` `"English"`, dialect `en-US` long `"American English"`, short `"US English"`; `zh-Hant` dialect `zh-Hant` `"繁體中文"`; `en` `und` `"Unknown language"`; `fr-CA` `fon` `"fon"` (code-equal list);
  - `variants`: `en` `1996` `"German orthography of 1996"`; `ja` `1996` `"ドイツ語正書法(1996)"` (the composition turns its parentheses into brackets);
  - `currencies`: `en` `USD` `"US Dollar"`; for all 17,576 codes `AAA` to `ZZZ`, `en` has a name exactly for the 307 codes of `currencies.txt`;
  - `calendars`: `en` `gregory` `"Gregorian Calendar"`, `ethioaa` and `ethiopic-amete-alem` the same name, `islamicc` `islamic-civil`'s name, `islamic` and `islamic-rgsa` none, every one of the 16 supported calendars a name;
  - `fields`: `en` `year` long `"year"`, short `"yr."`, narrow `"yr"`; `weekday` `"day of the week"`, `"day of wk."`, `"day of wk."`; `de` `era` `"Epoche"`, `"Ep."`, `"E"`; `ar` `timeZoneName` long `"التوقيت"`;
  - `patterns`: `en` `["{0} ({1})", "{0}, {1}"]`; `zh-Hant`'s pattern uses U+FF08 and U+FF09; `ar`'s separator is `"{0}، {1}"`;
  - `every_locale` (Review Focus 5): for every locale of `locales.txt`, every kind and style, a lookup succeeds (the outer `Some`); every locale other than `und` has names for language `en`, region `US`, script `Latn`, calendar `gregory` and field `year` in each style, and both patterns; currency `USD` has a name in every locale other than `und` except a list the test pins, and none in each listed locale; for `und` (CLDR root has no language, calendar or currency names; ICU4X has no root language or currency data) every lookup succeeds, both patterns exist, and the test pins which of those names it has;
  - `rejects`: kind `"lang"`, style `"wide"`, locale `"en_US"` give the outer `None`; region `"ZZZZ"` gives no name without a panic.
- [ ] **Step 2: Run them to see them fail.** `check-intl-rust` fails to compile.
- [ ] **Step 3: Implement** `displaynames.rs`, the C interface and the two helpers, by decision 5. ICU4X lookups use the language identifier as `DataLocale` with the plugin's silent `load` pattern (`number.rs`), so fallback follows ICU4X's locale fallback. A malformed line in `display-names.txt` is a panic.
- [ ] **Step 4: Run `check-intl-rust` and `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Commit.**

### Task 5: Intl.DisplayNames

**Files:**
- Create: `intl/js/64-DisplayNames.js`
- Modify: `intl/js/10-Intl.js`, `intl/test/test262-expectations.txt`
- Test: Create `intl/test/lit/display-names.js`, `display-names-objects.js`; modify `shapes-table.js`, `tamper.js`

**Interfaces:**
- Consumes: Task 4's helpers, `canonicalizeTag`, `isUnicodeType`, `IsWellFormedCurrencyCode`, ResolveOptions with `require-options`.
- Produces: `DisplayNamesSlots`; CanonicalCodeForDisplayNames, IsValidDateTimeFieldCode, and the composition of decision 5.

- [ ] **Step 1: Write the failing lit test** `display-names.js`, `fallback: "none"` unless stated:
  - region: `en` `US` long `"United States"`, short and narrow `"US"`; `GB` short `"UK"`; `"us"` `"United States"` (decision 5); `"ZZZ"` throws `RangeError` (not `unicode_region_subtag`); `"ZZ"` is `"Unknown Region"` (CLDR names it); `"ZY"` with `fallback: "code"` is `"ZY"` and with `"none"` undefined;
  - script: `en` `Hant` `"Traditional"`, `"latn"` `"Latin"`;
  - language, `en`: `en-US` dialect long `"American English"`, short `"US English"`, standard long `"English (United States)"`, short `"English (US)"`; `zh-Hans-CN` `"Chinese (Simplified, China)"` in every style and display; `en-ZY` with `"code"` `"en-ZY"`; `und` `"Unknown language"`; `sgn-GR` with `"code"` `"gss"`; `de` `zh-Hans-CN` `"Chinesisch (Vereinfacht, China)"`, `en-GB` dialect short `"Englisch (GB)"`, standard short `"Englisch (UK)"`; `zh-Hant` `sr-Latn-RS` `"塞爾維亞文（拉丁字母，塞爾維亞）"`, `zh-Hant` dialect `"繁體中文"`, standard `"中文（繁體）"`; `ar` `zh-Hans-CN` `"الصينية (المبسطة، الصين)"`; `ja` `de-1996` `"ドイツ語 (ドイツ語正書法[1996])"`; `zh` `en-CD` `"英语（刚果［金］）"`;
  - language code checks: `"en-u-ca-gregory"`, `"en-x-foo"`, `"root"`, `"abcd"`, `"abcd-GB"`, `"en_GB"`, `"i-klingon"` throw `RangeError`; `"sh"` is the name of `sr-Latn`;
  - currency: `en` `"usd"` `"US Dollar"`; `"ABC"` with `"code"` `"ABC"`; `"US"` throws `RangeError`;
  - calendar: `en` `gregory` `"Gregorian Calendar"`; `"GREGORY"` the same name; `islamic` undefined; `"gregorian"` throws `RangeError` (9 letters); `"islamic_civil"` throws `RangeError`;
  - dateTimeField: `en` `year` long `"year"`, short `"yr."`, narrow `"yr"`; `weekday` `"day of the week"`; `de` `era` `"Epoche"`, `"Ep."`, `"E"`; `"Year"` and `"millisecond"` throw `RangeError`;
  - Review Focus 4: a tag with 100 distinct variants gives the code with `"code"` and no exception; `of(Symbol())` throws `TypeError`; `of({toString() { return "US"; }})` uses ToString;
  - `resolvedOptions()` of `new Intl.DisplayNames("en", {type: "language"})` is `{"locale":"en","style":"long","type":"language","fallback":"code","languageDisplay":"dialect"}`; with `type: "region"` and locale `"en-u-ca-chinese"` it is `{"locale":"en","style":"long","type":"region","fallback":"code"}`;
  - `new Intl.DisplayNames("en")` and `new Intl.DisplayNames("en", {})` throw `TypeError`; options `null` throws `TypeError`; options are read in the order `localeMatcher`, `style`, `type`, `fallback`, `languageDisplay` (also for type `"region"`); a throwing `prototype` getter on `new.target` throws before the options are read.
- [ ] **Step 2: `display-names-objects.js`, `shapes-table.js` and `tamper.js`:** Review Focus 3 and 2 for DisplayNames; rows for the constructor (length 2), `supportedLocalesOf` (1), `resolvedOptions` (0), `of` (1), `@@toStringTag`. The constructor's `length` is 2 (`Intl.DisplayNames ( locales, options )`; test262's `length.js`).
- [ ] **Step 3: Run them to see them fail.**
- [ ] **Step 4: Implement** `64-DisplayNames.js` step by step from `spec/displaynames.html`, with decision 5's composition.
- [ ] **Step 5: Run `check-intl-lit`.** Expected: pass.
- [ ] **Step 6: Update the expectations.** The 52 `Phase 5: Intl.DisplayNames` lines and the two `accepted-by-DisplayNames` lines go. Then run `check-intl-test262`. Expected: `0 problem(s)`.
- [ ] **Step 7: Commit.**

### Task 6: Intl.DurationFormat

**Files:**
- Create: `intl/js/66-DurationFormat.js`
- Modify: `intl/js/10-Intl.js`, `intl/test/test262-expectations.txt`
- Test: Create `intl/test/lit/duration-format.js`, `duration-format-limits.js`, `duration-format-objects.js`, `duration-format-locales.js`; modify `shapes-table.js`, `tamper.js`

**Interfaces:**
- Consumes: `CreateNumberFormat`, `PartitionNumberPattern`, `IntlMVFromNumber`, `makeIntlMV`, `numberFormatLocaleData`, `cachedNumberSymbols` (entry 21), Task 2's `CreateListFormat` and `CreatePartsFromList`.
- Produces: `DurationFormatSlots`; the draft's operations from GetDurationUnitOptions to PartitionDurationFormatPattern, by decision 7.

- [ ] **Step 1: Write the failing lit tests**, `en` unless stated, each with `JSON.stringify`:
  - `duration-format.js`:
    - `{}` with `{years: 1, months: 2, weeks: 3, days: 4, hours: 5, minutes: 6, seconds: 7, milliseconds: 8, microseconds: 9, nanoseconds: 10}` is `"1 yr, 2 mths, 3 wks, 4 days, 5 hr, 6 min, 7 sec, 8 ms, 9 μs, 10 ns"`, and its parts begin integer `"1"` (unit `year`), literal `" "` (unit `year`), unit `"yr"` (unit `year`), literal `", "` (no unit);
    - `long` `{years: 1, days: 2, hours: 3}` `"1 year, 2 days, 3 hours"`; `narrow` `{hours: 1, minutes: 2, seconds: 3}` `"1h 2m 3s"`;
    - `digital` `{hours: 1, minutes: 2, seconds: 3, milliseconds: 456}` `"1:02:03.456"` with parts integer `"1"` (hour), literal `":"`, integer `"02"` (minute), literal `":"`, integer `"03"` (second), decimal `"."` (second), fraction `"456"` (second); `{days: 1, minutes: 5}` `"1 day, 0:05:00"`; `fractionalDigits: 2` `{seconds: 3, milliseconds: 456, microseconds: 789}` `"0:00:03.45"`; `{hours: -1, minutes: -2, seconds: -3}` `"-1:02:03"` with a `minusSign` part first; `hoursDisplay: "auto"` `{minutes: 5, seconds: 6}` `"05:06"`;
    - `{hours: 0, minutes: -2, seconds: -3}` `"-2 min, 3 sec"`; `{hours: "numeric"}` `{hours: 1, seconds: 5}` `"1:00:05"`; `{minutes: "numeric"}` `{hours: 2, minutes: 3, seconds: 4}` `"2 hr, 3:04"`; `{seconds: "numeric", fractionalDigits: 3}` `{seconds: 1, milliseconds: 2, microseconds: 3}` `"1.002"`; `{milliseconds: "numeric"}` `{seconds: 1, milliseconds: 2, microseconds: 3, nanoseconds: 4}` `"1.002003004 sec"`; `{seconds: 10000000, nanoseconds: 1}` `"10,000,000 sec, 1 ns"`;
    - `{years: -0}` is `""`, and with `yearsDisplay: "always"` `"0 yrs"` (decision 7);
    - `de` long `{hours: 1, minutes: 30}` `"1 Stunde, 30 Minuten"`; `es` short `{years: 2, months: 1, days: 3}` `"2 a, 1 m., 3 d"`; `fr` digital `{hours: 12, minutes: 3, seconds: 4, milliseconds: 5}` `"12:03:04,005"`; `da` digital `{hours: 1, minutes: 2, seconds: 3}` `"1.02.03"` (decision 7); `ar` long `{hours: 2, minutes: 5}` `"ساعتان و5 دقائق"`, its first part unit `"ساعتان"` (unit `hour`) with no number part; `ar-u-nu-arab` digital `"١:٠٢:٠٣"`; `ja` narrow `"1h2m3s"`; `he` long `{hours: 1, minutes: 2, seconds: 11}` `"1 שעה, שתי דקות ו‑11 שניות"` with U+2011 (decision 2; Node has U+002D);
    - `resolvedOptions()` of `new Intl.DurationFormat("en")` lists `locale`, `numberingSystem`, `style`, then each unit and its display, in the draft's order, with no `fractionalDigits`; with `{style: "digital", fractionalDigits: 3}` hours `"numeric"`/`"always"`, minutes and seconds `"2-digit"`/`"always"`, milliseconds to nanoseconds `"numeric"`/`"auto"`, and `fractionalDigits: 3` last;
    - errors: `Intl.DurationFormat()` throws `TypeError`; options `null` throws `TypeError`; `{style: "digital", minutes: "long"}` throws `RangeError` (a later unit not numeric); `{milliseconds: "numeric", millisecondsDisplay: "always"}` throws `RangeError`; `format("PT1H")` throws `RangeError`, `format(1)` and `format({})` `TypeError`; `format({hours: 1.5})` `RangeError`; `format({hours: 1, minutes: -1})` `RangeError`; the fields are read in alphabetical order.
  - `duration-format-limits.js` (Review Focus 4): `years: 2**32 - 1` formats and `2**32` throws `RangeError`; `seconds: 2**53 - 1` formats and `2**53` throws; `{seconds: 2**53 - 1, milliseconds: 999}` formats and `{seconds: 2**53 - 1, milliseconds: 1000}` throws (the exact sum reaches 2^53); `milliseconds: 4503599627370496000` is `"4,503,599,627,370,496,000 ms"`, and `milliseconds: 4503599627370497000`, whose exact value is 4503599627370497024, is `"4,503,599,627,370,497,024 ms"` (decision 7); `{seconds: 9007199, milliseconds: 254740991, nanoseconds: 1}` digital with `fractionalDigits: 9` is `"0:00:9261939.991000001"` (Node's inexact sum gives `"…991000000"`); `nanoseconds: Number.MAX_SAFE_INTEGER` is `"9,007,199,254,740,991 ns"`; `fractionalDigits` 0 and 9 work and 10 throws `RangeError`.
  - `duration-format-locales.js` (Review Focus 5): for the 40 locales of `Inputs/datetime-locales.js`, each style formats `{days: 1, hours: 2, minutes: 3, seconds: 4, milliseconds: 5}` without an exception, and `formatToParts` joins to `format`.
  - `duration-format-objects.js`, `shapes-table.js` (constructor 0, `supportedLocalesOf` 1, `resolvedOptions` 0, `format` 1, `formatToParts` 1, `@@toStringTag`), `tamper.js` (Review Focus 3 and 2, including replaced `Intl.ListFormat`, `Intl.NumberFormat`, their prototype methods and `BigInt`).
- [ ] **Step 2: Run them to see them fail.**
- [ ] **Step 3: Implement** `66-DurationFormat.js` step by step from `spec/durationformat.html`, with decision 7.
- [ ] **Step 4: Run `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Update the expectations.** The 105 `Phase 5: Intl.DurationFormat` lines go. Then run `check-intl-test262`. Expected: `0 problem(s)`.
- [ ] **Step 6: Commit.**

### Task 7: Segmentation in the Rust core and the JSI layer

**Files:**
- Create: `intl/core/src/segmenter.rs`, `intl/jsi/SegmenterHelpers.cpp`
- Modify: `intl/core/src/lib.rs`, `intl/jsi/NativeHelpers.def`, `intl/jsi/RustCore.h`, `intl/CMakeLists.txt`, `intl/test/unit/CMakeLists.txt`
- Test: unit tests in `segmenter.rs`; gtests in `NativeHelpersTest.cpp`

**Interfaces:**
- Produces, by decisions 9 and 10:
  - `segmenter::Segmenter` (`HiSegmenter`, `Send` asserted): `Segmenter::new(locale: &str, granularity: &str) -> Option<Self>`;
  - `segmenter::Segments` (`HiSegments`, `Send` asserted): `Segmenter::segment(&self, text: &[u16]) -> Segments`; `Segments::find_boundary(&self, index: u32, after: bool) -> Option<u32>` (`index` < the length; before: the last boundary ≤ `index`; after: the first boundary > `index`); `Segments::is_word_like(&self, start: u32) -> Option<bool>` (`start` a boundary other than the length, word granularity only);
  - helper lines:
    - `HERMES_INTL_HELPER(segmenterInit, Undefined, Object, Utf8, Utf8)`: `(segmenter, locale, granularity)`;
    - `HERMES_INTL_HELPER(segmentsInit, Undefined, Object, Object, Utf16)`: `(segments, segmenter, string)`, attaches a `SegmentsState`; throws if `segmenter` has no `SegmenterState` or `segments` already has state;
    - `HERMES_INTL_HELPER(segmentsFindBoundary, Int, Object, Int, Bool)`: `(segments, index, after)`;
    - `HERMES_INTL_HELPER(segmentsIsWordLike, Bool, Object, Int)`: `(segments, start)`.

- [ ] **Step 1: Write the failing tests.**
  - Rust, in `segmenter.rs` (segments written as `text|text`, `*` for word-like):
    - `grapheme`: `"e\u0301👩‍👩‍👧‍👦🇺🇸\r\n\ud800x"` (a decomposed é) gives boundaries at 0, 2, 13, 17, 19, 20, 21, and the same with precomposed `"\u00e9"` at 0, 1, 12, 16, 18, 19, 20; `"क्षि ক্ষ্ম"` keeps each conjunct whole;
    - `word`: `"Hello, world! It's 3.14."` gives `Hello*|,| |world*|!| |It's*| |3.14*|.`; `ja` `"こんにちは世界"` `こんにちは*|世界*`; `zh` `"我爱北京天安门"` `我*|爱*|北京*|天安门*`; `th` `"ทุกสองสัปดาห์"` `ทุก*|สอง*|สัปดาห์*`;
    - `word_workarounds` (decision 9): `"𠀀𠀁𠀂 𪚥𪚥"` is `𠀀*|𠀁*|𠀂*| |𪚥*|𪚥*`; `"日本語𠀀テスト"` is `日本語*|𠀀*|テスト*`; decomposed `"café x"` (`e` + U+0301), `"की एस"` and `"كَتَبَ ولد"` have a word-like first segment; `"a\ud800b \udc00"` is `a*|\ud800|b*| |\udc00` with UTF-16 indices 0, 1, 2, 3, 4;
    - `sentence`: `"Mr. Smith left. Did he? Yes!\nOK"` gives `"Mr. "`, `"Smith left. "`, `"Did he? "`, `"Yes!\n"`, `"OK"`; `"Τι κάνεις; Καλά. Ναι;Όχι"` is 2 sentences with `en` and 4 with `el`;
    - `unbreakable` and `breakable`: test262's `containing/unbreakable-input.js` inputs are one segment in each granularity, and its `breakable-input.js` inputs at least two for grapheme and word;
    - `find_boundary`: for `"Hello world"` word, index 7 before is 6 and after is 11; index 5 before is 5 and after 6;
    - `every_locale` (Review Focus 5): each granularity initializes for every locale of `locales.txt`;
    - `large` (Review Focus 4): a one-million-code-unit string segments in each granularity and its last boundary is the length;
    - `rejects`: granularity `"line"`, locale `"en_US"`, `find_boundary` at the length, and `is_word_like` on a grapheme or sentence `Segments` or at a non-boundary give `None`.
  - gtests: `SegmenterStatesFreedWithRuntime` (a Segmenter and two Segments objects, one boundary query each, destroy the runtime; LeakSanitizer reports nothing); `SegmenterHelpersRejectMisuse` (each helper on an object without its state, `segmentsInit` twice, an index out of range: each throws an Error containing `Intl internal error`).
- [ ] **Step 2: Run them to see them fail.** `check-intl-rust` fails to compile.
- [ ] **Step 3: Implement** `segmenter.rs`, the C interface, the two states and the four helpers, by decision 9. The word-type re-check uses `icu::properties::CodePointMapData::<WordBreak>`.
- [ ] **Step 4: Run `check-intl-rust` and `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Commit.**

### Task 8: Intl.Segmenter

**Files:**
- Create: `intl/js/68-Segmenter.js`
- Modify: `intl/js/10-Intl.js`, `intl/test/test262-expectations.txt`
- Test: Create `intl/test/lit/segmenter.js`, `segmenter-text.js`, `segmenter-objects.js`, `segmenter-gc.js`; modify `shapes-table.js`, `tamper.js`

**Interfaces:**
- Consumes: Task 7's helpers, %Iterator.prototype% from Task 2, `ToIntegerOrInfinity`.
- Produces: `SegmenterSlots`, `SegmentsSlots`, `SegmentIteratorSlots`, %IntlSegmentsPrototype%, %IntlSegmentIteratorPrototype%, CreateSegmentsObject, CreateSegmentIterator, CreateSegmentDataObject, FindBoundary.

- [ ] **Step 1: Write the failing lit tests**, each with `JSON.stringify`:
  - `segmenter.js`:
    - word `"Hello world"`: `containing(7)` is `{"segment":"world","index":6,"input":"Hello world","isWordLike":true}`; `containing(-1)`, `containing(11)` and `containing(Infinity)` are undefined; `containing("5")` is the space at 5; `containing(NaN)` and `containing(-0.5)` are `"Hello"` at 0; `containing(1n)` and `containing(Symbol())` throw `TypeError`; `"".containing(0)` via `segment("")` is undefined;
    - iteration, calling `next` directly: grapheme `"e\u0301👩‍👩‍👧‍👦🇺🇸\r\n\ud800x"` gives `"e\u0301"`, `"👩‍👩‍👧‍👦"`, `"🇺🇸"`, `"\r\n"`, `"\ud800"`, `"x"` at 0, 2, 13, 17, 19, 20, and word `"Hello, world! It's 3.14."` gives Task 7's segments with their `isWordLike`; grapheme and sentence data objects have no own `isWordLike`; the own property names are `["segment", "index", "input", "isWordLike"]` for word; two iterators of one Segments object are independent, and `containing` between `next` calls changes nothing;
    - shapes Node shows: `Object.prototype.toString.call(seg.segment("a"))` is `"[object Object]"`; the iterator's is `"[object Segmenter String Iterator]"`; the iterator's prototype's prototype is %Iterator.prototype%; its prototype's own keys are `next` and `@@toStringTag` (non-writable, non-enumerable, configurable); `next.length` is 0; `[Symbol.iterator]` has `name` `"[Symbol.iterator]"`; the iterator's `@@iterator` returns itself;
    - `segment()` segments `"undefined"`; `segment(Symbol())` throws `TypeError`;
    - `resolvedOptions()`: `en-u-co-phonebk-x-foo` gives `{"locale":"en","granularity":"grapheme"}`; `["zh-Hant-TW", "de"]` with sentence `{"locale":"zh-Hant-TW","granularity":"sentence"}`; `"en-us"` `"en-US"`; `{granularity: "line"}` throws `RangeError`; options `"word"` and `null` throw `TypeError`; `Intl.Segmenter()` throws `TypeError`; `lineBreakStyle` is never read;
    - `supportedLocalesOf(["th", "xx", "zh-Hant-TW", "en-US-u-ca-buddhist"])` is `["th", "zh-Hant-TW", "en-US-u-ca-buddhist"]`.
  - `segmenter-text.js` (Review Focus 1 and 4): the word workaround cases of Task 7 through `segment`; unpaired surrogates and supplementary characters in every granularity, with segments that tile the input and indices that match `slice`; `el` sentences; a one-million-code-unit string iterated to the end and `containing(length - 1)`; Thai and Korean pinned to the plugin's output, with the ICU4C differences of decision 9 named in a comment.
  - `segmenter-objects.js`: Review Focus 3 for Segmenter, Segments and iterators (each method's brand check, including `next` on a Segments object and `containing` on an iterator).
  - `segmenter-gc.js`: 2,000 Segmenters and Segments objects alternating granularities, some iterated halfway, `gc()` every 100, survivors used afterwards; an iterator kept after its Segments object becomes unreachable still works. ASan and LeakSanitizer report nothing.
  - `shapes-table.js`: the constructor (0), `supportedLocalesOf` (1), `resolvedOptions` (0), `segment` (1), the prototype's `@@toStringTag`, %IntlSegmentsPrototype% `containing` (1) and `[Symbol.iterator]` (0), %IntlSegmentIteratorPrototype% `next` (0) and its `@@toStringTag`.
  - `tamper.js`: Review Focus 2 for Segmenter, including setters on `Object.prototype` for `segment`, `index`, `input`, `isWordLike`, `value` and `done`.
- [ ] **Step 2: Run them to see them fail.**
- [ ] **Step 3: Implement** `68-Segmenter.js` step by step from `spec/segmenter.html`, by decision 9. `containing` and `next` call `segmentsInit` lazily, after the draft's early returns; the Segments object's slots keep the Segmenter object, whose state `segmentsInit` reads.
- [ ] **Step 4: Run `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Update the expectations.** The 75 `Phase 5: Intl.Segmenter` lines go. Then run `check-intl-test262`. Expected: `0 problem(s)`.
- [ ] **Step 6: Commit.**

### Task 9: Acceptance, spec and README

**Files:**
- Modify:
  - `doc/superpowers/specs/2026-10-22-intl-plugin-design.md`:
    - "The JS layer": the new files, the iterator protocol, decision 8's internal functions, and the constructors' rules (decision 11);
    - "The native interface": decision 1's changes to "Strings" and "Parts", the three native states, and the new helpers; the Segmenter bullet of "Strings" names the UTF-8 word path (decision 9);
    - "Data": decisions 2, 4, 5, 6, 7 and 9, the sources of each constructor's data, and the library size after this phase in the size table; the Segmenter bullet gives the dictionary segmenter and its measured size;
    - "Phasing": Phase 5 done, with no expected failure;
  - `intl/README.md`: the layout table names the new Rust, C++ and JS files and `display-names.txt`; the opening paragraph lists the five constructors.

- [ ] **Step 1: Run everything.** Run `check-intl`, then a full `check-hermes`. Expected:
  - every part of `check-intl` passes, with test262 at `0 problem(s)`;
  - `check-hermes` has 0 unexpected results.

  Record the counts in the report.
- [ ] **Step 2: Check the expectations file.**
  - `grep -c 'Phase 5' intl/test/test262-expectations.txt` is 0.
  - `git diff <this plan's commit> -- intl/test/test262-expectations.txt` only removes lines.
- [ ] **Step 3: Measure the library.** Build the `hermesintl` target in Release, in a temporary worktree at `66951cb21` (the Phase 4 commit) and at the tip, each with its own build directory configured with Clang and `-DCMAKE_BUILD_TYPE=Release`. Record both sizes of `lib/libhermesintl.so` and of its `llvm-strip -o` output (`/usr/bin/llvm-strip-18` if `llvm-strip` is not on PATH), and the sizes of `.rodata`, `.text` and `.data.rel.ro` (`llvm-size -A`), then remove the worktree and the build directories.
- [ ] **Step 4: Update the spec and README** as listed above, in the documents' existing style. Write in plain English, with no jargon of our own.
- [ ] **Step 5: Commit.**

## After the tasks

When the whole-branch review is done and its findings are fixed:
1. Run a Codex code review (the `codex-review` skill) of the Phase 5 commits, from the plan commit to the tip.
2. Fix or drop every finding, with a reason. Never defer one.
3. Repeat until Codex reports every finding CLOSED.
4. Rerun `check-intl` and `check-hermes` on Linux, then ask the Mac agent (gist channel) for the same runs on macOS, and fix what it finds.
5. Squash the branch with the `squashing-feature-history` skill (fixes to earlier phases fold into those phases). If the new tip's tree is identical to the backup tag's, delete the backup.
