# Intl Plugin Phase 2 (Collator and case mapping) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `Intl.Collator` to the plugin with its whole public surface, and replace `String.prototype.localeCompare`, `toLocaleLowerCase` and `toLocaleUpperCase` with their ECMA-402 versions:
- `Intl.Collator`: the constructor and its options, `supportedLocalesOf`, `resolvedOptions`, the `compare` getter and its bound function;
- the String overrides, which use the Collator internals and ICU4X case mapping.

At the end, every test262 test the expectations file assigns to Phase 2 passes, except `intl402/Collator/usage-de.js`, which needs data ICU4X does not ship (decision 4).

**Architecture:**
- The Rust core gains a collator and a case mapper, both on ICU4X 2.3's compiled data.
- The collator is the plugin's first native state: a Rust `CollatorBorrowed<'static>` behind an opaque pointer, owned by a `jsi::NativeState` attached to the Collator instance.
- `NativeHelpers.h` gains the conversion tags this needs: object arguments, boolean arguments, UTF-16 strings both ways, integer results and `undefined` results.
- The ECMA-402 algorithms are JS:
  - the shared `ResolveOptions` goes in `03-locales.js`;
  - `Intl.Collator` goes in `intl/js/30-Collator.js`;
  - the String overrides go in `intl/js/80-builtins.js`, which later phases extend.

**Tech Stack:** Rust 1.88 with ICU4X 2.3 (`icu` with `compiled_data` and `sync`), C++17 and JSI, the plugin's JS compiled to Hermes bytecode, and test262 at `c9aa6126e8f6fe2c6f668c65bfba771ffc8a6096`.

**Spec:** `doc/superpowers/specs/2026-10-22-intl-plugin-design.md`. Read "Guiding rules", "The JS layer", "The native interface" (including "Threads" and "Strings"), "Data", "Testing" and "Phasing" before any task. Task 7 writes this plan's decisions into it.

**ECMA-402 text:** the https://tc39.es/ecma402/ draft dated October 24, 2026, which is tc39/ecma402 commit `3691634ebc` ("Editorial: Refactor SetNumberFormatDigitOptions for comprehensibility (#1050)"). The relevant files are:
- `spec/collator.html`: the constructor, `[[ResolutionOptionDescriptors]]`, the locale data constraints and CompareStrings;
- `spec/negotiation.html`: ResolveOptions;
- `spec/locale-sensitive-functions.html`: `localeCompare` and TransformCase.

Cite its section titles and number its steps. Fetch the text with `gh api -H 'Accept: application/vnd.github.raw' 'repos/tc39/ecma402/contents/spec/<file>?ref=3691634ebc'`.

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
  - no line may be added. The one reason Task 4 rewrites by hand is the only other change allowed.
- **Git:** at the end of each task, commit that task's files on `hermes-intl` with a plain-English message that says what changed and why. End the message with:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01JPBWtUGtZ5QcTC9dE7h3R8
  ```
  Never push, amend or reset.
- **Copyright header:** every new file gets the Meta MIT header used across the repo.
- **Separability:** code under `intl/` includes only `<jsi/...>`, `<hermes/Public/IntlPlugin.h>`, its own headers and the generated `hermes_intl_core.h`. The one exception is `intl/test/unit/`.
- **Plugin JS rules:** `intl/README.md`, "Writing plugin JS", rules 1 to 6. Reviewers check every line against them. In particular:
  - no `String.prototype.split`, no regular expressions, no spread, and no `Array.prototype` methods;
  - every array is built with `listPush`;
  - every record and descriptor has `__proto__: null`;
  - an optional parameter is a default parameter (`localeCompare(that, locales = undefined, options = undefined)`), which keeps `length` at the spec value. Never read `arguments[i]` past `arguments.length`: that would reach `Object.prototype`.
- **Native helpers:** `intl/README.md`, "Adding a native helper". JS validates every argument before calling a helper. A helper that gets an argument JS should never send throws `std::logic_error("Intl internal error: ...")`.
- **Rust:**
  - FFI functions never panic on bad input. Invalid UTF-8, an unknown option string, or a tag that does not parse returns `false` or a null pointer.
  - A panic is only for a broken invariant in our own data files.
  - Every new public function has a doc comment and a unit test.
  - Run `cargo fmt` in `intl/core`, and `clang-format` on changed C++.
- **Threads** (spec, "Threads"): a Rust type held in a `NativeState` must be `Send`, checked at compile time. A `NativeState` destructor frees only its own Rust object.
- **Prove checks can fail:** for every new test, make a stated mutation of the code under test, name the test that fails, then revert the mutation. Put this evidence in the task report.
- **Never bump `BYTECODE_VERSION`**, and never bump the plugin-interface version. Phase 2 adds native helpers and changes no entry-point, config or `IHermesBuiltinBytecode` interface.

