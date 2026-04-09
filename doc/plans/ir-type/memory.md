# Implementation Memory

Non-obvious gotchas and patterns.

## Build
- ASan build: `cmake -B cmake-build-asan -G Ninja -DCMAKE_BUILD_TYPE=Debug -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ -DHERMES_ENABLE_ADDRESS_SANITIZER=ON -DCMAKE_CXX_FLAGS="-O1" -DCMAKE_C_FLAGS="-O1"`
- IRTypeContext.cpp is part of `hermesFrontend` target in `lib/CMakeLists.txt`.
- Unit tests link `hermesFrontend` (see `unittests/IR/CMakeLists.txt`).

## API Design
- IRTypeContext public methods still use `uint32_t` IDs internally. `Type` wraps a `uint32_t id_` and delegates to `IRTypeContext::current()`.
- Well-known IDs 0-21 assigned (0-17 leaves, 18-21 unions), 22-31 reserved (NoType padding), kFirstDynamicId=32.
- The `TypeKind` enum is `enum class` (scoped), while the old `Type::TypeKind` is a plain enum inside the `Type` class. They coexist until P1-S8.
- Primitive kinds: Number, Int32, Uint32, Int31, String, BigInt, Null, Undefined, Boolean, Symbol. Matches old `PRIMITIVE_BITS` plus number subtypes.
- NonPtr kinds: Number, Int32, Uint32, Int31, Boolean, Null, Undefined. Matches old `NONPTR_BITS` plus number subtypes.
- `isPrimitive`/`isNonPtr` return false for NoType (matching old `bitmask_ &&` guard).
- `containsMatchingKind`/`allMatchKind` are private template helpers taking a predicate — used by `canBeX`/`isPrimitive`/`isNonPtr`.
- Kind helpers (`isNumberKind`, `isObjectKind`, `isPrimitiveKind`, `isNonPtrKind`) encode subtype relationships: Int32/Uint32 are number subtypes; ClassInstance/Array/Tuple/Function/ExactObject are object refinements.
- Each `canBeX` has well-known ID fast paths before falling back to `containsMatchingKind`.

## Interning & Type Operations
- Union intern table uses `DenseMap<UnionInternKey, uint32_t, UnionInternKeyInfo>`. Key is sorted arm IDs in SmallVector<uint32_t, 8>. Sentinels are size-1 vectors (UINT32_MAX / UINT32_MAX-1); real keys always >= 2 elements.
- Pre-populated for 4 well-known unions in constructor. `unionTy(Number, BigInt)` correctly returns `kNumericId`.
- `createUnionImpl` is the canonicalization workhorse: flatten → sort → dedup → subsume → intern. Only called when identity/subset shortcuts in `unionTy` don't fire.
- `isSubsetOf` and `areDisjoint` are `const`; `unionTy`/`intersectTy`/`subtractTy` are non-const (may create entries).
- Static helpers `isLeafSubtype` and `areLeafKindsDisjoint` are file-scoped in IRTypeContext.cpp (anonymous namespace).
- `intersectTy` and `subtractTy` distribute over unions via recursive calls + `unionTy` to reassemble results.
- `intersectTy` leaf-leaf case: returns `kInt31Id` for overlapping number-family types (Int32 ∩ Uint32), NoType for all other non-subset pairs. This closes the lattice so `intersectTy` and `areDisjoint` are consistent.
- `Int31` (integers in [0, 2^31-1]) is the intersection of Int32 and Uint32. Subtype rules: Int31 <: Int32, Int31 <: Uint32, Int31 <: Number. Pre-allocated at kInt32Id=15, kUint32Id=16, kInt31Id=17.
- **Iterator invalidation**: `intersectTy` and `subtractTy` must copy union arms to a `SmallVector` before calling `unionTy`, because `unionTy` may reallocate `typeArrays_` and invalidate the `ArrayRef` from `getUnionArms`.

## Thread-Local Context
- `IRTypeContext::current_` is `static thread_local`, defined in `IRTypeContext.cpp`, initialized to `nullptr`.
- `IRTypeContextRAII` is a friend of `IRTypeContext` to access `current_`. It saves/restores the pointer, supporting nesting.
- The test target name for IR unit tests is `HermesIRTests` (not `IRTypeContextTest`).

## Module Integration
- `IRTypeContext.h` included in `IR.h` at the top (no dependency on `Type`).
- `Module` has `IRTypeContext typeContext_` member, default-constructed. Access via `getTypeContext()`.
- `Module` class starts at ~line 2538 in IR.h (after many other classes: SideEffect, Value, Instruction, etc.).

## RAII Guard Sites
- Production: `BCProviderFromSrc.cpp:233`, `CompilerDriver.cpp:1983`, `shermes.cpp:911`, `HBC.cpp` (lazy: `compileLazyFunctionWorker`, eval: `compileEvalWorker`).
- ALL test files creating Module need guards — not just those using `Type::` directly.
- No explicit include of `IRTypeContext.h` needed — `IR.h` already includes it (from P1-S6).

## Type Class (after P1-S8 rewrite)
- `Type` is now 4 bytes (`uint32_t id_`), not 2 bytes. `IRTypeContext` is a `friend` of `Type`.
- All non-trivial `Type` methods defined in `IRTypeContext.cpp` (not `IR.h`) to avoid include-order coupling.
- `getUnionArms` is out-of-line (needs complete `Type` definition for pointer arithmetic).
- `typeArrays_` is `vector<Type>` (not `vector<uint32_t>`). Layout-compatible since `sizeof(Type) == sizeof(uint32_t)`.
- Old nested `Type::TypeKind` enum removed. Callers use `TypeKind::` (the `enum class` from `IRTypeContext.h`).
- `Type::LAST_TYPE` removed. SH.cpp uses `default:` instead.
- `constexpr` operations (`createNoType`, `createAnyType`, etc.) use well-known IDs directly. Non-constexpr operations (`unionTy`, etc.) delegate to `IRTypeContext::current()`.

## Formatting
- `format` uses "any" shorthand when `isSubsetOf(kAnyTypeId, id)`.
- `kindName()` helper is file-scoped in `IRTypeContext.cpp`, covers all `TypeKind` values.
- `llvh::raw_ostream` forward-declared in `IRTypeContext.h`.
