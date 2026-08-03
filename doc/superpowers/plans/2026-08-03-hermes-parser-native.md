# hermes-parser-native Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fork `hermes-parser` into `hermes-parser-native`, an npm package that reaches the Hermes parser through a Node-API addon instead of a WebAssembly blob.

**Architecture:** A new C++ directory `tools/hermes-parser-native/` forks the wasm serializer, replacing embedded 32-bit pointers with a deduplicated string table and emitting the whole AST as one self-contained buffer. A new JavaScript workspace `tools/hermes-parser/js/hermes-parser-native/` forks the JS package; only two of its files diverge from the original. The existing wasm parser is never modified and serves as a byte-exact reference for differential testing.

**Tech Stack:** C++17 (no exceptions, no RTTI), CMake + Ninja, Node-API, Node 24, Yarn 1 workspaces, Jest.

**Spec:** `doc/superpowers/specs/2026-08-03-hermes-parser-native-design.md`

## Global Constraints

- **Never `cd` out of the repository root.** Pass paths to commands. If unavoidable, use a subshell: `(cd dir; cmd)`.
- **Copyright header on every new file**, C++ and JavaScript both:
  ```
  Copyright (c) Meta Platforms, Inc. and affiliates.

  This source code is licensed under the MIT license found in the
  LICENSE file in the root directory of this source tree.
  ```
- **C++ style:** C++17, no exceptions, no RTTI. Classes `PascalCase`, functions `camelCase`, member variables `trailing_`. 80-column limit, 2-space indent. Doc comment on every declaration.
- **Never modify** anything under `tools/hermes-parser/` except `tools/hermes-parser/js/package.json` (adding one workspace entry) and new files under `tools/hermes-parser/js/hermes-parser-native/` and `tools/hermes-parser/js/scripts/`.
- **Package name:** `hermes-parser-native`, unscoped. **Version:** `0.37.0` (monorepo lockstep). **Runtime dependencies:** exactly one, `hermes-estree`.
- **Platform matrix:** `linux-x64`, `linux-arm64`, `darwin-x64`, `darwin-arm64`. Windows is out of scope.
- **Build directories.** Two, deliberately:
  - `cmake-build-asan` — for the C++ gtest unit tests (Tasks 2-5). This is
    where the memory-safety risk lives (raw pointers, `memcpy`), and ASan
    works normally for a native executable.
  - `cmake-build-debug` — for the `.node` addon whenever Node loads it
    (Tasks 1, 6-12). An ASan-instrumented shared module **cannot be
    `dlopen`'d by a non-ASan `node`**; it fails with `undefined symbol:
    __asan_option_detect_stack_use_after_return`. This is CLAUDE.md's
    "specific reason not to" carve-out, not a casual opt-out. Do not work
    around it with `LD_PRELOAD`.

  Configure the non-ASan directory once with:
  ```bash
  cmake -B cmake-build-debug -G Ninja -DCMAKE_BUILD_TYPE=Debug \
    -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++
  ```
- The AST produced must be `deepEqual` to the wasm parser's for all inputs.

---

## File Structure

**C++ — `tools/hermes-parser-native/` (all new):**

| File | Responsibility |
| --- | --- |
| `CMakeLists.txt` | Builds the `.node` MODULE library |
| `hermes-parser-napi.cpp` | Node-API entry point; the only file that knows about N-API |
| `StringTable.h` | Deduplicating UTF-8 string table; standalone and unit-testable |
| `ContainerWriter.h` | Assembles header + four regions into one byte buffer |
| `HermesParserJSSerializer.h/.cpp` | Forked serializer; AST → program/position buffers |
| `HermesParserDiagHandler.h/.cpp` | Forked verbatim from the wasm tool |
| `KindHash.h` | Computes the node-kind table hash from `ESTree.def` |

**JavaScript — `tools/hermes-parser/js/hermes-parser-native/` (fork):**

Copied unchanged from `hermes-parser/`: everything except the two files below.

| File | Responsibility |
| --- | --- |
| `src/HermesParser.js` | **Diverges.** Loads the addon; calls `parse`; throws `SyntaxError` |
| `src/HermesParserDeserializer.js` | **Diverges.** Region views, string table, decode cache |
| `src/HermesParserAddon.js` | **New.** Platform resolution and addon loading |
| `src/HermesParserKindHash.js` | **New, generated.** The expected kind hash |

**Build scripts — `tools/hermes-parser/js/scripts/` (new files only):**

| File | Responsibility |
| --- | --- |
| `genKindHash.js` | Emits `HermesParserKindHash.js` from the ESTree JSON |
| `build-native.sh` | Codegen + dist assembly for the native package |

---

### Task 1: Addon skeleton that loads and is callable

Establishes the end-to-end loop first: a `.node` file that builds, loads in Node, and exports `parse`. Everything after this has a place to plug into.

**Files:**
- Create: `tools/hermes-parser-native/CMakeLists.txt`
- Create: `tools/hermes-parser-native/hermes-parser-napi.cpp`
- Modify: `tools/CMakeLists.txt` (add one `add_subdirectory` line)
- Test: `tools/hermes-parser-native/__tests__/smoke.js`

**Interfaces:**
- Consumes: nothing.
- Produces: a CMake target `hermes-parser-napi` whose output is
  `<build>/tools/hermes-parser-native/hermes-parser.node`, exporting a single
  function `parse(source: string, options: object)`.

- [ ] **Step 1: Write the failing test**

Create `tools/hermes-parser-native/__tests__/smoke.js`:

```js
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

'use strict';

const assert = require('assert');
const path = require('path');

const addonPath = process.argv[2];
assert.ok(addonPath, 'usage: node smoke.js <path-to-hermes-parser.node>');

const addon = require(path.resolve(addonPath));
assert.strictEqual(typeof addon.parse, 'function', 'parse must be exported');

let threw = null;
try {
  addon.parse('var x = 1;', {});
} catch (e) {
  threw = e;
}
assert.ok(threw, 'parse must throw while unimplemented');
assert.match(threw.message, /not implemented/);

console.log('smoke OK');
```

- [ ] **Step 2: Run it to verify it fails**

```bash
node tools/hermes-parser-native/__tests__/smoke.js cmake-build-debug/tools/hermes-parser-native/hermes-parser.node
```

Expected: FAIL — `Cannot find module` (nothing built yet).

- [ ] **Step 3: Write the CMake target**

Create `tools/hermes-parser-native/CMakeLists.txt`:

```cmake
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# A Node-API addon. Built as a MODULE library named "hermes-parser.node".
# The napi_* symbols are deliberately left undefined: they resolve from the
# host process at dlopen time, which is what allows one binary to work in
# both Node and any other Node-API host.
add_library(hermes-parser-napi MODULE
  hermes-parser-napi.cpp
  )

set_target_properties(hermes-parser-napi PROPERTIES
  PREFIX ""
  OUTPUT_NAME "hermes-parser"
  SUFFIX ".node"
  POSITION_INDEPENDENT_CODE ON
  )

target_include_directories(hermes-parser-napi PRIVATE
  ${HERMES_SOURCE_DIR}/include/hermes/napi
  )

if (APPLE)
  # Allow napi_* to be resolved by the loading process.
  target_link_options(hermes-parser-napi PRIVATE "SHELL:-undefined dynamic_lookup")
endif ()
```

Add to `tools/CMakeLists.txt`, immediately after the existing
`add_subdirectory(hermes-parser)` line:

```cmake
add_subdirectory(hermes-parser-native)
```

- [ ] **Step 4: Write the minimal addon**

Create `tools/hermes-parser-native/hermes-parser-napi.cpp`:

```cpp
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "node_api.h"

namespace {

/// Entry point for the exported `parse` function.
napi_value parse(napi_env env, napi_callback_info info) {
  napi_throw_error(env, nullptr, "hermes-parser-native: not implemented");
  return nullptr;
}

/// Module initializer. Registers `parse` on the exports object.
napi_value init(napi_env env, napi_value exports) {
  napi_value fn;
  if (napi_create_function(env, "parse", NAPI_AUTO_LENGTH, parse, nullptr,
                           &fn) != napi_ok) {
    return nullptr;
  }
  if (napi_set_named_property(env, exports, "parse", fn) != napi_ok) {
    return nullptr;
  }
  return exports;
}

} // namespace

NAPI_MODULE(NODE_GYP_MODULE_NAME, init)
```

- [ ] **Step 5: Build it**

```bash
cmake --build cmake-build-debug --target hermes-parser-napi
```

Expected: produces `cmake-build-debug/tools/hermes-parser-native/hermes-parser.node`.

If the build directory does not exist yet:

```bash
cmake -B cmake-build-asan -G Ninja -DCMAKE_BUILD_TYPE=Debug \
  -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ \
  -DHERMES_ENABLE_ADDRESS_SANITIZER=ON \
  -DCMAKE_CXX_FLAGS="-O1" -DCMAKE_C_FLAGS="-O1"
```

- [ ] **Step 6: Run the test to verify it passes**

```bash
node tools/hermes-parser-native/__tests__/smoke.js cmake-build-debug/tools/hermes-parser-native/hermes-parser.node
```

Expected: `smoke OK`.

- [ ] **Step 7: Commit**

```bash
git add tools/hermes-parser-native tools/CMakeLists.txt
git commit -m "Add hermes-parser-native addon skeleton"
```

---

### Task 2: String table

A standalone, unit-testable component. Interning identifiers is where the format's dedup and the downstream interning win both come from.

**Files:**
- Create: `tools/hermes-parser-native/StringTable.h`
- Test: `unittests/HermesParserNative/StringTableTest.cpp`
- Create: `unittests/HermesParserNative/CMakeLists.txt`
- Modify: `unittests/CMakeLists.txt` (add one `add_subdirectory` line)

**Interfaces:**
- Consumes: nothing.
- Produces: `hermes::StringTable` with
  `uint32_t intern(llvh::StringRef)`, `const std::string &data() const`,
  `const std::vector<uint32_t> &offsets() const`, `uint32_t count() const`.
  `offsets()` has `count() + 1` entries; string *i* is
  `data()[offsets()[i] .. offsets()[i+1]]`.

- [ ] **Step 1: Write the failing test**

Create `unittests/HermesParserNative/StringTableTest.cpp`:

```cpp
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "StringTable.h"