## Planning decisions

Task 7 writes these into the spec. Implement them as stated.

1. **`%Intl.Collator%.[[RelevantExtensionKeys]]` is « "co", "kf", "kn" »**, the keys Phase 1's `[[LocaleExtensionKeys]]` assumed. `[[ResolutionOptionDescriptors]]` is the draft's: `co`/`collation`; `kn`/`numeric` (boolean); `kf`/`caseFirst` with values « "upper", "lower", "false" ».
2. **Locale data.** For every available locale `L`:

   | Field | Value |
   |---|---|
   | `[[co]]` | « null » followed by `collationsOfLocale(L)`, the native helper `getCollations` uses, so the two agree (spec, "The native interface") |
   | `[[kn]]` | « "false", "true" » |
   | `[[kf]]` | ICU4X's default case-first for `L` first, then the other two of « "upper", "lower", "false" ». The default is "upper" for `da` and `mt`, "false" for the others. |
   | `[[ignorePunctuation]]` | whether ICU4X's default alternate handling for `L` is shifted (`true` for `th`) |
   | `[[sensitivity]]` | "variant" (only `[[SearchLocaleData]]` has it) |

   `[[SortLocaleData]]` and `[[SearchLocaleData]]` are otherwise the same. The defaults come from one native helper, `collatorLocaleDefaults`, which reads ICU4X's resolved options for a collator made with no options.
