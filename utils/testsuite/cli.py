#!/usr/bin/env python3
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

import argparse
import asyncio
import dataclasses
import importlib.resources
import os
import shutil
import sys
import tempfile
import time
from asyncio import Semaphore
from collections import defaultdict
from typing import Awaitable, Dict, List, Optional

from . import utils
from .hermes import ExtraCompileVMArgs
from .progress import (
    ProgressBar,
    SimpleProgressBar,
    TerminalController,
    TestCaseResult,
    TestingProgressDisplay,
)
from .skiplist import SkipCategory, SkippedPathsOrFeatures
from .test_run_defs import get_suite, StrictMode, TestRunArgs
from .typing_defs import PathT
from .utils import Color, TestResultCode

N_DEFAULT_CPU = 10
DEFAULT_TIMEOUT = 200
try:
    RESOURCE_ROOT = importlib.resources.files(__package__)
except TypeError:
    from pathlib import Path

    # With Python 3.9.6 on MacOS, importlib.resources.files raises a TypeError,
    # simply set it to the directory of current file as a fallback (it's unclear
    # whether this will always work).
    RESOURCE_ROOT = Path(os.path.dirname(__file__))


def strip_nargs(value: str):
    """
    Strip the leading/trailing whitespaces in values passed to nargs.

    Adding leading/trailing whitespaces is a common workaround to pass argument
    values with '-' in it (otherwise, argparse will treat it as another flag).
    """
    return value.strip()


def create_parser():
    # Common argument for running all testsuite
    desp = """
    Run Hermes testsuite. Supported testsuites are:
    - test262
    - mjsunit
    - esprima
    - CVEs
    - flow
    """
    parser = argparse.ArgumentParser(description=desp)
    parser.add_argument(
        "-b",
        "--binary-dir",
        dest="binary_directory",
        default=RESOURCE_ROOT,
        help="Path to hermes built binary directory, default to the package root",
    )
    parser.add_argument(
        "--bytecode-compat-check",
        dest="bytecode_compat_check",
        default=False,
        action="store_true",
        help="""When this flag is provided, specified binary directory expects
        an additional b_hermes binary. The hermes binary will be used to compile
        the test file to bytecode, while b_hermes is used to run the bytecode.
        This has no effect in shermes mode and lazy mode.""",
    )
    parser.add_argument(
        "-j",
        "--jobs",
        dest="n_jobs",
        default=os.cpu_count() or N_DEFAULT_CPU,
        type=int,
        help="Number of tests to run simultaneously, default is number of logical CPUs",
    )
    parser.add_argument(
        "--test-skiplist",
        dest="test_skiplist",
        action="store_true",
        help="Run tests in the skiplist and remove them from the list if passed",
    )
    parser.add_argument(
        "--intl-plugin",
        dest="intl_plugin",
        default=None,
        help="Intl plugin mode: load this plugin library in every run, run "
        "intl402, and judge results against --intl-expectations",
    )
    parser.add_argument(
        "--intl-expectations",
        dest="intl_expectations",
        default=None,
        help="Expected-failures file for --intl-plugin",
    )
    parser.add_argument(
        "--intl-update-expectations",
        dest="intl_update_expectations",
        action="store_true",
        help="Rewrite --intl-expectations from this run's results",
    )
    parser.add_argument(
        "--intl-allow-commit-mismatch",
        dest="intl_allow_commit_mismatch",
        action="store_true",
        help="Run even if the test262 checkout is not at the expectations' commit",
    )
    parser.add_argument(
        "-v",
        "--verbose",
        dest="verbose",
        default=False,
        action="store_true",
        help="Show intermediate output",
    )
    parser.add_argument(
        "--work-dir",
        dest="work_dir",
        default=None,
        type=str,
        help="Specifies work directory where the test files will be generated. "
        "The work directory will be deleted if it is not empty",
    )
    group = parser.add_mutually_exclusive_group()
    group.add_argument(
        "--lazy", dest="lazy", action="store_true", help="Force lazy evaluation"
    )
    group.add_argument(
        "--shermes", dest="shermes", action="store_true", help="Test with shermes"
    )
    parser.add_argument(
        "--opt", dest="opt", action="store_true", help="Enable compiler optimizations"
    )
    parser.add_argument(
        "--compile-args",
        dest="extra_compile_args",
        nargs="+",
        default=[],
        type=strip_nargs,
        help="Extra compiler arguments, separated by whitespace",
    )
    parser.add_argument(
        "--vm-args",
        dest="extra_vm_args",
        nargs="+",
        default=[],
        type=strip_nargs,
        help="Extra VM arguments, separated by whitespace",
    )
    parser.add_argument(
        "-d",
        "--dump-source",
        dest="dump_source",
        action="store_true",
        help="Instead of running any tests, print the preprocessed source of "
        "test case to standard output, including any generated use-strict "
        "directives or stubbed pragmas. When this flag is provided, only one "
        "path is expected.",
    )
    parser.add_argument(
        "--timeout",
        dest="timeout",
        default=DEFAULT_TIMEOUT,
        type=int,
        help="Timeout for compiling and running each test.",
    )
    parser.add_argument(
        "--show-slowest-tests",
        dest="show_slowest_tests",
        default=0,
        type=int,
        help="Print the N slowest tests at the end of the run. 0 disables (default).",
    )
    parser.add_argument(
        "paths", type=str, nargs="+", help="Paths to testsuite, can be dir or file"
    )

    return parser


