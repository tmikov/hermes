#!/usr/bin/env python3
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

"""Mutation survey for `hermes_sema::check_implicit_return`.

Re-derives the witness tables quoted in
`rust/crates/sema/tests/sema_corpus/MANIFEST.md` and
`rust/crates/sema/tests/sema_corpus_parser/MANIFEST.md`.

See README.md in this directory for what the numbers mean and how to read a
run. Usage:

    python3 rust/crates/sema/tests/implicit_return_survey/survey.py

Options:
    --only M1,M14,MATCH-C   run just those mutations
    --keep-going            do not stop when a mutation fails to build

The survey mutates `rust/crates/sema/src/check_implicit_return.rs` in place,
one decision at a time, rebuilding `sema-dump` for each and re-running it over
both differential corpora, then restores the file. It refuses to start if that
file already has uncommitted changes, and restores it from the on-disk copy it
saved (not from git) on any exit path, including Ctrl-C.

Rust-vs-Rust is enough: the clean port matches the C++ oracle byte-for-byte on
every corpus file, so "differs from the clean port" and "differs from the
oracle" are the same question. That is what makes this harness runnable with
no C++ build present.
"""

import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
# <repo>/rust/crates/sema/tests/implicit_return_survey -> <repo>
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", "..", "..", ".."))
TARGET = os.path.join(ROOT, "rust/crates/sema/src/check_implicit_return.rs")
MANIFEST = os.path.join(ROOT, "rust/Cargo.toml")
CORPORA = [
    # (label, corpus dir, extra argv for `sema-dump`)
    ("driver", os.path.join(ROOT, "rust/crates/sema/tests/sema_corpus"), []),
    (
        "parser",
        os.path.join(ROOT, "rust/crates/sema/tests/sema_corpus_parser"),
        ["--parser-entry"],
    ),
]