3. **Collators use `[[AvailableLocales]]`**, the plugin's one list (`isAvailableLocale`), as `getCollations` already does.
4. **No search collations.** ICU4X's compiled data has no "search" tailorings (checked in `icu_collator_data` 2.3.0), and the spec rules out a datagen step. So `usage: "search"` uses the sort tailoring, with its own default sensitivity. `intl402/Collator/usage-de.js` therefore keeps failing. Its reason becomes `ICU4X's compiled data has no search collations, so usage "search" sorts as "sort"`.
5. **Options to ICU4X.** Each value is passed explicitly, never through `-u-` keywords in the tag:

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

   Only the language identifier of `[[Locale]]` picks the tailoring (`crate::locale::language_id`).
6. **Strings.** CompareStrings uses ICU4X's `compare_utf16`, which reads an unpaired surrogate as U+FFFD (checked). Case mapping splits the text at unpaired surrogates, maps each well-formed run with ICU4X's `CaseMapper`, and copies each unpaired surrogate through unchanged (spec, "Strings").
7. **TransformCase's available locales are « "az", "el", "hy", "lt", "tr" »**: the languages ICU4X's `CaseMapper` tailors for upper and lower case. `nl` is left out because it tailors only titlecase. A tag matches by prefix (`tr-TR` gives `tr`). No match gives `"und"`, the default mapping.
8. **Internal use.** `localeCompare` creates its collator through the internal creation function, never through `Intl.Collator`. When `locales` and `options` are both `undefined`, it reuses one default collator kept in a JS variable (spec, "Resistance to tampering").
9. **`ResolveOptions` is shared.** It goes in `03-locales.js`, so Phases 3 to 5 reuse it. The service constructor it takes is a JS record: `{__proto__: null, availableLocales, relevantExtensionKeys, resolutionOptionDescriptors}`.

## Review Focus

These are the five inputs most likely to hurt someone that the spec implies but test262 does not exercise. Each one has a test in the task named.

1. **Unusual text.** This covers unpaired surrogates (`"\uD800"`, `"a\uDC00b"`), supplementary characters (`"𐐨"`, Deseret), NUL (`"a\0b"`), mappings that change the length (`"ß"`, `"ﬃ"`, `"İ"` lowercased) and a 1,000,000-character string. The expected behavior:
   - `compare(x, x)` is 0, and `compare(x, y)` is `-compare(y, x)`;
   - `"a\0b"` and `"a\0c"` differ;
   - case mapping keeps every unpaired surrogate where it was (`"a\uD800b".toLocaleUpperCase()` is `"A\uD800B"`) and maps supplementary characters (`"𐐨"` gives `"𐐀"`);
   - nothing crashes.

   Covered in Task 1 and Task 5 (Rust tests), Task 4 (`collator-edge.js`) and Task 6 (`string-case-edge.js`).
2. **Collators freed by the GC.** Make 2,000 collators in a loop, calling `gc()` every 100, keep every tenth alive, compare with the survivors afterwards, and end the script with live collators. The results are right, and ASan and LeakSanitizer report nothing. Covered in Task 2 (a gtest destroying a runtime that holds a `CollatorState`) and Task 4 (`collator-gc.js`).
3. **Tampering after the plugin loads:**
   - accessors on `Object.prototype` for the names the internal records use (`localeMatcher`, `co`, `kn`, `kf`, `locale`, `sensitivity`, `ignorePunctuation`), with `options` undefined;
   - a setter for `"0"` on `Array.prototype`;
   - a replaced `Intl.Collator`, `Intl.Collator.prototype.resolvedOptions` and `compare` getter;
   - a replaced `String.prototype.toLowerCase`, `toUpperCase` and `charCodeAt`.

   `new Intl.Collator(...)`, its `compare`, `resolvedOptions`, `localeCompare` and `toLocaleUpperCase` give the same results regardless. Covered in Task 4 and Task 6 (`tamper.js`).
4. **Objects that look like Collators:**
   - `Object.create(Intl.Collator.prototype)`;
   - a `Proxy` around a Collator;
   - an `Intl.Locale`;
   - a subclass instance;
   - the bound `compare`, detached and called with any `this`;
   - `Intl.Collator.call(collator, "de")`.

   The expected behavior:
   - the `compare` getter and `resolvedOptions` throw `TypeError` on the first three and work on the subclass;
   - the detached function still compares with its own collator;
   - the `call` returns a new collator and leaves `collator` unchanged;
   - no `Exception in HostFunction:` ever appears.

   Covered in Task 4 (`collator-objects.js`).
5. **Locales and options with no data or odd values:**
   - locales: `und`, `zxx`, `posix`, `en-u-co-standard`, `en-u-co-search`, `en-u-kn-yes`, `en-u-kf-yes`, `th-u-co-emoji`, `de-u-co-phonebk` with `collation: "eor"`;
   - options: `collation: ""` and `collation: "abcdefghi"` (`RangeError`), `numeric: "false"` (`true`, by ToBoolean), `caseFirst: "FALSE"` (`RangeError`);
   - `da` and `mt`, whose `caseFirst` resolves to `"upper"`;
   - `th` and `th-TH`, whose `ignorePunctuation` resolves to `true`.

   Each gives what the draft gives, and no case reaches native code with a value it rejects. Covered in Task 1 (Rust tests) and Task 4 (`collator-options-edge.js`).

---

### Task 1: The collator in the Rust core

**Files:**
- Create: `intl/core/src/collator.rs`
- Modify: `intl/core/src/lib.rs` (`mod collator;`, the FFI functions, `HiCollator`)
- Test: unit tests in `intl/core/src/collator.rs`

**Interfaces:**
- Consumes: `crate::locale::language_id(&str) -> Option<LanguageIdentifier>`, `crate::supported::COLLATIONS`.
- Produces:
  - `collator::new_collator(locale: &str, collation: &str, sensitivity: &str, ignore_punctuation: bool, numeric: bool, case_first: &str) -> Option<CollatorBorrowed<'static>>`:
    - `locale` is a canonical tag, and only its language identifier is used;
    - `collation` is `"default"` or a value of `COLLATIONS`;
    - `sensitivity` is one of `"base"`, `"accent"`, `"case"`, `"variant"`;
    - `case_first` is one of `"upper"`, `"lower"`, `"false"`;
    - the result is `None` for any other value, or if ICU4X fails to load the data.
  - `collator::locale_defaults(locale: &str) -> Option<String>`: `"<caseFirst> <ignorePunctuation>"` for a collator made with no options, for example `"false false"`, `"upper false"` for `da`, `"false true"` for `th`.
  - C interface in `lib.rs`:
    - `HiCollator`, an opaque struct holding the `CollatorBorrowed<'static>`;
    - `hi_collator_new(locale, locale_len, collation, collation_len, sensitivity, sensitivity_len, ignore_punctuation: bool, numeric: bool, case_first, case_first_len) -> *mut HiCollator`, null on failure;
    - `hi_collator_free(*mut HiCollator)`, which accepts null;
    - `hi_collator_compare(*const HiCollator, x: *const u16, x_len: usize, y: *const u16, y_len: usize) -> i32`, which returns -1, 0 or 1;
    - `hi_collator_locale_defaults(locale, len, out: *mut HiString) -> bool`.
  - A compile-time assertion next to `HiCollator`: `const _: () = { const fn assert_send<T: Send>() {} assert_send::<HiCollator>(); };`.

