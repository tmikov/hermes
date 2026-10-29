/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// On Linux the plugin takes its default locale from LC_ALL, LC_MESSAGES and
// LANG, and its default time zone from TZ. A POSIX locale with a modifier
// that names no BCP 47 subtag still gives its language and region, and a
// time zone link resolves to its zone.
// REQUIRES: linux
// RUN: env LC_ALL= LC_MESSAGES= LANG=en_GB@foo=bar TZ=US/Pacific %hermes %s | %FileCheck --match-full-lines %s

print(Intl.Collator().resolvedOptions().locale);
// CHECK: en-GB
print(Intl.DateTimeFormat().resolvedOptions().locale);
// CHECK-NEXT: en-GB

var timeZone = new Intl.DateTimeFormat().resolvedOptions().timeZone;
print(timeZone);
// CHECK-NEXT: America/Los_Angeles
var options = {
  year: 'numeric',
  month: 'numeric',
  day: 'numeric',
  hour: 'numeric',
  minute: 'numeric',
  second: 'numeric',
  hour12: false,
};
var date = new Date(Date.UTC(2020, 0, 2, 3, 45, 0, 30));
print(new Intl.DateTimeFormat('en-US', options).format(date));
// CHECK-NEXT: 1/1/2020, 19:45:00
options.timeZone = timeZone;
print(new Intl.DateTimeFormat('en-US', options).format(date));
// CHECK-NEXT: 1/1/2020, 19:45:00