#include "gtest/gtest.h"

using namespace hermes;

namespace {

TEST(StringTableTest, EmptyTableHasOneOffset) {
  StringTable table;
  EXPECT_EQ(0u, table.count());
  ASSERT_EQ(1u, table.offsets().size());
  EXPECT_EQ(0u, table.offsets()[0]);
}

TEST(StringTableTest, InternReturnsSequentialIds) {
  StringTable table;
  EXPECT_EQ(0u, table.intern("foo"));
  EXPECT_EQ(1u, table.intern("bar"));
  EXPECT_EQ(2u, table.count());
}

TEST(StringTableTest, InternDeduplicates) {
  StringTable table;
  EXPECT_EQ(0u, table.intern("foo"));
  EXPECT_EQ(1u, table.intern("bar"));
  EXPECT_EQ(0u, table.intern("foo"));
  EXPECT_EQ(2u, table.count());
  EXPECT_EQ("foobar", table.data());
}

TEST(StringTableTest, OffsetsDelimitStrings) {
  StringTable table;
  table.intern("alpha");
  table.intern("be");
  ASSERT_EQ(3u, table.offsets().size());
  EXPECT_EQ(0u, table.offsets()[0]);
  EXPECT_EQ(5u, table.offsets()[1]);
  EXPECT_EQ(7u, table.offsets()[2]);
}

TEST(StringTableTest, HandlesEmptyString) {
  StringTable table;
  EXPECT_EQ(0u, table.intern(""));
  EXPECT_EQ(1u, table.count());
  EXPECT_EQ(0u, table.offsets()[0]);
  EXPECT_EQ(0u, table.offsets()[1]);
}

TEST(StringTableTest, HandlesEmbeddedNulAndUtf8) {
  StringTable table;
  llvh::StringRef withNul("a\0b", 3);
  EXPECT_EQ(0u, table.intern(withNul));
  EXPECT_EQ(1u, table.intern("\xC3\xA9"));
  EXPECT_EQ(3u, table.offsets()[1]);
  EXPECT_EQ(5u, table.offsets()[2]);
}

} // namespace
```

- [ ] **Step 2: Write the unittest CMake wiring**

Create `unittests/HermesParserNative/CMakeLists.txt`:

```cmake
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

set(HermesParserNativeTestSources
  StringTableTest.cpp
  )

add_hermes_unittest(HermesParserNativeTests
  ${HermesParserNativeTestSources}
  )

target_include_directories(HermesParserNativeTests PRIVATE
  ${HERMES_SOURCE_DIR}/tools/hermes-parser-native
  )

target_link_libraries(HermesParserNativeTests
  LLVHSupport
  )
```

Add to `unittests/CMakeLists.txt` alongside the other `add_subdirectory` calls:

```cmake
add_subdirectory(HermesParserNative)
```

- [ ] **Step 3: Run the test to verify it fails**

```bash
cmake --build cmake-build-asan --target HermesParserNativeTests
```

Expected: FAIL — `StringTable.h` does not exist.

- [ ] **Step 4: Write the implementation**

Create `tools/hermes-parser-native/StringTable.h`:

```cpp
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_TOOLS_HERMESPARSERNATIVE_STRINGTABLE_H
#define HERMES_TOOLS_HERMESPARSERNATIVE_STRINGTABLE_H

#include <string>
#include <vector>

#include "llvh/ADT/DenseMap.h"
#include "llvh/ADT/StringRef.h"

namespace hermes {

/// A deduplicating table of UTF-8 strings, serialized as a single blob of
/// bytes plus an offset array.
///
/// String \c i occupies <tt>data()[offsets()[i] .. offsets()[i+1]]</tt>, so
/// \c offsets() always holds one more entry than there are strings and no
/// separate length array is needed.
///
/// Keys are stored as \c StringRef, so callers must guarantee the referenced
/// bytes outlive the table. Every current caller satisfies this: identifiers
/// live in the parser's identifier table and comment and token text point
/// into the source buffer, both of which are owned by the \c Context that
/// outlives serialization.
class StringTable {
 public:
  /// Add \p str to the table, or return the existing id if already present.
  /// \return the id of the string, counting from zero.
  uint32_t intern(llvh::StringRef str) {
    auto it = index_.find(str);
    if (it != index_.end()) {
      return it->second;
    }

    uint32_t id = (uint32_t)(offsets_.size() - 1);
    data_.append(str.data(), str.size());
    offsets_.push_back((uint32_t)data_.size());
    index_.try_emplace(str, id);
    return id;
  }

  /// \return the concatenated UTF-8 bytes of every interned string.
  const std::string &data() const {
    return data_;
  }

  /// \return the offset array, with \c count() + 1 entries.
  const std::vector<uint32_t> &offsets() const {
    return offsets_;
  }

  /// \return the number of distinct strings interned.
  uint32_t count() const {
    return (uint32_t)(offsets_.size() - 1);
  }

 private:
  llvh::DenseMap<llvh::StringRef, uint32_t> index_{};
  std::vector<uint32_t> offsets_{0};
  std::string data_{};
};

} // namespace hermes

#endif
```

- [ ] **Step 5: Run the tests to verify they pass**

```bash
cmake --build cmake-build-asan --target HermesParserNativeTests && \
  cmake-build-asan/unittests/HermesParserNative/HermesParserNativeTests
```

Expected: all 5 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add tools/hermes-parser-native/StringTable.h unittests/HermesParserNative unittests/CMakeLists.txt
git commit -m "Add deduplicating StringTable for hermes-parser-native"
```

---

### Task 3: Kind hash

Both sides must agree on the node-kind ordering. C++ derives the hash from `ESTree.def` at compile time; Task 7 generates the matching JavaScript constant from the same source.

**Files:**
- Create: `tools/hermes-parser-native/KindHash.h`
- Test: `unittests/HermesParserNative/KindHashTest.cpp`
- Modify: `unittests/HermesParserNative/CMakeLists.txt:6` (add the new source)

**Interfaces:**
- Consumes: nothing.
- Produces: `uint32_t hermes::computeKindHash()` — FNV-1a over the ordered
  node-kind names, each followed by `'\n'`. Names for `ESTREE_FIRST(X)` and
  `ESTREE_LAST(X)` are `"XFirst"` and `"XLast"`; all others are the bare name.
  This ordering matches `NODE_DESERIALIZERS` index-for-index.

- [ ] **Step 1: Write the failing test**

Create `unittests/HermesParserNative/KindHashTest.cpp`:

```cpp
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
```

- [ ] **Step 2: Add the source to the unittest CMake**

In `unittests/HermesParserNative/CMakeLists.txt`, extend the source list:

```cmake
set(HermesParserNativeTestSources
  StringTableTest.cpp
  KindHashTest.cpp
  )
```

- [ ] **Step 3: Run the test to verify it fails**

```bash
cmake --build cmake-build-asan --target HermesParserNativeTests
```

Expected: FAIL — `KindHash.h` does not exist.

- [ ] **Step 4: Write the implementation**

Create `tools/hermes-parser-native/KindHash.h`:

```cpp
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
```

- [ ] **Step 5: Run the tests to verify they pass**

```bash
cmake --build cmake-build-asan --target HermesParserNativeTests && \
  cmake-build-asan/unittests/HermesParserNative/HermesParserNativeTests
```

Expected: all 8 tests PASS.

- [ ] **Step 6: Print the hash for later use**

Add a temporary throwaway check to confirm the value is stable, then record it:

```bash
cmake-build-asan/unittests/HermesParserNative/HermesParserNativeTests \
  --gtest_filter=KindHashTest.*
```

Expected: PASS. The generated JavaScript constant in Task 7 must match at runtime; nothing is hardcoded here.

- [ ] **Step 7: Commit**

```bash
git add tools/hermes-parser-native/KindHash.h unittests/HermesParserNative
git commit -m "Add ESTree.def kind-table hash for hermes-parser-native"
```

---

### Task 4: Container writer

Assembles the header and the four regions into one buffer with the alignment guarantee the deserializer depends on.

**Files:**
- Create: `tools/hermes-parser-native/ContainerWriter.h`
- Test: `unittests/HermesParserNative/ContainerWriterTest.cpp`
- Modify: `unittests/HermesParserNative/CMakeLists.txt:6` (add the new source)

**Interfaces:**
- Consumes: `hermes::StringTable` (Task 2), `hermes::computeKindHash` (Task 3).
- Produces: `std::vector<uint8_t> hermes::writeContainer(const std::vector<uint32_t> &program, const std::vector<PositionResult> &positions, const StringTable &strings)`.

Header layout, all `uint32_t` little-endian:

| Byte offset | Field |
| --- | --- |
| 0 | magic `0x484D5052` |
| 4 | format version `1` |
| 8 | kind hash |
| 12 | program byte offset |
| 16 | program length, in `u32` units |
| 20 | positions byte offset |
| 24 | position entry count (5 `u32` each) |
| 28 | string-offsets byte offset |
| 32 | string count `n` (the array has `n + 1` entries) |
| 36 | string-data byte offset |
| 40 | string-data length, in bytes |
| 44 | padding, zero |

Header is 48 bytes, so the program region always begins 8-byte aligned.

- [ ] **Step 1: Write the failing test**

Create `unittests/HermesParserNative/ContainerWriterTest.cpp`:

