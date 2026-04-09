# Implementation Memory

Non-obvious gotchas and patterns.

## Build
- ASan build: `cmake -B cmake-build-asan -G Ninja -DCMAKE_BUILD_TYPE=Debug -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ -DHERMES_ENABLE_ADDRESS_SANITIZER=ON -DCMAKE_CXX_FLAGS="-O1" -DCMAKE_C_FLAGS="-O1"`
- IRTypeContext.cpp is part of `hermesFrontend` target in `lib/CMakeLists.txt`.
- Unit tests link `hermesFrontend` (see `unittests/IR/CMakeLists.txt`).

## API Design
- All IRTypeContext public methods use `uint32_t` IDs (not `Type`). Will change to `Type` in P1-S8.
- TypeEntry union payloads also use `uint32_t` for type refs (not `Type`), since `Type` is still a 2-byte bitmask.
- Well-known IDs 0-18 assigned, 19-31 reserved (NoType padding), kFirstDynamicId=32.
- The `TypeKind` enum is `enum class` (scoped), while the old `Type::TypeKind` is a plain enum inside the `Type` class. They coexist until P1-S8.
- Primitive kinds: Number, Int32, Uint32, String, BigInt, Null, Undefined, Boolean, Symbol. Matches old `PRIMITIVE_BITS` plus number subtypes.
- NonPtr kinds: Number, Int32, Uint32, Boolean, Null, Undefined. Matches old `NONPTR_BITS` plus number subtypes.
- `isPrimitive`/`isNonPtr` return false for NoType (matching old `bitmask_ &&` guard).
- `containsMatchingKind`/`allMatchKind` are private helpers taking a predicate — used by `canBeX`/`isPrimitive`/`isNonPtr`.
- Kind helpers (`isNumberKind`, `isObjectKind`, `isPrimitiveKind`, `isNonPtrKind`) encode subtype relationships: Int32/Uint32 are number subtypes; ClassInstance/Array/Tuple/Function/ExactObject are object refinements.
- Each `canBeX` has well-known ID fast paths before falling back to `containsMatchingKind`.