def validate_args(args: argparse.Namespace):
    """
    Validate provided arguments. For -b flag, check whether necessary Hermes
    executables do exist.
    """

    if args.dump_source:
        if len(args.paths) != 1:
            print("Argument error: Only one path is expected with --dump-source.")
            sys.exit(1)

    if args.intl_plugin:
        if not args.intl_expectations:
            print("Argument error: --intl-plugin requires --intl-expectations.")
            sys.exit(1)
        # Plugin mode classifies runs only for the default compile-then-run
        # path.
        for flag, value in (
            ("--lazy", args.lazy),
            ("--shermes", args.shermes),
            ("--bytecode-compat-check", args.bytecode_compat_check),
        ):
            if value:
                print(f"Argument error: --intl-plugin cannot be used with {flag}.")
                sys.exit(1)

    utils.check_hermes_exe(
        args.binary_directory, args.shermes, args.bytecode_compat_check
    )


def print_stats(stats: dict) -> None:
    """Print the result stats."""
    total = sum(stats.values())
    failed = (
        stats[TestResultCode.COMPILE_FAILED]
        + stats[TestResultCode.COMPILE_TIMEOUT]
        + stats[TestResultCode.EXECUTE_FAILED]
        + stats[TestResultCode.EXECUTE_TIMEOUT]
        + stats[TestResultCode.TEST_FAILED]
    )
    executed = (
        sum(stats.values())
        - stats[TestResultCode.TEST_SKIPPED]
        - stats[TestResultCode.TEST_PERMANENTLY_SKIPPED]
    )

    if executed > 0:
        passRate = "{0:.2%}".format(stats[TestResultCode.TEST_PASSED] / executed)
    else:
        passRate = "--"

    if (executed - stats[TestResultCode.TEST_PASSED]) > 0:
        resultStr = "{}FAIL{}".format(Color.RED, Color.RESET)
    else:
        resultStr = "{}PASS{}".format(Color.GREEN, Color.RESET)

    # Turn off formatting so that the table looks nice in source code.
    # fmt: off
    print("-----------------------------------")
    print("| Results              |   {}   |".format(resultStr))
    print("|----------------------+----------|")
    print("| Total                | {:>8} |".format(total))
    print("| Passes               | {:>8} |".format(stats[TestResultCode.TEST_PASSED]))
    print("| Failures             | {:>8} |".format(failed))
    print("| Skipped              | {:>8} |".format(stats[TestResultCode.TEST_SKIPPED]))
    print("| Permanently Skipped  | {:>8} |".format(stats[TestResultCode.TEST_PERMANENTLY_SKIPPED]))
    print("| Pass Rate            | {:>8} |".format(passRate))
    print("-----------------------------------")
    print("| Failures             |          |")
    print("|----------------------+----------|")
    print("| Compile fail         | {:>8} |".format(stats[TestResultCode.COMPILE_FAILED]))
    print("| Compile timeout      | {:>8} |".format(stats[TestResultCode.COMPILE_TIMEOUT]))
    print("| Execute fail         | {:>8} |".format(stats[TestResultCode.EXECUTE_FAILED]))
    print("| Execute timeout      | {:>8} |".format(stats[TestResultCode.EXECUTE_TIMEOUT]))
    print("| Other                | {:>8} |".format(stats[TestResultCode.TEST_FAILED]))
    print("-----------------------------------")
    # fmt: on


