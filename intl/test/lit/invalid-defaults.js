/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// An invalid explicit default is fatal when the runtime is created; a valid
// one is canonicalized and reduced to an available locale.

// RUN: not --crash %hermes -Xintl-default-time-zone=Mars/Olympus_Mons %s 2>&1 | %FileCheck --check-prefix=ZONE %s
// ZONE: rejected the default locale or time zone: IntlDefaultTimeZone: "Mars/Olympus_Mons" is not an IANA time zone the Intl plugin knows
// RUN: not --crash %hermes -Xintl-default-locale=en_US %s 2>&1 | %FileCheck --check-prefix=LOCALE %s
// LOCALE: rejected the default locale or time zone: IntlDefaultLocales: "en_US" is not a well-formed BCP 47 language tag
// RUN: not --crash %hermes -Xintl-default-locale=tlh -Xintl-default-locale=tlh-US %s 2>&1 | %FileCheck --check-prefix=NONE %s
// NONE: rejected the default locale or time zone: IntlDefaultLocales: none of "tlh", "tlh-US" is an available locale with data in its own script
// ff and bal are available, but their data is only in Adlam and Latin, not in
// their likely scripts (Latin and Arabic).
// RUN: not --crash %hermes -Xintl-default-locale=ff -Xintl-default-locale=ff-BF -Xintl-default-locale=bal %s 2>&1 | %FileCheck --check-prefix=SCRIPT %s
// SCRIPT: rejected the default locale or time zone: IntlDefaultLocales: none of "ff", "ff-BF", "bal" is an available locale with data in its own script
// RUN: %hermes -Xintl-default-locale=ff -Xintl-default-locale=ff-Adlm-BF %s | %FileCheck --check-prefix=OK %s
// RUN: %hermes -Xintl-default-locale=EN-us -Xintl-default-time-zone=asia/calcutta %s | %FileCheck --check-prefix=OK %s
// RUN: %hermes -Xintl-default-locale=tlh -Xintl-default-locale=en-US-u-ca-buddhist %s | %FileCheck --check-prefix=OK %s
// OK: ok
print('ok');
