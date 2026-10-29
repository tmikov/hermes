/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// A program compiled by shermes takes its runtime flags through
// buildRuntimeConfig (lib/VM/RuntimeFlags.cpp), as tools/test-runner does.
// Each of the four Intl plugin flags must reach the loader from there.

// RUN: %shermes -exec %s | %FileCheck --match-full-lines --check-prefix=ON %s
// ON: object

// With the plugin off, a plugin path that does not exist is never loaded,
// which would be fatal, and there is no Intl.
// RUN: %shermes -exec %s -Wx,-Xintl-plugin=false -Wx,-Xintl-plugin-path=/nonexistent/libhermesintl.so | %FileCheck --match-full-lines --check-prefix=OFF %s
// OFF: undefined

// RUN: not --crash %shermes -exec %s -Wx,-Xintl-plugin-path=/nonexistent/libhermesintl.so 2>&1 | %FileCheck --check-prefix=PATH %s
// PATH: LLVM ERROR: Intl plugin /nonexistent/libhermesintl.so: cannot be loaded:

// RUN: not --crash %shermes -exec %s -Wx,-Xintl-default-locale=en_US 2>&1 | %FileCheck --check-prefix=LOCALE %s
// LOCALE: rejected the default locale or time zone: IntlDefaultLocales: "en_US" is not a well-formed BCP 47 language tag

// RUN: not --crash %shermes -exec %s -Wx,-Xintl-default-time-zone=Mars/Olympus_Mons 2>&1 | %FileCheck --check-prefix=ZONE %s
// ZONE: rejected the default locale or time zone: IntlDefaultTimeZone: "Mars/Olympus_Mons" is not an IANA time zone the Intl plugin knows

print(typeof Intl);
