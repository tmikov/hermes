# Squash the port onto `origin/static_h`

> **DONE 2026-08-14; historical record — do not re-run.** `rust` is now 12
> commits above `origin/static_h` @ `14112ce36`; the pre-squash history is at
> tag `rust-history-2026-08-14` / branch `rust-presquash-backup`, both
> `550c5db8f`. Four of this plan's "Verified facts" were superseded during
> execution and the text below is left as written:
>
> 1. `origin/static_h` fast-forwarded to **`14112ce36`**, not `2d3e9018b`;
>    that is the base actually used.
> 2. `5ae5260c8` **landed upstream as `594e9c6a1`**, so `CheckImplicitReturn.cpp`
>    is upstream's verbatim and the tree does not run ahead of upstream.
> 3. `CompilerDriver.cpp`'s `result = InvalidFlags;` was never ours — it is
>    upstream's own line, deleted upstream in `ce5efcd53` to fix a clang17
>    `-Wunreachable-code -Werror` break. Taken as deleted.
> 4. So the "three local C++ deltas" of Task 2 step 10 / Task 4 step 2 are in
>    reality **one**: the `SemanticResolver.cpp` cosmetic re-wrap.
>
> Also: `doc/` is not wholly ours (9 upstream paths), and 13 test files were
> added, not 15. Current state: `doc/superpowers/UpstreamSyncState.md`.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace 552 chronological, largely unreviewable commits with **~11
commits organised by subsystem**, based directly on `origin/static_h`. The
current branch is preserved as a backup.

**Architecture:** This is a **content-based reconstruction, not a rebase.**
Rebasing 552 commits would be conflict hell and pointless when the result is
squashed anyway. Instead: branch from `origin/static_h`, then materialise our
content in curated commits by checking paths out of the old branch. Correctness
is guaranteed by a **tree-equality invariant** (below), not by trusting the
process.

**Tech Stack:** git, CMake/Ninja (ASan+Debug+clang), the Rust workspace and its
differential harnesses.

## THE SAFETY INVARIANT (the whole plan rests on this)

After reconstruction, for every path:

- **Paths we own** — `rust/`, `doc/`, `tools/{js-lexer-dump,json-parse-dump,preparse-dump,sema-parser-dump,parse-bench}/`,
  `tools/CMakeLists.txt`, `.github/workflows/rust*.yml`, `.gitignore`,
  `unsupported/juno/crates/command_line/src/{opt,parser}.rs`,
  `unsupported/juno/crates/juno_support/src/scoped_hashmap.rs`,
  `lib/Sema/CheckImplicitReturn.cpp`, `lib/Sema/SemanticResolver.cpp`,
  `lib/CompilerDriver/CompilerDriver.cpp`, and the 15 test files we added —
  the new tree must be **byte-identical to old `rust`**.
- **Every other path** — the new tree must be **byte-identical to
  `origin/static_h`**.

Both halves are mechanically checkable and Task 4 checks them. Any deviation is
a defect, not a judgement call.

## Global Constraints

- **NEVER `cd`.** `git -C /home/tmikov/work/hermes-rust …`, absolute paths.
- **Preserve the old branch.** Before anything: a branch **and** an annotated
  tag, both pointing at today's `rust` tip. Never delete them in this plan.
- **Do not resolve conflicts by hand-editing content.** There should be no
  content conflicts — we are checking paths out wholesale.
- **`origin/static_h` is the base.** Do not merge, do not cherry-pick the 19
  already-landed fixes (they are in `origin/static_h` already — verified).
- **USER DECISIONS (locked):** ~10 subsystem commits (list in Task 2); keep
  both incidental C++ diffs as an explicit "local C++ deltas" commit.
- Commit trailers on every commit:
  `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>` and
  `Claude-Session: https://claude.ai/code/session_01ERsmFoVAnZCRwfapbPMibv`

## Verified facts (do not re-derive; do re-check before relying on them)

- Fork point `60b5c73db`; `origin/static_h` = `2d3e9018b`; 147 commits between.
- **11 of our 14 modified C++ source files are already in `origin/static_h`.**
  The three that differ: `CheckImplicitReturn.cpp` (the unlanded `5ae5260c8`),
  `SemanticResolver.cpp` (3-line cosmetic re-wrap), `CompilerDriver.cpp`
  (`+result = InvalidFlags;` after an `llvm_unreachable`).
- The 15 `test/` files we added are **also already in `origin/static_h`** —
  except any that came with `5ae5260c8`. Verify per-file (Task 1 Step 3).
- Files under `tools/hermes-parser/js/flow-api-translator/` that appear
  "missing" upstream are **upstream's own deletions**, not our content. Take
  upstream's state; do not resurrect them.