def print_failed_tests(tests: dict) -> None:
    def print_test_list(cat: TestResultCode, header: str):
        if len(tests[cat]) > 0:
            print("-----------------------------------")
            print(header)
            for test in tests[cat]:
                print(test)
            print("-----------------------------------")

    if len(tests) == 0:
        return
    print("\nDetails:")
    print_test_list(TestResultCode.COMPILE_FAILED, "Compile failed:")
    print_test_list(TestResultCode.COMPILE_TIMEOUT, "Compile timeout:")
    print_test_list(TestResultCode.EXECUTE_FAILED, "Execute failed:")
    print_test_list(TestResultCode.EXECUTE_TIMEOUT, "Execute timeout:")
    print_test_list(TestResultCode.TEST_FAILED, "Other test failure:")


def print_slowest_tests(results: List[TestCaseResult], n: int = 30) -> None:
    """Print the top N slowest tests by subprocess wall-clock time."""
    if not results:
        return
    slowest = sorted(results, key=lambda r: r.duration, reverse=True)[:n]
    if slowest[0].duration == 0.0:
        return
    print(f"\nSlowest {min(n, len(slowest))} tests:")
    print("-----------------------------------")
    for r in slowest:
        print(f"{r.duration:>7.2f}s  {r.test_name}")
    print("-----------------------------------")


def print_skipped_passed_tests(tests: Dict[str, str]) -> None:
    print("\nPassed tests in skiplist:")
    print("-----------------------------------")
    for test_name in tests:
        print(test_name)
    print("-----------------------------------")


def remove_tests_from_skiplist(
    tests: Dict[str, str], skipped_paths_features: SkippedPathsOrFeatures
) -> None:
    """Remove passed tests from the skiplist config."""

    ans = input("\nRemove these passed tests from skiplist? [y]es/[n]o: ").lower()
    if ans == "y" or ans == "yes":
        # Original full test names have form of "<suite> :: relative/path",
        # we need to convert them to form of "<suite>/relative/path" to match
        # paths in skiplist.
        tests_with_rel_paths = {
            test_name.replace(" :: ", os.sep): suite_dir
            for test_name, suite_dir in tests.items()
        }

        skipped_paths_features.remove_tests(tests_with_rel_paths)
        skipped_paths_features.persist()


