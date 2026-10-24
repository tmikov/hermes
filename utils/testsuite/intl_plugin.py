# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

"""test262 plugin mode for the Intl plugin: expectations, per-mode results,
and the run's verdict. See "Testing" in
doc/superpowers/specs/2026-10-22-intl-plugin-design.md."""

import os
import re
import subprocess
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple

PASS = "pass"
JS_FAIL = "js-fail"
HARD_FAIL = "hard-fail"
OUTCOMES = (PASS, JS_FAIL, HARD_FAIL)

MODES = ("strict", "non-strict")

# Reasons for new expected failures, by path prefix. Anything else is
# UNEXPLAINED and must be explained by hand before the run passes.
PHASE_REASONS = [
    ("test/intl402/Locale/", "Phase 1: Intl.Locale"),
    ("test/intl402/Collator/", "Phase 2: Intl.Collator"),
    ("test/intl402/String/", "Phase 2: locale-sensitive String methods"),
    ("test/built-ins/String/prototype/localeCompare/", "Phase 2: String.prototype.localeCompare"),
    ("test/built-ins/String/prototype/toLocale", "Phase 2: locale-sensitive case mapping"),
    ("test/intl402/NumberFormat/", "Phase 3: Intl.NumberFormat"),
    ("test/intl402/PluralRules/", "Phase 3: Intl.PluralRules"),
    ("test/intl402/Number/", "Phase 3: Number.prototype.toLocaleString"),
    ("test/intl402/BigInt/", "Phase 3: BigInt.prototype.toLocaleString"),
    ("test/intl402/Array/", "Phase 3: Array.prototype.toLocaleString"),
    ("test/intl402/TypedArray/", "Phase 3: TypedArray.prototype.toLocaleString"),
    ("test/built-ins/Number/prototype/toLocaleString/", "Phase 3: Number.prototype.toLocaleString"),
    ("test/built-ins/BigInt/prototype/toLocaleString/", "Phase 3: BigInt.prototype.toLocaleString"),
    ("test/built-ins/Array/prototype/toLocaleString/", "Phase 3: Array.prototype.toLocaleString"),
    ("test/built-ins/TypedArray/prototype/toLocaleString/", "Phase 3: TypedArray.prototype.toLocaleString"),
    ("test/intl402/DateTimeFormat/", "Phase 4: Intl.DateTimeFormat"),
    ("test/intl402/Date/", "Phase 4: Date.prototype.toLocale*String"),
    ("test/built-ins/Date/prototype/toLocale", "Phase 4: Date.prototype.toLocale*String"),
    ("test/intl402/ListFormat/", "Phase 5: Intl.ListFormat"),
    ("test/intl402/RelativeTimeFormat/", "Phase 5: Intl.RelativeTimeFormat"),
    ("test/intl402/DisplayNames/", "Phase 5: Intl.DisplayNames"),
    ("test/intl402/DurationFormat/", "Phase 5: Intl.DurationFormat"),
    ("test/intl402/Segmenter/", "Phase 5: Intl.Segmenter"),
]

UNEXPLAINED = "UNEXPLAINED"


@dataclass
class Expectations:
    """An expectations file: the test262 commit, and path -> mode -> reason."""

    commit: str
    entries: Dict[str, Dict[str, str]] = field(default_factory=dict)


def parse_expectations(text: str) -> Expectations:
    commit = ""
    entries: Dict[str, Dict[str, str]] = {}
    for number, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("test262-commit:"):
            commit = line.split(":", 1)[1].strip()
            continue
        parts = line.split(None, 2)
        if len(parts) != 3 or parts[1] not in ("both",) + MODES:
            raise ValueError(f"expectations line {number}: cannot parse {raw!r}")
        path, mode, reason = parts
        modes = MODES if mode == "both" else (mode,)
        for m in modes:
            entries.setdefault(path, {})[m] = reason
    if not commit:
        raise ValueError("expectations: missing 'test262-commit:' line")
    return Expectations(commit, entries)


