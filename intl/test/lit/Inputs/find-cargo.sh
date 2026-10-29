#!/bin/bash
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Run by find-cargo.js: where hermes_intl_find_cargo (through
# Inputs/find-cargo.cmake) and utils/build-apple-framework.sh find cargo,
# each case in an environment of only PATH, CARGO_HOME and HOME. $1 is
# cmake; $2 is a scratch directory, shown as <t>. Fake cargos sit on a PATH
# directory, in $CARGO_HOME/bin and in $HOME/.cargo/bin; each names its
# place as its host triple, and the rustc next to it records that it ran
# and prints a sysroot with the macOS targets' standard libraries. The
# script cases source the script (whose entry point then only prints the
# macosx targets) and run its find_cargo and check_rust_for_platform.
set -e
cmake=$1
t=$2
rm -rf "$t"
mkdir -p "$t/empty" "$t/tools"
script="$(cd "$(dirname "$0")/../../../.." && pwd)/utils/build-apple-framework.sh"
bash=$(command -v bash)

# A fake cargo and rustc in $1, the cargo naming host $2.
fake_rust() {
  mkdir -p "$1"
  printf '#!/bin/sh\necho "host: %s"\n' "$2" > "$1/cargo"
  printf '#!/bin/sh\necho "$0" >> "%s/rustc.log"\necho "%s/sysroot"\n' \
    "$t" "$t" > "$1/rustc"
  chmod +x "$1/cargo" "$1/rustc"
}
fake_rust "$t/pathdir" path-host
fake_rust "$t/cargohome/bin" cargo-home-host
fake_rust "$t/home/.cargo/bin" home-host
mkdir -p "$t/sysroot/lib/rustlib/x86_64-apple-darwin/lib" \
  "$t/sysroot/lib/rustlib/aarch64-apple-darwin/lib" "$t/nocargo" "$t/nohome"

# The commands the script needs, and nothing else, for its PATH.
printf '#!/bin/sh\nexec "%s" "$@"\n' "$cmake" > "$t/tools/cmake"
chmod +x "$t/tools/cmake"
for c in dirname getconf; do
  ln -s "$(command -v $c)" "$t/tools/$c"
done

# Run case $1 with PATH $2, CARGO_HOME $3 and HOME $4 ("-" for unset).
run() {
  local name=$1 env=(PATH="$2")
  [[ $3 != - ]] && env+=(CARGO_HOME="$3")
  [[ $4 != - ]] && env+=(HOME="$4")
  echo "cmake, $name:"
  env -i "${env[@]}" "$cmake" -DROOT="$t" -P "$(dirname "$0")/find-cargo.cmake"
  env[0]="PATH=$t/tools:$2"
  rm -f "$t/rustc.log"
  echo "script, $name:"
  env -i "${env[@]}" "$bash" -c '
    source "$1" rust_targets macosx > /dev/null
    echo "cargo: [$(find_cargo)]"
    check_rust_for_platform macosx && echo "check passed"' _ "$script" \
    2>&1 | sed "s|$t|<t>|g" || true
  if [[ -f "$t/rustc.log" ]]; then
    sed "s|$t|<t>|g; s|^|rustc: |" "$t/rustc.log"
  fi
}

run "on PATH" "$t/pathdir" "$t/cargohome" "$t/home"
run "CARGO_HOME" "$t/empty" "$t/cargohome" "$t/home"
run "HOME" "$t/empty" - "$t/home"
run "CARGO_HOME without cargo" "$t/empty" "$t/nocargo" "$t/home"
run "none" "$t/empty" "$t/nocargo" "$t/nohome"
run "none, no CARGO_HOME or HOME" "$t/empty" - -
