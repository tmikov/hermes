/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

#include "hermes/WasmFrontend/WasmTypes.h"

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

} // namespace
