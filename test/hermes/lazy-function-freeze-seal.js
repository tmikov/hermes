/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -O -target=HBC %s | %FileCheck --match-full-lines %s
// RUN: %hermes -O -target=HBC -emit-binary -out %t.hbc %s && %hermes %t.hbc | %FileCheck --match-full-lines %s
// RUN: %shermes -exec %s | %FileCheck --match-full-lines %s
// XFAIL: *

// Currently failing: Object.freeze()/Object.seal() mark the function object
// before its lazy "length", "name" and "prototype" properties have been
// materialized, so those properties appear afterwards with their default
// attributes. See dz issue 01a06a36.

// Regression test: Object.freeze() and Object.seal() on lazy function objects
// must materialize lazy properties (length, name, prototype) with the correct
// attributes. The bug was that freeze/seal would mark the object before lazy
// properties were materialized, so they'd later appear with their default
// (configurable) attributes on an object that claimed to be frozen/sealed.

"use strict";

function mustThrow(f) {
    try {
        f();
    } catch (e) {
        print("caught", e.name, e.message);
        return;
    }
    print("DID NOT THROW");
}

//
// 1. Object.freeze() on a regular function (has prototype property).
//
print("freeze regular function");
// CHECK-LABEL: freeze regular function
(function() {
    function foo(a, b, c) {}
    Object.freeze(foo);

    print(Object.isFrozen(foo));
    // CHECK-NEXT: true

    // length: should be non-configurable and non-writable.
    var ld = Object.getOwnPropertyDescriptor(foo, 'length');
    print('length', ld.value, ld.configurable, ld.writable);
    // CHECK-NEXT: length 3 false false

    // name: should be non-configurable and non-writable.
    var nd = Object.getOwnPropertyDescriptor(foo, 'name');
    print('name', nd.value, nd.configurable, nd.writable);
    // CHECK-NEXT: name foo false false

    // prototype: should be non-configurable and non-writable.
    var pd = Object.getOwnPropertyDescriptor(foo, 'prototype');
    print('prototype', pd.configurable, pd.writable);
    // CHECK-NEXT: prototype false false

    // delete on frozen properties must throw.
    mustThrow(function() { delete foo.length; });
    // CHECK-NEXT: caught TypeError {{.*}}
    mustThrow(function() { delete foo.name; });
    // CHECK-NEXT: caught TypeError {{.*}}
    mustThrow(function() { delete foo.prototype; });
    // CHECK-NEXT: caught TypeError {{.*}}

    // Assignment to frozen properties must throw.
    mustThrow(function() { foo.length = 99; });
    // CHECK-NEXT: caught TypeError {{.*}}
    mustThrow(function() { foo.name = 'bar'; });
    // CHECK-NEXT: caught TypeError {{.*}}

    // Adding new properties must throw.
    mustThrow(function() { foo.newProp = 1; });
    // CHECK-NEXT: caught TypeError {{.*}}
})();

//
// 2. Object.freeze() on an arrow function (no prototype property).
//
print("freeze arrow function");
// CHECK-LABEL: freeze arrow function
(function() {
    var arrow = (x, y) => x + y;
    Object.freeze(arrow);

    print(Object.isFrozen(arrow));
    // CHECK-NEXT: true

    var ld = Object.getOwnPropertyDescriptor(arrow, 'length');
    print('length', ld.value, ld.configurable, ld.writable);
    // CHECK-NEXT: length 2 false false

    var nd = Object.getOwnPropertyDescriptor(arrow, 'name');
    print('name', nd.value, nd.configurable, nd.writable);
    // CHECK-NEXT: name arrow false false

    // Arrow functions have no prototype property.
    print('has prototype', arrow.hasOwnProperty('prototype'));
    // CHECK-NEXT: has prototype false

    mustThrow(function() { delete arrow.length; });
    // CHECK-NEXT: caught TypeError {{.*}}
    mustThrow(function() { arrow.newProp = 1; });
    // CHECK-NEXT: caught TypeError {{.*}}
})();

