/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/WasmFrontend/BinaryReaderHermesIRGen.h"
#include "hermes/WasmFrontend/WasmCompile.h"
#include "hermes/WasmFrontend/WasmModuleInfo.h"
#include "hermes/WasmFrontend/WasmTypes.h"

#include "hermes/AST/Context.h"
#include "hermes/IR/IR.h"

// wabt headers use #if on macros that may not be defined, triggering -Wundef.
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wundef"
#include "wabt/binary-reader.h"
#include "wabt/feature.h"
#pragma GCC diagnostic pop

#include "gtest/gtest.h"

using namespace hermes::wasm;

namespace {

TEST(WasmTypesTest, ValTypeEncoding) {
  // Verify that the enum values match the Wasm binary encoding.
  EXPECT_EQ(static_cast<uint8_t>(WasmValType::I32), 0x7F);
  EXPECT_EQ(static_cast<uint8_t>(WasmValType::I64), 0x7E);
  EXPECT_EQ(static_cast<uint8_t>(WasmValType::F32), 0x7D);
  EXPECT_EQ(static_cast<uint8_t>(WasmValType::F64), 0x7C);
  EXPECT_EQ(static_cast<uint8_t>(WasmValType::V128), 0x7B);
  EXPECT_EQ(static_cast<uint8_t>(WasmValType::FuncRef), 0x70);
  EXPECT_EQ(static_cast<uint8_t>(WasmValType::ExternRef), 0x6F);
}

TEST(WasmTypesTest, FuncType) {
  WasmFuncType ft;
  ft.params = {WasmValType::I32, WasmValType::I32};
  ft.results = {WasmValType::I32};

  EXPECT_EQ(ft.params.size(), 2u);
  EXPECT_EQ(ft.results.size(), 1u);
  EXPECT_EQ(ft.params[0], WasmValType::I32);
  EXPECT_EQ(ft.params[1], WasmValType::I32);
  EXPECT_EQ(ft.results[0], WasmValType::I32);
}

TEST(WasmTypesTest, FuncTypeEmpty) {
  // A function with no params and no results (void -> void).
  WasmFuncType ft;
  EXPECT_TRUE(ft.params.empty());
  EXPECT_TRUE(ft.results.empty());
}

TEST(WasmTypesTest, FuncTypeMultiReturn) {
  WasmFuncType ft;
  ft.params = {WasmValType::F64};
  ft.results = {WasmValType::I32, WasmValType::F64};

  EXPECT_EQ(ft.params.size(), 1u);
  EXPECT_EQ(ft.results.size(), 2u);
  EXPECT_EQ(ft.results[0], WasmValType::I32);
  EXPECT_EQ(ft.results[1], WasmValType::F64);
}

TEST(WasmTypesTest, LimitsDefaults) {
  WasmLimits lim;
  EXPECT_EQ(lim.initial, 0u);
  EXPECT_EQ(lim.maximum, UINT32_MAX);
  EXPECT_FALSE(lim.hasMaximum);
}

TEST(WasmTypesTest, LimitsWithMax) {
  WasmLimits lim;
  lim.initial = 1;
  lim.maximum = 10;
  lim.hasMaximum = true;

  EXPECT_EQ(lim.initial, 1u);
  EXPECT_EQ(lim.maximum, 10u);
  EXPECT_TRUE(lim.hasMaximum);
}

TEST(WasmTypesTest, TableType) {
  WasmTableType tt;
  EXPECT_EQ(tt.elemType, WasmValType::FuncRef);
  EXPECT_EQ(tt.limits.initial, 0u);
  EXPECT_FALSE(tt.limits.hasMaximum);

  tt.elemType = WasmValType::ExternRef;
  tt.limits.initial = 5;
  tt.limits.maximum = 100;
  tt.limits.hasMaximum = true;

  EXPECT_EQ(tt.elemType, WasmValType::ExternRef);
  EXPECT_EQ(tt.limits.initial, 5u);
  EXPECT_EQ(tt.limits.maximum, 100u);
  EXPECT_TRUE(tt.limits.hasMaximum);
}

TEST(WasmTypesTest, MemoryType) {
  WasmMemoryType mt;
  EXPECT_EQ(mt.limits.initial, 0u);
  EXPECT_FALSE(mt.limits.hasMaximum);

  mt.limits.initial = 1;
  mt.limits.maximum = 256;
  mt.limits.hasMaximum = true;

  EXPECT_EQ(mt.limits.initial, 1u);
  EXPECT_EQ(mt.limits.maximum, 256u);
  EXPECT_TRUE(mt.limits.hasMaximum);
}

TEST(WasmTypesTest, GlobalType) {
  WasmGlobalType gt;
  EXPECT_EQ(gt.type, WasmValType::I32);
  EXPECT_FALSE(gt.mutable_);

  gt.type = WasmValType::F64;
  gt.mutable_ = true;

  EXPECT_EQ(gt.type, WasmValType::F64);
  EXPECT_TRUE(gt.mutable_);
}

TEST(WasmTypesTest, ExternalKindEncoding) {
  EXPECT_EQ(static_cast<uint8_t>(WasmExternalKind::Function), 0);
  EXPECT_EQ(static_cast<uint8_t>(WasmExternalKind::Table), 1);
  EXPECT_EQ(static_cast<uint8_t>(WasmExternalKind::Memory), 2);
  EXPECT_EQ(static_cast<uint8_t>(WasmExternalKind::Global), 3);
}

TEST(WasmTypesTest, ImportFunction) {
  WasmImport imp;
  imp.moduleName = "env";
  imp.fieldName = "log";
  imp.kind = WasmExternalKind::Function;
  imp.typeIndex = 0;

  EXPECT_EQ(imp.moduleName, "env");
  EXPECT_EQ(imp.fieldName, "log");
  EXPECT_EQ(imp.kind, WasmExternalKind::Function);
  EXPECT_EQ(imp.typeIndex, 0u);
}

TEST(WasmTypesTest, ImportTable) {
  WasmImport imp;
  imp.moduleName = "js";
  imp.fieldName = "table";
  imp.kind = WasmExternalKind::Table;
  imp.tableType.elemType = WasmValType::FuncRef;
  imp.tableType.limits.initial = 10;
  imp.tableType.limits.maximum = 20;
  imp.tableType.limits.hasMaximum = true;

  EXPECT_EQ(imp.kind, WasmExternalKind::Table);
  EXPECT_EQ(imp.tableType.elemType, WasmValType::FuncRef);
  EXPECT_EQ(imp.tableType.limits.initial, 10u);
  EXPECT_EQ(imp.tableType.limits.maximum, 20u);
  EXPECT_TRUE(imp.tableType.limits.hasMaximum);
}

TEST(WasmTypesTest, ImportMemory) {
  WasmImport imp;
  imp.moduleName = "js";
  imp.fieldName = "mem";
  imp.kind = WasmExternalKind::Memory;
  imp.memoryType.limits.initial = 1;

  EXPECT_EQ(imp.kind, WasmExternalKind::Memory);
  EXPECT_EQ(imp.memoryType.limits.initial, 1u);
}

TEST(WasmTypesTest, ImportGlobal) {
  WasmImport imp;
  imp.moduleName = "env";
  imp.fieldName = "g";
  imp.kind = WasmExternalKind::Global;
  imp.globalType.type = WasmValType::I64;
  imp.globalType.mutable_ = true;

  EXPECT_EQ(imp.kind, WasmExternalKind::Global);
  EXPECT_EQ(imp.globalType.type, WasmValType::I64);
  EXPECT_TRUE(imp.globalType.mutable_);
}

TEST(WasmTypesTest, ImportDefaults) {
  WasmImport imp;
  EXPECT_TRUE(imp.moduleName.empty());
  EXPECT_TRUE(imp.fieldName.empty());
  EXPECT_EQ(imp.kind, WasmExternalKind::Function);
  EXPECT_EQ(imp.typeIndex, 0u);
}

TEST(WasmTypesTest, Export) {
  WasmExport exp;
  exp.name = "add";
  exp.kind = WasmExternalKind::Function;
  exp.index = 3;

  EXPECT_EQ(exp.name, "add");
  EXPECT_EQ(exp.kind, WasmExternalKind::Function);
  EXPECT_EQ(exp.index, 3u);
}

TEST(WasmTypesTest, ExportDefaults) {
  WasmExport exp;
  EXPECT_TRUE(exp.name.empty());
  EXPECT_EQ(exp.kind, WasmExternalKind::Function);
  EXPECT_EQ(exp.index, 0u);
}

TEST(WasmTypesTest, ExportMemory) {
  WasmExport exp;
  exp.name = "memory";
  exp.kind = WasmExternalKind::Memory;
  exp.index = 0;

  EXPECT_EQ(exp.kind, WasmExternalKind::Memory);
}

TEST(WasmTypesTest, Function) {
  WasmFunction fn;
  fn.typeIndex = 42;
  EXPECT_EQ(fn.typeIndex, 42u);
}

TEST(WasmTypesTest, FunctionDefault) {
  WasmFunction fn;
  EXPECT_EQ(fn.typeIndex, 0u);
}

TEST(WasmTypesTest, GlobalI32Const) {
  WasmGlobal g;
  g.type.type = WasmValType::I32;
  g.type.mutable_ = false;
  g.initKind = WasmGlobal::InitKind::I32Const;
  g.initValue.i32Val = 42;

  EXPECT_EQ(g.type.type, WasmValType::I32);
  EXPECT_FALSE(g.type.mutable_);
  EXPECT_EQ(g.initKind, WasmGlobal::InitKind::I32Const);
  EXPECT_EQ(g.initValue.i32Val, 42);
}

TEST(WasmTypesTest, GlobalI64Const) {
  WasmGlobal g;
  g.type.type = WasmValType::I64;
  g.type.mutable_ = true;
  g.initKind = WasmGlobal::InitKind::I64Const;
  g.initValue.i64Val = 0x100000000LL;

  EXPECT_EQ(g.initKind, WasmGlobal::InitKind::I64Const);
  EXPECT_EQ(g.initValue.i64Val, 0x100000000LL);
}

TEST(WasmTypesTest, GlobalF64Const) {
  WasmGlobal g;
  g.type.type = WasmValType::F64;
  g.initKind = WasmGlobal::InitKind::F64Const;
  g.initValue.f64Val = 3.14;

  EXPECT_EQ(g.initKind, WasmGlobal::InitKind::F64Const);
  EXPECT_DOUBLE_EQ(g.initValue.f64Val, 3.14);
}

TEST(WasmTypesTest, GlobalF32Const) {
  WasmGlobal g;
  g.type.type = WasmValType::F32;
  g.initKind = WasmGlobal::InitKind::F32Const;
  g.initValue.f32Val = 2.5f;

  EXPECT_EQ(g.initKind, WasmGlobal::InitKind::F32Const);
  EXPECT_FLOAT_EQ(g.initValue.f32Val, 2.5f);
}

TEST(WasmTypesTest, GlobalGetInit) {
  WasmGlobal g;
  g.initKind = WasmGlobal::InitKind::GlobalGet;
  g.initValue.globalIndex = 7;

  EXPECT_EQ(g.initKind, WasmGlobal::InitKind::GlobalGet);
  EXPECT_EQ(g.initValue.globalIndex, 7u);
}

TEST(WasmTypesTest, GlobalRefNull) {
  WasmGlobal g;
  g.type.type = WasmValType::FuncRef;
  g.initKind = WasmGlobal::InitKind::RefNull;

  EXPECT_EQ(g.initKind, WasmGlobal::InitKind::RefNull);
}

TEST(WasmTypesTest, GlobalRefFunc) {
  WasmGlobal g;
  g.type.type = WasmValType::FuncRef;
  g.initKind = WasmGlobal::InitKind::RefFunc;
  g.initValue.funcIndex = 5;

  EXPECT_EQ(g.initKind, WasmGlobal::InitKind::RefFunc);
  EXPECT_EQ(g.initValue.funcIndex, 5u);
}

TEST(WasmTypesTest, GlobalDefaults) {
  WasmGlobal g;
  EXPECT_EQ(g.type.type, WasmValType::I32);
  EXPECT_FALSE(g.type.mutable_);
  EXPECT_EQ(g.initKind, WasmGlobal::InitKind::I32Const);
  EXPECT_EQ(g.initValue.i32Val, 0);
}

TEST(WasmTypesTest, ElemSegmentActiveDefaults) {
  WasmElemSegment seg;
  EXPECT_EQ(seg.mode, WasmElemSegment::Mode::Active);
  EXPECT_EQ(seg.tableIndex, 0u);
  EXPECT_EQ(seg.offsetKind, WasmGlobal::InitKind::I32Const);
  EXPECT_EQ(seg.offsetValue, 0);
  EXPECT_EQ(seg.offsetGlobalIdx, 0u);
  EXPECT_TRUE(seg.funcIndices.empty());
}

TEST(WasmTypesTest, ElemSegmentActive) {
  WasmElemSegment seg;
  seg.mode = WasmElemSegment::Mode::Active;
  seg.tableIndex = 0;
  seg.offsetKind = WasmGlobal::InitKind::I32Const;
  seg.offsetValue = 10;
  seg.funcIndices = {0, 1, 2, 3};

  EXPECT_EQ(seg.mode, WasmElemSegment::Mode::Active);
  EXPECT_EQ(seg.offsetValue, 10);
  EXPECT_EQ(seg.funcIndices.size(), 4u);
  EXPECT_EQ(seg.funcIndices[0], 0u);
  EXPECT_EQ(seg.funcIndices[3], 3u);
}

TEST(WasmTypesTest, ElemSegmentGlobalGetOffset) {
  WasmElemSegment seg;
  seg.mode = WasmElemSegment::Mode::Active;
  seg.offsetKind = WasmGlobal::InitKind::GlobalGet;
  seg.offsetGlobalIdx = 2;
  seg.funcIndices = {5};

  EXPECT_EQ(seg.offsetKind, WasmGlobal::InitKind::GlobalGet);
  EXPECT_EQ(seg.offsetGlobalIdx, 2u);
  EXPECT_EQ(seg.funcIndices.size(), 1u);
}

TEST(WasmTypesTest, ElemSegmentPassive) {
  WasmElemSegment seg;
  seg.mode = WasmElemSegment::Mode::Passive;
  seg.funcIndices = {7, 8};

  EXPECT_EQ(seg.mode, WasmElemSegment::Mode::Passive);
  EXPECT_EQ(seg.funcIndices.size(), 2u);
}

TEST(WasmTypesTest, ElemSegmentDeclarative) {
  WasmElemSegment seg;
  seg.mode = WasmElemSegment::Mode::Declarative;

  EXPECT_EQ(seg.mode, WasmElemSegment::Mode::Declarative);
}

TEST(WasmTypesTest, DataSegmentActiveDefaults) {
  WasmDataSegment seg;
  EXPECT_EQ(seg.mode, WasmDataSegment::Mode::Active);
  EXPECT_EQ(seg.memoryIndex, 0u);
  EXPECT_EQ(seg.offsetKind, WasmGlobal::InitKind::I32Const);
  EXPECT_EQ(seg.offsetValue, 0);
  EXPECT_EQ(seg.offsetGlobalIdx, 0u);
  EXPECT_TRUE(seg.data.empty());
}

TEST(WasmTypesTest, DataSegmentActive) {
  WasmDataSegment seg;
  seg.mode = WasmDataSegment::Mode::Active;
  seg.memoryIndex = 0;
  seg.offsetKind = WasmGlobal::InitKind::I32Const;
  seg.offsetValue = 1024;
  seg.data = {0x48, 0x65, 0x6C, 0x6C, 0x6F}; // "Hello"

  EXPECT_EQ(seg.mode, WasmDataSegment::Mode::Active);
  EXPECT_EQ(seg.memoryIndex, 0u);
  EXPECT_EQ(seg.offsetValue, 1024);
  EXPECT_EQ(seg.data.size(), 5u);
  EXPECT_EQ(seg.data[0], 0x48);
  EXPECT_EQ(seg.data[4], 0x6F);
}

TEST(WasmTypesTest, DataSegmentGlobalGetOffset) {
  WasmDataSegment seg;
  seg.mode = WasmDataSegment::Mode::Active;
  seg.offsetKind = WasmGlobal::InitKind::GlobalGet;
  seg.offsetGlobalIdx = 1;
  seg.data = {0xFF};

  EXPECT_EQ(seg.offsetKind, WasmGlobal::InitKind::GlobalGet);
  EXPECT_EQ(seg.offsetGlobalIdx, 1u);
}

TEST(WasmTypesTest, DataSegmentPassive) {
  WasmDataSegment seg;
  seg.mode = WasmDataSegment::Mode::Passive;
  seg.data = {0x00, 0x01, 0x02};

  EXPECT_EQ(seg.mode, WasmDataSegment::Mode::Passive);
  EXPECT_EQ(seg.data.size(), 3u);
}

TEST(WasmTypesTest, NameSectionDefaults) {
  WasmNameSection ns;
  EXPECT_TRUE(ns.moduleName.empty());
  EXPECT_TRUE(ns.functionNames.empty());
}

TEST(WasmTypesTest, NameSection) {
  WasmNameSection ns;
  ns.moduleName = "test_module";
  ns.functionNames = {"add", "sub", "mul"};

  EXPECT_EQ(ns.moduleName, "test_module");
  EXPECT_EQ(ns.functionNames.size(), 3u);
  EXPECT_EQ(ns.functionNames[0], "add");
  EXPECT_EQ(ns.functionNames[1], "sub");
  EXPECT_EQ(ns.functionNames[2], "mul");
}

TEST(WasmTypesTest, NameSectionEmptyModuleName) {
  WasmNameSection ns;
  ns.functionNames = {"f0", "", "f2"};

  EXPECT_TRUE(ns.moduleName.empty());
  EXPECT_EQ(ns.functionNames.size(), 3u);
  EXPECT_EQ(ns.functionNames[1], "");
}

// --- WasmModuleInfo tests ---

/// Helper to build a WasmModuleInfo with 2 imported functions and 3 defined
/// functions, plus some imports of other kinds.
static WasmModuleInfo buildTestModuleInfo() {
  WasmModuleInfo mod;

  // Type section: 3 distinct signatures.
  // Type 0: (i32, i32) -> i32
  WasmFuncType ft0;
  ft0.params = {WasmValType::I32, WasmValType::I32};
  ft0.results = {WasmValType::I32};
  mod.types.push_back(std::move(ft0));
  // Type 1: () -> void
  WasmFuncType ft1;
  mod.types.push_back(std::move(ft1));
  // Type 2: (f64) -> f64
  WasmFuncType ft2;
  ft2.params = {WasmValType::F64};
  ft2.results = {WasmValType::F64};
  mod.types.push_back(std::move(ft2));

  // Imports: 2 functions, 1 table, 1 memory, 1 global (5 total imports).
  {
    WasmImport imp;
    imp.moduleName = "env";
    imp.fieldName = "add";
    imp.kind = WasmExternalKind::Function;
    imp.typeIndex = 0; // (i32, i32) -> i32
    mod.imports.push_back(std::move(imp));
  }
  {
    WasmImport imp;
    imp.moduleName = "env";
    imp.fieldName = "table";
    imp.kind = WasmExternalKind::Table;
    imp.tableType.elemType = WasmValType::FuncRef;
    imp.tableType.limits.initial = 10;
    mod.imports.push_back(std::move(imp));
  }
  {
    WasmImport imp;
    imp.moduleName = "env";
    imp.fieldName = "nop";
    imp.kind = WasmExternalKind::Function;
    imp.typeIndex = 1; // () -> void
    mod.imports.push_back(std::move(imp));
  }
  {
    WasmImport imp;
    imp.moduleName = "env";
    imp.fieldName = "mem";
    imp.kind = WasmExternalKind::Memory;
    imp.memoryType.limits.initial = 1;
    mod.imports.push_back(std::move(imp));
  }
  {
    WasmImport imp;
    imp.moduleName = "env";
    imp.fieldName = "g";
    imp.kind = WasmExternalKind::Global;
    imp.globalType.type = WasmValType::I32;
    imp.globalType.mutable_ = false;
    mod.imports.push_back(std::move(imp));
  }

  // Defined functions: 3 functions.
  mod.functions.push_back(WasmFunction{0}); // type 0: (i32, i32) -> i32
  mod.functions.push_back(WasmFunction{2}); // type 2: (f64) -> f64
  mod.functions.push_back(WasmFunction{1}); // type 1: () -> void

  // Defined tables: 1.
  WasmTableType tt;
  tt.elemType = WasmValType::FuncRef;
  tt.limits.initial = 5;
  mod.tables.push_back(std::move(tt));

  // Defined memories: 1.
  WasmMemoryType mt;
  mt.limits.initial = 2;
  mod.memories.push_back(std::move(mt));

  // Defined globals: 2.
  WasmGlobal g0;
  g0.type.type = WasmValType::I32;
  g0.initKind = WasmGlobal::InitKind::I32Const;
  g0.initValue.i32Val = 100;
  mod.globals.push_back(std::move(g0));
  WasmGlobal g1;
  g1.type.type = WasmValType::F64;
  g1.initKind = WasmGlobal::InitKind::F64Const;
  g1.initValue.f64Val = 3.14;
  mod.globals.push_back(std::move(g1));

  return mod;
}

TEST(WasmModuleInfoTest, DefaultEmpty) {
  WasmModuleInfo mod;
  EXPECT_EQ(mod.totalFunctionCount(), 0u);
  EXPECT_EQ(mod.importedFunctionCount(), 0u);
  EXPECT_EQ(mod.totalGlobalCount(), 0u);
  EXPECT_EQ(mod.importedGlobalCount(), 0u);
  EXPECT_EQ(mod.totalTableCount(), 0u);
  EXPECT_EQ(mod.importedTableCount(), 0u);
  EXPECT_EQ(mod.totalMemoryCount(), 0u);
  EXPECT_EQ(mod.importedMemoryCount(), 0u);
  EXPECT_FALSE(mod.startFunction.has_value());
}

TEST(WasmModuleInfoTest, FunctionCounts) {
  auto mod = buildTestModuleInfo();
  EXPECT_EQ(mod.importedFunctionCount(), 2u);
  EXPECT_EQ(mod.totalFunctionCount(), 5u);
}

TEST(WasmModuleInfoTest, GetFunctionTypeImported) {
  auto mod = buildTestModuleInfo();
  // Func index 0 is the first imported function (type 0: (i32, i32) -> i32).
  const auto &ft0 = mod.getFunctionType(0);
  EXPECT_EQ(ft0.params.size(), 2u);
  EXPECT_EQ(ft0.params[0], WasmValType::I32);
  EXPECT_EQ(ft0.params[1], WasmValType::I32);
  EXPECT_EQ(ft0.results.size(), 1u);
  EXPECT_EQ(ft0.results[0], WasmValType::I32);

  // Func index 1 is the second imported function (type 1: () -> void).
  const auto &ft1 = mod.getFunctionType(1);
  EXPECT_TRUE(ft1.params.empty());
  EXPECT_TRUE(ft1.results.empty());
}

TEST(WasmModuleInfoTest, GetFunctionTypeDefined) {
  auto mod = buildTestModuleInfo();
  // Func index 2 is the first defined function (type 0: (i32, i32) -> i32).
  const auto &ft2 = mod.getFunctionType(2);
  EXPECT_EQ(ft2.params.size(), 2u);
  EXPECT_EQ(ft2.results.size(), 1u);
  EXPECT_EQ(ft2.results[0], WasmValType::I32);

  // Func index 3 is the second defined function (type 2: (f64) -> f64).
  const auto &ft3 = mod.getFunctionType(3);
  EXPECT_EQ(ft3.params.size(), 1u);
  EXPECT_EQ(ft3.params[0], WasmValType::F64);
  EXPECT_EQ(ft3.results.size(), 1u);
  EXPECT_EQ(ft3.results[0], WasmValType::F64);

  // Func index 4 is the third defined function (type 1: () -> void).
  const auto &ft4 = mod.getFunctionType(4);
  EXPECT_TRUE(ft4.params.empty());
  EXPECT_TRUE(ft4.results.empty());
}

TEST(WasmModuleInfoTest, GlobalCounts) {
  auto mod = buildTestModuleInfo();
  // 1 imported global + 2 defined globals = 3 total.
  EXPECT_EQ(mod.importedGlobalCount(), 1u);
  EXPECT_EQ(mod.totalGlobalCount(), 3u);
}

TEST(WasmModuleInfoTest, TableCounts) {
  auto mod = buildTestModuleInfo();
  // 1 imported table + 1 defined table = 2 total.
  EXPECT_EQ(mod.importedTableCount(), 1u);
  EXPECT_EQ(mod.totalTableCount(), 2u);
}

TEST(WasmModuleInfoTest, MemoryCounts) {
  auto mod = buildTestModuleInfo();
  // 1 imported memory + 1 defined memory = 2 total.
  EXPECT_EQ(mod.importedMemoryCount(), 1u);
  EXPECT_EQ(mod.totalMemoryCount(), 2u);
}

TEST(WasmModuleInfoTest, StartFunction) {
  WasmModuleInfo mod;
  EXPECT_FALSE(mod.startFunction.has_value());

  mod.startFunction = 3;
  EXPECT_TRUE(mod.startFunction.has_value());
  EXPECT_EQ(*mod.startFunction, 3u);
}

TEST(WasmModuleInfoTest, NoImportsCounts) {
  // Module with only defined items, no imports.
  WasmModuleInfo mod;
  WasmFuncType ft;
  ft.params = {WasmValType::I32};
  ft.results = {WasmValType::I32};
  mod.types.push_back(std::move(ft));
  mod.functions.push_back(WasmFunction{0});
  mod.functions.push_back(WasmFunction{0});

  WasmTableType tt;
  tt.elemType = WasmValType::FuncRef;
  tt.limits.initial = 1;
  mod.tables.push_back(std::move(tt));

  WasmMemoryType mt;
  mt.limits.initial = 1;
  mod.memories.push_back(std::move(mt));

  WasmGlobal g;
  g.type.type = WasmValType::I32;
  mod.globals.push_back(std::move(g));

  EXPECT_EQ(mod.importedFunctionCount(), 0u);
  EXPECT_EQ(mod.totalFunctionCount(), 2u);
  EXPECT_EQ(mod.importedTableCount(), 0u);
  EXPECT_EQ(mod.totalTableCount(), 1u);
  EXPECT_EQ(mod.importedMemoryCount(), 0u);
  EXPECT_EQ(mod.totalMemoryCount(), 1u);
  EXPECT_EQ(mod.importedGlobalCount(), 0u);
  EXPECT_EQ(mod.totalGlobalCount(), 1u);

  // getFunctionType for defined-only functions.
  const auto &ft0 = mod.getFunctionType(0);
  EXPECT_EQ(ft0.params.size(), 1u);
  EXPECT_EQ(ft0.params[0], WasmValType::I32);
}

TEST(WasmModuleInfoTest, OnlyImportsCounts) {
  // Module with only imports, no defined items.
  WasmModuleInfo mod;
  WasmFuncType ft;
  ft.results = {WasmValType::I32};
  mod.types.push_back(std::move(ft));

  WasmImport imp;
  imp.kind = WasmExternalKind::Function;
  imp.typeIndex = 0;
  mod.imports.push_back(std::move(imp));

  EXPECT_EQ(mod.importedFunctionCount(), 1u);
  EXPECT_EQ(mod.totalFunctionCount(), 1u);

  const auto &ft0 = mod.getFunctionType(0);
  EXPECT_EQ(ft0.results.size(), 1u);
  EXPECT_EQ(ft0.results[0], WasmValType::I32);
}

// --- BinaryReaderHermesIRGen tests ---

/// Helper to parse a Wasm binary byte array and populate a WasmModuleInfo.
/// \return true if parsing succeeded.
static bool parseWasmBinary(
    const std::vector<uint8_t> &binary,
    WasmModuleInfo &moduleInfo) {
  BinaryReaderHermesIRGen reader(moduleInfo);
  wabt::ReadBinaryOptions options;
  options.read_debug_names = true;
  wabt::Result result =
      wabt::ReadBinary(binary.data(), binary.size(), &reader, options);
  return wabt::Succeeded(result);
}

/// Build a minimal valid Wasm binary:
///   - 1 type: (i32, i32) -> i32
///   - 1 function with that type
///   - 1 memory (1 page min, 10 pages max)
///   - 1 export: function "add" at index 0
///   - Function body: empty (just returns)
static std::vector<uint8_t> buildMinimalWasm() {
  // clang-format off
  return {
    // Header
    0x00, 0x61, 0x73, 0x6D, // magic: \0asm
    0x01, 0x00, 0x00, 0x00, // version: 1

    // Type section (id=1)
    0x01,                   // section id
    0x07,                   // section size (7 bytes)
    0x01,                   // count: 1 type
    0x60,                   // func type
    0x02,                   // param count: 2
    0x7F, 0x7F,             // params: i32, i32
    0x01,                   // result count: 1
    0x7F,                   // result: i32

    // Function section (id=3)
    0x03,                   // section id
    0x02,                   // section size (2 bytes)
    0x01,                   // count: 1 function
    0x00,                   // type index: 0

    // Memory section (id=5)
    0x05,                   // section id
    0x04,                   // section size (4 bytes)
    0x01,                   // count: 1 memory
    0x01,                   // has max: yes
    0x01,                   // initial: 1 page
    0x0A,                   // maximum: 10 pages

    // Export section (id=7)
    0x07,                   // section id
    0x07,                   // section size (7 bytes)
    0x01,                   // count: 1 export
    0x03,                   // name length: 3
    0x61, 0x64, 0x64,       // name: "add"
    0x00,                   // kind: function
    0x00,                   // index: 0

    // Code section (id=10)
    0x0A,                   // section id
    0x06,                   // section size (6 bytes)
    0x01,                   // count: 1 function body
    0x04,                   // body size (4 bytes)
    0x00,                   // local decl count: 0
    0x20, 0x00,             // local.get 0
    0x0B,                   // end
  };
  // clang-format on
}

TEST(BinaryReaderTest, MinimalModule) {
  auto binary = buildMinimalWasm();
  WasmModuleInfo moduleInfo;
  ASSERT_TRUE(parseWasmBinary(binary, moduleInfo));

  // Type section: 1 type (i32, i32) -> i32
  ASSERT_EQ(moduleInfo.types.size(), 1u);
  EXPECT_EQ(moduleInfo.types[0].params.size(), 2u);
  EXPECT_EQ(moduleInfo.types[0].params[0], WasmValType::I32);
  EXPECT_EQ(moduleInfo.types[0].params[1], WasmValType::I32);
  EXPECT_EQ(moduleInfo.types[0].results.size(), 1u);
  EXPECT_EQ(moduleInfo.types[0].results[0], WasmValType::I32);

  // Function section: 1 function with type index 0
  ASSERT_EQ(moduleInfo.functions.size(), 1u);
  EXPECT_EQ(moduleInfo.functions[0].typeIndex, 0u);

  // Memory section: 1 memory with min=1, max=10
  ASSERT_EQ(moduleInfo.memories.size(), 1u);
  EXPECT_EQ(moduleInfo.memories[0].limits.initial, 1u);
  EXPECT_TRUE(moduleInfo.memories[0].limits.hasMaximum);
  EXPECT_EQ(moduleInfo.memories[0].limits.maximum, 10u);

  // Export section: 1 export "add" -> function 0
  ASSERT_EQ(moduleInfo.exports.size(), 1u);
  EXPECT_EQ(moduleInfo.exports[0].name, "add");
  EXPECT_EQ(moduleInfo.exports[0].kind, WasmExternalKind::Function);
  EXPECT_EQ(moduleInfo.exports[0].index, 0u);

  // No imports, no globals, no start, no elem/data segments.
  EXPECT_TRUE(moduleInfo.imports.empty());
  EXPECT_TRUE(moduleInfo.globals.empty());
  EXPECT_FALSE(moduleInfo.startFunction.has_value());
  EXPECT_TRUE(moduleInfo.elements.empty());
  EXPECT_TRUE(moduleInfo.dataSegments.empty());
  EXPECT_TRUE(moduleInfo.tables.empty());

  // Counts
  EXPECT_EQ(moduleInfo.totalFunctionCount(), 1u);
  EXPECT_EQ(moduleInfo.importedFunctionCount(), 0u);
  EXPECT_EQ(moduleInfo.totalMemoryCount(), 1u);
}

/// Build a Wasm binary with imports:
///   - 2 types: (i32, i32) -> i32, () -> void
///   - 2 function imports: "env"."add" (type 0), "env"."nop" (type 1)
///   - 1 memory import: "env"."mem" (1 page min)
///   - 1 global import: "env"."g" (i32, immutable)
///   - 1 defined function (type 0)
///   - 1 export: function "main" at index 2
static std::vector<uint8_t> buildImportsWasm() {
  // clang-format off
  return {
    // Header
    0x00, 0x61, 0x73, 0x6D,
    0x01, 0x00, 0x00, 0x00,

    // Type section (id=1, size=10)
    0x01, 0x0A,
    0x02,                   // count: 2 types
    0x60, 0x02, 0x7F, 0x7F, 0x01, 0x7F, // (i32, i32) -> i32
    0x60, 0x00, 0x00,                     // () -> void

    // Import section (id=2, size=41)
    0x02, 0x29,
    0x04,                   // count: 4 imports
    // Import 0: function "env"."add" (type 0)
    0x03, 0x65, 0x6E, 0x76, // "env"
    0x03, 0x61, 0x64, 0x64, // "add"
    0x00, 0x00,             // kind=func, type=0
    // Import 1: function "env"."nop" (type 1)
    0x03, 0x65, 0x6E, 0x76, // "env"
    0x03, 0x6E, 0x6F, 0x70, // "nop"
    0x00, 0x01,             // kind=func, type=1
    // Import 2: memory "env"."mem" (1 page, no max)
    0x03, 0x65, 0x6E, 0x76, // "env"
    0x03, 0x6D, 0x65, 0x6D, // "mem"
    0x02, 0x00, 0x01,       // kind=memory, no max, initial=1
    // Import 3: global "env"."g" (i32, immutable)
    0x03, 0x65, 0x6E, 0x76, // "env"
    0x01, 0x67,             // "g"
    0x03, 0x7F, 0x00,       // kind=global, i32, immutable

    // Function section (id=3, size=2)
    0x03, 0x02,
    0x01, 0x00,             // count=1, type=0

    // Export section (id=7, size=8)
    0x07, 0x08,
    0x01,                   // count: 1
    0x04, 0x6D, 0x61, 0x69, 0x6E, // "main"
    0x00, 0x02,             // kind=func, index=2

    // Code section (id=10, size=6)
    0x0A, 0x06,
    0x01,                   // count: 1
    0x04,                   // body size: 4
    0x00,                   // local decl count: 0
    0x20, 0x00,             // local.get 0
    0x0B,                   // end
  };
  // clang-format on
}

TEST(BinaryReaderTest, ImportsModule) {
  auto binary = buildImportsWasm();
  WasmModuleInfo moduleInfo;
  ASSERT_TRUE(parseWasmBinary(binary, moduleInfo));

  // Types
  ASSERT_EQ(moduleInfo.types.size(), 2u);

  // Imports: 4 total (2 func, 1 mem, 1 global)
  ASSERT_EQ(moduleInfo.imports.size(), 4u);

  // Import 0: function "env"."add"
  EXPECT_EQ(moduleInfo.imports[0].moduleName, "env");
  EXPECT_EQ(moduleInfo.imports[0].fieldName, "add");
  EXPECT_EQ(moduleInfo.imports[0].kind, WasmExternalKind::Function);
  EXPECT_EQ(moduleInfo.imports[0].typeIndex, 0u);

  // Import 1: function "env"."nop"
  EXPECT_EQ(moduleInfo.imports[1].moduleName, "env");
  EXPECT_EQ(moduleInfo.imports[1].fieldName, "nop");
  EXPECT_EQ(moduleInfo.imports[1].kind, WasmExternalKind::Function);
  EXPECT_EQ(moduleInfo.imports[1].typeIndex, 1u);

  // Import 2: memory "env"."mem"
  EXPECT_EQ(moduleInfo.imports[2].moduleName, "env");
  EXPECT_EQ(moduleInfo.imports[2].fieldName, "mem");
  EXPECT_EQ(moduleInfo.imports[2].kind, WasmExternalKind::Memory);
  EXPECT_EQ(moduleInfo.imports[2].memoryType.limits.initial, 1u);
  EXPECT_FALSE(moduleInfo.imports[2].memoryType.limits.hasMaximum);

  // Import 3: global "env"."g"
  EXPECT_EQ(moduleInfo.imports[3].moduleName, "env");
  EXPECT_EQ(moduleInfo.imports[3].fieldName, "g");
  EXPECT_EQ(moduleInfo.imports[3].kind, WasmExternalKind::Global);
  EXPECT_EQ(moduleInfo.imports[3].globalType.type, WasmValType::I32);
  EXPECT_FALSE(moduleInfo.imports[3].globalType.mutable_);

  // 1 defined function
  ASSERT_EQ(moduleInfo.functions.size(), 1u);

  // Counts
  EXPECT_EQ(moduleInfo.importedFunctionCount(), 2u);
  EXPECT_EQ(moduleInfo.totalFunctionCount(), 3u);
  EXPECT_EQ(moduleInfo.importedMemoryCount(), 1u);
  EXPECT_EQ(moduleInfo.totalMemoryCount(), 1u);
  EXPECT_EQ(moduleInfo.importedGlobalCount(), 1u);
  EXPECT_EQ(moduleInfo.totalGlobalCount(), 1u);

  // Export
  ASSERT_EQ(moduleInfo.exports.size(), 1u);
  EXPECT_EQ(moduleInfo.exports[0].name, "main");
  EXPECT_EQ(moduleInfo.exports[0].index, 2u);
}

/// Build a Wasm binary with globals, data segments, and element segments:
///   - 1 type: () -> void
///   - 1 table (funcref, min=2)
///   - 1 memory (min=1)
///   - 2 globals: i32 mutable init=42, f64 immutable init=3.14
///   - 1 function (type 0)
///   - 1 element segment: active, table 0, offset i32.const 0, [func 0]
///   - 1 data segment: active, memory 0, offset i32.const 0, "Hi"
static std::vector<uint8_t> buildSegmentsWasm() {
  // clang-format off
  return {
    // Header
    0x00, 0x61, 0x73, 0x6D,
    0x01, 0x00, 0x00, 0x00,

    // Type section (id=1, size=4)
    0x01, 0x04,
    0x01,                   // count: 1
    0x60, 0x00, 0x00,       // () -> void

    // Function section (id=3, size=2)
    0x03, 0x02,
    0x01, 0x00,             // count=1, type=0

    // Table section (id=4, size=4)
    0x04, 0x04,
    0x01,                   // count: 1
    0x70, 0x00, 0x02,       // funcref, no max, initial=2

    // Memory section (id=5, size=3)
    0x05, 0x03,
    0x01,                   // count: 1
    0x00, 0x01,             // no max, initial=1

    // Global section (id=6, size=18)
    0x06, 0x12,
    0x02,                   // count: 2
    // Global 0: i32, mutable, init = 42
    0x7F, 0x01,             // type=i32, mutable
    0x41, 0x2A, 0x0B,       // i32.const 42, end
    // Global 1: f64, immutable, init = 3.14
    0x7C, 0x00,             // type=f64, immutable
    0x44,                   // f64.const
    0x1F, 0x85, 0xEB, 0x51, 0xB8, 0x1E, 0x09, 0x40, // 3.14 LE
    0x0B,                   // end

    // Export section (id=7, size=5)
    0x07, 0x05,
    0x01,                   // count: 1
    0x01, 0x66,             // "f"
    0x00, 0x00,             // kind=func, index=0

    // Element section (id=9, size=7)
    0x09, 0x07,
    0x01,                   // count: 1
    0x00,                   // flags: active, table 0 implicit
    0x41, 0x00, 0x0B,       // i32.const 0, end
    0x01,                   // num elements: 1
    0x00,                   // func index: 0

    // Code section (id=10, size=4)
    0x0A, 0x04,
    0x01,                   // count: 1
    0x02,                   // body size: 2
    0x00,                   // local decl count: 0
    0x0B,                   // end

    // Data section (id=11, size=8)
    0x0B, 0x08,
    0x01,                   // count: 1
    0x00,                   // flags: active, memory 0
    0x41, 0x00, 0x0B,       // i32.const 0, end
    0x02,                   // data size: 2
    0x48, 0x69,             // "Hi"
  };
  // clang-format on
}

TEST(BinaryReaderTest, SegmentsModule) {
  auto binary = buildSegmentsWasm();
  WasmModuleInfo moduleInfo;
  ASSERT_TRUE(parseWasmBinary(binary, moduleInfo));

  // Tables
  ASSERT_EQ(moduleInfo.tables.size(), 1u);
  EXPECT_EQ(moduleInfo.tables[0].elemType, WasmValType::FuncRef);
  EXPECT_EQ(moduleInfo.tables[0].limits.initial, 2u);
  EXPECT_FALSE(moduleInfo.tables[0].limits.hasMaximum);

  // Globals
  ASSERT_EQ(moduleInfo.globals.size(), 2u);

  // Global 0: i32, mutable, init = 42
  EXPECT_EQ(moduleInfo.globals[0].type.type, WasmValType::I32);
  EXPECT_TRUE(moduleInfo.globals[0].type.mutable_);
  EXPECT_EQ(moduleInfo.globals[0].initKind, WasmGlobal::InitKind::I32Const);
  EXPECT_EQ(moduleInfo.globals[0].initValue.i32Val, 42);

  // Global 1: f64, immutable, init = 3.14
  EXPECT_EQ(moduleInfo.globals[1].type.type, WasmValType::F64);
  EXPECT_FALSE(moduleInfo.globals[1].type.mutable_);
  EXPECT_EQ(moduleInfo.globals[1].initKind, WasmGlobal::InitKind::F64Const);
  EXPECT_DOUBLE_EQ(moduleInfo.globals[1].initValue.f64Val, 3.14);

  // Element segments
  ASSERT_EQ(moduleInfo.elements.size(), 1u);
  EXPECT_EQ(moduleInfo.elements[0].mode, WasmElemSegment::Mode::Active);
  EXPECT_EQ(moduleInfo.elements[0].tableIndex, 0u);
  EXPECT_EQ(
      moduleInfo.elements[0].offsetKind, WasmGlobal::InitKind::I32Const);
  EXPECT_EQ(moduleInfo.elements[0].offsetValue, 0);
  ASSERT_EQ(moduleInfo.elements[0].funcIndices.size(), 1u);
  EXPECT_EQ(moduleInfo.elements[0].funcIndices[0], 0u);

  // Data segments
  ASSERT_EQ(moduleInfo.dataSegments.size(), 1u);
  EXPECT_EQ(moduleInfo.dataSegments[0].mode, WasmDataSegment::Mode::Active);
  EXPECT_EQ(moduleInfo.dataSegments[0].memoryIndex, 0u);
  EXPECT_EQ(
      moduleInfo.dataSegments[0].offsetKind, WasmGlobal::InitKind::I32Const);
  EXPECT_EQ(moduleInfo.dataSegments[0].offsetValue, 0);
  ASSERT_EQ(moduleInfo.dataSegments[0].data.size(), 2u);
  EXPECT_EQ(moduleInfo.dataSegments[0].data[0], 'H');
  EXPECT_EQ(moduleInfo.dataSegments[0].data[1], 'i');
}

TEST(BinaryReaderTest, InvalidMagic) {
  // Invalid magic bytes.
  std::vector<uint8_t> binary = {0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00};
  WasmModuleInfo moduleInfo;
  EXPECT_FALSE(parseWasmBinary(binary, moduleInfo));
}

TEST(BinaryReaderTest, Truncated) {
  // Truncated binary (only magic, no version).
  std::vector<uint8_t> binary = {0x00, 0x61, 0x73, 0x6D};
  WasmModuleInfo moduleInfo;
  EXPECT_FALSE(parseWasmBinary(binary, moduleInfo));
}

TEST(BinaryReaderTest, EmptyBinary) {
  std::vector<uint8_t> binary;
  WasmModuleInfo moduleInfo;
  EXPECT_FALSE(parseWasmBinary(binary, moduleInfo));
}

/// Build a Wasm binary with a start function.
TEST(BinaryReaderTest, StartFunction) {
  // clang-format off
  std::vector<uint8_t> binary = {
    // Header
    0x00, 0x61, 0x73, 0x6D,
    0x01, 0x00, 0x00, 0x00,

    // Type section
    0x01, 0x04, 0x01,
    0x60, 0x00, 0x00,       // () -> void

    // Function section
    0x03, 0x02, 0x01, 0x00, // 1 function, type 0

    // Start section (id=8)
    0x08, 0x01, 0x00,       // start function: index 0

    // Code section
    0x0A, 0x04, 0x01,
    0x02, 0x00, 0x0B,       // body: empty
  };
  // clang-format on

  WasmModuleInfo moduleInfo;
  ASSERT_TRUE(parseWasmBinary(binary, moduleInfo));
  ASSERT_TRUE(moduleInfo.startFunction.has_value());
  EXPECT_EQ(*moduleInfo.startFunction, 0u);
}

/// Build a Wasm binary with multiple types including f64 and multi-value.
TEST(BinaryReaderTest, MultipleTypes) {
  // clang-format off
  std::vector<uint8_t> binary = {
    // Header
    0x00, 0x61, 0x73, 0x6D,
    0x01, 0x00, 0x00, 0x00,

    // Type section (id=1, size=16)
    0x01, 0x10,
    0x03,                   // count: 3
    // Type 0: (i32) -> i32
    0x60, 0x01, 0x7F, 0x01, 0x7F,
    // Type 1: (f64, f64) -> f64
    0x60, 0x02, 0x7C, 0x7C, 0x01, 0x7C,
    // Type 2: (i64) -> void
    0x60, 0x01, 0x7E, 0x00,
  };
  // clang-format on

  WasmModuleInfo moduleInfo;
  ASSERT_TRUE(parseWasmBinary(binary, moduleInfo));

  ASSERT_EQ(moduleInfo.types.size(), 3u);

  // Type 0: (i32) -> i32
  EXPECT_EQ(moduleInfo.types[0].params.size(), 1u);
  EXPECT_EQ(moduleInfo.types[0].params[0], WasmValType::I32);
  EXPECT_EQ(moduleInfo.types[0].results.size(), 1u);
  EXPECT_EQ(moduleInfo.types[0].results[0], WasmValType::I32);

  // Type 1: (f64, f64) -> f64
  EXPECT_EQ(moduleInfo.types[1].params.size(), 2u);
  EXPECT_EQ(moduleInfo.types[1].params[0], WasmValType::F64);
  EXPECT_EQ(moduleInfo.types[1].params[1], WasmValType::F64);
  EXPECT_EQ(moduleInfo.types[1].results.size(), 1u);
  EXPECT_EQ(moduleInfo.types[1].results[0], WasmValType::F64);

  // Type 2: (i64) -> void
  EXPECT_EQ(moduleInfo.types[2].params.size(), 1u);
  EXPECT_EQ(moduleInfo.types[2].params[0], WasmValType::I64);
  EXPECT_EQ(moduleInfo.types[2].results.size(), 0u);
}

/// Test a table import.
TEST(BinaryReaderTest, TableImport) {
  // clang-format off
  std::vector<uint8_t> binary = {
    // Header
    0x00, 0x61, 0x73, 0x6D,
    0x01, 0x00, 0x00, 0x00,

    // Import section (id=2, size=11)
    0x02, 0x0B,
    0x01,                   // count: 1
    // Import: table "js"."t" funcref min=5, max=10
    0x02, 0x6A, 0x73,       // "js"
    0x01, 0x74,             // "t"
    0x01,                   // kind: table
    0x70,                   // funcref
    0x01,                   // has max
    0x05,                   // min: 5
    0x0A,                   // max: 10
  };
  // clang-format on

  WasmModuleInfo moduleInfo;
  ASSERT_TRUE(parseWasmBinary(binary, moduleInfo));

  ASSERT_EQ(moduleInfo.imports.size(), 1u);
  EXPECT_EQ(moduleInfo.imports[0].moduleName, "js");
  EXPECT_EQ(moduleInfo.imports[0].fieldName, "t");
  EXPECT_EQ(moduleInfo.imports[0].kind, WasmExternalKind::Table);
  EXPECT_EQ(moduleInfo.imports[0].tableType.elemType, WasmValType::FuncRef);
  EXPECT_EQ(moduleInfo.imports[0].tableType.limits.initial, 5u);
  EXPECT_TRUE(moduleInfo.imports[0].tableType.limits.hasMaximum);
  EXPECT_EQ(moduleInfo.imports[0].tableType.limits.maximum, 10u);

  EXPECT_EQ(moduleInfo.importedTableCount(), 1u);
  EXPECT_EQ(moduleInfo.totalTableCount(), 1u);
}

/// Test multiple exports of different kinds.
TEST(BinaryReaderTest, MultipleExports) {
  // clang-format off
  std::vector<uint8_t> binary = {
    // Header
    0x00, 0x61, 0x73, 0x6D,
    0x01, 0x00, 0x00, 0x00,

    // Type section (id=1, size=4)
    0x01, 0x04,
    0x01,                   // count: 1
    0x60, 0x00, 0x00,       // () -> void

    // Function section (id=3, size=2)
    0x03, 0x02,
    0x01, 0x00,             // count=1, type=0

    // Memory section (id=5, size=3)
    0x05, 0x03,
    0x01,                   // count: 1
    0x00, 0x01,             // no max, initial=1

    // Export section (id=7, size=9)
    0x07, 0x09,
    0x02,                   // count: 2
    // Export 0: function "f" index 0
    0x01, 0x66,             // "f"
    0x00, 0x00,             // kind=func, index=0
    // Export 1: memory "m" index 0
    0x01, 0x6D,             // "m"
    0x02, 0x00,             // kind=memory, index=0

    // Code section (id=10, size=4)
    0x0A, 0x04,
    0x01,                   // count: 1
    0x02,                   // body size: 2
    0x00,                   // local decl count: 0
    0x0B,                   // end
  };
  // clang-format on

  WasmModuleInfo moduleInfo;
  ASSERT_TRUE(parseWasmBinary(binary, moduleInfo));

  ASSERT_EQ(moduleInfo.exports.size(), 2u);
  EXPECT_EQ(moduleInfo.exports[0].name, "f");
  EXPECT_EQ(moduleInfo.exports[0].kind, WasmExternalKind::Function);
  EXPECT_EQ(moduleInfo.exports[1].name, "m");
  EXPECT_EQ(moduleInfo.exports[1].kind, WasmExternalKind::Memory);
}

// --- compileWasmModule tests ---

TEST(CompileWasmTest, ValidModule) {
  auto binary = buildMinimalWasm();
  auto context = std::make_shared<hermes::Context>();
  hermes::Module M(context);
  std::string errorMsg;
  EXPECT_TRUE(
      hermes::compileWasmModule(binary.data(), binary.size(), M, errorMsg));
  EXPECT_TRUE(errorMsg.empty());
}

TEST(CompileWasmTest, InvalidModule) {
  // Invalid magic bytes.
  std::vector<uint8_t> binary = {
      0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00};
  auto context = std::make_shared<hermes::Context>();
  hermes::Module M(context);
  std::string errorMsg;
  EXPECT_FALSE(
      hermes::compileWasmModule(binary.data(), binary.size(), M, errorMsg));
  EXPECT_FALSE(errorMsg.empty());
}

TEST(CompileWasmTest, EmptyBuffer) {
  auto context = std::make_shared<hermes::Context>();
  hermes::Module M(context);
  std::string errorMsg;
  EXPECT_FALSE(hermes::compileWasmModule(nullptr, 0, M, errorMsg));
  EXPECT_FALSE(errorMsg.empty());
}

} // namespace
