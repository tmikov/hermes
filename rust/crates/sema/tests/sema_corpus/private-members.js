/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Every legal private-name shape collectDeclaredPrivateIdentifiers
// (SemanticResolver.cpp:2157-2274) declares, and every way the rest of the
// resolver reaches one:
//   - declarePrivateName (cpp:2047-2064): the '#'-prefixed Decl name comes
//     from Context::getPrivateNameIdentifier, and `isStatic` becomes
//     Decl::Special::PrivateStatic (methods/accessors only).
//   - the five private Decl kinds: PrivateField, PrivateMethod,
//     PrivateGetter, PrivateSetter and the PrivateGetterSetter a legal
//     getter+setter pair upgrades to (cpp:2267-2271), where BOTH accessors'
//     identifiers bind to the SAME decl.
//   - resolvePrivateName (cpp:2066-2080) through visit(PrivateNameNode *)
//     (cpp:952-963) and through the MemberExpression branches
//     (cpp:1221-1309).
//   - visit(ClassPrivatePropertyNode *) (cpp:965-1011): a private field's
//     initializer runs in the synthetic elements-init function, so
//     `arguments` is declared in the class scope for it.

class A {
  // Fields, with and without an initializer, instance and static. Each
  // creates (or reuses) an elements-init FunctionInfo even with no value.
  #f1;
  #f2 = 1;
  static #s1;
  static #s2 = 2;

  // Methods: instance and static, plus generator/async bodies.
  #m1() {}
  static #m2() {}
  *#m3() {}
  async #m4() {}

  // A legal getter+setter pair: one PrivateGetterSetter decl, referenced by
  // both identifiers. Declared getter-first here...
  get #both1() {}
  set #both1(v) {}
  // ...and setter-first here, so both upgrade orders are pinned.
  set #both2(v) {}
  get #both2() {}
  // A legal static pair (same static-ness on both halves).
  static get #bothStatic() {}
  static set #bothStatic(v) {}

  // Accessors that stay one-sided.
  get #onlyGetter() {}
  set #onlySetter(v) {}
  // ...and their STATIC forms, which are the only way to reach the
  // `PrivateGetter PrivateStatic` / `PrivateSetter PrivateStatic` decl+special
  // pairs: `declarePrivateName(id, kind, method->_static)` (cpp:2240-2243 →
  // 2047-2058) sets `Decl::Special::PrivateStatic` for every static private
  // method or accessor, but a one-sided static accessor is neither upgraded to
  // `PrivateGetterSetter` (the static pair above) nor a `PrivateMethod`. The
  // S2 T8 sweep inventoried every decl-kind/special pair in the corpus dumps
  // and found these two missing.
  static get #onlyGetterStatic() {}
  static set #onlySetterStatic(v) {}
}

// Private names resolve anywhere inside the class, including in members
// declared BEFORE the one they name (the whole point of running
// collectDeclaredPrivateIdentifiers before the body walk).
class Uses {
  early = this.#late;
  m(o) {
    this.#late;
    o.#late;
    o?.#late;
    // ES2022 private-name `in` check: a PrivateName as a BinaryExpression
    // operand, which reaches visit(PrivateNameNode *) directly rather than
    // through a member expression.
    return #late in o;
  }
  get #late() {}
  set #late(v) {}
  static #sf = 3;
  static sm(o) {
    return o.#sf;
  }
}

// A private field initializer is resolved in the synthetic elements-init
// function, so `this` and a nested arrow behave exactly as in a public one.
class Init {
  #a = this;
  #b = () => this.#a;
  static #c = Init;
  // Initializers that FOLD, rebuilding the ClassPrivateProperty (and hence
  // the ClassBody and the class node) — the dump shows the folded literal.
  #d = 1 + 2;
  static #e = 3 + 4;
}

// Nested classes each get their own private-name scope; the inner class can
// see the outer class's names, and identical spellings in the two classes
// are distinct decls.
class Outer {
  #x = 1;
  m() {
    class Inner {
      #x = 2;
      n(o) {
        return o.#x;
      }
    }
    return this.#x;
  }
}

// A private name in a derived class, plus the class-expression form.
class Base {}
class Derived extends Base {
  #d;
  m() {
    return this.#d;
  }
}
var CE = class {
  #e;
  m() {
    return this.#e;
  }
};