```cpp
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "ContainerWriter.h"

#include "KindHash.h"
#include "gtest/gtest.h"

using namespace hermes;

namespace {

/// Read the \p i th uint32 field of the container header.
uint32_t headerField(const std::vector<uint8_t> &buf, size_t i) {
  uint32_t v;
  memcpy(&v, buf.data() + i * 4, sizeof(v));
  return v;
}

TEST(ContainerWriterTest, WritesMagicVersionAndHash) {
  StringTable strings;
  std::vector<uint32_t> program;
  std::vector<PositionResult> positions;

  auto buf = writeContainer(program, positions, strings);

  EXPECT_EQ(0x484D5052u, headerField(buf, 0));
  EXPECT_EQ(1u, headerField(buf, 1));
  EXPECT_EQ(computeKindHash(), headerField(buf, 2));
}

TEST(ContainerWriterTest, ProgramRegionIsEightByteAligned) {
  StringTable strings;
  std::vector<uint32_t> program{1, 2, 3};
  std::vector<PositionResult> positions;

  auto buf = writeContainer(program, positions, strings);

  uint32_t programOffset = headerField(buf, 3);
  EXPECT_EQ(48u, programOffset);
  EXPECT_EQ(0u, programOffset % 8);
}

TEST(ContainerWriterTest, RoundTripsProgramAndStrings) {
  StringTable strings;
  strings.intern("foo");
  strings.intern("bar");
  std::vector<uint32_t> program{7, 8, 9};
  std::vector<PositionResult> positions;
  positions.emplace_back(3, PositionInfo::Kind::Start, 10, 20, 30);

  auto buf = writeContainer(program, positions, strings);

  uint32_t programOffset = headerField(buf, 3);
  EXPECT_EQ(3u, headerField(buf, 4));
  uint32_t first;
  memcpy(&first, buf.data() + programOffset, sizeof(first));
  EXPECT_EQ(7u, first);

  EXPECT_EQ(1u, headerField(buf, 6));

  EXPECT_EQ(2u, headerField(buf, 8));
  uint32_t strDataOffset = headerField(buf, 9);
  EXPECT_EQ(6u, headerField(buf, 10));
  std::string data((const char *)buf.data() + strDataOffset, 6);
  EXPECT_EQ("foobar", data);
}

TEST(ContainerWriterTest, StringOffsetArrayHasCountPlusOneEntries) {
  StringTable strings;
  strings.intern("alpha");
  strings.intern("be");
  std::vector<uint32_t> program;
  std::vector<PositionResult> positions;

  auto buf = writeContainer(program, positions, strings);

  uint32_t strOffsetsOffset = headerField(buf, 7);
  uint32_t count = headerField(buf, 8);
  ASSERT_EQ(2u, count);

  std::vector<uint32_t> offsets(count + 1);
  memcpy(offsets.data(), buf.data() + strOffsetsOffset,
         (count + 1) * sizeof(uint32_t));
  EXPECT_EQ(0u, offsets[0]);
  EXPECT_EQ(5u, offsets[1]);
  EXPECT_EQ(7u, offsets[2]);
}

} // namespace
```

- [ ] **Step 2: Add the source to the unittest CMake**

```cmake
set(HermesParserNativeTestSources
  StringTableTest.cpp
  KindHashTest.cpp
  ContainerWriterTest.cpp
  )
```

The test includes `ContainerWriter.h`, which includes the forked serializer
header for `PositionResult`. Add the include path so it resolves — it is
already present from Task 2, so no CMake change beyond the source list.

- [ ] **Step 3: Run the test to verify it fails**

```bash
cmake --build cmake-build-asan --target HermesParserNativeTests
```

Expected: FAIL — `ContainerWriter.h` does not exist.

- [ ] **Step 4: Write the implementation**

Create `tools/hermes-parser-native/ContainerWriter.h`:

```cpp
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#ifndef HERMES_TOOLS_HERMESPARSERNATIVE_CONTAINERWRITER_H
#define HERMES_TOOLS_HERMESPARSERNATIVE_CONTAINERWRITER_H

#include <cstring>
#include <vector>

#include "HermesParserJSSerializer.h"
#include "KindHash.h"
#include "StringTable.h"

namespace hermes {

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
    const StringTable &strings) {
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
    memcpy(buf.data() + strOffsetsOffset, strings.offsets().data(),
           strOffsetsBytes);
  }
  if (strDataBytes != 0) {
    memcpy(buf.data() + strDataOffset, strings.data().data(), strDataBytes);
  }

  return buf;
}

} // namespace hermes

#endif
```

This includes `HermesParserJSSerializer.h`, which does not exist in this
directory yet. Copy it in now as an unmodified fork so the header resolves —
Task 5 makes the behavioral changes:

```bash
cp tools/hermes-parser/HermesParserJSSerializer.h tools/hermes-parser-native/
cp tools/hermes-parser/HermesParserJSSerializer.cpp tools/hermes-parser-native/
cp tools/hermes-parser/HermesParserDiagHandler.h tools/hermes-parser-native/
cp tools/hermes-parser/HermesParserDiagHandler.cpp tools/hermes-parser-native/
```

Change the include guards in the copied headers so they do not collide with
the originals: in `tools/hermes-parser-native/HermesParserJSSerializer.h`
replace both occurrences of `HERMES_TOOLS_HERMESPARSER_HERMESPARSERJSSERIALIZER_H`
with `HERMES_TOOLS_HERMESPARSERNATIVE_HERMESPARSERJSSERIALIZER_H`, and do the
same for the diag handler header, replacing
`HERMES_TOOLS_HERMESPARSER_HERMESPARSERDIAGHANDLER_H` with
`HERMES_TOOLS_HERMESPARSERNATIVE_HERMESPARSERDIAGHANDLER_H`.

- [ ] **Step 5: Run the tests to verify they pass**

```bash
cmake --build cmake-build-asan --target HermesParserNativeTests && \
  cmake-build-asan/unittests/HermesParserNative/HermesParserNativeTests
```

Expected: all 12 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add tools/hermes-parser-native unittests/HermesParserNative
git commit -m "Add container writer and fork serializer sources"
```

---

### Task 5: Serializer changes — string table and index-based padding

The behavioral heart of the fork. Three pointer sites become string-table ids; number padding stops depending on the allocator.

**Files:**
- Modify: `tools/hermes-parser-native/HermesParserJSSerializer.h` (add the table to `ParseResult`)
- Modify: `tools/hermes-parser-native/HermesParserJSSerializer.cpp:46-64, 68-79, 183-191, 283-291`
- Test: `unittests/HermesParserNative/SerializerTest.cpp`
- Modify: `unittests/HermesParserNative/CMakeLists.txt`

**Interfaces:**
- Consumes: `hermes::StringTable` (Task 2).
- Produces: `ParseResult` gains a public `StringTable stringTable_;`. Every
  string-valued field in the program buffer is now a single `u32`: `0` means
  null, and any other value `v` means string id `v - 1`.

- [ ] **Step 1: Write the failing test**

Create `unittests/HermesParserNative/SerializerTest.cpp`:

```cpp
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "HermesParserJSSerializer.h"

#include <algorithm>
#include <cstdint>
#include <cstring>

#include "hermes/AST/Context.h"
#include "hermes/Parser/JSParser.h"
#include "gtest/gtest.h"

using namespace hermes;

namespace {

/// Parse \p source and serialize it, returning the populated result.
std::unique_ptr<ParseResult> parseAndSerialize(const char *source) {
  auto result = std::make_unique<ParseResult>();
  auto context = std::make_shared<Context>();
  auto &sm = context->getSourceErrorManager();

  auto fileBuf = llvh::MemoryBuffer::getMemBuffer(llvh::StringRef{source});
  int fileBufId = sm.addNewSourceBuffer(std::move(fileBuf));

  parser::JSParser parser(*context, fileBufId);
  auto parsed = parser.parse();
  EXPECT_TRUE(parsed.hasValue());

  serialize(
      llvh::cast<ESTree::ProgramNode>(parsed.getValue()), &sm, *result, false);

  result->context_ = context;
  return result;
}

TEST(SerializerTest, InternsRepeatedIdentifiersOnce) {
  auto result = parseAndSerialize("var foo; foo; foo; foo;");

  uint32_t fooCount = 0;
  for (uint32_t i = 0; i < result->stringTable_.count(); ++i) {
    uint32_t start = result->stringTable_.offsets()[i];
    uint32_t end = result->stringTable_.offsets()[i + 1];
    if (result->stringTable_.data().substr(start, end - start) == "foo") {
      ++fooCount;
    }
  }

  EXPECT_EQ(1u, fooCount) << "identifier must be interned exactly once";
}

TEST(SerializerTest, PadsNumbersToEvenIndex) {
  auto result = parseAndSerialize("1.5;");

  // Locate the IEEE-754 halves of 1.5 and assert the pair starts on an even
  // index, which is what lets a Float64Array view over the region address it.
  double value = 1.5;
  uint64_t bits;
  memcpy(&bits, &value, sizeof(bits));
  const uint32_t lo = (uint32_t)bits;
  const uint32_t hi = (uint32_t)(bits >> 32);

  const auto &buf = result->programBuffer_;
  bool found = false;
  for (size_t i = 0; i + 1 < buf.size(); ++i) {
    if (buf[i] == lo && buf[i + 1] == hi) {
      EXPECT_EQ(0u, i % 2) << "double must start on an even index";
      found = true;
    }
  }
  EXPECT_TRUE(found) << "1.5 must appear in the program buffer";
}

TEST(SerializerTest, StringIdsAreBiasedByOne) {
  auto result = parseAndSerialize("var foo;");

  // Find the table id assigned to "foo".
  uint32_t fooId = UINT32_MAX;
  for (uint32_t i = 0; i < result->stringTable_.count(); ++i) {
    uint32_t start = result->stringTable_.offsets()[i];
    uint32_t end = result->stringTable_.offsets()[i + 1];
    if (result->stringTable_.data().substr(start, end - start) == "foo") {
      fooId = i;
    }
  }
  ASSERT_NE(UINT32_MAX, fooId) << "\"foo\" must be interned";

  // The program buffer must reference it as id + 1, since 0 means null.
  const auto &buf = result->programBuffer_;
  EXPECT_NE(std::find(buf.begin(), buf.end(), fooId + 1), buf.end())
      << "program buffer must reference the string as id + 1";
}

} // namespace
```

- [ ] **Step 2: Add the source and link the parser libraries**

`unittests/HermesParserNative/CMakeLists.txt`:

```cmake
set(HermesParserNativeTestSources
  StringTableTest.cpp
  KindHashTest.cpp
  ContainerWriterTest.cpp
  SerializerTest.cpp
  ../../tools/hermes-parser-native/HermesParserJSSerializer.cpp
  ../../tools/hermes-parser-native/HermesParserDiagHandler.cpp
  )

