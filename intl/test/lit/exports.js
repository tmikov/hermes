/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The plugin exports exactly one defined global symbol, its entry point:
// nothing from the Rust core and no C++ inline or template instances. A
// Windows DLL's export table is listed by name (Inputs/coff-exports.py).
// RUN: %list_exports %intl_plugin > %t.syms
// RUN: count 1 < %t.syms
// RUN: %FileCheck --match-full-lines --check-prefix=%exports_prefix -DENTRY=%intl_entry_symbol %s < %t.syms
// NM: {{[0-9a-f]+}} T [[ENTRY]]
// COFF: [[ENTRY]]
