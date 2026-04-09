# Implementation Memory

Non-obvious gotchas and patterns.

## Build
- ASan build: `cmake -B cmake-build-asan -G Ninja -DCMAKE_BUILD_TYPE=Debug -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ -DHERMES_ENABLE_ADDRESS_SANITIZER=ON -DCMAKE_CXX_FLAGS="-O1" -DCMAKE_C_FLAGS="-O1"`
- IRTypeContext.cpp is part of `hermesFrontend` target in `lib/CMakeLists.txt`.
- Unit tests link `hermesFrontend` (see `unittests/IR/CMakeLists.txt`).

## API Design
- All IRTypeContext public methods use `uint32_t` IDs (not `Type`). Will change to `Type` in P1-S8.
- TypeEntry union payloads also use `uint32_t` for type refs (not `Type`), since `Type` is still a 2-byte bitmask.
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