- [ ] **Step 1: Write the failing tests** in `collator.rs`, and add `mod collator;` to `lib.rs` so they compile. Compare with `compare_utf16` on `encode_utf16()` input.
  - `option_mapping`: on `"en"` with collation `"default"` and ignorePunctuation `false`, the four rows of the sensitivity table in test262's `non-normative-sensitivity.js`. With target `"Aa"` and input `["Aa", "bA", "E", "b", "aA", "fC", "áÁ", "Aã"]`, the strings that compare equal are:

    | Sensitivity | Equal to `"Aa"` |
    |---|---|
    | base | `["Aa", "aA", "áÁ", "Aã"]` |
    | accent | `["Aa", "aA"]` |
    | case | `["Aa", "Aã"]` |
    | variant | `["Aa"]` |

  - `ignore_punctuation`: with ignorePunctuation `true`, `"" == " "` and `"" == "*"`, for `"en"` and `"th"`; with `false`, both compare `Less`, for `"en"` and for `"th"`.
  - `numeric_and_case_first`:
    - numeric `true` gives `"2" < "10"`, and `false` gives `"10" < "2"`;
    - caseFirst `"upper"` gives `"A" < "a"`, and `"lower"` gives `"a" < "A"`.
  - `collation_types`: sorting `["A", "b", "Af", "Ab", "od", "off", "Ä", "ö"]` with `de` and `phonebk` gives `["A", "Ab", "Ä", "Af", "b", "od", "ö", "off"]`, the values of test262's `non-normative-phonebook.js`.
  - `canonical_equivalence`: equal for `("o\u{308}", "ö")`, `("Å", "A\u{30A}")` and `("\u{1111}\u{1171}\u{11B6}", "퓛")`.
  - `unusual_text`:
    - `compare(x, x) == Equal` and `compare(x, y) == compare(y, x).reverse()` over `"\u{D800}"` (as `&[0xD800]`), `"a\u{0}b"`, `"a\u{0}c"`, `"\u{10428}"` and a 1,000,000-unit string of `'a'`;
    - `"a\0b" != "a\0c"`.
  - `rejects_bad_values`: `None` for collation `"standard"`, `"search"` and `"xyz"`, sensitivity `"Base"`, case_first `"FALSE"`, and locale `"en_US"`.
  - `locale_defaults_of_locales`: `en`, `ja`, `de`, `und` give `"false false"`; `da` and `mt` give `"upper false"`; `th` and `th-TH` give `"false true"`; `"en_US"` gives `None`.
- [ ] **Step 2: Run them to see them fail.** `check-intl-rust` fails to compile, because `new_collator` and `locale_defaults` do not exist.
- [ ] **Step 3: Implement `collator.rs` and the C interface.**
  - Build `CollatorPreferences` from the language identifier, then set `collation_type`, `case_first` and `numeric_ordering`.
  - Build `CollatorOptions` from decision 5.
  - `locale_defaults` reads `resolved_options()` of `Collator::try_new(prefs, CollatorOptions::default())`.
  - `hi_collator_compare` maps `Ordering` to -1, 0, 1. It accepts `len == 0` with any pointer, as `as_str` does.
- [ ] **Step 4: Run `check-intl-rust`.** Expected: all tests pass.
- [ ] **Step 5: Commit.**

### Task 2: Native state and the collator helpers in the JSI layer

**Files:**
- Modify:
  - `intl/jsi/NativeHelpers.h` (new tags), `intl/jsi/NativeHelpers.def`, `intl/jsi/RustCore.h`;
  - `intl/CMakeLists.txt` and `intl/test/unit/CMakeLists.txt`, both of which list the helper sources. The unit-test target compiles `NativeHelpers.cpp`, which refers to every helper in the table, so it needs `CollatorHelpers.cpp` too.
- Create: `intl/jsi/CollatorHelpers.cpp`
- Test: `intl/test/unit/NativeHelpersTest.cpp`

**Interfaces:**
- Consumes: Task 1's C interface.
- Produces:
  - Conversion tags in `NativeHelpers.h`, documented in the `.def` header comment:

    | Tag | C++ type | Direction | Notes |
    |---|---|---|---|
    | `Bool` | `bool` | arguments too | an argument must be a JS boolean |
    | `Object` | `jsi::Object` | arguments only | |
    | `Utf16` | `std::u16string` | both | `String::utf16` and `String::createFromUtf16`, lossless for unpaired surrogates |
    | `Int` | `int` | results only | |
    | `Undefined` | `std::monostate` | results only | |

    A wrong JS type is an internal error naming the helper.
  - `class CollatorState : public jsi::NativeState`, which owns a `HiCollator *` and frees it in its destructor. It is declared in `CollatorHelpers.cpp`.
  - Helper lines:
    - `HERMES_INTL_HELPER(collatorLocaleDefaults, Utf8, Utf8)`: `"<caseFirst> <ignorePunctuation>"` for an available locale. It throws if the locale does not parse.
    - `HERMES_INTL_HELPER(collatorInit, Undefined, Object, Utf8, Utf8, Utf8, Bool, Bool, Utf8)`: `(collator, locale, collation, sensitivity, ignorePunctuation, numeric, caseFirst)`. It attaches a `CollatorState`. It throws if `hi_collator_new` fails or the object already has native state.
    - `HERMES_INTL_HELPER(collatorCompare, Int, Object, Utf16, Utf16)`: `(collator, x, y)`, giving -1, 0 or 1. It throws if the object has no `CollatorState`.

