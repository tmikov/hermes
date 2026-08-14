/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

type A = [X, Y];
type B = [a: X, b?: Y];
type C = [X, ...Y];
type D = [...rest: Y];
type E = [+a: X, -b: Y];
type F = [X, ...];
type G = [];
type H = [readonly a: X, writeonly b: Y];
type I = [readonly: X];