# ---------------------------------------------------------------------------
# The mutation catalogue.
#
# Each entry is (id, description, anchor, replacement). `anchor` must occur
# EXACTLY ONCE in check_implicit_return.rs; the survey aborts if it does not,
# rather than silently reporting a zero — a mutation that no longer applies is
# a catalogue bug, not a coverage result.
#
# The ids match the tables in both MANIFESTs: M1-M18 are the task-2 survey,
# M19-M21 the three decisions upstream `5ae5260c8` (try-catch-finally) adds,
# MATCH-A..F the task-3 survey of `653e49c60`'s Flow-`match` arm. M18 and
# THROW are controls (known-caught before either survey).
# ---------------------------------------------------------------------------
MUTATIONS = [
    (
        "M1",
        "if-without-else: the kNextStatementLabel insert",
        """                } else {
                    // No alternate, so this statement can also continue.
                    consequent_res
                        .target_labels
                        .insert(TerminationResult::K_NEXT_STATEMENT_LABEL);
                }""",
        """                } else {
                }""",
    ),
    (
        "M2",
        "if-with-else: the alternate's target labels",
        """                    let alternate_res = self.check_termination(alternate);
                    consequent_res
                        .target_labels
                        .extend(alternate_res.target_labels);""",
        """                    let _alternate_res = self.check_termination(alternate);""",
    ),
    (
        "M3",
        "do-while's must_execute = true",
        """            Node::DoWhileStatement(n) => self
                .check_termination_loop_or_labeled_statement(
                    n.label_index.get(),
                    n.body,
                    true,
                ),""",
        """            Node::DoWhileStatement(n) => self
                .check_termination_loop_or_labeled_statement(
                    n.label_index.get(),
                    n.body,
                    false,
                ),""",
    ),
    (
        "M4",
        "labeled statement's must_execute = true",
        """            Node::LabeledStatement(n) => self
                .check_termination_loop_or_labeled_statement(
                    n.label_index.get(),
                    n.body,
                    true,
                ),""",
        """            Node::LabeledStatement(n) => self
                .check_termination_loop_or_labeled_statement(
                    n.label_index.get(),
                    n.body,
                    false,
                ),""",
    ),
    (
        "M5",
        "loop: break-to-own-label becomes a continuation",
        """        if body_res.target_labels.remove(&label_index) {
            // Breaks within this labeled statement are continues after it.
            may_execute_next_statement = true;
        }""",
        """        body_res.target_labels.remove(&label_index);""",
    ),
    (
        "M6",
        "loop: the next-statement label insert",
        """        if may_execute_next_statement {
            body_res
                .target_labels
                .insert(TerminationResult::K_NEXT_STATEMENT_LABEL);
        }
        body_res""",
        """        let _ = may_execute_next_statement;
        body_res""",
    ),
    (
        "M7",
        "statement list: the fall-through erase",
        """            // Check for continuation from previous statement and erase the
            // continue, because this is the continuation.
            result
                .target_labels
                .remove(&TerminationResult::K_NEXT_STATEMENT_LABEL);

            // Add all the possible target labels to the final result.""",
        """            // Add all the possible target labels to the final result.""",
    ),
    (
        "M8",
        "statement list: the ran-off-the-end label",
        """        // Made it through the whole statement list.
        result
            .target_labels
            .insert(TerminationResult::K_NEXT_STATEMENT_LABEL);
        result
    }""",
        """        // Made it through the whole statement list.
        result
    }""",
    ),
    (
        "M9",
        "statement list: stop scanning after a terminator",
        """            if !may_execute_next_statement {
                // Statement list doesn't continue, so we're done scanning
                // it.
                return result;
            }""",
        """            let _ = may_execute_next_statement;""",
    ),
    (
        "M10",
        "switch: found_default",
        """            if switch_case.test.is_none() {
                found_default = true;
            }""",
        """            if switch_case.test.is_none() {}""",
    ),
    (
        "M11",
        "switch: found_explicit_break",
        """        let found_explicit_break =
            result.target_labels.remove(&node.label_index.get());""",
        """        result.target_labels.remove(&node.label_index.get());
        let found_explicit_break = false;""",
    ),
    (
        "M12",
        "switch: the past-the-switch label insert",
        """        if found_explicit_break || !found_default {
            result
                .target_labels
                .insert(TerminationResult::K_NEXT_STATEMENT_LABEL);
        }""",
        """        let _ = (found_explicit_break, found_default);""",
    ),
    (
        "M13",
        "switch: the per-case fall-through erase",
        """            // Check for fallthrough from previous case and erase the
            // continue, because this is the continuation.
            result
                .target_labels
                .remove(&TerminationResult::K_NEXT_STATEMENT_LABEL);

            let switch_case = child""",
        """            let switch_case = child""",
    ),
    (
        "M14",
        "try/catch: the catch clause's target labels",
        """            let catch_res = self.check_termination(catch_clause.body);
            inner_res.target_labels.extend(catch_res.target_labels);""",
        """            let _catch_res = self.check_termination(catch_clause.body);""",
    ),
    (
        "M15",
        "try/finally: the terminating-finally shortcut",
        """        if finally_res.must_terminate() {
            // If the finally block terminates, the try-finally will
            // terminate after executing the finally.
            return finally_res;
        }""",
        """        if false && finally_res.must_terminate() {
            return finally_res;
        }""",
    ),
    (
        "M16",
        "try/finally: the terminating-try shortcut",
        """        if try_res.must_terminate() && finally_res.must_execute_next_statement()
        {""",
        """        if false
            && try_res.must_terminate()
            && finally_res.must_execute_next_statement()
        {""",
    ),
    (
        "M17",
        "`continue` is not terminating",
        """                TerminationResult::make_single_label(n.label_index.get())
            }
            Node::BreakStatement(n) => {""",
        """                let _ = n;
                TerminationResult::make_must_terminate()
            }
            Node::BreakStatement(n) => {""",
    ),
    (
        "M18",
        "(control) `return` is terminating",
        """            Node::ReturnStatement(_) => {
                // Explicit return will always prevent implicit return.
                TerminationResult::make_must_terminate()
            }""",
        """            Node::ReturnStatement(_) => {
                TerminationResult::make_next_statement()
            }""",
    ),
    (
        "THROW",
        "(control) `throw` is terminating",
        """            Node::ThrowStatement(_) => {
                // Throw will prevent the next statement in the current list
                // from executing. It's possible it will result in execution
                // of a catch or finally in this function, but that is
                // handled at the TryStatement level.
                TerminationResult::make_must_terminate()
            }""",
        """            Node::ThrowStatement(_) => {
                TerminationResult::make_next_statement()
            }""",
    ),
    (
        "M19",
        "try-catch-finally: the finalizer half (pre-5ae5260c8 release behavior)",
        """        let Some(finalizer) = node.finalizer else {
            return inner_res;
        };""",
        """        if node.handler.is_some() {
            return inner_res;
        }
        let Some(finalizer) = node.finalizer else {
            return inner_res;
        };""",
    ),
    (
        "M20",
        "try-catch-finally: the handler half",
        """        if let Some(handler) = node.handler {""",
        """        if let Some(handler) = node.handler.filter(|_| node.finalizer.is_none())
        {""",
    ),
    (
        "M21",
        "try-catch-finally: handler-then-finalizer order",
        """        let mut inner_res = self.check_termination(node.block);
        if let Some(handler) = node.handler {
            // Both the try and catch must be terminating for the pair to
            // terminate.
            let catch_clause = handler
                .as_catch_clause()
                .expect("a TryStatement handler is a CatchClause");
            let catch_res = self.check_termination(catch_clause.body);
            inner_res.target_labels.extend(catch_res.target_labels);
        }""",
        """        let mut inner_res = self.check_termination(node.block);
        if let (Some(handler), Some(finalizer)) = (node.handler, node.finalizer)
        {
            let catch_clause = handler
                .as_catch_clause()
                .expect("a TryStatement handler is a CatchClause");
            let catch_res = self.check_termination(catch_clause.body);
            let mut fin_res =
                self.check_termination_finalizer(inner_res, finalizer);
            fin_res.target_labels.extend(catch_res.target_labels);
            return fin_res;
        }
        if let Some(handler) = node.handler {
            let catch_clause = handler
                .as_catch_clause()
                .expect("a TryStatement handler is a CatchClause");
            let catch_res = self.check_termination(catch_clause.body);
            inner_res.target_labels.extend(catch_res.target_labels);
        }""",
    ),
    (
        "MATCH-A",
        "match: the MatchStatement arm removed entirely (pre-653e49c60)",
        """            Node::MatchStatement(n) => {
                self.check_termination_match_statement(n)
            }""",
        """            Node::MatchStatement(n) if false => {
                self.check_termination_match_statement(n)
            }""",
    ),
    (
        "MATCH-B",
        "match: the arm reduced to make_next_statement (pre-fix release)",
        """            Node::MatchStatement(n) => {
                self.check_termination_match_statement(n)
            }""",
        """            Node::MatchStatement(n) => {
                let _ = n;
                TerminationResult::make_next_statement()
            }""",
    ),
    (
        "MATCH-C",
        "match: is_irrefutable_match_pattern always false",
        """    fn is_irrefutable_match_pattern(pattern: &Node) -> bool {""",
        """    fn is_irrefutable_match_pattern(pattern: &Node) -> bool {
        if true {
            let _ = pattern;
            return false;
        }""",
    ),
    (
        "MATCH-D",
        "match: the !guard half of the irrefutable test",
        """            if match_case.guard.is_none()
                && Self::is_irrefutable_match_pattern(match_case.pattern)
            {""",
        """            if Self::is_irrefutable_match_pattern(match_case.pattern) {""",
    ),
    (
        "MATCH-E",
        "match: the break after the first irrefutable case",
        """                found_irrefutable = true;
                // Cases are tested in order and the first match wins, so no
                // later case can run. Stop instead of unioning labels that a
                // dead case targets, which would report control flow that
                // cannot happen.
                break;""",
        """                found_irrefutable = true;""",
    ),
    (
        "MATCH-F",
        "match: the MatchAsPattern unwrap loop",
        """        let mut pattern = pattern;
        while let Node::MatchAsPattern(as_pattern) = pattern {
            pattern = as_pattern.pattern;
        }""",
        """        let pattern = pattern;""",
    ),
]


