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
    parser.add_argument('--tests', nargs='*',
                        help='Specific test names (without .wast)')
    args = parser.parse_args()

    runner = os.path.join(os.path.dirname(__file__), 'run-spec-test.py')

    # Default test list
    default_tests = [
        'nop', 'type', 'start', 'func',
        'i32', 'i64', 'f32', 'f64',
        'block', 'loop', 'if',
        'br', 'br_if', 'br_table',
        'call', 'call_indirect',
        'local_get', 'local_set', 'local_tee',
        'global', 'memory', 'select',
        'conversions', 'return', 'unreachable',
        'traps', 'stack', 'names',
        'table', 'elem', 'data',
        'imports', 'exports',
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
                 '--hermes', args.hermes,
                 wast],
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
