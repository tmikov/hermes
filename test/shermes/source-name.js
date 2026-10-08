/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %shermes -source-name=my/module.js -exec %s -g1 | %FileCheck --match-full-lines %s
// RUN: (! %shermes -source-name=x.js -emit-c -o %t.c %s %s 2>&1) | %FileCheck --check-prefix=MULTI %s
// RUN: echo '//# sourceURL=real.js' > %t.url.js
// RUN: echo 'function f() { throw new Error("x"); }' >> %t.url.js
// RUN: echo 'try { f(); } catch (e) { print(e.stack.split("\n")[1].trim()); }' >> %t.url.js
// RUN: %shermes -source-name=staged.js -exec %t.url.js -g1 | %FileCheck --check-prefix=URL %s

function inner() {
  throw new Error('boom');
}
try {
  inner();
} catch (e) {
  print(e.stack.split('\n')[1].trim());
}
// CHECK: at inner (my/module.js:{{[0-9]+}}:{{[0-9]+}})

// MULTI: -source-name can only be used with a single input file

// URL: at f (real.js:{{[0-9]+}}:{{[0-9]+}})