add_hermes_unittest(HermesParserNativeTests
  ${HermesParserNativeTestSources}
  )

target_include_directories(HermesParserNativeTests PRIVATE
  ${HERMES_SOURCE_DIR}/tools/hermes-parser-native
  )

target_link_libraries(HermesParserNativeTests
  hermesAST
  hermesParser
  hermesSema
  LLVHSupport
  )
```

- [ ] **Step 3: Run the test to verify it fails**

```bash
cmake --build cmake-build-asan --target HermesParserNativeTests
```

Expected: FAIL to compile — `ParseResult` has no member `stringTable_`.

- [ ] **Step 4: Add the string table to ParseResult**

In `tools/hermes-parser-native/HermesParserJSSerializer.h`, add the include
near the top, after the existing includes:

```cpp
#include "StringTable.h"
```

and add this member to `class ParseResult`, immediately after
`positionBuffer_`:

```cpp
  /// Deduplicated table of every string referenced by the program buffer.
  StringTable stringTable_;
```

- [ ] **Step 5: Change number padding to index parity**

In `tools/hermes-parser-native/HermesParserJSSerializer.cpp`, replace the body
of `serializeNode(NodeNumber num)` (currently lines 46-64) with:

```cpp
  /// Numbers are serialized directly into program buffer, but must be aligned
  /// on 8-byte boundaries.
  void serializeNode(NodeNumber num) {
    // HEAPF64 requires doubles aligned on 8 byte boundaries but we are using a
    // buffer of 4 byte values, so add 4 byte padding if necessary.
    //
    // Padding is decided by the index within the buffer rather than by the
    // address of the buffer's storage. The consumer creates its Float64Array
    // view over the program region, so only the region-relative parity
    // matters, and this avoids depending on the allocator returning 8-byte
    // aligned memory.
    if (result_.programBuffer_.size() % 2) {
      result_.programBuffer_.emplace_back(0);
    }

    // Split up number into two 4-byte sections so it can be written into
    // program buffer.
    uint64_t bytes;
    memcpy(&bytes, &num, sizeof(uint64_t));

    result_.programBuffer_.emplace_back((uint32_t)bytes);
    result_.programBuffer_.emplace_back((uint32_t)(bytes >> 32));
  }
```

- [ ] **Step 6: Change the three string sites to table ids**

Replace `serializeNode(NodeLabel label)` (currently lines 68-79) with:

```cpp
  /// Strings are serialized as a single 4-byte string-table id, biased by one
  /// so that zero can represent a null string.
  void serializeNode(NodeLabel label) {
    if (label == nullptr) {
      result_.programBuffer_.emplace_back(0);
      return;
    }

    result_.programBuffer_.emplace_back(
        result_.stringTable_.intern(label->str()) + 1);
  }
```

In the comment serialization (currently lines 190-191), replace the two
`emplace_back` calls that write `value.begin()` and `value.size()` with:

```cpp
      result_.programBuffer_.emplace_back(
          result_.stringTable_.intern(value) + 1);
```

In the token serialization (currently lines 289-291), replace the two
`emplace_back` calls that write `start` and the range length with:

```cpp
    result_.programBuffer_.emplace_back(
        result_.stringTable_.intern(
            llvh::StringRef(start, (size_t)(range.End.getPointer() - start))) +
        1);
```

- [ ] **Step 7: Run the tests to verify they pass**

```bash
cmake --build cmake-build-asan --target HermesParserNativeTests && \
  cmake-build-asan/unittests/HermesParserNative/HermesParserNativeTests
```

Expected: all 15 tests PASS.

- [ ] **Step 8: Commit**

```bash
git add tools/hermes-parser-native unittests/HermesParserNative
git commit -m "Serialize strings as table ids and pad numbers by index parity"
```

---

### Task 6: Wire the addon to the real parser

Replaces the skeleton's `not implemented` with an actual parse, returning the container or an error descriptor.

**Files:**
- Modify: `tools/hermes-parser-native/hermes-parser-napi.cpp` (whole file)
- Modify: `tools/hermes-parser-native/CMakeLists.txt` (add sources and libraries)
- Test: `tools/hermes-parser-native/__tests__/container.js`

**Interfaces:**
- Consumes: `writeContainer` (Task 4), the forked serializer (Task 5),
  `HermesParserDiagHandler`.
- Produces: `parse(source: string, options: object)` returning either
  `{buffer: ArrayBuffer}` or `{error: string, line: number, column: number}`.
  Recognized option keys, all optional booleans defaulting to `false`:
  `detectFlow`, `enableExperimentalComponentSyntax`,
  `enableExperimentalFlowMatchSyntax`, `enableExperimentalFlowRecordSyntax`,
  `tokens`, `allowReturnOutsideFunction`.

- [ ] **Step 1: Write the failing test**

Create `tools/hermes-parser-native/__tests__/container.js`:

```js
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

'use strict';

const assert = require('assert');
const path = require('path');

const addon = require(path.resolve(process.argv[2]));

// --- successful parse returns a container ---
const ok = addon.parse('var x = 1;', {});
assert.ok(ok.buffer instanceof ArrayBuffer, 'expected an ArrayBuffer');
assert.strictEqual(ok.error, undefined);

const header = new Uint32Array(ok.buffer, 0, 12);
assert.strictEqual(header[0], 0x484d5052, 'magic');
assert.strictEqual(header[1], 1, 'format version');
assert.notStrictEqual(header[2], 0, 'kind hash must be set');
assert.strictEqual(header[3], 48, 'program region starts after the header');
assert.strictEqual(header[3] % 8, 0, 'program region must be 8-byte aligned');
assert.ok(header[4] > 0, 'program region must be non-empty');

// Region bounds must all lie inside the buffer.
const total = ok.buffer.byteLength;
assert.ok(header[5] + header[6] * 20 <= total, 'positions in bounds');
assert.ok(header[9] + header[10] <= total, 'string data in bounds');

// --- syntax error returns a descriptor, not a throw ---
const bad = addon.parse('var = ;', {});
assert.strictEqual(bad.buffer, undefined);
assert.strictEqual(typeof bad.error, 'string');
assert.ok(bad.error.length > 0);
assert.strictEqual(typeof bad.line, 'number');
assert.strictEqual(typeof bad.column, 'number');

// --- the string table holds the identifier exactly once ---
const withDupes = addon.parse('var foo; foo; foo; foo;', {});
const h2 = new Uint32Array(withDupes.buffer, 0, 12);
const strCount = h2[8];
const strOffsets = new Uint32Array(withDupes.buffer, h2[7], strCount + 1);
const strData = new Uint8Array(withDupes.buffer, h2[9], h2[10]);
let fooCount = 0;
for (let i = 0; i < strCount; i++) {
  const s = Buffer.from(
    strData.subarray(strOffsets[i], strOffsets[i + 1]),
  ).toString('utf8');
  if (s === 'foo') fooCount++;
}
assert.strictEqual(fooCount, 1, 'identifier must be interned once');

console.log('container OK');
```

- [ ] **Step 2: Run it to verify it fails**

```bash
node tools/hermes-parser-native/__tests__/container.js \
  cmake-build-debug/tools/hermes-parser-native/hermes-parser.node
```

Expected: FAIL — the addon throws `not implemented`.

- [ ] **Step 3: Extend the CMake target**

Replace the `add_library` call in `tools/hermes-parser-native/CMakeLists.txt`
with:

```cmake
add_library(hermes-parser-napi MODULE
  hermes-parser-napi.cpp
  HermesParserJSSerializer.cpp
  HermesParserDiagHandler.cpp
  )

target_link_libraries(hermes-parser-napi
  hermesAST
  hermesParser
  hermesSema
  LLVHSupport
  )
```

Keep the existing `set_target_properties`, `target_include_directories`, and
the `APPLE` block unchanged.

- [ ] **Step 4: Write the implementation**

Replace `tools/hermes-parser-native/hermes-parser-napi.cpp` entirely:

```cpp
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include <memory>
#include <string>
#include <vector>

#include "ContainerWriter.h"
#include "HermesParserDiagHandler.h"
#include "HermesParserJSSerializer.h"
#include "hermes/AST/Context.h"
#include "hermes/AST/ESTree.h"
#include "hermes/Parser/JSParser.h"
#include "node_api.h"

using namespace hermes;

