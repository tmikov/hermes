# Intl Plugin Phase 7 (Delete the Old Implementation) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Delete Hermes's old built-in ECMA-402 implementation (`lib/VM/JSLib/Intl.cpp`, the platform backends in `lib/Platform/Intl/` with their Java classes, the `HERMES_ENABLE_INTL` paths and `.def` entries, `android/intltest`, and everything that exists only for them), so that `Intl` comes only from the plugin. A runtime without the plugin has no `Intl` and uses Hermes's own locale fallbacks for the locale-sensitive built-ins. The `Intl` field of `RuntimeConfig` stays, documented as deprecated and ignored.

**Architecture:** Pure removal plus the edits that keep every remaining build and test consistent: the VM keeps each `#else` fallback of the old dispatch sites; the build options, CI, test runners, mobile build scripts and test apps stop referring to the old implementation; the docs and spec describe the plugin as the only `Intl`.

**Tech Stack:** CMake, C++ (VM), Gradle/Android NDK and the emulator (this host), Xcode/CocoaPods (the Mac agent), Python (the test262 runner), lit.

**Spec:** `doc/superpowers/specs/2026-10-22-intl-plugin-design.md`. Read "Packaging", "Loader" (Workers), "`RuntimeConfig`", "Coexistence with the old implementation" and "Phasing" item 7 before any task. Task 5 writes this plan's decisions into it.

**Research:** `/home/tmikov/work/hermes-intl/.superpowers/p7-research/old-intl-inventory.md` lists every file and line the old implementation touches (as of b5670bae2). Implementers start from it; the plan's decisions win where they differ.

## Global Constraints

- **Worktree:** work only in `/home/tmikov/work/hermes-intl`, on branch `hermes-intl`. **Never `cd`**; pass absolute paths. Use a subshell `( cd x; cmd )` only when a tool insists.
- **Build directories:** `/home/tmikov/work/hermes-intl/cmake-build-asan` is the host ASan build (Clang, Debug `-O1`, `JSI_UNSTABLE=ON`, test262 dir set). Do not reconfigure it by hand. Mobile and other builds go under `/home/tmikov/work/hermes-intl/.superpowers/p7-builds/<name>` or reuse the Phase 6 directories under `.superpowers/p6-builds/` (git-ignored), never inside tracked directories.
- **Commands (host):** `check-intl` (plugin-mode test262 included), `check-hermes`, `check-shermes`, each as `cmake --build /home/tmikov/work/hermes-intl/cmake-build-asan --target <t>`; narrow lit with `LIT_FILTER=<regex>`.
- **Git:** at the end of each task, commit that task's files on `hermes-intl` with a plain-English message ending with the line `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Never push, amend or reset.
- **No regressions on the host:** after every task, plugin-mode test262 gives 2696 pass, 18 js-fail, 0 problems; `check-hermes` and `check-shermes` have 0 unexpected results. Their pass counts drop only by the deleted old-Intl tests, which the task report lists.
- **Prove checks can fail:** every new or changed test is shown failing under a stated mutation, with the failing test named, in the task report. Mobile runs that cannot be mutated cheaply show a negative case instead.
- **Never bump `BYTECODE_VERSION`.** Nothing here changes bytecode: `Builtins.def` has no Intl entries, and the removed `.def` entries are runtime-only.
- **The Mac:** tasks that need macOS write the exact commands into the task report under "Mac commands"; the controller posts them and returns the results.

## Planning decisions

Task 5 writes these into the spec.

