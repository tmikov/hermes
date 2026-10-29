#!/bin/bash
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Build the CocoaPods consumer app of the Intl plugin against a hermes-ios
# tarball, run it on a new simulator, and print what it reports.
#
#   run-podapp.sh <hermes-ios tarball> <ios|tvos> <archs> [--remove-plugin]
#
# The app (main.mm, PodApp.xcodeproj, Podfile) is copied to a work directory
# ($PODAPP_WORK, default /tmp/hermes-podapp), which is emptied first, so it
# must be absent, empty, or one this script made before (it holds the marker
# file .hermes-podapp-work); any other directory is refused, never deleted.
# There `pod install` takes
# hermes-engine from this repository's podspec with hermes-artifact-url set
# to the tarball. The app is built for the simulator of the platform, for
# <archs>: arm64, x86_64, or both as one argument ("arm64 x86_64", as a
# Release or "Any Simulator Device" build has); the simulator runs it as the
# Mac's own architecture when it has it. --remove-plugin deletes
# hermesintl.framework from the built app (and re-signs it) before it is
# installed. $HERMES_PODSPEC names another podspec to use instead of the
# repository's.
#
# It prints the app's link commands for Hermes's frameworks
# (LC_LOAD_DYLIB or LC_LOAD_WEAK_DYLIB), the frameworks in the app, and the
# app's report: "Intl.Segmenter: function" means Intl comes from the plugin,
# "undefined" means there is no plugin. Needs Xcode, CocoaPods and python3.

set -euo pipefail

die() {
  echo "run-podapp: $*" >&2
  exit 1
}

[[ $# -ge 3 ]] || die "usage: run-podapp.sh <hermes-ios tarball> <ios|tvos> <arm64|x86_64|\"arm64 x86_64\"> [--remove-plugin]"
TARBALL=$1
PLATFORM=$2
ARCH=$3
REMOVE_PLUGIN=${4:-}
[[ -f "$TARBALL" ]] || die "no tarball at $TARBALL"
TARBALL=$(cd "$(dirname "$TARBALL")" && pwd)/$(basename "$TARBALL")

case "$PLATFORM" in
  ios)
    SCHEME=PodApp-iOS SDK=iphonesimulator RUNTIME_PREFIX=iOS DEVICE_PREFIX=iPhone
    BUNDLE_ID=dev.hermesengine.intl.podapp.ios ;;
  tvos)
    SCHEME=PodApp-tvOS SDK=appletvsimulator RUNTIME_PREFIX=tvOS DEVICE_PREFIX="Apple TV"
    BUNDLE_ID=dev.hermesengine.intl.podapp.tvos ;;
  *) die "unknown platform $PLATFORM (ios or tvos)" ;;
esac
[[ -n "$ARCH" ]] || die "no architecture given"
for a in $ARCH; do
  case "$a" in
    arm64|x86_64) ;;
    *) die "unknown architecture $a (arm64 or x86_64)" ;;
  esac
done

SRC=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$SRC/../../../.." && pwd)
WORK=${PODAPP_WORK:-/tmp/hermes-podapp}

MARKER=.hermes-podapp-work
# Ownership is checked before the marker is written, so an existing
# directory never becomes this script's just before it is deleted.
if [[ -e "$WORK" || -L "$WORK" ]]; then
  if [[ -d "$WORK" && ! -L "$WORK" && -f "$WORK/$MARKER" ]]; then
    rm -rf "$WORK"
  elif [[ -d "$WORK" && ! -L "$WORK" && -z "$(ls -A "$WORK")" ]]; then
    :
  else
    die "$WORK exists and is not a directory run-podapp.sh made (one that" \
      "holds $MARKER); remove it or set PODAPP_WORK to a new directory"
  fi
fi
mkdir -p "$WORK"
touch "$WORK/$MARKER"

echo "== pod install ($TARBALL)"
cp -R "$SRC/Podfile" "$SRC/main.mm" "$SRC/PodApp.xcodeproj" "$WORK/"
# CocoaPods caches what it downloaded by URL, and the tarball's URL stays the
# same when it is rebuilt.
pod cache clean hermes-engine --all > /dev/null 2>&1 || true
(
  cd "$WORK"
  # The podspec reads this variable; its name is not a shell identifier.
  env HERMES_PODSPEC="${HERMES_PODSPEC:-$REPO/hermes-engine.podspec}" \
    "hermes-artifact-url=file://$TARBALL" pod install
)