HEADER = """\
# Expected test262 failures for the Hermes Intl plugin (check-intl).
# Format: <path> <both|strict|non-strict> <reason>
# Each line is a test that must fail, in that mode, with a JS exception or a
# test262 assertion. A crash, timeout or loader error never matches. A listed
# test that passes fails the run, so this list only shrinks. Regenerate with
# test_runner.py --intl-update-expectations; reasons are kept, and new
# entries outside the known phases are UNEXPLAINED until explained by hand.
# An entry for a test that the run covers but that got no result in that
# mode (skipped, or run only in the other mode) fails the run, and
# regenerating drops it. Regenerating writes this header again and keeps no
# other comment, so put every explanation in the reason.
# "Phase N" is the phase in doc/superpowers/specs/2026-10-22-intl-plugin-design.md
# that brings what the test needs.
"""


def render_expectations(e: Expectations) -> str:
    lines = [HEADER, f"test262-commit: {e.commit}"]
    for path in sorted(e.entries):
        modes = e.entries[path]
        if set(modes) == set(MODES) and modes["strict"] == modes["non-strict"]:
            lines.append(f"{path} both {modes['strict']}")
        else:
            for m in MODES:
                if m in modes:
                    lines.append(f"{path} {m} {modes[m]}")
    return "\n".join(lines) + "\n"


def derive_reason(path: str) -> str:
    for prefix, reason in PHASE_REASONS:
        if path.startswith(prefix):
            return reason
    return UNEXPLAINED


def result_path(result) -> str:
    """The test's path relative to the test262 checkout."""
    return result.test_name.split(" :: ", 1)[1]


def is_modeless_failure(result) -> bool:
    """Whether result failed without reaching the per-mode runs, for
    example in a suite other than test262. Such a failure is never a JS
    failure, so it counts as a hard failure."""
    if getattr(result, "modes", None):
        return False
    code = getattr(result, "code", None)
    return code is not None and code.is_failure


def judge(result, e: Expectations, updating: bool = False) -> List[str]:
    """The problems with one test's result against e; empty means it is as
    expected. When updating, only hard failures are problems, since every
    JS failure will be recorded."""
    if is_modeless_failure(result):
        return [f"failed before the per-mode runs: {result.test_name}: {result.msg}"]
    modes = getattr(result, "modes", None)
    if not modes:
        return []
    problems = []
    path = result_path(result)
    expected = e.entries.get(path, {})
    for mode, outcome in sorted(modes.items()):
        reason = expected.get(mode)
        if outcome == HARD_FAIL:
            problems.append(
                f"crash, timeout, loader or plugin internal error: {path} [{mode}]"
            )
        elif updating:
            continue
        elif outcome == PASS and reason is not None:
            problems.append(f"unexpected pass: {path} [{mode}] (listed: {reason})")
        elif outcome == JS_FAIL and reason is None:
            problems.append(f"unexpected failure: {path} [{mode}]")
    return problems


def under_roots(path: str, roots: List[str]) -> bool:
    """Whether path is in one of roots, the files and directories the run was
    given. Both are relative to the test262 checkout; "." is all of it."""
    for root in roots:
        root = root.rstrip("/")
        if root in ("", ".") or path == root or path.startswith(root + "/"):
            return True
    return False


def unrun_entries(results, e: Expectations, roots: List[str]) -> List[Tuple[str, str]]:
    """The (path, mode) entries of e for tests in roots that got no result in
    that mode: the test was skipped, or it runs only in the other mode."""
    ran = set()
    for result in results:
        for mode in getattr(result, "modes", None) or {}:
            ran.add((result_path(result), mode))
    return [
        (path, mode)
        for path, modes in sorted(e.entries.items())
        if under_roots(path, roots)
        for mode in sorted(modes)
        if (path, mode) not in ran
    ]


