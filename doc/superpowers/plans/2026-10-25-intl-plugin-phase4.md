# Intl Plugin Phase 4 (DateTimeFormat) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `Intl.DateTimeFormat` to the plugin with its whole public surface, and replace `Date.prototype.toLocaleString`, `toLocaleDateString` and `toLocaleTimeString` with their ECMA-402 versions:
- `Intl.DateTimeFormat`: the constructor and every option, legacy constructor behavior, `supportedLocalesOf`, `resolvedOptions`, the `format` getter and its bound function, `formatToParts`, `formatRange` and `formatRangeToParts`;
- every calendar of `Intl.supportedValuesOf("calendar")`, every numbering system NumberFormat supports, every time zone of `Intl.supportedValuesOf("timeZone")` and every offset time zone, over the whole ECMAScript time range (±8.64 × 10^15 ms);
- the time zone rules bundled in the library (IANA tzdb 2026e), so offsets are the same on every platform.

At the end, every test262 test the expectations file assigns to Phase 4 passes, except the four listed in decision 10.

**Architecture:**
- Formatting is JS, written step by step from the draft, as NumberFormat's is: FormatDateTimePattern, PartitionDateTimePattern, PartitionDateTimeRangePattern, BasicFormatMatcher, the best-fit matcher and DateTimeStyleFormat. Numbers in a date go through the internal NumberFormat, so every numbering system works.
- Native code supplies what needs data or calendar arithmetic: the CLDR patterns, from a new generated file; the calendar fields and the names of an instant (ICU4X's calendars and names); UTC offsets (a tzdb reader over the `jiff-tzdb` crate's data); and time zone names (ICU4X).
- Each DateTimeFormat attaches a `DateTimeFormatState` (`jsi::NativeState`) that holds its calendar, its time zone rules and the ICU4X names its patterns need.
- New JS files: `intl/js/50-DateTimeFormat.js` (the constructor and the prototype), `51-datetime-patterns.js` (pattern data, records, the matchers, range patterns), `52-datetime-format.js` (ToLocalTime and the formatting operations). The Date overrides go in `80-builtins.js`.

**Tech Stack:** Rust 1.88 with ICU4X 2.3 (`icu` with `compiled_data` and `sync`; `icu::datetime`, `icu::calendar`, `icu::time`) and `jiff-tzdb` 0.1.9; C++17 and JSI; Python 3 for the data generator; the plugin's JS compiled to Hermes bytecode; test262 at `c9aa6126e8f6fe2c6f668c65bfba771ffc8a6096`.

**Spec:** `doc/superpowers/specs/2026-10-22-intl-plugin-design.md`. Read "Guiding rules", "The JS layer", "The native interface" (including "Threads", "Strings" and "Parts"), "Data", "Testing" and "Phasing" before any task. Task 10 writes this plan's decisions into it.

**ECMA-402 text:** the https://tc39.es/ecma402/ draft dated October 24, 2026, which is tc39/ecma402 commit `3691634ebc` (still the latest on 2026-10-25). The relevant files are:
- `spec/datetimeformat.html`: the constructor, CreateDateTimeFormat, FormatOffsetTimeZoneIdentifier, the internal slots and locale data constraints, the record types, every abstract operation from DateTimeStyleFormat to UnwrapDateTimeFormat;
- `spec/locales-currencies-tz.html`: "Use of the IANA Time Zone Database", AvailableNamedTimeZoneIdentifiers, GetAvailableNamedTimeZoneIdentifier;
- `spec/locale-sensitive-functions.html`: the three Date overrides;
- `spec/negotiation.html`: ResolveOptions (already implemented).

Cite its section titles and number its steps. Fetch the text with `gh api -H 'Accept: application/vnd.github.raw' 'repos/tc39/ecma402/contents/spec/<file>?ref=3691634ebc'`. IsTimeZoneOffsetString, ParseTimeZoneOffsetString, the `UTCOffset` grammar, TimeClip, WeekDay and the other date functions are in ECMA-262; fetch `spec.html` from `tc39/ecma262` `main` the same way.

**Reference output:** `node` (Node 24, ICU 78.2) is on PATH. ICU 78.2 ships CLDR 48.0 and the plugin uses 48.2, so a string may differ for that reason alone; every other difference from Node is either one of the decisions below or a bug. Expected strings in this plan were taken from Node; where the plugin's output differs, the implementer checks CLDR 48.2, and reports the difference with its cause.

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
  - no line may be added and no reason changed, except where this plan says so. If a task finds a test it cannot make pass, or a test that newly fails, it stops and reports it with the failure output; the controller rules on it.
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
  - an optional parameter is a default parameter (`toLocaleString(locales = undefined, options = undefined)`), which keeps `length` at the spec value. Never read `arguments[i]` past `arguments.length`.
  - The file headers name the draft of October 24, 2026.
- **Native helpers:** `intl/README.md`, "Adding a native helper". JS validates every argument before calling a helper. A helper that gets an argument JS should never send throws `std::logic_error("Intl internal error: ...")`.
- **Rust:**
  - FFI functions never panic on bad input. Invalid UTF-8, an unknown calendar or symbol, a tag that does not parse, a zone without data or a time value outside ±8.64 × 10^15 returns `false` or a null pointer.
  - A panic is only for a broken invariant in our own data files.
  - Every new public function has a doc comment and a unit test.
  - Run `cargo fmt` in `intl/core`, and `clang-format` on changed C++.
- **Threads** (spec, "Threads"): a Rust type held in a `NativeState` must be `Send`, checked at compile time. A `NativeState` destructor frees only its own Rust object. The parsed data files and tzdb zones are process-wide immutable tables built once behind `std::sync::OnceLock`.
- **Prove checks can fail:** for every new test, make a stated mutation of the code under test, name the test that fails, then revert the mutation. Put this evidence in the task report.
- **Never bump `BYTECODE_VERSION`**, and never bump the plugin-interface version. Phase 4 adds native helpers and changes no entry-point, config or `IHermesBuiltinBytecode` interface.

## Planning decisions

Task 10 writes these into the spec. Implement them as stated.

