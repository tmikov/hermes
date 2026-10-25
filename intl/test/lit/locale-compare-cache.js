/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes -Xintl-default-locale=en-US %s | %FileCheck --match-full-lines %s

// localeCompare with locales or options shares one internal collator among
// the calls that read the same locales and option values. Every call still
// performs every observable step of Construct(%Intl.Collator%, « locales,
// options »), and the shared collators compare exactly as new
// Intl.Collator(locales, options) does, however many option sets a program
// uses. Canonicalizing a language tag is remembered too.

// Every call reads the locales and options anew, in the draft's order:
// CanonicalizeLocaleList (length, HasProperty "0", Get "0", ToString of the
// element), usage, then ResolveOptions (localeMatcher, then the resolution
// option descriptors collation, numeric and caseFirst), then sensitivity
// and ignorePunctuation. Each string option is an object, so its
// conversion by ToString shows up in the log too.
var log = [];
/// An object whose toString logs name and returns value.
function stringy(name, value) {
  return {
    toString() {
      log.push(name + '.toString');
      return value;
    },
  };
}
var optionValues = {
  usage: stringy('usage', 'sort'),
  localeMatcher: stringy('localeMatcher', 'lookup'),
  collation: stringy('collation', 'phonebk'),
  numeric: true,
  caseFirst: stringy('caseFirst', 'upper'),
  sensitivity: stringy('sensitivity', 'base'),
  ignorePunctuation: false,
};
var loggingOptions = new Proxy(optionValues, {
  get(target, key) {
    log.push('get ' + String(key));
    return target[key];
  },
});
var loggingLocales = new Proxy([stringy('locale', 'de')], {
  get(target, key) {
    log.push('get ' + String(key));
    return target[key];
  },
  has(target, key) {
    log.push('has ' + String(key));
    return key in target;
  },
});
for (var call = 0; call < 2; call++) {
  log = [];
  var result = 'ä'.localeCompare('ae', loggingLocales, loggingOptions);
  print(result, log.join(', '));
}
// CHECK: 0 get length, has 0, get 0, locale.toString, get usage, usage.toString, get localeMatcher, localeMatcher.toString, get collation, collation.toString, get numeric, get caseFirst, caseFirst.toString, get sensitivity, sensitivity.toString, get ignorePunctuation
// CHECK-NEXT: 0 get length, has 0, get 0, locale.toString, get usage, usage.toString, get localeMatcher, localeMatcher.toString, get collation, collation.toString, get numeric, get caseFirst, caseFirst.toString, get sensitivity, sensitivity.toString, get ignorePunctuation
// The constructor reads the same, in the same order.
log = [];
new Intl.Collator(loggingLocales, loggingOptions);
print(log.join(', '));
// CHECK-NEXT: get length, has 0, get 0, locale.toString, get usage, usage.toString, get localeMatcher, localeMatcher.toString, get collation, collation.toString, get numeric, get caseFirst, caseFirst.toString, get sensitivity, sensitivity.toString, get ignorePunctuation

/// The name of the error f throws, or "none".
function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.name;
  }
}
// An option that becomes invalid after a call that succeeded still throws,
// and so does a locale.
var reads = 0;
var changing = {
  get sensitivity() {
    return reads++ === 0 ? 'base' : 'bogus';
  },
};
print(thrown(() => 'a'.localeCompare('b', 'de', changing)), thrown(() => 'a'.localeCompare('b', 'de', changing)));
// CHECK-NEXT: none RangeError
var tags = ['de'];
print(thrown(() => 'a'.localeCompare('b', tags)));
tags[0] = 5;
print(thrown(() => 'a'.localeCompare('b', tags)), thrown(() => 'a'.localeCompare('b', 'de', null)));
// CHECK-NEXT: none
// CHECK-NEXT: TypeError TypeError

// The cache is keyed on the values read, not on the objects passed: a
// getter that alternates between two sensitivities gives alternating
// results, also once both collators are cached.
var flips = 0;
var flipping = {
  get sensitivity() {
    return flips++ % 2 === 0 ? 'base' : 'variant';
  },
};
var flipped = [];
for (var f = 0; f < 4; f++) flipped.push('a'.localeCompare('A', 'en', flipping));
print(flipped.join());
// CHECK-NEXT: 0,-1,0,-1
// Values that a careless key could confuse, each pair used twice in turn so
// that the second round takes both collators from the cache: an absent
// option and an explicit one that differs from the locale's default (th
// ignores punctuation by default, da sorts upper case first), and a list
// of locales and its first element alone (xx is not available, so the
// list resolves to sv and the single tag to the default, en-US).
var probes = [
  ['a b', 'ab', 'th', {ignorePunctuation: false}],
  ['a b', 'ab', 'th', {}],
  ['a', 'A', 'da', {caseFirst: 'false'}],
  ['a', 'A', 'da', {}],
  ['ä', 'z', ['xx', 'sv'], {}],
  ['ä', 'z', ['xx'], {}],
];
for (var r = 0; r < 2; r++) {
  var line = [];
  for (var pr = 0; pr < probes.length; pr++)
    line.push(probes[pr][0].localeCompare(probes[pr][1], probes[pr][2], probes[pr][3]));
  print(line.join());
}
// CHECK-NEXT: -1,0,-1,1,1,-1
// CHECK-NEXT: -1,0,-1,1,1,-1

