/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

'use strict';

const {parse: parseNative} = require('../src/HermesParser');
const {parse: parseWasm} = require('hermes-parser/dist/HermesParser');

const CASES = [
  'var x = 1;',
  'function f(a, b) { return a + b; }',
  'class C extends D { #p = 1; static m() {} }',
  'const {a, b: [c], ...rest} = obj;',
  'async function* g() { yield* await x; }',
  'type T = {a: number, b?: string};',
  'function f(x: number): string { return String(x); }',
  'const x: Array<?string> = [];',
  'interface I { m(): void }',
  'enum E { A, B }',
  '<div a="1" {...p}>{x}</div>;',
  'a?.b?.[c]?.();',
  'x ??= 1; y ||= 2; z &&= 3;',
  '`a${b}c${d}e`;',
  'label: for (const x of xs) { continue label; }',
  'try { f(); } catch { g(); } finally { h(); }',
  '/regex/gimsuy.test(s);',
  'const big = 1234567890123456789012345678901234567890n;',
  'const nums = [1, 1.5, .5, 1e10, 0x10, 0b11, 0o17, NaN, Infinity];',
  'const uni = {"\\u00e9": "caf\\u00e9", "emoji": "\\ud83d\\ude00"};',
  'export default function () {}',
  'export * as ns from "mod"; import x, {y as z} from "mod";',
  '"use strict"; with2 = 1;',
  'new.target; import.meta;',
  'const s = "line1\\nline2\\ttab\\\\back";',
];

// Parses with the given function, capturing either the resulting AST or the
// thrown SyntaxError's message/location. This lets test.each below compare
// outcomes even for CASES entries that turn out to be invalid JS (both
// parsers must then throw identically) instead of letting an uncaught
// exception from one side abort the test before the other side even runs.
function tryParse(parseFn, source, options) {
  try {
    return {ast: parseFn(source, options)};
  } catch (e) {
    return {error: {message: e.message, loc: e.loc}};
  }
}

describe('native parser matches wasm parser', () => {
  test.each(CASES)('%s', source => {
    const native = tryParse(parseNative, source, {});
    const wasm = tryParse(parseWasm, source, {});
    expect(native).toEqual(wasm);
  });

  test('matches with tokens enabled', () => {
    const source = 'const x = f(1, "two");';
    expect(parseNative(source, {tokens: true})).toEqual(
      parseWasm(source, {tokens: true}),
    );
  });

  test('matches with comments present', () => {
    const source = '// leading\nconst x = 1; /* trailing */';
    expect(parseNative(source, {})).toEqual(parseWasm(source, {}));
  });

  test('matches on syntax errors', () => {
    const source = 'var = ;';
    let nativeErr = null;
    let wasmErr = null;
    try {
      parseNative(source, {});
    } catch (e) {
      nativeErr = e;
    }
    try {
      parseWasm(source, {});
    } catch (e) {
      wasmErr = e;
    }
    expect(nativeErr).not.toBeNull();
    expect(wasmErr).not.toBeNull();
    expect(nativeErr.message).toBe(wasmErr.message);
    expect(nativeErr.loc).toEqual(wasmErr.loc);
    expect(nativeErr).toBeInstanceOf(SyntaxError);
  });
});

describe('bulk corpus', () => {
  const fs = require('fs');
  const path = require('path');

  // Beyond this package's own sources, also walk every sibling package's
  // hand-written `src/` in the workspace. This turns the corpus from ~30
  // self-referential files into ~180 real files covering a much wider mix
  // of syntax (JSX, Flow generics, decorators, generators, ESLint rule
  // bodies, etc.), which is a meaningfully stronger differential signal
  // than testing this package against only its own code.
  const roots = [
    '../src',
    '../../hermes-parser/src',
    '../../hermes-transform/src',
    '../../babel-plugin-syntax-hermes-parser/src',
    '../../flow-api-translator/src',
    '../../hermes-eslint/src',
    '../../hermes-estree/src',
  ].map(root => path.resolve(__dirname, root));

  const files = [];
  for (const root of roots) {
    const walk = dir => {
      for (const entry of fs.readdirSync(dir, {withFileTypes: true})) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) {
          walk(full);
        } else if (entry.name.endsWith('.js')) {
          files.push(full);
        }
      }
    };
    walk(root);
  }

  test('parses every source file identically', () => {
    expect(files.length).toBeGreaterThan(10);
    for (const file of files) {
      const source = fs.readFileSync(file, 'utf8');
      let native;
      let wasm;
      try {
        wasm = parseWasm(source, {});
      } catch (e) {
        continue; // Skip anything the reference cannot parse.
      }
      native = parseNative(source, {});
      expect({file, ast: native}).toEqual({file, ast: wasm});
    }
  });
});
