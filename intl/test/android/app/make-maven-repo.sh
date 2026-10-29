#!/bin/bash
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Make a local Maven repository that holds a hermes-android AAR as
# com.facebook.hermes:hermes-android:<version>, for this test app
# (-PhermesMavenRepo=<repo dir>). The POM lists no dependencies; the app
# names the ones it needs.
#
#   make-maven-repo.sh <aar> <repo dir> [version (default 0.0.0)]
#
# <repo dir> is emptied first, so it must be a new directory or one this
# script made before (it holds the marker file .hermes-maven-repo).

set -eu
[ $# -ge 2 ] || {
  echo "usage: make-maven-repo.sh <aar> <repo dir> [version]" >&2
  exit 2
}
aar=$1
repo=$2
version=${3:-0.0.0}
[ -f "$aar" ] || {
  echo "make-maven-repo.sh: no $aar" >&2
  exit 1
}
if [ -z "$repo" ]; then
  echo "make-maven-repo.sh: empty repo dir" >&2
  exit 1
fi
if [ -e "$repo" ] && [ ! -f "$repo/.hermes-maven-repo" ]; then
  echo "make-maven-repo.sh: $repo exists and was not made by this script;" \
    "remove it or name a new directory" >&2
  exit 1
fi
dir=$repo/com/facebook/hermes/hermes-android/$version
rm -rf "$repo"
mkdir -p "$dir"
touch "$repo/.hermes-maven-repo"
cp "$aar" "$dir/hermes-android-$version.aar"
cat >"$dir/hermes-android-$version.pom" <<POM
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.facebook.hermes</groupId>
  <artifactId>hermes-android</artifactId>
  <version>$version</version>
  <packaging>aar</packaging>
</project>
POM
echo "$dir"
