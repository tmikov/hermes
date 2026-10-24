# Intl Plugin Phase 1 (Intl.Locale) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `Intl.Locale` to the plugin with its whole public surface:
- the constructor and its options;
- the getters, `toString`, `maximize` and `minimize`;
- the locale information methods: `getCalendars`, `getCollations`, `getHourCycles`, `getNumberingSystems`, `getTimeZones`, `getTextInfo` and `getWeekInfo`;
- `Intl.Locale` objects accepted in every locale list.

At the end, every test262 `intl402/Locale` test passes, and so do the four other tests the expectations file assigns to Phase 1.

**Architecture:**
- The Rust core gains the likely-subtags algorithms and lookups of locale data. Each lookup is exposed through the existing `(data, len, out) -> bool` C interface and a native helper line in `NativeHelpers.def`.
- Two lookups (calendar preferences and hour cycles) read data files generated from CLDR 48.2. The rest read ICU4X's compiled data.
- The ECMA-402 algorithms are JS:
  - `intl/js/05-tags.js` holds the Unicode locale grammar and the locale-id abstract operations.
  - `intl/js/20-Locale.js` holds the constructor, its prototype and the information methods.
- The internal slots are private fields on a `ReturnOverride` subclass.

**Tech Stack:** Rust 1.88 with ICU4X 2.3 (`icu` with `compiled_data` and `sync`), C++17 and JSI, the plugin's JS compiled to Hermes bytecode, Python 3 (the CLDR generator and lit), and test262 at `c9aa6126e8f6fe2c6f668c65bfba771ffc8a6096`.

**Spec:** `doc/superpowers/specs/2026-10-22-intl-plugin-design.md`. Read "Guiding rules", "The JS layer", "The native interface", "Data", "Testing" and "Phasing" before any task. Task 7 writes this plan's decisions into it.

**ECMA-402 text:** the https://tc39.es/ecma402/ draft dated October 24, 2026, whose `spec/locale.html` is tc39/ecma402 commit `0eaf73fc7c` ("Editorial: Make the Locale object abstract operation algorithms more explicit"). The Intl Locale Info methods and `firstDayOfWeek` are part of it. Cite its section titles and number its steps.

## Global Constraints

- **Worktree:** work only in `/home/tmikov/work/hermes-intl`, on branch `hermes-intl`. **Never `cd`**; pass absolute paths. Use a subshell `( cd x; cmd )` only when a tool insists.
- **Build directory:** `/home/tmikov/work/hermes-intl/cmake-build-asan`. It is already configured: Clang, ASan, Debug with `-O1`, `JSI_UNSTABLE=ON`, and `HERMES_INTL_TEST262_DIR=/home/tmikov/work/hermes-intl/cmake-build-asan/test262` (the pinned checkout). Do not reconfigure it.
- **Commands:**
  - Rust tests: `cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-intl-rust`
  - Plugin lit tests and gtests: `cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-intl-lit`. Narrow a run with `LIT_FILTER=<regex>`.
  - test262 in plugin mode: `cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target check-intl-test262`
  - Everything: `check-intl`, then `check-hermes`.
  - Never run test262 in normal mode. The plugin-mode runs above are the only test262 runs allowed.
- **Updating the expectations:** use the command in `intl/README.md` ("Updating the expectations"), with the paths `check-intl-test262` passes in `intl/CMakeLists.txt`. After each update:
  - read the diff of `intl/test/test262-expectations.txt`;
  - every removed line must be a test this task made pass;
  - no line may be added.
- **Git:** at the end of each task, commit that task's files on `hermes-intl` with a plain-English message that says what changed and why. End the message with:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01JPBWtUGtZ5QcTC9dE7h3R8
  ```
  Never push, amend or reset.
- **Copyright header:** every new file gets the Meta MIT header used across the repo (`#` comments in Python).
- **Separability:** code under `intl/` includes only `<jsi/...>`, `<hermes/Public/IntlPlugin.h>`, its own headers and the generated `hermes_intl_core.h`. The one exception is `intl/test/unit/`.
- **Plugin JS rules:** `intl/README.md`, "Writing plugin JS", rules 1 to 6. Reviewers check every line against them. In particular:
  - no `String.prototype.split`, no regular expressions, and no `Array.prototype` methods;
  - every array is built with `listPush`;
  - every record and descriptor has `__proto__: null`.
- **Native helpers:** `intl/README.md`, "Adding a native helper". JS validates every argument before calling a helper. A helper that gets an argument JS should never send throws `std::logic_error("Intl internal error: ...")`.
- **Rust:**
  - FFI functions never panic on bad input. Invalid UTF-8, or a tag or region that does not parse, returns `false`.
  - A panic is only for a broken invariant in our own data files.
  - Every new public function has a doc comment and a unit test.
  - Run `cargo fmt` in `intl/core`, and `clang-format` on changed C++.
- **Data:** CLDR 48.2 (tag `release-48-2`), the CLDR release ICU4X 2.3 is built from. Generated files are committed, never edited by hand.
- **Prove checks can fail:** for every new test, make a stated mutation of the code under test, name the test that fails, then revert the mutation. Put this evidence in the task report.
- **Never bump `BYTECODE_VERSION`**, and never bump the plugin-interface version. Phase 1 adds native helpers and changes no entry-point, config or `IHermesBuiltinBytecode` interface.

## Planning decisions

Task 7 writes these into the spec. Implement them as stated.

