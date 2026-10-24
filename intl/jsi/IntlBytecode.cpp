/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include <cstddef>
#include <cstdint>
#include <utility>

namespace hermes_intl {

/// The plugin's JS (intl/js), compiled by this build's hermesc.
std::pair<const uint8_t *, size_t> intlBytecode() {
  // Hermes requires bytecode buffers to be 4-byte aligned.
  alignas(4) static const uint8_t kBytecode[] = {
#include "IntlBytecode.inc"
  };
  return {kBytecode, sizeof(kBytecode)};
}

} // namespace hermes_intl
