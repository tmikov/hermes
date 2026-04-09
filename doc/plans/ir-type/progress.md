# Implementation Progress

Tracks progress of `doc/plans/ir-type/phase1-steps.md` for
`doc/plans/ir-type/ir-type-system-v2-design.md` and
`doc/plans/ir-type/ir-type-v2-implementation.md`.

The file has two sections: "Status" and "Context Notes".

**Status Section**:

Each row contains the step label from the detailed, plan, a brief description, list of
dependency labels, Status (initially empty), optional brief note (initially empty).

The status of a row is one of:
- "" (empty) initially, before work has started
- "wip" as soon as work on that raw has started.
- "done" when work has completed successfully. Rarely "Brief Note" may contain very brief
explanation. More details in "Context Notes".
- "blocked" when work cannot proceed for some reason. "Brief Note" must contain a brief
explanation. More details in "Context Notes".

Example on start:

| Step 11 | Port util binding | 5 |  |  |

**Context Notes**:

After completing work on a step, either successfully or by blocking, a section for that
step is added. It needs to have the format from the following example (empty bullets can
be omitted):

```
### Step 11: Port util binding
- **Files**: created `foo.c`, modified `bar.c`.
- **Decisions**:
-- Decision 1 concise explanation
-- Decision 2 concise explanation
- **What was done**: ...
- **Issues**: ...
- **Notes for next step**: ...
```

## Status

| Step | Description | Depends On | Status | Brief Note (optional) |
|------|-------------|------------|--------|-----------------------|
| P1-S1 | IRTypeContext skeleton with well-known types | — | done | |
| P1-S2 | Type queries on IRTypeContext | P1-S1 | | |
| P1-S3 | Type operations with interning | P1-S2 | | |
| P1-S4 | Utility methods | P1-S3 | | |
| P1-S5 | Thread-local context and RAII guard | P1-S1 | | |
| P1-S6 | Wire IRTypeContext into Module | P1-S1 | | |
| P1-S7 | Install RAII guards at compilation entry points | P1-S5, P1-S6 | | |
| P1-S7.5 | Add RAII guards to unit tests | P1-S7 | | |
| P1-S8 | Rewrite Type class | P1-S4, P1-S7.5 | | |

## Context Notes

### P1-S1: IRTypeContext skeleton with well-known types
- **Files**: created `include/hermes/IR/IRTypeContext.h`, `lib/IR/IRTypeContext.cpp`, `unittests/IR/IRTypeContextTest.cpp`; modified `lib/CMakeLists.txt`, `unittests/IR/CMakeLists.txt`.
- **Decisions**:
  - TypeEntry uses raw `uint32_t` for all type references (not `Type`) since `Type` is still a bitmask. Changed to `Type` in P1-S8.
  - Added `kNullOrUndefId` (id=18) as a well-known union, needed for InstSimplify.cpp migration in P1-S8.
  - Reserved IDs 19-31 padded with NoType placeholders.
  - Union arm arrays stored in sorted order by ID for AnyType and AnyEmptyUninit.
- **What was done**: Created IRTypeContext with TypeKind enum (all kinds including deferred refined types), TypeEntry tagged-union struct, well-known ID constants, constructor that pre-allocates 15 leaf types and 4 well-known unions, getKind() and getUnionArms() accessors. Added to hermesFrontend build target. 6 unit tests.