def evaluate(
    results, e: Expectations, test262_root: str, roots: List[str]
) -> List[str]:
    """Every problem with this run against e; empty means it passed. roots
    are the files and directories the run was given, relative to
    test262_root."""
    problems = []
    for result in results:
        problems += judge(result, e)
    for path, modes in sorted(e.entries.items()):
        if not os.path.exists(os.path.join(test262_root, path)):
            problems.append(f"expectation for a missing test: {path}")
        if UNEXPLAINED in modes.values():
            problems.append(f"expectation without a reason: {path}")
    for path, mode in unrun_entries(results, e, roots):
        if os.path.exists(os.path.join(test262_root, path)):
            problems.append(
                f"expectation for a test that did not run in that mode: {path} [{mode}]"
            )
    return problems


def count_outcomes(results) -> Dict[str, int]:
    """How many runs, one per test and strictness mode, had each outcome."""
    counts = {o: 0 for o in OUTCOMES}
    for result in results:
        if is_modeless_failure(result):
            counts[HARD_FAIL] += 1
        for outcome in (getattr(result, "modes", None) or {}).values():
            counts[outcome] += 1
    return counts


def update(
    results, old: Expectations, test262_root: str, commit: str, roots: List[str]
) -> Optional[Expectations]:
    """New expectations from this run; None if the run had hard failures,
    which can never be expected. roots are as for evaluate. Within them only
    this run's failures are kept; outside them, the old entries for tests
    that still exist."""
    hard = []
    entries: Dict[str, Dict[str, str]] = {}
    for result in results:
        if is_modeless_failure(result):
            hard.append(result.test_name)
            continue
        modes = getattr(result, "modes", None)
        if not modes:
            continue
        path = result_path(result)
        for mode, outcome in modes.items():
            if outcome == HARD_FAIL:
                hard.append(f"{path} [{mode}]")
            elif outcome == JS_FAIL:
                reason = old.entries.get(path, {}).get(mode) or derive_reason(path)
                entries.setdefault(path, {})[mode] = reason
    if hard:
        print(
            "Not updating expectations; these crashed, timed out, failed to load"
            " or hit a plugin internal error:"
        )
        for h in sorted(hard):
            print("  " + h)
        return None
    # Keep entries for tests outside this run's roots, if they still exist.
    for path, modes in old.entries.items():
        if not under_roots(path, roots) and os.path.exists(
            os.path.join(test262_root, path)
        ):
            entries[path] = dict(modes)
    return Expectations(commit, entries)


def checkout_commit(test262_root: str) -> str:
    out = subprocess.run(
        ["git", "-C", test262_root, "rev-parse", "HEAD"],
        capture_output=True,
        text=True,
    )
    return out.stdout.strip() if out.returncode == 0 else ""


PROBE_TIMEOUT_SECONDS = 60


def has_fatal_diagnostics(stderr: str) -> bool:
    """Whether stderr shows a sanitizer report or a Hermes fatal error."""
    return "Sanitizer" in stderr or "LLVM ERROR" in stderr


# What Hermes puts in the message of an Error made from a C++ exception thrown
# by a host function. test262 calls no host functions but the plugin's, so
# this marks an internal error in the plugin, never a missing feature.
HOST_FUNCTION_EXCEPTION = "Exception in HostFunction:"

# The prefix of the message of every error the plugin's own JS throws for a
# broken invariant (intl/js), and of the C++ exceptions its native helpers
# throw (intl/jsi). Like HOST_FUNCTION_EXCEPTION, it marks a plugin bug.
INTL_INTERNAL_ERROR = "Intl internal error"


