/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// RUN: %hermes %s | %FileCheck --match-full-lines %s

function thrown(f) {
  try {
    f();
    return 'none';
  } catch (e) {
    return e.constructor.name;
  }
}
/// "ok" if list is a non-empty array sorted by code units without duplicates
/// and containing every element of mustHave.
function check(key, mustHave) {
  var list = Intl.supportedValuesOf(key);
  if (!Array.isArray(list) || list.length === 0) return key + ': empty';
  for (var i = 1; i < list.length; i++) {
    if (!(list[i - 1] < list[i])) return key + ': not sorted at ' + list[i];
  }
  for (var j = 0; j < mustHave.length; j++) {
    if (list.indexOf(mustHave[j]) === -1) return key + ': missing ' + mustHave[j];
  }
  return key + ': ok';
}
print(check('calendar', ['ethioaa', 'gregory', 'islamic-civil', 'iso8601', 'japanese']));
// CHECK: calendar: ok
// AvailableCalendars includes these aliases (ECMA-402 6.9.1); step 2.b drops
// them because CanonicalizeUValue changes them.
print(['ethiopic-amete-alem', 'islamicc'].map((c) => Intl.supportedValuesOf('calendar').indexOf(c)).join());
// CHECK-NEXT: -1,-1
print(check('collation', ['phonebk', 'pinyin']));
// CHECK-NEXT: collation: ok
print(check('currency', ['EUR', 'USD']));
// CHECK-NEXT: currency: ok
print(check('numberingSystem', ['arab', 'latn', 'thai']));
// CHECK-NEXT: numberingSystem: ok
print(check('timeZone', ['America/New_York', 'Asia/Kolkata', 'UTC']));
// CHECK-NEXT: timeZone: ok
print(check('unit', ['meter', 'mile-scandinavian', 'percent']));
// CHECK-NEXT: unit: ok
print(Intl.supportedValuesOf('unit').length);
// CHECK-NEXT: 45
print(['standard', 'search'].some((c) => Intl.supportedValuesOf('collation').indexOf(c) !== -1));
// CHECK-NEXT: false
print(Intl.supportedValuesOf('timeZone').indexOf('Etc/UTC'));
// CHECK-NEXT: -1
print(Intl.supportedValuesOf({toString() { return 'calendar'; }}).length > 0);
// CHECK-NEXT: true
print(Intl.supportedValuesOf('unit') !== Intl.supportedValuesOf('unit'));
// CHECK-NEXT: true
print(thrown(() => Intl.supportedValuesOf('Calendar')));
// CHECK-NEXT: RangeError
// Step 1 converts the key once; an unknown key is a RangeError (step 8).
var calls = 0;
var unknownKey = {
  toString() {
    calls++;
    return 'calendars';
  },
};
print(thrown(() => Intl.supportedValuesOf(unknownKey)), calls);
// CHECK-NEXT: RangeError 1
print(thrown(() => Intl.supportedValuesOf(Symbol())));
// CHECK-NEXT: TypeError