1. **`[[LocaleExtensionKeys]]` is « "ca", "co", "fw", "hc", "kf", "kn", "nu" ».** Phase 2's Collator supports `kf` and `kn` (ICU4X's collator has both options), so the draft's condition holds. `caseFirst` and `numeric` therefore exist as getters, and `[[CaseFirst]]` and `[[Numeric]]` as slots.
2. **Where each piece of locale data comes from:**

   | Abstract operation | Data |
   |---|---|
   | Add / Remove Likely Subtags | ICU4X `LocaleExpander::new_extended()`: `maximize`, and `minimize`, which favors the region as UTS 35 does |
   | CalendarsOfLocale | CLDR `supplementalData.xml` `calendarPreferenceData`, with CLDR calendar names mapped to BCP 47 types through `bcp47/calendar.xml` aliases (`gregorian` → `gregory`) |
   | HourCyclesOfLocale | CLDR `supplementalData.xml` `timeData`, keyed by region or by language and region (`en_001` → `en-001`). `preferred` comes first, then `allowed`, without repeats. `h`, `hb` and `hB` map to `h12`, `H` to `h23`, `K` to `h11` and `k` to `h24`. |
   | TimeZonesOfLocale | CLDR `bcp47/timezone.xml`. A non-deprecated zone belongs to its `region` attribute, or else to the first two letters of its id, uppercased. `utc`, `gmt`, `unk`, `utce*` and `utcw*` belong to no region. Each zone is given as its primary identifier, through `host::time_zone_canonicalize` (ICU4X's IANA parser, then `primary_time_zone`), so the lists agree with `Intl.supportedValuesOf("timeZone")`. CLDR's first alias is not always the primary id (`inccu` lists `Asia/Calcutta` first). |
   | WeekInfoOfLocale | ICU4X `WeekInformation` for `und-<region>`, with ICU4X's fallback to `001` (see decision 3) |
   | TextDirectionOfLocale | ICU4X `LocaleDirectionality::new_extended()` on `und-<script>` |
   | NumberingSystemsOfLocale | the numbering system ICU4X's decimal symbols data gives the found locale (`DecimalSymbolsV1`'s `numsys()`) |
   | CollationsOfLocale | the collation types in `supported.rs`'s `COLLATIONS` that have a tailoring, loaded exactly, for the found locale or any locale on its ICU4X fallback chain down to `und` |

   The two CLDR-derived files come from `intl/core/data/update-cldr-lists.py`, extended in Task 2.
