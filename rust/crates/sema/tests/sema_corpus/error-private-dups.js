/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// The rows of collectDeclaredPrivateIdentifiers's early-error matrix
// (SemanticResolver.cpp:2173-2290) that the imported
// private-declaration-dup-error.js does NOT cover. ES2024 15.7.1: declared
// private names may not contain duplicates, unless the name is used once for
// a getter and once for a setter, in no other entries, and both are static or
// both are not.
//
// Note the diagnostic locations: every "Duplicate private identifier
// declaration." points at the *Identifier* inside the private name (so at
// `a`, not at `#a` and not at `get`), because C++ reports
// id->getSourceRange() (cpp:2210/2235/2271) — the same node for all three
// element kinds.

// Method + method (cpp:2231-2238: no @Hermes.overload in untyped mode, so the
// second one is an error and its identifier is resolved to the first's decl).
class MethodDup {
  #a() {}
  #a() {}
}

// Setter + setter (cpp:2269-2270's `isSetter && existingInfo.isSetter` arm —
// the getter+getter counterpart of the imported file's DupAccessors).
class SetterDup {
  set #b(v) {}
  set #b(v) {}
}

// Accessor, then a field with the same name: the field branch (cpp:2208-2210)
// errors on ANY existing entry, accessor or not.
class AccessorThenField {
  get #c() {}
  #c;
}

// Method, then a field.
class MethodThenField {
  #d() {}
  #d;
}

// Accessor, then a method: the method branch (cpp:2233-2238) errors and then
// resolves the identifier onto the accessor's decl.
class AccessorThenMethod {
  set #e(v) {}
  #e() {}
}

// A complete getter+setter pair followed by a THIRD accessor: the pair is
// legal (it upgraded to PrivateGetterSetter), but the third entry now
// collides with both halves.
class PairPlusOne {
  get #f() {}
  set #f(v) {}
  get #f() {}
}

// The static-mismatch rule (cpp:2274-2279), in the opposite order from the
// imported file's: a static getter first, then a non-static setter.
class StaticMismatch {
  static get #g() {}
  set #g(v) {}
}

// A static getter + static setter pair IS legal (both static) — included
// here as the negative control for the row above; it produces no diagnostic,
// and its decl still upgrades to PrivateGetterSetter with PrivateStatic.
class StaticPairOk {
  static get #h() {}
  static set #h(v) {}
}
