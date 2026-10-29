#!/usr/bin/env python3
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

"""Make the Intl plugin's tvOS and visionOS simulator slices match hermesvm's.

Usage: intl-xcframework-sim-slices.py <path to hermesintl.xcframework>

The plugin's tvOS and visionOS simulator binaries are arm64 only (Rust has no
supported x86_64 target there), while hermesvm's are arm64 and x86_64. A
simulator build of an app for x86_64, or for arm64 and x86_64, takes only a
slice that lists every architecture it builds, and CocoaPods' "[CP] Copy
XCFrameworks" phase then copies the slice named like hermesvm's
(<p>-arm64_x86_64-simulator) from every xcframework of the pod. So each
<p>-arm64-simulator entry (p = tvos, xros) is rewritten to list x86_64 too and
renamed, with its directory, to <p>-arm64_x86_64-simulator. The binary stays
arm64 only: the linker ignores it for x86_64 (with the weak link nothing
refers to it), the embed phase copies it, and an x86_64 process finds no
usable plugin, which the loader treats as no plugin.

Every other entry is left alone, and so is an entry that already lists
x86_64, so a second run changes nothing. Anything unexpected is an error
(exit status 1) and nothing is changed.
"""

import os
import plistlib
import sys
import xml.parsers.expat

# The platforms whose simulator plugin is arm64 only.
PLATFORMS = ("tvos", "xros")


def fail(msg):
    print("error: %s" % msg, file=sys.stderr)
    sys.exit(1)


def main(argv):
    if len(argv) != 2:
        fail("usage: intl-xcframework-sim-slices.py <xcframework>")
    xcf = argv[1]
    plist_path = os.path.join(xcf, "Info.plist")
    try:
        with open(plist_path, "rb") as f:
            plist = plistlib.load(f)
    except (
        OSError,
        ValueError,
        plistlib.InvalidFileException,
        xml.parsers.expat.ExpatError,
    ) as e:
        fail("cannot read %s: %s" % (plist_path, e))
    libs = plist.get("AvailableLibraries") if isinstance(plist, dict) else None
    if not isinstance(libs, list) or not all(isinstance(l, dict) for l in libs):
        fail("%s has no list of AvailableLibraries" % plist_path)

    # Check every entry and directory before changing anything.
    renames = []
    for lib in libs:
        platform = lib.get("SupportedPlatform")
        archs = lib.get("SupportedArchitectures", [])
        if (
            platform not in PLATFORMS
            or lib.get("SupportedPlatformVariant") != "simulator"
            or "x86_64" in archs
        ):
            continue
        old_id = "%s-arm64-simulator" % platform
        new_id = "%s-arm64_x86_64-simulator" % platform
        if archs != ["arm64"] or lib.get("LibraryIdentifier") != old_id:
            fail(
                "%s: unexpected %s simulator entry %s (%s); expected %s (arm64)"
                % (
                    xcf,
                    platform,
                    lib.get("LibraryIdentifier"),
                    ",".join(archs),
                    old_id,
                )
            )
        if not os.path.isdir(os.path.join(xcf, old_id)):
            fail("%s: no slice directory %s" % (xcf, old_id))
        if os.path.lexists(os.path.join(xcf, new_id)):
            fail("%s: %s already exists" % (xcf, new_id))
        renames.append((lib, old_id, new_id))

    if not renames:
        return
    for lib, old_id, new_id in renames:
        lib["LibraryIdentifier"] = new_id
        lib["SupportedArchitectures"] = ["arm64", "x86_64"]
    # Write the new Info.plist next to the old one, rename the directories,
    # then replace the Info.plist; on a failure, undo what was done.
    tmp = plist_path + ".tmp"
    done = []
    try:
        with open(tmp, "wb") as f:
            plistlib.dump(plist, f)
        for _, old_id, new_id in renames:
            os.rename(os.path.join(xcf, old_id), os.path.join(xcf, new_id))
            done.append((old_id, new_id))
        os.replace(tmp, plist_path)
    except OSError as e:
        for old_id, new_id in reversed(done):
            os.rename(os.path.join(xcf, new_id), os.path.join(xcf, old_id))
        if os.path.exists(tmp):
            os.remove(tmp)
        fail("could not rewrite %s: %s" % (xcf, e))
    for _, old_id, new_id in renames:
        print(
            "%s: %s -> %s (lists x86_64; the binary is arm64 only)"
            % (os.path.basename(xcf), old_id, new_id)
        )


if __name__ == "__main__":
    main(sys.argv)