namespace {

/// Read an optional boolean property from \p obj, defaulting to false.
bool boolOption(napi_env env, napi_value obj, const char *name) {
  napi_value prop;
  if (napi_get_named_property(env, obj, name, &prop) != napi_ok) {
    return false;
  }

  napi_valuetype type;
  if (napi_typeof(env, prop, &type) != napi_ok || type != napi_boolean) {
    return false;
  }

  bool value = false;
  if (napi_get_value_bool(env, prop, &value) != napi_ok) {
    return false;
  }
  return value;
}

/// Set \p name on \p obj to the given uint32 value.
void setUint32(napi_env env, napi_value obj, const char *name, uint32_t v) {
  napi_value num;
  if (napi_create_uint32(env, v, &num) == napi_ok) {
    napi_set_named_property(env, obj, name, num);
  }
}

/// Set \p name on \p obj to the given UTF-8 string.
void setString(
    napi_env env,
    napi_value obj,
    const char *name,
    const std::string &v) {
  napi_value str;
  if (napi_create_string_utf8(env, v.data(), v.size(), &str) == napi_ok) {
    napi_set_named_property(env, obj, name, str);
  }
}

/// Build the `{error, line, column}` descriptor. The caller in JavaScript
/// turns this into a SyntaxError; Node-API cannot construct one directly.
napi_value errorResult(
    napi_env env,
    const std::string &message,
    uint32_t line,
    uint32_t column) {
  napi_value obj;
  if (napi_create_object(env, &obj) != napi_ok) {
    return nullptr;
  }
  setString(env, obj, "error", message);
  setUint32(env, obj, "line", line);
  setUint32(env, obj, "column", column);
  return obj;
}

/// Parse a source string and return either `{buffer}` or
/// `{error, line, column}`.
napi_value parse(napi_env env, napi_callback_info info) {
  size_t argc = 2;
  napi_value argv[2];
  if (napi_get_cb_info(env, info, &argc, argv, nullptr, nullptr) != napi_ok) {
    return nullptr;
  }
  if (argc < 2) {
    napi_throw_type_error(env, nullptr, "parse(source, options) requires two arguments");
    return nullptr;
  }

  // Copy the source out as NUL-terminated UTF-8. napi_get_value_string_utf8
  // NUL-terminates for us, so the parser's zero-termination requirement is
  // satisfied without a separate guard.
  size_t sourceLen = 0;
  if (napi_get_value_string_utf8(env, argv[0], nullptr, 0, &sourceLen) !=
      napi_ok) {
    napi_throw_type_error(env, nullptr, "source must be a string");
    return nullptr;
  }
  std::vector<char> source(sourceLen + 1, '\0');
  if (napi_get_value_string_utf8(
          env, argv[0], source.data(), source.size(), &sourceLen) != napi_ok) {
    napi_throw_error(env, nullptr, "failed to read source");
    return nullptr;
  }

  const bool detectFlow = boolOption(env, argv[1], "detectFlow");
  const bool componentSyntax =
      boolOption(env, argv[1], "enableExperimentalComponentSyntax");
  const bool matchSyntax =
      boolOption(env, argv[1], "enableExperimentalFlowMatchSyntax");
  const bool recordSyntax =
      boolOption(env, argv[1], "enableExperimentalFlowRecordSyntax");
  const bool tokens = boolOption(env, argv[1], "tokens");
  const bool allowReturnOutsideFunction =
      boolOption(env, argv[1], "allowReturnOutsideFunction");

  ParseResult result;
  auto context = std::make_shared<Context>();
  auto &sm = context->getSourceErrorManager();
  const auto diagHandler = HermesParserDiagHandler(sm);

  auto fileBuf = llvh::MemoryBuffer::getMemBuffer(
      llvh::StringRef{source.data(), sourceLen});
  int fileBufId = sm.addNewSourceBuffer(std::move(fileBuf));

  context->setParseFlow(
      detectFlow ? ParseFlowSetting::UNAMBIGUOUS : ParseFlowSetting::ALL);
  context->setParseJSX(true);
  context->setParseComponentSyntax(componentSyntax);
  context->setParseFlowMatch(matchSyntax);
  context->setParseFlowRecord(recordSyntax);
  context->setAllowReturnOutsideFunction(allowReturnOutsideFunction);

  parser::JSParser parser(*context, fileBufId);
  auto parsed = parser.parse();

  if (!parsed.hasValue() || diagHandler.hasError()) {
    return errorResult(
        env,
        diagHandler.getErrorString(),
        diagHandler.getErrorLine(),
        diagHandler.getErrorColumn());
  }

  serialize(
      llvh::cast<ESTree::ProgramNode>(parsed.getValue()), &sm, result, tokens);

  auto container = writeContainer(
      result.programBuffer_, result.positionBuffer_, result.stringTable_);

  void *data = nullptr;
  napi_value arrayBuffer;
  if (napi_create_arraybuffer(env, container.size(), &data, &arrayBuffer) !=
      napi_ok) {
    napi_throw_error(env, nullptr, "failed to allocate result buffer");
    return nullptr;
  }
  memcpy(data, container.data(), container.size());

  napi_value obj;
  if (napi_create_object(env, &obj) != napi_ok) {
    return nullptr;
  }
  napi_set_named_property(env, obj, "buffer", arrayBuffer);
  return obj;
}

/// Module initializer. Registers `parse` on the exports object.
napi_value init(napi_env env, napi_value exports) {
  napi_value fn;
  if (napi_create_function(env, "parse", NAPI_AUTO_LENGTH, parse, nullptr,
                           &fn) != napi_ok) {
    return nullptr;
  }
  if (napi_set_named_property(env, exports, "parse", fn) != napi_ok) {
    return nullptr;
  }
  return exports;
}

} // namespace

NAPI_MODULE(NODE_GYP_MODULE_NAME, init)
```

Note: the exact `Context` setter names and the `HermesParserDiagHandler`
accessors must be taken from
`tools/hermes-parser/hermes-parser-wasm.cpp:32-117` — copy the option plumbing
from there verbatim rather than guessing, since that file is the working
reference for how these flags are applied.

- [ ] **Step 5: Build and run the test**

```bash
cmake --build cmake-build-debug --target hermes-parser-napi && \
node tools/hermes-parser-native/__tests__/container.js \
  cmake-build-debug/tools/hermes-parser-native/hermes-parser.node
```

Expected: `container OK`.

- [ ] **Step 6: Re-run the earlier smoke test and update it**

The smoke test asserts `parse` throws. That is no longer true. Replace its
final block with:

```js
const result = addon.parse('var x = 1;', {});
assert.ok(result.buffer instanceof ArrayBuffer, 'parse must return a buffer');
```

Run both:

```bash
node tools/hermes-parser-native/__tests__/smoke.js \
  cmake-build-debug/tools/hermes-parser-native/hermes-parser.node && \
node tools/hermes-parser-native/__tests__/container.js \
  cmake-build-debug/tools/hermes-parser-native/hermes-parser.node
```

Expected: `smoke OK` then `container OK`.

- [ ] **Step 7: Commit**

```bash
git add tools/hermes-parser-native
git commit -m "Wire hermes-parser-native addon to the parser"
```

---

### Task 7: Fork the JavaScript package

Creates the workspace, the addon loader, the generated kind-hash constant, and the two diverging source files. Ends with the first real AST.

**Files:**
- Create: `tools/hermes-parser/js/hermes-parser-native/` (copy of `hermes-parser/`)
- Create: `tools/hermes-parser/js/hermes-parser-native/src/HermesParserAddon.js`
- Modify: `tools/hermes-parser/js/hermes-parser-native/src/HermesParser.js`
- Modify: `tools/hermes-parser/js/hermes-parser-native/src/HermesParserDeserializer.js`
- Create: `tools/hermes-parser/js/scripts/genKindHash.js`
- Modify: `tools/hermes-parser/js/package.json:41-48` (one workspace entry)
- Test: `tools/hermes-parser/js/hermes-parser-native/__tests__/Native-test.js`

**Interfaces:**
- Consumes: the addon's `parse` (Task 6), the container layout (Task 4).
- Produces: `parse(source: string, options: ParserOptions): HermesNode`, the
  same signature the original exports.

- [ ] **Step 1: Copy the package and register the workspace**

```bash
cp -r tools/hermes-parser/js/hermes-parser tools/hermes-parser/js/hermes-parser-native
rm -rf tools/hermes-parser/js/hermes-parser-native/dist
rm -f tools/hermes-parser/js/hermes-parser-native/src/HermesParserWASM.js.flow
```

In `tools/hermes-parser/js/hermes-parser-native/package.json` set
`"name": "hermes-parser-native"` and leave `"version": "0.37.0"` and the
`hermes-estree` dependency as they are.

In `tools/hermes-parser/js/package.json`, add to the `workspaces` array:

```json
    "hermes-parser-native",
```

- [ ] **Step 2: Write the failing test**

Create `tools/hermes-parser/js/hermes-parser-native/__tests__/Native-test.js`:

```js
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

'use strict';

const {parse} = require('../src/HermesParser');

describe('hermes-parser-native', () => {
  test('parses a variable declaration', () => {
    const ast = parse('var x = 1;', {});
    expect(ast.type).toBe('Program');
    expect(ast.body).toHaveLength(1);
    expect(ast.body[0].type).toBe('VariableDeclaration');
    expect(ast.body[0].declarations[0].id.name).toBe('x');
    expect(ast.body[0].declarations[0].init.value).toBe(1);
  });

  test('interns repeated identifiers to one string object', () => {
    const ast = parse('foo; foo;', {});
    const first = ast.body[0].expression.name;
    const second = ast.body[1].expression.name;
    expect(first).toBe('foo');
    expect(second).toBe('foo');
    // The interning property: the same JS string object, not just equal.
    expect(Object.is(first, second)).toBe(true);
  });

  test('throws a SyntaxError with loc', () => {
    let err = null;
    try {
      parse('var = ;', {});
    } catch (e) {
      err = e;
    }
    expect(err).toBeInstanceOf(SyntaxError);
    expect(typeof err.loc.line).toBe('number');
    expect(typeof err.loc.column).toBe('number');
  });
});
```

- [ ] **Step 3: Run it to verify it fails**

```bash
(cd tools/hermes-parser/js; yarn jest hermes-parser-native)
```

Expected: FAIL — `HermesParserAddon` not found.

- [ ] **Step 4: Write the addon loader**

Create `tools/hermes-parser/js/hermes-parser-native/src/HermesParserAddon.js`:

```js
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @flow strict-local
 * @format
 */

'use strict';

const path = require('path');

const SUPPORTED = [
  'linux-x64',
  'linux-arm64',
  'darwin-x64',
  'darwin-arm64',
];

/**
 * Locate and load the prebuilt addon for the running platform.
 *
 * The path can be overridden with HERMES_PARSER_NATIVE_ADDON, which the
 * in-tree test setup uses to point at a freshly built binary.
 */
function loadAddon() {
  const override = process.env.HERMES_PARSER_NATIVE_ADDON;
  if (override != null && override !== '') {
    /* $FlowFixMe[unsupported-syntax] dynamic require by design */
    return require(path.resolve(override));
  }

  const target = `${process.platform}-${process.arch}`;
  if (!SUPPORTED.includes(target)) {
    throw new Error(
      `hermes-parser-native: no prebuilt addon for ${target}. ` +
        `Supported platforms: ${SUPPORTED.join(', ')}.`,
    );
  }

  const addonPath = path.join(
    __dirname,
    '..',
    'prebuilds',
    target,
    'hermes-parser.node',
  );

  try {
    /* $FlowFixMe[unsupported-syntax] dynamic require by design */
    return require(addonPath);
  } catch (e) {
    throw new Error(
      `hermes-parser-native: failed to load the prebuilt addon for ${target} ` +
        `at ${addonPath}: ${e.message}`,
    );
  }
}

