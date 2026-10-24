/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: cat %S/Inputs/strict.js %intl_js/01-intrinsics.js %intl_js/02-shapes.js %intl_js/03-locales.js %intl_js/04-options.js %s > %t.js
// RUN: %hermes_default -Xintl-plugin=false %t.js | %FileCheck --match-full-lines %s
// RUN: %hermesc -O -emit-binary -out=%t.hbc %t.js && %hermes_default -Xintl-plugin=false -b %t.hbc | %FileCheck --match-full-lines %s

// Unicode locale extension parsing and insertion (ECMA-402 9.2.2, 9.2.5,
// 9.2.6) in isolation, with stub natives.

// Stubs for what the plugin's C++ passes to setup.
var available = ['en', 'en-US', 'de', 'fr'];
var native = {
  canonicalizeLocale: function (tag) {
    if (!/^[a-z]{2,3}(-[A-Za-z0-9]{1,8})*$/.test(tag)) return undefined;
    // One UTS 35 value alias, so that callers which read a value back from
    // the canonical tag see it change.
    return tag.replace(/-u-ca-islamicc(?=-|$)/, '-u-ca-islamic-civil');
  },
  isAvailableLocale: function (tag) {
    return available.indexOf(tag) !== -1;
  },
};
var hostDefaults = {locales: ['fr'], timeZone: 'UTC'};
/// The class of the exception f throws, or 'none'.
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}

function split(l) {
  var r = splitUnicodeExtension(l);
  return r.base + ' | ' + r.extension;
}
print(split('en'));
// CHECK: en | undefined
print(split('en-u-ca-gregory'));
// CHECK-NEXT: en | -u-ca-gregory
print(split('de-a-foo-u-co-phonebk-x-u-bar'));
// CHECK-NEXT: de-a-foo-x-u-bar | -u-co-phonebk
print(split('en-x-u-ca'));
// CHECK-NEXT: en-x-u-ca | undefined

function components(e) {
  var c = UnicodeExtensionComponents(e);
  var ks = [];
  for (var i = 0; i < c.keywords.length; i++)
    ks.push(c.keywords[i].key + '=' + c.keywords[i].value);
  return c.attributes.join(',') + ' | ' + ks.join(',');
}
print(components('-u-attr1-attr2-ca-islamic-civil-kn'));
// CHECK-NEXT: attr1,attr2 | ca=islamic-civil,kn=
print(components('-u-ca-gregory-ca-buddhist'));
// CHECK-NEXT:  | ca=gregory

print(InsertUnicodeExtensionAndCanonicalize('en', [], [{key: 'ca', value: 'gregory'}, {key: 'kn', value: ''}]));
// CHECK-NEXT: en-u-ca-gregory-kn
print(InsertUnicodeExtensionAndCanonicalize('en-x-priv', ['a1b'], [{key: 'nu', value: 'thai'}]));
// CHECK-NEXT: en-u-a1b-nu-thai-x-priv
print(InsertUnicodeExtensionAndCanonicalize('en', [], []));
// CHECK-NEXT: en

print(JSON.stringify(CanonicalizeUValue('ca', 'GREGORY')));
// CHECK-NEXT: "gregory"
// The value is read back from the canonical tag, so aliases apply.
print(JSON.stringify(CanonicalizeUValue('ca', 'IslamicC')));
// CHECK-NEXT: "islamic-civil"
// A canonicalizer that drops the extension leaves the lowercased value.
var savedNative = native;
native = {
  canonicalizeLocale: function (tag) { return 'und'; },
  isAvailableLocale: savedNative.isAvailableLocale,
};
print(JSON.stringify(CanonicalizeUValue('ca', 'GREGORY')));
// CHECK-NEXT: "gregory"
native = savedNative;
