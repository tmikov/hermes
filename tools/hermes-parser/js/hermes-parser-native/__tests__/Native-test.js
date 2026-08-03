/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

'use strict';

// Wrap the real UTF-8 decoder in a jest.fn() so the "interns repeated
// identifiers" test below can observe how many times it actually runs,
// rather than only inspecting the (necessarily equal, even without any
// caching) decoded values. See that test for why this matters.
jest.mock('../src/HermesParserDecodeUTF8String', () => {
  const actual = jest.requireActual('../src/HermesParserDecodeUTF8String');
  return {__esModule: true, default: jest.fn(actual.default)};
});

const {parse} = require('../src/HermesParser');
const HermesParserDecodeUTF8String = require(
  '../src/HermesParserDecodeUTF8String',
).default;

describe('hermes-parser-native', () => {
  test('parses a variable declaration', () => {
    const ast = parse('var x = 1;', {});
    expect(ast.type).toBe('Program');
    expect(ast.body).toHaveLength(1);
    expect(ast.body[0].type).toBe('VariableDeclaration');
    expect(ast.body[0].declarations[0].id.name).toBe('x');
    expect(ast.body[0].declarations[0].init.value).toBe(1);
  });

  test('interns repeated identifiers to one string object', () => {
    const ast = parse('foo; foo; foo;', {});
    const names = ast.body.map(statement => statement.expression.name);
    expect(names).toEqual(['foo', 'foo', 'foo']);

    // `Object.is` on two independently-decoded primitive strings with the
    // same value is also `true` (value equality, not identity), so it
    // cannot by itself distinguish a real cache from no cache at all. What
    // actually proves the decode cache in HermesParserDeserializer.getString
    // is working is that the underlying decoder ran exactly once for "foo"
    // even though it is referenced 3 times: every reference after the first
    // must have been served from the cache instead of re-decoding.
    const fooDecodeCount = HermesParserDecodeUTF8String.mock.results.filter(
      result => result.type === 'return' && result.value === 'foo',
    ).length;
    expect(fooDecodeCount).toBe(1);
  });

  test('throws a SyntaxError with loc', () => {
    let err = null;
    try {
      parse('var = ;', {});
    } catch (e) {
      err = e;
    }
    expect(err).toBeInstanceOf(SyntaxError);
    expect(typeof err.loc.line).toBe('number');
    expect(typeof err.loc.column).toBe('number');
  });
});
