/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

'use strict';

const {parse} = require('../src/HermesParser');

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
    const ast = parse('foo; foo;', {});
    const first = ast.body[0].expression.name;
    const second = ast.body[1].expression.name;
    expect(first).toBe('foo');
    expect(second).toBe('foo');
    // The interning property: the same JS string object, not just equal.
    expect(Object.is(first, second)).toBe(true);
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
