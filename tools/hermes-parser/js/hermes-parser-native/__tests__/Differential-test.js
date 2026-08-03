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
  // Both `new.target` and `import.meta` are also exercised above in a
  // position where they are early errors (Program top level), which only
  // proves the two parsers reject invalid input identically. These two
  // additional cases exercise the two MetaProperty forms actually
  // succeeding and producing a real MetaProperty node, which nothing else
  // in this list covers.
  'function f() { new.target; }',
  'import.meta;',
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
    expect(native).toStrictEqual(wasm);
  });

  test('matches with tokens enabled', () => {
    const source = 'const x = f(1, "two");';
    expect(parseNative(source, {tokens: true})).toStrictEqual(
      parseWasm(source, {tokens: true}),
    );
  });

  test('matches with comments present', () => {
    const source = '// leading\nconst x = 1; /* trailing */';
    expect(parseNative(source, {})).toStrictEqual(parseWasm(source, {}));
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

  // The CASES above all run with every addon flag false/default. These
  // exercise the remaining ParserOptions that pick different code paths in
  // the addon (detectFlow, allowReturnOutsideFunction, and the three
  // experimental-syntax flags) with a small, targeted input per option
  // rather than a combinatorial matrix.
  test('matches with flow: detect and an @flow pragma', () => {
    const source = '/* @flow */\ntype T = number;\nconst x: T = 1;';
    expect(parseNative(source, {flow: 'detect'})).toStrictEqual(
      parseWasm(source, {flow: 'detect'}),
    );
  });

  test('matches with allowReturnOutsideFunction', () => {
    const source = 'return 1;';
    expect(
      parseNative(source, {allowReturnOutsideFunction: true}),
    ).toStrictEqual(parseWasm(source, {allowReturnOutsideFunction: true}));
  });

  test('matches with experimental component syntax enabled', () => {
    const source = 'component Foo(bar: string) { return bar; }';
    expect(
      parseNative(source, {enableExperimentalComponentSyntax: true}),
    ).toStrictEqual(
      parseWasm(source, {enableExperimentalComponentSyntax: true}),
    );
  });

  test('matches with experimental flow match syntax enabled', () => {
    const source = 'const e = match (x) { 1 => 2 };';
    expect(
      parseNative(source, {enableExperimentalFlowMatchSyntax: true}),
    ).toStrictEqual(
      parseWasm(source, {enableExperimentalFlowMatchSyntax: true}),
    );
  });

  test('matches with experimental flow record syntax enabled', () => {
    const source = 'record R { x: number }';
    expect(
      parseNative(source, {enableExperimentalFlowRecordSyntax: true}),
    ).toStrictEqual(
      parseWasm(source, {enableExperimentalFlowRecordSyntax: true}),
    );
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
    // `files.length` only counts files discovered on disk, not files that
    // actually made it through a comparison below (the catch below skips
    // anything the wasm reference itself can't parse). Track how many were
    // actually compared and assert it against the discovered count, so this
    // test cannot quietly degrade into "compared nothing" while still
    // reporting green. As of writing, the reference parses all of them, so
    // this equality is not a hypothetical.
    let compared = 0;
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
      expect({file, ast: native}).toStrictEqual({file, ast: wasm});
      compared++;
    }
    expect(compared).toBe(files.length);
  });
});
