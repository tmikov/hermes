/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

type A = typeof x;
type B = typeof x.y;
type C = typeof (x);
type D = typeof x<Y>;
type E = typeof ((x));
type N3 = typeof x<A<B<C>>>;