echo "== xcodebuild $SCHEME, $SDK, ARCHS=$ARCH"
LOG=$WORK/xcodebuild.log
if ! (
  cd "$WORK"
  xcodebuild -workspace PodApp.xcworkspace -scheme "$SCHEME" \
    -configuration Debug -sdk "$SDK" -derivedDataPath "$WORK/dd" \
    ARCHS="$ARCH" ONLY_ACTIVE_ARCH=NO build
) > "$LOG" 2>&1; then
  grep -E 'error:|warning: \[CP\]|ignoring file|PhaseScriptExecution|unbound variable|rsync' "$LOG" |
    head -40 >&2 || true
  die "xcodebuild failed; the log is $LOG"
fi
echo "-- CocoaPods slice selection and linker notes:"
grep -E 'Selected xcframework slice|Unable to find matching|ignoring file' "$LOG" |
  sort -u || true

APP=$WORK/dd/Build/Products/Debug-$SDK/$SCHEME.app
[[ -d "$APP" ]] || die "no app at $APP"
echo "-- link commands of $SCHEME for Hermes's frameworks (per architecture):"
# otool -l prints "cmd LC_LOAD_WEAK_DYLIB", "cmdsize ..." and then
# "name <path> (offset 24)" for each dylib; a universal binary prints an
# "(architecture ...)" header before each architecture's commands.
otool -l "$APP/$SCHEME" | awk '
  /\(architecture / { print }
  /^ *cmd / { c = $2 }
  /^ *name .*hermes/ { print c, $2; n++ }
  END { if (!n) print "(none)" }'
lipo -info "$APP/$SCHEME"
echo "-- frameworks in the app:"
ls "$APP/Frameworks" 2> /dev/null || echo "(none)"
if [[ -f "$APP/Frameworks/hermesintl.framework/hermesintl" ]]; then
  lipo -info "$APP/Frameworks/hermesintl.framework/hermesintl"
fi

if [[ "$REMOVE_PLUGIN" == "--remove-plugin" ]]; then
  echo "== removing hermesintl.framework from the app"
  rm -rf "$APP/Frameworks/hermesintl.framework"
  codesign --force --sign - --preserve-metadata=identifier,entitlements \
    --timestamp=none "$APP"
  ls "$APP/Frameworks"
fi

echo "== simulator"
read -r RUNTIME DEVTYPE < <(xcrun simctl list -j runtimes devicetypes |
  python3 -c '
import json, sys
prefix, device = sys.argv[1], sys.argv[2]
d = json.load(sys.stdin)
rts = [r for r in d["runtimes"]
       if r.get("isAvailable") and r["name"].startswith(prefix + " ")]
if not rts:
    sys.exit("no available %s simulator runtime" % prefix)
rt = max(rts, key=lambda r: [int(x) for x in r["version"].split(".")])
types = [t["identifier"] for t in rt.get("supportedDeviceTypes", [])
         if t["name"].startswith(device)]
if not types:
    types = [t["identifier"] for t in d["devicetypes"]
             if t["name"].startswith(device)]
print(rt["identifier"], types[0])
' "$RUNTIME_PREFIX" "$DEVICE_PREFIX")
UDID=$(xcrun simctl create "hermes-podapp-$PLATFORM" "$DEVTYPE" "$RUNTIME")
cleanup() {
  xcrun simctl shutdown "$UDID" > /dev/null 2>&1 || true
  xcrun simctl delete "$UDID" > /dev/null 2>&1 || true
}
trap cleanup EXIT
echo "device $UDID ($DEVTYPE, $RUNTIME)"
xcrun simctl bootstatus "$UDID" -b > /dev/null
xcrun simctl install "$UDID" "$APP"
echo "== launch (the app's stdout follows)"
xcrun simctl launch --console-pty --terminate-running-process "$UDID" "$BUNDLE_ID" || true
echo "== the app's report file"
DATA=$(xcrun simctl get_app_container "$UDID" "$BUNDLE_ID" data) ||
  die "the app has no data container; it did not launch (a crash at launch is logged in ~/Library/Logs/DiagnosticReports)"
cat "$DATA/tmp/podapp-result.txt" ||
  die "the app wrote no report; a crash at launch is logged in ~/Library/Logs/DiagnosticReports"
