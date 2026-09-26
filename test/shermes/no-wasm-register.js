/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// --exported-unit also serves ordinary JS units, which must NOT acquire a
// Wasm registration. Without this, every exported JS unit would advertise
// itself to WebAssembly.Module.fromNativeUnit().

// REQUIRES: shermes
// RUN: %shermes -exported-unit=plainjs -emit-c -o - %s | %FileCheck %s

print('hi');

// CHECK-NOT: _sh_wasm_register_unit
