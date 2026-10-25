/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Helpers the number-format*.js tests share: their RUN lines put this file
// in front of the test.

/// s with the invisible and space characters the patterns use (U+00A0,
/// U+061C, U+2000 to U+200F, U+202F) written as \uXXXX escapes, so the
/// checks can show them.
function escape(s) {
  var out = '';
  for (var i = 0; i < s.length; i++) {
    var c = s.charCodeAt(i);
    if (c === 0xa0 || c === 0x61c || (c >= 0x2000 && c <= 0x200f) || c === 0x202f)
      out += '\\u' + c.toString(16).padStart(4, '0');
    else out += s[i];
  }
  return out;
}

/// JSON.stringify(v), escaped.
function stringify(v) {
  return escape(JSON.stringify(v));
}