- The 5 oracle tools + 2 `rust*.yml` workflows are genuinely ours.

---

### Task 1: Backup, base branch, and a precise manifest

**Files:** none modified; produces refs and a manifest file.

- [ ] **Step 1: Back up, twice**

```bash
git -C /home/tmikov/work/hermes-rust branch rust-preswquash-backup rust
git -C /home/tmikov/work/hermes-rust tag -a rust-history-2026-08-14 rust \
  -m "Full chronological history of the Rust port before the subsystem squash"
```
(Fix the branch-name typo — use `rust-presquash-backup`.) Record both hashes in
your report. Verify `git log --oneline | wc -l` on the backup matches the
pre-existing tip.

- [ ] **Step 2: Confirm the base is what we think**

Verify `origin/static_h` is `2d3e9018b`, that `5ae5260c8` is NOT an ancestor of
it, and that the 18 other cherry-picked fixes ARE (patch-id or ancestry — the
sync doc `doc/superpowers/UpstreamSyncState.md` lists them). If any is missing,
STOP and report: the collapse premise fails and the plan needs revisiting.

- [ ] **Step 3: Build the ownership manifest**

Produce, as a checked-in-nowhere scratch file, three lists:
(a) every path where old-`rust` differs from `origin/static_h`;
(b) of those, the ones we own (the invariant's first bullet — expand the
    globs to concrete paths);
(c) the remainder, each classified as *upstream advanced past us* (we take
    upstream) or *unexplained* (STOP and report).
The third list existing and being empty-of-unexplained is the gate for Task 2.
Specifically resolve: which of the 15 added `test/` files are already upstream,
and whether `test/Sema/implicit-return-try-catch-finally.js` is ours to add.

- [ ] **Step 4: Report** — no commit yet. Report the three lists' sizes and any
  surprise.

---

### Task 2: Reconstruct as ~11 subsystem commits

**Files:** the new branch's whole content.

- [ ] **Step 1: Create the branch**

```bash
git -C /home/tmikov/work/hermes-rust checkout -b rust-squashed origin/static_h
```
Work on `rust-squashed`; `rust` is not touched until Task 5.

- [ ] **Step 2: Materialise the commits, in this order**

For each, `git checkout rust -- <paths>` then commit. Paths must be disjoint
and must together cover exactly list (b). The order is dependency-shaped so
each commit's message can stand alone:

1. **`hermes-support`, `atom_table`, `unicode`** — the support layer, plus
   `rust/Cargo.toml`, `rust/Cargo.lock`, `rust/rust-toolchain.toml`,
   `rust/.gitignore`, `rust/LICENSE`, `rust/NOTICE`.
2. **`hermes-ast`** — the node set, the `gen_nodes.py` generator, the arena,
   visitors, the ESTree JSON dumper.
3. **JS lexer** — `crates/parser/src/lexer/`, `token*.rs`, `cursor.rs`,
   `utf8.rs`, `html_entities.rs`, `number.rs`.
4. **JSON parser** — `crates/parser/src/json/`, `support/src/json_emitter.rs`.
5. **JS parser core** — `crates/parser/src/js/` minus the dialect dirs.
6. **Flow / TypeScript / JSX dialects** — `js/flow/`, `js/ts/`, `js/jsx.rs`.
7. **Pre/Lazy passes** — `js/pre_lazy.rs` and the reclamation machinery.
8. **Sema** — `crates/sema/` entire.
9. **C++ oracle tools + differential harnesses** — the 5 `tools/*` C++ tools,
   `tools/CMakeLists.txt`, `crates/tools/`, and the corpora/MANIFESTs.
10. **Local C++ deltas** — `lib/Sema/CheckImplicitReturn.cpp` (+ its test if
    ours), `lib/Sema/SemanticResolver.cpp`, `lib/CompilerDriver/CompilerDriver.cpp`,
    and the two juno `unsafe` removals. Message must explain each: the
    unlanded `5ae5260c8`, a cosmetic re-wrap, a defensive assignment of
    untraceable provenance, and two juno improvements.
11. **Docs, CI, tooling config** — `doc/`, `.github/workflows/rust*.yml`,
    `.gitignore`, and anything in list (b) not yet placed.

Each message: what the subsystem is, how it is validated (name the gate and its
figures), and any load-bearing design decision a reader needs. These are the
permanent record — write them for someone meeting the port for the first time.
No "Task N" or plan references.

- [ ] **Step 3: Nothing left behind**

`git diff --name-only rust rust-squashed` must list only paths where upstream
legitimately advanced past us. Anything in list (b) appearing here means a
commit missed it.

- [ ] **Step 4: Report** the commit list with one-line summaries and the
  Step 3 output.

---

### Task 3: Make it build and pass on the new base

