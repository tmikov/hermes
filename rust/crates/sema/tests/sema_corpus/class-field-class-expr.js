/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %shermes -dump-sema -fno-std-globals %s | %FileCheckOrRegen %s --match-full-lines

// Regression test: a class expression in a field initializer creates a scope
// in the synthesized initializer function, but it used to be parented in the
// enclosing class scope, which belongs to the outer function. The dumper's
// scope walk then failed to visit it and asserted. The scope must be parented
// in the initializer function's body scope.

class C {
  x = class {};
  static y = class {};
}

// Auto-generated content below. Please do not modify manually.

// CHECK:SemContext
// CHECK-NEXT:Func loose
// CHECK-NEXT:    Scope %s.1
// CHECK-NEXT:        Decl %d.1 'C' Class
// CHECK-NEXT:        Scope %s.2
// CHECK-NEXT:            Decl %d.2 'C' ClassExprName
// CHECK-NEXT:    Func strict
// CHECK-NEXT:        Scope %s.3
// CHECK-NEXT:            Decl %d.3 'arguments' Var Arguments
// CHECK-NEXT:            Scope %s.4
// CHECK-NEXT:        Func strict
// CHECK-NEXT:            Scope %s.5
// CHECK-NEXT:    Func strict
// CHECK-NEXT:        Scope %s.6
// CHECK-NEXT:            Decl %d.4 'arguments' Var Arguments
// CHECK-NEXT:            Scope %s.7
// CHECK-NEXT:        Func strict
// CHECK-NEXT:            Scope %s.8
// CHECK-NEXT:    Func strict
// CHECK-NEXT:        Scope %s.9

// CHECK:Program Scope %s.1
// CHECK-NEXT:    ClassDeclaration Scope %s.2
// CHECK-NEXT:        Id 'C' [D:%d.1 E:%d.2 'C']
// CHECK-NEXT:        ClassBody
// CHECK-NEXT:            ClassProperty
// CHECK-NEXT:                Id 'x'
// CHECK-NEXT:                ClassExpression Scope %s.4
// CHECK-NEXT:                    ClassBody
// CHECK-NEXT:            ClassProperty
// CHECK-NEXT:                Id 'y'
// CHECK-NEXT:                ClassExpression Scope %s.7
// CHECK-NEXT:                    ClassBody
