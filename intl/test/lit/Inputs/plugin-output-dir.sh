#!/bin/bash
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Run by plugin-output-dir.js: configures Inputs/plugin-output-dir with each
# generator and hermesvm setting, and prints each configuration's hermesvm
# and plugin directories, relative to the build directory, and whether they
# are the same. $1 is cmake; $2 is a scratch directory.
set -e
cmake=$1
t=$2
src="$(cd "$(dirname "$0")" && pwd)/plugin-output-dir"
rm -rf "$t"
mkdir -p "$t"

# Configure case $1 with generator $2 and the remaining arguments.
run() {
  local name=$1 gen=$2 b="$t/$1"
  shift 2
  local configs=-DCMAKE_BUILD_TYPE=Release
  [[ $gen == "Ninja Multi-Config" ]] &&
    configs="-DCMAKE_CONFIGURATION_TYPES=Debug;Release"
  "$cmake" -S "$src" -B "$b" -G "$gen" "$configs" "$@" > "$b.log" 2>&1 ||
    { cat "$b.log"; exit 1; }
  for f in "$b"/dirs-*.txt; do
    local config=${f##*/dirs-} vm plugin
    vm=$(sed -n 's/^vm: //p' "$f")
    plugin=$(sed -n 's/^plugin: //p' "$f")
    local same=different
    [[ $vm == "$plugin" ]] && same=same
    echo "$name ${config%.txt}: vm ${vm#$b/}, plugin ${plugin#$b/}: $same"
  done
}

run framework-single Ninja -DFAKE_FRAMEWORK=ON
run framework-multi "Ninja Multi-Config" -DFAKE_FRAMEWORK=ON
run framework-multi-dirs "Ninja Multi-Config" -DFAKE_FRAMEWORK=ON \
  -DVM_DIR=fw -DVM_RELEASE_DIR=fw-release
run plain-single Ninja
run plain-multi "Ninja Multi-Config"