module.exports = loadAddon;
```

- [ ] **Step 5: Write the kind-hash generator**

Create `tools/hermes-parser/js/scripts/genKindHash.js`:

```js
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

'use strict';

const fs = require('fs');
const path = require('path');

const OUTPUT_FILE = path.resolve(
  __dirname,
  '../hermes-parser-native/src/HermesParserKindHash.js',
);

/**
 * FNV-1a over each name followed by a newline. Must stay identical to
 * computeKindHash() in tools/hermes-parser-native/KindHash.h.
 */
function fnv1a(names) {
  let hash = 0x811c9dc5;
  const feed = byte => {
    hash ^= byte;
    hash = Math.imul(hash, 16777619) >>> 0;
  };
  for (const name of names) {
    for (let i = 0; i < name.length; i++) {
      feed(name.charCodeAt(i) & 0xff);
    }
    feed(0x0a);
  }
  return hash >>> 0;
}

/**
 * Extract the ordered node-kind names from ESTree.def. ESTREE_FIRST(X) and
 * ESTREE_LAST(X) contribute "XFirst" and "XLast"; every other macro
 * contributes the bare name. This ordering matches NODE_DESERIALIZERS.
 */
function extractNames(defPath) {
  const text = fs.readFileSync(defPath, 'utf8').replace(/\n/g, ' ');
  const re = /ESTREE_(NODE_\d+_ARGS|FIRST|LAST)\(\s*([A-Za-z0-9_]+)/g;
  const names = [];
  let m;
  while ((m = re.exec(text)) !== null) {
    // Skip the macro definitions at the top of the file, which use the
    // literal parameter name NAME rather than a real node name.
    if (m[2] === 'NAME') {
      continue;
    }
    if (m[1] === 'FIRST') {
      names.push(m[2] + 'First');
    } else if (m[1] === 'LAST') {
      names.push(m[2] + 'Last');
    } else {
      names.push(m[2]);
    }
  }
  return names;
}

const includePath = process.argv[2];
if (includePath == null) {
  console.error('usage: genKindHash.js <hermes-include-path>');
  process.exit(1);
}

const names = extractNames(path.join(includePath, 'hermes/AST/ESTree.def'));
const hash = fnv1a(names);

fs.writeFileSync(
  OUTPUT_FILE,
  `/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @flow strict
 * @format
 * @generated
 */

'use strict';

// Hash of the ${names.length} node-kind names in ESTree.def, in order.
// Must match computeKindHash() in tools/hermes-parser-native/KindHash.h.
export default ${hash};
`,
);

console.log(`genKindHash: ${names.length} kinds, hash ${hash}`);
```

Run it:

```bash
node tools/hermes-parser/js/scripts/genKindHash.js \
  /home/tmikov/work/hermes-parser-native/include
```

Expected: reports 295 kinds and writes the constant.

- [ ] **Step 6: Rewrite HermesParser.js**

Replace `tools/hermes-parser/js/hermes-parser-native/src/HermesParser.js`
entirely:

```js
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @flow strict-local
 * @format
 */

'use strict';

import type {HermesNode} from './HermesAST';
import type {ParserOptions} from './ParserOptions';

import HermesParserDeserializer from './HermesParserDeserializer';
import EXPECTED_KIND_HASH from './HermesParserKindHash';

const loadAddon = require('./HermesParserAddon');

const CONTAINER_MAGIC = 0x484d5052;
const CONTAINER_VERSION = 1;

let addon = null;

function getAddon() {
  if (addon == null) {
    addon = loadAddon();
  }
  return addon;
}

export function parse(source: string, options: ParserOptions): HermesNode {
  const result = getAddon().parse(source, {
    detectFlow: options.flow === 'detect',
    enableExperimentalComponentSyntax:
      options.enableExperimentalComponentSyntax === true,
    enableExperimentalFlowMatchSyntax:
      options.enableExperimentalFlowMatchSyntax === true,
    enableExperimentalFlowRecordSyntax:
      options.enableExperimentalFlowRecordSyntax === true,
    tokens: options.tokens === true,
    allowReturnOutsideFunction: options.allowReturnOutsideFunction === true,
  });

  if (result.error != null) {
    // Node-API cannot construct a SyntaxError, so the addon returns a
    // descriptor and we build the error here. This keeps the thrown value
    // identical in shape to what the WASM parser throws.
    const syntaxError = new SyntaxError(result.error);
    // $FlowExpectedError[prop-missing]
    syntaxError.loc = {line: result.line, column: result.column};
    throw syntaxError;
  }

  const header = new Uint32Array(result.buffer, 0, 12);

  if (header[0] !== CONTAINER_MAGIC || header[1] !== CONTAINER_VERSION) {
    throw new Error(
      'hermes-parser-native: unrecognized parse container ' +
        `(magic ${header[0]}, version ${header[1]})`,
    );
  }

  if (header[2] !== EXPECTED_KIND_HASH) {
    throw new Error(
      'hermes-parser-native: node-kind table mismatch. The native addon ' +
        `reports hash ${header[2]} but this JavaScript package was ` +
        `generated for ${EXPECTED_KIND_HASH}. The addon and the JavaScript ` +
        'package were built from different versions of ESTree.def.',
    );
  }

  const deserializer = new HermesParserDeserializer(
    result.buffer,
    header,
    options,
  );
  return deserializer.deserialize();
}
```

- [ ] **Step 7: Rewrite the deserializer's entry points**

In `tools/hermes-parser/js/hermes-parser-native/src/HermesParserDeserializer.js`,
replace the constructor and the `next`, `deserializeString`, and
`deserializeNumber`-adjacent plumbing. Replace the constructor with:

```js
  constructor(buffer, header, options) {
    const programOffset = header[3];
    const programLength = header[4];
    const positionOffset = header[5];
    const positionCount = header[6];
    const strOffsetsOffset = header[7];
    const stringCount = header[8];
    const strDataOffset = header[9];
    const strDataLength = header[10];

    this.programBuffer = new Uint32Array(
      buffer,
      programOffset,
      programLength,
    );
    this.programFloats = new Float64Array(
      buffer,
      programOffset,
      programLength >> 1,
    );
    this.positionBuffer = new Uint32Array(
      buffer,
      positionOffset,
      positionCount * 5,
    );
    this.stringOffsets = new Uint32Array(
      buffer,
      strOffsetsOffset,
      stringCount + 1,
    );
    this.stringData = new Uint8Array(buffer, strDataOffset, strDataLength);
    this.stringCache = new Array(stringCount);

    // Indices are region-relative, so both start at zero rather than at a
    // pointer divided by four.
    this.programBufferIdx = 0;
    this.positionBufferIdx = 0;
    this.positionBufferSize = positionCount * 5;

    this.locMap = {};
    this.options = options;
    this.commentTypes = ['CommentLine', 'CommentBlock', 'InterpreterDirective'];
    this.tokenTypes = [
      'Boolean',
      'Identifier',
      'Keyword',
      'Null',
      'Numeric',
      'BigInt',
      'Punctuator',
      'String',
      'RegularExpression',
      'Template',
      'JSXText',
    ];
  }

  /**
   * Decode string `id` from the table, caching the result so each unique
   * string is decoded exactly once and every reference shares one JS string.
   */
  getString(id) {
    const cached = this.stringCache[id];
    if (cached !== undefined) {
      return cached;
    }
    const start = this.stringOffsets[id];
    const end = this.stringOffsets[id + 1];
    const str = HermesParserDecodeUTF8String(
      start,
      end - start,
      this.stringData,
    );
    this.stringCache[id] = str;
    return str;
  }
```

Replace `next()` to read from the region view:

```js
  next() {
    return this.programBuffer[this.programBufferIdx++];
  }
```

Replace `deserializeString()`:

```js
  /**
   * Strings are serialized as a single string-table id biased by one, so that
   * zero represents a null string.
   */
  deserializeString(): ?string {
    const id = this.next();
    if (id === 0) {
      return null;
    }
    return this.getString(id - 1);
  }
```

In `deserializeNumber`, replace the two `this.HEAPF64[floatIdx]` reads with
`this.programFloats[floatIdx]`. The parity arithmetic on lines 120-126 stays
exactly as it is — region-relative views preserve it.

Replace the remaining `this.HEAPU32[this.positionBufferIdx++]` reads in the
position loop with `this.positionBuffer[this.positionBufferIdx++]`.

- [ ] **Step 8: Run the tests to verify they pass**

```bash
ADDON=$PWD/cmake-build-debug/tools/hermes-parser-native/hermes-parser.node
(cd tools/hermes-parser/js; HERMES_PARSER_NATIVE_ADDON=$ADDON yarn jest hermes-parser-native)
```

Expected: all 3 tests PASS.

- [ ] **Step 9: Commit**

```bash
git add tools/hermes-parser/js
git commit -m "Fork hermes-parser JS package onto the native addon"
```

---

### Task 8: Differential testing against the wasm parser

The real correctness gate. Everything before this proves the pieces work; this proves the ASTs match.

**Files:**
- Create: `tools/hermes-parser/js/hermes-parser-native/__tests__/Differential-test.js`
- Create: `tools/hermes-parser/js/hermes-parser-native/__tests__/corpus/`

**Interfaces:**
- Consumes: `parse` from both packages.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Write the differential test**

Create
`tools/hermes-parser/js/hermes-parser-native/__tests__/Differential-test.js`:

```js
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

'use strict';

const {parse: parseNative} = require('../src/HermesParser');
const {parse: parseWasm} = require('hermes-parser/dist/HermesParser');

const CASES = [
  'var x = 1;',
  'function f(a, b) { return a + b; }',
  'class C extends D { #p = 1; static m() {} }',
  'const {a, b: [c], ...rest} = obj;',
  'async function* g() { yield* await x; }',
  'type T = {a: number, b?: string};',
  'function f(x: number): string { return String(x); }',
  'const x: Array<?string> = [];',
  'interface I { m(): void }',
  'enum E { A, B }',
  '<div a="1" {...p}>{x}</div>;',
  'a?.b?.[c]?.();',
  'x ??= 1; y ||= 2; z &&= 3;',
  '`a${b}c${d}e`;',
  'label: for (const x of xs) { continue label; }',
  'try { f(); } catch { g(); } finally { h(); }',
  '/regex/gimsuy.test(s);',
  'const big = 1234567890123456789012345678901234567890n;',
  'const nums = [1, 1.5, .5, 1e10, 0x10, 0b11, 0o17, NaN, Infinity];',
  'const uni = {"\\u00e9": "caf\\u00e9", "emoji": "\\ud83d\\ude00"};',
  'export default function () {}',
  'export * as ns from "mod"; import x, {y as z} from "mod";',
  '"use strict"; with2 = 1;',
  'new.target; import.meta;',
  'const s = "line1\\nline2\\ttab\\\\back";',
];

describe('native parser matches wasm parser', () => {
  test.each(CASES)('%s', source => {
    const native = parseNative(source, {});
    const wasm = parseWasm(source, {});
    expect(native).toEqual(wasm);
  });

  test('matches with tokens enabled', () => {
    const source = 'const x = f(1, "two");';
    expect(parseNative(source, {tokens: true})).toEqual(
      parseWasm(source, {tokens: true}),
    );
  });

  test('matches with comments present', () => {
    const source = '// leading\nconst x = 1; /* trailing */';
    expect(parseNative(source, {})).toEqual(parseWasm(source, {}));
  });

  test('matches on syntax errors', () => {
    const source = 'var = ;';
    let nativeErr = null;
    let wasmErr = null;
    try {
      parseNative(source, {});
    } catch (e) {
      nativeErr = e;
    }
    try {
      parseWasm(source, {});
    } catch (e) {
      wasmErr = e;
    }
    expect(nativeErr).not.toBeNull();
    expect(wasmErr).not.toBeNull();
    expect(nativeErr.message).toBe(wasmErr.message);
    expect(nativeErr.loc).toEqual(wasmErr.loc);
    expect(nativeErr).toBeInstanceOf(SyntaxError);
  });
});
```

- [ ] **Step 2: Run it and expect failures to investigate**

```bash
ADDON=$PWD/cmake-build-debug/tools/hermes-parser-native/hermes-parser.node
(cd tools/hermes-parser/js; HERMES_PARSER_NATIVE_ADDON=$ADDON yarn jest Differential)
```

Expected initially: some cases may FAIL. Each failure is a real divergence
with a known-good answer on the right-hand side. Fix the native side until all
pass. Do not adjust the expectations.

- [ ] **Step 3: Add a bulk corpus test**

Append to the same file:

```js
describe('bulk corpus', () => {
  const fs = require('fs');
  const path = require('path');

  const roots = [path.resolve(__dirname, '../src')];

  const files = [];
  for (const root of roots) {
    const walk = dir => {
      for (const entry of fs.readdirSync(dir, {withFileTypes: true})) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) {
          walk(full);
        } else if (entry.name.endsWith('.js')) {
          files.push(full);
        }
      }
    };
    walk(root);
  }

  test('parses every source file identically', () => {
    expect(files.length).toBeGreaterThan(10);
    for (const file of files) {
      const source = fs.readFileSync(file, 'utf8');
      let native;
      let wasm;
      try {
        wasm = parseWasm(source, {});
      } catch (e) {
        continue; // Skip anything the reference cannot parse.
      }
      native = parseNative(source, {});
      expect({file, ast: native}).toEqual({file, ast: wasm});
    }
  });
});
```

- [ ] **Step 4: Run the full differential suite**

```bash
ADDON=$PWD/cmake-build-debug/tools/hermes-parser-native/hermes-parser.node
(cd tools/hermes-parser/js; HERMES_PARSER_NATIVE_ADDON=$ADDON yarn jest Differential)
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add tools/hermes-parser/js/hermes-parser-native/__tests__
git commit -m "Add differential tests against the wasm parser"
```

---

### Task 9: Port the existing test suite

The original package's 54 test files are the accumulated behavioral record. They should pass unmodified.

**Files:**
- Modify: `tools/hermes-parser/js/hermes-parser-native/__tests__/` (copied in Task 7)
- Modify: `tools/hermes-parser/js/jest.config.js` if the new workspace needs registering

**Interfaces:**
- Consumes: the forked package.
- Produces: nothing.

- [ ] **Step 1: Confirm the copied tests are present**

```bash
ls tools/hermes-parser/js/hermes-parser-native/__tests__/ | wc -l
```

Expected: 54 or more, since Task 7 copied the directory wholesale and Tasks 7-8
added files.

- [ ] **Step 2: Run them**

```bash
ADDON=$PWD/cmake-build-debug/tools/hermes-parser-native/hermes-parser.node
(cd tools/hermes-parser/js; HERMES_PARSER_NATIVE_ADDON=$ADDON yarn jest hermes-parser-native)
```

Expected: failures only where a test imports `hermes-parser` by name rather
than by relative path.

- [ ] **Step 3: Repoint package-name imports**

For each failing test, change imports of the form
`require('hermes-parser')` to `require('../src')`. Do not change assertions.
Any assertion failure is a real divergence and must be fixed in the native
implementation, not in the test.

- [ ] **Step 4: Run again to verify they pass**

```bash
ADDON=$PWD/cmake-build-debug/tools/hermes-parser-native/hermes-parser.node
(cd tools/hermes-parser/js; HERMES_PARSER_NATIVE_ADDON=$ADDON yarn jest hermes-parser-native)
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add tools/hermes-parser/js/hermes-parser-native
git commit -m "Port the hermes-parser test suite to the native package"
```

---

### Task 10: Failure modes

Covers the two new error paths and establishes actual native stack behavior on deeply nested input.

**Files:**
- Create: `tools/hermes-parser/js/hermes-parser-native/__tests__/Failures-test.js`

**Interfaces:**
- Consumes: `HermesParserAddon`, `parse`.
- Produces: nothing.

- [ ] **Step 1: Write the test**

Create `tools/hermes-parser/js/hermes-parser-native/__tests__/Failures-test.js`:

```js
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

