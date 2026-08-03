#!/bin/bash
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

set -xe -o pipefail

THIS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
JS_DIR="$(cd "$THIS_DIR/.." && pwd)"
PACKAGE_DIR="$JS_DIR/hermes-parser-native"

# Path to the hermes include directory, used to drive codegen.
INCLUDE_PATH="$1"
if [[ ! -d "$INCLUDE_PATH" ]]; then
  echo "usage: build-native.sh <hermes-include-path> <prebuilds-dir>" 1>&2
  exit 1
fi

# Directory holding prebuilt addons, laid out as <platform>-<arch>/hermes-parser.node
PREBUILDS_SRC="$2"
if [[ ! -d "$PREBUILDS_SRC" ]]; then
  echo "usage: build-native.sh <hermes-include-path> <prebuilds-dir>" 1>&2
  exit 1
fi

# yarn (and the babel invocation below) must run with the workspace root
# (this directory's parent) as the working directory, regardless of where
# the caller invoked this script from. A subshell keeps the caller's cwd
# untouched.
(cd "$JS_DIR" && yarn install)

# Regenerate the kind hash that guards against ESTree.def drift. This is the
# only codegen step this script drives: the other generators under this
# directory (genESTreeJSON.js, genNodeDeserializers.js, etc.) write into the
# original hermes-parser/hermes-estree/hermes-transform packages, which are
# out of scope here.
node "$THIS_DIR/genKindHash.js" "$INCLUDE_PATH"

# Assemble dist from src.
DIST_DIR="$PACKAGE_DIR/dist"
rm -rf "$DIST_DIR"
cp -r "$PACKAGE_DIR/src" "$DIST_DIR"

find "$DIST_DIR" -type f -name "*.js" | while read -r file; do
  if grep -q " @flow" "$file"; then
    new_file="${file}.flow"
    if [ ! -f "$new_file" ]; then
      cp "$file" "$new_file"
    fi
  fi
done

rsync -a --include="*/" --include="*.js" --exclude="*" \
  "$PACKAGE_DIR/src" "$DIST_DIR"

# The .js files under dist/ still use Flow type syntax at this point (the
# copies above only produced the .js.flow *declaration* siblings). Strip
# Flow and transpile down to plain CommonJS so dist/index.js can be
# `require()`-d directly, matching what build.sh does for the sibling
# packages' dist directories.
(cd "$JS_DIR" && yarn babel --config-file="$JS_DIR/babel.config.js" "$DIST_DIR" --out-dir="$DIST_DIR")

# Copy prebuilt addons into the package.
#
# A package with a missing prebuild is broken on that platform, so a missing
# one is a hard error by default: otherwise this script can report success
# while producing a package with no binaries at all. Set
# HERMES_PARSER_ALLOW_MISSING_PREBUILDS=1 to downgrade it to a warning, which
# is what a local single-platform build wants.
rm -rf "$PACKAGE_DIR/prebuilds"
mkdir -p "$PACKAGE_DIR/prebuilds"
missing=()
for target in linux-x64 linux-arm64 darwin-x64 darwin-arm64; do
  if [[ -f "$PREBUILDS_SRC/$target/hermes-parser.node" ]]; then
    mkdir -p "$PACKAGE_DIR/prebuilds/$target"
    cp "$PREBUILDS_SRC/$target/hermes-parser.node" \
      "$PACKAGE_DIR/prebuilds/$target/"
  else
    missing+=("$target")
  fi
done

if [[ ${#missing[@]} -gt 0 ]]; then
  if [[ "${HERMES_PARSER_ALLOW_MISSING_PREBUILDS:-0}" == "1" ]]; then
    echo "WARNING: missing prebuilds: ${missing[*]}" 1>&2
  else
    echo "ERROR: missing prebuilds: ${missing[*]}" 1>&2
    echo "Build them under $PREBUILDS_SRC/<platform>-<arch>/, or set" 1>&2
    echo "HERMES_PARSER_ALLOW_MISSING_PREBUILDS=1 to package anyway." 1>&2
    exit 1
  fi
fi