- [ ] **Step 1: Write the failing gtests** in `NativeHelpersTest.cpp`, with test-local helpers declared as the existing `noArguments` one is:
  - `Utf16RoundTrip`: a `Utf16 -> Utf16` echo helper returns `"a\uD800b\uDC00𐐨"` code unit for code unit. Check it with `charCodeAt` in JS.
  - `ArgumentTypesAreChecked`: an `Object, Bool` helper called with `(1, true)` and with `({}, 1)` throws a `jsi::JSError` whose message names the helper.
  - `IntAndUndefinedResults`: helpers returning `-1` and `std::monostate{}` give the JS values `-1` and `undefined`.
  - `CollatorStateFreedWithRuntime`: with the plugin's real helpers (`makeNativeHelpers`), `collatorInit` on a fresh object, `collatorCompare` of `"a"` and `"b"` is -1, then destroy the runtime. Under ASan, LeakSanitizer must report nothing.
  - `CollatorHelpersRejectMisuse`: these each throw an Error containing `Intl internal error`:
    - `collatorCompare` on an object without state;
    - `collatorInit` twice on one object;
    - `collatorInit` with sensitivity `"Base"`.
- [ ] **Step 2: Run them to see them fail.** `check-intl-lit` fails to build.
- [ ] **Step 3: Implement the tags, `CollatorState`, the RustCore.h wrappers and the three helpers.**
- [ ] **Step 4: Run `check-intl-lit`.** Expected: the gtests pass, and so do the lit tests, which are unchanged.
- [ ] **Step 5: Commit.**

### Task 3: ResolveOptions

**Files:**
- Modify: `intl/js/03-locales.js`
- Test: `intl/test/lit/js/resolve-options.js`, an isolation test built like `intl/test/lit/js/resolve-locale.js`, concatenating `01-intrinsics.js` to `05-tags.js`

**Interfaces:**
- Consumes: `ResolveLocale`, `CanonicalizeLocaleList`, `GetOption`, `GetOptionsObject`, `CoerceOptionsToObject`, `isUnicodeType`.
- Produces: `ResolveOptions(ctor, localeData, locales, options, specialBehaviours, modifyResolutionOptions)`. Its parameters:
  - `ctor` is the record of decision 9;
  - `localeData` is a function, as `ResolveLocale` takes it;
  - `specialBehaviours` is `undefined` or a list containing `'require-options'` or `'coerce-options'`;
  - `modifyResolutionOptions` is `undefined` or a function of the `opt` record.

  It returns `{__proto__: null, options, resolvedLocale, resolutionOptions}`.

- [ ] **Step 1: Write the failing test.** Use a fake service with `relevantExtensionKeys: ['co', 'kn', 'kf']` and the descriptors of decision 1. Print each case with `JSON.stringify` and check it with FileCheck:
  - options read through a logging Proxy are read in this order: `localeMatcher`, `collation`, `numeric`, `caseFirst`;
  - `numeric: true` gives `opt.kn === "true"`, and `numeric: false` gives `"false"`;
  - `collation: "a b"` and `collation: ""` throw `RangeError`, from the type check;
  - `caseFirst: "x"` throws `RangeError`, from GetOption;
  - `options: 5` throws `TypeError` without `'coerce-options'`, and is coerced with it;
  - `options: undefined` throws `TypeError` with `'require-options'`;
  - `modifyResolutionOptions` sees `opt` before ResolveLocale;
  - `resolvedLocale.locale` for `["de-u-co-phonebk"]` with a localeData whose `co` contains `"phonebk"` is `"de-u-co-phonebk"`.
