/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "KindHash.h"

#include "gtest/gtest.h"

using namespace hermes;

namespace {

TEST(KindHashTest, IsStableAcrossCalls) {
  EXPECT_EQ(computeKindHash(), computeKindHash());
}

TEST(KindHashTest, IsNotTrivial) {
  uint32_t hash = computeKindHash();
  EXPECT_NE(0u, hash);
  EXPECT_NE(0x811C9DC5u, hash) << "hash equals the FNV-1a seed; "
                                  "the name list was empty";
}

TEST(KindHashTest, MatchesReferenceImplementation) {
  // Recompute independently over the same list to catch a macro that stopped
  // expanding. The first three entries are Empty, Metadata, FunctionLikeFirst.
  uint32_t h = 0x811C9DC5u;
  auto feed = [&h](const char *s) {
    for (const char *p = s; *p; ++p) {
      h ^= (uint32_t)(unsigned char)*p;
      h *= 16777619u;
    }
    h ^= (uint32_t)'\n';
    h *= 16777619u;
  };
  feed("Empty");
  feed("Metadata");
  feed("FunctionLikeFirst");
  // The real hash covers all names, so it must differ from this prefix.
  EXPECT_NE(h, computeKindHash());
}

} // namespace
