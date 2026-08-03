/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_TOOLS_HERMESPARSERNATIVE_KINDHASH_H
#define HERMES_TOOLS_HERMESPARSERNATIVE_KINDHASH_H

#include <cstdint>

namespace hermes {

/// The ordered list of node-kind names, derived from ESTree.def.
///
/// Index \c i corresponds to \c NodeKind \c i, which is the same indexing the
/// generated JavaScript NODE_DESERIALIZERS array uses. The wire format emits
/// <tt>kind + 1</tt> so that zero can mean "null node".
static const char *const kNodeKindNames[] = {
#define ESTREE_NODE_0_ARGS(NAME, ...) #NAME,
#define ESTREE_NODE_1_ARGS(NAME, ...) #NAME,
#define ESTREE_NODE_2_ARGS(NAME, ...) #NAME,
#define ESTREE_NODE_3_ARGS(NAME, ...) #NAME,
#define ESTREE_NODE_4_ARGS(NAME, ...) #NAME,
#define ESTREE_NODE_5_ARGS(NAME, ...) #NAME,
#define ESTREE_NODE_6_ARGS(NAME, ...) #NAME,
#define ESTREE_NODE_7_ARGS(NAME, ...) #NAME,
#define ESTREE_NODE_8_ARGS(NAME, ...) #NAME,
#define ESTREE_NODE_9_ARGS(NAME, ...) #NAME,
#define ESTREE_NODE_10_ARGS(NAME, ...) #NAME,
#define ESTREE_FIRST(NAME, ...) #NAME "First",
#define ESTREE_LAST(NAME) #NAME "Last",
#include "hermes/AST/ESTree.def"
};

/// \return an FNV-1a hash over every entry of \c kNodeKindNames, each followed
/// by a newline. Any insertion, removal, or reordering of node kinds changes
/// the result, which is what lets the JavaScript side detect that it was
/// generated from a different ESTree.def than the addon was built from.
inline uint32_t computeKindHash() {
  uint32_t hash = 0x811C9DC5u;
  const auto feedByte = [&hash](unsigned char c) {
    hash ^= (uint32_t)c;
    hash *= 16777619u;
  };

  for (const char *name : kNodeKindNames) {
    for (const char *p = name; *p != '\0'; ++p) {
      feedByte((unsigned char)*p);
    }
    feedByte((unsigned char)'\n');
  }

  return hash;
}

} // namespace hermes

#endif
