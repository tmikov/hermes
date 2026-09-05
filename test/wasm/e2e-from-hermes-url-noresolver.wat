;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; WebAssembly.Module.fromHermesURL exists unconditionally (it is not gated),
;; but with no embedder-installed resolver there is nothing to resolve the URL
;; to, so it throws. The positive path needs a natively installed resolver and
;; is covered by the API tests.

;; REQUIRES: wasm
;; RUN: %hermes -Xhermes-internal-test-methods %S/e2e-from-hermes-url-noresolver-driver.js_ | %FileCheck --match-full-lines %s

;; CHECK: fromHermesURL is function: true
;; CHECK-NEXT: no resolver: TypeError
;; CHECK-NEXT: done