1. **What goes.** Everything the inventory's "Files to delete" lists: `lib/VM/JSLib/Intl.cpp`; `lib/Platform/Intl/` (C++, Objective-C++, the dead `PlatformIntlShared.cpp`, `CMakeLists.txt`, `java/` with its `notes.md` and `.docx`); `include/hermes/Platform/Intl/`; `android/intltest/`; `unittests/PlatformIntl/`; `utils/IntlGen/`; the old-Intl lit tests (`test/hermes/intl/`, `test/shermes/intl/`, `test/hermes/regress-intl-number-format-bigint-input.js`); `doc/IntlAPIs.md`. Also: `JSLibInternal.h`'s `intl::createIntlObject`; the `HERMES_ENABLE_INTL` blocks of `NativeFunctions.def`, `PredefinedStrings.def` and `RuntimeHermesValueFields.def`; `NAMED_PROP(IntlNativeType)` in `InternalProperties.def` (its IDs are runtime-only, so renumbering the later internal properties is safe); the `Intl` global in `GlobalObject.cpp` and its unconditional `PlatformIntl.h` include; `Runtime::hasIntl()` and `hasIntl_`; `${FOUNDATION}` (its only user is the old Apple backend; `CORE_FOUNDATION` stays); the `add_subdirectory` of `android/intltest`'s CMake and the `hermesPlatformIntl`/`hermesBCP47Parser` links in `lib/CMakeLists.txt`, `lib/Platform/CMakeLists.txt` and `lib/VM/CMakeLists.txt`. fbjni stays (Unicode and the finalizer runner use it).
2. **Fallbacks stay.** Each dispatch site keeps its `#else` branch as the only code: `String.prototype.toLocaleLowerCase`/`toLocaleUpperCase` (`convertCase`), `localeCompare` (`platform_unicode::localeCompare`), `Number.prototype.toLocaleString` (`toString`), `Date.prototype.toLocale{,Date,Time}String` (`platform_unicode`, "Invalid Date" for non-finite), `Array.prototype.toLocaleString` and `%TypedArray%.prototype.toLocaleString` (`executeCall0`, ECMA-262; the plugin replaces both). With the plugin loaded nothing changes: the plugin replaces `globalThis.Intl` and these built-ins.
3. **`HERMES_ENABLE_INTL`.** The option and its definition go. React Native's own builds pass `-DHERMES_ENABLE_INTL=True` today, so a configure that sets it ON prints a deprecation warning naming the plugin (`HERMES_BUILD_INTL`) and continues; it has no other effect.
4. **`RuntimeConfig`'s `Intl` field and `-Xintl`.** The field stays where it is (removing it would change `RuntimeConfig`'s layout), documented as deprecated and ignored; `SynthTrace` keeps writing and parsing it. `-Xintl` stays an accepted option, described as deprecated and ignored, because scripts pass it. The loader no longer copies the field into Worker configurations (`IntlRecord::oldIntl` and its `IntlLoaderTest` assertions go): a Worker's `Intl` comes from the plugin it inherits, as before.
5. **Tests.** Old-Intl lit tests are deleted. Four behaviours they covered get plugin tests in `intl/test/lit/` if no equivalent exists (the task checks and lists what it found): `NumberFormat.prototype.format(0n)` and `formatToParts(0n)`; a legacy default-locale extension (`_HERMES_TEST_LOCALE="en-GB@foo=bar"` or the plugin's own default-locale override, whichever the plugin honours); a default time zone from `TZ=US/Pacific`; the test262-derived `toLocaleLowerCase` cases. `test/lit.cfg` loses the `intl` feature and `%old_intl`; `test/hermes/number-functions.js` keeps only the fallback lines ("Infinity"); `UNSUPPORTED: intl` lines go from `date-locale.js`, `date-format-fixed.js` and `intl-plugin/worker-inherits-disabled.js`; `intl/test/lit/shermes-flags.js` and `tool-flags.js` tighten `OFF: {{object|undefined}}` to `undefined`. A new lit test pins decision 2: with `-Xintl-plugin=false`, `typeof Intl` is `"undefined"` and every fallback built-in in decision 2, plus `BigInt.prototype.toLocaleString`, returns its fallback result without throwing. A unit test pins decision 4: `RuntimeConfig` `Intl` true and false, each with the plugin enabled and disabled, for a parent runtime and a Worker created from it, gives the same effective result (the plugin's `Intl` when enabled, no `Intl` when disabled).
6. **Test runners.** `--test-intl` goes from `utils/testsuite` and `tools/test-runner`. Normal mode skips `test262/test/intl402` as a fixed rule (it has no `Intl`); plugin mode is unchanged. The `platform_skip_list.darwin` entries (old Apple Collator) and the `intl402` entries in `manual_skip_list`, `skip_list` and `permanent_skip_list` go, since neither mode reads them any more. The `platform_skip_list.linux` entries for `built-ins/String/prototype/toLocale{Lower,Upper}Case` concern Hermes's own fallback, not the old Intl: they stay unless a normal-mode run of exactly those directories passes without them (the only normal-mode test262 run allowed in this phase).
7. **CI.** `build.yml`'s old-Intl step in `test-macos-test262` becomes a normal-mode run of the whole `test262/test` tree without `--test-intl`, keeping `matrix.test_runner_flags` (the `-Xjit=force` run), so macOS keeps its general test262 coverage (it may reuse the plugin step's build directory, since normal mode disables the plugin); the plugin step drops `-DHERMES_ENABLE_INTL=OFF`. `.circleci/config.yml`'s `test-macos` drops `-DHERMES_ENABLE_INTL=ON` and `--test-intl` and keeps `-DHERMES_BUILD_INTL=OFF` (no Rust there): it tests Hermes without `Intl`.
8. **Android.** `android/build.gradle.kts` drops `-DHERMES_ENABLE_INTL=True`, the `lib/Platform/Intl/java` source directory (keeps `lib/Platform/Unicode/java`), and `androidx.annotation` if nothing else uses it; `android/settings.gradle.kts` drops `:intltest`; build-logic properties used only by `intltest` go. The emulator smoke test and app expect no `Intl` at all without the plugin (`typeof Intl === "undefined"`) and the plugin's `Intl` with it. The AAR has no `com/facebook/hermes/intl` classes.
9. **Apple.** `utils/build-apple-framework.sh` drops `-DHERMES_ENABLE_INTL:BOOLEAN=true`; `hermesvm.framework` no longer links Foundation unless something else needs it (the task checks with `otool -L` on the Mac). The podspec's comment about the built-in `Intl` becomes "no `Intl` when the plugin is absent". The smoke test and the CocoaPods app expect no `Intl` without the plugin.
10. **React Native.** Without the React Native podspec change already recorded in the spec, React Native apps on Apple platforms have no `Intl` after this phase; on Android an app has `Intl` only when `libhermesintl.so` is packaged (the AAR carries it). ProGuard keep rules for `com.facebook.hermes.intl` in React Native become stale. That change is in another repository and cannot be made or tested here, so the spec's "prerequisite for Phase 7" becomes a **release gate**: Phase 7 lands on this branch, and a Hermes release that contains it must not ship to React Native until React Native's podspec vendors `hermesintl.xcframework` weakly and its source builds pass Rust or `-DHERMES_BUILD_INTL=OFF`; the spec records the gate and leaves a place for the compatible React Native revision. Task 5 writes this into the spec's React Native section and "Phasing".
11. **Size.** The task for each platform records the size of `libhermesvm.so` (per ABI) and `hermesvm.framework` (per slice, from the Mac) before and after.

