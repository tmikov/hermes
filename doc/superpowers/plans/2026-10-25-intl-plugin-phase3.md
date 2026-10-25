# Intl Plugin Phase 3 (NumberFormat and PluralRules) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `Intl.NumberFormat` and `Intl.PluralRules` to the plugin with their whole public surface, and replace `Number.prototype.toLocaleString`, `BigInt.prototype.toLocaleString`, `Array.prototype.toLocaleString` and `%TypedArray%.prototype.toLocaleString` with their ECMA-402 versions:
- `Intl.NumberFormat`: the constructor and every option, legacy constructor behavior, `supportedLocalesOf`, `resolvedOptions`, the `format` getter and its bound function, `formatToParts`, `formatRange` and `formatRangeToParts`; every style (decimal, percent, currency, unit) and notation (standard, scientific, engineering, compact);
- `Intl.PluralRules`: the constructor, `supportedLocalesOf`, `resolvedOptions`, `select` and `selectRange`;
- digits for every numbering system of ECMA-402's simple-digit table, including the ones ICU4X has no data for.

At the end, every test262 test the expectations file assigns to Phase 3 passes, except `intl402/PluralRules/prototype/select/notation.js` (decision 13).

**Architecture:**
- ICU4X 2.3's number formatters do not fit ECMA-402 (they round on their own, emit few parts, have no scientific notation, ranges, NaN or infinity), so the plugin uses only ICU4X's **data** for numbers. Formatting is the draft's algorithms, written in JS: rounding with exact BigInt arithmetic, then PartitionNumberPattern over patterns and symbols that native code supplies.
- Native code supplies locale data as lists of strings, from two sources: ICU4X's compiled data where ICU4X has it, and two new files generated from CLDR 48.2 for what it lacks (decision 4). It also wraps ICU4X's plural rules, the only number service that keeps native state.
- `Intl.NumberFormat` has no native state of its own. It keeps the slots and the data it was given, and takes plural forms from a cached internal plural-rules object.
- New JS files: `intl/js/07-number.js` (Intl mathematical values and rounding, shared by NumberFormat, PluralRules and the later phases), `40-NumberFormat.js`, `41-number-patterns.js` and `45-PluralRules.js`. The overrides go in `80-builtins.js`.

**Tech Stack:** Rust 1.88 with ICU4X 2.3 (`icu` with `compiled_data` and `sync`, plus `icu_decimal` and `icu_plurals` with `unstable`, and `icu_experimental` 0.6), C++17 and JSI, Python 3 for the data generator, the plugin's JS compiled to Hermes bytecode, and test262 at `c9aa6126e8f6fe2c6f668c65bfba771ffc8a6096`.

**Spec:** `doc/superpowers/specs/2026-10-22-intl-plugin-design.md`. Read "Guiding rules", "The JS layer", "The native interface" (including "Threads", "Strings" and "Parts"), "Data", "Testing" and "Phasing" before any task. Task 12 writes this plan's decisions into it.

**ECMA-402 text:** the https://tc39.es/ecma402/ draft dated October 24, 2026, which is tc39/ecma402 commit `3691634ebc`. The relevant files are:
- `spec/numberformat.html`: the constructor, SetNumberFormatDigitOptions, SetNumberFormatUnitOptions, the internal slots and locale data constraints, every abstract operation from CurrencyDigits to FormatNumericRangeToParts, and the simple-digit table;
- `spec/pluralrules.html`: the constructor, the internal slots, ResolvePlural and ResolvePluralRange;
- `spec/locale-sensitive-functions.html`: the four `toLocaleString` overrides;
- `spec/negotiation.html`: ResolveOptions (already implemented).

Cite its section titles and number its steps. Fetch the text with `gh api -H 'Accept: application/vnd.github.raw' 'repos/tc39/ecma402/contents/spec/<file>?ref=3691634ebc'`. JoinIndexedProperties is in ECMA-262; fetch `spec.html` from `tc39/ecma262` `main` the same way.

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
  - an optional parameter is a default parameter (`toLocaleString(locales = undefined, options = undefined)`), which keeps `length` at the spec value. Never read `arguments[i]` past `arguments.length`;
  - BigInt arithmetic on primitives (`n * 10n`, `n % d`) is allowed; `BigInt(s)` goes through the captured `BigInt`.
- **Native helpers:** `intl/README.md`, "Adding a native helper". JS validates every argument before calling a helper. A helper that gets an argument JS should never send throws `std::logic_error("Intl internal error: ...")`.
- **Rust:**
  - FFI functions never panic on bad input. Invalid UTF-8, an unknown option string, a tag that does not parse or a malformed decimal string returns `false` or a null pointer.
  - Never call `Decimal::try_from_str` on a string with an exponent: its exponent arithmetic can overflow and panic. JS sends plain decimal strings (`"123.450"`), and Rust rejects any other character.
  - A panic is only for a broken invariant in our own data files.
  - Every new public function has a doc comment and a unit test.
  - Run `cargo fmt` in `intl/core`, and `clang-format` on changed C++.
- **Threads** (spec, "Threads"): a Rust type held in a `NativeState` must be `Send`, checked at compile time. A `NativeState` destructor frees only its own Rust object. The parsed CLDR files are process-wide immutable tables built once behind `std::sync::OnceLock`.
- **Prove checks can fail:** for every new test, make a stated mutation of the code under test, name the test that fails, then revert the mutation. Put this evidence in the task report.
- **Never bump `BYTECODE_VERSION`**, and never bump the plugin-interface version. Phase 3 adds native helpers and changes no entry-point, config or `IHermesBuiltinBytecode` interface.

## Planning decisions

Task 12 writes these into the spec. Implement them as stated.