def sh(cmd, **kw):
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, **kw)


def per_file_flags(path):
    """The corpus `// FLAGS: <args>` convention: first line only."""
    with open(path, "rb") as f:
        first = f.readline().decode("utf-8", "replace").rstrip("\n")
    m = re.match(r"^// FLAGS:(.*)$", first)
    return m.group(1).split() if m else []


def build():
    """Build `sema-dump`; return its path, or None with the log on failure."""
    r = sh(
        [
            "cargo",
            "build",
            "--manifest-path",
            MANIFEST,
            "-p",
            "tools",
            "--bin",
            "sema-dump",
        ]
    )
    if r.returncode != 0:
        return None, r.stderr.decode("utf-8", "replace")
    return os.path.join(ROOT, "rust/target/debug/sema-dump"), ""


def run_corpus(binary, corpus_dir, extra):
    """Run `binary` over every .js in `corpus_dir`; return {name: triple}."""
    out = {}
    for name in sorted(os.listdir(corpus_dir)):
        if not name.endswith(".js"):
            continue
        p = os.path.join(corpus_dir, name)
        r = subprocess.run(
            [binary] + extra + per_file_flags(p) + [p],
            capture_output=True,
        )
        out[name] = (r.returncode, r.stdout, r.stderr)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", default="")
    ap.add_argument("--keep-going", action="store_true")
    args = ap.parse_args()

    wanted = set(x.strip() for x in args.only.split(",") if x.strip())
    todo = [m for m in MUTATIONS if not wanted or m[0] in wanted]
    unknown = wanted - {m[0] for m in MUTATIONS}
    if unknown:
        sys.exit(f"unknown mutation id(s): {sorted(unknown)}")

    dirty = sh(["git", "status", "--porcelain", "--", TARGET]).stdout
    if dirty.strip():
        sys.exit(
            "check_implicit_return.rs has uncommitted changes; the survey "
            "rewrites it in place and will not run over unsaved work."
        )

    clean_src = open(TARGET).read()
    # Anchor check FIRST, before anything is built or written: a catalogue
    # that no longer matches the source must fail loudly, not report zeros.
    for mid, desc, anchor, _ in todo:
        n = clean_src.count(anchor)
        if n != 1:
            sys.exit(
                f"mutation {mid} ({desc}): anchor matches {n} times, "
                "expected exactly 1 — the catalogue is stale, update it "
                "against check_implicit_return.rs before trusting any run."
            )

    backup = tempfile.NamedTemporaryFile(
        "w", suffix=".rs", delete=False, prefix="check_implicit_return-"
    )
    backup.write(clean_src)
    backup.close()

    results = {}
    try:
        binary, log = build()
        if binary is None:
            sys.exit("clean build failed:\n" + log)
        baseline = [
            (label, run_corpus(binary, d, extra)) for label, d, extra in CORPORA
        ]
        sizes = {label: len(b) for label, b in baseline}
        print(
            "baseline: "
            + ", ".join(f"{label} {n} files" for label, n in sizes.items())
        )

        for mid, desc, anchor, repl in todo:
            open(TARGET, "w").write(clean_src.replace(anchor, repl))
            binary, log = build()
            if binary is None:
                msg = "BUILD FAILED"
                print(f"{mid:8s} {msg}")
                if not args.keep_going:
                    sys.exit(f"mutation {mid} did not compile:\n{log}")
                results[mid] = None
                continue
            row = {}
            for (label, corpus_dir, extra), (_, base) in zip(
                CORPORA, baseline
            ):
                cur = run_corpus(binary, corpus_dir, extra)
                row[label] = sum(1 for k in base if base[k] != cur.get(k))
            results[mid] = row
            print(
                f"{mid:8s} "
                + "  ".join(f"{label}={row[label]}" for label in row)
                + f"   {desc}"
            )
    finally:
        shutil.copyfile(backup.name, TARGET)
        os.unlink(backup.name)
        # Leave a usable binary behind rather than the last mutant.
        build()

    print()
    print("| # | Decision deleted / inverted | driver | parser |")
    print("|---|---|---|---|")
    for mid, desc, _, _ in todo:
        row = results.get(mid)
        if row is None:
            print(f"| {mid} | {desc} | build failed | build failed |")
            continue
        def fmt(n):
            return f"**{n}**" if n == 0 else str(n)
        print(f"| {mid} | {desc} | {fmt(row['driver'])} | {fmt(row['parser'])} |")
    zeros = [
        mid
        for mid, row in results.items()
        if row and row["driver"] == 0 and row["parser"] == 0
    ]
    print()
    print(
        "decisions with ZERO witnesses in either corpus: "
        + (", ".join(zeros) if zeros else "none")
    )
    return 1 if zeros else 0


if __name__ == "__main__":
    sys.exit(main())
