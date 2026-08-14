/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

type A = Foo;
type B = Foo<X>;
type C = Foo.Bar<X, Y>;
type D = Foo<>;
type E = Foo.Bar.Baz;
type F = Foo.if.else;
type G = this;
type H = static;
type N1 = Foo<Bar<U>>;
type N2 = Foo<Bar<Baz<U>>>;
