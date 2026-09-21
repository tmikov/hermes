;; Copyright (c) Meta Platforms, Inc. and affiliates.
;;
;; This source code is licensed under the MIT license found in the
;; LICENSE file in the root directory of this source tree.

;; Test that wat2wasm produces a valid minimal Wasm binary.

;; REQUIRES: wasm
;; RUN: %wat2wasm %s -o %t.wasm && wc -c < %t.wasm | %FileCheck %s

(module)

;; CHECK: 8