async def run(
    tests_paths: List[PathT],
    binary_directory: PathT,
    skipped_paths_features: SkippedPathsOrFeatures,
    work_dir: PathT,
    n_jobs: int,
    test_skiplist: bool,
    bytecode_compat_check: bool,
    lazy: bool,
    shermes: bool,
    opt: bool,
    extra_compile_vm_args: ExtraCompileVMArgs,
    timeout: int,
    verbose: bool,
    show_slowest_tests: int = 0,
    intl: Optional[dict] = None,
) -> int:
    """
    Run all tests with async subprocess and wait for results in completion order
    of test jobs. Each subprocess invokes hermes binary on a given test file, so
    to restrict resource contention, we use asyncio.Semaphore to control the
    maximum number of alive tasks (configured by n_jobs).

    We first collect all tasks for given tests_paths, then wrap each of them
    in a new task guarded by Semaphore, and pass them to asyncio.as_completed().
    Each task can be awaited in it to get earliest next result. And once a
    task is done, its Semaphore counter is released and a new task can
    continue to run.

    After all tasks are done, print the test stats and relative paths of all
    failing tests (if there are).

    In Intl plugin mode (intl is the dict from intl_prepare), the skiplist's
    path entries do not apply to intl402, and the return value is the
    verdict against the expectations file instead. Normal mode has no Intl,
    so it skips every intl402 test (see intl402_skip).
    """

    # Get the common path of all file/directory paths to be used when creating
    # temporary files.
    tests_home = os.path.commonpath(tests_paths)
    tests_files = utils.list_all_files(tests_paths)

    header = f"-- Testing: {len(tests_files)} tests, max {n_jobs} concurrent tasks --"
    try:
        term = TerminalController()
        pb = ProgressBar(term, header)
    except ValueError:
        print(header)
        pb = SimpleProgressBar("Testing: ")
    pd = TestingProgressDisplay(len(tests_files), pb, verbose)
    current_n_tasks = Semaphore(n_jobs)

    start_time = time.time()
    tasks = []
    stats = defaultdict(int)
    # Record skipped tests and its suite path (as dict value).
    # we need the suite path because some paths in the skiplist config are
    # directories, we need to get their absolute paths to get all files under
    # them.
    skipped_tests = {}
    for test_file in tests_files:
        suite = get_suite(test_file)
        assert suite, f"Test suite root directory is not found for {test_file}"
        full_test_name = suite.get_full_test_name(test_file)

        # Normal mode skips intl402 whatever the skiplist and
        # --test-skiplist say.
        if test_result := intl402_skip(test_file, full_test_name, intl):
            pd.update(test_result)
            stats[test_result.code] += 1
            continue

        # Plugin mode ignores every skiplist path entry for intl402. It also
        # ignores the platform skip list, which has no entries for Linux or
        # macOS, the hosts plugin mode supports. The other lists record
        # non-Intl Hermes bugs and still apply.
        if not (intl and in_intl402(test_file)):
            # Check if this file should be skipped w.r.t. the test_skiplist flag.
            skip_categories = [
                SkipCategory.SKIP_LIST,
                SkipCategory.PERMANENT_SKIP_LIST,
                SkipCategory.MANUAL_SKIP_LIST,
            ]
            if not intl:
                skip_categories.append(SkipCategory.PLATFORM_SKIP_LIST)
            if lazy:
                skip_categories.append(SkipCategory.LAZY_SKIP_LIST)
            if test_result := skipped_paths_features.try_skip(
                test_file,
                skip_categories,
                full_test_name,
            ):
                if not test_skiplist:
                    pd.update(test_result)
                    stats[test_result.code] += 1
                    continue
                else:
                    skipped_tests[full_test_name] = suite.directory

        test_run_args = TestRunArgs(
            test_file,
            tests_home,
            work_dir,
            binary_directory,
            skipped_paths_features,
            test_skiplist,
            bytecode_compat_check,
            lazy,
            shermes,
            opt,
            extra_compile_vm_args,
            timeout,
            intl_plugin=intl["plugin"] if intl else None,
        )
        tasks.append(suite.run_test(test_run_args))

    # Control maximum running tasks.
    async def wrap_task(fut: Awaitable) -> TestCaseResult:
        # Acquire the Semaphore and decrement it by one. If the counter is
        # already zero, wait until one running task is done, which will release
        # the Semaphore and increment the counter.
        async with current_n_tasks:
            return await fut

    failed_cases = defaultdict(list)
    # Similar to skipped_tests, but only store those passed.
    skipped_passed = {}
    all_results: List[TestCaseResult] = []
    for task in asyncio.as_completed([wrap_task(fut) for fut in tasks]):
        result = await task
        stats[result.code] += 1
        if intl:
            pd.update(intl_display_result(result, intl))
        else:
            pd.update(result)
        if result.code.is_failure:
            failed_cases[result.code].append(result.test_name)
        # Collect non-skipped results for slowest-tests report.
        if result.code not in (
            TestResultCode.TEST_SKIPPED,
            TestResultCode.TEST_PERMANENTLY_SKIPPED,
        ):
            all_results.append(result)
        # Some tests might be skipped due to unsupported features, we should consider
        # them as "passed" as well since they will be skipped anyway.
        if (
            result.code == TestResultCode.TEST_PASSED
            or result.code == TestResultCode.TEST_SKIPPED
        ):
            if test_suite_dir := skipped_tests.get(result.test_name):
                skipped_passed[result.test_name] = test_suite_dir

    pd.finish()
    elapsed = time.time() - start_time
    print(f"Testing time: {elapsed:.2f}")

    if intl:
        skipped = (
            stats[TestResultCode.TEST_SKIPPED]
            + stats[TestResultCode.TEST_PERMANENTLY_SKIPPED]
        )
        print(f"Intl plugin mode: {len(tests_files)} tests, {skipped} skipped")
        return intl_verdict(all_results, intl)

    # Print result
    print_stats(stats)
    if show_slowest_tests > 0:
        print_slowest_tests(all_results, min(show_slowest_tests, len(all_results)))
    print_failed_tests(failed_cases)
    if len(skipped_passed) > 0 and test_skiplist:
        print_skipped_passed_tests(skipped_passed)
        remove_tests_from_skiplist(skipped_passed, skipped_paths_features)

    return 1 if len(failed_cases) > 0 else 0


