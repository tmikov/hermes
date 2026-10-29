# Testing the Intl plugin

This document covers the Intl plugin's tests beyond running `check-intl`:
the test262 checkout, updating the expected failures, and testing on
Android devices, Apple platforms and Windows. The [README](../README.md)
is the introduction; `intl/CMakeLists.txt` defines what `check-intl` runs.

## test262

`check-intl` runs test262 in plugin mode (`utils/test_runner.py
--intl-plugin`) and needs a checkout at the commit named on the
`test262-commit:` line of `test/test262-expectations.txt`
(`c9aa6126e8f6fe2c6f668c65bfba771ffc8a6096`, the one CI uses). Point the
build at it:

```
git clone https://github.com/tc39/test262
git -C test262 checkout c9aa6126e8f6fe2c6f668c65bfba771ffc8a6096
cmake ... -DHERMES_INTL_TEST262_DIR=$PWD/test262
```

Without `HERMES_INTL_TEST262_DIR`, `check-intl` fails and says so. The
runner also refuses a checkout at another commit unless given
`--intl-allow-commit-mismatch`.

The header of `test/test262-expectations.txt` says how each test is judged
against it. After a change that makes tests pass (or a new test262
commit), rewrite the file from a run:

```
python3 utils/test_runner.py --intl-plugin <build>/lib/libhermesintl.so \
  --intl-expectations intl/test/test262-expectations.txt \
  --intl-update-expectations -b <build>/bin \
  <test262>/test/intl402 <the built-ins paths from check-intl-test262>
```

The paths are the ones `check-intl-test262` passes in
`intl/CMakeLists.txt`. Every `UNEXPLAINED` entry must be investigated and
given a real reason by hand before `check-intl` passes again. The update
refuses to run if anything crashed or hit a plugin internal error.

## On an Android device

`utils/testsuite/adb-hermes` runs plugin-mode test262 on a device or
emulator; its header comment says what it does and which environment
variables it takes. Build `hermes` and `hermesintl` for the device's ABI,
and the host Hermes the cross build's `IMPORT_HOST_COMPILERS` came from,
then:

```
utils/testsuite/adb-hermes setup <android build> <host build> <bin dir>
python3 utils/test_runner.py -b <bin dir> \
  --intl-plugin <android build>/lib/libhermesintl.so \
  --intl-expectations intl/test/test262-expectations.txt \
  <test262>/test/intl402 <the built-ins paths from check-intl-test262>
```

The results must match the host's. The ARM translation of an x86_64
emulator ignores `LD_LIBRARY_PATH`, so for arm64 binaries there configure
the Android build with
`-DCMAKE_EXE_LINKER_FLAGS=-Wl,-rpath,<ADB_HERMES_DIR>/lib`.

### The smoke test

Run `hermes-intl-android-smoke` (`test/android/smoke.cpp`) in three
layouts: with the plugin next to `libhermesvm.so`, with the plugin only on
`LD_LIBRARY_PATH`, and with every library in an APK-style zip, loaded with
`LD_LIBRARY_PATH=<apk>!/lib/<abi>`. The zip must store the libraries
uncompressed and 16 KB aligned (`zip -0`, then `zipalign -P 16 -f 4`), as
AGP does.

### The test app

`test/android/app/` loads Hermes from the AAR through SoLoader, as React
Native does, with the libraries left in the APK, and logs what the plugin
gives it (tag `HermesIntlApp`); `build.gradle.kts` lists its properties.
Build the AAR, wrap it in a local Maven repository, and build the app
against it, from the checkout's root (`$PWD`); Gradle writes `build/`
there:

```
export ANDROID_HOME=<Android SDK> HERMES_WS_DIR=$PWD
android/gradlew -p android :assembleRelease
intl/test/android/app/make-maven-repo.sh \
  build/outputs/aar/hermes-engine-release.aar /tmp/hermes-maven
android/gradlew -p intl/test/android/app assembleDebug \
  -PhermesMavenRepo=/tmp/hermes-maven \
  -PhermesJniLibs=$PWD/build/intermediates/cmake/release/obj
adb install -r \
  intl/test/android/app/build/outputs/apk/debug/hermes-intl-test-app-debug.apk
adb logcat -c
adb shell am start -W -n com.facebook.hermes.intlapp/.MainActivity
adb logcat -d -s HermesIntlApp:I
```

The log should show `Intl.Segmenter: function`, `number: 1.234,5`, the
default locale and time zone, and the plugin's code mapped from
`base.apk`. Without `libhermesintl.so` in the APK the app still runs and
shows `typeof Intl: undefined` and the results of Hermes's own locale
fallbacks, including those that call Android's Java Unicode class.

## On Apple platforms

These run by hand on a Mac with Xcode, rustup and the Apple targets
(`utils/build-apple-framework.sh rust_targets <platform>`), and CocoaPods
for the consumer app.

- **A universal macOS host build** runs `check-intl` like any host build:
  configure with `-DCMAKE_OSX_ARCHITECTURES="arm64;x86_64"` and the usual
  test262 directory. To run test262 under Rosetta, give `test_runner.py
  -b` a directory whose `hermes` runs `arch -x86_64 <build>/bin/hermes
  "$@"`.
- **The slices and xcframeworks:** with `BUILD_TYPE=Release` (the slices
  are then MinSizeRel), `utils/build-apple-framework.sh <platform>` for
  each slice, then `utils/build-apple-framework.sh build_framework`.
- **The smoke host** (`test/apple/smoke.cpp`): after a slice is built,
  `cmake --build build_<platform> --target hermes-intl-apple-smoke`. For a
  simulator, run it with `xcrun simctl spawn <device> <path>`; to test
  default detection there, set `AppleLanguages` with `xcrun simctl spawn
  <device> defaults write -g AppleLanguages -array de-DE`, and the zone
  with `SIMCTL_CHILD_TZ=<zone>`.
- **The hermes-ios tarball**, from the checkout's root:

  ```
  node -e 'console.log(require("./utils/scripts/hermes/hermes-utils")
    .createHermesPrebuiltArtifactsTarball(process.cwd(), "Release",
      "<out dir>", true))'
  ```
- **The CocoaPods consumer app:** `intl/test/apple/podapp/run-podapp.sh`
  builds an app against the tarball and runs it on a new simulator; its
  header comment has the usage. The plugin must be linked as
  `LC_LOAD_WEAK_DYLIB`, and with `--remove-plugin` the app must still
  launch and report `typeof Intl: undefined`.
- **`pod lib lint`:** `env "hermes-artifact-url=file://<tarball>" pod lib
  lint hermes-engine.podspec --platforms=ios,tvos,macos --allow-warnings`,
  and the same with `--platforms=visionos`. With Xcode 27.1 or later, the
  macOS lint fails because the podspec's macOS minimum (10.15) is below
  Xcode's 12.0, so lint `--platforms=ios,tvos` there.

## On Windows

The Windows build runs under Wine on Linux, test262 included: see
[Running the Windows build under Wine](Building.md#running-the-windows-build-under-wine).
