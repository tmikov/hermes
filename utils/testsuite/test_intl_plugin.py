# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

"""Unit tests for testsuite/intl_plugin.py. Run with
PYTHONPATH=<repo>/utils python3 -m unittest testsuite.test_intl_plugin"""

import contextlib
import io
import os
import shutil
import tempfile
import types
import unittest

from testsuite.intl_plugin import (
    Expectations,
    HARD_FAIL,
    HEADER,
    JS_FAIL,
    PASS,
    classify,
    compile_is_hard_failure,
    count_outcomes,
    derive_reason,
    evaluate,
    judge,
    parse_expectations,
    render_expectations,
    unrun_entries,
    update,
    vm_args,
    args_for_binary,
    DISABLE_PLUGIN_FLAG,
)
from testsuite.utils import TestCaseResult, TestResultCode


class Result:
    """The parts of TestCaseResult evaluate() reads."""

    def __init__(self, path, modes):
        self.test_name = "test262 :: " + path
        self.modes = modes


# The run's roots in most tests: all of intl402.
ROOTS = ["test/intl402"]


class IntlPluginTest(unittest.TestCase):
    def setUp(self):
        self.root = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, self.root)
        for p in [
            "test/intl402/a.js",
            "test/intl402/b.js",
            "test/intl402/c.js",
            "test/intl402/skipped.js",
            "test/built-ins/x/d.js",
        ]:
            os.makedirs(os.path.dirname(os.path.join(self.root, p)), exist_ok=True)
            open(os.path.join(self.root, p), "w").close()

    def expectations(self, text):
        return parse_expectations("test262-commit: abc\n" + text)

    def test_parse_expands_both(self):
        e = self.expectations("test/intl402/a.js both Phase 2: Intl.Collator\n")
        self.assertEqual(e.commit, "abc")
        self.assertEqual(
            e.entries["test/intl402/a.js"],
            {"strict": "Phase 2: Intl.Collator", "non-strict": "Phase 2: Intl.Collator"},
        )

    def test_expected_js_failure_is_ok(self):
        e = self.expectations("test/intl402/a.js strict why\n")
        problems = evaluate(
            [Result("test/intl402/a.js", {"strict": JS_FAIL})], e, self.root, ROOTS
        )
        self.assertEqual(problems, [])

    def test_hard_failure_never_matches(self):
        e = self.expectations("test/intl402/a.js strict why\n")
        problems = evaluate(
            [Result("test/intl402/a.js", {"strict": HARD_FAIL})], e, self.root, ROOTS
        )
        self.assertEqual(len(problems), 1)
        self.assertIn("crash", problems[0])

    def test_unexpected_pass_fails(self):
        e = self.expectations("test/intl402/a.js both why\n")
        problems = evaluate(
            [Result("test/intl402/a.js", {"strict": PASS, "non-strict": JS_FAIL})],
            e,
            self.root,
            ROOTS,
        )
        self.assertEqual(len(problems), 1)
        self.assertIn("unexpected pass", problems[0])

    def test_unexpected_failure_fails(self):
        e = self.expectations("")
        problems = evaluate(
            [Result("test/intl402/b.js", {"non-strict": JS_FAIL})], e, self.root, ROOTS
        )
        self.assertEqual(len(problems), 1)
        self.assertIn("unexpected failure", problems[0])

    def test_stale_and_unexplained_entries_fail(self):
        e = self.expectations(
            "test/intl402/gone.js both why\ntest/intl402/c.js both UNEXPLAINED\n"
        )
        # No roots: the run covered nothing, so c.js not running is fine.
        problems = evaluate([], e, self.root, [])
        self.assertEqual(len(problems), 2)

    def test_unrun_entries_fail(self):
        e = self.expectations(
            "test/intl402/a.js both ran only in strict mode\n"
            "test/intl402/c.js strict ran\n"
            "test/intl402/skipped.js both skipped\n"
            "test/intl402/gone.js both deleted\n"
            "test/built-ins/x/d.js both outside the roots\n"
        )
        results = [
            Result("test/intl402/a.js", {"strict": JS_FAIL}),
            Result("test/intl402/c.js", {"strict": JS_FAIL}),
        ]
        self.assertEqual(
            unrun_entries(results, e, ROOTS),
            [
                ("test/intl402/a.js", "non-strict"),
                ("test/intl402/gone.js", "non-strict"),
                ("test/intl402/gone.js", "strict"),
                ("test/intl402/skipped.js", "non-strict"),
                ("test/intl402/skipped.js", "strict"),
            ],
        )
        # A deleted test is reported as missing, not also as not run.
        self.assertEqual(
            evaluate(results, e, self.root, ROOTS),
            [
                "expectation for a missing test: test/intl402/gone.js",
                "expectation for a test that did not run in that mode: "
                "test/intl402/a.js [non-strict]",
                "expectation for a test that did not run in that mode: "
                "test/intl402/skipped.js [non-strict]",
                "expectation for a test that did not run in that mode: "
                "test/intl402/skipped.js [strict]",
            ],
        )
        # A root can be one file, or the whole checkout.
        self.assertEqual(
            unrun_entries(results, e, ["test/intl402/a.js"]),
            [("test/intl402/a.js", "non-strict")],
        )
        self.assertIn(
            ("test/built-ins/x/d.js", "strict"), unrun_entries(results, e, ["."])
        )
        # "test/intl4" is not a directory prefix of test/intl402/.
        self.assertEqual(unrun_entries(results, e, ["test/intl4"]), [])

    def test_render_round_trips_and_merges_modes(self):
        e = Expectations(
            "abc",
            {
                "test/intl402/b.js": {"strict": "r1", "non-strict": "r1"},
                "test/intl402/a.js": {"strict": "r2"},
            },
        )
        text = render_expectations(e)
        self.assertIn("test/intl402/b.js both r1", text)
        self.assertLess(text.index("a.js"), text.index("b.js"))
        self.assertEqual(parse_expectations(text).entries, e.entries)

    def test_classify(self):
        ok, uncaught = "", "Uncaught Test262Error: boom\n"
        asan = "==1==ERROR: AddressSanitizer: heap-use-after-free\n"
        self.assertEqual(classify(0, "", ok, False, False), PASS)
        self.assertEqual(classify(1, "", uncaught, False, False), JS_FAIL)
        self.assertEqual(classify(1, "", uncaught, True, False), PASS)
        self.assertEqual(classify(0, "", ok, True, False), JS_FAIL)
        # A sanitizer report exits 1 too; it never passes, even as an
        # expected runtime error, and is never a JS failure.
        self.assertEqual(classify(1, "", asan, True, False), HARD_FAIL)
        self.assertEqual(classify(1, "", asan + uncaught, False, False), HARD_FAIL)
        self.assertEqual(classify(0, "", asan, False, False), HARD_FAIL)
        self.assertEqual(classify(-6, "", "LLVM ERROR: x\n", True, False), HARD_FAIL)
        self.assertEqual(classify(1, "", "some other error\n", True, False), HARD_FAIL)
        self.assertEqual(classify(2, "", ok, False, False), HARD_FAIL)
        self.assertEqual(classify(0, "Test262:AsyncTestComplete\n", ok, False, True), PASS)
        self.assertEqual(classify(0, "Test262:AsyncTestFailure:x\n", ok, False, True), JS_FAIL)
        self.assertEqual(classify(0, "", ok, False, True), JS_FAIL)
        # A C++ exception from a plugin host function is an internal error,
        # never an ordinary JS failure, whether it was uncaught or reported
        # by the async harness.
        host = "Uncaught Error: Exception in HostFunction: Intl internal error: x\n"
        self.assertEqual(classify(1, "", host, False, False), HARD_FAIL)
        self.assertEqual(classify(1, "", host, True, False), HARD_FAIL)
        self.assertEqual(
            classify(
                0,
                "Test262:AsyncTestFailure:Error: Exception in HostFunction: x\n",
                ok,
                False,
                True,
            ),
            HARD_FAIL,
        )
        # So is an internal error thrown by the plugin's JS, which is a
        # TypeError, whether it was uncaught or reported by the async
        # harness.
        js = "Uncaught TypeError: Intl internal error: ill-formed locale x\n"
        self.assertEqual(classify(1, "", js, False, False), HARD_FAIL)
        self.assertEqual(classify(1, "", js, True, False), HARD_FAIL)
        self.assertEqual(
            classify(
                0,
                "Test262:AsyncTestFailure:TypeError: Intl internal error: x\n",
                ok,
                False,
                True,
            ),
            HARD_FAIL,
        )

    def test_compile_outcome(self):
        asan = "==1==ERROR: AddressSanitizer: stack-overflow\n"
        self.assertTrue(compile_is_hard_failure(1, asan))
        self.assertTrue(compile_is_hard_failure(0, asan))
        self.assertTrue(compile_is_hard_failure(-11, ""))
        self.assertFalse(compile_is_hard_failure(1, "error: unexpected token\n"))
        self.assertFalse(compile_is_hard_failure(0, ""))

    def test_probe_rejects_hangs_and_diagnostics(self):
        import stat

        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        d = tmp.name
        hang = os.path.join(d, "hang")
        with open(hang, "w") as f:
            # exec, so the timeout kills sleep itself, not just the shell.
            f.write("#!/bin/sh\nexec sleep 1000\n")
        noisy = os.path.join(d, "noisy")
        with open(noisy, "w") as f:
            f.write("#!/bin/sh\necho function\necho '==1==ERROR: AddressSanitizer: x' >&2\n")
        good = os.path.join(d, "good")
        with open(good, "w") as f:
            f.write("#!/bin/sh\necho function\n")
        for x in (hang, noisy, good):
            os.chmod(x, stat.S_IRWXU)
        import testsuite.intl_plugin as ip

        saved = ip.PROBE_TIMEOUT_SECONDS
        ip.PROBE_TIMEOUT_SECONDS = 1
        try:
            self.assertIn("timed out", ip.probe(hang, "p.so", d))
        finally:
            ip.PROBE_TIMEOUT_SECONDS = saved
        self.assertIsNotNone(ip.probe(noisy, "p.so", d))
        self.assertIsNone(ip.probe(good, "p.so", d))

    def test_judge_one_result(self):
        e = self.expectations("test/intl402/a.js strict why\n")
        ok = Result("test/intl402/a.js", {"strict": JS_FAIL, "non-strict": PASS})
        self.assertEqual(judge(ok, e), [])
        bad = Result("test/intl402/b.js", {"strict": JS_FAIL, "non-strict": HARD_FAIL})
        self.assertEqual(len(judge(bad, e)), 2)
        # When updating, only the hard failure is a problem.
        self.assertEqual(len(judge(bad, e, updating=True)), 1)
        self.assertIn("crash", judge(bad, e, updating=True)[0])

    def test_update(self):
        old = self.expectations(
            "test/intl402/a.js both hand-written reason\n"
            "test/intl402/c.js both ran only in strict mode\n"
            "test/intl402/skipped.js strict skipped this time\n"
            "test/built-ins/x/d.js strict not run this time\n"
            "test/intl402/gone.js both deleted test\n"
        )
        results = [
            Result("test/intl402/a.js", {"strict": JS_FAIL, "non-strict": PASS}),
            Result("test/intl402/b.js", {"strict": JS_FAIL, "non-strict": JS_FAIL}),
            Result("test/intl402/c.js", {"strict": JS_FAIL}),
        ]
        new = update(results, old, self.root, "def", ROOTS)
        self.assertEqual(new.commit, "def")
        # An existing reason is kept, and a mode that now passes is dropped.
        self.assertEqual(new.entries["test/intl402/a.js"], {"strict": "hand-written reason"})
        # A new failure outside the known phases is UNEXPLAINED.
        self.assertEqual(
            new.entries["test/intl402/b.js"],
            {"strict": "UNEXPLAINED", "non-strict": "UNEXPLAINED"},
        )
        # Within the roots, a mode the test did not run in is dropped, and so
        # is a test that did not run at all.
        self.assertEqual(
            new.entries["test/intl402/c.js"], {"strict": "ran only in strict mode"}
        )
        self.assertNotIn("test/intl402/skipped.js", new.entries)
        # A test outside the roots keeps its entry if it still exists.
        self.assertEqual(
            new.entries["test/built-ins/x/d.js"], {"strict": "not run this time"}
        )
        self.assertNotIn("test/intl402/gone.js", new.entries)
        # Against the same run, the new entries' only problem is the
        # UNEXPLAINED reason.
        self.assertEqual(
            evaluate(results, new, self.root, ROOTS),
            ["expectation without a reason: test/intl402/b.js"],
        )
        # A hard failure anywhere means nothing is written.
        results.append(Result("test/intl402/c.js", {"strict": HARD_FAIL}))
        self.assertIsNone(update(results, old, self.root, "def", ROOTS))

    def test_update_with_unexplained_entries_writes_and_fails(self):
        from testsuite.cli import intl_verdict

        out_file = os.path.join(self.root, "expectations.txt")

        def verdict(old_text, results):
            intl = {
                "expectations": self.expectations(old_text),
                "file": out_file,
                "update": True,
                "root": self.root,
                "roots": ROOTS,
                "commit": "def",
            }
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                code = intl_verdict(results, intl)
            with open(out_file) as f:
                return code, stdout.getvalue(), f.read()

        # Every entry has a reason: written, exit 0.
        code, out, written = verdict(
            "", [Result("test/intl402/a.js", {"strict": PASS, "non-strict": PASS})]
        )
        self.assertEqual(code, 0)
        self.assertNotIn("UNEXPLAINED", out)
        # A new failure with no phase, and a retained entry outside the run's
        # roots, are both UNEXPLAINED: the file is still written, every such
        # entry is listed, and the exit code is 1.
        code, out, written = verdict(
            "test/built-ins/x/d.js strict UNEXPLAINED\n"
            "test/intl402/a.js both explained\n",
            [
                Result(
                    "test/intl402/b.js", {"strict": JS_FAIL, "non-strict": JS_FAIL}
                ),
                Result("test/intl402/a.js", {"strict": JS_FAIL, "non-strict": PASS}),
            ],
        )
        self.assertEqual(code, 1)
        self.assertIn("test/intl402/b.js both UNEXPLAINED", written)
        self.assertIn("test/built-ins/x/d.js strict UNEXPLAINED", written)
        self.assertIn("test/intl402/b.js [strict]", out)
        self.assertIn("test/intl402/b.js [non-strict]", out)
        self.assertIn("test/built-ins/x/d.js [strict]", out)
        self.assertNotIn("test/intl402/a.js [", out)

    def test_regenerating_keeps_only_the_header_comments(self):
        text = (
            HEADER
            + "# A hand-written note.\n"
            + "test262-commit: abc\n"
            + "test/intl402/a.js both reason\n"
        )
        out = render_expectations(parse_expectations(text))
        self.assertTrue(out.startswith(HEADER))
        self.assertNotIn("hand-written note", out)
        # The header says so.
        header_text = " ".join(HEADER.replace("#", " ").split())
        self.assertIn("keeps no other comment", header_text)

    def test_modeless_failure_is_a_hard_failure(self):
        # A result that failed before the per-mode runs has no modes. It is
        # a problem everywhere: in judge, evaluate, the counts, update and
        # the progress display.
        from testsuite.cli import intl_display_result

        e = self.expectations("")
        failed = TestCaseResult(
            "test262 :: test/intl402/a.js", TestResultCode.TEST_FAILED, "boom", "out"
        )
        self.assertEqual(len(judge(failed, e)), 1)
        self.assertIn("boom", judge(failed, e)[0])
        self.assertEqual(len(judge(failed, e, updating=True)), 1)
        self.assertEqual(len(evaluate([failed], e, self.root, ROOTS)), 1)
        self.assertEqual(count_outcomes([failed])[HARD_FAIL], 1)
        self.assertIsNone(update([failed], e, self.root, "abc", ROOTS))
        shown = intl_display_result(failed, {"expectations": e, "update": False})
        self.assertTrue(shown.code.is_failure)
        self.assertEqual(shown.output, "out")
        # Mode-less passes and skips stay quiet.
        for code in (
            TestResultCode.TEST_PASSED,
            TestResultCode.TEST_SKIPPED,
            TestResultCode.TEST_PERMANENTLY_SKIPPED,
        ):
            r = TestCaseResult("test262 :: test/intl402/a.js", code)
            self.assertEqual(judge(r, e), [])
            self.assertEqual(evaluate([r], e, self.root, ROOTS), [])
            shown = intl_display_result(r, {"expectations": e, "update": False})
            self.assertFalse(shown.code.is_failure)

    def test_vm_args(self):
        # Normal mode turns the plugin off; plugin mode loads the given one.
        self.assertIn("-Xintl-plugin=false", vm_args(None))
        plugin_args = vm_args("/p/libhermesintl.so")
        self.assertNotIn("-Xintl-plugin=false", plugin_args)
        self.assertIn("-Xintl-plugin-path=/p/libhermesintl.so", plugin_args)

    def test_args_for_binary(self):
        # A binary that knows -Xintl-plugin gets the flag; one from before
        # the plugin (only -Xintl, the old Intl) does not.
        args = ["file.hbc", "-b", DISABLE_PLUGIN_FLAG]
        new_help = (
            "  -Xintl                - Enable support for ECMA-402 Intl APIs\n"
            "  -Xintl-plugin         - Load the Intl plugin when one is available\n"
            "  -Xintl-plugin-path=<path> - Load the Intl plugin from this library\n"
        )
        old_help = "  -Xintl                - Enable support for ECMA-402 Intl APIs\n"
        self.assertEqual(args_for_binary(args, new_help), args)
        self.assertEqual(args_for_binary(args, old_help), ["file.hbc", "-b"])
        self.assertEqual(args_for_binary(args, ""), ["file.hbc", "-b"])

    def test_load_expectations(self):
        from testsuite.cli import intl_load_expectations

        def load(path, updating):
            args = types.SimpleNamespace(
                intl_expectations=path, intl_update_expectations=updating
            )
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                e = intl_load_expectations(args)
            return e, out.getvalue()

        def write(name, text):
            path = os.path.join(self.root, name)
            with open(path, "w") as f:
                f.write(text)
            return path

        missing = os.path.join(self.root, "missing.txt")
        # A missing file starts out empty when updating...
        e, out = load(missing, True)
        self.assertEqual((e.commit, e.entries, out), ("", {}, ""))
        # ...and is an error otherwise.
        e, out = load(missing, False)
        self.assertIsNone(e)
        self.assertIn("Cannot read " + missing, out)
        # So is a file that cannot be read, even when updating.
        e, out = load(self.root, True)
        self.assertIsNone(e)
        self.assertIn("Cannot read " + self.root, out)
        # A line that does not parse, or a missing commit line, is an error
        # that names the file, even when updating.
        bad = write("bad.txt", "test262-commit: abc\ntest/intl402/a.js sometimes x\n")
        for updating in (False, True):
            e, out = load(bad, updating)
            self.assertIsNone(e)
            self.assertIn(bad + ": expectations line 2: cannot parse", out)
        e, out = load(write("nocommit.txt", "test/intl402/a.js both x\n"), False)
        self.assertIsNone(e)
        self.assertIn("missing 'test262-commit:' line", out)
        # A good file parses.
        good = write("good.txt", "test262-commit: abc\ntest/intl402/a.js strict x\n")
        e, out = load(good, False)
        self.assertEqual(
            (e.commit, e.entries, out),
            ("abc", {"test/intl402/a.js": {"strict": "x"}}, ""),
        )

    def test_derive_reason(self):
        self.assertEqual(
            derive_reason("test/intl402/Collator/x.js"), "Phase 2: Intl.Collator"
        )
        self.assertEqual(derive_reason("test/intl402/Intl/x.js"), "UNEXPLAINED")


if __name__ == "__main__":
    unittest.main()