- [ ] **Step 2: Run it to see it fail.** `LIT_FILTER=resolve-options` with `check-intl-lit` fails: ResolveOptions is not defined.
- [ ] **Step 3: Implement ResolveOptions** step by step from `spec/negotiation.html`.
- [ ] **Step 4: Run `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Commit.**

### Task 4: Intl.Collator

**Files:**
- Create: `intl/js/30-Collator.js`
- Modify: `intl/js/01-intrinsics.js` (only for intrinsics the new code needs), `intl/js/10-Intl.js` (call `installCollator(IntlObject)` before `installLocale`), `intl/test/test262-expectations.txt`
- Test:
  - Create: `intl/test/lit/collator.js`, `collator-objects.js`, `collator-options-edge.js`, `collator-edge.js`, `collator-gc.js`
  - Modify: `shapes-table.js`, `tamper.js`

**Interfaces:**
- Consumes: Task 2's helpers, Task 3's `ResolveOptions`, `FilterLocales`, `CreateDataPropertyOrThrow`, `defineConstructor`, `defineMethods`, `defineGetters`, `setToStringTag`, `ReturnOverride`.
- Produces (for Task 6):
  - `CreateCollator(locales, options)`: a new initialized Collator made from the intrinsic `%Intl.Collator.prototype%`. It is the draft's constructor steps with `newTarget` fixed.
  - `CompareStrings(collator, x, y)`: `native.collatorCompare(collator, x, y)` for strings `x` and `y`.
  - `CollatorSlots`, a `ReturnOverride` subclass with `#locale`, `#usage`, `#collation`, `#numeric`, `#caseFirst`, `#sensitivity`, `#ignorePunctuation` and `#boundCompare`, and static `is(o)` and `get(o, name)` as `LocaleSlots` has.

- [ ] **Step 1: Write the failing lit tests.** Run each with `%hermes -Xintl-default-locale=en-US %s`.
  - `collator.js`, the basic surface:
    - `resolvedOptions()` of `new Intl.Collator()` is `{"locale":"en-US","usage":"sort","sensitivity":"variant","ignorePunctuation":false,"collation":"default","numeric":false,"caseFirst":"false"}`;
    - `de-u-co-phonebk` resolves to locale `de-u-co-phonebk` and collation `phonebk`;
    - `Intl.Collator("de")` without `new` is a Collator;
    - `supportedLocalesOf(["de", "xx", "zh-Hant-TW"])` is `["de", "zh-Hant-TW"]`;
    - sorting `["b", "a", "B", "ä"]` with `new Intl.Collator("de").compare` gives `["a", "ä", "b", "B"]`;
    - `compare` returns the same function each time;
    - `Object.prototype.toString.call(new Intl.Collator())` is `"[object Intl.Collator]"`;
    - for every value of `Intl.supportedValuesOf("collation")`, a locale from test262's `collations-accepted-by-Collator.js` list resolves it.
  - `collator-objects.js`: Review Focus 4.
  - `collator-options-edge.js`: Review Focus 5, each case printing `resolvedOptions()` or the error's constructor name.
  - `collator-edge.js`: Review Focus 1, the compare half.
  - `collator-gc.js`: Review Focus 2.
  - `shapes-table.js`: add these rows. The `bound` kind is new:
    - it reads the function as `holder[property]`, because `compare` is a getter on the prototype, not an own property of the instance;
    - it checks `name` `""`, the given `length`, not constructible, and no `prototype`.

    Add a row with holder `new Intl.Collator()` (the table evals each holder), property `compare`, kind `bound`, length 2.

    | Holder | Property | Kind | Name | Length |
    |---|---|---|---|---|
    | `Intl` | `Collator` | constructor | `Collator` | 0 |
    | `Intl.Collator` | `supportedLocalesOf` | method | `supportedLocalesOf` | 1 |
    | `Intl.Collator.prototype` | `resolvedOptions` | method | `resolvedOptions` | 0 |
    | `Intl.Collator.prototype` | `compare` | getter | `get compare` | |
    | `Intl.Collator.prototype` | `@@toStringTag` | toStringTag | `Intl.Collator` | |

  - `tamper.js`: Review Focus 3, the Collator half.
- [ ] **Step 2: Run them to see them fail.** `LIT_FILTER=collator` with `check-intl-lit` fails: `Intl.Collator` is undefined.
- [ ] **Step 3: Implement `30-Collator.js`.**
  - Write these step by step from `spec/collator.html`:
    - the constructor, with `CreateCollator` sharing its steps;
    - `supportedLocalesOf`, `resolvedOptions` and the `compare` getter;
    - `@@toStringTag`.
  - The locale data function returns the record of decision 2. It takes `co` from `splitSpaces(native.collationsOfLocale(locale))`, and `kf` and `ignorePunctuation` from `native.collatorLocaleDefaults(locale)`.
  - The bound compare is an arrow function returned by a factory function, so its name is `""`. It is created once and kept in `#boundCompare`.
  - Native state is attached by `native.collatorInit` once every slot is set.
