/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Every LoopStatement shape: the scope each one creates (or doesn't) and the
// declarations hoisted into it.

while (cond) {
  var w = 1;
}

do {
  let d = 2;
} while (cond);

for (var i = 0, j = 1; i < 10; ++i) {
  let inner = i;
}

for (let k = 0; k < 10; ++k) ;

for (;;) ;

for (a in obj) ;

for (const p in obj) {
  let q = p;
}

for (b of iter) ;

for (let e of iter) {
  const f = e;
}

function inFunction(o) {
  for (let x in o) {
    for (let y of o) {
      var z = 1;
    }
  }
}