## Review Focus

1. **A runtime without the plugin.** No `Intl` global, and the locale-sensitive built-ins (`toLocaleString` on Number, BigInt, Date, Array and typed arrays, `String.prototype.toLocale{Lower,Upper}Case`, `localeCompare`, `Date.prototype.toLocale{Date,Time}String`) return their fallback results without throwing, including for non-finite dates and numbers. Task 1's new lit test.
2. **Builds that still pass `-DHERMES_ENABLE_INTL=ON`** (React Native today): configure warns and succeeds. Task 1, a configure check.
3. **Embedders and tools using the deprecated knobs:** `RuntimeConfig::withIntl(true/false)` and `-Xintl` are accepted and change nothing, with or without the plugin, Workers included. Task 1 (lit with `-Xintl`, a unit test with the field set both ways).
4. **Apps with the plugin removed:** the Android emulator app and the CocoaPods app launch and report no `Intl`, without a crash or a JNI lookup of a deleted class. Tasks 2 and 3.
5. **Mobile artifacts after the deletion:** the AAR has no `com/facebook/hermes/intl` classes and `libhermesvm.so` names none of them; `hermesvm.framework` loses Foundation only if nothing else needs it. Tasks 2 and 3.

---

### Task 1: The VM, the build and the host tests

**Files:**
- Delete: the decision 1 files except `android/intltest/` and the docs.
- Modify: `CMakeLists.txt`, `lib/CMakeLists.txt`, `lib/Platform/CMakeLists.txt`, `lib/VM/CMakeLists.txt`, `unittests/CMakeLists.txt`, the three `.def` files and `InternalProperties.def`, `lib/VM/JSLib/{GlobalObject,String,Number,Date,Array,TypedArray}.cpp`, `JSLibInternal.h`, `include/hermes/VM/Runtime.h`, `lib/VM/Runtime.cpp`, `public/hermes/Public/RuntimeConfig.h` (comment), `include/hermes/VM/RuntimeFlags.h` (help text), `API/hermes/IntlPluginLoader.cpp`, `unittests/IntlLoader/IntlLoaderTest.cpp`, `test/lit.cfg`, the lit tests of decision 5, `intl/test/lit/{shermes-flags,tool-flags}.js`.
- Test: new `test/hermes/no-intl-fallbacks.js` and `no-intl-fallbacks-tr.js` (decision 5's fallback tests), an `intl/test/lit/` test for `-Xintl` with the plugin, `intl/test/lit/` plugin tests for the decision 5 behaviours that lack one, a configure check for decision 3.

**Interfaces:**
- Produces a VM with no old `Intl`; Tasks 2 and 3 build it for mobile.

- [ ] **Step 1: Write the new tests first.**
  - `no-intl-fallbacks.js`, every RUN line with `_HERMES_TEST_LOCALE=en_US` and `%hermes -Xintl-plugin=false`: `typeof Intl` is `"undefined"`; `(1234.5).toLocaleString()` is `"1234.5"`, `Infinity.toLocaleString()` is `"Infinity"`; `(12n).toLocaleString()` is `"12"`; `[1, 2].toLocaleString()` and `new Uint8Array([1, 2]).toLocaleString()` are `"1,2"`; an array element and a typed-array element whose `toLocaleString` records `arguments.length` show 0 when called with `("de", {})` (the fallback's `executeCall0`); `new Date(NaN).toLocaleString()`, `toLocaleDateString()` and `toLocaleTimeString()` are `"Invalid Date"`, and a finite date gives a non-empty string from each of the three; `"I".toLocaleLowerCase()` is `"i"`, `"i".toLocaleUpperCase()` is `"I"`; `"a".localeCompare("b") < 0`. The same checks run with `-Xintl` and with `-Xintl=false` (same output). The test needs no plugin.
  - `no-intl-fallbacks-tr.js` (`REQUIRES: unicode_hermes || unicode_cf`, the backends that honour `_HERMES_TEST_LOCALE`; Lite and Emscripten use root casing, Java uses its own default locale): with `_HERMES_TEST_LOCALE=tr`, `"I".toLocaleLowerCase()` is `"ı"` and `"i".toLocaleUpperCase()` is `"İ"` (the fallback stays locale-sensitive).
  - In `intl/test/lit/` (whose `%hermes` has the real plugin): with the plugin enabled, `-Xintl` and `-Xintl=false` both give the plugin's `Intl` (`typeof Intl.Segmenter` is `"function"`).
  - The decision 4 unit test in `unittests/IntlLoader/IntlLoaderTest.cpp` (fake plugin fixture), replacing the `oldIntl` assertions.
  - The plugin tests of decision 5 for whatever behaviour has no equivalent yet.
  - Run them. The host build never had the old Intl, so most pass before the change; the report records which, and shows each new test failing under a mutation of the code it covers.
- [ ] **Step 2: Delete and edit** per decisions 1-5. `HERMES_ENABLE_INTL` ON gives `message(WARNING ...)` naming `HERMES_BUILD_INTL`; check it with a throwaway configure under `.superpowers/p7-builds/`.
- [ ] **Step 3: Verify:** `grep -rn HERMES_ENABLE_INTL` over the tree (excluding build dirs, `external/`, `.superpowers/`, and historical files under `doc/superpowers/`) finds only the warning, the docs Task 5 rewrites, and the files Tasks 2-4 own (`android/build.gradle.kts`, `android/intltest/`, `utils/build-apple-framework.sh`, `.github/workflows/build.yml`, `.circleci/config.yml`); Task 5 repeats the search with no exceptions but the warning; `check-intl`, `check-hermes`, `check-shermes` per the Global Constraints; `IntlLoaderTests` and the unit tests pass; list the deleted lit tests and the count change.
- [ ] **Step 4: Commit.**

### Task 2: Android

**Files:**
- Delete: `android/intltest/`.
- Modify: `android/build.gradle.kts`, `android/settings.gradle.kts`, build-logic files only `intltest` used, `intl/test/android/smoke.cpp`, `intl/test/android/app/src/main/cpp/IntlApp.cpp`, the emulator scripts or README text that expect the old `Intl`.
- Test: the AAR, the emulator smoke test (plugin present and absent) and app (plugin present and removed), plugin-mode test262 on the device.

**Interfaces:**
- Consumes Task 1's VM.

- [ ] **Step 1: Record the before state** from the Phase 6 build directories: `libhermesvm.so` size per ABI, and the AAR's `com/facebook/hermes/intl` classes (`unzip -l` of `classes.jar`).
- [ ] **Step 2: Change** per decision 8. The no-plugin branches of the smoke test and the app check `typeof Intl === "undefined"`. Only the app (which has a JVM) then exercises the fallbacks that go through Android's Java Unicode class (`lib/Platform/Unicode/PlatformUnicodeJava.cpp`): `"a".localeCompare("b")`, `"I".toLocaleLowerCase()`, and `toLocaleString`, `toLocaleDateString` and `toLocaleTimeString` of a finite date, printing each result without a JNI error. The smoke executable has no JVM, so it checks only `typeof Intl` and JVM-independent fallbacks (`Infinity.toLocaleString()`, `[1, 2].toLocaleString()`).
- [ ] **Step 3: Verify:** the AAR builds for the four ABIs and has no `com/facebook/hermes/intl` classes; `strings`/`nm` of `libhermesvm.so` show no `com/facebook/hermes/intl`; the emulator smoke test with the plugin (plugin's `Intl`) and without (no `Intl`, the JVM-independent fallback results, exit 0); the app with the plugin (plugin's `Intl`) and with `libhermesintl.so` removed (launches, no `Intl`, the fallback results); device plugin-mode test262 on x86_64 gives the host's totals; sizes after.
- [ ] **Step 4: Commit.**

### Task 3: Apple

**Files:**
- Modify: `utils/build-apple-framework.sh`, `hermes-engine.podspec` (comment), `intl/test/apple/smoke.cpp`, `intl/test/apple/podapp/main.mm`, `intl/test/apple/podapp/run-podapp.sh` expectations, the Mac-run instructions in `intl/README.md` that mention the old `Intl`.
- Test: "Mac commands" in the report.

**Interfaces:**
- Consumes Task 1's VM.

- [ ] **Step 1: Change** per decision 9. The no-plugin branches of the smoke test and the CocoaPods app check `typeof Intl === "undefined"` and then print the same fallback results as Task 2 (comparison, case conversion, finite-date `toLocale*String`), which on Apple go through CoreFoundation (`lib/Platform/Unicode/PlatformUnicodeCF.cpp`).
- [ ] **Step 2: Verify on this host** what can be: `bash -n` on the scripts; the lit tests that run the Apple helpers (`apple-framework.js`, `apple-xcframework.js`).
- [ ] **Step 3: Write the Mac commands:** rebuild the slices and both xcframeworks; `otool -L` of `hermesvm` per slice (Foundation present or not, and why); hermesvm sizes per slice before (round 2 of Phase 6: `mac44-reply.md` M4) and after; the iOS simulator smoke test with the plugin and without (no `Intl`, the fallback results); macOS and Catalyst runs; the CocoaPods app (iOS arm64 with and without the plugin, tvOS arm64) with the expected reports, the no-plugin one including the fallback results; `pod lib lint --platforms=ios,tvos` and `--platforms=visionos`.
- [ ] **Step 4: Commit.**

### Task 4: Test runners, skiplists and CI

**Files:**
- Modify: `utils/testsuite/cli.py`, `skiplist.py`, `typing_defs.py`, `skiplist.json`, `utils/testsuite/test_intl_plugin.py` (if it refers to the old help text), `tools/test-runner/{main.cpp,Skiplist.h}` (and its sources), `.github/workflows/build.yml`, `.circleci/config.yml`.
- Test: the runner's own unit tests (`utils/testsuite/test_*.py`), a normal-mode and a plugin-mode run of a few directories.

**Interfaces:**
- Independent of Tasks 2 and 3.

- [ ] **Step 1: Change** per decisions 6 and 7.
- [ ] **Step 2: Verify:** the runner's unit tests pass; `--test-intl` is rejected as unknown by both runners; through both `utils/testsuite` and `tools/test-runner`, a normal-mode run of `test262/test/intl402/Collator` together with a small non-Intl control directory (e.g. `test262/test/built-ins/Array/prototype/at`) reports every intl402 test skipped and the control tests run; the Python run is repeated with `--test-skiplist` to show it does not bypass the fixed rule; the decision 6 check of the Linux String `toLocale*Case` entries (normal mode, those two directories only) and the resulting keep or delete; plugin-mode `check-intl-test262` gives 2696/18/0; `test-runner` builds and `--help` no longer lists `--test-intl`; `actionlint` or `yamllint` on the two CI files if available, else a careful read.
- [ ] **Step 3: Commit.**

### Task 5: Docs, spec and acceptance

**Files:**
- Delete: `doc/IntlAPIs.md`.
- Modify: `doc/Features.md`, `doc/BuildingAndRunning.md`, `doc/CrossCompilation.md`, `intl/README.md`, `doc/superpowers/specs/2026-10-22-intl-plugin-design.md`.

- [ ] **Step 1: Run everything on Linux** (`check-intl`, `check-hermes`, `check-shermes`) and record the counts.
- [ ] **Step 2: Update the docs:** `Features.md` (Intl through the plugin; link to `intl/README.md`), `BuildingAndRunning.md` (remove the ICU/`HERMES_ENABLE_INTL` text, describe `HERMES_BUILD_INTL` and Rust, and the plugin-mode test262 example), `CrossCompilation.md`, `intl/README.md` (no "Phase 7 deletes", `-Xintl` deprecated and ignored).
- [ ] **Step 3: Update the spec:** "Coexistence with the old implementation" becomes "Without the plugin" (decision 2); "`RuntimeConfig`" and the Workers text (decision 4); "Packaging" (decisions 8, 9 and the deleted `android/intltest`); React Native (decision 10); sizes (decision 11, from Tasks 2 and 3; the Mac's numbers when they arrive); "Phasing" item 7 done. Every "until Phase 7" and "old `Intl`" in the spec is resolved.
- [ ] **Step 4: Commit.**

## After the tasks

When the whole-branch review is done and its findings are fixed:
1. Run a Codex code review (the `codex-review` skill) of the Phase 7 commits, from the plan commit to the tip, until every finding is CLOSED.
2. Rerun `check-intl`, `check-hermes` and `check-shermes` on Linux and the emulator runs; ask the Mac agent for Task 3's runs at the final commit; fix what fails.
3. Squash with the `squashing-feature-history` skill into one Phase 7 commit (fixes to an earlier phase fold into that phase). If the new tip's tree is identical to the backup tag's, delete the backup.