//
// 3. Object.seal() on a regular function.
//    seal makes properties non-configurable but does NOT change writable.
//    Function length/name default to writable:false, so they stay non-writable.
//    Function prototype defaults to writable:true, so it stays writable.
//
print("seal regular function");
// CHECK-LABEL: seal regular function
(function() {
    function bar(a) {}
    Object.seal(bar);

    print(Object.isSealed(bar));
    // CHECK-NEXT: true

    // length: non-configurable, writable unchanged (false).
    var ld = Object.getOwnPropertyDescriptor(bar, 'length');
    print('length', ld.value, ld.configurable, ld.writable);
    // CHECK-NEXT: length 1 false false

    // name: non-configurable, writable unchanged (false).
    var nd = Object.getOwnPropertyDescriptor(bar, 'name');
    print('name', nd.value, nd.configurable, nd.writable);
    // CHECK-NEXT: name bar false false

    // prototype: non-configurable, but writable should remain true.
    var pd = Object.getOwnPropertyDescriptor(bar, 'prototype');
    print('prototype', pd.configurable, pd.writable);
    // CHECK-NEXT: prototype false true

    // delete on sealed properties must throw.
    mustThrow(function() { delete bar.length; });
    // CHECK-NEXT: caught TypeError {{.*}}
    mustThrow(function() { delete bar.name; });
    // CHECK-NEXT: caught TypeError {{.*}}
    mustThrow(function() { delete bar.prototype; });
    // CHECK-NEXT: caught TypeError {{.*}}

    // Writing to writable sealed property (prototype) should succeed.
    bar.prototype = {x: 1};
    print('prototype written', bar.prototype.x);
    // CHECK-NEXT: prototype written 1

    // Adding new properties must throw.
    mustThrow(function() { bar.newProp = 1; });
    // CHECK-NEXT: caught TypeError {{.*}}
})();

//
// 4. Object.seal() on an arrow function.
//
print("seal arrow function");
// CHECK-LABEL: seal arrow function
(function() {
    var arrow = (a, b, c, d) => {};
    Object.seal(arrow);

    print(Object.isSealed(arrow));
    // CHECK-NEXT: true

    var ld = Object.getOwnPropertyDescriptor(arrow, 'length');
    print('length', ld.value, ld.configurable, ld.writable);
    // CHECK-NEXT: length 4 false false

    var nd = Object.getOwnPropertyDescriptor(arrow, 'name');
    print('name', nd.value, nd.configurable, nd.writable);
    // CHECK-NEXT: name arrow false false

    mustThrow(function() { delete arrow.length; });
    // CHECK-NEXT: caught TypeError {{.*}}
})();

//
// 5. Object.preventExtensions() on a lazy function should be fine:
//    properties materialize with their normal attributes.
//
print("preventExtensions regular function");
// CHECK-LABEL: preventExtensions regular function
(function() {
    function baz(a, b) {}
    Object.preventExtensions(baz);

    print(Object.isExtensible(baz));
    // CHECK-NEXT: false

    // Properties should exist with their normal (configurable) attributes.
    var ld = Object.getOwnPropertyDescriptor(baz, 'length');
    print('length', ld.value, ld.configurable, ld.writable);
    // CHECK-NEXT: length 2 true false

    var nd = Object.getOwnPropertyDescriptor(baz, 'name');
    print('name', nd.value, nd.configurable, nd.writable);
    // CHECK-NEXT: name baz true false

    var pd = Object.getOwnPropertyDescriptor(baz, 'prototype');
    print('prototype', pd.configurable, pd.writable);
    // CHECK-NEXT: prototype false true

    // Can still delete configurable properties.
    delete baz.length;
    print('length deleted', baz.hasOwnProperty('length'));
    // CHECK-NEXT: length deleted false

    // But cannot add new properties.
    mustThrow(function() { baz.newProp = 1; });
    // CHECK-NEXT: caught TypeError {{.*}}
})();
