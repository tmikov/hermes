#!/usr/bin/env python3
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

"""
Run all Wasm spec tests and generate a summary report.

Usage:
    python3 run-all-spec-tests.py --wast2json PATH --hermes PATH --testsuite DIR
"""

import argparse
import os
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description='Run all Wasm spec tests')
    parser.add_argument('--wast2json', required=True, help='Path to wast2json')
    parser.add_argument('--hermes', required=True, help='Path to hermes')
    parser.add_argument('--testsuite', required=True, help='Path to testsuite dir')
    parser.add_argument('--timeout', type=int, default=60,
                        help='Timeout per test (seconds)')
    parser.add_argument('--hermes-arg', action='append', default=[],
                        help='Extra flag to pass to hermes (repeatable)')
    parser.add_argument('--tests', nargs='*',
                        help='Specific test names (without .wast)')
    args = parser.parse_args()

    runner = os.path.join(os.path.dirname(__file__), 'run-spec-test.py')

    # Default test list — core Wasm MVP and bulk memory tests.
    # Excludes SIMD, GC, memory64, and other advanced proposal tests.
    # Also excludes tests that are known to crash (fac, throw,
    # memory_trap) or always timeout (skip-stack-guard-page).
    default_tests = [
        # Basic structure and types
        'nop', 'type', 'start', 'func', 'func_ptrs',
        # Numeric operations
        'i32', 'i64', 'f32', 'f64',
        'f32_bitwise', 'f32_cmp', 'f64_bitwise', 'f64_cmp',
        'float_exprs', 'float_literals', 'float_memory', 'float_misc',
        'int_exprs', 'int_literals', 'conversions', 'const',
        # Control flow
        'block', 'loop', 'if',
        'br', 'br_if', 'br_table',
        'switch', 'return', 'unreachable',
        'unreached-valid', 'labels', 'forward', 'left-to-right',
        # Calls
        'call', 'call_indirect',
        # Variables
        'local_get', 'local_set', 'local_tee', 'global',
        # Memory
        'memory', 'load', 'store',
        'memory_grow', 'memory_size', 'memory_redundancy',
        'address', 'align', 'endianness',
        # Bulk memory operations
        'memory_copy', 'memory_fill', 'memory_init',
        'bulk', 'data',
        # Table operations
        'table', 'elem',
        'table_copy', 'table_fill', 'table_get', 'table_grow',
        'table_init', 'table_set', 'table_size',
        # Other
        'select', 'stack', 'traps', 'unwind',
        'names', 'imports', 'exports', 'custom', 'binary',
        'binary-leb128', 'comments', 'token',
        # UTF-8 validation
        'utf8-custom-section-id', 'utf8-import-field',
        'utf8-import-module', 'utf8-invalid-encoding',
        # Exception handling
        'tag',
        # Reference types
        'ref_is_null', 'ref_null',
        # Linking
        'linking',
    ]

    test_names = args.tests or default_tests

    results = []
    for name in test_names:
        wast = os.path.join(args.testsuite, name + '.wast')
        if not os.path.exists(wast):
            results.append((name, 'NOT_FOUND', 0, 0, 0))
            continue

        try:
            r = subprocess.run(
                ['python3', runner,
                 '--wast2json', args.wast2json,
                 '--hermes', args.hermes]
                + ['--hermes-arg=' + a for a in args.hermes_arg]
                + [wast],
                capture_output=True, text=True, timeout=args.timeout
            )
            output = r.stdout
            passed = failed = skipped = 0
            for line in output.split('\n'):
                if line.startswith('PASSED:'):
                    passed = int(line.split(':')[1].strip())
                elif line.startswith('FAILED:'):
                    failed = int(line.split(':')[1].strip())
                elif line.startswith('SKIPPED:'):
                    skipped = int(line.split(':')[1].strip())

            if 'SPEC TEST PASSED' in output:
                results.append((name, 'PASS', passed, failed, skipped))
            elif 'SPEC TEST FAILED' in output:
                results.append((name, 'FAIL', passed, failed, skipped))
            else:
                results.append((name, 'CRASH', passed, failed, skipped))
        except subprocess.TimeoutExpired:
            results.append((name, 'TIMEOUT', 0, 0, 0))

    # Print results table
    print(f"{'Test':<25} {'Status':<8} {'Passed':>7} {'Failed':>7} {'Skipped':>7}")
    print('-' * 60)

    total_pass = total_fail = total_crash = total_timeout = 0
    total_assertions_pass = total_assertions_fail = 0

    for name, status, passed, failed, skipped in results:
        print(f'{name:<25} {status:<8} {passed:>7} {failed:>7} {skipped:>7}')
        total_assertions_pass += passed
        total_assertions_fail += failed
        if status == 'PASS':
            total_pass += 1
        elif status == 'FAIL':
            total_fail += 1
        elif status == 'CRASH':
            total_crash += 1
        elif status == 'TIMEOUT':
            total_timeout += 1

    print('-' * 60)
    total = len(results)
    print(f"{'TOTAL':<25} {'':>8} {total_assertions_pass:>7} {total_assertions_fail:>7}")
    print()
    print(f"Files: {total_pass} pass, {total_fail} fail, "
          f"{total_crash} crash, {total_timeout} timeout "
          f"(out of {total})")
    print(f"Assertions: {total_assertions_pass} pass, "
          f"{total_assertions_fail} fail")

    return 0 if total_fail == 0 and total_crash == 0 and total_timeout == 0 else 1


if __name__ == '__main__':
    sys.exit(main())
