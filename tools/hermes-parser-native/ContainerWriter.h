/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_TOOLS_HERMESPARSERNATIVE_CONTAINERWRITER_H
#define HERMES_TOOLS_HERMESPARSERNATIVE_CONTAINERWRITER_H

#include <cstdint>
#include <cstring>
#include <type_traits>
#include <vector>

#include "HermesParserJSSerializer.h"
#include "KindHash.h"
#include "StringTable.h"

namespace hermes {

/// The position region is written by memcpy-ing raw \c PositionResult objects,
/// and the JavaScript deserializer reads it back as exactly five consecutive
/// uint32 words per entry (HermesParserDeserializer.fillLocs). Both properties
/// are pinned here because nothing else on either side enforces them: adding a
/// field or letting the compiler insert padding would silently shift every
/// subsequent position, and the container's own bounds checks would still
/// pass.
static_assert(
    sizeof(PositionResult) == 20,
    "PositionResult must be exactly five uint32 words; the JavaScript "
    "deserializer reads 5 words per position entry");
static_assert(
    std::is_trivially_copyable<PositionResult>::value,
    "PositionResult is memcpy-ed into the container, so it must be "
    "trivially copyable");

/// Size of the container header in bytes. Chosen so that the program region,
/// which immediately follows, starts 8-byte aligned. The JavaScript
/// deserializer creates a Float64Array view over that region, which requires
/// 8-byte alignment, and its number-padding logic depends on index parity
/// being preserved.
static constexpr uint32_t kHeaderSize = 48;

/// Magic value identifying a hermes-parser-native container: 'HMPR'.
static constexpr uint32_t kContainerMagic = 0x484D5052;

/// Version of the container layout. Bump on any incompatible change.
static constexpr uint32_t kContainerVersion = 1;

/// Serialize \p program, \p positions and \p strings into a single
/// self-contained buffer. See the header table in the design spec for the
/// field layout.
inline std::vector<uint8_t> writeContainer(
    const std::vector<uint32_t> &program,
    const std::vector<PositionResult> &positions,
    const NativeStringTable &strings) {
  const uint32_t programBytes = (uint32_t)(program.size() * sizeof(uint32_t));
  const uint32_t positionBytes =
      (uint32_t)(positions.size() * sizeof(PositionResult));
  const uint32_t strOffsetsBytes =
      (uint32_t)(strings.offsets().size() * sizeof(uint32_t));
  const uint32_t strDataBytes = (uint32_t)strings.data().size();

  const uint32_t programOffset = kHeaderSize;
  const uint32_t positionOffset = programOffset + programBytes;
  const uint32_t strOffsetsOffset = positionOffset + positionBytes;
  const uint32_t strDataOffset = strOffsetsOffset + strOffsetsBytes;
  const uint32_t total = strDataOffset + strDataBytes;

  std::vector<uint8_t> buf(total, 0);

  const uint32_t header[] = {
      kContainerMagic,
      kContainerVersion,
      computeKindHash(),
      programOffset,
      (uint32_t)program.size(),
      positionOffset,
      (uint32_t)positions.size(),
      strOffsetsOffset,
      strings.count(),
      strDataOffset,
      strDataBytes,
      0,
  };
  static_assert(sizeof(header) == kHeaderSize, "header size mismatch");
  memcpy(buf.data(), header, sizeof(header));

  if (programBytes != 0) {
    memcpy(buf.data() + programOffset, program.data(), programBytes);
  }
  if (positionBytes != 0) {
    memcpy(buf.data() + positionOffset, positions.data(), positionBytes);
  }
  if (strOffsetsBytes != 0) {
    memcpy(
        buf.data() + strOffsetsOffset,
        strings.offsets().data(),
        strOffsetsBytes);
  }
  if (strDataBytes != 0) {
    memcpy(buf.data() + strDataOffset, strings.data().data(), strDataBytes);
  }

  return buf;
}

} // namespace hermes

#endif
