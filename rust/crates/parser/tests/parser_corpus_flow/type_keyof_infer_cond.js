/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

type A = keyof X;
type B = X extends Y ? A : B;
type C = X extends infer U ? U : never;
type D = X extends infer U extends V ? U : never;
type E = -3;
type F = -2n;
type G = infer U extends V ? A : B;
