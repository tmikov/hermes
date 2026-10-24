/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

// Every failure with an explicit IntlPluginPath is fatal and names the
// library and the reason.

// RUN: %not --crash %hermes -Xintl-plugin-path=%t.missing.so %s 2>&1 | %FileCheck --check-prefix=MISSING %s
// MISSING: LLVM ERROR: Intl plugin {{.*}}.missing.so: cannot be loaded:

// RUN: echo "not a library" > %t.text.so
// RUN: %not --crash %hermes -Xintl-plugin-path=%t.text.so %s 2>&1 | %FileCheck --check-prefix=TEXT %s
// TEXT: LLVM ERROR: Intl plugin {{.*}}.text.so: cannot be loaded:

// RUN: %not --crash %hermes -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_no_entry.so %s 2>&1 | %FileCheck --check-prefix=NOENTRY %s
// NOENTRY: LLVM ERROR: Intl plugin {{.*}}libfake_intl_no_entry.so: does not export hermes_intl_get_entry

// RUN: %not --crash %hermes -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_null_entry.so %s 2>&1 | %FileCheck --check-prefix=NULLENTRY %s
// NULLENTRY: LLVM ERROR: Intl plugin {{.*}}libfake_intl_null_entry.so: returned no entry

// RUN: %not --crash %hermes -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_bad_interface.so %s 2>&1 | %FileCheck --check-prefix=INTERFACE %s
// INTERFACE: LLVM ERROR: Intl plugin {{.*}}libfake_intl_bad_interface.so: plugin interface version {{[0-9]+}}, Hermes expects {{[0-9]+}}

// RUN: %not --crash %hermes -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_bad_bytecode.so %s 2>&1 | %FileCheck --check-prefix=BYTECODE %s
// BYTECODE: LLVM ERROR: Intl plugin {{.*}}libfake_intl_bad_bytecode.so: bytecode version {{[0-9]+}}, Hermes expects {{[0-9]+}}

// RUN: %not --crash %hermes -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_bad_flags.so %s 2>&1 | %FileCheck --check-prefix=FLAGS %s
// FLAGS: LLVM ERROR: Intl plugin {{.*}}libfake_intl_bad_flags.so: built with JSI_UNSTABLE

// RUN: %not --crash %hermes -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_throwing.so %s 2>&1 | %FileCheck --check-prefix=THROWS %s
// THROWS: LLVM ERROR: Intl plugin {{.*}}libfake_intl_throwing.so: setup failed: fake plugin setup failure

// RUN: %not --crash %hermes -Xintl-plugin-path=%intl_fixture_dir/libfake_intl_rejects_defaults.so %s 2>&1 | %FileCheck --check-prefix=REJECTS %s
// REJECTS: LLVM ERROR: Intl plugin {{.*}}libfake_intl_rejects_defaults.so: rejected the default locale or time zone: fake plugin rejects every default

print('unreachable');
