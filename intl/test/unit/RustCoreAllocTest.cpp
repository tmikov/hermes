/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// takeString must free the Rust buffer even when copying it throws
// std::bad_alloc. A leak is reported by LeakSanitizer at exit, so this
// test fails (in an ASan build) if the buffer is not freed.

#include "RustCore.h"

#include <gtest/gtest.h>

#include <cstddef>
#include <cstdlib>
#include <limits>
#include <new>

namespace {

/// When true, the next allocation through operator new throws; the flag is
/// cleared as it fires, so only one allocation is affected.
bool failNextAllocation = false;

} // namespace

/// Allocate \p size bytes at \p align, throwing std::bad_alloc if the
/// injected failure is armed or the allocation fails. Every operator new and
/// delete below goes through this and std::free, so that no allocation made
/// here is released by a different allocator.
static void *allocate(std::size_t size, std::size_t align) {
  if (failNextAllocation) {
    failNextAllocation = false;
    throw std::bad_alloc();
  }
  if (!size)
    size = 1;
  void *p;
  if (align <= alignof(std::max_align_t)) {
    p = std::malloc(size);
  } else {
    // Rounding up below would wrap for sizes near SIZE_MAX.
    if (size > std::numeric_limits<std::size_t>::max() - (align - 1))
      throw std::bad_alloc();
    // aligned_alloc wants a multiple of the alignment.
    p = std::aligned_alloc(align, (size + align - 1) / align * align);
  }
  if (!p)
    throw std::bad_alloc();
  return p;
}

/// The nothrow form of allocate.
static void *allocateNoThrow(std::size_t size, std::size_t align) noexcept {
  try {
    return allocate(size, align);
  } catch (const std::bad_alloc &) {
    return nullptr;
  }
}

// Test-only replacements of every global operator new and delete, so that
// one chosen allocation can fail. They are inert unless failNextAllocation
// is set.
void *operator new(std::size_t n) {
  return allocate(n, 1);
}
void *operator new[](std::size_t n) {
  return allocate(n, 1);
}
void *operator new(std::size_t n, const std::nothrow_t &) noexcept {
  return allocateNoThrow(n, 1);
}
void *operator new[](std::size_t n, const std::nothrow_t &) noexcept {
  return allocateNoThrow(n, 1);
}
void *operator new(std::size_t n, std::align_val_t a) {
  return allocate(n, static_cast<std::size_t>(a));
}
void *operator new[](std::size_t n, std::align_val_t a) {
  return allocate(n, static_cast<std::size_t>(a));
}
void *operator new(
    std::size_t n,
    std::align_val_t a,
    const std::nothrow_t &) noexcept {
  return allocateNoThrow(n, static_cast<std::size_t>(a));
}
void *operator new[](
    std::size_t n,
    std::align_val_t a,
    const std::nothrow_t &) noexcept {
  return allocateNoThrow(n, static_cast<std::size_t>(a));
}
void operator delete(void *p) noexcept {
  std::free(p);
}
void operator delete[](void *p) noexcept {
  std::free(p);
}
void operator delete(void *p, std::size_t) noexcept {
  std::free(p);
}
void operator delete[](void *p, std::size_t) noexcept {
  std::free(p);
}
void operator delete(void *p, const std::nothrow_t &) noexcept {
  std::free(p);
}
void operator delete[](void *p, const std::nothrow_t &) noexcept {
  std::free(p);
}
void operator delete(void *p, std::align_val_t) noexcept {
  std::free(p);
}
void operator delete[](void *p, std::align_val_t) noexcept {
  std::free(p);
}
void operator delete(void *p, std::size_t, std::align_val_t) noexcept {
  std::free(p);
}
void operator delete[](void *p, std::size_t, std::align_val_t) noexcept {
  std::free(p);
}
void operator delete(
    void *p,
    std::align_val_t,
    const std::nothrow_t &) noexcept {
  std::free(p);
}
void operator delete[](
    void *p,
    std::align_val_t,
    const std::nothrow_t &) noexcept {
  std::free(p);
}

namespace {

using namespace hermes_intl::core;

TEST(RustCoreAllocTest, TakeStringFreesWhenCopyThrows) {
  std::string tag = "en-u-ca-gregory-nu-latn-hc-h23-co-phonebk-kf-upper";
  HiString out;
  ASSERT_TRUE(hi_locale_canonicalize(
      reinterpret_cast<const uint8_t *>(tag.data()), tag.size(), &out));
  ASSERT_GT(out.len, 32u);
  failNextAllocation = true;
  EXPECT_THROW(takeString(out), std::bad_alloc);
  failNextAllocation = false;
}

} // namespace