def probe(hermes: str, plugin: str, work_dir: str) -> Optional[str]:
    """None if the plugin loads into hermes; else what went wrong."""
    path = os.path.join(work_dir, "intl-plugin-probe.js")
    with open(path, "w") as f:
        f.write("print(typeof Intl === 'object' && typeof Intl.getCanonicalLocales);\n")
    try:
        out = subprocess.run(
            [hermes, f"-Xintl-plugin-path={plugin}", path],
            capture_output=True,
            text=True,
            timeout=PROBE_TIMEOUT_SECONDS,
        )
    except subprocess.TimeoutExpired:
        return f"timed out after {PROBE_TIMEOUT_SECONDS} seconds"
    if (
        out.returncode == 0
        and out.stdout.strip() == "function"
        and not has_fatal_diagnostics(out.stderr)
    ):
        return None
    return f"exit {out.returncode}\nstdout: {out.stdout}\nstderr: {out.stderr}"


# The flag normal mode passes to every hermes run.
DISABLE_PLUGIN_FLAG = "-Xintl-plugin=false"


def vm_args(plugin: Optional[str]) -> List[str]:
    """The Intl arguments for every hermes run. In plugin mode (plugin is the
    library's path) they load that plugin with fixed defaults. In normal mode
    (plugin is None) they disable the plugin: normal mode measures the engine
    without Intl, whether or not the build has a plugin next to hermes, so
    its results mean what they meant before the plugin existed."""
    if plugin is None:
        return [DISABLE_PLUGIN_FLAG]
    return [
        f"-Xintl-plugin-path={plugin}",
        "-Xintl-default-locale=en-US",
        "-Xintl-default-time-zone=UTC",
    ]


def args_for_binary(args: List[str], help_text: str) -> List[str]:
    """args, minus the flag that disables the plugin when the binary whose
    -help output is help_text does not know it. Such a binary (an external
    b_hermes from before the plugin) would reject the flag, and it cannot
    load a plugin anyway."""
    if re.search(r"-Xintl-plugin(?![\w-])", help_text):
        return args
    return [a for a in args if a != DISABLE_PLUGIN_FLAG]


# Binary path -> its -help output, probed once per binary.
_help_texts: Dict[str, str] = {}


def binary_help(exe: str) -> str:
    """The -help output of exe, run once and cached; "" if it fails."""
    if exe not in _help_texts:
        try:
            out = subprocess.run(
                [exe, "-help"],
                capture_output=True,
                text=True,
                timeout=PROBE_TIMEOUT_SECONDS,
            )
            _help_texts[exe] = out.stdout + out.stderr
        except (OSError, subprocess.TimeoutExpired):
            _help_texts[exe] = ""
    return _help_texts[exe]


def compile_is_hard_failure(returncode: int, stderr: str) -> bool:
    """Plugin mode: whether a compile step crashed rather than reporting a
    compile error. Hermes reports compile errors with exit codes 1-8."""
    return has_fatal_diagnostics(stderr) or returncode < 0 or returncode > 8


def classify(
    returncode: int,
    stdout: str,
    stderr: str,
    expect_runtime_error: bool,
    is_async: bool,
) -> str:
    """PASS, JS_FAIL or HARD_FAIL for one finished hermes run in plugin mode.

    Only a process that exited normally (0, or 1 for an uncaught exception)
    with no sanitizer report and no fatal error can pass or fail in JS;
    anything else is HARD_FAIL, whatever the test expected. So is a run
    that reports an exception thrown by a plugin host function, or an
    internal error of the plugin's JS, even an expected one: it is a bug in
    the plugin, not a missing feature."""
    if has_fatal_diagnostics(stderr) or returncode not in (0, 1):
        return HARD_FAIL
    for marker in (HOST_FUNCTION_EXCEPTION, INTL_INTERNAL_ERROR):
        if marker in stderr or marker in stdout:
            return HARD_FAIL
    uncaught = "Uncaught " in stderr
    if returncode == 1 and not uncaught:
        return HARD_FAIL
    threw = returncode == 1
    if is_async and not threw:
        if (
            "Test262:AsyncTestFailure" in stdout
            or "Test262:AsyncTestComplete" not in stdout
        ):
            return JS_FAIL
    if threw != expect_runtime_error:
        return JS_FAIL
    return PASS