- [ ] **Step 4: Run `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Update the expectations.**
  - 64 lines must go:
    - every `Phase 2: Intl.Collator` line except `usage-de.js`;
    - `supportedValuesOf/collations-accepted-by-Collator.js`;
    - `String/prototype/localeCompare/default-options-object-prototype.js`, which only needs a Collator to exist, because Hermes's own `localeCompare` already orders `"a"` before `"A"`.
  - Rewrite `usage-de.js`'s reason by hand to decision 4's text.
  - Then run `check-intl-test262`. Expected: `0 problem(s)`.
- [ ] **Step 6: Commit.**

### Task 5: Case mapping in the Rust core and its helper

**Files:**
- Create: `intl/core/src/casemap.rs`, `intl/jsi/StringHelpers.cpp`
- Modify:
  - `intl/core/src/lib.rs` (`mod casemap;`, `HiString16`, `hi_string16_free`, `hi_case_map`);
  - `intl/jsi/RustCore.h`, `intl/jsi/NativeHelpers.def`;
  - `intl/CMakeLists.txt` and `intl/test/unit/CMakeLists.txt` (the new `StringHelpers.cpp`, for the same reason as in Task 2)
- Test: unit tests in `casemap.rs`; the `NativeHelpersTest.cpp` gtest `CaseMapKeepsUnpairedSurrogates`

**Interfaces:**
- Consumes: Task 2's `Utf16` and `Bool` tags.
- Produces:
  - `casemap::case_map(text: &[u16], language: &str, upper: bool) -> Option<Vec<u16>>`. `language` is `"und"` or one of decision 7's languages, and the result is `None` if it does not parse.
  - C interface: `HiString16 {data: *mut u16, len, capacity}` and `hi_string16_free(HiString16)`, and `hi_case_map(text: *const u16, len, language: *const u8, language_len, upper: bool, out: *mut HiString16) -> bool`.
  - `HERMES_INTL_HELPER(caseMap, Utf16, Utf16, Utf8, Bool)`: `(text, language, upper)`. It throws if the language does not parse.

- [ ] **Step 1: Write the failing tests** in `casemap.rs`, and add `mod casemap;` to `lib.rs` so they compile. Use `encode_utf16` input:
  - `default_mapping`: under `"und"`:
    - `"ß"` uppercases to `"SS"` and `"ﬃ"` to `"FFI"`;
    - `"İ"` lowercases to `"i\u{307}"`;
    - `"ΑΣ"` lowercases to `"ας"` (final sigma);
    - `"\u{10428}"` uppercases to `"\u{10400}"`.
  - `tailored_mapping`:
    - `"tr"` and `"az"`: `"i"` uppercases to `"İ"`, and `"I"` lowercases to `"ı"`;
    - `"lt"`: `"Ì"` lowercases to `"i\u{307}\u{300}"`;
    - `"el"`: `"άλφα"` uppercases to `"ΑΛΦΑ"`;
    - `"hy"`: `"և"` uppercases to `"ԵՎ"`.
  - `unpaired_surrogates`:
    - `[0x61, 0xD800, 0x62]` uppercases to `[0x41, 0xD800, 0x42]`;
    - `[0xDC00, 0xD800]` is unchanged;
    - `[0xD801, 0xDC28, 0xDC00]` uppercases to `[0xD801, 0xDC00, 0xDC00]`;
    - the empty input gives the empty output.
  - `rejects_bad_language`: `"en_US"` gives `None`.

  The gtest `CaseMapKeepsUnpairedSurrogates` calls `caseMap("a\uD800b", "und", true)` and checks that it gives `"A\uD800B"`, read with `charCodeAt`.
- [ ] **Step 2: Run them to see them fail.** `check-intl-rust` fails to compile, because `case_map` does not exist.
- [ ] **Step 3: Implement** with `CaseMapper::new()`, on each maximal well-formed run decoded with `char::decode_utf16`.
- [ ] **Step 4: Run `check-intl-rust` and `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Commit.**

### Task 6: The String overrides

**Files:**
- Create: `intl/js/80-builtins.js`
- Modify: `intl/js/99-footer.js` (setup step 3: `installBuiltinOverrides()`, after `installIntl()`), `intl/js/01-intrinsics.js` (capture `String.prototype`), `intl/test/test262-expectations.txt`
- Test:
  - Create: `intl/test/lit/string-methods.js`, `string-case-edge.js`
  - Modify: `shapes-table.js`, `tamper.js`

**Interfaces:**
- Consumes: `CreateCollator`, `CompareStrings` (Task 4), `native.caseMap` (Task 5), `CanonicalizeLocaleList`, `LookupMatchingLocaleByPrefix`, `DefaultLocale`.
- Produces: `installBuiltinOverrides()`, and `TransformCase(string, locales, targetCase)` with `targetCase` `'lower'` or `'upper'`.