'use strict';

const {parse} = require('../src/HermesParser');

describe('failure modes', () => {
  test('unsupported platform names the platform and the supported set', () => {
    jest.resetModules();
    const originalPlatform = process.platform;
    const originalOverride = process.env.HERMES_PARSER_NATIVE_ADDON;
    delete process.env.HERMES_PARSER_NATIVE_ADDON;
    Object.defineProperty(process, 'platform', {value: 'sunos'});

    try {
      const loadAddon = require('../src/HermesParserAddon');
      expect(() => loadAddon()).toThrow(/no prebuilt addon for sunos/);
      expect(() => loadAddon()).toThrow(/Supported platforms/);
    } finally {
      Object.defineProperty(process, 'platform', {value: originalPlatform});
      if (originalOverride != null) {
        process.env.HERMES_PARSER_NATIVE_ADDON = originalOverride;
      }
      jest.resetModules();
    }
  });

  test('deeply nested input fails cleanly rather than crashing', () => {
    const depth = 5000;
    const source = '('.repeat(depth) + '1' + ')'.repeat(depth);

    let threw = null;
    try {
      parse(source, {});
    } catch (e) {
      threw = e;
    }

    // Either it parses or it reports an error. What it must not do is abort
    // the process, which would fail the test run outright.
    if (threw != null) {
      expect(threw).toBeInstanceOf(Error);
    }
  });

  test('syntax error carries line and column', () => {
    let err = null;
    try {
      parse('function f( {\n', {});
    } catch (e) {
      err = e;
    }
    expect(err).toBeInstanceOf(SyntaxError);
    expect(err.loc.line).toBeGreaterThan(0);
    expect(err.loc.column).toBeGreaterThanOrEqual(0);
  });
});
```

- [ ] **Step 2: Run it**

```bash
ADDON=$PWD/cmake-build-debug/tools/hermes-parser-native/hermes-parser.node
(cd tools/hermes-parser/js; HERMES_PARSER_NATIVE_ADDON=$ADDON yarn jest Failures)
```

Expected: all PASS. If the deep-nesting case aborts the process rather than
throwing, record the depth at which it happens and add a recursion guard to
the addon before proceeding.

- [ ] **Step 3: Add a kind-hash mismatch test**

Append to the same file:

```js
describe('kind hash guard', () => {
  test('rejects a container whose hash does not match', () => {
    const addon = require('../src/HermesParserAddon')();
    const result = addon.parse('var x = 1;', {});
    const header = new Uint32Array(result.buffer, 0, 12);

    // Corrupt the hash and re-run the checking path by calling parse with a
    // stubbed addon that returns the mutated container.
    header[2] = header[2] ^ 0xffffffff;

    jest.resetModules();
    jest.doMock('../src/HermesParserAddon', () => () => ({
      parse: () => result,
    }));
    const {parse: parseWithStub} = require('../src/HermesParser');

    expect(() => parseWithStub('var x = 1;', {})).toThrow(
      /node-kind table mismatch/,
    );

    jest.dontMock('../src/HermesParserAddon');
    jest.resetModules();
  });
});
```

- [ ] **Step 4: Run again**

```bash
ADDON=$PWD/cmake-build-debug/tools/hermes-parser-native/hermes-parser.node
(cd tools/hermes-parser/js; HERMES_PARSER_NATIVE_ADDON=$ADDON yarn jest Failures)
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add tools/hermes-parser/js/hermes-parser-native/__tests__/Failures-test.js
git commit -m "Add failure-mode tests for hermes-parser-native"
```

---

### Task 11: Packaging and the build script

Produces something installable: prebuilds in place, dist assembled, and the consumer alias verified end to end.

**Files:**
- Create: `tools/hermes-parser/js/scripts/build-native.sh`
- Modify: `tools/hermes-parser/js/hermes-parser-native/package.json`
- Test: `tools/hermes-parser/js/hermes-parser-native/__tests__/Packaging-test.js`

**Interfaces:**
- Consumes: everything above.
- Produces: a `dist/` with `prebuilds/<platform>-<arch>/hermes-parser.node`.

- [ ] **Step 1: Write the packaging test**

Create
`tools/hermes-parser/js/hermes-parser-native/__tests__/Packaging-test.js`:

```js
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

