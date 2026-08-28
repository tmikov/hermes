# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

set(WasmFrontendSources
  WasmCompileTest.cpp
  )

add_hermes_unittest(WasmFrontendTests
  ${WasmFrontendSources}
  )

target_link_libraries(WasmFrontendTests
  hermesWasmFrontend
  wabt
  )
