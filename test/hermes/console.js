/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The hermes CLI and shermes provide separate console implementations, so run
// both and require identical behaviour from each.

// RUN: %hermes %s 2>/dev/null | %FileCheck --match-full-lines --check-prefix=OUT %s
// RUN: %hermes %s 2>&1 >/dev/null | %FileCheck --match-full-lines --check-prefix=ERR %s
// RUN: %shermes -exec %s 2>/dev/null | %FileCheck --match-full-lines --check-prefix=OUT %s
// RUN: %shermes -exec %s 2>&1 >/dev/null | %FileCheck --match-full-lines --check-prefix=ERR %s

// log, info and debug write to stdout, like print().
console.log('hello');
// OUT: hello
console.log('log', 1, true);
// OUT-NEXT: log 1 true
console.info('info');
// OUT-NEXT: info
console.debug('debug');
// OUT-NEXT: debug

// warn and error write to stderr.
console.warn('warn', 2);
// ERR: warn 2
console.error('error');
// ERR-NEXT: error

// assert reports only when its condition is falsy. The condition uses
// ToBoolean, so it is not restricted to booleans.
console.assert(true, 'not reported');
console.assert(1 === 1, 'not reported either');
console.assert('non-empty', 'not reported either');
console.assert({}, 'objects are truthy');
console.assert(false);
// ERR-NEXT: Assertion failed
console.assert(false, 'with', 'detail');
// ERR-NEXT: Assertion failed: with detail
console.assert(0, 'falsy number');
// ERR-NEXT: Assertion failed: falsy number
console.assert(NaN, 'NaN is falsy');
// ERR-NEXT: Assertion failed: NaN is falsy
console.assert('', 'empty string is falsy');
// ERR-NEXT: Assertion failed: empty string is falsy
console.assert(null, 'null is falsy');
// ERR-NEXT: Assertion failed: null is falsy
console.assert(undefined, 'undefined is falsy');
// ERR-NEXT: Assertion failed: undefined is falsy
console.assert(0n, 'zero bigint is falsy');
// ERR-NEXT: Assertion failed: zero bigint is falsy
console.assert(1n, 'non-zero bigint is truthy');

// Arguments are stringified the same way print() does.
console.error({}, [1, 2], null, undefined);
// ERR-NEXT: [object Object] 1,2 null undefined