1. **Formatting is JS; native code only supplies data.** FormatNumericToString, ToRawPrecision, ToRawFixed, ComputeExponent, PartitionNumberPattern, PartitionNotationSubPattern, GetNumberFormatPattern, the range operations and ResolvePlural are JS, step by step from the draft. Native code returns symbols and patterns (decision 5) and selects plural categories. `Intl.NumberFormat` attaches no native state (a change to the spec's "Native state" example); `Intl.PluralRules` does (decision 13).
2. **Intl mathematical values.** An Intl mathematical value is one of the strings `'nan'`, `'posInf'`, `'negInf'`, `'negZero'`, or a record `{__proto__: null, neg, n, e}` for `(-1)^neg × n × 10^e`, with `n` a non-negative BigInt and `e` an integer Number. The record is normalized: `n` has no trailing decimal zero (`e` absorbs them), and zero is `{neg: false, n: 0n, e: 0}`.
   - ToIntlMathematicalValue follows the draft: ToPrimitive with hint number, written in JS from ECMA-262 (`@@toPrimitive`, then OrdinaryToPrimitive), because no intrinsic exposes it. A BigInt gives its value. A Number gives `'negZero'` for -0; otherwise its string is `Number::toString(x, 10)`, which is `` `${x}` `` (no user code runs on a primitive).
   - The string is parsed with the StringNumericLiteral grammar (ECMA-262 StrWhiteSpace, StrDecimalLiteral with an optional sign, `Infinity`, NonDecimalIntegerLiteral through the captured `BigInt`). Anything else is `'nan'`.
   - RoundMVResult is `ToNumber` of the same string: it uses the same grammar and the same rounding, so `+str` being ±Infinity gives `'posInf'`/`'negInf'`, and `+str` being ±0 for a non-zero value gives 0 or `'negZero'`.
3. **Rounding uses exact BigInt arithmetic.** ToRawFixed rounds `x` to multiples of `roundingIncrement × 10^-f`, and ToRawPrecision to `10^(e-p+1)`, by one BigInt division with remainder. ApplyUnsignedRoundingMode compares twice the remainder with the divisor; half-even uses the parity of the quotient (the draft's `cardinality`). The results are the draft's records with `roundedNumber` as an Intl mathematical value and `formattedString` in ASCII with `"."`.
4. **Data sources.** ICU4X 2.3 has no unit display names for 21 of the 45 sanctioned units (none for `bit`/`byte` and their multiples, `celsius`, `fahrenheit`, `degree`, `percent`, or any `-per-` unit), and its long names for the units it has are wrong (`en` long `meter` renders `"1.5 m"`). It also has no infinity, NaN, exponent or approximately sign, and no range pattern. So:

   | Data | Source |
   |---|---|
   | decimal and grouping separators, plus and minus signs, grouping sizes | ICU4X `DecimalSymbolsV1` (`icu::decimal::provider`) |
   | compact patterns | ICU4X `DecimalCompactShortV1` / `DecimalCompactLongV1` (`icu_decimal`, `unstable`) |
   | currency patterns (standard, accounting, alpha-next-to-number), symbols, narrow symbols, plural names, name patterns, digits | `icu_experimental` 0.6 provider structs: `CurrencyEssentialsV1`, `CurrencySymbolsV1`, `CurrencyExtendedDataV1`, `CurrencyPatternsDataV1`, `CurrencyFractionsV1`. Read the data structs directly; never use `CurrencyFormatter`, which ignores the digit options, grouping and the locale's name patterns. |
   | plural rules, plural ranges, plural categories | ICU4X `PluralRules`, `PluralRulesWithRanges` (`icu_plurals`, `unstable`) |
   | percent sign, exponent symbol, infinity, NaN, approximately sign, percent pattern, range pattern | `intl/core/data/number-symbols.txt`, generated from CLDR 48.2 |
   | unit patterns | `intl/core/data/units.txt`, generated from CLDR 48.2 |
   | digits | ECMA-402's simple-digit table, in JS (`41-number-patterns.js`); native digits are never used |
   | default numbering system | the existing `defaultNumberingSystem` helper, so `getNumberingSystems` and NumberFormat agree (spec, "The native interface") |

   The generated files come from the CLDR JSON packages (`cldr-core`, `cldr-numbers-full`, `cldr-units-full`, npm version `48.2.0` for the tag `release-48-2`), whose locale data is already resolved, so no inheritance logic is needed in the generator. `intl/core/data/LICENSE` gets the Unicode License V3 text, which covers every CLDR-derived file in that directory.
5. **The data JS gets.** Each helper returns a list of strings (a new `Utf8List` result tag), and JS reads the entries by index. Patterns are already in the draft's placeholder form (`{number}`, `{minusSign}`, `{plusSign}`, `{percentSign}`, `{currencyPrefix}`, `{currencySuffix}`), except plural-dependent ones (units, currency names, compact), which stay in CLDR's `{0}` form for JS to split. The exact layouts are in Tasks 3 and 4.
6. **Numbering-system fallback for symbols.** Every symbol and pattern for `(locale, nu)` comes from that pair if the data has it, else from `(locale, "latn")`, else from the locale's default. This is CLDR's rule (symbols not defined for a numbering system are `latn`'s). ICU4X's own formatter falls back to the locale default instead, which is why the plugin reads the data itself.
7. **Lookups.** Native data is looked up by the language identifier of `[[Locale]]` (`crate::locale::language_id`; the `-u-` keywords are already in the slots) and the numbering system passed explicitly. In the two generated files, the entry for a locale is the first one ICU4X's `LocaleFallbacker` reaches from it; every locale in `locales.txt` has an entry, so this finds the entry ICU4X's own data would use. Inside `units.txt`, a missing field is inherited along the parents the file itself records.
8. **CurrencyDigits** is CLDR's `digits` for the currency (`CurrencyFractionsV1`, not the cash digits), and 2 for a currency CLDR does not list. The draft asks only for "the number of fractional digits used when formatting quantities of the currency", and CLDR's values are the ones engines built on ICU use.
9. **Grouping.** With primary size `p`, secondary size `s` and minimum grouping `g` from the data, and `n` integer digits:
   - `false`: no grouping;
   - `"always"`: group when `n > p`;
   - `"auto"`: group when `n >= p + g`;
   - `"min2"`: group when `n >= p + max(g, 2)`.

   Groups are the last `p` digits and then groups of `s`. `"always"` ignores the locale's minimum grouping (`pl` 1234 is `"1 234"`), as ICU4C does and ICU4X's `Always` does not.
10. **Parts from plural-dependent text.** A unit pattern, currency name pattern or compact pattern is split around `{0}`. Whitespace adjacent to `{0}` becomes a `literal` part, and the rest of each side becomes one `unit`, `currency` or `compact` part (`"時速 {0} キロメートル"` gives unit `"時速"`, literal `" "`, the number, literal `" "`, unit `"キロメートル"`). Whitespace is ECMA-262 WhiteSpace and LineTerminator, tested by code unit. A percent pattern's text other than `%` is literal.

    CLDR has patterns without `{0}`: `ar` long `meter` "one" is `"متر"`, and `fr` long compact has the exact-1 pattern `"mille"`. The draft's locale-data constraint wants `{number}` in every pattern; ICU4C uses these patterns as they are (`(1).toLocaleString("ar", {style: "unit", unit: "meter", unitDisplay: "long"})` is `"متر"`), and so does the plugin, as one part of the pattern's type with no number parts. Such a pattern is used only when the chosen sign pattern is the zero pattern, that is, when no sign is shown. Otherwise the plugin uses the category's pattern without the exact value, and if that has no `{0}` either, the "other" pattern. ICU4C drops the sign instead (`-1` gives `"متر"`); the plugin never hides a sign. Every "other" pattern contains `{0}`, which Task 3's and Task 4's tests check for every locale.
11. **Signs and accounting.** For decimal and percent, the minus and plus signs go where CLDR's sign goes: before the whole pattern when the locale's sign is a prefix. Currency patterns come from ICU4X's explicit positive and negative patterns. An accounting negative pattern has no `{minusSign}` (`"($0.00)"`), unlike the draft's locale-data constraint; test262 and ICU4C both produce it. The currency name display has no accounting form and uses the standard sign patterns. A unit or currency name pattern wraps the signed number (`"時速 -987 キロメートル"`).
12. **Notations.**
   - Scientific and engineering use the sub-pattern `"{number}{scientificSeparator}{scientificExponent}"` with the locale's exponent symbol. The exponent digits are transliterated into the numbering system like the number's.
   - Compact: ICU4X stores compact patterns as entries, each with the magnitude it starts at (CLDR's "type"), its exponent, and its patterns; consecutive magnitudes with compatible patterns are one entry, and magnitudes below the first entry are not compact. ComputeExponentForMagnitude takes the exponent of the last entry starting at or below the magnitude, and 0 if there is none (ja 1000 to 9999, de short thousands). Exponent 0 formats with `"{number}"`.
   - Compact pattern selection is ICU4X's (`PluralElements::get`): when the displayed number is exactly 0 or 1 (no fraction digits) and the entry has an exact-0 or exact-1 pattern, that pattern; otherwise the pattern for the plural category of the displayed number alone, with exponent 0 (ru long `1000` is `"1 тысяча"`, fr long `1000` is `"mille"`). Decision 10 decides when a pattern without `{0}` may be used.
   - Compact with style currency formats the compact number inside the standard currency pattern (`"$1.2K"`; `de` `1234567` with `EUR` is `"1,2 Mio. €"`, and `1234` is `"1234 €"`), the results ICU4C gives.
13. **Plural rules.** A `PluralRulesState` owns an ICU4X `PluralRulesWithRanges<PluralRules>` for a locale and type. Selection takes a significand string and an integer exponent and selects for the value significand × 10^exponent:
   - operands are built exactly with `RawPluralOperands` (`icu_plurals`, `unstable`) from the digits of the value, as UTS 35 defines `i`, `v`, `w`, `f` and `t`, so `v` and `f` keep trailing zeros (`"1.0"` is "other" in `en`). ICU4X's own conversions keep only 18 fraction digits, which changes results for longer fractions (bs `0.00000000000000000001` is "one"; `f = 1`). An integer operand longer than 18 digits keeps its last 18 digits plus 10^18, ICU4X's convention for `i`, so modulo and equality tests keep their results;
   - `c` (also `e`) is the exponent when it is positive and 0 otherwise. CLDR defines it only for compact and scientific exponents of large numbers;
   - Intl.PluralRules follows the draft: ResolvePlural rounds the value itself with FormatNumericToString, and PluralRuleSelect, which is implementation-defined, takes that string, computes the exponent `E` for the notation (scientific: the magnitude; engineering: the magnitude rounded down to a multiple of 3; compact: decision 12's exponent; standard, and any notation when the string is zero: 0), and calls the helper with the string's decimal point moved `E` places left (an exact digit shift, no rounding) and `E`, so the value is unchanged and only `c` is added (en ordinal scientific `21` sends `"2.1"` and 1, and selects "one"). So fr `1e6` and `1.5e6` in compact notation are "many" (`c = 6`), as test262 expects;
   - fr `1e-6` in scientific and engineering notation is "one" (`i = 0`, and fr's "one" rule `i = 0,1` matches first), where test262's `intl402/PluralRules/prototype/select/notation.js` expects "many". That test stays in the expectations file with the reason `CLDR defines the e operand only for non-negative exponents; for fr 1e-6 in scientific notation i = 0 selects "one", not test262's "many"`;
   - NumberFormat selects the plural form of unit and currency name patterns for what it displays: its rounded significand with the notation's exponent (ru long compact `1000` kilometers is `"1 тысяча километров"`, as in ICU4C). NumberFormat rounds the scaled significand and PluralRules the whole value, as the draft says, so the two can differ: ru `10001` in scientific notation displays `1E4` and NumberFormat selects "many", while `Intl.PluralRules("ru", {notation: "scientific"}).select(10001)` is "one". Compact patterns use decision 12's rule.
14. **Units.** `units.txt` holds every pattern of the 45 sanctioned units and the per-unit patterns, for widths long, short and narrow, and CLDR's dedicated compound units whose two halves are sanctioned (`en` has `kilometer-per-hour`, `liter-per-kilometer`, `meter-per-second`, `mile-per-gallon` and `mile-per-hour`). A `X-per-Y` unit uses, in this order:
    1. the dedicated compound unit, if the locale has one;
    2. Y's per-unit pattern with `{0}` replaced by X's pattern for the plural category;
    3. the width's compound pattern (`"{0}/{1}"`, `"{0} per {1}"`) with `{0}` replaced by X's pattern for the plural category and `{1}` by Y's "one" pattern with `{0}` and the whitespace next to it removed.

    This is ICU4C's order.
15. **Approximately and ranges.**
    - FormatApproximately inserts the `approximatelySign` part before the first `minusSign` or `plusSign` part. If there is none and the value is not negative, it inserts it where `{plusSign}` stands in the formatter's positive pattern (`"~$3"`, `"~3 €"`, and `"~$3.00"` for accounting, as ICU4C gives). If there is none and the value is negative (an accounting negative), it inserts it before the first part (`"~($3.00)"`). ICU4C omits the sign there, which the draft also allows (the sign may be empty); the plugin chooses to show it, so an approximate range never looks exact.
    - CollapseNumberRange splits each side into a prefix (the parts before the first numeric part), the numeric parts (`integer`, `group`, `decimal`, `fraction`, `exponentSeparator`, `exponentMinusSign`, `exponentInteger`, `nan`, `infinity`) and a suffix. It never collapses when either side has no numeric part (decision 10's patterns without `{0}`). Otherwise it collapses when the two sides have the same exponent (decision 12; 0 for standard notation), the two prefixes match, the two suffixes match, and the affix text has more than one code point. Parts match if they have the same type and, except for `unit`, `currency` and `compact` parts (whose text depends on the plural form, and which the exponent condition keeps from hiding a different power of ten: `en` long compact `3000` to `5000000` stays `"3 thousand – 5 million"`), the same value. PartitionNumberPattern therefore returns the exponent it used with the parts (Task 6). A collapsed range is the start's prefix, the start's numbers, the separator, the end's numbers and the end's suffix. Otherwise it is both sides in full, with a space added on each side of the separator that has none, if either side has any affix. These are ICU4C's rules for its default collapse mode, and they give test262's `"$3 – $5"`, `"+$2.90–3.10"` and `"3 - 5 €"`.
    - The separator is the text between `{0}` and `{1}` of the locale's range pattern.
16. **Caches**, all unobservable:
    - each native data request is cached in a null-prototype JS object keyed by its arguments, 64 entries per helper, emptied when full;
    - NumberFormat's internal plural-rules objects are cached per locale, 32 entries, emptied when full;
    - `Number.prototype.toLocaleString` and `BigInt.prototype.toLocaleString` reuse one default NumberFormat when `locales` and `options` are both `undefined`. Other calls create a NumberFormat through the internal creation function, which performs every observable step.
17. **`[[RelevantExtensionKeys]]` is « "nu" »**, the draft's. `[[LocaleData]].[[L]].[[nu]]` is the locale's default numbering system followed by the other entries of `numbering-systems.txt`, in that file's order. `Intl.PluralRules` has no relevant extension keys. Both use the plugin's one `[[AvailableLocales]]`.
18. **Legacy constructor behavior.** `%Intl%.[[FallbackSymbol]]` is one `Symbol("IntlLegacyConstructedSymbol")` created at setup. ChainNumberFormat and UnwrapNumberFormat are implemented, with OrdinaryHasInstance written in JS over the captured `Object.getPrototypeOf`. Only the `format` getter and `resolvedOptions` unwrap.
19. **The list separator** of Array and TypedArray `toLocaleString` is `","`, as Hermes's own `Array.prototype.toLocaleString` uses.

## Review Focus

These are the five inputs most likely to hurt someone that the spec implies but test262 does not exercise. Each one has a test in the task named.

1. **Extreme and odd values.** `1e308`, `5e-324`, `-0`, `2n ** 1000n`, `"0." + "0".repeat(1000) + "1"`, `"1e400"` (∞), `"-1e-400"` (-0), `" 12 "`, `"0x1F"`, `"-0x1F"` (NaN), `"1_000"` (NaN), `""` (0), and `maximumFractionDigits: 100`. Each formats exactly what the draft says, for example `new Intl.NumberFormat("en", {maximumFractionDigits: 100}).format("0." + "0".repeat(99) + "1")` ends in `"1"`. Nothing reaches Rust except plain decimal strings. Covered in Task 5 (`number-core.js`) and Task 6 (`number-format-values.js`).
2. **Numbering systems and scripts.** `bali` and `mathbold` (no ICU4X digits; `mathbold` digits are supplementary characters), `ar-EG` (default `arab`, signs with bidi marks), `fa`, `hi-u-nu-deva`, `ar-u-nu-bali` (symbols fall back to `latn`'s by decision 6). Expected: the digits of the table, `formatToParts` values concatenating to `format`, and groups never splitting a surrogate pair. Covered in Task 6 (`number-format-nu.js`) and Task 3 (Rust tests).
3. **Tampering after the plugin loads:**
   - accessors on `Object.prototype` for every option name and every internal record field name (`n`, `e`, `neg`, `type`, `value`, `source`, `pattern`);
   - a setter for `"0"` on `Array.prototype`;
   - replaced `Intl.NumberFormat`, its prototype methods and `format` getter, `Intl.PluralRules.prototype.select`, `Number.prototype.valueOf`, `BigInt.prototype.valueOf`, `BigInt`, `Math.max`, `String.prototype.indexOf`, `Symbol.toPrimitive` on `Number.prototype`.

   `format`, `formatToParts`, `formatRange`, `select` and every `toLocaleString` override give the same results regardless. Covered in Tasks 6, 10 and 11 (`tamper.js`).
4. **Objects that look like formatters:** `Object.create(Intl.NumberFormat.prototype)`, a Proxy around a NumberFormat, an `Intl.Collator`, a subclass instance, the legacy wrapper `Intl.NumberFormat.call(Object.create(Intl.NumberFormat.prototype))`, the detached bound `format`, and the same set for PluralRules. The `format` getter and `resolvedOptions` accept the wrapper and the subclass, and every other method rejects the wrapper with `TypeError`. No `Exception in HostFunction:` ever appears. Covered in Task 6 (`number-format-objects.js`) and Task 10 (`plural-rules-objects.js`).
5. **Data for every locale.** For each locale in `locales.txt`, with its default numbering system and with `latn`: format `-1234.5` in every style and display (each of the 45 units in all three widths, plus `kilometer-per-hour` and `byte-per-second`; currencies `USD`, `EUR`, `JPY`, `XXX` and `ABC` in all four displays and both signs), compact short and long for `1e3` to `1e15`, and plural categories, select and selectRange for cardinal and ordinal. Every lookup succeeds and returns non-empty patterns that contain the placeholders decision 5 promises; a plural pattern contains `{0}` at most once, and every "other" pattern contains it. Covered in Task 3 and Task 4 (Rust tests over every locale, which is faster than JS).

---

### Task 1: The CLDR number data files

**Files:**
- Modify: `intl/core/data/update-cldr-lists.py`, `intl/core/data/test_update_cldr_lists.py`
- Create: `intl/core/data/number-symbols.txt`, `intl/core/data/units.txt` (both generated), `intl/core/data/LICENSE`
- Modify: `intl/README.md` ("Updating the CLDR lists": the two new files and the JSON packages)

**Interfaces:**
- Produces two files. Every value is checked by the generator to contain no tab or newline, and generation fails if one does.
  - `number-symbols.txt`: one line per locale of `locales.txt` (and `und`) and per numbering system that its `numbers.json` has `symbols-numberSystem-<nu>` for:
    `<locale>\t<nu>\t<percentSign>\t<exponential>\t<infinity>\t<nan>\t<approximatelySign>\t<percentPattern>\t<rangePattern>`
    The percent pattern is CLDR's `percentFormats-numberSystem-<nu>` `standard` (`"#,##0%"`), and the range pattern is `miscPatterns-numberSystem-<nu>` `range` (`"{0}–{1}"`).
  - `units.txt`, in three sections:
    1. `strings <count>`, then each distinct pattern on its own line, numbered from 0 in order;
    2. `locale <tag> <parent>` lines, where `<parent>` is `-` for `und`;
    3. after each `locale` line, its entries `<width> <unit> <key>=<index>...`, with `<width>` one of `long`, `short`, `narrow`, `<key>` one of `zero`, `one`, `two`, `few`, `many`, `other`, `per`, and `<unit>` a sanctioned unit, a dedicated compound unit (decision 14), or `-` for the width's compound pattern (key `per`).

    A locale lists only the entries that differ from what its parent chain gives. The parent is CLDR's `parentLocales` entry, else the tag with its last subtag removed, else `und`, skipping tags that are not in the file.
- File keys are the tags of `locales.txt`. A tag with no directory in `cldr-*-full/main` is looked up with the script `likelySubtags` gives inserted (`zh-HK` is `zh-Hant-HK`); if that is missing too, generation fails naming the tag.

- [ ] **Step 1: Write the failing generator tests** in `test_update_cldr_lists.py`, on small fixture JSON written by the test (two locales with a parent, one with two numbering systems):
  - `test_number_symbols_lines`: the lines for both numbering systems, with the fields in the order above.
  - `test_units_dedup_against_parent`: a child whose `short-kilometer` equals the parent's has no entry for it, and one that differs has one.
  - `test_units_string_table`: equal patterns share one index, and the indices resolve to the patterns.
  - `test_units_compound_filter`: a dedicated compound unit with two sanctioned halves is kept, and one with an unsanctioned half is dropped.
  - `test_missing_locale_uses_likely_script`: a tag without a directory resolves through `likelySubtags`, and a tag that cannot resolve fails.
  - `test_rejects_tabs`: a value with a tab fails.
- [ ] **Step 2: Run them to see them fail.** `check-intl-data` fails: the functions do not exist.
- [ ] **Step 3: Implement the generator.** Fetch the three npm tarballs (`https://registry.npmjs.org/<package>/-/<package>-48.2.0.tgz`) to a temporary directory, derive `48.2.0` from the tag argument, and read `main/<locale>/numbers.json`, `main/<locale>/units.json`, `supplemental/parentLocales.json` and `supplemental/likelySubtags.json`. The sanctioned units are read from `SANCTIONED_UNITS` in `intl/js/10-Intl.js`, so there is one list.
- [ ] **Step 4: Run the generator** with `python3 intl/core/data/update-cldr-lists.py release-48-2`. Check:
  - the existing generated files are unchanged (`git diff --stat`);
  - `units.txt` is about 1 MB (record its size and line count in the report);
  - `en` short `kilometer` `other` resolves to `"{0} km"`, and `ja` long `kilometer-per-hour` `other` to `"時速 {0} キロメートル"`.
- [ ] **Step 5: Run `check-intl-data`.** Expected: pass.
- [ ] **Step 6: Commit** the generator, its tests, the two data files, `LICENSE` and the README change.

### Task 2: Plural rules in the Rust core and the JSI layer

**Files:**
- Modify: `intl/core/Cargo.toml` (`icu_plurals = { version = "~2.3", features = ["unstable"] }`, `icu_decimal = { version = "~2.3", features = ["unstable"] }`, `icu_experimental = "~0.6"`, `fixed_decimal = "~0.7"`), `intl/core/Cargo.lock` (only the additions these need; no ICU4X minor version may change)
- Create: `intl/core/src/plurals.rs`, `intl/jsi/PluralRulesHelpers.cpp`
- Modify: `intl/core/src/lib.rs`, `intl/jsi/NativeHelpers.h`, `intl/jsi/NativeHelpers.def`, `intl/jsi/RustCore.h`, `intl/CMakeLists.txt`, `intl/test/unit/CMakeLists.txt`
- Test: unit tests in `plurals.rs`; `intl/test/unit/NativeHelpersTest.cpp`

**Interfaces:**
- Produces:
  - `plurals::new_plural_rules(locale: &str, ordinal: bool) -> Option<PluralRulesWithRanges<PluralRules>>`;
  - `plurals::select(rules, significand: &str, exponent: i32) -> Option<PluralCategory>`: the category of significand × 10^exponent by decision 13. `significand` is ASCII digits with at most one `"."` and no sign; `None` for anything else;
  - `plurals::categories(rules) -> Vec<&'static str>`, in the order zero, one, two, few, many, other;
  - C interface: `HiPluralRules` (opaque, with a compile-time `Send` assertion), `hi_plural_rules_new(locale, len, ordinal: bool) -> *mut HiPluralRules`, `hi_plural_rules_free`, `hi_plural_rules_select(rules, decimal, len, exponent: i32, out: *mut HiString) -> bool`, `hi_plural_rules_select_range(rules, start, start_len, end, end_len, out) -> bool` (categories as strings), `hi_plural_rules_categories(rules, out) -> bool` (space-separated);
  - tags in `NativeHelpers.h`, documented in the `.def` header comment:

    | Tag | C++ type | Direction | Notes |
    |---|---|---|---|
    | `Int` | `int` | arguments too | an argument must be a JS number that is an integer in `int` range |
    | `Utf8List` | `std::vector<std::string>` | results only | a new JS array of strings |

  - `class PluralRulesState : public jsi::NativeState` in `PluralRulesHelpers.cpp`;
  - helper lines:
    - `HERMES_INTL_HELPER(pluralRulesInit, Undefined, Object, Utf8, Bool)`: `(obj, locale, ordinal)`; throws if creation fails or `obj` already has native state;
    - `HERMES_INTL_HELPER(pluralSelect, Utf8, Object, Utf8, Int)`: `(obj, decimal, exponent)`;
    - `HERMES_INTL_HELPER(pluralSelectRange, Utf8, Object, Utf8, Utf8)`: `(obj, startCategory, endCategory)`;
    - `HERMES_INTL_HELPER(pluralCategories, Utf8List, Object)`.

- [ ] **Step 1: Write the failing tests.**
  - Rust, in `plurals.rs`:
    - `cardinal_and_ordinal`: `en` cardinal `"1"` is one, `"1.0"` other, `"2"` other; `en` ordinal `"2"` two, `"3"` few, `"11"` other; `ar` cardinal `"0"` zero, `"2"` two, `"3"` few, `"11"` many;
    - `exponents`: `fr` `"1"` with exponent 6 is many, `"1.5"` with 6 many, `"1"` with 0 one, `"1"` with -6 one (`i = 0`); `ru` `"1"` with -3 is other;
    - `long_operands`: `bs` `"0.00000000000000000001"` is one (`f = 1`, past ICU4X's 18 digits); `ru` `"100000000000000000000001"` is one (`i % 10 = 1`, `i % 100 = 1`, 24 digits); `en` `"1"` followed by 30 zeros is other;
    - `ranges`: `en` (one, other) is other; `fr` (one, one) is one;
    - `categories_order`: `en` ordinal is `["one", "two", "few", "other"]`; `ar` cardinal is all six in order;
    - `rejects_bad_input`: `"-1"`, `"1e3"`, `"1.2.3"`, `""` and `"en_US"` as a locale give `None`.
  - Every locale in `locales.txt`, cardinal and ordinal: creation succeeds, `categories` ends in `"other"`, and selecting `"0"`, `"1"`, `"2.5"` succeeds (Review Focus 5, the plural half).
  - gtests in `NativeHelpersTest.cpp`:
    - `Utf8ListResult`: a test helper returning `{"a", "", "b"}` gives a JS array of length 3 with those strings;
    - `IntArgumentsAreChecked`: an `Int` helper called with `1.5`, `"1"` and `2 ** 40` throws an Error whose message names the helper;
    - `PluralRulesStateFreedWithRuntime`: `pluralRulesInit` then `pluralSelect(obj, "1", 0)` is `"one"`, then destroy the runtime; LeakSanitizer reports nothing;
    - `PluralHelpersRejectMisuse`: `pluralSelect` on an object without state, `pluralRulesInit` twice and `pluralSelect` with `"1e3"` each throw an Error containing `Intl internal error`.
- [ ] **Step 2: Run them to see them fail.** `check-intl-rust` fails to compile.
- [ ] **Step 3: Implement** `plurals.rs`, the C interface, the two tags, `PluralRulesState` and the four helpers. Validate the significand string, form the digits of significand × 10^exponent by moving the decimal point (no arithmetic and no `Decimal` parsing), compute `i`, `v`, `w`, `f` and `t` from those digits as UTS 35 defines them, with decision 13's 18-digit convention, set `c` by decision 13, and select with `RawPluralOperands`. Never use `PluralOperands::from_significand_and_exponent` or ICU4X's `Decimal` conversions here: they keep only 18 fraction digits and take a `u8` exponent.
- [ ] **Step 4: Run `check-intl-rust` and `check-intl-lit`.** Expected: pass; the lit tests are unchanged.
- [ ] **Step 5: Commit.**

### Task 3: Symbols, currency and compact data in the Rust core

**Files:**
- Create: `intl/core/src/number.rs`, `intl/core/src/cldr.rs` (parsing `number-symbols.txt` and `units.txt` with `include_str!`, behind `OnceLock`), `intl/jsi/NumberHelpers.cpp`
- Modify: `intl/core/src/lib.rs`, `intl/jsi/NativeHelpers.def`, `intl/jsi/RustCore.h`, `intl/CMakeLists.txt`, `intl/test/unit/CMakeLists.txt`
- Test: unit tests in `number.rs` and `cldr.rs`

**Interfaces:**
- Consumes: Task 1's `number-symbols.txt`; Task 2's `Utf8List` tag.
- Produces:
  - `number::symbols(locale: &str, nu: &str) -> Option<Vec<String>>`, the list below, by decision 6's fallback;
  - `number::currency_patterns(locale, nu, currency: &str, display: &str, accounting: bool) -> Option<Vec<String>>`;
  - `number::compact_patterns(locale, nu, long: bool) -> Option<Vec<String>>`;
  - `number::currency_digits(currency: &str) -> Option<u8>`;
  - C functions for each, returning the list as one string with fields separated by U+0000 (`HiString`), which `RustCore.h` turns into `std::vector<std::string>`;
  - helper lines:
    - `HERMES_INTL_HELPER(numberSymbols, Utf8List, Utf8, Utf8)`: `(locale, nu)`;
    - `HERMES_INTL_HELPER(currencyPatterns, Utf8List, Utf8, Utf8, Utf8, Utf8, Bool)`: `(locale, nu, currency, display, accounting)`;
    - `HERMES_INTL_HELPER(compactPatterns, Utf8List, Utf8, Utf8, Bool)`: `(locale, nu, long)`;
    - `HERMES_INTL_HELPER(currencyDigits, Int, Utf8)`.

  The lists:

  | Helper | Entries |
  |---|---|
  | `numberSymbols` | 0 decimal separator; 1 group separator; 2 minus sign; 3 plus sign; 4 percent sign; 5 exponent symbol; 6 infinity; 7 NaN; 8 approximately sign; 9 primary group size; 10 secondary group size; 11 minimum grouping (9 to 11 as decimal strings); 12-14 decimal zero, positive and negative patterns; 15-17 percent zero, positive and negative patterns; 18 range pattern in CLDR form (`"{0}–{1}"`) |
  | `currencyPatterns`, display `code`, `symbol` or `narrowSymbol` | 0-2 zero, positive and negative patterns with `{currencyPrefix}` or `{currencySuffix}`; 3 the currency string |
  | `currencyPatterns`, display `name` | 0-5 the name pattern for zero, one, two, few, many and other, in CLDR form with the plural name substituted (`"{0} US dollars"`); a category the locale lacks gets other's |
  | `compactPatterns` | 0 the number of entries `N`; then for each of ICU4X's entries (decision 12), in increasing magnitude, 10 strings: the magnitude it starts at, its exponent, the exact-0 and exact-1 patterns (empty when the entry has none), and the zero, one, two, few, many and other patterns, in CLDR form (`"{0}K"`; a pattern may lack `{0}`, as `fr` long's exact-1 `"mille"` does). A category the locale lacks gets other's |

  Decimal and percent patterns follow decision 11. A currency pattern uses ICU4X's alpha-next-to-number variant when the character of the currency string next to the number is neither a symbol (general category S) nor a separator (Z), as CLDR's currency spacing says. Display `code` uses the ISO code as the currency string, with the same spacing. A currency CLDR has no symbol or name for uses its code.

- [ ] **Step 1: Write the failing tests** in `number.rs` and `cldr.rs`:
  - `symbols_en_de_ar`:
    - `en`/`latn` gives `"."`, `","`, `"-"`, `"+"`, `"%"`, `"E"`, `"∞"`, `"NaN"`, `"~"`, `"3"`, `"3"`, `"1"`, `"{number}"`, `"{plusSign}{number}"`, `"{minusSign}{number}"`, `"{number}{percentSign}"`, `"{plusSign}{number}{percentSign}"`, `"{minusSign}{number}{percentSign}"` and `"{0}–{1}"`;
    - `de`/`latn` gives `","` and `"."`, and percent zero `"{number} {percentSign}"` with U+00A0;
    - `ar-EG`/`arab` has the Arabic separators, and `ar-EG`/`latn` the Latin ones;
    - `ar`/`bali` equals `ar`/`latn` (decision 6);
    - `en-IN` has primary 3 and secondary 2, and `pl` has minimum grouping 2.
  - `currency_patterns`:
    - `en` `USD` symbol: zero `"{currencyPrefix}{number}"`, string `"$"`;
    - accounting negative `"({currencyPrefix}{number})"`;
    - code: zero is `"{currencyPrefix}\u{a0}{number}"`, string `"USD"`;
    - `de` `EUR` symbol: zero `"{number}\u{a0}{currencySuffix}"`, string `"€"`;
    - `en` `USD` name: one is `"{0} US dollar"`, other `"{0} US dollars"`;
    - `ja` `USD` name: other is `"{0}米ドル"`, with no space (the locale's pattern);
    - `en` `ABC` symbol: string `"ABC"`;
    - narrowSymbol `en` `CAD` is `"$"`.
  - `compact_patterns`:
    - `en` short: the entries start at magnitudes 3, 6, 9 and 12, with exponents 3, 6, 9 and 12 and other patterns `"{0}K"`, `"{0}M"`, `"{0}B"` and `"{0}T"`;
    - `fr` long: the entry at magnitude 3 has the exact-1 pattern `"mille"`, one `"{0} millier"` and other `"{0} mille"` (check the strings against CLDR 48.2's `fr` `numbers.json` and use its exact values);
    - `de` short has no entry at magnitude 3, so thousands are not compact;
    - `ja` short: the first entry starts at magnitude 4, with exponent 4 and `"{0}万"`;
    - `en` long: magnitude 6 other is `"{0} million"`.
  - `currency_digits`: `USD` 2, `JPY` 0, `KWD` 3, `CLF` 4, `XXX` 2, `ABC` 2.
  - `every_locale_has_data` (Review Focus 5, this task's half): for every locale of `locales.txt`, with its default numbering system and with `latn`, `symbols` succeeds with 19 non-empty entries; every currency of the Review Focus in every display and sign succeeds and has the placeholders of the table; compact short and long succeed.
  - `rejects_bad_values`: an unknown display, a currency `"usd"` (JS sends uppercase) and the locale `"en_US"` give `None`.
- [ ] **Step 2: Run them to see them fail.** `check-intl-rust` fails to compile.
- [ ] **Step 3: Implement** `cldr.rs` (the parser for both generated files, used by Task 4 too; a malformed line is a panic, because the files are ours), `number.rs`, the C interface, `NumberHelpers.cpp` and the four helpers. Load ICU4X data through the baked providers (`icu::decimal::provider::Baked`, `icu_experimental::provider::Baked`) with `DataRequest`s carrying the numbering system as the marker attribute, as `info.rs` does for `DecimalSymbolsV1`. Convert CLDR's percent pattern syntax (`#`, `0`, `,`, `.`, `%`, quotes) to the placeholder form. Find the general category with `icu::properties`.
- [ ] **Step 4: Run `check-intl-rust` and `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Commit.**

### Task 4: Unit data in the Rust core

**Files:**
- Create: `intl/core/src/units.rs`
- Modify: `intl/core/src/lib.rs`, `intl/jsi/NativeHelpers.def`, `intl/jsi/NumberHelpers.cpp`, `intl/jsi/RustCore.h`
- Test: unit tests in `units.rs`

**Interfaces:**
- Consumes: Task 1's `units.txt`; Task 3's `cldr.rs` parser and `Utf8List` conversion.
- Produces:
  - `units::unit_patterns(locale: &str, unit: &str, width: &str) -> Option<Vec<String>>`: six patterns in CLDR form for zero, one, two, few, many and other (a missing category gets other's). `unit` is a sanctioned unit or `X-per-Y` of two sanctioned units, resolved by decision 14;
  - `hi_unit_patterns(...)` and `HERMES_INTL_HELPER(unitPatterns, Utf8List, Utf8, Utf8, Utf8)`: `(locale, unit, width)`.

- [ ] **Step 1: Write the failing tests** in `units.rs`:
  - `simple_units`: `en` short `kilometer` other is `"{0} km"`; `en` long `mile` one is `"{0} mile"` and other `"{0} miles"`; `de` long `byte` other is `"{0} Byte"`; `en` short `percent` other is `"{0}%"`; `en` narrow `celsius` other is `"{0}°C"`;
  - `long_meter_is_long`: `en` long `meter` other is `"{0} meters"`, the case ICU4X's own formatter gets wrong;
  - `dedicated_compound`: `en` short `kilometer-per-hour` other is `"{0} km/h"`; `ja` long `kilometer-per-hour` other is `"時速 {0} キロメートル"`;
  - `per_unit_pattern`: `en` short `kilobyte-per-hour` other is `"{0} kB/h"`, from `hour`'s per-unit pattern `"{0}/h"`; `en` long `byte-per-hour` one is `"{0} byte per hour"`;
  - `compound_pattern`: `en` long `byte-per-acre` other is `"{0} bytes per acre"`;
  - `inherits_from_parent`: a locale whose entries are all inherited (pick one with an empty section in `units.txt`) gives its parent's patterns;
  - `every_locale_every_unit` (Review Focus 5, the unit half): every locale × the 45 units × 3 widths, plus `kilometer-per-hour` and `byte-per-second`, gives six patterns, each containing `{0}` at most once, with "other" containing it;
  - `number_less_patterns`: `ar` long `meter` one is `"متر"`, CLDR's pattern without `{0}` (decision 10);
  - `rejects_bad_units`: `"furlong"`, `"meter-per-"`, `"meter-per-meter-per-meter"`, width `"Long"` give `None`.
- [ ] **Step 2: Run them to see them fail.** `check-intl-rust` fails to compile.
- [ ] **Step 3: Implement** `units.rs` and the helper.
- [ ] **Step 4: Run `check-intl-rust` and `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Commit.**

### Task 5: Intl mathematical values and rounding in JS

**Files:**
- Create: `intl/js/07-number.js`
- Modify: `intl/js/01-intrinsics.js` (capture `BigInt`, `Symbol.toPrimitive`, `Object.getPrototypeOf`, `Number.prototype.valueOf`, `BigInt.prototype.valueOf`), `intl/js/04-options.js` (renumber SetNumberFormatDigitOptions' step comments to the draft's refactored steps; the behavior does not change, and `test/lit/js/digit-options.js` keeps passing)
- Test: `intl/test/lit/js/number-core.js`, an isolation test built like `resolve-options.js`, concatenating `01-intrinsics.js` to `07-number.js`

**Interfaces:**
- Produces, all following decision 2 and 3:
  - `ToPrimitiveNumber(v)`: ECMA-262 ToPrimitive with hint number;
  - `ToIntlMathematicalValue(value)`;
  - `IntlMVFromBigInt(b)`, `IntlMVFromNumber(x)` for the overrides;
  - `FormatNumericToString(intlObj, x)` returning `{__proto__: null, roundedNumber, formattedString}`, with `intlObj` the record SetNumberFormatDigitOptions fills;
  - `ToRawPrecision(x, minPrecision, maxPrecision, unsignedRoundingMode)`, `ToRawFixed(x, minFraction, maxFraction, roundingIncrement, unsignedRoundingMode)`, `GetUnsignedRoundingMode(roundingMode, sign)`, `ApplyUnsignedRoundingMode` on the quotient and remainder;
  - `intlMVMagnitude(x)`: floor(log10(x)) of a non-zero record; `intlMVScale(x, k)`: x × 10^k; `intlMVIsZero(x)`; `intlMVNegate(x)`;
  - `ComputeExponentForMagnitude` and `ComputeExponent(intlObj, x, compactExponentOfMagnitude)`, where the last argument is a function from a magnitude to the compact exponent, used only for notation `"compact"`.

- [ ] **Step 1: Write the failing test.** Print each case with `JSON.stringify` (with a replacer that turns BigInts into strings) and check it with FileCheck:
  - ToIntlMathematicalValue:
    - `1.5` is `{neg: false, n: "15", e: -1}`; `-0` is `"negZero"`; `1e21` is `{n: "1", e: 21}`; `5e-324` is `{n: "5", e: -324}`;
    - `123n` and `-(2n ** 70n)` are exact;
    - the strings `" 12 "`, `"+1.50"`, `".5"`, `"5."`, `"1e400"` (`"posInf"`), `"-1e-400"` (`"negZero"`), `"-Infinity"`, `"0x1F"` (31), `"-0x1F"` (`"nan"`), `"0b101"`, `"1_000"` (`"nan"`), `""` (zero), `"  7 ﻿"` (7);
    - an object with `@@toPrimitive` returning `"2.5"`, and one with only `valueOf` returning `3n`; `@@toPrimitive` returning an object throws `TypeError`; a Symbol throws `TypeError`.
  - ToRawFixed and ToRawPrecision against the draft's examples and test262's rounding tables. Take the halfEven, halfExpand and ceil rows of `format-rounding-mode-*.js` (1.1, 1.15, 1.2, 1.25, 1.3, 1.35, -1.1, ...) at `maximumFractionDigits: 1`, and the increments of `format-rounding-increment-25.js` and `-5000.js`.
  - FormatNumericToString with `roundingPriority` `"morePrecision"` and `"lessPrecision"` on the cases of `format-rounding-priority-*.js`; `trailingZeroDisplay: "stripIfInteger"` on `1.00` and `1.50`; `minimumIntegerDigits: 3` on `5`.
  - Review Focus 1: `maximumFractionDigits: 100` on `"0." + "0".repeat(99) + "1"`; `2n ** 1000n` keeps all 302 digits; `1e308` gives 309 digits.
  - ComputeExponent for scientific (`123456` gives 5, `0.00123` gives -3), engineering (`123456` gives 3) and `999.99` with `maximumFractionDigits: 0` in scientific (gives 3, the "999 rounds to 1000" case).
- [ ] **Step 2: Run it to see it fail.** `LIT_FILTER=number-core` with `check-intl-lit` fails: the functions are not defined.
- [ ] **Step 3: Implement `07-number.js`** step by step from `spec/numberformat.html`, and renumber SetNumberFormatDigitOptions' comments.
- [ ] **Step 4: Run `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Commit.**

### Task 6: Intl.NumberFormat with decimal and percent styles

**Files:**
- Create: `intl/js/40-NumberFormat.js` (constructor, slots, options, resolvedOptions, supportedLocalesOf, legacy behavior, the methods), `intl/js/41-number-patterns.js` (the digit table, data requests and caches, GetNumberFormatPattern, PartitionNumberPattern, PartitionNotationSubPattern, grouping)
- Modify: `intl/js/10-Intl.js` (call `installNumberFormat(IntlObject)`; create the fallback symbol at setup), `intl/test/test262-expectations.txt`
- Test:
  - Create: `intl/test/lit/number-format.js`, `number-format-values.js`, `number-format-nu.js`, `number-format-objects.js`
  - Modify: `shapes-table.js`, `tamper.js`

**Interfaces:**
- Consumes: Task 3's `numberSymbols`, Task 5's functions, `ResolveOptions`, `FilterLocales`, `SetNumberFormatDigitOptions`, `GetBooleanOrStringNumberFormatOption`, the shape helpers.
- Produces (for Tasks 7 to 11):
  - `NumberFormatSlots`, a `ReturnOverride` subclass with one private field per draft slot plus `#data` (the formatter's data, decision 5), and static `is(o)` and `get(o, name)`;
  - `CreateNumberFormat(locales, options)`: a new initialized NumberFormat from the intrinsic prototype, the draft's constructor steps with `newTarget` fixed, used by the overrides;
  - `PartitionNumberPattern(nf, x)` returning `{__proto__: null, parts, exponent}`: `parts` is a list of `{__proto__: null, type, value}`, and `exponent` the one ComputeExponent chose (0 for NaN, infinities and standard notation). FormatNumeric and FormatNumericToParts use `parts`; Task 9's range operations use both;
  - `FormatNumeric(nf, x)`, `FormatNumericToParts(nf, x)`;
  - `NUMBERING_SYSTEM_DIGITS`: numbering system to its ten digits as strings, every row of the draft's simple-digit table;
  - the style hooks Task 7 fills in: GetNumberFormatPattern's currency and unit branches throw `TypeError("Intl internal error: style not implemented")` until then, and the constructor accepts those styles' options already.

- [ ] **Step 1: Write the failing lit tests.** Run each with `%hermes -Xintl-default-locale=en-US %s`.
  - `number-format.js`, the basic surface:
    - `resolvedOptions()` of `new Intl.NumberFormat()` prints, in the draft's order, `{"locale":"en-US","numberingSystem":"latn","style":"decimal","minimumIntegerDigits":1,"minimumFractionDigits":0,"maximumFractionDigits":3,"useGrouping":"auto","notation":"standard","signDisplay":"auto","roundingIncrement":1,"roundingMode":"halfExpand","roundingPriority":"auto","trailingZeroDisplay":"auto"}`;
    - `format(1234567.891)` is `"1,234,567.891"`; `de` gives `"1.234.567,891"`; `en-IN` gives `"12,34,567.891"`;
    - `formatToParts(-1234.5)` is minusSign, integer `"1"`, group, integer `"234"`, decimal, fraction `"5"`;
    - `style: "percent"`: `0.256` is `"26%"`, `de` is `"26 %"` with U+00A0;
    - `useGrouping` `false`, `"always"` (`pl` 1234 is `"1 234"`), `"min2"` (`en` 1234 is `"1234"`);
    - `signDisplay` `"exceptZero"` on `0`, `-0` and `1`;
    - NaN, Infinity and -Infinity;
    - `Intl.NumberFormat("de")` without `new` is a NumberFormat; `supportedLocalesOf(["de", "xx"])` is `["de"]`; `format` returns the same function each time;
    - `Object.prototype.toString.call(new Intl.NumberFormat())` is `"[object Intl.NumberFormat]"`.
  - `number-format-values.js`: Review Focus 1, through `format` and `formatToParts`.
  - `number-format-nu.js`: Review Focus 2.
  - `number-format-objects.js`: Review Focus 4, the NumberFormat half, including ChainNumberFormat's property descriptor on the wrapper.
  - `shapes-table.js`: these rows, and a `bound` row for holder `new Intl.NumberFormat()`, property `format`, length 1.

    | Holder | Property | Kind | Name | Length |
    |---|---|---|---|---|
    | `Intl` | `NumberFormat` | constructor | `NumberFormat` | 0 |
    | `Intl.NumberFormat` | `supportedLocalesOf` | method | `supportedLocalesOf` | 1 |
    | `Intl.NumberFormat.prototype` | `resolvedOptions` | method | `resolvedOptions` | 0 |
    | `Intl.NumberFormat.prototype` | `format` | getter | `get format` | |
    | `Intl.NumberFormat.prototype` | `formatToParts` | method | `formatToParts` | 1 |
    | `Intl.NumberFormat.prototype` | `formatRange` | method | `formatRange` | 2 |
    | `Intl.NumberFormat.prototype` | `formatRangeToParts` | method | `formatRangeToParts` | 2 |
    | `Intl.NumberFormat.prototype` | `@@toStringTag` | toStringTag | `Intl.NumberFormat` | |

  - `tamper.js`: Review Focus 3, the NumberFormat half, except `formatRange`, which Task 9 adds.
- [ ] **Step 2: Run them to see them fail.** `LIT_FILTER=number-format` with `check-intl-lit` fails: `Intl.NumberFormat` is undefined.
- [ ] **Step 3: Implement `40-NumberFormat.js` and `41-number-patterns.js`.**
  - Write these step by step from `spec/numberformat.html`: the constructor and ChainNumberFormat, SetNumberFormatUnitOptions, `supportedLocalesOf`, `resolvedOptions` with UnwrapNumberFormat, the `format` getter, `formatToParts`, CurrencyDigits, PartitionNumberPattern, PartitionNotationSubPattern, GetNumberFormatPattern, FormatNumeric, FormatNumericToParts, UnwrapNumberFormat and `@@toStringTag`.
  - `formatRange` and `formatRangeToParts` are installed now with their RequireInternalSlot, undefined checks and conversions, and call FormatNumericRange, which Task 9 writes; until then it throws `TypeError("Intl internal error: ranges not implemented")`.
  - The formatter's data is requested once, at the end of the constructor, through the data cache (decision 16).
  - Grouping follows decision 9 on the ASCII integer digits, before transliteration, so supplementary digits are never split.
- [ ] **Step 4: Run `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Update the expectations.** The lines of the decimal- and percent-style tests that now pass go: the constructor, option, `resolvedOptions`, `supportedLocalesOf`, prototype-property, `format` and `formatToParts` tests that use neither currency, unit, a non-standard notation nor a range. List them in the report. Then run `check-intl-test262`. Expected: `0 problem(s)`.
- [ ] **Step 6: Commit.**

### Task 7: Currency and unit styles

**Files:**
- Modify: `intl/js/40-NumberFormat.js`, `intl/js/41-number-patterns.js`, `intl/test/test262-expectations.txt`
- Test: Create `intl/test/lit/number-format-currency.js`, `number-format-unit.js`, `number-format-gc.js`

**Interfaces:**
- Consumes: Task 2's plural helpers, Task 3's `currencyPatterns` and `currencyDigits`, Task 4's `unitPatterns`, Task 6's hooks.
- Produces:
  - GetNumberFormatPattern's currency and unit branches;
  - the splitting of decision 10 (`splitPluralPattern(pattern, partType)`), which Task 8 reuses for compact patterns;
  - `internalPluralRules(locale, type)` in `41-number-patterns.js`: an internal null-prototype object with a `PluralRulesState`, cached per locale and type (decision 16), never reachable from user code;
  - `numberFormatPlural(nf, formattedString, exponent)`: the category for a formatter's locale, through `internalPluralRules` (decision 13).

- [ ] **Step 1: Write the failing lit tests.**
  - `number-format-currency.js`:
    - `en` `USD`: `1234.5` is `"$1,234.50"`; accounting `-1` is `"($1.00)"` and `-0` is `"($0.00)"`; code is `"USD 1,234.50"` with U+00A0; name `1` is `"1.00 US dollars"` and `formatToParts` has literal `" "` and currency `"US dollars"`;
    - `de` `EUR` is `"1.234,50 €"`; `ja` `JPY` has no fraction digits;
    - `ja` `USD` name has no space, from the locale's pattern;
    - `en` `ABC` is `"ABC 1.00"`; `minimumFractionDigits: 0` with `USD` gives `"$1"` for `1`;
    - `currencyDisplay: "narrowSymbol"` with `CAD` gives `"$1.00"` in `en`.
  - `number-format-unit.js`:
    - `en` short `kilometer-per-hour` `-987` is `"-987 km/h"`, narrow `"-987km/h"`;
    - `ja` long gives `"時速 -987 キロメートル"` and the parts of decision 10;
    - `en` long `mile`: `1` is `"1 mile"`, `1.5` is `"1.5 miles"`, `1.0` with `minimumFractionDigits: 1` is `"1.0 miles"`;
    - `byte-per-second` and `percent` (`"50%"`);
    - `style: "unit"` without `unit` throws `TypeError`, and `unit: "furlong"` throws `RangeError`.
  - `number-format-gc.js`: 2,000 NumberFormats alternating currency name and unit styles in a loop (both plural-dependent, so each uses a plural-rules object), `gc()` every 100, every tenth kept; format with the survivors afterwards; end with live formatters. ASan and LeakSanitizer report nothing.
- [ ] **Step 2: Run them to see them fail.** The currency and unit branches throw the internal-error `TypeError`.
- [ ] **Step 3: Implement** the branches, and the plural selection of decision 13 for unit and currency name patterns.
- [ ] **Step 4: Run `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Update the expectations.** The currency, unit, `units.js`, `currency-*`, `style-unit.js`, `signDisplay-currency-*` and `signDisplay-negative-currency-*` lines, and `Intl/supportedValuesOf/currencies-accepted-by-NumberFormat.js` and `units-accepted-by-NumberFormat.js`, go if they pass. Then run `check-intl-test262`. Expected: `0 problem(s)`.
- [ ] **Step 6: Commit.**

### Task 8: Scientific, engineering and compact notation

**Files:**
- Modify: `intl/js/40-NumberFormat.js`, `intl/js/41-number-patterns.js`, `intl/test/test262-expectations.txt`
- Test: Create `intl/test/lit/number-format-notation.js`

**Interfaces:**
- Consumes: Task 3's `compactPatterns`, Task 5's ComputeExponent, Task 7's `splitPluralPattern`.
- Produces: GetNotationSubPattern, the compact exponent table of a formatter (`compactExponentOfMagnitude(nf)`, used by Task 10 too), and the notation branches of PartitionNotationSubPattern.

- [ ] **Step 1: Write the failing lit test** `number-format-notation.js`:
  - scientific `en`: `123456` is `"1.235E5"`, `0.000123` is `"1.23E-4"`, and its parts are integer, decimal, fraction, exponentSeparator, exponentMinusSign, exponentInteger;
  - engineering `en`: `123456` is `"123.456E3"`;
  - scientific with `ar-EG` transliterates the exponent (decision 12);
  - compact short `en`: `1234` is `"1.2K"`, `999999` is `"1M"`, `1650` is `"1.7K"` (halfExpand, not ICU4X's half-even), `1e16` is `"10,000T"`;
  - compact long `en`: `1234` is `"1.2 thousand"`, with literal `" "` and compact `"thousand"`;
  - compact `ja`: `9876` is `"9876"` and `98765` is `"9.9万"`;
  - compact with currency: `en` `USD` `1234` is `"$1.2K"`; `de` `EUR` `1234567` is `"1,2 Mio. €"` and `1234` is `"1234 €"`;
  - compact long `ru` `1000` is `"1 тысяча"`, and with style unit `kilometer` long `"1 тысяча километров"` (decisions 12 and 13); `fr` long `1000` is `"mille"`, a single compact part, and `-1000` is `"-1 millier"` (decision 10);
  - compact defaults `useGrouping` to `"min2"` (`resolvedOptions`).
- [ ] **Step 2: Run it to see it fail.**
- [ ] **Step 3: Implement** the notations, decision 12.
- [ ] **Step 4: Run `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Update the expectations.** The `engineering-scientific-*`, `notation-compact-*` and `constructor-notation.js` lines and any other notation test that now passes go. Then run `check-intl-test262`. Expected: `0 problem(s)`.
- [ ] **Step 6: Commit.**

### Task 9: formatRange and formatRangeToParts

**Files:**
- Modify: `intl/js/41-number-patterns.js`, `intl/test/test262-expectations.txt`
- Test: Create `intl/test/lit/number-format-range.js`; modify `tamper.js`

**Interfaces:**
- Consumes: Task 6's PartitionNumberPattern and the range pattern of `numberSymbols`.
- Produces: PartitionNumberRangePattern, FormatApproximately, CollapseNumberRange (decision 15), FormatNumericRange, FormatNumericRangeToParts.

- [ ] **Step 1: Write the failing lit test** `number-format-range.js`:
  - test262's `en-US` and `pt-PT` cases (`"$3 – $5"`, `"~$3"`, `"+$2.90–3.10"`, `"3 - 5 €"`, `"~3 €"`), restated so the plugin's own suite pins them;
  - `en` short `kilometer` `3` to `5` is `"3–5 km"`; long `mile` `1` to `5` is `"1–5 miles"`;
  - percent `3` to `5` (`0.03`, `0.05`) is `"3% – 5%"`;
  - `-5` to `5` is `"-5 – 5"`; `5` to `3` does not throw;
  - `formatRangeToParts` sources: shared, startRange, endRange as decision 15 assigns them;
  - a NaN argument throws `RangeError`, an undefined one `TypeError`;
  - `en` long compact `3000` to `5000000` is `"3 thousand – 5 million"`, not collapsed (decision 15);
  - accounting `USD`: `formatRange(3, 3)` is `"~$3.00"` and `formatRange(-3, -3)` is `"~($3.00)"`;
  - `tamper.js`: Review Focus 3, the `formatRange` half.
- [ ] **Step 2: Run it to see it fail.** It throws the internal-error `TypeError` of Task 6.
- [ ] **Step 3: Implement** the range operations, step by step from the draft, and decision 15.
- [ ] **Step 4: Run `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Update the expectations.** The `formatRange` and `formatRangeToParts` lines go. Then run `check-intl-test262`. Expected: `0 problem(s)`.
- [ ] **Step 6: Commit.**

### Task 10: Intl.PluralRules

**Files:**
- Create: `intl/js/45-PluralRules.js`
- Modify: `intl/js/10-Intl.js` (call `installPluralRules(IntlObject)`), `intl/test/test262-expectations.txt`
- Test:
  - Create: `intl/test/lit/plural-rules.js`, `plural-rules-objects.js`, `plural-rules-gc.js`
  - Modify: `shapes-table.js`, `tamper.js`

**Interfaces:**
- Consumes: Task 2's helpers, Task 5's FormatNumericToString and ComputeExponent, Task 8's compact exponent table (for notation `"compact"`, built from `compactPatterns` with the locale's default numbering system, since PluralRules has no `nu`).
- Produces: `PluralRulesSlots`, ResolvePlural, ResolvePluralRange. Each PluralRules instance attaches its own `PluralRulesState` with `pluralRulesInit`.

- [ ] **Step 1: Write the failing lit tests.**
  - `plural-rules.js`:
    - `resolvedOptions()` of `new Intl.PluralRules("en")` prints `{"locale":"en","type":"cardinal","notation":"standard","minimumIntegerDigits":1,"minimumFractionDigits":0,"maximumFractionDigits":3,"pluralCategories":["one","other"],"roundingIncrement":1,"roundingMode":"halfExpand","roundingPriority":"auto","trailingZeroDisplay":"auto"}`;
    - `select(1)` is `"one"`, `select("1.0")` is `"other"` with `minimumFractionDigits: 1`, and `select(NaN)` is `"other"`;
    - `en` ordinal `select(22)` is `"two"`;
    - `ar` `pluralCategories` is all six in order;
    - test262's `fr` notation table, restated with decision 13's "one" for `1e-6` in scientific and engineering notation;
    - `bs` with `maximumFractionDigits: 20`: `select("0.00000000000000000001")` is "one";
    - `en` ordinal scientific `select(21)` is "one"; `ru` scientific `select(10001)` is "one" while `new Intl.NumberFormat("ru", {notation: "scientific", style: "unit", unit: "meter", unitDisplay: "long"}).format(10001)` uses the "many" pattern (decision 13);
    - `selectRange(1, 5)` is `"other"` in `en`, and `fr` (1, 1.5) is `"one"`;
    - `Intl.PluralRules()` without `new` throws `TypeError`.
  - `plural-rules-objects.js`: Review Focus 4, the PluralRules half.
  - `plural-rules-gc.js`: as `number-format-gc.js`, with 2,000 PluralRules.
  - `shapes-table.js` rows:

    | Holder | Property | Kind | Name | Length |
    |---|---|---|---|---|
    | `Intl` | `PluralRules` | constructor | `PluralRules` | 0 |
    | `Intl.PluralRules` | `supportedLocalesOf` | method | `supportedLocalesOf` | 1 |
    | `Intl.PluralRules.prototype` | `resolvedOptions` | method | `resolvedOptions` | 0 |
    | `Intl.PluralRules.prototype` | `select` | method | `select` | 1 |
    | `Intl.PluralRules.prototype` | `selectRange` | method | `selectRange` | 2 |
    | `Intl.PluralRules.prototype` | `@@toStringTag` | toStringTag | `Intl.PluralRules` | |

  - `tamper.js`: Review Focus 3, the PluralRules half.
- [ ] **Step 2: Run them to see them fail.** `Intl.PluralRules` is undefined.
- [ ] **Step 3: Implement `45-PluralRules.js`** step by step from `spec/pluralrules.html`. ResolvePlural is the draft's; PluralRuleSelect computes the exponent from the formatted string for non-standard notations (decision 13).
- [ ] **Step 4: Run `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Update the expectations.** Every `Phase 3: Intl.PluralRules` line goes, except `intl402/PluralRules/prototype/select/notation.js`, whose reason becomes decision 13's text. Then run `check-intl-test262`. Expected: `0 problem(s)`.
- [ ] **Step 6: Commit.**

### Task 11: The toLocaleString overrides

**Files:**
- Modify: `intl/js/80-builtins.js`, `intl/js/01-intrinsics.js` (capture `%TypedArray%.prototype` and its `at` method and `length` getter), `intl/test/test262-expectations.txt`
- Test:
  - Create: `intl/test/lit/to-locale-string.js`
  - Modify: `shapes-table.js`, `tamper.js`

**Interfaces:**
- Consumes: Task 6's `CreateNumberFormat` and `FormatNumeric`, Task 5's `IntlMVFromNumber` and `IntlMVFromBigInt`.
- Produces: the four overrides on `Number.prototype`, `BigInt.prototype`, `Array.prototype` and `%TypedArray%.prototype`, and JoinIndexedProperties.

- [ ] **Step 1: Write the failing lit test** `to-locale-string.js`:
  - `(1234.5).toLocaleString("de")` is `"1.234,5"`; `(1234.5).toLocaleString()` is `"1,234.5"`; `(5).toLocaleString("en", {style: "percent"})` is `"500%"`;
  - `12345678901234567890n.toLocaleString("en")` keeps every digit;
  - `Number.prototype.toLocaleString.call("1")` and `BigInt.prototype.toLocaleString.call(1)` throw `TypeError`;
  - `[1234.5, null, undefined, 7n, {toLocaleString(l, o) { return l + "/" + o.style; }}].toLocaleString("de", {style: "decimal"})` is `"1.234,5,,,7,de/decimal"`;
  - `new Float64Array([1234.5, 2]).toLocaleString("de")` is `"1.234,5,2"`; a detached typed array throws `TypeError`, as ValidateTypedArray requires (perform it by calling the captured `%TypedArray%.prototype.at` with `0`, which validates first and has no other effect);
  - shapes rows, all kind `method`: `Number.prototype` `toLocaleString` length 0, `BigInt.prototype` `toLocaleString` 0, `Array.prototype` `toLocaleString` 0, `Object.getPrototypeOf(Int8Array.prototype)` `toLocaleString` 0;
  - `tamper.js`: Review Focus 3, the overrides half, including a replaced `Intl.NumberFormat` (the overrides must not look it up).
- [ ] **Step 2: Run it to see it fail.** Hermes's own methods ignore `locales`.
- [ ] **Step 3: Implement** the overrides step by step from `spec/locale-sensitive-functions.html` and ECMA-262's JoinIndexedProperties, with decision 16's default formatter and decision 19's separator.
- [ ] **Step 4: Run `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Update the expectations.** The Number, BigInt and Array `toLocaleString` lines go. The `built-ins` tests of the four methods that pass now must still pass. Then run `check-intl-test262`. Expected: `0 problem(s)`.
- [ ] **Step 6: Commit.**

### Task 12: Acceptance, spec and README

**Files:**
- Modify:
  - `doc/superpowers/specs/2026-10-22-intl-plugin-design.md`:
    - "The JS layer": the new files, decision 1, the caches of decision 16, and the legacy behavior of decision 18;
    - "The native interface": the `Int` argument and `Utf8List` tags, `PluralRulesState`, the plural and number helpers, decision 13, and the change to the "Native state" example;
    - "Parts": decision 10;
    - "Data": decisions 4 to 9, 11, 12, 14, 15, 17 and 19, and the library size before and after this phase;
  - `intl/README.md`:
    - the layout table names `number.rs`, `units.rs`, `plurals.rs`, `cldr.rs`, `NumberHelpers.cpp`, `PluralRulesHelpers.cpp`, `07-number.js`, `40-NumberFormat.js`, `41-number-patterns.js`, `45-PluralRules.js` and the two data files;
    - "Adding a native helper" lists the new tags.

- [ ] **Step 1: Run everything.** Run `check-intl`, then a full `check-hermes`. Expected:
  - every part of `check-intl` passes, with test262 at `0 problem(s)`;
  - `check-hermes` has 0 unexpected results.

  Record the counts in the report.
- [ ] **Step 2: Check the expectations file.**
  - `grep -c 'Phase 3' intl/test/test262-expectations.txt` is 0. `intl402/PluralRules/prototype/select/notation.js` keeps its line, with the reason decision 13 gives (Task 10 writes it), and no other line remains for a Phase 3 test unless the controller ruled on it by name.
  - `git diff <Phase 3's plan commit> -- intl/test/test262-expectations.txt` only removes lines, apart from the new reason of `select/notation.js`.
- [ ] **Step 3: Measure the library.** Build the `hermesintl` target in Release, in a temporary worktree at `9e8eb068e` (the Phase 2 commit) and at the tip, each with its own build directory configured with Clang and `-DCMAKE_BUILD_TYPE=Release`. Record both sizes of `lib/libhermesintl.so` (and stripped), then remove the worktree and the build directories.
- [ ] **Step 4: Update the spec and README** as listed above, in the documents' existing style. Write in plain English, with no jargon of our own.
- [ ] **Step 5: Commit.**

## After the tasks

When the whole-branch review is done and its findings are fixed:
1. Run a Codex code review (the `codex-review` skill) of the Phase 3 commits, from the plan commit to the tip.
2. Fix or drop every finding, with a reason. Never defer one.
3. Repeat until Codex reports every finding CLOSED.
4. Rerun `check-intl` and `check-hermes` on Linux, then ask the Mac agent (gist channel) for the same runs on macOS, and fix what it finds.
5. Squash the branch with the `squashing-feature-history` skill (fixes to earlier phases fold into those phases). If the new tip's tree is identical to the backup tag's, delete the backup.