The oracle binaries change — we gained 147 upstream commits including
FlowChecker work. **Nothing here is a formality.**

- [ ] **Step 1: Configure and build the C++ oracle from scratch**

Use a fresh build dir so no stale artifacts hide a problem:
`cmake -B cmake-build-squash -G Ninja -DCMAKE_BUILD_TYPE=Debug -DHERMES_ENABLE_ADDRESS_SANITIZER=ON -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ -DCMAKE_CXX_FLAGS="-O1" -DCMAKE_C_FLAGS="-O1"`
then build `hermesc sema-parser-dump json-parse-dump preparse-dump js-lexer-dump`.
Report any build failure in our tools caused by upstream API drift — that is a
real finding, not a nuisance.

- [ ] **Step 2: Run every gate and report the true figures**

Expected from the old base: sema **224 (111)** + parser-entry **17 (9)**;
parser 8/8; json 1/1; preparse 4/4; lexer 6/6; full workspace;
`RUSTFLAGS="-D warnings"`; `cargo publish --dry-run` all seven in one call.
**If a figure moved, do not "fix" the corpus to match.** Diagnose why: an
upstream commit changing untyped-path behavior is a finding to report and
understand before anything else happens.

- [ ] **Step 3: Remap the citations — and treat this as the tool's exam**

The C++ tree moved by 147 commits, so `cpp:NNNN` citations will be stale. Run
the citation checker's `remap`, then `check`. Report: how many were stale, how
many remapped, how many declined. Declines are expected where upstream
*changed* cited code rather than moving it — list them; they are the honest
output. Re-bless only after reviewing what changed, and say what you reviewed.

- [ ] **Step 4: The C++ lit suite**

`LIT_FILTER="Sema|Parser" cmake --build … --target check-hermes`, plus a full
`check-hermes`. Our `test/Sema` additions must still pass on the new base.

- [ ] **Step 5: Commit any fallout** as a clearly-labelled follow-up commit
  (not folded into the subsystem commits, so the reconstruction stays auditable).

---

### Task 4: Verify the invariant mechanically

- [ ] **Step 1: Owned paths are byte-identical to old `rust`**

For every path in list (b): `git diff rust-history-2026-08-14 rust-squashed -- <path>`
must be empty, EXCEPT paths Task 3 deliberately changed (the citation snapshot
after remap, and any documented fallout). Enumerate the exceptions explicitly
with their justification — an unexplained difference is a Critical finding.

- [ ] **Step 2: Everything else is byte-identical to `origin/static_h`**

`git diff origin/static_h rust-squashed -- . ':!rust' ':!doc'` must reduce to
exactly: the 5 oracle tools, `tools/CMakeLists.txt`, the 2 workflows,
`.gitignore`, the 3 local C++ deltas, the 2 juno files, and any test file
established as ours in Task 1 Step 3. Anything else is a defect.

- [ ] **Step 3: History sanity**

`git log --oneline origin/static_h..rust-squashed` is ~11 commits; every commit
builds is NOT required (that would need 11 full builds), but the TIP must, and
Task 3 established that. Confirm no commit is empty and no message references
plans or task numbers.

- [ ] **Step 4: Report** with both diffs' full output.

---

### Task 5: Switch `rust` over

Only after Task 4 is clean.

- [ ] **Step 1: Move the branch**

```bash
git -C /home/tmikov/work/hermes-rust branch -f rust rust-squashed
git -C /home/tmikov/work/hermes-rust checkout rust
git -C /home/tmikov/work/hermes-rust branch -D rust-squashed
```
The backup branch and tag remain.

- [ ] **Step 2: Re-point the release tag**

`hermes-crates-v0.1.0` points into the old history; the crate READMEs link
through it. Move it to the new tip and verify the READMEs' links still describe
reachable paths.

- [ ] **Step 3: Re-run the gates once more on `rust`** — cheap insurance that
  the branch move changed nothing.

- [ ] **Step 4: Record it**

`doc/superpowers/UpstreamSyncState.md`: the port now sits directly on
`origin/static_h` @ `2d3e9018b` plus the unlanded `5ae5260c8` and the local
deltas; the pre-squash history is at tag `rust-history-2026-08-14` and branch
`rust-presquash-backup`. Update `SESSION-HANDOFF.md` similarly — a future
session must not be confused by the vanished history.

- [ ] **Step 5: Commit** the doc update.

## Self-Review

- The invariant makes the reconstruction checkable rather than trusted.
- Task 3 is where the real risk lives (new oracle); it is explicitly told not
  to paper over a moved gate figure.
- Task 5 is deliberately last and trivial, so the dangerous work happens on a
  branch nobody depends on.
- The backup is created before anything and never deleted by this plan.