- [ ] **Step 1: Write the failing lit tests.** Run each with `%hermes -Xintl-default-locale=en-US %s`.
  - `string-methods.js`:
    - `"a".localeCompare("b")` is -1;
    - `"ä".localeCompare("z", "sv")` is 1, and `"ä".localeCompare("z", "de")` is -1;
    - `"a".localeCompare("A", undefined, {sensitivity: "base"})` is 0;
    - `"i".toLocaleUpperCase("tr")` is `"İ"`, `"i".toLocaleUpperCase("tr-TR")` is `"İ"`, and `"i".toLocaleUpperCase("en")` is `"I"`;
    - `"I".toLocaleLowerCase(["az", "en"])` is `"ı"`;
    - `"άλφα".toLocaleUpperCase("el")` is `"ΑΛΦΑ"`;
    - `"x".toLocaleUpperCase("xx-invalid-")` throws `RangeError`;
    - `String.prototype.localeCompare.call(null, "a")` throws `TypeError`;
    - `toLocaleUpperCase` is generic: `String.prototype.toLocaleUpperCase.call(true)` is `"TRUE"`.
  - `string-case-edge.js`: Review Focus 1, the case-mapping half, through the JS API.
  - `shapes-table.js`: add these rows.

    | Holder | Property | Kind | Name | Length |
    |---|---|---|---|---|
    | `String.prototype` | `localeCompare` | method | `localeCompare` | 1 |
    | `String.prototype` | `toLocaleLowerCase` | method | `toLocaleLowerCase` | 0 |
    | `String.prototype` | `toLocaleUpperCase` | method | `toLocaleUpperCase` | 0 |

  - `tamper.js`: Review Focus 3, the String half. This includes a replaced `Intl.Collator` and `Intl.Collator.prototype.compare` getter, with `localeCompare` called both with and without `locales`.
- [ ] **Step 2: Run them to see them fail.** The tailored case results and the locale-sensitive comparisons fail, because Hermes's own methods are still installed. The new shapes rows already pass, because Hermes's methods have the same names and lengths; they stay as regression checks.
- [ ] **Step 3: Implement `80-builtins.js`.**
  - Write `localeCompare`, `toLocaleLowerCase`, `toLocaleUpperCase` and TransformCase step by step from `spec/locale-sensitive-functions.html`.
  - Install them on `String.prototype` with `defineMethods`.
  - `localeCompare` follows decision 8.
  - TransformCase's available locales follow decision 7: a predicate over the five tags, passed to `LookupMatchingLocaleByPrefix`.
- [ ] **Step 4: Run `check-intl-lit`.** Expected: pass.
- [ ] **Step 5: Update the expectations.**
  - The 10 remaining `Phase 2: locale-sensitive String methods` lines must go.
  - The `built-ins/String/prototype` tests that pass now must still pass.
  - Then run `check-intl-test262`. Expected: `0 problem(s)`.
- [ ] **Step 6: Commit.**

### Task 7: Acceptance, spec and README

**Files:**
- Modify:
  - `doc/superpowers/specs/2026-10-22-intl-plugin-design.md`:
    - "The native interface": the new conversion tags; `CollatorState`, the first native state, and its `Send` check; the collator and case-mapping helpers;
    - "Strings": decision 6's U+FFFD reading;
    - "Data": decisions 2, 4, 5 and 7;
    - "The JS layer", "Files": ResolveOptions (decision 9) and `80-builtins.js`;
  - `intl/README.md`:
    - the layout table names `collator.rs`, `casemap.rs`, `CollatorHelpers.cpp`, `StringHelpers.cpp`, `30-Collator.js` and `80-builtins.js`;
    - "Adding a native helper" lists the tags.

- [ ] **Step 1: Run everything.** Run `check-intl`, then a full `check-hermes`. Expected:
  - every part of `check-intl` passes, with test262 at `0 problem(s)`;
  - `check-hermes` has 0 unexpected results.

  Record the counts in the report.
- [ ] **Step 2: Check the expectations file.**
  - `grep -c 'Phase 2' intl/test/test262-expectations.txt` is 0.
  - `grep -c 'intl402/Collator/' intl/test/test262-expectations.txt` is 1 (`usage-de.js`).
  - `git diff <Phase 2's first commit>^ -- intl/test/test262-expectations.txt` only removes lines, apart from `usage-de.js`'s reason.
- [ ] **Step 3: Update the spec and README** as listed above, in the documents' existing style. Write in plain English, with no jargon of our own.
- [ ] **Step 4: Commit.**

## After the tasks

When the whole-branch review is done and its findings are fixed:
1. Run a Codex code review (the `codex-review` skill) of the Phase 2 commits, from the plan commit to the tip.
2. Fix or drop every finding, with a reason. Never defer one.
3. Repeat until Codex reports every finding CLOSED.
4. Rerun `check-intl` and `check-hermes` on Linux, then ask the Mac agent (gist channel) for the same runs on macOS.
