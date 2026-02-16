You are running autonomously in an unattended loop. NEVER ask the user
questions — make reasonable decisions and document them.

## Setup

1. Read `CLAUDE.md` for project conventions and build configuration.
2. Read `history/wasm/memory.md` for accumulated project knowledge.
3. Read `history/wasm/project.md` for project context and architecture.
4. Read `history/wasm/progress.md` to find the current state of all tasks.
5. If there is a task marked `wip`, resume it — review what was already done
   (check git log, existing code) and continue from where it left off.
6. Otherwise, identify the next pending task whose dependencies are all
   `done`. If multiple tasks are unblocked, pick the lowest step number.
7. If no tasks can be started (all pending tasks have unmet dependencies, or
   all tasks are done), print exactly `RALPH_DONE` and stop immediately.
8. Immediately mark the chosen task as `wip` in progress.md before doing
   anything else. Do not commit — this is a crash-recovery marker only.

## Implement

9. Read `history/wasm/WasmImplementationPlan.md` to find
   the specification for **your chosen task only** (each Fix N in the plan
   corresponds to one row in progress.md). Do NOT read the plan as a
   sequential workflow — implement only the single task you picked in
   step 6, then proceed to Finish.
10. Follow all conventions in `CLAUDE.md` (build config, code style, etc.).
11. Build and run tests as described in `CLAUDE.md`. All builds must succeed
    and all tests must pass.

## Finish

12. Update `history/wasm/progress.md`:
    - Mark the task as `done` in the Status table.
    - Add a Context Notes entry following the format documented in the
      progress file itself.
13. Update `history/wasm/memory.md` with anything future sessions should
    know: gotchas, API quirks, build issues, patterns, conventions learned.
    This is NOT optional — every task teaches something. Review what you
    did and write it down. Be as concise as possible — memory consumes
    tokens on every session, so keep entries terse and remove outdated ones.
14. Commit all changes (implementation + progress update + memory) with a
    descriptive message. Include the step ID, e.g.: "Step 1: Create repo
    and CMake scaffolding".

## Rules

- Only implement ONE task per session. Stop after committing.
- NEVER ask the user for input or clarification. You are unattended.
- If a task cannot be completed (build fails after reasonable attempts,
  unclear spec, blocked on something unexpected), mark it as `blocked` in
  progress.md with a brief explanation, commit any partial work, print
  `RALPH_BLOCKED: <reason>`, and stop.
- Follow the code style and patterns described in `CLAUDE.md`.
