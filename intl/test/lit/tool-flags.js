/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// hermes (running a file, and its REPL) and hvm build their RuntimeConfig
// themselves and take the Intl plugin flags through cli::withIntlPluginFlags
// (lib/VM/RuntimeFlags.cpp); shermes-flags.js covers buildRuntimeConfig. Each
// of the four flags must reach the loader from each of them. With the plugin
// off, a plugin path that does not exist is never loaded, which would be
// fatal, and there is no Intl.

// RUN: %hermes_default %s | %FileCheck --match-full-lines --check-prefix=ON %s
// RUN: %hermes_default -Xintl-plugin=false -Xintl-plugin-path=/nonexistent/libhermesintl.so %s | %FileCheck --match-full-lines --check-prefix=OFF %s
// RUN: not --crash %hermes_default -Xintl-plugin-path=/nonexistent/libhermesintl.so %s 2>&1 | %FileCheck --check-prefix=PATH %s
// RUN: not --crash %hermes_default -Xintl-default-locale=en_US %s 2>&1 | %FileCheck --check-prefix=LOCALE %s
// RUN: not --crash %hermes_default -Xintl-default-time-zone=Mars/Olympus_Mons %s 2>&1 | %FileCheck --check-prefix=ZONE %s

// RUN: echo 'typeof Intl' | %hermes_default -prompt= -prompt2= | %FileCheck --match-full-lines --check-prefix=REPLON %s
// RUN: echo 'typeof Intl' | %hermes_default -prompt= -prompt2= -Xintl-plugin=false -Xintl-plugin-path=/nonexistent/libhermesintl.so | %FileCheck --match-full-lines --check-prefix=REPLOFF %s
// RUN: echo 'typeof Intl' | not --crash %hermes_default -prompt= -Xintl-plugin-path=/nonexistent/libhermesintl.so 2>&1 | %FileCheck --check-prefix=PATH %s
// RUN: echo 'typeof Intl' | not --crash %hermes_default -prompt= -Xintl-default-locale=en_US 2>&1 | %FileCheck --check-prefix=LOCALE %s
// RUN: echo 'typeof Intl' | not --crash %hermes_default -prompt= -Xintl-default-time-zone=Mars/Olympus_Mons 2>&1 | %FileCheck --check-prefix=ZONE %s

// RUN: %hermesc -emit-binary -out=%t.hbc %s
// RUN: %hvm %t.hbc | %FileCheck --match-full-lines --check-prefix=ON %s
// RUN: %hvm -Xintl-plugin=false -Xintl-plugin-path=/nonexistent/libhermesintl.so %t.hbc | %FileCheck --match-full-lines --check-prefix=OFF %s
// RUN: not --crash %hvm -Xintl-plugin-path=/nonexistent/libhermesintl.so %t.hbc 2>&1 | %FileCheck --check-prefix=PATH %s
// RUN: not --crash %hvm -Xintl-default-locale=en_US %t.hbc 2>&1 | %FileCheck --check-prefix=LOCALE %s
// RUN: not --crash %hvm -Xintl-default-time-zone=Mars/Olympus_Mons %t.hbc 2>&1 | %FileCheck --check-prefix=ZONE %s

// ON: object
// OFF: undefined
// REPLON: "object"
// REPLOFF: "undefined"
// PATH: LLVM ERROR: Intl plugin /nonexistent/libhermesintl.so: cannot be loaded:
// LOCALE: rejected the default locale or time zone: IntlDefaultLocales: "en_US" is not a well-formed BCP 47 language tag
// ZONE: rejected the default locale or time zone: IntlDefaultTimeZone: "Mars/Olympus_Mons" is not an IANA time zone the Intl plugin knows

print(typeof Intl);
