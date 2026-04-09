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
| P1-S2 | Type queries on IRTypeContext | P1-S1 | done | |
| P1-S3 | Type operations with interning | P1-S2 | done | |
| P1-S4 | Utility methods | P1-S3 | | |
| P1-S5 | Thread-local context and RAII guard | P1-S1 | | |
| P1-S6 | Wire IRTypeContext into Module | P1-S1 | | |
| P1-S7 | Install RAII guards at compilation entry points | P1-S5, P1-S6 | | |
| P1-S7.5 | Add RAII guards to unit tests | P1-S7 | | |
| P1-S8 | Rewrite Type class | P1-S4, P1-S7.5 | | |

## Context Notes

### P1-S2: Type queries on IRTypeContext
- **Files**: modified `include/hermes/IR/IRTypeContext.h`, `lib/IR/IRTypeContext.cpp`, `unittests/IR/IRTypeContextTest.cpp`.
- **What was done**: Added `canBeNumber`, `canBeString`, `canBeObject`, `canBeNull`, `canBeUndefined`, `canBeEmpty`, `canBeUninit`, `canBeBigInt`, `canBeBoolean`, `canBeSymbol`, `isNoType`, `isPrimitive`, `canBePrimitive`, `isNonPtr`. Private helpers `containsMatchingKind` and `allMatchKind` take predicate functions for leaf/union dispatch. File-scoped kind helpers (`isNumberKind`, `isObjectKind`, `isPrimitiveKind`, `isNonPtrKind`) encode subtype relationships. 7 new unit tests (13 total).
- **Decisions**:
  - Well-known ID fast paths in each `canBeX` method, per the plan spec. Avoids table lookups for common types.
  - Kind helpers are subtype-aware: `isNumberKind` includes Int32/Uint32, `isObjectKind` includes ClassInstance/Array/Tuple/Function/ExactObject, `isPrimitiveKind` and `isNonPtrKind` include number subtypes. Refined kinds are dead code in Phase 1 but the predicates are correct when they activate.
  - `isPrimitive`/`isNonPtr` return false for NoType, matching the old bitmask semantics (`bitmask_ && !(bitmask_ & ~BITS)`).

### P1-S3: Type operations with interning
- **Files**: modified `include/hermes/IR/IRTypeContext.h`, `lib/IR/IRTypeContext.cpp`, `unittests/IR/IRTypeContextTest.cpp`.
- **What was done**: Added `isSubsetOf`, `areDisjoint`, `unionTy`, `intersectTy`, `subtractTy` public methods. Added `UnionInternKey`/`UnionInternKeyInfo` structs for DenseMap-based union interning. Private `createUnionImpl` handles full canonicalization (flatten, sort, dedup, subsume, intern). Static helpers `isLeafSubtype` and `areLeafKindsDisjoint` handle leaf-level type relationships including future-proof Number/Object family rules. Intern table pre-populated with well-known unions in constructor. 10 new unit tests (23 total IRTypeContext tests).
- **Decisions**:
  - Used `DenseMap<UnionInternKey, uint32_t, UnionInternKeyInfo>` with SmallVector<uint32_t, 8> key and custom DenseMapInfo (sentinels use size-1 vectors with UINT32_MAX/UINT32_MAX-1, real keys always size >= 2).
  - `isLeafSubtype` handles Int32/Uint32 <: Number and object refinements <: Object even though unused in Phase 1 — keeps algorithms correct when refined types arrive.
  - `areLeafKindsDisjoint` uses the subtype check plus Number-family overlap rule; all other distinct kinds are disjoint.
  - `subtractTy` returns conservative approximation (returns `a` unchanged) when a is a leaf that's not subset of b and not disjoint from b.
  - `intersectTy` distributes over unions recursively; base case for two non-subset number-family leafs returns `kInt31Id` (the only such overlap); other non-subset leafs return NoType.
  - Added `TypeKind::Int31` (Int32 ∩ Uint32, integers in [0, 2^31-1]) to close the lattice gap. Int31 <: Int32, Int31 <: Uint32, Int31 <: Number. Pre-allocated as well-known IDs: kInt32Id=15, kUint32Id=16, kInt31Id=17 (shifted union IDs up by 3). 5 new unit tests (28 total).

### P1-S1: IRTypeContext skeleton with well-known types
- **Files**: created `include/hermes/IR/IRTypeContext.h`, `lib/IR/IRTypeContext.cpp`, `unittests/IR/IRTypeContextTest.cpp`; modified `lib/CMakeLists.txt`, `unittests/IR/CMakeLists.txt`.
- **Decisions**:
  - TypeEntry uses raw `uint32_t` for all type references (not `Type`) since `Type` is still a bitmask. Changed to `Type` in P1-S8.
  - Added `kNullOrUndefId` (id=18) as a well-known union, needed for InstSimplify.cpp migration in P1-S8.
  - Reserved IDs 19-31 padded with NoType placeholders.
  - Union arm arrays stored in sorted order by ID for AnyType and AnyEmptyUninit.
- **What was done**: Created IRTypeContext with TypeKind enum (all kinds including deferred refined types), TypeEntry tagged-union struct, well-known ID constants, constructor that pre-allocates 15 leaf types and 4 well-known unions, getKind() and getUnionArms() accessors. Added to hermesFrontend build target. 6 unit tests.