def in_intl402(test_file: PathT) -> bool:
    """Whether test_file is in test262's intl402 directory."""
    return "/test/intl402/" in os.fspath(test_file).replace(os.sep, "/")


def intl402_skip(
    test_file: PathT, full_test_name: str, intl: Optional[dict]
) -> Optional[TestCaseResult]:
    """The skip result for an intl402 test in normal mode, else None. Normal
    mode disables the Intl plugin and Hermes has no Intl of its own, so no
    intl402 test can pass there; --test-skiplist does not run them either.
    Plugin mode (intl set) runs them all."""
    if intl or not in_intl402(test_file):
        return None
    return TestCaseResult(
        full_test_name,
        TestResultCode.TEST_SKIPPED,
        f"SKIP: intl402 needs the Intl plugin (--intl-plugin) ({test_file})",
    )


def test262_root(tests_paths) -> str:
    """The test262 checkout containing the first test path."""
    # The trailing "/" also finds the root when the path is <root>/test.
    p = os.path.abspath(tests_paths[0]).replace(os.sep, "/") + "/"
    i = p.rfind("/test/")
    return p[:i] if i >= 0 else p.rstrip("/")


def intl_display_result(result: TestCaseResult, intl: dict) -> TestCaseResult:
    """What plugin mode shows for one result: a pass when it is as the
    expectations say, so only real problems print their output."""
    from . import intl_plugin

    if result.code in (
        TestResultCode.TEST_SKIPPED,
        TestResultCode.TEST_PERMANENTLY_SKIPPED,
    ):
        return result
    problems = intl_plugin.judge(result, intl["expectations"], intl["update"])
    if not problems:
        return dataclasses.replace(
            result, code=TestResultCode.TEST_PASSED, msg="As expected"
        )
    return dataclasses.replace(
        result, code=TestResultCode.EXECUTE_FAILED, msg="; ".join(problems)
    )


def intl_load_expectations(args):
    """The parsed expectations file, or None after printing why not. Under
    --intl-update-expectations a missing file starts out empty."""
    from . import intl_plugin

    path = args.intl_expectations
    if args.intl_update_expectations and not os.path.exists(path):
        return intl_plugin.Expectations("")
    try:
        with open(path) as f:
            text = f.read()
    except OSError as err:
        print(f"Cannot read {path}: {err.strerror}")
        return None
    try:
        return intl_plugin.parse_expectations(text)
    except ValueError as err:
        print(f"{path}: {err}")
        return None


def intl_prepare(args, binary_directory, work_dir, tests_paths) -> Optional[dict]:
    """Check the plugin loads and the checkout's commit; None on failure."""
    from . import intl_plugin

    hermes = os.path.join(binary_directory, "hermes")
    if problem := intl_plugin.probe(hermes, args.intl_plugin, work_dir):
        print(f"Intl plugin probe failed:\n{problem}")
        return None
    expectations = intl_load_expectations(args)
    if expectations is None:
        return None
    root = test262_root(tests_paths)
    commit = intl_plugin.checkout_commit(root)
    if (
        commit != expectations.commit
        and not args.intl_allow_commit_mismatch
        and not args.intl_update_expectations
    ):
        print(
            f"test262 at {root} is at {commit or 'an unknown commit'}, but "
            f"{args.intl_expectations} was made against {expectations.commit}. "
            "Check out that commit, or pass --intl-allow-commit-mismatch."
        )
        return None
    return {
        "plugin": args.intl_plugin,
        "expectations": expectations,
        "file": args.intl_expectations,
        "update": args.intl_update_expectations,
        "root": root,
        # The run's test files and directories, relative to the checkout.
        "roots": [
            os.path.relpath(os.path.abspath(p), root).replace(os.sep, "/")
            for p in tests_paths
        ],
        "commit": commit,
    }


