#!/bin/bash
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Run by apple-xcframework.js: make a fake hermesintl.xcframework in $2 with
# the eight entries xcodebuild -create-xcframework writes for the plugin
# (each slice directory holds hermesintl.framework/marker), run
# utils/intl-xcframework-sim-slices.py on it, print the entries and the
# slice directories, then run the script again and say whether it changed
# anything. Then show the errors: a fresh xcframework whose tvOS entry is
# valid but whose visionOS entry is not is left unchanged, and a malformed
# Info.plist is reported. $1 is the Python interpreter.
set -e
PY=$1
DIR=$2
script="$(dirname "$0")/../../../../utils/intl-xcframework-sim-slices.py"
rm -rf "$DIR"

# make_xcf <xcframework> [bad-xros]: write the fake xcframework. With
# bad-xros, the visionOS simulator entry is xros-arm64e-simulator (arm64e),
# which the script does not expect.
make_xcf() {
  mkdir -p "$1"
  "$PY" - "$@" <<'EOF'
import os, plistlib, sys
xcf = sys.argv[1]
bad_xros = sys.argv[2:] == ["bad-xros"]
# (identifier, architectures, platform, variant, binary path), as in the
# plugin's xcframework (mac #43, round 1 step 6).
entries = [
    ("ios-arm64", ["arm64"], "ios", None, "hermesintl"),
    ("ios-arm64_x86_64-maccatalyst", ["arm64", "x86_64"], "ios",
     "maccatalyst", "Versions/1/hermesintl"),
    ("ios-arm64_x86_64-simulator", ["arm64", "x86_64"], "ios", "simulator",
     "hermesintl"),
    ("macos-arm64_x86_64", ["arm64", "x86_64"], "macos", None,
     "Versions/1/hermesintl"),
    ("tvos-arm64", ["arm64"], "tvos", None, "hermesintl"),
    ("tvos-arm64-simulator", ["arm64"], "tvos", "simulator", "hermesintl"),
    ("xros-arm64", ["arm64"], "xros", None, "hermesintl"),
    ("xros-arm64e-simulator", ["arm64e"], "xros", "simulator", "hermesintl")
    if bad_xros else
    ("xros-arm64-simulator", ["arm64"], "xros", "simulator", "hermesintl"),
]
libs = []
for ident, archs, platform, variant, binary in entries:
    lib = {
        "BinaryPath": "hermesintl.framework/" + binary,
        "LibraryIdentifier": ident,
        "LibraryPath": "hermesintl.framework",
        "SupportedArchitectures": archs,
        "SupportedPlatform": platform,
    }
    if variant:
        lib["SupportedPlatformVariant"] = variant
    libs.append(lib)
    os.makedirs(os.path.join(xcf, ident, "hermesintl.framework"))
    with open(os.path.join(xcf, ident, "hermesintl.framework", "marker"), "w") as f:
        f.write(ident + "\n")
with open(os.path.join(xcf, "Info.plist"), "wb") as f:
    plistlib.dump({"AvailableLibraries": libs, "CFBundlePackageType": "XFWK",
                   "XCFrameworkFormatVersion": "1.0"}, f)
EOF
}

# dump <xcframework>: print each entry, sorted by identifier, with which
# slice's marker its directory holds, then the slice directories.
dump() {
  "$PY" - "$1" <<'EOF'
import os, plistlib, sys
xcf = sys.argv[1]
with open(os.path.join(xcf, "Info.plist"), "rb") as f:
    plist = plistlib.load(f)
for lib in sorted(plist["AvailableLibraries"],
                  key=lambda l: l["LibraryIdentifier"]):
    ident = lib["LibraryIdentifier"]
    marker = os.path.join(xcf, ident, lib["LibraryPath"], "marker")
    origin = open(marker).read().strip() if os.path.exists(marker) else "MISSING"
    print("entry %s %s %s %s %s from %s" % (
        ident, ",".join(lib["SupportedArchitectures"]),
        lib["SupportedPlatform"], lib.get("SupportedPlatformVariant", "-"),
        lib["BinaryPath"], origin))
print("dirs " + " ".join(sorted(
    d for d in os.listdir(xcf) if os.path.isdir(os.path.join(xcf, d)))))
EOF
}

# snapshot <xcframework>: the Info.plist bytes and every path under it.
snapshot() {
  (cd "$1" && cat Info.plist && find . | LC_ALL=C sort)
}

# run_failing <xcframework>: run the script, expecting it to fail, and say
# whether it changed the xcframework.
run_failing() {
  local before status=0
  before=$(snapshot "$1")
  "$PY" "$script" "$1" || status=$?
  echo "exit status $status"
  if [[ "$(snapshot "$1")" == "$before" ]]; then
    echo "xcframework unchanged"
  else
    echo "xcframework CHANGED"
  fi
}

XCF=$DIR/hermesintl.xcframework
make_xcf "$XCF"
echo "first run:"
"$PY" "$script" "$XCF"
dump "$XCF"
before=$(snapshot "$XCF")

echo "second run:"
"$PY" "$script" "$XCF"
if [[ "$(snapshot "$XCF")" == "$before" ]]; then
  echo "second run changed nothing"
else
  echo "second run CHANGED the xcframework"
fi

echo "valid tvOS entry, unexpected visionOS entry:"
BAD=$DIR/bad/hermesintl.xcframework
make_xcf "$BAD" bad-xros
run_failing "$BAD"

echo "malformed Info.plist:"
MAL=$DIR/malformed/hermesintl.xcframework
make_xcf "$MAL"
echo '<?xml version="1.0"?><plist version="1.0"><dict><key>Avail' > "$MAL/Info.plist"
run_failing "$MAL"
