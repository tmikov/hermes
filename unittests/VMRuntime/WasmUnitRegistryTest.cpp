/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/VM/static_h.h"

#include "gtest/gtest.h"

#include <cstdlib>

#ifdef HERMES_ENABLE_WASM

namespace {

// None of these is ever called: the registry stores and compares creators,
// it never runs them. Distinct functions per test because the registry is a
// process-global list with no unregistration, so a creator one test
// registered stays registered for every test after it.
SHUnit *findCreator(void) {
  abort();
}
SHUnit *neverRegistered(void) {
  abort();
}
SHUnit *dupNameCreator(void) {
  abort();
}
SHUnit *sharedCreator(void) {
  abort();
}
SHUnit *nullConfigCreator(void) {
  abort();
}
SHUnit *conflictCreator(void) {
  abort();
}

TEST(WasmUnitRegistryTest, FindsTheRegistrationOfACreator) {
  static SHWasmUnitReg reg = {"regtest_find", findCreator, "cfg", nullptr};
  _sh_wasm_register_unit(&reg);
  EXPECT_EQ(&reg, _sh_wasm_find_unit_reg(findCreator));
  EXPECT_EQ(nullptr, _sh_wasm_find_unit_reg(neverRegistered));
}

TEST(WasmUnitRegistryDeathTest, DuplicateNameAlwaysAborts) {
  // The same creator and the same configuration: identical in every field
  // but the node itself, and still refused, because two units under one
  // name cannot both be found by fromNativeUnit().
  static SHWasmUnitReg first = {"regtest_dup", dupNameCreator, "cfg", nullptr};
  static SHWasmUnitReg second = {"regtest_dup", dupNameCreator, "cfg", nullptr};
  _sh_wasm_register_unit(&first);
  EXPECT_DEATH(
      _sh_wasm_register_unit(&second), "duplicate Wasm unit registration");
}

TEST(WasmUnitRegistryTest, OneCreatorUnderTwoNamesWithEqualConfigs) {
  // Equal text at two addresses: the registry must compare contents, and
  // two identical literals may be merged into one, which would let a
  // pointer comparison pass.
  static const char cfgA[] = "cfg";
  static const char cfgB[] = "cfg";
  ASSERT_NE(&cfgA[0], &cfgB[0]);
  static SHWasmUnitReg a = {"regtest_shared_a", sharedCreator, cfgA, nullptr};
  static SHWasmUnitReg b = {"regtest_shared_b", sharedCreator, cfgB, nullptr};
  _sh_wasm_register_unit(&a);
  _sh_wasm_register_unit(&b);
  const SHWasmUnitReg *found = _sh_wasm_find_unit_reg(sharedCreator);
  ASSERT_NE(nullptr, found);
  EXPECT_STREQ("cfg", found->codegen_config);
}

TEST(WasmUnitRegistryTest, NullConfigEqualsOnlyNull) {
  static SHWasmUnitReg a = {
      "regtest_null_a", nullConfigCreator, nullptr, nullptr};
  static SHWasmUnitReg b = {
      "regtest_null_b", nullConfigCreator, nullptr, nullptr};
  _sh_wasm_register_unit(&a);
  _sh_wasm_register_unit(&b);
  EXPECT_NE(nullptr, _sh_wasm_find_unit_reg(nullConfigCreator));
}

TEST(WasmUnitRegistryDeathTest, OneCreatorWithTwoConfigsAborts) {
  static const char cfg1[] = "cfg-1";
  static const char cfg2[] = "cfg-2";
  static SHWasmUnitReg first = {
      "regtest_conflict_a", conflictCreator, cfg1, nullptr};
  static SHWasmUnitReg differs = {
      "regtest_conflict_b", conflictCreator, cfg2, nullptr};
  static SHWasmUnitReg none = {
      "regtest_conflict_c", conflictCreator, nullptr, nullptr};
  _sh_wasm_register_unit(&first);
  EXPECT_DEATH(
      _sh_wasm_register_unit(&differs), "conflicting Wasm unit registrations");
  EXPECT_DEATH(
      _sh_wasm_register_unit(&none), "conflicting Wasm unit registrations");
}

} // namespace

#endif // HERMES_ENABLE_WASM