def intl_verdict(results, intl) -> int:
    """Plugin mode's exit code: write the expectations file when updating,
    otherwise judge results against it and print every problem."""
    from . import intl_plugin

    counts = intl_plugin.count_outcomes(results)
    print(
        "Intl plugin mode, runs by strictness mode: "
        + ", ".join(f"{counts[o]} {o}" for o in intl_plugin.OUTCOMES)
    )
    if intl["update"]:
        new = intl_plugin.update(
            results, intl["expectations"], intl["root"], intl["commit"], intl["roots"]
        )
        if new is None:
            return 1
        with open(intl["file"], "w") as f:
            f.write(intl_plugin.render_expectations(new))
        print(f"Wrote {intl['file']}")
        # The file is written either way, but an entry without a reason fails
        # check-intl, so the update fails too until each is explained.
        unexplained = [
            f"{path} [{mode}]"
            for path, modes in sorted(new.entries.items())
            for mode, reason in sorted(modes.items())
            if reason == intl_plugin.UNEXPLAINED
        ]
        if unexplained:
            print(
                f"{len(unexplained)} entr{'y' if len(unexplained) == 1 else 'ies'}"
                " without a reason (UNEXPLAINED); explain each by hand:"
            )
            for u in unexplained:
                print("  " + u)
            return 1
        return 0
    problems = intl_plugin.evaluate(
        results, intl["expectations"], intl["root"], intl["roots"]
    )
    for p in problems:
        print(p)
    print(f"Intl plugin mode: {len(problems)} problem(s)")
    return 1 if problems else 0


def print_preprocessed_source(path: PathT) -> None:
    """
    Print the preprocessed source to STDOUT. For test262/mjsunit, it may include
    harness code and macros are replaced by actual code. For CVEs/flow/esprima,
    the exact source code is returned.
    """

    from .preprocess import generate_source

    suite = get_suite(path)
    if not suite:
        print("Not a test from valid test suite.")
        return
    with open(path, "rb") as f:
        source = f.read().decode("utf-8")
    test_case = generate_source(source, suite.directory, suite.get_full_test_name(path))
    # This is specifically for test262, and we only insert the directive if
    # it's in strict only mode (when testing we would have to test both strict
    # and non-strict mode if none of them is specified).
    if StrictMode.STRICT in test_case.strict_mode:
        print("'use strict';")
    print(test_case.source)


async def main() -> int:
    parser = create_parser()
    args = parser.parse_args()
    validate_args(args)

    # If user wants to dump the preprocessed source for debugging, handle it
    # separately for readability.
    if args.dump_source:
        print_preprocessed_source(args.paths[0])
        return 0

    work_dir: Optional[str] = args.work_dir
    # Hold the reference to the temporary directory (if we do create one), so
    # it won't be deleted until we run all tests.
    tmp_work_dir = None
    if work_dir is None:
        tmp_work_dir = tempfile.TemporaryDirectory(suffix="hermes_workdir")
        work_dir = tmp_work_dir.name
    else:
        # Remove the directory if it exists.
        if os.path.exists(work_dir):
            shutil.rmtree(work_dir)
        os.makedirs(work_dir, exist_ok=False)

    with importlib.resources.as_file(
        RESOURCE_ROOT / "skiplist.json"
    ) as skip_list_cfg_path:
        skipped_paths_features = SkippedPathsOrFeatures(os.fspath(skip_list_cfg_path))

    # Consumes all extra flags.
    extra_compile_vm_args = ExtraCompileVMArgs(
        compile_args=args.extra_compile_args, vm_args=args.extra_vm_args
    )

    intl = None
    if args.intl_plugin:
        intl = intl_prepare(args, args.binary_directory, work_dir, args.paths)
        if intl is None:
            if tmp_work_dir:
                tmp_work_dir.cleanup()
            return 1

    exit_code = await run(
        args.paths,
        args.binary_directory,
        skipped_paths_features,
        work_dir,
        args.n_jobs,
        args.test_skiplist,
        args.bytecode_compat_check,
        args.lazy,
        args.shermes,
        args.opt,
        extra_compile_vm_args,
        args.timeout,
        args.verbose,
        args.show_slowest_tests,
        intl=intl,
    )

    # Explicitly clean up the temporary directory if we created one.
    if tmp_work_dir:
        tmp_work_dir.cleanup()

    return exit_code


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