3. **Week data has no "not available" case.** CLDR's `001` week data applies to every region it does not list, and ICU4X's baked data drops region entries that equal `001`. Testing "week data for region X is available" with an exact load would therefore wrongly skip a region whose data equals `001`. For example, `en-US-u-rg-gbzzzz` would get US data instead of GB's. So WeekInfoOfLocale uses the region override when there is one, and otherwise the region, and always loads with fallback.
4. **The `%Intl.Collator%` and `%Intl.NumberFormat%` `[[AvailableLocales]]`** that CollationsOfLocale and NumberingSystemsOfLocale use are the plugin's one `[[AvailableLocales]]` list (`isAvailableLocale`). Phases 2 and 3 must keep it that way, or update these two methods.
5. **The draft is newer than the pinned test262** in two normative changes:
   - CollationsOfLocale has no default-locale fallback (tc39/ecma402#1072).
   - WeekInfoOfLocale goes through RegionPreference (#1051).

   Follow the draft. The pinned tests do not contradict either change: `getCollations/output-array-values.js` and the `getWeekInfo` tests were checked.

## Review Focus

These are the five inputs most likely to hurt someone that the spec implies but test262 does not exercise. Each one has a test in the task named.

1. **Tampering after the plugin loads:**
   - accessors on `Object.prototype` for the names the constructor's internal records use;
   - setters for `"0"` on `Array.prototype`;
   - a replaced `String.prototype.split` or `Array.prototype.sort`;
   - replaced `Intl.Locale.prototype.toString` and `Intl.Locale.prototype.maximize`.

   `new Intl.Locale(...)`, its getters, `maximize`, `getCalendars` and `Intl.getCanonicalLocales([locale])` give the same results regardless. Covered in Task 5 and Task 6 (`tamper.js`).
2. **Objects that look like Locales but are not:**
   - `Object.create(Intl.Locale.prototype)`;
   - a `Proxy` around a Locale;
   - a subclass instance;
   - a Locale whose own `toString` was overridden.

   The brand check uses the private field, so:
   - getters throw `TypeError` on the first two;
   - `getCanonicalLocales` reads the tag of a real Locale or subclass instance from its slot and never calls its `toString`;
   - a proxy is treated as a plain object, so its `toString` call reaches the brand check and throws `TypeError`.

   Covered in Task 5 (`locale-objects.js`).
3. **Hostile option values:**
   - lone surrogates;
   - non-ASCII letters that look like ASCII (`"ı"`, `"ſ"`);
   - a 10,000-character `calendar`;
   - `variants: "a-b"` style values with empty or overlong subtags;
   - `language: "root"`;
   - `region: "1234"`.

   Each gives `RangeError`, never a crash or an ill-formed tag reaching native code. Covered in Task 4 (`locale-tags.js`) and Task 5 (`locale-edge.js`).
4. **Locales with no data:**
   - `und`, `zxx` and `qaa-Qaaa-QM`;
   - a 5-8 letter language (`posix`);
   - regions `ZZ`, `QO`, `AQ` and `419`;
   - `-u-rg-` and `-u-sd-` values that are not subdivisions;
   - `-u-fw-xyz`.

   Every information method returns what the draft gives, which is often not its last-resort fallback: RegionPreference turns a missing region into `001`, which has calendar, hour-cycle and week data. No method throws. Covered in Task 6 (`locale-info-fallbacks.js`).
5. **`maximize` and `minimize` round trips** on tags with variants, `-t-` and `-x-` sequences, and long languages, for example:
   - `sl-rozaj-biske-1994`;
   - `en-t-ja-x-a`;
   - `posix-u-ca-buddhist`.

   Extensions and the canonical variants are kept, a long language is never altered, and `maximize().minimize()` of a minimal tag is that tag. Covered in Task 1 (Rust tests) and Task 5 (`locale-edge.js`).

---

### Task 1: Likely subtags in the Rust core

**Files:**
- Modify: `intl/core/src/locale.rs`, `intl/core/src/lib.rs`, `intl/jsi/RustCore.h`, `intl/jsi/NativeHelpers.def`, `intl/jsi/LocaleHelpers.cpp`

**Interfaces:**
- Produces:
  - Rust `pub fn add_likely_subtags(tag: &str) -> Option<String>` and `pub fn remove_likely_subtags(tag: &str) -> Option<String>` in `locale.rs`. Both take a canonical `unicode_locale_id` and return a canonical tag, or `None` if `tag` does not parse.
  - FFI `hi_locale_add_likely_subtags(tag, len, out) -> bool` and `hi_locale_remove_likely_subtags(tag, len, out) -> bool`.
  - Helpers `HERMES_INTL_HELPER(addLikelySubtags, Utf8, Utf8)` and `HERMES_INTL_HELPER(removeLikelySubtags, Utf8, Utf8)`. They throw `std::logic_error` when Rust returns false, because JS passes only `[[Locale]]` values.

- [ ] **Step 1: Write the failing Rust tests** in `locale.rs`:
  - `adds_likely_subtags`: every pair of test262 `intl402/Locale/likely-subtags.js` `testDataMaximal`, each also with every suffix of its `extras` (`""`, `"-fonipa"`, `"-a-not-assigned"`, `"-u-attr"`, `"-u-co"`, `"-u-co-phonebk"`, `"-x-private"`). Also:
    - `"posix"` → `"posix"`;
    - `"xtg"` → `"xtg"`;
    - `"aa-x-private"` → `"aa-Latn-ET-x-private"`;
    - `"de-u-kf"` → `"de-Latn-DE-u-kf"`;
    - `"sl-rozaj-biske-1994"` → `"sl-Latn-SI-1994-biske-rozaj"` (canonical variant order);
    - `"en-t-ja-x-a"` → `"en-Latn-US-t-ja-x-a"`.
  - `removes_likely_subtags`: every pair of `testDataMinimal` with the same suffixes. Also:
    - `"posix-u-ca-buddhist"` → itself;
    - `"en-Latn-US-t-ja-x-a"` → `"en-t-ja-x-a"`;
    - `"zh-Hant-TW"` → `"zh-TW"`.
  - `likely_subtags_round_trip`: for each minimal value in `testDataMinimal`, `remove(add(m)) == m`.
  - `likely_subtags_reject_unparsable`: `""` and `"en_US"` give `None`.

- [ ] **Step 2: Run them and see them fail.** Run `check-intl-rust`. Expected: a compile error, because the functions do not exist.

- [ ] **Step 3: Implement both functions.**
  - Run `canonicalize(tag)` first and return `None` if it fails. ICU4X's parser accepts `_`, and `canonicalize` is what rejects it.
  - Parse the canonical tag with `icu::locale::Locale` and run the shared `expander()` on `loc.id` only. Variants (already in canonical order), extensions and private use stay as they are.
  - Re-serialize, and return `canonicalize`'s result of that.
  - A language of 5 to 8 letters cannot parse as an ICU4X `Language`. Return such a tag unchanged: UTS 35 Lookup has no `und` step for a non-`und` language, so no likely subtags apply.

  Then add the two FFI functions in `lib.rs`, the shape `hi_locale_canonicalize` has, with doc comments. Add the wrappers to `RustCore.h`, the two `.def` lines with comments, and the two helpers in `LocaleHelpers.cpp`.

- [ ] **Step 4: Run them and see them pass.** Run `check-intl-rust` and `check-intl-lit`. Expected: all pass.

- [ ] **Step 5: Prove the tests can fail.** Swap `minimize` for `minimize_favor_script`; `removes_likely_subtags` fails on `"zh-Hant-TW"` (it gives `zh-Hant`). Drop the variants before re-serializing; `adds_likely_subtags` fails on `-fonipa`. Revert both.

- [ ] **Step 6: Commit.**

### Task 2: CLDR calendar and hour-cycle data

**Files:**
- Modify: `intl/core/data/update-cldr-lists.py`, `intl/core/src/lib.rs`, `intl/jsi/RustCore.h`, `intl/jsi/NativeHelpers.def`, `intl/jsi/LocaleHelpers.cpp`, `intl/README.md` ("Updating the CLDR lists")
- Create: `intl/core/data/calendar-preferences.txt`, `intl/core/data/hour-cycles.txt`, `intl/core/src/info.rs`

**Interfaces:**
- Produces:
  - In the data files, one line per key: `<key> <value> <value>...`, keys sorted, values in CLDR preference order. A key is a region (`US`, `001`) or a language and region joined with `-` (`en-001`, `hi-IN`). In `calendar-preferences.txt` the values are BCP 47 calendar types. In `hour-cycles.txt` they are `h11`, `h12`, `h23` or `h24`.
  - Rust in `info.rs`:
    - `pub fn calendar_preferences(region: &str) -> Option<&'static [String]>`;
    - `pub fn hour_cycles(key: &str) -> Option<&'static [String]>`.

    Both return `None` when the file has no line for the key.
  - FFI `hi_locale_calendar_preferences(region, len, out) -> bool` and `hi_locale_hour_cycles(key, len, out) -> bool`. `out` is the space-separated values, and the result is false when there is no line.
  - Helpers `HERMES_INTL_HELPER(calendarPreferences, OptUtf8, Utf8)` and `HERMES_INTL_HELPER(hourCycles, OptUtf8, Utf8)`.

- [ ] **Step 1: Extend the generator.**
  - `main()` also fetches `supplemental/supplementalData.xml` and `bcp47/calendar.xml`, and writes the two files, each with a header comment naming its source.
  - Calendar names map through `calendar.xml`: each `<type name=... alias=...>` turns an alias into its name, and other names are kept.
  - Hour symbols map as in decision 2. An unknown symbol or calendar name stops the script with an error.
  - `regions` attributes span lines, so split on whitespace.

  Run `python3 /home/tmikov/work/hermes-intl/intl/core/data/update-cldr-lists.py release-48-2`. Expected: the two new files exist, and `currencies.txt` and `numbering-systems.txt` are unchanged (`git diff --stat` shows neither).

- [ ] **Step 2: Write the failing Rust tests** in `info.rs`:
  - `calendar_preferences_follow_cldr`:
    - `TH` → `["buddhist", "gregory"]`;
    - `SA` → `["gregory", "islamic-umalqura", "islamic", "islamic-rgsa"]`;
    - `001` → `["gregory"]`;
    - `TW` → `["gregory", "roc", "chinese"]`;
    - `US` → `None`.
  - `hour_cycles_follow_cldr`:
    - `US` → `["h12", "h23"]`;
    - `JP` → `["h23", "h11", "h12"]`;
    - `hi-IN` → `["h12", "h23"]`;
    - `DE` → `["h23", "h12"]`;
    - `001` → `["h23", "h12"]`;
    - `en-001` → `["h12", "h23"]`;
    - `AQ` → `None`.
  - `data_files_are_well_formed`:
    - every key is a region `R` or `<language>-R`, where `canonicalize("und-R")` is `"und-R"` and the language is canonical;
    - every calendar is in `supported::values(Kind::Calendar)`, or is one of `islamic` and `islamic-rgsa`;
    - every value list is non-empty and has no repeats.

  These values are read from CLDR 48.2 `supplementalData.xml`. If the generated file disagrees, check the XML. Fix the generator if it is wrong; if the XML says otherwise, the XML wins, and the report must say so.

- [ ] **Step 3: Run them and see them fail.** Expected: a compile error.

- [ ] **Step 4: Implement `info.rs`.**
  - Parse each file once into a `BTreeMap` behind a `OnceLock`, with `include_str!`.
  - Declare `mod info;` in `lib.rs`, and add the FFI functions, the `RustCore.h` wrappers, the `.def` lines and the helpers.
  - Update the README's "Updating the CLDR lists" to name all four generated files.

- [ ] **Step 5: Run them and see them pass.** Run `check-intl-rust` and `check-intl-lit`.

- [ ] **Step 6: Prove the tests can fail.** Map `hB` to `h23` in the generator, rerun it, and `hour_cycles_follow_cldr` fails on `DE` (`preferred="H" allowed="H hB"`). Drop the alias mapping, and `calendar_preferences_follow_cldr` fails on `gregorian`. Revert both and regenerate.

- [ ] **Step 7: Commit.**

### Task 3: Locale data from ICU4X and the time-zone table

**Files:**
- Modify: `intl/core/src/info.rs`, `intl/core/src/lib.rs`, `intl/core/data/update-cldr-lists.py`, `intl/jsi/RustCore.h`, `intl/jsi/NativeHelpers.def`, `intl/jsi/LocaleHelpers.cpp`
- Create: `intl/core/data/time-zones-by-region.txt`

**Interfaces:**
- Consumes: `supported::COLLATIONS`. Make it `pub(crate)` if it is not.
- Produces Rust functions in `info.rs`, and their helpers. Every Utf8 list is space-separated.

  | Rust | Helper (`.def` line) | Meaning |
  |---|---|---|
  | `time_zones_of_region(region: &str) -> Vec<String>` | `timeZonesOfRegion, Utf8, Utf8` | the region's primary IANA ids, sorted, without repeats; empty for an unknown region |
  | `week_info(region: &str) -> Option<(u8, Vec<u8>)>` | `weekInfo, Utf8, Utf8` → `"<firstDay> <weekend day>..."` | ISO weekday numbers, Monday 1 to Sunday 7, with the weekend ascending. None, and the helper throws, only if the region does not parse. |
  | `script_direction(script: &str) -> Option<&'static str>` | `scriptDirection, OptUtf8, Utf8` | `"ltr"`, `"rtl"`, or None when unknown |
  | `default_numbering_system(locale: &str) -> Option<String>` | `defaultNumberingSystem, Utf8, Utf8` | the numbering system of the locale's decimal data. The helper throws on None. |
  | `collations_of_locale(locale: &str) -> Vec<&'static str>` | `collationsOfLocale, Utf8, Utf8` | sorted collation types |

  The FFI functions follow the `(data, len, out) -> bool` shape.

- [ ] **Step 1: Generate the time-zone table.**
  - The generator also fetches `bcp47/timezone.xml` and writes `time-zones-by-region.txt`, with one line per region: `<REGION> <IANA id>...`.
  - Each id is the first `alias` of the zone's `<type>`. The region follows decision 2.
  - The script stops with an error if a derived region is not two ASCII letters.

  Run it with `release-48-2`. Expected:
  - `PS` lists `Asia/Gaza Asia/Hebron`;
  - `IL` lists `Asia/Jerusalem`;
  - `DE` lists `Europe/Berlin Europe/Busingen`;
  - no line holds an `Etc/` id;
  - the other generated files are unchanged.

- [ ] **Step 2: Write the failing Rust tests** in `info.rs`:
  - `time_zones_of_regions`:
    - `JP` → `["Asia/Tokyo"]`;
    - `IN` → `["Asia/Kolkata"]`, the primary id, not `Asia/Calcutta`;
    - `PS` → `["Asia/Gaza", "Asia/Hebron"]`;
    - `US` contains `America/New_York`, `America/Los_Angeles`, `America/Indiana/Vevay` and `Pacific/Honolulu`, and nothing starting with `Etc/`;
    - `419` → `[]`;
    - `ZZ` → `[]`.
  - `time_zones_are_supported_primary_ids`: every zone in every region is in `supported::values(Kind::TimeZone)`.
  - `week_info_follows_cldr`:
    - `US` → `(7, [6, 7])`;
    - `GB` → `(1, [6, 7])`;
    - `IL` → `(7, [5, 6])`;
    - `AF` → `(6, [4, 5])`;
    - `IN` → `(7, [7])`;
    - `001` → `(1, [6, 7])`;
    - `AQ` → `(1, [6, 7])`.
  - `script_directions`:
    - `Arab`, `Hebr` and `Thaa` → `rtl`;
    - `Latn`, `Cyrl` and `Hani` → `ltr`;
    - `Zyyy` and `Qaaa` → None.
  - `default_numbering_systems`:
    - `en` → `latn`;
    - `ar-EG` → `arab`;
    - `fa` → `arabext`;
    - `hi-IN` → `latn`;
    - `und` → `latn`.
  - `collations_of_locales`:
    - `en` → `["emoji", "eor"]`;
    - `de` → `["emoji", "eor", "phonebk"]`;
    - `es` → `["emoji", "eor", "trad"]`;
    - `zh` contains `pinyin`, `stroke` and `zhuyin`;
    - no result contains `standard` or `search`.

  These values are read from CLDR 48.2 (week data and default numbering systems) and from ICU4X's compiled collation data. If the code disagrees, check the source. If the source says otherwise, the source wins, and the report must say so.

- [ ] **Step 3: Run them and see them fail.** Expected: a compile error.

- [ ] **Step 4: Implement the functions.**
  - `time_zones_of_region` reads the generated file once into a `BTreeMap` behind a `OnceLock`, maps each id through `host::time_zone_canonicalize` (an id it does not know is a broken data file: panic), then sorts and removes repeats.
  - `week_info` builds an ICU4X `WeekInformation` from the preferences of `und-<region>`, with fallback.
  - `script_direction` uses a `LocaleDirectionality::new_extended()` held in a `OnceLock`, applied to `und-<script>`.
  - `default_numbering_system` loads `DecimalSymbolsV1` with fallback for the locale and returns `numsys()`.
  - `collations_of_locale`: load every `COLLATIONS` entry for the locale the way the test helper `tailoring_loads` in `supported.rs` does. The baked provider falls back through the locale's parents to `und`, never across collation types, so `emoji` and `eor` load for every locale and `phonebk` only for `de` and its children. Keep the types that load, sorted.

  Then add the FFI functions, the wrappers, the `.def` lines and the helpers.

- [ ] **Step 5: Run them and see them pass.** Run `check-intl-rust` and `check-intl-lit`.

- [ ] **Step 6: Prove the tests can fail.**
  - Use the file's ids without `time_zone_canonicalize`, and `time_zones_of_regions` fails on `IN`.
  - Load the week data without fallback, and `week_info_follows_cldr` fails on `GB`.
  - Count a type only when the response metadata says the data came from the requested locale itself (no fallback), and `collations_of_locales` fails on `en`.

  Revert each mutation.

- [ ] **Step 7: Commit.**

### Task 4: The locale grammar and the Locale abstract operations in JS

**Files:**
- Create: `intl/js/05-tags.js`, `intl/test/lit/js/locale-tags.js`
- Modify: `intl/js/01-intrinsics.js`, if a new intrinsic is needed

**Interfaces:**
- Consumes: `native.canonicalizeLocale` (Phase 0), `native.addLikelySubtags` (Task 1), and `splitUnicodeExtension`, `UnicodeExtensionComponents`, `findKeyword` and `asciiLowercase` (Phase 0 JS).
- Produces, in `05-tags.js`:
  - Grammar checks, each taking a string and returning a boolean. Case-insensitive ASCII only (UTS 35 section 3.2); a non-ASCII code unit never matches.
    - `isUnicodeLanguageSubtag(s)`: alpha{2,3} or alpha{5,8};
    - `isUnicodeScriptSubtag(s)`: alpha{4};
    - `isUnicodeRegionSubtag(s)`: alpha{2} or digit{3};
    - `isUnicodeVariantSubtag(s)`: alphanum{5,8}, or a digit followed by alphanum{3};
    - `isUnicodeType(s)`: one or more `-`-separated alphanum{3,8};
    - `isUnicodeSubdivisionId(s)`: a region subtag followed by alphanum{1,4}.
  - `StringSplitToList(s, separator)` (ECMA-262), with no use of `split`.
  - The ECMA-402 operations with the draft's names and signatures:
    - `GetLocaleBaseName(locale)`, `GetLocaleLanguage(locale)`, `GetLocaleScript(locale)`, `GetLocaleRegion(locale)` and `GetLocaleVariants(locale)`;
    - `UnicodeExtensionValue(locale, key)`, which returns `undefined` for ~empty~;
    - `CanonicalUnicodeSubdivision(locale, key)`;
    - `RegionPreference(locale)`, which returns `{__proto__: null, region, regionOverride}`;
    - `WeekdayToUValue(fw)` and `WeekdayUValueToNumber(fw)`.

- [ ] **Step 1: Write the failing lit test** `intl/test/lit/js/locale-tags.js`.
  - Use the layout of `intl/test/lit/js/resolve-locale.js`: concatenate `Inputs/strict.js`, `01`, `02`, `03`, `04` and `05`, then run it as source and as bytecode.
  - Stub natives:
    - `canonicalizeLocale` lowercases the language, uppercases regions and title-cases scripts;
    - `addLikelySubtags` maps `en` → `en-Latn-US`, `und` → `en-Latn-US` and `zxx` → `zxx`.

  CHECK lines:
  - Grammar, each check against its own inputs:
    - language: accepts `en`, `haw` and `posix`; rejects `e`, `engl` and `english12`;
    - script: accepts `Latn`; rejects `Lat` and `Latin`;
    - region: accepts `US` and `419`; rejects `U`, `USA` and `4190`;
    - variant: accepts `fonipa` and `1994`; rejects `abcd` and `fonipa123`;
    - type: accepts `abc`, `buddhist` and `islamic-civil`; rejects `ab`, `abc-`, `-abc` and `abc--abc`;
    - subdivision: accepts `gbzzzz` and `usca`; rejects `gb`, `gbzzzzz` and `u`;
    - `"ı"`, `"ſ"` and a lone surrogate match nothing;
    - a 10,000-character `abc-abc-...` value is accepted by `isUnicodeType` in under a second and rejected by every other check.
  - `GetLocaleBaseName`:
    - `"en-Latn-US-fonipa-u-ca-x-a"` → `"en-Latn-US-fonipa"`;
    - `"en-x-a"` → `"en"`;
    - `"en-t-ja"` → `"en"`.
  - `GetLocaleScript("en-US")` → undefined.
  - `GetLocaleRegion("en-Latn")` → undefined.
  - `GetLocaleRegion("es-419")` → `"419"`.
  - `GetLocaleVariants`:
    - `"sl-rozaj-biske"` → `"rozaj-biske"`;
    - `"sl-IT-rozaj"` → `"rozaj"`;
    - `"en"` → undefined.
  - `UnicodeExtensionValue`:
    - `("en-u-ca-gregory", "ca")` → `"gregory"`;
    - `("en-u-kn", "kn")` → `""`;
    - `("en", "ca")` → undefined.
  - `CanonicalUnicodeSubdivision`:
    - `("en-u-rg-gbzzzz", "rg")` → `"GB"`;
    - `("en-u-rg-gb", "rg")` → undefined;
    - `("en-u-sd-419abc", "sd")` → `"419"`.
  - `RegionPreference`:
    - `"en"` → `{region: "US", regionOverride: undefined}`;
    - `"zxx"` → `{region: "001", ...}`;
    - `"en-u-rg-gbzzzz"` → `{region: "US", regionOverride: "GB"}`;
    - `"en-u-sd-gbsct"` → `{region: "GB"}`.
  - `WeekdayToUValue`:
    - `"0"` → `"sun"`;
    - `"7"` → `"sun"`;
    - `"1"` → `"mon"`;
    - `"mon"` → `"mon"`;
    - `"x"` → `"x"`.
  - `WeekdayUValueToNumber`:
    - `"sun"` → 7;
    - `"mon"` → 1;
    - `"0"` → undefined.

- [ ] **Step 2: Run it and see it fail.** Run `LIT_FILTER=locale-tags cmake --build ... --target check-intl-lit`. Expected: FAIL, because the functions are not defined.

- [ ] **Step 3: Implement `05-tags.js`.**
  - It gets the file header of the other files, naming the draft of October 24, 2026.
  - Every operation cites its section and numbers its steps, as in `03-locales.js`.
  - The grammar checks are small character loops over `charCodeAt`.

- [ ] **Step 4: Run it and see it pass.**

- [ ] **Step 5: Prove the test can fail.** Accept 4 digits in `isUnicodeRegionSubtag`, and `locale-tags.js` fails on `4190`. Make `GetLocaleVariants` return the subtag after the region, and it fails on `sl-rozaj-biske`. Revert both.

- [ ] **Step 6: Commit.**

### Task 5: The Intl.Locale constructor, getters, toString, maximize and minimize

**Files:**
- Create: `intl/js/20-Locale.js`, `intl/test/lit/locale-objects.js`, `intl/test/lit/locale-edge.js`
- Modify:
  - `intl/js/03-locales.js`: delete the stubs `isLocaleObject` and `localeObjectTag`, which `20-Locale.js` now defines;
  - `intl/test/lit/js/canonicalize-locale-list.js`, and any other `intl/test/lit/js/` test that concatenates `03-locales.js` without `20-Locale.js`: define the two functions in the test's own stubs (no object is a Locale there), so it keeps passing;
  - `intl/js/10-Intl.js`: `installIntl` calls `installLocale(IntlObject)`;
  - `intl/test/lit/shapes-table.js`, `intl/test/lit/tamper.js`, `intl/test/test262-expectations.txt`

**Interfaces:**
- Consumes: Task 4's operations, Task 1's helpers, and the Phase 0 `GetOption`, `CoerceOptionsToObject`, `CanonicalizeUValue`, `InsertUnicodeExtensionAndCanonicalize`, `defineConstructor`, `defineMethods`, `defineGetters`, `setToStringTag` and `ReturnOverride`.
- Produces, in `20-Locale.js`:
  - `var LOCALE_EXTENSION_KEYS = ['ca', 'co', 'fw', 'hc', 'kf', 'kn', 'nu'];` (decision 1).
  - `class LocaleSlots extends ReturnOverride`, with the private fields `#locale`, `#calendar`, `#collation`, `#firstDayOfWeek`, `#hourCycle`, `#caseFirst`, `#numeric` and `#numberingSystem`. It has:
    - a constructor `(o, record)` that sets them;
    - `static is(o)`, which is false for non-objects and otherwise `#locale in o`;
    - `static get(o, name)` for the getters.
  - `function isLocaleObject(o)` and `function localeObjectTag(o)`, used by `CanonicalizeLocaleList`.
  - `function Locale(tag, options)`: the constructor. It is a plain function, so Hermes creates `this` from `new.target`.
  - `function UpdateLanguageId(tag, options)` and `function MakeLocaleRecord(tag, options, localeExtensionKeys)`.
  - `function installLocale(intl)`.
  - Task 6 adds the information methods to the same prototype literal.

- [ ] **Step 1: Write the failing lit tests** (real plugin, `RUN: %hermes %s | %FileCheck --match-full-lines %s`).
  - `shapes-table.js`: add the rows and two new kinds, with their checks.
    - `['Intl', 'Locale', 'constructor', 'Locale', 1]` checks:
      - the holder property is writable, non-enumerable and configurable;
      - the function is constructible, has its `name` and `length`, and prints `[native code]`;
      - `prototype` is non-writable, non-enumerable and non-configurable;
      - `prototype.constructor` is the function.
    - `['Intl.Locale.prototype', 'baseName', 'getter', 'get baseName']` checks:
      - the property is a non-enumerable, configurable accessor with no setter;
      - the getter's `name` is the value given and its `length` is 0;
      - the getter is not constructible and prints `[native code]`.
    - Getter rows: `baseName`, `calendar`, `caseFirst`, `collation`, `firstDayOfWeek`, `hourCycle`, `language`, `numberingSystem`, `numeric`, `region`, `script`, `variants`.
    - Method rows, each of length 0: `maximize`, `minimize`, `toString`.
    - `['Intl.Locale.prototype', '@@toStringTag', 'toStringTag', 'Intl.Locale']`.
    - The own-keys line of `Intl` becomes `getCanonicalLocales,supportedValuesOf,Locale,Symbol(Symbol.toStringTag)`. Add an own-keys line for `Intl.Locale.prototype` that lists exactly the table's keys and `constructor`.
  - `locale-objects.js` (Review Focus 2):
    - `Object.create(Intl.Locale.prototype)`: `.baseName` throws `TypeError`, and `Intl.getCanonicalLocales([it])` throws `TypeError`, because `toString` reaches the brand check;
    - the same kind of object with an own `toString` returning `'de'`: `Intl.getCanonicalLocales([it])` gives `["de"]`, because it is a plain object, converted with `ToString`;
    - a `Proxy` of `new Intl.Locale('en')`: `.language` throws `TypeError`;
    - `class L extends Intl.Locale {}`: `new L('en-us')` gives `.baseName` `en-US`, and `getCanonicalLocales([it])` gives `["en-US"]`;
    - a Locale with an own `toString` that throws: `getCanonicalLocales([it, 'en-US'])` gives `["en-US"]` without calling it;
    - `[new Intl.Locale('en-us'), 'EN-US']` dedupes to `["en-US"]`;
    - `Intl.Locale('en')` without `new` throws `TypeError`.
  - `locale-edge.js` (Review Focus 3 and 5): each `new Intl.Locale('en', {...})` with these options throws `RangeError`:
    - `calendar: '\uD800'`;
    - `calendar: 'ſtr'`;
    - `calendar` of 10,000 characters that is not a valid type (`'abcdefghi'` repeated);
    - `language: 'root'`;
    - `region: '1234'`;
    - `variants: ''`, `variants: 'fonipa-'` and `variants: 'fonipa-fonipa'`.

    A 10,000-character valid `calendar` (`'abc-'` repeated, ending in `abc`) succeeds and round-trips through `.calendar`. These `maximize()` / `minimize()` strings match:
    - `sl-rozaj-biske-1994` → `sl-Latn-SI-1994-biske-rozaj` / `sl-1994-biske-rozaj`;
    - `en-t-ja-x-a` → `en-Latn-US-t-ja-x-a` / `en-t-ja-x-a`;
    - `posix-u-ca-buddhist` → itself / itself.
  - `tamper.js`: extend the existing test, keeping its rule that nothing calls `Intl` before the tampering. Add:
    - accessors on `Object.prototype` named `locale`, `calendar`, `ca`, `key`, `value`, `base` and `extension`;
    - a `"0"` setter on `Array.prototype`;
    - replacements of `String.prototype.split`, `Array.prototype.sort`, `Intl.Locale.prototype.toString` and `Intl.Locale.prototype.maximize`.

    Before tampering, capture `Reflect.apply` and the original `toString`, `maximize` and `minimize` of `Intl.Locale.prototype`, and call them only through the captured `Reflect.apply`: the test's own call of a replaced method would reach the replacement. After tampering, also replace the global `Intl.Locale` with a function that throws. Then check, through the captured methods:
    - `new Intl.Locale('en-latn-us', {region: 'GB', calendar: 'gregory'})` (constructed through a reference to `Intl.Locale` captured before the tampering) gives `"en-Latn-GB-u-ca-gregory"`;
    - its captured `maximize`, then `toString`, gives `"en-Latn-GB-u-ca-gregory"`, and its `minimize` gives `"en-GB-u-ca-gregory"`;
    - `Intl.getCanonicalLocales([locale])` is `["en-Latn-GB-u-ca-gregory"]`, without calling the replaced `toString`.

- [ ] **Step 2: Run them and see them fail.** Run `LIT_FILTER='shapes-table|locale-|tamper' ... check-intl-lit`. Expected: FAIL, because `Intl.Locale` is undefined.

- [ ] **Step 3: Implement `20-Locale.js`.**
  - Follow the draft's "Intl.Locale ( tag [ , options ] )", UpdateLanguageId and MakeLocaleRecord step by step:
    - throw `TypeError` when `new.target` is `undefined`;
    - throw `TypeError` for a `tag` that is neither a string nor an object;
    - take the tag from the slot for a Locale object, and otherwise use `ToString`;
    - call `CoerceOptionsToObject` before the well-formedness check;
    - an ill-formed tag, or a value that fails its grammar check, throws `RangeError`;
    - `firstDayOfWeek` goes through `WeekdayToUValue` and then the `type` check;
    - `numeric` is read as a boolean and then `ToString`.
  - Attach the slots with `new LocaleSlots(this, record)`.
  - The getters and `toString` call `LocaleSlots.get(this, ...)`, which throws `TypeError` on a failed brand check.
  - `maximize` and `minimize` call `native.addLikelySubtags` or `native.removeLikelySubtags` on `[[Locale]]`, then return `new Locale(result)` through the local binding, not `Intl.Locale`.
  - `installLocale(intl)`:
    - builds the prototype with `defineMethods` and `defineGetters` over object literals;
    - calls `defineConstructor(Locale, 1, proto)` and `setToStringTag(proto, 'Intl.Locale')`;
    - defines `intl.Locale` writable, non-enumerable and configurable.

- [ ] **Step 4: Run them and see them pass.** Run `check-intl-lit`.

- [ ] **Step 5: Bring test262 up to date.**
  - Run `check-intl-test262`. It fails only with unexpected passes.
  - Update the expectations (Global Constraints). The removed lines must be:
    - every `intl402/Locale/` test that does not use an information method (the `firstDayOfWeek` option and getter are this task's);
    - `intl402/Intl/getCanonicalLocales/Locale-object.js`;
    - the three `intl402/Intl/supportedValuesOf/` tests marked Phase 1.
  - Any other `intl402/Locale/` entry that remains must use `getCalendars`, `getCollations`, `getHourCycles`, `getNumberingSystems`, `getTimeZones`, `getTextInfo` or `getWeekInfo`, or be a defect this task must fix now.
  - Rerun `check-intl-test262`. Expected: `0 problem(s)`.

- [ ] **Step 6: Prove the tests can fail.**
  - Make `isLocaleObject` check `o instanceof Locale`, and `locale-objects.js` fails on the object with its own `toString`.
  - Read `options.calendar` before `CoerceOptionsToObject`, and test262 `constructor-getter-order.js` fails.
  - Call `Intl.Locale` instead of the local `Locale` in `maximize`, and `tamper.js` fails.

  Revert each mutation.

- [ ] **Step 7: Commit.**

### Task 6: The locale information methods

**Files:**
- Modify: `intl/js/20-Locale.js`, `intl/test/lit/shapes-table.js`, `intl/test/lit/tamper.js`, `intl/test/test262-expectations.txt`
- Create: `intl/test/lit/locale-info.js`, `intl/test/lit/locale-info-fallbacks.js`

**Interfaces:**
- Consumes: the helpers from Tasks 2 and 3, and Task 4's `RegionPreference`, `GetLocaleRegion`, `GetLocaleLanguage`, `GetLocaleScript` and `WeekdayUValueToNumber`.
- Produces: `CalendarsOfLocale(loc)`, `CollationsOfLocale(loc)`, `HourCyclesOfLocale(loc)`, `NumberingSystemsOfLocale(loc)`, `TimeZonesOfLocale(loc)`, `TextDirectionOfLocale(loc)` and `WeekInfoOfLocale(loc)`. Each takes a value that passed the brand check. Also the prototype methods `getCalendars`, `getCollations`, `getHourCycles`, `getNumberingSystems`, `getTimeZones`, `getTextInfo` and `getWeekInfo`, each of length 0.

- [ ] **Step 1: Write the failing lit tests.**
  - `shapes-table.js`: add the seven methods as `method` rows of length 0, and extend the prototype's own-keys line.
  - `locale-info.js`: print `JSON.stringify` of each result.
    - `getCalendars`:
      - `en-US` → `["gregory"]`;
      - `th` → `["buddhist","gregory"]`;
      - `ar-SA` → `["gregory","islamic-umalqura"]`;
      - `fa-IR` → `["persian","gregory","islamic-civil","islamic-tbla"]`;
      - `en-US-u-rg-thzzzz` → `["buddhist","gregory"]`;
      - `ja-u-ca-japanese` → `["japanese"]`.
    - `getHourCycles`:
      - `en-US` → `["h12","h23"]`;
      - `ja-JP` → `["h23","h11","h12"]`;
      - `hi-IN` → `["h12","h23"]`;
      - `en-001` → `["h12","h23"]`;
      - `en-u-hc-h24` → `["h24"]`.
    - `getWeekInfo`:
      - `en-US` → `{"firstDay":7,"weekend":[6,7]}`;
      - `en-GB` → `{"firstDay":1,"weekend":[6,7]}`;
      - `he-IL` → `{"firstDay":7,"weekend":[5,6]}`;
      - `en-US-u-rg-gbzzzz` → `{"firstDay":1,"weekend":[6,7]}`;
      - `en-US-u-fw-mon` → `{"firstDay":1,"weekend":[6,7]}`;
      - `new Intl.Locale('en', {firstDayOfWeek: 0})` → firstDay 7.
    - `getTextInfo().direction`:
      - `en` → `ltr`;
      - `ar` → `rtl`;
      - `az-Arab` → `rtl`.
    - `getTimeZones`:
      - `ja-JP` → `["Asia/Tokyo"]`;
      - `ar-PS` → `["Asia/Gaza","Asia/Hebron"]`;
      - `en` → `undefined`;
      - `en-US` is sorted and contains `America/New_York`.
    - `getNumberingSystems`:
      - `ar-EG` → `["arab"]`;
      - `en-u-nu-thai` → `["thai"]`;
      - `fa` → `["arabext"]`.
    - `getCollations`:
      - `de` → `["emoji","eor","phonebk"]`;
      - `en-u-co-phonebk` → `["phonebk"]`.
    - Every array returned is a fresh array, and every object has `Object.prototype`.
  - `locale-info-fallbacks.js` (Review Focus 4): for each of `und`, `zxx`, `qaa-Qaaa-QM`, `posix`, `en-ZZ`, `en-QO`, `en-AQ`, `es-419`, `en-u-rg-xx`, `en-u-sd-foobarx` and `en-u-fw-xyz`, call all seven methods and print the results. Work out each expected line from the draft's steps and the data (Tasks 2 and 3), and say in a comment which step gives it. None may throw. For example:
    - `zxx` (no likely subtags, so region `001`) gives `getCalendars` `["gregory"]` (the `001` line), `getHourCycles` `["h23","h12"]` (the `001` line), `getNumberingSystems` `["latn"]` (no available prefix), `getCollations` `["emoji","eor"]` (no available prefix), `getTimeZones` `undefined`, `getWeekInfo` the `001` data, and `getTextInfo` `{direction: undefined}` (own key present);
    - `en-AQ` gives `getHourCycles` `["h23"]`: neither `en-AQ` nor `AQ` has time data, so the last-resort fallback applies;
    - `en-AQ` gives `getTimeZones` the sorted `AQ` zones;
    - `es-419` gives `[]`.
  - `tamper.js`: after the tampering, through the constructor and `Reflect.apply` captured before it (Task 5), `getCalendars` of `th` is `["buddhist","gregory"]` and the `weekend` of `getWeekInfo` of `en-US` is `[6,7]`.

  Take the expected values from Tasks 2 and 3's tests, which were checked against CLDR 48.2. Where this list and those tests disagree, those tests win.

- [ ] **Step 2: Run them and see them fail.**

- [ ] **Step 3: Implement the seven operations and methods,** step by step from the draft.
  - The JS does the region preference and the loops; native code only answers data lookups.
  - Parse native lists with `splitSpaces` from `10-Intl.js`.
  - CalendarsOfLocale filters through `CanonicalizeUValue('ca', ...)` and `AvailableCalendars()`. The latter is the native `supportedValues('calendar')` list, aliases included.
  - The `getTextInfo` and `getWeekInfo` result objects are made with `{}` (the intrinsic `Object.prototype`), with properties set by `ObjectDefineProperty` as data properties that are writable, enumerable and configurable.

- [ ] **Step 4: Run them and see them pass.**

- [ ] **Step 5: Bring test262 up to date,** as in Task 5. Afterwards, no `intl402/Locale/` entry and no other `Phase 1` entry remains, and `check-intl-test262` reports `0 problem(s)`.

- [ ] **Step 6: Prove the tests can fail.**
  - Skip the region override in CalendarsOfLocale, and `locale-info.js` fails on `en-US-u-rg-thzzzz`.
  - Return `[]` instead of `["gregory"]` as CalendarsOfLocale's last-resort fallback, and `locale-info-fallbacks.js` fails on `en-AQ` (neither `en-AQ` nor `AQ` has calendar-preference data; `zxx` gets `gregory` from the `001` line).
  - Sort the weekend descending, and `locale-info.js` fails on `he-IL`.

  Revert each mutation.

- [ ] **Step 7: Commit.**

### Task 7: Acceptance, spec and README

**Files:**
- Modify:
  - `doc/superpowers/specs/2026-10-22-intl-plugin-design.md`:
    - "Data": a paragraph on the locale-information sources, from decisions 2 and 3;
    - "Phasing", item 1: list the information methods;
    - "The native interface": decision 4;
  - `intl/README.md`: the layout table names `05-tags.js`, `20-Locale.js`, `info.rs` and the three new data files; "Updating the CLDR lists" names all generated files.

- [ ] **Step 1: Run everything.** Run `check-intl`, then a full `check-hermes`. Expected:
  - every part of `check-intl` passes, with test262 at `0 problem(s)`;
  - `check-hermes` has 0 unexpected results.

  Record the counts in the report.

- [ ] **Step 2: Check the expectations file.**
  - `grep -c 'intl402/Locale/' intl/test/test262-expectations.txt` is 0.
  - `grep -c 'Phase 1' intl/test/test262-expectations.txt` is 0.
  - `git diff 02ac812ac -- intl/test/test262-expectations.txt` only removes lines.

- [ ] **Step 3: Update the spec and README** as listed above, in the documents' existing style. Write in plain English, with no jargon of our own.

- [ ] **Step 4: Commit.**