'use strict';

const fs = require('fs');
const path = require('path');

const pkg = require('../package.json');

describe('packaging', () => {
  test('package metadata is correct', () => {
    expect(pkg.name).toBe('hermes-parser-native');
    expect(pkg.version).toBe('0.37.0');
    expect(Object.keys(pkg.dependencies)).toEqual(['hermes-estree']);
    expect(pkg.files).toContain('dist');
    expect(pkg.files).toContain('prebuilds');
  });

  test('no wasm blob is shipped', () => {
    const src = path.resolve(__dirname, '../src');
    const names = fs.readdirSync(src);
    expect(names).not.toContain('HermesParserWASM.js');
    expect(names).not.toContain('HermesParserWASM.js.flow');
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

```bash
(cd tools/hermes-parser/js; yarn jest Packaging)
```

Expected: FAIL — `files` does not list `prebuilds`.

- [ ] **Step 3: Update package.json**

In `tools/hermes-parser/js/hermes-parser-native/package.json`, set:

```json
  "name": "hermes-parser-native",
  "version": "0.37.0",
  "description": "A JavaScript parser built from the Hermes engine, as a native addon",
  "main": "dist/index.js",
  "license": "MIT",
  "dependencies": {
    "hermes-estree": "0.37.0"
  },
  "files": [
    "dist",
    "prebuilds",
    "LICENSE",
    "README.md"
  ]
```

- [ ] **Step 4: Run to verify it passes**

```bash
(cd tools/hermes-parser/js; yarn jest Packaging)
```

Expected: PASS.

- [ ] **Step 5: Write the build script**

Create `tools/hermes-parser/js/scripts/build-native.sh`:

```bash
#!/bin/bash
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

set -xe -o pipefail

THIS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGE_DIR="$THIS_DIR/../hermes-parser-native"

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

yarn install

# Generate sources shared with the original package.
yarn babel-node "$THIS_DIR/genESTreeJSON.js" "$INCLUDE_PATH"
yarn babel-node "$THIS_DIR/genNodeDeserializers.js" "$INCLUDE_PATH"
yarn babel-node "$THIS_DIR/genParserVisitorKeys.js"
yarn babel-node "$THIS_DIR/genESTreeVisitorKeys.js"
yarn babel-node "$THIS_DIR/genPredicateFunctions.js"
yarn babel-node "$THIS_DIR/genTransformNodeTypes.js"

# Generate the kind hash that guards against ESTree.def drift.
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

# Copy prebuilt addons into the package.
rm -rf "$PACKAGE_DIR/prebuilds"
mkdir -p "$PACKAGE_DIR/prebuilds"
for target in linux-x64 linux-arm64 darwin-x64 darwin-arm64; do
  if [[ -f "$PREBUILDS_SRC/$target/hermes-parser.node" ]]; then
    mkdir -p "$PACKAGE_DIR/prebuilds/$target"
    cp "$PREBUILDS_SRC/$target/hermes-parser.node" \
      "$PACKAGE_DIR/prebuilds/$target/"
  else
    echo "WARNING: missing prebuild for $target" 1>&2
  fi
done
```

Make it executable:

```bash
chmod +x tools/hermes-parser/js/scripts/build-native.sh
```

- [ ] **Step 6: Verify the local prebuild path works**

Stage the locally built addon as if it were a prebuild and confirm the loader
finds it without the environment override:

```bash
mkdir -p /tmp/prebuilds/linux-x64
cp cmake-build-debug/tools/hermes-parser-native/hermes-parser.node \
  /tmp/prebuilds/linux-x64/
tools/hermes-parser/js/scripts/build-native.sh \
  "$PWD/include" /tmp/prebuilds
(cd tools/hermes-parser/js; yarn jest hermes-parser-native)
```

Expected: all tests PASS with no `HERMES_PARSER_NATIVE_ADDON` set, because the
loader resolves `prebuilds/linux-x64/hermes-parser.node`.

- [ ] **Step 7: Verify the consumer alias end to end**

```bash
mkdir -p /tmp/aliascheck
cat > /tmp/aliascheck/package.json <<'EOF'
{
  "name": "aliascheck",
  "version": "1.0.0",
  "private": true,
  "dependencies": {"hermes-transform": "0.37.0"},
  "resolutions": {"hermes-parser": "file:REPLACE_ME"}
}
EOF
```

Replace `REPLACE_ME` with the absolute path to
`tools/hermes-parser/js/hermes-parser-native`, run `yarn install`, then:

```bash
node -e "console.log(require('hermes-parser/package.json').name)"
```

Expected: `hermes-parser-native`, confirming the transitive redirect works.

- [ ] **Step 8: Commit**

```bash
git add tools/hermes-parser/js
git commit -m "Add packaging and build script for hermes-parser-native"
```

---

### Task 12: Benchmark

Holds the performance claim honest and tells you whether the deferred optimizations are worth doing.

**Files:**
- Create: `tools/hermes-parser/js/hermes-parser-native/__benchmarks__/parse-bench.js`

**Interfaces:**
- Consumes: both parsers.
- Produces: nothing.

- [ ] **Step 1: Write the benchmark**

Create
`tools/hermes-parser/js/hermes-parser-native/__benchmarks__/parse-bench.js`:

```js
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

'use strict';

const fs = require('fs');
const path = require('path');

const {parse: parseNative} = require('../src/HermesParser');
const {parse: parseWasm} = require('hermes-parser/dist/HermesParser');

const files = [];
const walk = dir => {
  for (const entry of fs.readdirSync(dir, {withFileTypes: true})) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      walk(full);
    } else if (entry.name.endsWith('.js')) {
      files.push(fs.readFileSync(full, 'utf8'));
    }
  }
};
walk(path.resolve(__dirname, '../src'));

const totalBytes = files.reduce((n, s) => n + Buffer.byteLength(s), 0);

function time(label, parse) {
  // Warm up.
  for (const source of files) {
    try {
      parse(source, {});
    } catch (e) {}
  }

  const start = process.hrtime.bigint();
  let iterations = 0;
  for (let i = 0; i < 5; i++) {
    for (const source of files) {
      try {
        parse(source, {});
        iterations++;
      } catch (e) {}
    }
  }
  const elapsedMs = Number(process.hrtime.bigint() - start) / 1e6;
  const mbps = (totalBytes * 5) / 1024 / 1024 / (elapsedMs / 1000);
  console.log(
    `${label.padEnd(8)} ${elapsedMs.toFixed(1)} ms  ` +
      `${mbps.toFixed(1)} MB/s  (${iterations} parses)`,
  );
  return elapsedMs;
}

console.log(`corpus: ${files.length} files, ${totalBytes} bytes`);
const wasmMs = time('wasm', parseWasm);
const nativeMs = time('native', parseNative);
console.log(`native is ${(wasmMs / nativeMs).toFixed(2)}x the wasm throughput`);
```

- [ ] **Step 2: Run it**

```bash
HERMES_PARSER_NATIVE_ADDON=$PWD/cmake-build-debug/tools/hermes-parser-native/hermes-parser.node \
  node tools/hermes-parser/js/hermes-parser-native/__benchmarks__/parse-bench.js
```

Expected: prints both throughputs and the ratio. Record the number in the
commit message. Note that an ASan build understates native performance
substantially; re-run against a Release build for a meaningful figure.

- [ ] **Step 3: Re-run against a Release build**

```bash
cmake -B cmake-build-release -G Ninja -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++
cmake --build cmake-build-release --target hermes-parser-napi
HERMES_PARSER_NATIVE_ADDON=$PWD/cmake-build-release/tools/hermes-parser-native/hermes-parser.node \
  node tools/hermes-parser/js/hermes-parser-native/__benchmarks__/parse-bench.js
```

- [ ] **Step 4: Commit**

```bash
git add tools/hermes-parser/js/hermes-parser-native/__benchmarks__
git commit -m "Add parse benchmark comparing native and wasm parsers"
```

---

## Self-Review

**Spec coverage.** Every spec section maps to a task: layout (Tasks 1, 7),
wire format (Tasks 2-5), alignment rule (Task 4 Step 1, Task 5 Step 5), the
three pointer sites (Task 5 Step 6), string interning (Tasks 2, 7 Step 7, and
asserted in Task 7 Step 2), version guard (Tasks 3, 7 Step 5, 10 Step 3),
native binding and lifetime (Task 6), build and packaging (Tasks 1, 11),
consumer alias (Task 11 Step 7), error handling (Tasks 6, 10), and all six
testing items (Tasks 8, 9, 10, 11, 12).

**Known gaps, deliberately left open.** Two details cannot be pinned down
without the compiler in front of you, and both are flagged inline rather than
guessed at:

1. Task 6 Step 4 uses `Context` setters and `HermesParserDiagHandler`
   accessors whose exact names must be copied from
   `tools/hermes-parser/hermes-parser-wasm.cpp:32-117`. The plan says so
   explicitly instead of inventing signatures.
2. Task 5's `SerializerTest.cpp` calls `serialize()` directly; if the parser
   setup there does not compile as written, the container test in Task 6 covers
   the same behavior through the addon and is the authoritative check.

**Type consistency.** `StringTable::intern/data/offsets/count` are used
consistently in Tasks 2, 4, 5. `writeContainer(program, positions, strings)`
has the same signature in Tasks 4 and 6. `computeKindHash()` matches between
Task 3 and its JavaScript twin in Task 7. The header field indices used in
Task 6's JavaScript test, Task 7's constructor, and Task 10's guard test all
agree with the table in Task 4.

---

## Execution Handoff

Two execution options:

**1. Subagent-Driven (recommended)** — a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — execute tasks in this session with checkpoints for review.
