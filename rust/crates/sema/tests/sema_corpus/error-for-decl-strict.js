/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// A `var` declared in a ForStatement's init still hoists to the function
// scope, so `validateDeclarationName` rejects the strict-mode reserved names
// there too. (Extracted from `test/Sema/invalid-args-eval.js`, which as a
// whole cannot be imported — see MANIFEST.md.)

"use strict";

for (var arguments = 0; arguments < 10; ) {}
