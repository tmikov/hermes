#!/bin/bash
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Run by apple-framework.js: print "<slice>: <triples>" for each slice of
# utils/build-apple-framework.sh, from its rust_targets command, which CI
# passes to rustup and the script checks before building. $1 is the cmake
# the script runs.
set -e
# The script traces every command (set -x) when CI is set.
unset CI
export PATH="$(dirname "$1"):$PATH"
script="$(dirname "$0")/../../../../utils/build-apple-framework.sh"
for slice in macosx iphoneos iphonesimulator catalyst xros xrsimulator \
             appletvos appletvsimulator; do
  echo "$slice: $(bash "$script" rust_targets "$slice")"
done