// Canonicalizing a tag is remembered, also past the bound of what is
// remembered: 100 tags with a language alias, in three passes, each
// canonicalized in lower case, then upper case, then lower case again
// (taken from the cache), still give their canonical forms.
var tagsWrong = 0;
for (var tp = 0; tp < 3; tp++) {
  for (var t = 0; t < 100; t++) {
    var got = Intl.getCanonicalLocales(['iw-x-t' + t, 'IW-X-T' + t, 'iw-x-t' + t]);
    if (got.length !== 1 || got[0] !== 'he-x-t' + t) tagsWrong++;
  }
}
print(tagsWrong, 'wrong canonical tags');
// CHECK-NEXT: 0 wrong canonical tags

// localeCompare sorts exactly as new Intl.Collator(locales, options).compare
// for many locales and option sets, each used twice, so the second use
// takes its collator from the cache. Each option set differs from another
// set of the same locale in a single option, and the words below order
// differently under each option, so two option sets that wrongly shared a
// collator would sort differently.
var words = [
  'a', 'A', 'á', 'Á', 'b', 'B', 'ä', 'ae', 'af', 'z', 'o', 'ö', 'oe', 'å',
  'aa', 'ch', 'h', 'ı', 'i', 'a-b', 'ab', 'a b', '2', '10', 'x2', 'x10',
  'côte', 'cote', 'Côte', 'coté',
];
var testLocales = ['en', 'de', 'sv', 'da', 'th', 'fr', 'de-u-co-phonebk', 'en-u-kn-kf-upper'];
var optionSets = [
  undefined,
  {},
  {sensitivity: 'base'},
  {sensitivity: 'accent'},
  {sensitivity: 'case'},
  {sensitivity: 'variant'},
  {ignorePunctuation: true},
  {ignorePunctuation: false},
  {numeric: true},
  {numeric: false},
  {caseFirst: 'upper'},
  {caseFirst: 'lower'},
  {caseFirst: 'false'},
  {collation: 'phonebk'},
  {collation: 'eor'},
  {usage: 'search'},
  {usage: 'search', sensitivity: 'base'},
  {sensitivity: 'case', caseFirst: 'upper'},
  {sensitivity: 'base', ignorePunctuation: true, numeric: true},
];
var combinations = 0;
var mismatches = 0;
for (var round = 0; round < 2; round++) {
  for (var l = 0; l < testLocales.length; l++) {
    for (var o = 0; o < optionSets.length; o++) {
      var locale = testLocales[l];
      var options = optionSets[o];
      var expected = words.slice().sort(new Intl.Collator(locale, options).compare).join('|');
      var actual = words.slice().sort((x, y) => x.localeCompare(y, locale, options)).join('|');
      combinations++;
      if (actual !== expected) {
        mismatches++;
        print('mismatch:', locale, JSON.stringify(options));
      }
    }
  }
}
print(combinations, 'sorts,', mismatches, 'mismatches');
// CHECK-NEXT: 304 sorts, 0 mismatches

// The cache holds a bounded number of collators. 192 option sets that
// resolve to different options, far more than the cache holds, used in two
// different orders, still compare as new collators with the same options.
var boundLocales = ['en', 'de', 'sv', 'da'];
var sensitivities = ['base', 'accent', 'case', 'variant'];
var caseFirsts = ['upper', 'lower', 'false'];
var many = [];
for (var bl = 0; bl < boundLocales.length; bl++)
  for (var s = 0; s < sensitivities.length; s++)
    for (var cf = 0; cf < caseFirsts.length; cf++)
      for (var n = 0; n < 2; n++)
        for (var ip = 0; ip < 2; ip++) {
          many.push([
            boundLocales[bl],
            {sensitivity: sensitivities[s], caseFirst: caseFirsts[cf], numeric: n === 1, ignorePunctuation: ip === 1},
          ]);
        }
var resolved = new Set();
var pairs = [['a', 'A'], ['a', 'á'], ['2', '10'], ['a-b', 'ab'], ['ä', 'z'], ['å', 'z'], ['b', 'A']];
var wrong = 0;
for (var pass = 0; pass < 2; pass++) {
  for (var m = 0; m < many.length; m++) {
    // The second pass goes through the sets in a different order.
    var entry = many[pass === 0 ? m : (m * 7) % many.length];
    var collator = new Intl.Collator(entry[0], entry[1]);
    resolved.add(JSON.stringify(collator.resolvedOptions()));
    for (var p = 0; p < pairs.length; p++) {
      if (pairs[p][0].localeCompare(pairs[p][1], entry[0], entry[1]) !== collator.compare(pairs[p][0], pairs[p][1]))
        wrong++;
    }
  }
}
print(resolved.size, 'option sets,', wrong, 'wrong');
// CHECK-NEXT: 192 option sets, 0 wrong
