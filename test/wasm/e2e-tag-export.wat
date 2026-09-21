;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; A module's exported tag is a real WebAssembly.Tag.
;;
;; It used to be a plain object carrying its signature in an ordinary
;; `__wasm_type__` string property -- a spec gap as well as the hole
;; e2e-tag-import-brand.wat covers, because the JS API says a tag export is a
;; WebAssembly.Tag and several things require one. `new
;; WebAssembly.Exception(moduleTag, payload)` in particular could not work at
;; all: it starts with a dyn_vmcast to a Tag cell, which a plain object could
;; never pass. This file pins that it works now, including for the reference
;; parameter types JSWebAssemblyTag::ValType gained for this.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s --enable-exceptions -o %t.wasm && %hermesc --wasm -emit-binary -out %t.hbc %t.wasm && %hermes -Xhermes-internal-test-methods -Xenable-untrusted-bytecode-from-js %S/e2e-tag-export-driver.js_ -- %t.hbc | %FileCheck --match-full-lines %s

(module
  (tag (export "tag_empty"))
  (tag (export "tag_i32") (param i32))
  (tag (export "tag_f32") (param f32))
  (tag (export "tag_i32_f64") (param i32 f64))
  (tag (export "tag_ext") (param externref))
  (tag (export "tag_fn") (param funcref))
  ;; Declared separately with the SAME signature as tag_i32. Wasm tag identity
  ;; is nominal, so the two must still be distinct objects.
  (tag (export "tag_i32_b") (param i32))
  (func (export "f") (result i32) (i32.const 7))
)

;; Every exported tag is the same kind of object `new WebAssembly.Tag(...)`
;; produces, and none of them publishes anything.
;;
;; The @@toStringTag reads "Tag" where the spec and node say
;; "WebAssembly.Tag". That is pre-existing, applies to every WebAssembly class
;; rather than to tags, and is filed as 01a09d2d-2d1d; it is pinned here as
;; what Hermes does today, not endorsed.
;; CHECK: tag_empty: Tag true, toString [object Tag], published undefined
;; CHECK-NEXT: tag_i32: Tag true, toString [object Tag], published undefined
;; CHECK-NEXT: tag_f32: Tag true, toString [object Tag], published undefined
;; CHECK-NEXT: tag_i32_f64: Tag true, toString [object Tag], published undefined
;; CHECK-NEXT: tag_ext: Tag true, toString [object Tag], published undefined
;; CHECK-NEXT: tag_fn: Tag true, toString [object Tag], published undefined
;; CHECK-NEXT: tag_i32_b: Tag true, toString [object Tag], published undefined

;; Distinct tags stay distinct. This compares tag_i32 against tag_i32_b, which
;; are declared separately with the IDENTICAL signature (param i32) -- an
;; implementation that interned tags by signature would hand back one object
;; for both and fail here. Comparing differently-shaped tags would not have
;; caught that.
;; CHECK-NEXT: same signature, different tags: true
;; The second instance comes from the SAME Module object, so an implementation
;; caching tags on the Module would fail here; building a second Module would
;; have let that pass.
;; CHECK-NEXT: and across instances of the same module: true

;; new WebAssembly.Exception(moduleTag, ...) now works, and coerces each
;; payload value by its declared parameter type.
;; CHECK-NEXT: i32 payload 42.7 -> 42
;; CHECK-NEXT: f32 payload 0.1 -> 0.10000000149011612

;; A REFERENCE payload is passed through rather than run through ToNumber,
;; which for an object calls valueOf/toString and yields NaN, some unrelated
;; number, or an exception -- losing the reference every way. This arm only
;; became reachable when module tags became Tag cells.
;; CHECK-NEXT: externref payload is the very object: true
;; CHECK-NEXT: funcref payload is the exported function: true

;; and a funcref payload admits only null or an Exported Function, the same
;; admission the funcref table and global funnels make.
;; CHECK-NEXT: funcref payload null: true
;; CHECK-NEXT: funcref payload a plain function: TypeError
;; CHECK-NEXT: done