1. **Formatting is JS; native code supplies data, calendar fields and names.** FormatDateTimePattern, PartitionDateTimePattern, PartitionDateTimeRangePattern, FormatDateTime(Range)(ToParts), BasicFormatMatcher, the best-fit matcher (decision 6), DateTimeStyleFormat and the range patterns (decision 11) are JS, step by step from the draft where it has steps. This changes three statements of the spec: option-bag matching and `h24` are JS, not gaps the Rust core fills (`h24` is the draft's own step in FormatDateTimePattern), and DateTimeFormat builds its parts in JS, as NumberFormat does, so it needs no native `...ToParts` helper.
2. **Time zone rules.** The tzdb is the `jiff-tzdb` crate, pinned as `jiff-tzdb = "=0.1.9"`: IANA tzdb **2026e**, compiled by `zic` in the "rearguard" format (so Europe/Dublin's daylight saving time is in summer) and the "slim" layout (explicit transitions, then a POSIX TZ string for the rules after them), 207 KB, MIT or Unlicense. The plugin reads it with its own reader, `intl/core/src/tz.rs`, not with `jiff`, because `jiff`'s timestamps stop at years ±9999 and ECMAScript's run to ±275,760:
   - a zone is looked up by its primary identifier with `jiff_tzdb::get`, and its TZif data (version 2 or later; the 64-bit section and the footer) is parsed once per process, behind a `OnceLock`;
   - an instant before the first transition has the zone's first local time type (RFC 8536: local mean time, with its seconds, `America/New_York` 1800 is −4:56:02); one after the last transition has the offset the footer's POSIX TZ string gives, computed for that year with exact integer day arithmetic, which works for any year;
   - [[InDST]] is the `isdst` flag of the local time type (or, after the transitions, whether the POSIX rule is in its daylight part);
   - the offset is in seconds; the draft's GetNamedTimeZoneOffsetNanoseconds is that × 10^9.

   ICU4X's own zone list (CLDR 48.2) decides which identifiers exist (`Intl.supportedValuesOf("timeZone")`, Phase 0). A Rust test checks that every one of its primary identifiers has tzdb data.
3. **ToLocalTime.** Native code computes the record for an instant: the offset (decision 2, or the fixed offset of an offset time zone), then the local time `epochMs + offset`, split into days and milliseconds of the day with floor division, and the date of that day in the formatter's calendar with ICU4X's `Date::from_rata_die` (the day `d` since 1970-01-01 is Rata Die `d + 719163`; ICU4X's range, years ±999,999, covers ECMAScript's). The fields:
   - [[Weekday]] is ECMA-262's WeekDay, 0 for Sunday;
   - [[Year]] is the calendar's era year for a calendar with eras (ICU4X `YearInfo::Era`'s `year`), the cyclic year 1 to 60 for `chinese` and `dangi` (`YearInfo::Cyclic`'s `year`), and the ISO year for `iso8601`. The draft's "if the value is ≤ 0, set it to 1 − value" step then applies to the displayed year, as written. [[Era]] is ICU4X's era code ("bce", "ce", "reiwa", ...), "" for `chinese`, `dangi` and `iso8601`;
   - [[RelatedYear]] is the related ISO year for `chinese` and `dangi` (`YearInfo::Cyclic`'s `related_iso`), else undefined; [[YearName]] is the cyclic year's name;
   - [[Month]] is zero-based, as the draft's Gregorian table has it (MonthFromTime), because FormatDateTimePattern adds 1 before showing it. Native code returns the 1-based number shown: for `chinese` and `dangi` the number of the month code (`"M04L"` is 4) with a leap flag, for every other calendar the ordinal month; JS subtracts 1. The leap flag makes a numeric month take the locale's numeric leap pattern (`zh` `"闰{0}"`, `en` `"{0}bis"`, decision 5); text months come from ICU4X with the leap month already named;
   - [[Day]], [[Hour]], [[Minute]], [[Second]], [[Millisecond]] from the local time; [[InDST]] from decision 2;
   - `iso8601` is computed as `gregory`, with no era and the ISO year, so year 0 shows as `"1"` and −1 as `"2"`, as ICU4C shows them. Its `{era}` placeholder gives no part (ICU4C prints none).

   PartitionDateTimeRangePattern compares fields with two refinements, both because the draft's fields repeat where the dates differ: [[Month]] is compared by month code, so a leap month and its base month differ; and [[Year]] of `chinese` and `dangi` is compared by [[RelatedYear]], because the cyclic year repeats every 60 years (1984-02-02 and 2044-01-30 are both the first day of a 甲子 year; ICU4C gives `"1984甲子年–2044甲子年"`).
4. **Names.** Month, weekday, era, cyclic year and AM/PM names, and time zone names, come from ICU4X 2.3, through its public `FixedCalendarDateTimeNames<C, CompositeDateTimeFieldSet>` (one per CLDR symbol the formatter's patterns use, loaded with `include_for_pattern` for a one-field pattern and formatted with `with_pattern_unchecked`), with `C` the ICU4X calendar type of the formatter's calendar (a Rust enum over ICU4X's formattable calendars; `iso8601` uses `Gregorian`'s names). So ICU4X's handling of leap months (Hebrew Adar I, Chinese leap months), era codes and cyclic years is the plugin's. The input of each is the instant's `ZonedDateTime`, whose `TimeZoneInfo` has the zone's ICU4X id (from `IanaParser`, `TimeZone::UNKNOWN` for an offset time zone), the offset of decision 2, and the instant as its zone name timestamp.
   - Only the symbols a calendar has are valid: `U` only for `chinese` and `dangi`, `G` for every calendar but `chinese`, `dangi` and `iso8601`. Any other pair is an internal error, and so is any error ICU4X's formatting returns (its fallback text is never used).
   - ICU4X chooses a specific name (`z`, `zzzz`) by comparing the offset with CLDR's standard and daylight offsets of the zone's metazone at that time; it ignores any variant it is given (`set_time_zone_variant` is a no-op in 2.3). So [[InDST]] from the tzdb does not choose the name, and where the tzdb's offset matches neither of CLDR's (local mean time, or a rule change newer than CLDR 48.2) the name falls back to the localized GMT format (`America/New_York` 1800 `z` is `"GMT-4:56:02"`), as ICU4C's does.
   - Month and weekday names use the context of the CLDR pattern letter they come from: `M`, `E` format; `L`, `c` stand-alone.
   - Time zone names are formatted with Latin digits (the preferences' numbering system is `latn`), and JS transliterates the ASCII digits of the `timeZoneName` part into [[NumberingSystem]]'s digits with Phase 3's digit table, which gives ICU4C's `"GMT+๕:๓๐"` for `en-u-nu-thai` and `"غرينتش+٥:٣٠"` for `ar-EG`.
   - The draft's `dayPeriod` is CLDR's flexible day period (`B`), which ICU4X does not have. Its rules and names come from `intl/core/data/day-periods.txt` (decision 5), and JS chooses the period: the rule whose range holds the minute of the day; "noon" only when the hour is 12 and the formatter shows no non-zero minute or second (`"12 noon"`, `"12:30 in the afternoon"`); "midnight" never, as ICU4C (in `en`, 0:00 is `"in the morning"`, CLDR 48.2's `morning1` from 0:00 to 12:00, which test262's `dayPeriod-*-en.js` accept).
5. **Pattern data.** ICU4X 2.3 has no CLDR `availableFormats`, `intervalFormats` or flexible day periods, so `update-cldr-lists.py` generates two files from CLDR 48.2:
   - `intl/core/data/date-patterns.txt`: for each locale of `locales.txt` (and `und`) and each calendar, the `dateFormats`, `timeFormats`, `dateTimeFormats` (standard and `atTime`), `availableFormats`, `intervalFormats` with their `intervalFormatFallback`, the `Timezone` append item, and for `chinese` and `dangi` the numeric leap month pattern (`monthPatterns` numeric `leap`). Sources: `cldr-dates-full` (`ca-gregorian.json`, `ca-generic.json`) and the `cldr-cal-*-full` packages (npm version 48.2.0), and `common/main/root.xml` for `iso8601`, which only root defines in CLDR 48.2 (the generator checks that no other locale's XML on the chains of `locales.txt` defines it). Alternates (`-alt-ascii`, `-alt-variant`) and count variants (`-count-`) are left out. An entry whose skeleton has a field ECMA-402 does not have (`Q`, `w`, `W`, `D`, `F`, `g`, `e`, `u`, `Y`, `x`, `X`, `Z`, `V`) is left out; any such letter in a kept pattern fails generation, naming the locale, calendar and pattern, except two that CLDR 48.2 writes for the letters meant (12 patterns, among them `gl`'s `dateFormats` full): `Y` (week-based year) is written as `y`, and `e` with 3 to 6 letters (the day of the week) as `E`. A pattern's `_numbers` override (`he` `hebrew` `hebr`, `ja` `japanese` `y=jpanyear`, `chinese` and `dangi` `hanidec` and `d=hanidays` in `ja`, `zh` and `yue`) is left out (decision 17). The format is `units.txt`'s: a string table, `locale <tag> <parent>` lines, and after each `<calendar> <key>=<index>...` lines with what differs from the parent, or `<calendar> = <other calendar>` when the locale's whole data for the calendar equals its data for another calendar listed before it (5,761 of 8,080 locale-calendar pairs). The generator checks that every pair reads back to its CLDR data, and that a child's keys include its parent's. About 46,000 entries and 6,000 strings; record the file's size.
   - `intl/core/data/day-periods.txt`: CLDR's `dayPeriodRuleSet` (supplemental `dayPeriods.json`) per language, and each locale's format-context `dayPeriods` names (abbreviated, wide, narrow) for the periods its rules use, deduplicated against the parent chain.
6. **[[formats]] and the matchers.**
   - **Records.** A CLDR pattern becomes a DateTime Format Record: a pattern string with the draft's placeholders and a width for each field, by this table (`n` letters):

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

     Quoted text and other characters are literal. In `chinese` and `dangi`, `y` is `{relatedYear}`, as in ICU4X and ICU4C (`en` `"M/d/y"` is `"1/1/1984"` with a `relatedYear` part). In `chinese`, `dangi` and `iso8601`, which have no eras, `{era}` gives no part and asks native code for no name. A record keeps, besides the draft's fields, the stand-alone flags of its month and weekday and its AM/PM width. A `B` the request did not ask for (a locale whose 12-hour patterns use `B`) formats as the day period but adds no `dayPeriod` field, so `resolvedOptions` does not report it (`zh-TW`).
   - **Hour-cycle families.** [[HourCycle]] is known before matching (CreateDateTimeFormat step 13), so matching uses the records of its family: `h11`/`h12` take the entries whose pattern has `h` or `K` (and those with `B`); `h23`/`h24` those with `H` or `k`; both take the entries without an hour. A record's [[pattern]] and [[pattern12]] are the same string. The draft keys [[formats]] by calendar only; the plugin keys it by calendar and hour-cycle family, which is how the 12- and 24-hour widths can differ (`en` `"9:07 AM"`, `h23` `"09:07"`).
   - **[[formats]]** for a locale, calendar and family is: the records of `availableFormats`; a one-field record for each field that has no one-field entry (`{minute}` numeric, `{second}`, `{timeZoneName}`, `{dayPeriod}`, `{era}`...), and `{fractionalSecondDigits}` with each width 1, 2 and 3; for every record with `{second}` and no fraction, three more with `{fractionalSecondDigits}` of width 1, 2 and 3 after `{second}` (decimal separator as in best fit's step 4); and every date record glued to every time record (below). `formatMatcher: "basic"` runs the draft's BasicFormatMatcher over it. The draft requires its 14 field sets in the calendar's list, which here is the union of the two families: CLDR's patterns with an hour and a day period are all 12-hour, so the 24-hour family has no record with both, and a 24-hour request with `dayPeriod` and an hour drops the day period, as ICU4C does (`en` `{dayPeriod: "short", hour: "numeric", hourCycle: "h23"}` is `"09"`, with no `dayPeriod` in `resolvedOptions`). The one-field `{dayPeriod}` record is in both families, so `{dayPeriod: "short", hourCycle: "h23"}` is `"in the morning"` and reports `dayPeriod`. Tests check that the union has a record for each of the 14 field sets: Task 5's for the fixture, Task 7's through `formatMatcher: "basic"` and `resolvedOptions` for 40 locales and every calendar. The fraction's decimal separator depends on [[NumberingSystem]], so the records are cached by locale, calendar, family and that separator (decision 13).
   - **Best fit** (the default) is the plugin's BestFitFormatMatcher:
     1. score the `availableFormats` and one-field records with BasicFormatMatcher's penalties; ties go, in `chinese` and `dangi`, to a record with `{relatedYear}` (`en` has `UMd` `"M/d/U"` before `yyyyMd` `"M/d/r"`, and ICU4C shows `"1/1/1984"`), then to the first in CLDR's order;
     2. if the best record has every requested field, return it, with widths adjusted;
     3. else, if the request has both date fields (`weekday`, `era`, `year`, `month`, `day`) and time fields (`dayPeriod`, `hour`, `minute`, `second`, `fractionalSecondDigits`, `timeZoneName`), match each half by this algorithm and glue them;
     4. else append the missing fields: `fractionalSecondDigits` goes right after `{second}`, preceded by the locale's decimal separator for [[NumberingSystem]] (`de` `"3,045"`); `timeZoneName` by the `Timezone` append item (`"{0} {1}"`, `"9 AM UTC"`).

     A request with `fractionalSecondDigits` and an hour or minute is matched as if it also asked for `second: "numeric"`, as ICU4C does (`{minute, fractionalSecondDigits: 2}` gives `"07:03.04"`).
   - **Gluing.** The date record's pattern replaces `{1}` and the time record's `{0}` in the `atTime` dateTimeFormat (the standard one where the locale has no `atTime`), chosen by UTS 35's rule: `full` if the date has a long month and a weekday, `long` if it has a long month, `medium` if a short month, else `short` (`"January at 7"`, `"Monday, January 5, 2026 at 3:07 PM"`, `"Jan 5, 2026, 9:07 AM"`).
   - **Width adjustment** (best fit only), for each field both the request and the record have: both numeric (`numeric`, `2-digit`): `"2-digit"` if either is, else `"numeric"` (`hh` for a 2-digit hour, `HH:mm` stays 2-digit, minutes stay `mm`); both text: the requested width (`MMM` becomes `MMMM`); `timeZoneName` of the same kind (specific, offset, generic): the requested one; `fractionalSecondDigits`: the requested number; otherwise the record's. `resolvedOptions` reports the adjusted widths, as V8 does (`minute: "2-digit"`).
7. **[[styles]].** The DateTime Styles Record's [[Date]] and [[Time]] records come from `dateFormats` and `timeFormats`, [[Connector]] from the `atTime` dateTimeFormats (Node: `"Monday, January 5, 2026 at 3:07 PM"`), and [[DateTimeRangeFormat]] from decision 11. A time style whose hour letter is of the other family than [[HourCycle]] is replaced by the best-fit record for its fields and widths in [[HourCycle]]'s family (`en` `timeStyle: "short"` with `h23` is `"09:07"`, `en-GB` with `hour12: true` is `"09:07 am"`).
8. **Hour cycles.** `[[LocaleData]].[[L]].[[hc]]` is « null, "h11", "h12", "h23", "h24" ». For the locale L that ResolveLocale finds, let `list` be its hour cycles as `Intl.Locale.prototype.getHourCycles` computes them (Phase 1's HourCyclesOfLocale on L, which has no `-u-hc`): [[hourCycle]] is `list[0]`, [[hourCycle12]] the first `h11` or `h12` in `list` (else `"h12"`), and [[hourCycle24]] the first `h23` or `h24` (else `"h23"`). So `ja` with `hour12: true` is `h11` (CLDR's JP allows `H K h`), as test262 expects. 20-Locale.js's HourCyclesOfLocale and CalendarsOfLocale are split into a function of a tag that the Locale methods and DateTimeFormat share.
9. **Calendars and numbering systems.** `[[LocaleData]].[[L]].[[ca]]` is the first calendar of L's CalendarsOfLocale that `Intl.supportedValuesOf("calendar")` lists (`th` `buddhist`, `fa` `persian`; CLDR 48.2 gives `ar-SA` `gregory` first, as Node does), then the other supported calendars in that list's order. `"islamic"` and `"islamic-rgsa"`, which ICU4X does not implement, are not in it, so a request for them resolves to the locale's default calendar, which test262 accepts. `[[nu]]` is NumberFormat's list (Phase 3, decision 17).
10. **Time zone identifiers.** The draft's CreateDateTimeFormat stores [[PrimaryIdentifier]] (`Asia/Calcutta` is `Asia/Kolkata`, `Etc/UTC` is `UTC`), and so does the plugin. test262's `canonical-tz` tests come from Temporal's newer rule, which keeps the identifier as given; these four stay in the expectations file with the reason `the draft of October 24, 2026 stores the primary time zone identifier; test262's canonical-tz feature (Temporal) keeps the identifier as given`:
    - `intl402/DateTimeFormat/timezone-not-canonicalized.js`
    - `intl402/DateTimeFormat/canonicalize-utc-timezone.js`
    - `intl402/DateTimeFormat/timezone-case-insensitive.js`
    - `intl402/DateTimeFormat/canonicalize-timezone.js` (also a `canonical-tz` test at the pinned commit: it expects `Australia/Canberra` kept as given) An offset time zone is parsed with ECMA-262's `UTCOffset` grammar (ASCII signs only; `+05:30`, `+0530`, `+05`), seconds are a `RangeError` as the draft says, and [[TimeZone]] is FormatOffsetTimeZoneIdentifier's `"+05:30"`; its names are ICU4X's localized GMT format (`"GMT-3:30"`, `"GMT-03:30"`).
11. **Range patterns.** For a record R with date fields D and time fields T, and its pattern P, let FB(p) be the parts « startRange p, shared separator, endRange p », with the separator the text between `{0}` and `{1}` of `intervalFormatFallback` (text around them is shared). An `intervalFormats` entry is used for a part of R only if its skeleton has the same fields as that part (widths aside; `z`, `O` and `v` count as one field); its patterns get R's widths and contexts. A converted interval pattern that does not show the same year placeholders (`{year}`, `{relatedYear}`, `{yearName}`) as R is not used either: `en` `chinese` `y`'s interval `"U – U"` would drop the related years of R's `"r(U)"`, so a year difference takes FB(P), `"1984(jia-zi) – 2044(jia-zi)"`, as ICU4C gives. A CLDR interval pattern is split at the first field that repeats (UTS 35), and the sources follow ICU4C: the fields that appear in both halves are the overlap; in each half, from the first to the last overlap field is `startRange` or `endRange`, and the rest is `shared` (`"Jan 5 – 9, 2026"`: month shared, `"5"` startRange, `" – "` shared, `"9"` endRange, `", 2026"` shared; `"1/3/2019 – 1/5/2019"`: each date whole). The range record, in the draft's field order:
    - [[Default]]: FB(P) if D is not empty; else FB of the best-fit pattern for `year`, `month`, `day` numeric plus T (`"1/5/2026, 3:07 PM – 1/9/2026, 5:08 PM"`).
    - [[Era]], [[Year]], [[Month]], [[Day]]: if T is empty, CLDR's pattern for `G`, `y`, `M` or `d`; a field R shows without its own pattern takes the nearest larger field's, else FB(P); [[Year]] for an R with a month or day but no year is FB of the best fit for R plus `year: "numeric"` (`"Jan 5, 2026 – Feb 9, 2027"`). If both D and T are non-empty, all four are FB(P). If D is empty, all four are absent, so a different day gives [[Default]].
    - [[AmPm]], [[DayPeriod]], [[Hour]], [[Minute]], [[Second]], [[FractionalSecondDigits]] (T not empty): CLDR's patterns for `a`, `B`, `h`/`H`, `m`; a field R shows without one takes FB(P) (`"9:07:03 AM – 9:07:10 AM"`); a field from [[AmPm]] down that R does not show, but that is larger than the smallest field R shows, takes the pattern of the next smaller field R shows (AM/PM in a 24-hour pattern takes [[Hour]]'s, so `"09:07 – 14:07"`; AM/PM in a pattern with `B` takes [[DayPeriod]]'s, so `"9:07 in the morning – 2:07 in the afternoon"`; [[DayPeriod]] in a pattern without `B` takes [[Hour]]'s, `"1:07 – 6:07 PM"`); fields smaller than the smallest one R shows are absent, so their differences are not shown. A test runs the draft's loop over every pair of hours of a day for `h`, `H`, `Bh`, `hm`, `Hm`, `Bhm` and `hms` records and checks that no two different times collapse and that no same-day pair selects [[Default]]. If D is not empty, each such pattern is the date part (its pattern in a glued record, else the best fit for D), shared, joined to the time parts by the standard (not `atTime`) dateTimeFormat (`"Jan 5, 2026, 9:07 – 10:07 AM"`, `"Monday, 9 – 10 AM"`).

    The draft's loop needs this coverage: once a field has a pattern, a smaller field without one stops the search and its difference is not shown. ICU4C differs in two ways, both documented: it checks no day period difference (`"1:07 – 6:07 in the evening"`, where the plugin gives `"1:07 in the afternoon – 6:07 in the evening"`), and its intervals ignore 2-digit requests (`"1/5/26 – 1/6/26"`, where the plugin gives `"01/05/26 – 01/06/26"`).
12. **Numbers.** FormatDateTimePattern's `nf`, `nf2` and `nf3` are internal NumberFormats made with Phase 3's creation function from null-prototype option records, once per DateTimeFormat on first use; user code never sees them. Two-digit values keep their last two code points, as the draft says.
13. **Caches**, all unobservable, as Phase 3's:
    - `dateTimePatterns` per locale and calendar, and `dayPeriods` per locale, 64 entries each, emptied when full;
    - the records and [[formats]] lists of a locale, calendar, hour-cycle family and fraction decimal separator, 32 entries;
    - a DateTimeFormat keeps the parsed parts of every pattern it has used;
    - the three Date overrides each reuse one default DateTimeFormat, made on first use, when `locales` and `options` are both `undefined`; any other call creates one through the internal creation function, which performs every observable step.
14. **Legacy constructor behavior.** ChainDateTimeFormat and UnwrapDateTimeFormat, with Phase 3's fallback symbol and OrdinaryHasInstance. Only the `format` getter and `resolvedOptions` unwrap.
15. **Date overrides.** The draft's three methods: the this value's [[DateValue]] through the captured `Date.prototype.getTime` (which throws the `TypeError` for a non-Date), `"Invalid Date"` for NaN, then CreateDateTimeFormat with (`any`, `all`), (`date`, `date`) and (`time`, `time`). `test/hermes` tests of Hermes's own Date methods run with `-Xintl-plugin=false`.
16. **Native state.** `DateTimeFormatState` (`DateTimeFormatHelpers.cpp`) owns a `HiDateTimeFormat`: the ICU4X calendar, the zone (parsed tzdb data or a fixed offset, and its ICU4X id) and the names objects of decision 4. It is `Send`. A new argument tag, `Double`, carries the time value: a finite JS number; anything else is an internal error.
17. **Known differences from ICU4C**, besides those above: `iso8601` patterns are CLDR's root ones with Gregorian names (`dateStyle: "long"` is `"2026 January 5"`, where ICU 78.2 prints `"2026  5"`); `japanese` dates before Meiji use ICU4X's `ce`/`bce` eras (ICU4C has every historical era); a year before a calendar's epoch follows the draft's 1 − value step (`fa` year 2 CE is `"624"`, ICU4C `"−623"`); the far future's time zone names follow ICU4X's last metazone; CLDR's numbering overrides of some `dateFormats` (Hebrew letters in `he` `hebrew`, `元年` in `ja` `japanese`, Chinese day names) are not applied, because the draft formats every number with [[NumberingSystem]] (`he-u-ca-hebrew` `dateStyle: "full"` shows the year with [[NumberingSystem]]'s digits, where ICU4C writes `תש״ף`); a `Y` or `e` in CLDR's data is read as `y` or `E`.

## Review Focus

These are the five inputs most likely to hurt someone that the spec implies but test262 does not exercise. Each one has a test in the task named.

1. **Extreme and awkward instants.** `±8.64e15`, `0`, `-1`, `Date.UTC(-271821, 3, 20)`; the offsets of `America/New_York` around 2026-03-08 06:59:59.999Z and 07:00Z (`"1:59 AM"`, `"3:00 AM"`) and 2026-11-01; `Australia/Lord_Howe`'s 30-minute daylight saving time (`"GMT+11:00"` in January, `"GMT+10:30"` in July); `America/New_York` in 1800 (`"GMT-04:56:02"`, local mean time); `America/New_York` in July 275000 (offset −4:00, from the POSIX rule); `Europe/Dublin` summer (`"Irish Standard Time"`). Each formats and formats to parts, the parts join to the formatted string, and no `Exception in HostFunction:` appears. Covered in Task 2 (Rust offsets) and Task 7 (`datetime-format-values.js`).
2. **Calendars.** Each supported calendar, with `en` and its own locale (`th` buddhist, `ja` japanese, `zh` chinese, `ko` dangi, `he` hebrew, `fa` persian, `ar-SA` islamic-umalqura, `am` ethiopic): `chinese` and `dangi` leap months (2020-05-23 is M04L) numeric and text; `hebrew` Adar I and II in 5784 and Adar in 5783; the `japanese` era change on 2019-04-30/05-01; `roc` before 1912; `ethioaa`; `iso8601` year 0. Covered in Task 4 (Rust fields and names) and Task 7 (`datetime-format-calendars.js`).
3. **Tampering after the plugin loads:**
   - accessors on `Object.prototype` for every option name and every internal record field name (`pattern`, `pattern12`, `type`, `value`, `source`, `hour`, `weekday`, ...);
   - a setter for `"0"` on `Array.prototype`;
   - replaced `Intl.DateTimeFormat`, its prototype methods and `format` getter, `Intl.NumberFormat` and its prototype (DateTimeFormat must not use them), `Date.now`, `Date.prototype.getTime`, `Date.prototype.valueOf`, `Number.prototype.toLocaleString`, `Math.floor`.

   `format`, `formatToParts`, `formatRange` and the Date overrides give the same results regardless (a date passed to `format` is still converted with ToNumber, so its own `valueOf` and `@@toPrimitive` are called, as the draft says). Covered in Tasks 6 to 9 (`tamper.js`).
4. **Objects that look like formatters:** `Object.create(Intl.DateTimeFormat.prototype)`, a Proxy around a DateTimeFormat, an `Intl.NumberFormat`, a subclass instance, the legacy wrapper `Intl.DateTimeFormat.call(Object.create(Intl.DateTimeFormat.prototype))`, the detached bound `format`, and the Date overrides called on `{}`, `Date.prototype` and a Proxy of a Date. The `format` getter and `resolvedOptions` accept the wrapper and the subclass; every other method rejects the wrapper with `TypeError`. No `Exception in HostFunction:` ever appears. Covered in Task 6 (`datetime-format-objects.js`) and Task 9 (`to-locale-date-string.js`).
5. **Data for every locale.** For each locale of `locales.txt` and each supported calendar: `dateTimePatterns` succeeds; every pattern converts with decision 6's table; every interval pattern has a field that repeats; `dayPeriods` succeeds and its rules cover all 1,440 minutes of the day exactly once (noon and midnight aside); a `DateTimeFormatState` with every name symbol initializes and gives the local time of three instants. Covered in Task 3 and Task 4 (Rust tests over every locale, which are faster than JS), and Task 7 (`datetime-format-locales.js`: `dateStyle` and `timeStyle` `full` for 40 locales).

---

### Task 1: The CLDR date data files

**Files:**
- Modify: `intl/core/data/update-cldr-lists.py`, `intl/core/data/test_update_cldr_lists.py`
- Create: `intl/core/data/date-patterns.txt`, `intl/core/data/day-periods.txt` (both generated)
- Modify: `intl/README.md` ("Updating the CLDR lists": the two files, the new npm packages and the format of each)

**Interfaces:**
- Produces two files. The generator checks that no value has a tab or a newline.
  - `date-patterns.txt`, in units.txt's three sections (decision 5):
    1. `strings <count>`, then each distinct string on its own line, numbered from 0;
    2. `locale <tag> <parent>` lines (`-` for `und`'s parent), the parent as `units.txt` computes it;
    3. after each `locale` line, `<calendar> <key>=<index>...` lines and `<calendar> = <calendar>` lines. Calendars are BCP 47 ids: `gregory`, `buddhist`, `chinese`, `coptic`, `dangi`, `ethioaa`, `ethiopic`, `hebrew`, `indian`, `islamic-civil`, `islamic-tbla`, `islamic-umalqura`, `iso8601`, `japanese`, `persian`, `roc`, and `generic` (CLDR's, used only as a target of `=`). Keys: `d0`-`d3` dateFormats full to short; `t0`-`t3` timeFormats; `c0`-`c3` the standard dateTimeFormats; `a0`-`a3` the `atTime` ones (absent where CLDR has none); `fb` intervalFormatFallback; `lp` the numeric leap month pattern; `tz` the `Timezone` append item; `f.<skeleton>` an availableFormats entry; `i.<skeleton>.<letter>` an intervalFormats entry.

    Reading `(L, cal)`: if L has a `cal = other` line, it is `(L, other)`; else it is `(parent(L), cal)` with L's own `cal` entries over it; `und` has full data for every calendar.
  - `day-periods.txt`:
    1. `rules <key>` lines, each followed by `<period> <from> <before>` lines in minutes (`morning1 0 720`; 24:00 is 1440; `before` may be smaller than `from` for a period that crosses midnight; `noon 720 720` and `midnight 0 0` for `at` rules). The keys are CLDR's `dayPeriodRuleSet` locales as BCP 47 tags; most are languages, and CLDR 48.2 also has `es-CO` and `hi-Latn`;
    2. `locale <tag> <parent> <rules-key>` lines, each followed by `<period> <abbreviated>\t<wide>\t<narrow>` lines for the names that differ from the parent chain. A period with no name in CLDR has no line, and its name reads as `""`.

    A locale's rules key is found by truncating its tag (ICU's lookup, not the file's parent chain: `zh-Hant` and `zh-HK` use `zh`, not `und`), the first truncation that has a rule set, else `und` (`es-CO` has its own, so midnight is `morning2` `"de la mañana"`, where `es` gives `morning1` `"de la madrugada"`; `en-GB` uses `en`). Where a locale has a rule set but no name for a period (`lij`, `scn`), JS shows the AM/PM name of the hour in the same width, as ICU4C does, so a record with `{dayPeriod}` also asks native code for the `a` names when the locale's list has an empty name.
- File keys are the tags of `locales.txt`, resolved to JSON directories as Phase 3's `resolve_locale_dirs` does.

- [ ] **Step 1: Write the failing generator tests** in `test_update_cldr_lists.py`, on small fixture JSON written by the test (`und` and two locales with a parent; one with a `buddhist` calendar equal to its `generic`, one with its own `buddhist` pattern):
  - `test_date_pattern_keys`: a locale's `dateFormats`, `timeFormats`, both dateTimeFormats, one `availableFormats` and one `intervalFormats` entry come out under the keys above;
  - `test_date_pattern_calendar_alias`: the calendar equal to an earlier one is written `buddhist = generic`, and the one with its own pattern gets entries;
  - `test_date_pattern_dedup`: a child entry equal to the parent's is not written, and one that differs is;
  - `test_date_pattern_reads_back`: reading every pair back gives the fixture's data, and a fixture whose child lacks a parent key fails generation;
  - `test_date_pattern_rejects_symbols`: a kept pattern with `Q`, and a `timeFormats` pattern with `Z`, fail generation with the locale named; an `availableFormats` entry with a `w` skeleton is dropped;
  - `test_alternates_dropped`: `-alt-ascii`, `-alt-variant` and `-count-` entries are not written;
  - `test_iso8601_from_root_xml`: a fixture `root.xml` with an `iso8601` calendar gives `und`'s `iso8601` entries, and a second locale XML with one fails generation;
  - `test_day_period_rules`: `en`'s rules come out in minutes, a period crossing midnight keeps `from > before`, `at` rules give `noon 720 720`, and a fixture with an `es` and an `es_CO` rule set writes both, as `rules es` and `rules es-CO`;
  - `test_day_period_names`: names deduplicated against the parent, three widths per line.
- [ ] **Step 2: Run them to see them fail.** `check-intl-data` fails: the functions do not exist.
- [ ] **Step 3: Implement the generator.** Download `cldr-dates-full` and the eleven `cldr-cal-*-full` packages (`buddhist`, `chinese`, `coptic`, `dangi`, `ethiopic`, `hebrew`, `indian`, `islamic`, `japanese`, `persian`, `roc`) with the existing tarball code; `ethioaa` is `ca-ethiopic-amete-alem.json` and the three Islamic calendars are `ca-islamic-civil.json`, `ca-islamic-tbla.json` and `ca-islamic-umalqura.json`. Rules come from `cldr-core` `supplemental/dayPeriods.json`. `root.xml` and the other XML files come from the existing XML loader.
- [ ] **Step 4: Run the generator** with `python3 intl/core/data/update-cldr-lists.py release-48-2`. Check:
  - the other generated files are unchanged (`git diff --stat`);
  - record both files' sizes, line counts and the number of `=` lines in the report;
  - `en` `gregory` `f.yMMMd` resolves to `"MMM d, y"`, `i.yMMMd.d` to `"MMM d – d, y"` (with U+2009 around the dash), `a0` to `"{1} 'at' {0}"`; `en` `chinese` `lp` is `"{0}bis"`; `und` `iso8601` `d3` is `"y-MM-dd"`;
  - `en` day periods (CLDR 48.2; 47 had other times): `morning1 0 720`, `afternoon1 720 1080`, `evening1 1080 1260`, `night1 1260 1440`, names `"in the morning"` (all three widths) and noon `"noon"`.
- [ ] **Step 5: Run `check-intl-data`.** Expected: pass.
- [ ] **Step 6: Commit** the generator, its tests, the two data files and the README change.

### Task 2: Time zone rules in the Rust core

**Files:**
- Modify: `intl/core/Cargo.toml` (`jiff-tzdb = "=0.1.9"`, with a comment that the version pins tzdb 2026e), `intl/core/Cargo.lock` (only that addition)
- Create: `intl/core/src/tz.rs`
- Modify: `intl/core/src/lib.rs` (the module), `intl/THIRD_PARTY_NOTICES.md` (jiff-tzdb's MIT license and the tzdb's public-domain notice)
- Test: unit tests in `tz.rs`

**Interfaces:**
- Produces, by decision 2:
  - `tz::Zone`: a parsed zone (transition times, local time types with offset in seconds and `isdst`, and the footer rule);
  - `tz::zone(primary_id: &str) -> Option<&'static Zone>`: the zone for an identifier `jiff_tzdb::get` knows, parsed once and kept for the process;
  - `Zone::offset_at(epoch_seconds: i64) -> (i32, bool)`: the UTC offset in seconds and the daylight saving flag at that instant, for any `epoch_seconds` in ±8.64 × 10^12;
  - `tz::TZDB_VERSION: &str` = `"2026e"`, checked against `jiff_tzdb::VERSION` by a test.

- [ ] **Step 1: Write the failing tests** in `tz.rs`:
  - `new_york_transitions`: `America/New_York` at 2026-03-08 06:59:59Z is (−18000, false), at 07:00:00Z (−14400, true); at 2026-11-01 05:59:59Z (−14400, true), 06:00:00Z (−18000, false);
  - `lord_howe_half_hour`: `Australia/Lord_Howe` January 2026 is (39600, true), July (37800, false);
  - `local_mean_time`: `America/New_York` at 1800-01-01T00:00Z is (−17762, false); `Asia/Kolkata` at year 2 is (21208, false);
  - `far_future_rule`: `America/New_York` at July 1 of years 2100, 9999, 10000 and 275000 is (−14400, true), and January 1 is (−18000, false); `Europe/London` July 275000 is (3600, true);
  - `far_past`: `America/New_York` at −8.64 × 10^12 s gives its first local time type;
  - `rearguard_dublin`: `Europe/Dublin` July 2026 is (3600, true) and January (0, false);
  - `fixed_zones`: `UTC` and `Etc/GMT+5` at several instants are (0, false) and (−18000, false);
  - `every_supported_zone_has_data`: every value of `supported::values(Kind::TimeZone)` has a zone, and `offset_at` succeeds at −8.64 × 10^12, 0 and 8.64 × 10^12;
  - `version`: `jiff_tzdb::VERSION == Some(TZDB_VERSION)`;
  - `rejects`: an unknown name gives `None`; a corrupted TZif buffer (truncated, bad magic, a footer that does not parse) is rejected by the parser function without a panic.
- [ ] **Step 2: Run them to see them fail.** `check-intl-rust` fails to compile.
- [ ] **Step 3: Implement `tz.rs`.** Parse RFC 8536 version 2+ data, using the 64-bit section and ignoring the version 1 section. The footer's POSIX TZ string has `std offset [dst [offset] [,start[/time],end[/time]]]` with `Jn`, `n` and `Mm.w.d` dates and times in −167..167 hours (RFC 8536 section 3.3.1); compute a year's two transitions from the year's Rata Die with exact `i64` arithmetic. If any supported zone has no data, stop and report: decision 2 depends on it.
- [ ] **Step 4: Run `check-intl-rust`.** Expected: pass.
- [ ] **Step 5: Commit.**

### Task 3: Pattern data, day periods and time zone lookup in the Rust core and the JSI layer

**Files:**
- Create: `intl/core/src/date_data.rs` (parsing `date-patterns.txt` and `day-periods.txt` with `include_str!`, behind `OnceLock`), `intl/jsi/DateTimeFormatHelpers.cpp`
- Modify: `intl/core/src/lib.rs`, `intl/jsi/NativeHelpers.def`, `intl/jsi/RustCore.h`, `intl/CMakeLists.txt`, `intl/test/unit/CMakeLists.txt`
- Test: unit tests in `date_data.rs`

**Interfaces:**
- Consumes: Task 1's files; Phase 3's `Utf8List` tag and `cldr.rs` string-table helpers.
- Produces:
  - `date_data::date_time_patterns(locale: &str, calendar: &str) -> Option<Vec<String>>`, the list below; the locale entry is the first one ICU4X's `LocaleFallbacker` reaches from the language identifier, as Phase 3's lookups do;
  - `date_data::day_periods(locale: &str) -> Option<Vec<String>>`;
  - helper lines:
    - `HERMES_INTL_HELPER(dateTimePatterns, Utf8List, Utf8, Utf8)`: `(locale, calendar)`;
    - `HERMES_INTL_HELPER(dayPeriods, Utf8List, Utf8)`: `(locale)`;
    - `HERMES_INTL_HELPER(canonicalizeTimeZone, OptUtf8, Utf8)`: `(name)`, the primary identifier (`host::time_zone_canonicalize`), or undefined for a name that is not a time zone. Case-insensitive.

  | Helper | Entries (CLDR pattern syntax) |
  |---|---|
  | `dateTimePatterns` | 0-3 dateFormats full, long, medium, short; 4-7 timeFormats; 8-11 standard dateTimeFormats; 12-15 `atTime` dateTimeFormats (the standard ones where the locale has none); 16 intervalFormatFallback; 17 numeric leap month pattern (`""` if none); 18 `Timezone` append item; 19 the number N of availableFormats entries, then N pairs (skeleton, pattern) in CLDR's order; then the number M of interval entries, then M triples (skeleton, greatest-difference letter, pattern) |
  | `dayPeriods` | 0 the number N of periods; then N groups of six: id, start minute, end minute (as in the file), abbreviated, wide, narrow name; then noon's abbreviated, wide and narrow names (`""` if the rules have no noon) |

- [ ] **Step 1: Write the failing tests** in `date_data.rs`:
  - `en_gregory`: entries 0-3 are `"EEEE, MMMM d, y"`, `"MMMM d, y"`, `"MMM d, y"`, `"M/d/yy"`; 7 is `"h:mm\u{202f}a"`; 12 is `"{1} 'at' {0}"`; the availableFormats have `("yMMMd", "MMM d, y")` and `("Hm", "HH:mm")`; the intervals have `("yMMMd", "d", "MMM d\u{2009}–\u{2009}d, y")`;
  - `calendars`: `ja` `japanese` has a `"G"` in its `yMMMd` pattern; `en` `chinese` 17 is `"{0}bis"` and `zh` `chinese` 17 is `"闰{0}"`; `und` `iso8601` 3 is `"y-MM-dd"`; `en-GB` `buddhist` (an `=` line or inherited) gives the same list as reading its chain by hand;
  - `fallback`: `en-AU` reads its own differences over `en`'s; a tag with no entry of its own reaches its language's;
  - `day_periods_en`: four periods with the minutes of Task 1 Step 4 and noon `"noon"`; `es-CO` puts minute 0 in `morning2` and `es` in `morning1`; `hi-Latn` has a noon and `hi` has none;
  - `every_locale_every_calendar` (Review Focus 5, the data half): for every locale of `locales.txt` and each of the 16 calendars, `date_time_patterns` succeeds; every pattern (styles, connectors, availableFormats, intervals) uses only decision 6's letters; every connector has `{0}` and `{1}`; every interval pattern has a field that repeats; and `day_periods` succeeds with rules that cover each minute of the day exactly once outside `at` rules;
  - `rejects`: an unknown calendar `"julian"` and the locale `"en_US"` give `None`.
- [ ] **Step 2: Run them to see them fail.** `check-intl-rust` fails to compile.
- [ ] **Step 3: Implement** `date_data.rs`, the C interface and the three helpers. A malformed line in the data files is a panic, because the files are ours.
- [ ] **Step 4: Run `check-intl-rust` and `check-intl-lit`.** Expected: pass; the lit tests are unchanged.
- [ ] **Step 5: Commit.**

### Task 4: Calendar fields and names in the Rust core and the JSI layer

**Files:**
- Create: `intl/core/src/datetime.rs`
- Modify: `intl/core/src/lib.rs`, `intl/jsi/DateTimeFormatHelpers.cpp`, `intl/jsi/NativeHelpers.h` (the `Double` tag), `intl/jsi/NativeHelpers.def`, `intl/jsi/RustCore.h`, `intl/core/Cargo.toml` (`writeable` if a direct dependency is needed, at the version the lock file already has)
- Test: unit tests in `datetime.rs`; `intl/test/unit/NativeHelpersTest.cpp`

**Interfaces:**
- Consumes: Task 2's `tz`, Phase 0's calendar list and `host::time_zone_canonicalize`.
- Produces:
  - `datetime::DateTimeFormat` (`HiDateTimeFormat` in C, opaque, with a compile-time `Send` assertion), by decisions 3, 4 and 16;
  - `datetime::DateTimeFormat::new(locale: &str, calendar: &str, time_zone: &str, symbols: &[&str]) -> Option<Self>`: `calendar` one of the 16 BCP 47 ids; `time_zone` a primary identifier or a `±HH:MM` offset; `symbols` CLDR name symbols from this set: `G GGGG GGGGG MMM MMMM MMMMM LLL LLLL LLLLL EEE EEEE EEEEE ccc cccc ccccc a aaaa aaaaa U z zzzz O OOOO v vvvv`;
  - `DateTimeFormat::local_time(&self, epoch_ms: f64) -> Option<Vec<String>>`, for an integral `epoch_ms` in ±8.64 × 10^15, the list below;
  - the `Double` tag: an argument that must be a finite JS number, else an internal error naming the helper;
  - `class DateTimeFormatState : public jsi::NativeState` in `DateTimeFormatHelpers.cpp`;
  - helper lines:
    - `HERMES_INTL_HELPER(dateTimeFormatInit, Undefined, Object, Utf8, Utf8, Utf8, Utf8)`: `(obj, locale, calendar, timeZone, symbols)`, symbols space-separated (possibly empty); throws if creation fails or `obj` already has native state;
    - `HERMES_INTL_HELPER(dateTimeFormatLocalTime, Utf8List, Object, Double)`: `(obj, epochMs)`.

  | Index | `dateTimeFormatLocalTime` entry |
  |---|---|
  | 0 | [[Weekday]], `"0"` (Sunday) to `"6"` |
  | 1 | [[Era]] code, `""` if none |
  | 2 | [[Year]] (decision 3), a decimal integer |
  | 3 | [[RelatedYear]], `""` if none |
  | 4 | [[Month]] number (decision 3) |
  | 5 | the month code (`"M04L"`) |
  | 6 | `"1"` for a leap month, else `"0"` |
  | 7-11 | [[Day]], [[Hour]], [[Minute]], [[Second]], [[Millisecond]] |
  | 12 | [[InDST]], `"1"` or `"0"` |
  | 13 | the offset in seconds |
  | 14 + i | the text of `symbols[i]` for this instant (`"January"`, `"AD"`, `"Eastern Standard Time"`, ASCII digits in zone names) |

- [ ] **Step 1: Write the failing tests.**
  - Rust, in `datetime.rs`, with `UTC` unless stated:
    - `gregory_fields`: `en` `gregory`, 2026-01-05T15:07:09.123Z gives weekday 1, era `"ce"`, year 2026, month 1, day 5, 15, 7, 9, 123; `-62198755200000` (2 BC) gives era `"bce"` and year 2 (so the draft's step leaves it alone); `8.64e15` is Saturday, September 13, 275760; `-8.64e15` is Tuesday, April 20, 271822 BC;
    - `names`: symbols `EEEE MMMM LLLL G GGGG a` give `"Monday"`, `"January"`, `"January"`, `"AD"`, `"Anno Domini"`, `"PM"`; `de` `MMMM` gives `"Januar"`; `pl` `MMMM` and `LLLL` for January give `"stycznia"` and `"styczeń"`;
    - `zones`: `America/New_York` with `z zzzz O OOOO v vvvv` at 2026-07-01T12:00Z gives `"EDT"`, `"Eastern Daylight Time"`, `"GMT-4"`, `"GMT-04:00"`, `"ET"`, `"Eastern Time"`, offset −14400 and InDST `"1"`; `+05:30` gives `"GMT+5:30"` for `z`; `UTC` gives `"UTC"` and `"Coordinated Universal Time"`; `America/New_York` at 1800 gives `"GMT-04:56:02"` for `OOOO` and, since the offset is not one of CLDR's, `"GMT-4:56:02"` for `z`; `Europe/Dublin` July 2026 `zzzz` is `"Irish Standard Time"`;
    - `lunisolar`: `en` `chinese` at 2020-05-23 gives month 4, code `"M04L"`, leap `"1"`, related year 2020, a `U` name; `zh` `chinese` 2019-06-01 gives `U` `"己亥"` and related year 2019; `en` `hebrew` 2024-02-15 gives code `"M05L"` and `MMMM` `"Adar I"`, 2024-03-15 `"Adar II"`, 2023-03-15 `"Adar"`;
    - `eras`: `ja` `japanese` with `G` on 2019-04-30 and 2019-05-01 gives `"平成"` and `"令和"`, years 31 and 1; `en` `roc` 1911-06-01 gives the before-ROC era; `ethioaa` gives one era;
    - `iso8601`: year 0 (`-62167219200000`, which is `Date.parse("0000-01-01T00:00:00Z")`; `Date.UTC(0, 0, 1)` is 1900) gives year 0 and era `""`;
    - `every_calendar_every_locale` (Review Focus 5, the names half): for every locale of `locales.txt` and each of the 16 calendars, initializing with every symbol decision 4 allows for that calendar succeeds, and `local_time` at −8.64e15, 0 and 8.64e15 succeeds with 14 entries plus one per symbol, none of them produced by an ICU4X formatting error;
    - `rejects_bad_input`: an unknown calendar or symbol, `U` with `gregory`, `G` with `chinese`, `"en_US"`, an unknown zone, `"+25:00"`, `epoch_ms` `1.5`, `NaN` and `8.64e15 + 1` give `None`.
  - gtests in `NativeHelpersTest.cpp`:
    - `DoubleArgumentsAreChecked`: a `Double` helper called with `"1"`, `undefined` and `Infinity` throws an Error whose message names the helper;
    - `DateTimeFormatStateFreedWithRuntime`: `dateTimeFormatInit` then `dateTimeFormatLocalTime(obj, 0)` returns 15 entries with entry 14 the requested symbol's text, then destroy the runtime; LeakSanitizer reports nothing;
    - `DateTimeFormatHelpersRejectMisuse`: `dateTimeFormatLocalTime` on an object without state, `dateTimeFormatInit` twice, and an unknown symbol each throw an Error containing `Intl internal error`.
- [ ] **Step 2: Run them to see them fail.** `check-intl-rust` fails to compile.
- [ ] **Step 3: Implement** `datetime.rs`, the C interface, the tag, `DateTimeFormatState` and the two helpers, by decisions 3 and 4. The calendar comes from Phase 0's BCP 47 to ICU4X mapping (`supported.rs`'s test `calendars_are_icu4x_calendars` shows it). If ICU4X's public API cannot give one of decision 4's names, stop and report which, with the API you tried.
- [ ] **Step 4: Run `check-intl-rust` and `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Commit.**

### Task 5: Patterns, records and format matching in JS

**Files:**
- Create: `intl/js/51-datetime-patterns.js`
- Test: `intl/test/lit/js/datetime-patterns.js`, an isolation test built like `number-core.js`, concatenating `01-intrinsics.js` to `07-number.js` and `51-datetime-patterns.js`, with fixture data: the `en` `gregory` lists that `dateTimePatterns` returns (captured once from the native helper and written into the test) and `en`'s `dayPeriods` list

**Interfaces:**
- Produces (for Tasks 6 to 9), by decisions 6, 7 and 11:
  - `cldrPatternToRecord(pattern) -> record`: `{__proto__: null, pattern, pattern12, <field>: width..., monthStandalone, weekdayStandalone, ampmWidth, dayPeriodWidth}` by decision 6's table (`pattern12` equals `pattern`);
  - `dateTimeLocaleData(patterns, calendar, decimalSeparator) -> {__proto__: null, formats: {h12: list, h24: list}, styles: ..., ...}` from a `dateTimePatterns` list, the calendar (decision 6's `chinese`, `dangi` and `iso8601` rules) and the fraction separator;
  - `BasicFormatMatcher(options, formats)`, the draft's, step by step;
  - `BestFitFormatMatcher(options, data, family)`, decision 6;
  - `DateTimeStyleFormat(dateStyle, timeStyle, styles)`, the draft's, with decision 7's records;
  - `rangePatternsOf(record, data, family) -> DateTime Range Pattern Record` with `PatternParts` lists, decision 11;
  - `nameSymbolsOf(record, rangePatterns, calendar) -> string`: the space-separated CLDR name symbols the record and its range patterns need (decision 4), never one the calendar does not have, for `dateTimeFormatInit`;
  - `dayPeriodOf(dayPeriods, hour, minute, second, showsMinute, showsSecond) -> id` and the name lookup, decision 4.

- [ ] **Step 1: Write the failing test.** Print each case with `JSON.stringify` and check it with FileCheck:
  - records: `"MMM d, y"` is `{month: "short", day: "numeric", year: "numeric", pattern: "{month} {day}, {year}"}`; `"h:mm a"` has `pattern: "{hour}:{minute} {ampm}"`; `"LLLL"` is a stand-alone long month; `"EEEE, MMMM d, y 'at' h:mm a"` keeps `" at "` as literal text; `"r(U)"` is `{year: "numeric", pattern: "{relatedYear}({yearName})"}`;
  - best fit, `en`, family `h12` unless stated:
    - `{month: "2-digit", day: "2-digit"}` gives `"{month}/{day}"` with both `"2-digit"`;
    - `{hour: "2-digit", minute: "2-digit"}` gives `hour: "2-digit"`, `minute: "2-digit"`;
    - `{hour: "numeric", minute: "numeric"}` with family `h24` gives `"{hour}:{minute}"` with `hour: "2-digit"`;
    - `{month: "long", minute: "numeric"}` gives `"{month} 'at' {minute}"` as `"{month} at {minute}"` with a stand-alone long month;
    - `{weekday: "long", year: "numeric", month: "long", day: "numeric", hour: "numeric", minute: "numeric"}` gives `"{weekday}, {month} {day}, {year} at {hour}:{minute} {ampm}"`, `minute: "2-digit"`;
    - `{year: "numeric", month: "short", day: "numeric", hour: "numeric", minute: "numeric"}` uses `", "` between date and time;
    - `{hour: "numeric", timeZoneName: "long"}` gives `"{hour} {ampm} {timeZoneName}"`;
    - `{minute: "numeric", fractionalSecondDigits: 2}` gives `"{minute}:{second}.{fractionalSecondDigits}"` with the decimal separator passed in;
    - `{minute: "numeric"}` alone gives `"{minute}"`, `minute: "numeric"`;
  - basic: `{month: "2-digit", day: "2-digit"}` gives `"{month}/{day}"` with both `"numeric"` (no width adjustment); `{hour, minute, second: "numeric", fractionalSecondDigits: 3}` keeps the fraction, `fractionalSecondDigits: 3`; `{fractionalSecondDigits: 1}` and `{fractionalSecondDigits: 2}` alone give their own widths, in best fit too;
  - required subsets: the union of the fixture's two families has a record for each of the draft's 14 required field sets (decision 6); `{dayPeriod: "short", hour: "numeric"}` in family `h24` gives `"{hour}"` with `hour: "2-digit"` and no `dayPeriod`, and `{dayPeriod: "short"}` in family `h24` gives `"{dayPeriod}"`;
  - calendars: with `en` `chinese` fixture data, `"M/d/y"` gives `"{month}/{day}/{relatedYear}"`, and best fit for `{year, month, day}` numeric picks `yyyyMd`'s `"{month}/{day}/{relatedYear}"` over `UMd`'s `"{month}/{day}/{yearName}"`, while `{year: "numeric"}` alone still picks `"{relatedYear}({yearName})"`; `nameSymbolsOf` of an `iso8601` `"G y"` record has no `G`;
  - separators: records built with `"."` and with `"٫"` are cached apart, in either order of creation;
  - styles: `DateTimeStyleFormat("full", "short", ...)` gives `"{weekday}, {month} {day}, {year} at {hour}:{minute} {ampm}"`; time style `short` in family `h24` gives `"{hour}:{minute}"` with `hour: "2-digit"`;
  - range patterns (decision 11), each printed as `[source, pattern]` pairs:
    - `yMMMd`'s [[Day]] is `[shared "{month} "], [startRange "{day}"], [shared " – "], [endRange "{day}"], [shared ", {year}"]` (the dash with U+2009 around it), [[Month]] has the month and day in the start and end parts, and [[Default]] is FB(P);
    - `yMd`'s [[Day]] has each date whole;
    - `hm`'s [[Era]] to [[Day]] are absent, [[AmPm]] is CLDR's `a` pattern, [[DayPeriod]] equals [[Hour]], and [[Default]] is FB(`"{month}/{day}/{year}, {hour}:{minute} {ampm}"`);
    - `Hm` (family `h24`): [[AmPm]] equals [[Hour]];
    - `Bhm`: [[AmPm]] equals [[DayPeriod]], which is CLDR's `B` pattern;
    - decision 11's all-pairs check over the hours of a day for the `h`, `H`, `Bh`, `hm`, `Hm`, `Bhm` and `hms` records, running the draft's loop on those range records;
    - `hms`: [[Hour]], [[Minute]] and [[Second]] are FB(P);
    - `MMMd`: [[Year]] is FB of `"{month} {day}, {year}"`;
    - the glued `yMMMd` plus `hm`: [[Year]] to [[Day]] are FB(P) and [[Hour]] starts with the shared `"{month} {day}, {year}, "`;
  - names: `nameSymbolsOf` of the glued full record is `"EEEE MMMM a"` (in record order, no repeats);
  - day periods with `en`'s fixture: 0:00 is `morning1`, 6:00 `morning1`, 12:00 with no minutes shown is noon, 12:30 with minutes shown is `afternoon1`, 12:00 with minutes shown and minute 0 is noon, 21:00 `night1`.
- [ ] **Step 2: Run it to see it fail.** `LIT_FILTER=datetime-patterns` with `check-intl-lit` fails: the functions are not defined.
- [ ] **Step 3: Implement `51-datetime-patterns.js`.** BasicFormatMatcher and DateTimeStyleFormat step by step from `spec/datetimeformat.html`; the rest by decisions 6, 7 and 11, each function commented with the decision it implements. The CLDR pattern reader handles quotes (`''` is a quote) by code unit, with no regular expression.
- [ ] **Step 4: Run `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Commit.**

### Task 6: The Intl.DateTimeFormat constructor and resolvedOptions

**Files:**
- Create: `intl/js/50-DateTimeFormat.js` (the constructor, CreateDateTimeFormat, ChainDateTimeFormat, UnwrapDateTimeFormat, the time zone option operations, `supportedLocalesOf`, `resolvedOptions`, `@@toStringTag`, and the methods as stubs), `intl/js/52-datetime-format.js` (the stubs' targets)
- Modify: `intl/js/01-intrinsics.js` (capture `Date.now` and `Date.prototype.getTime`), `intl/js/10-Intl.js` (call `installDateTimeFormat(IntlObject)`), `intl/js/20-Locale.js` (decision 8's split), `intl/test/test262-expectations.txt`
- Test:
  - Create: `intl/test/lit/datetime-format.js`, `datetime-format-objects.js`, `datetime-format-time-zones.js`
  - Modify: `shapes-table.js`, `tamper.js`

**Interfaces:**
- Consumes: Task 3's `dateTimePatterns` and `canonicalizeTimeZone`, Task 4's `dateTimeFormatInit`, Task 5's functions, `ResolveOptions`, `FilterLocales`, `GetOption`, `GetNumberOption`, the shape helpers, Phase 3's fallback symbol and OrdinaryHasInstance.
- Produces (for Tasks 7 to 9):
  - `DateTimeFormatSlots`, a `ReturnOverride` subclass with one private field per draft slot plus `#data` (the record's parsed patterns, range patterns, name symbols and internal NumberFormats), and static `is(o)` and `get(o, name)`;
  - `initializeDateTimeFormat(dtf, locales, options, required, defaults)`: the draft's CreateDateTimeFormat from step 2 on, on an object that already exists, as `initializeNumberFormat` is in `40-NumberFormat.js`. The constructor passes the `this` Hermes created from `new.target` (or, without `new`, an object it creates from %Intl.DateTimeFormat.prototype%), so `new.target.prototype` is read once (the spec's documented deviation for plain-function constructors);
  - `CreateDateTimeFormat(locales, options, required, defaults)`: a new object from %Intl.DateTimeFormat.prototype%, initialized; the Date overrides use it;
  - the methods `format` (getter), `formatToParts`, `formatRange` and `formatRangeToParts` installed with their RequireInternalSlot, undefined checks and conversions, calling `FormatDateTime`, `FormatDateTimeToParts`, `FormatDateTimeRange` and `FormatDateTimeRangeToParts` in `52-datetime-format.js`, which throw `TypeError("Intl internal error: formatting not implemented")` until Tasks 7 and 8.

- [ ] **Step 1: Write the failing lit tests.** Run each with `%hermes -Xintl-default-locale=en-US -Xintl-default-time-zone=UTC %s`.
  - `datetime-format.js`:
    - `resolvedOptions()` of `new Intl.DateTimeFormat()` prints `{"locale":"en-US","calendar":"gregory","numberingSystem":"latn","timeZone":"UTC","year":"numeric","month":"numeric","day":"numeric"}`;
    - `{hour: "numeric", minute: "numeric"}` prints `hourCycle: "h12"`, `hour12: true`, `hour: "numeric"`, `minute: "2-digit"`;
    - `hour12` and `hourCycle`: `en` `hour12: false` gives `h23`; `ja` `hour12: true` gives `h11`; `hour12` wins over `hourCycle`; `en-u-hc-h23` gives `h23`, and with `hour12: true` the `-u-hc` is dropped from `locale`;
    - `{dateStyle: "full", timeStyle: "short"}` prints `dateStyle`, `timeStyle` and `hourCycle` and no component fields; `{dateStyle: "short", hour: "numeric"}` throws `TypeError`;
    - `new Intl.DateTimeFormat("en", {calendar: "iso8601", era: "short", year: "numeric"})` and the same with `chinese` and `dangi` construct;
    - `calendar`: `th` resolves `buddhist`, `en-u-ca-japanese` `japanese`, `{calendar: "islamic"}` the locale default, `"ethiopic-amete-alem"` `ethioaa`, `{calendar: "gregory!"}` throws `RangeError`;
    - `numberingSystem`: `ar-EG` `arab`, `{numberingSystem: "thai"}` `thai`;
    - `{era: "short"}` adds `year`, `month` and `day`; `fractionalSecondDigits: 0` throws `RangeError`; `{formatMatcher: "basic", month: "2-digit", day: "2-digit"}` reports both `"numeric"` and the default reports both `"2-digit"`;
    - `Intl.DateTimeFormat("de")` without `new` is a DateTimeFormat; `supportedLocalesOf(["de", "xx"])` is `["de"]`; `format` returns the same function each time; `Object.prototype.toString.call(new Intl.DateTimeFormat())` is `"[object Intl.DateTimeFormat]"`.
  - `datetime-format-time-zones.js`:
    - `"utc"`, `"Etc/UTC"`, `"Etc/GMT"` and `"GMT"` resolve to `"UTC"`; `"asia/calcutta"` to `"Asia/Kolkata"`; `"Pacific/Truk"` to `"Pacific/Chuuk"`; `"Atlantic/Jan_Mayen"` to `"Arctic/Longyearbyen"`;
    - offsets: `"+05:30"`, `"+0530"` and `"+05"` resolve to `"+05:30"` and `"+05:00"`; `"-00:00"` to `"+00:00"`; `"+05:30:00"`, `"−05:00"`, `"+5"`, `"+05:3"` and `"+24:00"` throw `RangeError`;
    - the 25 non-IANA names of test262's `timezone-legacy-non-iana.js` (`"PST"`, `"IST"`, ...) each throw `RangeError`;
    - every value of `Intl.supportedValuesOf("timeZone")` is accepted and resolves to itself.
  - `datetime-format-objects.js`: Review Focus 4, the DateTimeFormat half, including ChainDateTimeFormat's property descriptor on the wrapper, and `Reflect.construct(Intl.DateTimeFormat, [], proxy)` with a Proxy `newTarget` whose `get` trap counts reads of `"prototype"`: exactly one.
  - `shapes-table.js`: these rows, and a `bound` row for holder `new Intl.DateTimeFormat()`, property `format`, length 1.

    | Holder | Property | Kind | Name | Length |
    |---|---|---|---|---|
    | `Intl` | `DateTimeFormat` | constructor | `DateTimeFormat` | 0 |
    | `Intl.DateTimeFormat` | `supportedLocalesOf` | method | `supportedLocalesOf` | 1 |
    | `Intl.DateTimeFormat.prototype` | `resolvedOptions` | method | `resolvedOptions` | 0 |
    | `Intl.DateTimeFormat.prototype` | `format` | getter | `get format` | |
    | `Intl.DateTimeFormat.prototype` | `formatToParts` | method | `formatToParts` | 1 |
    | `Intl.DateTimeFormat.prototype` | `formatRange` | method | `formatRange` | 2 |
    | `Intl.DateTimeFormat.prototype` | `formatRangeToParts` | method | `formatRangeToParts` | 2 |
    | `Intl.DateTimeFormat.prototype` | `@@toStringTag` | toStringTag | `Intl.DateTimeFormat` | |

  - `tamper.js`: Review Focus 3 for the constructor and `resolvedOptions`.
- [ ] **Step 2: Run them to see them fail.** `LIT_FILTER=datetime-format` with `check-intl-lit` fails: `Intl.DateTimeFormat` is undefined.
- [ ] **Step 3: Implement** `50-DateTimeFormat.js` step by step from `spec/datetimeformat.html`, and IsTimeZoneOffsetString, ParseTimeZoneOffsetString and FormatOffsetTimeZoneIdentifier from ECMA-262 and the draft. Note that ResolveOptions turns `hour12` into the string `"true"` or `"false"` (its step 6.d.i), so CreateDateTimeFormat's "if hour12 is true" compares with those strings, which is the draft's evident intent. SystemTimeZoneIdentifier() is the default time zone the loader resolved (`defaults.timeZone`). The constructor ends by calling `dateTimeFormatInit` with the language identifier of [[Locale]], [[Calendar]], [[TimeZone]] and `nameSymbolsOf` of the record and its range patterns.
- [ ] **Step 4: Run `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Update the expectations.** The constructor, option, `resolvedOptions`, `supportedLocalesOf`, property, `name`, `length`, `prop-desc`, `builtin`, `toStringTag`, legacy and `taint-*` lines that now pass go, and so do the `testWithIntlConstructors` lines (`Collator/this-value-ignored.js`, `NumberFormat/this-value-ignored.js`, `PluralRules/undefined-newtarget-throws.js`, `constructors-taint-*`, `supportedLocalesOf-*`, `default-locale-*`, `language-tags-*`, `fallback-locales-are-supported.js`) and `Intl/supportedValuesOf/*-accepted-by-DateTimeFormat.js` if they pass. The four lines of decision 10 get its reason. List every removed line in the report. Then run `check-intl-test262`. Expected: `0 problem(s)`.
- [ ] **Step 6: Commit.**

### Task 7: format and formatToParts

**Files:**
- Modify: `intl/js/52-datetime-format.js` (ToLocalTime, FormatDateTimePattern, PartitionDateTimePattern, FormatDateTime, FormatDateTimeToParts, TimeClip), `intl/js/51-datetime-patterns.js` if needed, `intl/test/test262-expectations.txt`
- Test: Create `intl/test/lit/datetime-format-values.js`, `datetime-format-calendars.js`, `datetime-format-nu.js`, `datetime-format-locales.js`, `datetime-format-gc.js`; modify `tamper.js`

**Interfaces:**
- Consumes: Task 4's `dateTimeFormatLocalTime`, Task 5's records and day periods, Task 6's slots, Phase 3's internal NumberFormat creation and FormatNumeric, and the digit table.
- Produces: `ToLocalTime(dtf, epochMs) -> record` (decision 3's fields, plus the names by symbol), `FormatDateTimePattern(dtf, format, pattern, epochMs) -> parts`, `PartitionDateTimePattern`, `FormatDateTime`, `FormatDateTimeToParts`. Task 8 uses the first two; Task 9 uses `FormatDateTime`. The draft passes epoch nanoseconds as a BigInt; the plugin passes the TimeClip'd millisecond Number, which holds the same value (the draft multiplies an integer by 10^6), so no BigInt is made.

- [ ] **Step 1: Write the failing lit tests**, with `-Xintl-default-time-zone=UTC` and the instant `d = Date.UTC(2026, 0, 5, 15, 7, 9, 123)` unless stated:
  - `datetime-format-values.js`:
    - `en` `format(d)` is `"1/5/2026"`, `de` `"5.1.2026"`, `ja` `"2026/1/5"`, `ar-EG` `"٥‏/١‏/٢٠٢٦"`;
    - styles: `en` short `"1/5/26"`, medium `"Jan 5, 2026"`, long `"January 5, 2026"`, full `"Monday, January 5, 2026"`; time short `"3:07 PM"`, medium `"3:07:09 PM"`, long `"3:07:09 PM UTC"`; `en-GB` full and long `"Monday, 5 January 2026 at 15:07:09 UTC"`; `de` full and short `"Montag, 5. Januar 2026 um 15:07"`; `ja` full and short `"2026年1月5日月曜日 15:07"`;
    - hour cycles at 00:07: `h11` `"0 AM"`, `h12` `"12 AM"`, `h23` `"00"`, `h24` `"24"`;
    - `{year: "numeric", era: "short"}` at `-62198755200000` is `"2 BC"`; `{year: "2-digit"}` is `"26"`;
    - `{second: "numeric", fractionalSecondDigits: 3}` is `"9.123"` in `en` and `"9,123"` in `de`; `{fractionalSecondDigits: 2}` is `"12"`;
    - `{dayPeriod: "long"}` over the 24 hours of a day in `en` gives exactly test262's five values in order, and `{hour: "numeric", minute: "numeric", dayPeriod: "short"}` at 09:07 is `"9:07 in the morning"`;
    - time zones: `America/New_York` `timeZoneName` `short` at 2026-07-01T12Z is `"7/1/2026, EDT"`, `long` at 2026-01-01T12Z `"1/1/2026, Eastern Standard Time"`, `shortGeneric` `"1/5/2026, ET"`; `Asia/Kolkata` `longOffset` `"1/5/2026, GMT+05:30"`; `-03:30` with `hour` and `short` `"11 AM GMT-3:30"`;
    - Review Focus 1: each instant and zone it lists, through `format` and `formatToParts`, with the parts' values joining to `format`'s result; `8.64e15` with `{weekday: "long", year: "numeric", month: "long", day: "numeric"}` is `"Saturday, September 13, 275760"`, and `-8.64e15` with `era: "short"` added is `"Tuesday, April 20, 271822 BC"`;
    - `format(NaN)`, `format(8.64e15 + 1)` and `formatToParts(Infinity)` throw `RangeError`; `format()` formats `Date.now()`; `format("2026-01-05")` converts with ToNumber and throws `RangeError` (NaN);
    - `formatToParts(d)` with `{hour: "numeric", minute: "numeric", second: "numeric", fractionalSecondDigits: 2, timeZoneName: "short"}` is hour, literal, minute, literal, second, literal `"."`, fractionalSecond `"12"`, literal, dayPeriod `"PM"`, literal, timeZoneName `"UTC"`.
  - `datetime-format-calendars.js`: Review Focus 2. Check each value against Node, then explain every difference in the report (decision 17 lists the expected ones). Among them: `zh-u-ca-chinese` `{year: "numeric"}` at `Date.UTC(2019, 5, 1)` is `"2019己亥年"` with parts relatedYear, yearName, literal; `zh-u-ca-chinese` `{month: "numeric", day: "numeric"}` at 2020-06-01 is `"闰4-10"`; `en-u-ca-chinese` `dateStyle: "full"` at 2020-06-01 is `"Monday, Fourth Monthbis 10, 2020(geng-zi)"`; `ja-u-ca-japanese` `{era: "long", year: "numeric"}` at 2020-06-01 is `"令和2年"`; `en-u-ca-hebrew` `{month: "long"}` on 2024-02-15, 2024-03-15 and 2023-03-15 is `"Adar I"`, `"Adar II"`, `"Adar"`; `en-u-ca-iso8601` `{}` is `"2026-01-05"`; `formatToParts(0)` with `{calendar: "iso8601", era: "short", year: "numeric"}` has no `era` part (literal `" "`, year `"1970"`, as Node gives), and the same with `chinese` and `dangi` has none either; `en-u-ca-chinese` `{year: "numeric", month: "numeric", day: "numeric"}` at 1984-02-02 is `"1/1/1984"` with a `relatedYear` part.
  - `datetime-format-nu.js`: `en-u-nu-arab` `{timeZoneName: "short", timeZone: "Asia/Kolkata"}` is `"٦/١/٢٠٢٠, GMT+٥:٣٠"` at 2020-06-01T12Z; `en-u-nu-thai` with `hour` and `shortOffset` `"๕ PM GMT+๕:๓๐"`; `en-u-nu-mathbold` `{}` `"𝟔/𝟏/𝟐𝟎𝟐𝟎"`; `fa` `{}` `"۱۳۹۹/۳/۱۲"`; `2-digit` fields in `mathbold` keep two code points, not two code units.
  - `datetime-format-locales.js`: Review Focus 5, the JS half: `dateStyle` and `timeStyle` `full` for 40 locales chosen across scripts and regions (`en`, `en-GB`, `de`, `fr`, `es`, `pt-BR`, `it`, `nl`, `pl`, `ru`, `uk`, `cs`, `tr`, `el`, `he`, `ar`, `ar-EG`, `fa`, `hi`, `bn`, `ta`, `th`, `vi`, `id`, `zh`, `zh-Hant`, `zh-HK`, `ja`, `ko`, `sw`, `am`, `my`, `km`, `ka`, `hy`, `kk`, `mn`, `sr-Latn`, `bal-Latn`, `ff-Adlm`), each with its default calendar and numbering system: no exception, and `formatToParts` joins to `format`. For the same locales and every supported calendar, `formatMatcher: "basic"` with each of the draft's 14 required field sets (all `numeric`, `weekday: "short"`, `dayPeriod: "short"`, `fractionalSecondDigits: 3`, and `hour12: true` for the sets with `dayPeriod`) gives `resolvedOptions` with every field of the set and no other field except `era` in a calendar with eras (`en` `japanese` `yyyyMd` is `"M/d/y GGGGG"`, so it adds `era: "narrow"`, as Node shows). At 09:00, `{dayPeriod: "short"}` is `"in the morning"` in `en` with `hourCycle: "h23"` and `"morgens"` in `de` (24-hour by default), and both report `dayPeriod`.
  - `datetime-format-gc.js`: 2,000 DateTimeFormats alternating locales, calendars and time zones in a loop, `gc()` every 100, every tenth kept; format with the survivors afterwards; end with live formatters. ASan and LeakSanitizer report nothing.
  - `tamper.js`: Review Focus 3, the `format` and `formatToParts` half.
- [ ] **Step 2: Run them to see them fail.** The methods throw the internal-error `TypeError` of Task 6.
- [ ] **Step 3: Implement** the operations step by step from the draft, with decisions 3, 4 and 12.
- [ ] **Step 4: Run `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Update the expectations.** The `format` and `formatToParts` lines go, and any other line that now passes. Then run `check-intl-test262`. Expected: `0 problem(s)`.
- [ ] **Step 6: Commit.**

### Task 8: formatRange and formatRangeToParts

**Files:**
- Modify: `intl/js/52-datetime-format.js` (PartitionDateTimeRangePattern, FormatDateTimeRange, FormatDateTimeRangeToParts), `intl/js/51-datetime-patterns.js` if needed, `intl/test/test262-expectations.txt`
- Test: Create `intl/test/lit/datetime-format-range.js`; modify `tamper.js`

**Interfaces:**
- Consumes: Task 5's `rangePatternsOf`, Task 7's ToLocalTime and FormatDateTimePattern, Task 5's day periods (the draft's [[DayPeriod]] comparison).
- Produces: PartitionDateTimeRangePattern, FormatDateTimeRange, FormatDateTimeRangeToParts.

- [ ] **Step 1: Write the failing lit test** `datetime-format-range.js`, `en`, `UTC`, `b = Date.UTC(2026, 0, 5, 9, 7, 3, 45)`, `H` an hour in ms (each dash with U+2009 around it, as CLDR 48.2 has it):
  - test262's `en-US.js` cases for `formatRangeToParts`, restated so the plugin's own suite pins them;
  - `{dateStyle: "medium"}` from 2026-01-05 to 2026-01-09 is month shared, literal shared, day startRange `"5"`, literal shared `" – "`, day endRange `"9"`, literal shared `", "`, year shared;
  - `{hour, minute}`: `b` to `b + 5H` `"9:07 AM – 2:07 PM"`; `b + 4H` to `b + 9H` `"1:07 – 6:07 PM"`; with `h23` `"09:07 – 14:07"`; `b` to `b + 4 days` `"1/5/2026, 9:07 AM – 1/9/2026, 1:07 PM"`;
  - `{hour, minute, second}`: `b` to `b + 7s` `"9:07:03 AM – 9:07:10 AM"`; with `fractionalSecondDigits: 2`, `b` to `b + 120` `"9:07:03.04 AM – 9:07:03.16 AM"`;
  - `{hour, minute, dayPeriod: "short"}`: `b` to `b + 5H` `"9:07 in the morning – 2:07 in the afternoon"` (AM/PM differs, and takes [[DayPeriod]]'s pattern); `b + 4H` to `b + 9H` `"1:07 in the afternoon – 6:07 in the evening"` (decision 11);
  - `{dateStyle: "medium", timeStyle: "short"}`: `b` to `b + H` `"Jan 5, 2026, 9:07 – 10:07 AM"`; `b` to `b + 30H` `"Jan 5, 2026, 9:07 AM – Jan 6, 2026, 3:07 PM"`; `{dateStyle: "full", timeStyle: "short"}` `b` to `b + H` `"Monday, January 5, 2026, 9:07 – 10:07 AM"`;
  - `{month: "short", day: "numeric"}` `b` to `b + 400 days` `"Jan 5, 2026 – Feb 9, 2027"`; `{dateStyle: "long"}` `b` to `b + 40 days` `"January 5 – February 14, 2026"`; `{weekday: "long", month: "long", day: "numeric"}` `b` to `b + 1 day` `"Monday, January 5 – Tuesday, January 6"`;
  - `{weekday: "long", hour: "numeric"}` `b` to `b + H` `"Monday, 9 – 10 AM"`; `{hour: "numeric", timeZoneName: "short"}` `"9 – 10 AM UTC"`; `{minute: "numeric"}` `b` to `b + 60000` `"7 – 8"`;
  - `{year: "2-digit", month: "2-digit", day: "2-digit"}` `b` to `b + 1 day` `"01/05/26 – 01/06/26"` (decision 11);
  - `zh-u-ca-chinese` and `ko-u-ca-dangi` ranges across a leap month and its base month differ (the month code); `en-u-ca-chinese` and `en-u-ca-dangi` from 1984-02-02 to 2044-01-30 (both first days of a 甲子 year) are, with `{year: "numeric"}`, `"1984(jia-zi) – 2044(jia-zi)"`, and with `{year: "numeric", month: "numeric", day: "numeric"}`, `"1/1/1984 – 1/1/2044"` with `relatedYear` parts (decisions 3, 6 and 11; the dash with CLDR 48.2's spaces);
  - equal formatted ends collapse to the single format with every part `shared`; `b + H` to `b` does not throw; `undefined` throws `TypeError`, `NaN` and `8.64e15 + 1` throw `RangeError`; the two arguments are converted in order, so a throwing `valueOf` on the second runs after the first's;
  - `tamper.js`: Review Focus 3, the range half.
- [ ] **Step 2: Run it to see it fail.** It throws the internal-error `TypeError` of Task 6.
- [ ] **Step 3: Implement** the range operations step by step from the draft, with decision 11.
- [ ] **Step 4: Run `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Update the expectations.** The `formatRange` and `formatRangeToParts` lines go. Then run `check-intl-test262`. Expected: `0 problem(s)`.
- [ ] **Step 6: Commit.**

### Task 9: The Date toLocale* overrides

**Files:**
- Modify: `intl/js/80-builtins.js`, `intl/test/test262-expectations.txt`, and any `test/hermes` test of Hermes's own Date `toLocale*` methods (add `-Xintl-plugin=false` to its RUN line, as Phase 3 did for `number-functions.js`; list the files in the report)
- Test:
  - Create: `intl/test/lit/to-locale-date-string.js`
  - Modify: `shapes-table.js`, `tamper.js`

**Interfaces:**
- Consumes: Task 6's `CreateDateTimeFormat`, Task 7's `FormatDateTime`.
- Produces: `Date.prototype.toLocaleString`, `toLocaleDateString` and `toLocaleTimeString`, by decisions 13 and 15.

- [ ] **Step 1: Write the failing lit test** `to-locale-date-string.js`, `UTC`:
  - `new Date(d).toLocaleString("en")` is `"1/5/2026, 3:07:09 PM"`, `toLocaleDateString("en")` `"1/5/2026"`, `toLocaleTimeString("en")` `"3:07:09 PM"`; `toLocaleString()` uses `en-US`;
  - `toLocaleDateString("en", {hour: "numeric"})` is `"1/5/2026, 3 PM"` (the date defaults stay); `toLocaleTimeString("en", {dateStyle: "short"})` throws `TypeError` (CreateDateTimeFormat's `required` check);
  - `new Date(NaN).toLocaleString()` is `"Invalid Date"`;
  - `Date.prototype.toLocaleString.call({})` and `.call(Date.prototype)` throw `TypeError`;
  - every output equals `new Intl.DateTimeFormat(locales, options).format(d)` for the same arguments, with the right defaults (the check test262's `returns-same-results-as-DateTimeFormat.js` makes);
  - shapes rows, all kind `method`, length 0: `Date.prototype` `toLocaleString`, `toLocaleDateString`, `toLocaleTimeString`;
  - `tamper.js`: Review Focus 3, the overrides half, including a replaced `Intl.DateTimeFormat` (the overrides must not look it up).
- [ ] **Step 2: Run it to see it fail.** Hermes's own methods ignore `locales`.
- [ ] **Step 3: Implement** the overrides step by step from `spec/locale-sensitive-functions.html`.
- [ ] **Step 4: Run `check-intl-lit`, and `check-hermes` filtered to the `test/hermes` tests you changed.** Expected: pass.
- [ ] **Step 5: Update the expectations.** The `Date/prototype` lines go. The `built-ins/Date/prototype/toLocale*String` tests that pass now must still pass. Then run `check-intl-test262`. Expected: `0 problem(s)`.
- [ ] **Step 6: Commit.**

### Task 10: Acceptance, spec and README

**Files:**
- Modify:
  - `doc/superpowers/specs/2026-10-22-intl-plugin-design.md`:
    - "The JS layer": the new files, decision 1, the caches of decision 13, the legacy behavior of decision 14 and the Date overrides of decision 15;
    - "The native interface": the `Double` tag, `DateTimeFormatState`, the date helpers, and the change in decision 1 to the list of gaps the Rust core fills and to "Parts";
    - "Data": decisions 2 to 11 and 17, the tzdb version, and the library size after this phase;
    - "Testing": the `test/hermes` tests that now run with the plugin off;
    - "Phasing": Phase 4 done, with the four expected failures of decision 10;
  - `intl/README.md`:
    - the layout table names `tz.rs`, `date_data.rs`, `datetime.rs`, `DateTimeFormatHelpers.cpp`, the three JS files and the two data files;
    - "Adding a native helper" lists the `Double` tag.

- [ ] **Step 1: Run everything.** Run `check-intl`, then a full `check-hermes`. Expected:
  - every part of `check-intl` passes, with test262 at `0 problem(s)`;
  - `check-hermes` has 0 unexpected results.

  Record the counts in the report.
- [ ] **Step 2: Check the expectations file.**
  - `grep -c 'Phase 4' intl/test/test262-expectations.txt` is 0, and the four tests of decision 10 keep their lines with its reason. No other line remains for a Phase 4 test unless the controller ruled on it by name.
  - `git diff <Phase 4's plan commit> -- intl/test/test262-expectations.txt` only removes lines, apart from decision 10's four reasons.
- [ ] **Step 3: Measure the library.** Build the `hermesintl` target in Release, in a temporary worktree at `5211bc6a7` (the Phase 3 commit) and at the tip, each with its own build directory configured with Clang and `-DCMAKE_BUILD_TYPE=Release`. Record both sizes of `lib/libhermesintl.so` and of its `llvm-strip -o` output (`/usr/bin/llvm-strip-18` if `llvm-strip` is not on PATH), and the sizes of `.rodata`, `.text` and `.data.rel.ro` (`llvm-size -A`), then remove the worktree and the build directories.
- [ ] **Step 4: Update the spec and README** as listed above, in the documents' existing style. Write in plain English, with no jargon of our own.
- [ ] **Step 5: Commit.**

## After the tasks

When the whole-branch review is done and its findings are fixed:
1. Run a Codex code review (the `codex-review` skill) of the Phase 4 commits, from the plan commit to the tip.
2. Fix or drop every finding, with a reason. Never defer one.
3. Repeat until Codex reports every finding CLOSED.
4. Rerun `check-intl` and `check-hermes` on Linux, then ask the Mac agent (gist channel) for the same runs on macOS, and fix what it finds.
5. Squash the branch with the `squashing-feature-history` skill (fixes to earlier phases fold into those phases). If the new tip's tree is identical to the backup tag's, delete the backup.
