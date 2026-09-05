/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// `--wasm` on a .hbc asserts that the bytecode came from `hermesc --wasm`.
// There is no marker in the file to check, so the claim is only tested when
// the driver goes to look for the {instantiate, exportDescs, importDescs}
// module object the top level should have returned. Ordinary JS bytecode
// returns something else -- here `undefined`, the completion value of a
// `print` statement -- and that must be reported plainly and exit non-zero
// rather than being silently ignored.
//
// The script's own output is checked too: it proves the bytecode really did
// run and that the diagnosis is about its result, not about failing to load.

// REQUIRES: wasm

// RUN: %hermesc -emit-binary -out %t.hbc %s
// RUN: (! %hermes --wasm %t.hbc 2>&1) | %FileCheck --match-full-lines %s

print('ordinary JS ran');

// CHECK: ordinary JS ran
// CHECK-NEXT: Error: --wasm was specified, but the bytecode did not return a WebAssembly module object with a callable instantiate property.
// CHECK-NEXT: The input is not a WebAssembly module, nor bytecode compiled from one.
